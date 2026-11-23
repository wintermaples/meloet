//! Surge XT's modulation routing and the parameters of its patch that are not in the
//! plugin's parameter list as they are written (`crate::surge`). Each write reads the
//! plugin's state now, edits its patch XML by the bytes it means, and puts the state back as
//! one undo step; the answer is the patch as the plugin says it after.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_core::ai::{AiAnswer, AiAsk, AiParams, AiSong};
use meloet_model::PluginPlace;
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::mixer::BusArg;
use super::plugstate::{PluginStateGot, off_thread, plugin_place};
use super::{Tools, done, done_value, failed};
use crate::surge::patch::{self, ParamEdit, Patch, Routing};
use crate::surge::{Format, format_of, param_id, sources, with_xml, xml_of};

/// Edits of parameters one call takes.
const MOST_EDITS: usize = 64;
/// How long the plugin's list of parameters is waited for (only for their names).
const NAMES_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ListArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// Only routings whose target's patch name holds this text (any case), like "pitch".
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub target: Option<String>,
    /// Only routings from this source: its number, short name ("vlfo_1") or name ("LFO 1").
    #[serde(default)]
    pub source: Option<Value>,
    /// Routings in this answer (default 100, at most 500).
    #[serde(default)]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ParamsArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// Only parameters whose patch name holds this text (any case), like "a_lfo0_".
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub name: Option<String>,
    /// Parameters in this answer (default 50, at most 500).
    #[serde(default)]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ParamEditArg {
    /// The parameter's name in the patch, like "a_lfo0_rate".
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
    /// The value in the patch's own units (see surge_get_patch_params). An LFO's shape
    /// (`…_shape`): 0 sine, 1 triangle, 2 square, 3 ramp, 4 noise, 5 sample & hold, 6
    /// envelope, 7 step sequencer, 8 MSEG, 9 formula; its trigger mode (`…_trigmode`): 0
    /// free running, 1 from each key, 2 random.
    #[serde(default)]
    pub value: Option<f64>,
    /// For an LFO rate (`…_lfoN_rate`): the rate in Hz, in place of `value`.
    #[serde(default)]
    pub hz: Option<f64>,
    /// For an LFO's delay, attack, hold, decay or release: the time in seconds, in place of
    /// `value`.
    #[serde(default)]
    pub seconds: Option<f64>,
    /// `deactivated="1|0"`: an LFO's delay that is deactivated does nothing; a rate that is
    /// deactivated stops the LFO.
    #[serde(default)]
    pub deactivated: Option<bool>,
    #[serde(default)]
    pub temposync: Option<bool>,
    #[serde(default)]
    pub extend_range: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SetParamsArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// 1 to 64 changes, made in this order as one step.
    #[schemars(length(min = 1, max = 64))]
    pub params: Vec<ParamEditArg>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SetModulationArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// The parameter moved, by its patch name, like "a_pitch" (scene A's pitch).
    #[schemars(length(min = 1, max = 64))]
    pub target: String,
    /// The source: its number, short name ("vlfo_1") or name ("LFO 1").
    pub source: Value,
    /// How far the source at its full swing moves the target, in the target's own units:
    /// semitones for a_pitch (12 times that when its extend_range is on), as Surge XT keeps
    /// it. Negative moves the other way.
    pub depth: f64,
    #[serde(default)]
    pub muted: Option<bool>,
    /// For a target of the patch as a whole (not "a_…"/"b_…"): the scene of the source,
    /// "a" or "b" (default "a").
    #[serde(default)]
    pub scene: Option<String>,
    /// Changes of patch parameters made in the same step, as surge_set_patch_params takes
    /// them (for example the LFO's rate and delay).
    #[serde(default)]
    #[schemars(length(max = 64))]
    pub params: Option<Vec<ParamEditArg>>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct RemoveModulationArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// The parameter, by its patch name, like "a_pitch".
    #[schemars(length(min = 1, max = 64))]
    pub target: String,
    /// The source whose routings go. Leave out to take away every routing of the target.
    #[serde(default)]
    pub source: Option<Value>,
}

/// A Surge XT where the AI app named a plugin, and its state now.
struct Surge {
    place: PluginPlace,
    format: Format,
    got: PluginStateGot,
}

fn source_of(value: &Value) -> Result<&'static sources::Source, CallToolResult> {
    let found = match value {
        Value::Number(n) => n
            .as_u64()
            .and_then(|id| sources::by_id(u32::try_from(id).ok()?)),
        Value::String(text) => sources::find(text),
        _ => None,
    };
    found.ok_or_else(|| {
        let names: Vec<String> = (sources::SOURCES.iter())
            .map(|s| format!("{} {} ({})", s.id, s.short, s.name))
            .collect();
        failed(format!(
            "there is no source {value}; the sources are: {}",
            names.join(", ")
        ))
    })
}

fn source_value(id: i64) -> Value {
    let found = u32::try_from(id).ok().and_then(sources::by_id);
    json!({
        "id": id,
        "short": found.map(|s| s.short),
        "name": found.map(|s| s.name),
    })
}

fn edits_of(args: &[ParamEditArg]) -> Result<Vec<ParamEdit>, String> {
    if args.len() > MOST_EDITS {
        return Err(format!("give at most {MOST_EDITS} changes"));
    }
    let mut edits = Vec::with_capacity(args.len());
    for a in args {
        let name = a.name.trim();
        if is_fx_type(name) {
            return Err(format!(
                "{name}: change an effect's type through the plugin's parameter (set_param on \
                 \"FX A1 FX Type\" … as get_plugin_params lists them, with `text` such as \
                 \"Delay\"): that sets the effect's own parameters to its defaults, while \
                 the patch keeps the old values when only its type changes"
            ));
        }
        let value = match (a.value, a.hz, a.seconds) {
            (v, None, None) => v,
            (None, Some(hz), None) => Some(patch::from_natural(name, hz, "Hz")?),
            (None, None, Some(s)) => Some(patch::from_natural(name, s, "s")?),
            _ => return Err(format!("{name}: give one of `value`, `hz` and `seconds`")),
        };
        let flags: Vec<(&'static str, bool)> = [
            ("deactivated", a.deactivated),
            ("temposync", a.temposync),
            ("extend_range", a.extend_range),
        ]
        .into_iter()
        .filter_map(|(f, on)| Some((f, on?)))
        .collect();
        if value.is_none() && flags.is_empty() {
            return Err(format!("{name}: nothing to change"));
        }
        edits.push(ParamEdit {
            name: name.to_owned(),
            value,
            flags,
        });
    }
    Ok(edits)
}

/// `fx<N>_type`.
fn is_fx_type(name: &str) -> bool {
    (name
        .strip_prefix("fx")
        .and_then(|r| r.strip_suffix("_type")))
    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn routing_value(r: &Routing, names: &HashMap<u32, String>, format: Format) -> Value {
    let id = param_id(format, &r.target);
    json!({
        "target": r.target,
        "target_name": names.get(&id),
        "source": source_value(r.source),
        "depth": r.depth,
        "muted": r.muted,
        "source_index": r.source_index,
        "source_scene": r.source_scene,
    })
}

fn param_value(p: &patch::Param, names: &HashMap<u32, String>, format: Format) -> Value {
    let id = param_id(format, &p.name);
    let number = p.number();
    let natural =
        number.and_then(|v| patch::natural(&p.name, v, p.flag("temposync") == Some(true)));
    let attrs: serde_json::Map<String, Value> = (p.attrs.iter())
        .map(|(n, v)| (n.clone(), Value::String(v.clone())))
        .collect();
    json!({
        "name": p.name,
        "type": match p.kind { Some(0) => "int", Some(2) => "float", _ => "other" },
        "value": match number {
            Some(v) if p.kind == Some(0) && v.fract() == 0.0 => json!(v as i64),
            Some(v) => json!(v),
            None => json!(p.value),
        },
        "natural": natural.map(|(v, unit)| json!({ "value": (v * 1e4).round() / 1e4, "unit": unit })),
        "attributes": attrs,
        "routings": p.routings.len(),
        "param_id": names.contains_key(&id).then_some(id),
        "param_name": names.get(&id),
    })
}

fn paged(total: usize, offset: usize, shown: usize) -> Value {
    let next = offset + shown;
    json!({
        "total": total,
        "offset": offset,
        "next_offset": (next < total).then_some(next),
    })
}

impl Tools {
    /// The Surge XT at the place the AI app named, and its state now; any other plugin is
    /// refused before it is asked.
    async fn surge_at(
        &self,
        read: &AiSong,
        track: Option<u32>,
        bus: Option<BusArg>,
        slot: Option<u32>,
    ) -> Result<Surge, CallToolResult> {
        let place = plugin_place(read, track, bus, slot)?;
        if let Some(plugin) = read.song.doc.plugin(place) {
            format_of(plugin).map_err(failed)?;
        }
        let got = self.plugin_state(place).await?;
        let format = format_of(&got.plugin).map_err(failed)?;
        Ok(Surge { place, format, got })
    }

    /// The patch XML of a state, read on the blocking pool.
    async fn surge_xml(&self, format: Format, bytes: Arc<[u8]>) -> Result<String, CallToolResult> {
        off_thread(move || xml_of(format, &bytes))
            .await?
            .map_err(|e| failed(format!("Could not read Surge XT's patch: {e}.")))
    }

    /// The names of the plugin's parameters by their ids, or none when the plugin does not
    /// answer soon (they are only a help).
    async fn surge_names(&self, place: PluginPlace) -> HashMap<u32, String> {
        let until = Instant::now() + NAMES_WAIT;
        loop {
            match self.answer(AiAsk::PluginParams { place }).await {
                Ok(AiAnswer::Params(AiParams::Listed(list))) => {
                    return list.into_iter().map(|p| (p.id, p.name)).collect();
                }
                Ok(AiAnswer::Params(AiParams::Waiting)) if Instant::now() < until => {}
                _ => return HashMap::new(),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Edits the patch of the Surge XT at `s` with `edit` and puts it in as one step; then
    /// the patch as the plugin says it.
    async fn surge_write(
        &self,
        s: &Surge,
        edit: impl FnOnce(&str) -> Result<String, String> + Send + 'static,
    ) -> Result<(Value, String), CallToolResult> {
        let (format, bytes) = (s.format, s.got.bytes.clone());
        let made = off_thread(move || {
            let xml = xml_of(format, &bytes)?;
            let new = edit(&xml)?;
            (new != xml)
                .then(|| with_xml(format, &bytes, &new))
                .transpose()
        })
        .await?;
        let state = match made {
            Ok(Some(state)) => state,
            Ok(None) => {
                let read = self.read().await?;
                let value =
                    json!({ "changed": false, "step": null, "undo_steps": read.undo_count });
                return Ok((value, xml_of(format, &s.got.bytes).unwrap_or_default()));
            }
            Err(why) => return Err(failed(format!("Nothing was changed: {why}."))),
        };
        let d = self.put_plugin_state(s.place, state.into()).await?;
        let now = self.plugin_state(s.place).await?;
        let xml = self.surge_xml(format, now.bytes).await?;
        Ok((done_value(&d), xml))
    }

    /// The answer of a write: what was done, and the routings of `target` (or the
    /// parameters `params`) as Surge XT has them now.
    async fn surge_answer(
        &self,
        s: &Surge,
        mut value: Value,
        xml: String,
        target: Option<String>,
        params: Vec<String>,
        what: &str,
    ) -> Result<CallToolResult, McpError> {
        let names = self.surge_names(s.place).await;
        let format = s.format;
        let now = check!(
            off_thread(move || {
                let patch = Patch::parse(&xml).map_err(failed)?;
                let routings: Vec<Value> = match &target {
                    Some(t) => (patch
                        .param(t)
                        .map(|p| p.routings)
                        .unwrap_or_default()
                        .iter())
                    .map(|r| routing_value(r, &names, format))
                    .collect(),
                    None => Vec::new(),
                };
                let params: Vec<Value> = (params.iter())
                    .filter_map(|n| patch.param(n))
                    .map(|p| param_value(&p, &names, format))
                    .collect();
                Ok::<_, CallToolResult>((routings, params))
            })
            .await
        );
        let (routings, params) = check!(now);
        if value.get("target").is_some() {
            value["routings_now"] = json!(routings);
        }
        if !params.is_empty() {
            value["params_now"] = json!(params);
        }
        let text = match value["changed"] == true {
            true => format!("Done: {what}. As Surge XT has it now:\n{value}"),
            false => format!("No change: the patch already is like that.\n{value}"),
        };
        Ok(done(text, value))
    }
}

#[tool_router(router = surge_router, vis = "pub(super)")]
impl Tools {
    /// Surge XT only: its modulation routings now. Target: the patch's name ("a_pitch";
    /// LFOs count from 0 there: a_lfo0 is LFO 1, a_lfo6-11 the scene LFOs) and the plugin's
    /// own parameter name. Source: 17-22 LFO 1-6, 23-28 scene LFOs, 7-14 macros, 6
    /// modulation wheel, 1 velocity … Depth in the target's own units.
    #[tool(annotations(
        title = "Surge XT: list modulations",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn surge_list_modulations(
        &self,
        Parameters(a): Parameters<ListArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let source = match &a.source {
            Some(v) => Some(check!(source_of(v)).id),
            None => None,
        };
        let read = check!(self.read().await);
        let s = check!(self.surge_at(&read, a.track, a.bus, a.slot).await);
        let xml = check!(self.surge_xml(s.format, s.got.bytes.clone()).await);
        let names = self.surge_names(s.place).await;
        let (format, target) = (s.format, a.target.map(|t| t.to_lowercase()));
        let offset = a.offset.unwrap_or(0) as usize;
        let limit = a.limit.unwrap_or(100).clamp(1, 500) as usize;
        let listed = check!(
            off_thread(move || {
                let patch = Patch::parse(&xml)?;
                let all: Vec<Routing> = (patch.routings().into_iter())
                    .filter(|r| {
                        target
                            .as_ref()
                            .is_none_or(|t| r.target.to_lowercase().contains(t))
                    })
                    .filter(|r| source.is_none_or(|s| r.source == i64::from(s)))
                    .collect();
                let shown: Vec<Value> = (all.iter().skip(offset).take(limit))
                    .map(|r| routing_value(r, &names, format))
                    .collect();
                Ok::<_, String>((patch.meta(), all.len(), shown))
            })
            .await
        );
        let (meta, total, shown) = match listed {
            Ok(listed) => listed,
            Err(why) => return Ok(failed(format!("Could not read Surge XT's patch: {why}."))),
        };
        let mut value = json!({
            "plugin": { "name": &*s.got.plugin.name, "format": s.got.format },
            "patch": { "name": meta.name, "category": meta.category, "revision": meta.revision },
            "routings": shown,
        });
        value["page"] = paged(
            total,
            offset,
            value["routings"].as_array().map_or(0, Vec::len),
        );
        let text = format!(
            "{} routing(s) in the patch \"{}\".\n{value}",
            total,
            meta.name.as_deref().unwrap_or("")
        );
        Ok(done(text, value))
    }

    /// Surge XT only: parameters of its patch as it keeps them: int or float, value in the
    /// patch's units (LFO rates and envelope times also in Hz or seconds: 2^value),
    /// attributes (deactivated, temposync …), routings. `param_id`/`param_name`: the same
    /// parameter in get_plugin_params and set_param, where most are too (as 0..1).
    #[tool(annotations(
        title = "Surge XT: get patch parameters",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn surge_get_patch_params(
        &self,
        Parameters(a): Parameters<ParamsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let s = check!(self.surge_at(&read, a.track, a.bus, a.slot).await);
        let xml = check!(self.surge_xml(s.format, s.got.bytes.clone()).await);
        let names = self.surge_names(s.place).await;
        let (format, filter) = (s.format, a.name.map(|t| t.to_lowercase()));
        let offset = a.offset.unwrap_or(0) as usize;
        let limit = a.limit.unwrap_or(50).clamp(1, 500) as usize;
        let listed = check!(
            off_thread(move || {
                let patch = Patch::parse(&xml)?;
                let all: Vec<patch::Param> = (patch.params().into_iter())
                    .filter(|p| {
                        filter
                            .as_ref()
                            .is_none_or(|f| p.name.to_lowercase().contains(f))
                    })
                    .collect();
                let shown: Vec<Value> = (all.iter().skip(offset).take(limit))
                    .map(|p| param_value(p, &names, format))
                    .collect();
                Ok::<_, String>((patch.meta(), all.len(), shown))
            })
            .await
        );
        let (meta, total, shown) = match listed {
            Ok(listed) => listed,
            Err(why) => return Ok(failed(format!("Could not read Surge XT's patch: {why}."))),
        };
        let count = shown.len();
        let value = json!({
            "patch": { "name": meta.name, "category": meta.category, "revision": meta.revision },
            "params": shown,
            "page": paged(total, offset, count),
        });
        Ok(done(format!("{total} parameter(s).\n{value}"), value))
    }

    /// Surge XT only: makes a source move a float parameter (adds the routing or sets its
    /// depth), one undo step; `params` changes patch parameters in the same step. A late
    /// vibrato: target "a_pitch", source "LFO 1", depth 0.3 (semitones), params [{name
    /// "a_lfo0_rate", hz 5}, {name "a_lfo0_delay", seconds 0.5, deactivated false}].
    /// Answers what Surge XT kept.
    #[tool(annotations(
        title = "Surge XT: set modulation",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn surge_set_modulation(
        &self,
        Parameters(a): Parameters<SetModulationArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let source = check!(source_of(&a.source));
        let scene = match a.scene.as_deref().map(str::trim) {
            None => None,
            Some("a" | "A") => Some(0),
            Some("b" | "B") => Some(1),
            Some(_) => return Ok(failed("`scene` is \"a\" or \"b\"")),
        };
        let edits = check!(edits_of(a.params.as_deref().unwrap_or_default()).map_err(failed));
        let read = check!(self.read().await);
        let s = check!(self.surge_at(&read, a.track, a.bus, a.slot).await);
        let target = a.target.trim().to_owned();
        let (to, id, depth, muted) = (target.clone(), source.id, a.depth, a.muted);
        let names: Vec<String> = edits.iter().map(|e| e.name.clone()).collect();
        let written = self
            .surge_write(&s, move |xml| {
                let mut xml = patch::set_routing(xml, &to, id, depth, muted, scene)?;
                for edit in &edits {
                    xml = patch::set_param(&xml, edit)?;
                }
                Ok(xml)
            })
            .await;
        let (mut value, xml) = check!(written);
        value["target"] = json!(target);
        value["source"] = source_value(i64::from(source.id));
        let kept = Patch::parse(&xml)
            .ok()
            .and_then(|p| p.param(&target))
            .is_some_and(|p| p.routings.iter().any(|r| r.source == i64::from(source.id)));
        if !kept {
            value["note"] = json!("Surge XT did not keep this routing; undo takes the step back");
        }
        let what = format!("{} moves {target} by {depth}", source.name);
        self.surge_answer(&s, value, xml, Some(target), names, &what)
            .await
    }

    /// Surge XT only: takes away the routings of a parameter from one source, or all of
    /// them, as one undo step.
    #[tool(annotations(
        title = "Surge XT: remove modulation",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn surge_remove_modulation(
        &self,
        Parameters(a): Parameters<RemoveModulationArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let source = match &a.source {
            Some(v) => Some(check!(source_of(v)).id),
            None => None,
        };
        let read = check!(self.read().await);
        let s = check!(self.surge_at(&read, a.track, a.bus, a.slot).await);
        let target = a.target.trim().to_owned();
        let to = target.clone();
        let written = self
            .surge_write(&s, move |xml| {
                let (xml, gone) = patch::remove_routings(xml, &to, source)?;
                match gone {
                    0 => Err(format!("{to} has no such routing")),
                    _ => Ok(xml),
                }
            })
            .await;
        let (mut value, xml) = check!(written);
        value["target"] = json!(target);
        let what = format!("took routings of {target} away");
        self.surge_answer(&s, value, xml, Some(target), Vec::new(), &what)
            .await
    }

    /// Surge XT only: sets values (in the patch's units, or `hz` / `seconds` for LFO rates
    /// and envelope times) and the attributes deactivated, temposync and extend_range,
    /// which the plugin's parameter list does not reach, of its patch parameters: one undo
    /// step. Effect types (fxN_type) are refused: use set_param. Answers what Surge XT kept.
    #[tool(annotations(
        title = "Surge XT: set patch parameters",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn surge_set_patch_params(
        &self,
        Parameters(a): Parameters<SetParamsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.params.is_empty() {
            return Ok(failed("give 1 to 64 changes in `params`"));
        }
        let edits = check!(edits_of(&a.params).map_err(failed));
        let read = check!(self.read().await);
        let s = check!(self.surge_at(&read, a.track, a.bus, a.slot).await);
        let names: Vec<String> = edits.iter().map(|e| e.name.clone()).collect();
        let written = self
            .surge_write(&s, move |xml| {
                let mut xml = xml.to_owned();
                for edit in &edits {
                    xml = patch::set_param(&xml, edit)?;
                }
                Ok(xml)
            })
            .await;
        let (value, xml) = check!(written);
        let what = format!("set {}", names.join(", "));
        self.surge_answer(&s, value, xml, None, names, &what).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_are_checked_before_anything_is_asked() {
        let arg = |name: &str| ParamEditArg {
            name: name.into(),
            value: None,
            hz: None,
            seconds: None,
            deactivated: None,
            temposync: None,
            extend_range: None,
        };
        let rate = ParamEditArg {
            hz: Some(4.0),
            ..arg("a_lfo0_rate")
        };
        let delay = ParamEditArg {
            seconds: Some(0.5),
            deactivated: Some(false),
            ..arg("a_lfo0_delay")
        };
        let edits = edits_of(&[rate, delay]).unwrap();
        assert_eq!(edits[0].value, Some(2.0));
        assert_eq!(edits[1].value, Some(-1.0));
        assert_eq!(edits[1].flags, [("deactivated", false)]);
        let both = ParamEditArg {
            value: Some(1.0),
            hz: Some(1.0),
            ..arg("a_lfo0_rate")
        };
        for bad in [
            arg("a_pitch"),
            both,
            ParamEditArg {
                value: Some(1.0),
                ..arg("fx3_type")
            },
        ] {
            assert!(edits_of(&[bad]).is_err());
        }
        assert!(is_fx_type("fx16_type") && !is_fx_type("fx_type") && !is_fx_type("fx1_p0"));
        assert_eq!(source_of(&json!(17)).unwrap().short, "vlfo_1");
        assert_eq!(source_of(&json!("Scene LFO 1")).unwrap().id, 23);
        assert!(source_of(&json!(0)).is_err() && source_of(&json!(true)).is_err());
    }
}
