//! The output sets of a track's instrument plugin (docs/architecture.md 20-15): what
//! get_mixer shows of them, their settings, and each measured alone. Pans are in the unit of
//! the strips' pan: -100 (all left) to 100 (all right).

use meloet_core::ai::{
    AiAnswer, AiAsk, AiOutputLevels, AiOutputsMeasure, AiRefusal, AiSong, OutputSet,
};
use meloet_model::{Gain, InstrumentRef, OutputChange, OutputSetting, Pan, PluginRef, Track};
use rmcp::model::{CallToolResult, ProgressNotificationParam, RequestMetaObject};
use rmcp::{
    ErrorData as McpError, Peer, RoleServer, handler::server::wrapper::Parameters, schemars, tool,
    tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{Tools, UNSAVED, analyze, closing, done, failed, levels_value, refusal_text, shown_db};
use crate::trackref::{TrackRef, id_text};

/// What `measure_plugin_outputs` says it measured.
const MEASURED: &str = "each output alone at 0 dB in its default position, before its own \
volume, pan and on/off and before the track's effects and fader";

/// Characters of an output's id.
const MAX_ID: usize = 256;

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct OutputArgs {
    /// A track whose instrument is a plugin: its number (from 1) or its id ("id:7").
    pub track: TrackRef,
    /// The output's id, as get_mixer lists it in the track's `outputs`.
    #[schemars(length(min = 1, max = 256))]
    pub output: String,
    /// dB, -60 to 6. To silence the output, give `on: false`.
    #[serde(default)]
    #[schemars(range(min = -60.0, max = 6.0))]
    pub volume_db: Option<f64>,
    /// -100 (all left) to 100 (all right); 0 is the middle. Where a mono output sounds; the
    /// balance of a stereo pair.
    #[serde(default)]
    #[schemars(range(min = -100, max = 100))]
    pub pan: Option<i32>,
    /// false takes the output out of the mix; true puts it in.
    #[serde(default)]
    pub on: Option<bool>,
    /// true: back to the defaults first (0 dB, the plugin's own pan and on/off); the other
    /// fields then apply.
    #[serde(default)]
    pub reset: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MeasureOutputsArgs {
    /// A track whose instrument is a plugin: its number (from 1) or its id ("id:7").
    pub track: TrackRef,
    /// The first bar (from 1).
    #[schemars(range(min = 1, max = 9999))]
    pub from_bar: u32,
    /// The last bar, measured to its end.
    #[schemars(range(min = 1, max = 9999))]
    pub to_bar: u32,
}

/// A pan of the plugin host (-1 to 1) as the tools give pans.
fn host_pan(pan: f32) -> i32 {
    ((f64::from(pan) * 100.0).round() as i32).clamp(-100, 100)
}

fn volume_db(setting: Option<&OutputSetting>) -> f64 {
    setting.map_or(0.0, |s| s.gain.db().unwrap_or(Gain::MIN_DB))
}

/// One output set with the values in force: the song's setting, else the default.
fn set_value(set: &OutputSet, setting: Option<&OutputSetting>) -> Value {
    json!({
        "id": set.id,
        "name": set.name,
        "channels": set.channels,
        "on": setting.and_then(|s| s.on).unwrap_or(set.on),
        "volume_db": volume_db(setting),
        "pan": setting.and_then(|s| s.pan).map_or(host_pan(set.pan), |p| i32::from(p.get())),
        "default": { "on": set.on, "pan": host_pan(set.pan) },
    })
}

/// A setting the song keeps of an output that is not known now; `null` where the default
/// rule decides.
fn kept_value(setting: &OutputSetting) -> Value {
    json!({
        "id": &*setting.id,
        "on": setting.on,
        "volume_db": volume_db(Some(setting)),
        "pan": setting.pan.map(|p| p.get()),
    })
}

/// The output sets the loaded plugin of the track has; `None` while it is not loaded.
fn sets_of<'a>(read: &'a AiSong, track: &Track) -> Option<&'a [OutputSet]> {
    (read.instrument_sets.iter())
        .find(|(id, _)| *id == track.id)
        .map(|(_, sets)| &sets[..])
}

fn plugin_of(track: &Track) -> Option<&PluginRef> {
    match &track.instrument {
        InstrumentRef::Plugin(p) if !track.kind.is_vocal() => Some(p),
        _ => None,
    }
}

/// What get_mixer adds to a track whose instrument is a plugin: `outputs` (`null` while the
/// plugin is not loaded) and, when there are any, the settings the song keeps of outputs the
/// plugin does not have (all of them while it is not loaded).
pub(super) fn add_track_outputs(read: &AiSong, track: &Track, value: &mut Value) {
    let Some(plugin) = plugin_of(track) else {
        return;
    };
    let sets = sets_of(read, track);
    value["outputs"] = match sets {
        Some(sets) => (sets.iter())
            .map(|s| set_value(s, plugin.outputs.get(&s.id)))
            .collect(),
        None => Value::Null,
    };
    let without: Vec<Value> = (plugin.outputs.sets.iter())
        .filter(|kept| sets.is_none_or(|sets| !sets.iter().any(|s| *s.id == *kept.id)))
        .map(kept_value)
        .collect();
    if !without.is_empty() {
        value["output_settings_without_output"] = json!(without);
    }
}

/// The output `id` of the track as get_mixer would give it; what is not known is `null`.
fn output_value(read: &AiSong, track: &Track, id: &str) -> Value {
    let setting = plugin_of(track).and_then(|p| p.outputs.get(id));
    let set = sets_of(read, track).and_then(|sets| sets.iter().find(|s| s.id == id));
    match set {
        Some(set) => set_value(set, setting),
        None => json!({
            "id": id,
            "name": null,
            "channels": null,
            "on": setting.and_then(|s| s.on),
            "volume_db": volume_db(setting),
            "pan": setting.and_then(|s| s.pan).map(|p| p.get()),
            "default": null,
        }),
    }
}

fn not_a_plugin(number: u32, track: &Track, what: &str) -> CallToolResult {
    failed(format!(
        "track {number} \"{}\" has no instrument plugin; only the outputs of an instrument \
         plugin can be {what} (get_mixer shows each track's instrument)",
        track.name
    ))
}

fn level_fields(levels: Option<&meloet_core::ai::PassLevels>, rate: u32) -> Value {
    match levels {
        Some(l) => levels_value(l, rate),
        None => json!({
            "peak_dbfs": null,
            "rms_dbfs": null,
            "lufs": null,
            "short_term_max_lufs": null,
            "over_0db_places": null,
            "seconds": null,
        }),
    }
}

#[tool_router(router = outputs_router, vis = "pub(super)")]
impl Tools {
    /// Sets one output of a track's instrument plugin: volume, pan, on, or reset; one undo
    /// step. Settings stay with the output's id when the plugin loads another kit. on false
    /// takes it out of the mix (VST3 switches the bus off; the plugin may then send it to its
    /// main output). Pan: position if mono, balance if stereo. Not automatable.
    #[tool(annotations(
        title = "Set plugin output",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_plugin_output(
        &self,
        Parameters(args): Parameters<OutputArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let id = args.output.trim().to_owned();
        if id.is_empty() || id.chars().count() > MAX_ID || id.chars().any(char::is_control) {
            return Ok(failed(format!(
                "`output` is an output's id from get_mixer, 1 to {MAX_ID} characters"
            )));
        }
        if args.volume_db.is_none() && args.pan.is_none() && args.on.is_none() && !args.reset {
            return Ok(failed(
                "give at least one of `volume_db`, `pan`, `on` or `reset: true`",
            ));
        }
        if let Some(db) = args.volume_db
            && !(-60.0..=6.0).contains(&db)
        {
            return Ok(failed(
                "the volume is -60 to 6 dB; to silence the output give `on: false`",
            ));
        }
        if args.pan.is_some_and(|p| !(-100..=100).contains(&p)) {
            return Ok(failed("the pan is -100 to 100"));
        }
        let read = check!(self.read().await);
        let (number, track) = check!(args.track.find(&read).map_err(failed));
        if plugin_of(track).is_none() {
            return Ok(not_a_plugin(number, track, "set"));
        }
        let ask = AiAsk::SetPluginOutput {
            track: track.id.0,
            output: id.clone(),
            change: OutputChange {
                gain: args.volume_db.map(Gain::from_db),
                pan: args.pan.map(|p| Some(Pan::new(p))),
                on: args.on.map(Some),
                reset: args.reset,
            },
        };
        let what = ask.name();
        let (changed, step) = match check!(self.answer(ask).await) {
            AiAnswer::Done(d) => (true, d.step),
            AiAnswer::Refused(AiRefusal::Unchanged) => (false, None),
            AiAnswer::Refused(AiRefusal::Unsaved) => return Ok(failed(UNSAVED)),
            AiAnswer::Refused(r) => {
                return Ok(failed(format!("Could not {what}: {}.", refusal_text(&r))));
            }
            _ => return Ok(closing()),
        };
        let after = check!(self.read().await);
        let (number, track) = check!(TrackRef::Id(track.id.0).find(&after).map_err(failed));
        let value = json!({
            "changed": changed,
            "step": step,
            "undo_steps": after.undo_count,
            "track": number,
            "track_id": id_text(track.id),
            "track_name": track.name,
            "output": output_value(&after, track, &id),
        });
        let summary = match changed {
            true => format!("Done: {what}."),
            false => "No change: the song already is like that.".to_owned(),
        };
        Ok(done(format!("{summary}\n{value}"), value))
    }

    /// Renders bars of one track alone like measure_mix (mute and solo ignored) and gives the
    /// levels of each output of its instrument plugin alone: peak, RMS, LUFS. The values are
    /// before the output's own volume, pan and on/off, and before the track's effects and
    /// fader. Numbers only; changes nothing.
    #[tool(annotations(
        title = "Measure plugin outputs",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn measure_plugin_outputs(
        &self,
        Parameters(args): Parameters<MeasureOutputsArgs>,
        meta: RequestMetaObject,
        peer: Peer<RoleServer>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let (number, track) = check!(args.track.find(&read).map_err(failed));
        if plugin_of(track).is_none() {
            return Ok(not_a_plugin(number, track, "measured"));
        }
        let range = check!(analyze::measure_ask(
            &read,
            args.from_bar,
            args.to_bar,
            None,
            None,
            true
        ));
        let ask = AiOutputsMeasure {
            span: range.span,
            track: number,
        };
        let levels = check!(self.run_measure_outputs(ask, &meta, &peer, &ct).await);
        let after = check!(self.read().await);
        let rate = levels.sample_rate;
        let (n, name) = &levels.track;
        let found = TrackRef::Number(*n).find(&after).ok().map(|(_, t)| t);
        let plugin = found.and_then(|t| plugin_of(t));
        let setting = |id: &str| plugin.and_then(|p| p.outputs.get(id));
        let outputs: Vec<Value> = (levels.sets.iter())
            .map(|o| {
                let mut v = set_value(&o.set, setting(&o.set.id));
                let fields = level_fields(o.levels.as_ref(), rate);
                for (key, field) in fields.as_object().into_iter().flatten() {
                    v[key] = field.clone();
                }
                v
            })
            .collect();
        let mut lines = vec![format!(
            "Track {n} \"{name}\", bars {} to {}, {MEASURED}:",
            args.from_bar, args.to_bar
        )];
        for o in &outputs {
            let (id, name) = (o["id"].as_str().unwrap_or(""), o["name"].as_str());
            lines.push(match o["seconds"].is_null() {
                true => format!(
                    "{id} \"{}\": not measured (the plugin kept nothing of it).",
                    name.unwrap_or("")
                ),
                false => format!(
                    "{id} \"{}\": peak {}, RMS {}, {} LUFS.",
                    name.unwrap_or(""),
                    shown_db(&o["peak_dbfs"]),
                    shown_db(&o["rms_dbfs"]),
                    analyze::shown_lufs(&o["lufs"]),
                ),
            });
        }
        let value = json!({
            "from_bar": args.from_bar,
            "to_bar": args.to_bar,
            "sample_rate": rate,
            "track": {
                "number": n,
                "id": crate::trackref::id_of(&after, *n),
                "name": name,
            },
            "measured": MEASURED,
            "outputs": outputs,
        });
        lines.push(value.to_string());
        Ok(done(lines.join("\n"), value))
    }
}

impl Tools {
    /// Measures `ask` on a thread of the blocking pool, telling the progress when a
    /// `progressToken` came.
    async fn run_measure_outputs(
        &self,
        ask: AiOutputsMeasure,
        meta: &RequestMetaObject,
        peer: &Peer<RoleServer>,
        ct: &CancellationToken,
    ) -> Result<AiOutputLevels, CallToolResult> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u8>();
        let meloet = self.meloet.clone();
        let stop = ct.clone();
        let mut job = tokio::task::spawn_blocking(move || {
            let mut progress = |percent| {
                let _ = tx.send(percent);
            };
            meloet.measure_outputs(ask, &mut progress, &|| stop.is_cancelled())
        });
        let token = meta.get_progress_token();
        let mut last = None;
        let answer = loop {
            tokio::select! {
                answer = &mut job => break answer,
                Some(percent) = rx.recv() => {
                    if let Some(token) = &token
                        && last.is_none_or(|l| percent > l)
                    {
                        last = Some(percent);
                        let note = ProgressNotificationParam::new(token.clone(), f64::from(percent))
                            .with_total(100.0);
                        let _ = peer.notify_progress(note).await;
                    }
                }
            }
        };
        match answer {
            Ok(Ok(levels)) => Ok(levels),
            Ok(Err(r)) => Err(failed(format!("Could not measure: {}.", refusal_text(&r)))),
            Err(_) => Err(closing()),
        }
    }
}
