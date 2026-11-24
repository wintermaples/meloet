//! The mixer: strips, sends, mute and solo, and insert effects. Their parameters are in
//! `params.rs`.

use meloet_core::ai::{AiAsk, AiEffect};
use meloet_model::effects::EffectKind;
use meloet_model::{Bus, Channel, Insert, InsertId, StripId};
use meloet_protocol::{
    Command, EffectKind as ScreenEffect, InsertRef, PluginKindView, SendBus, StripRef,
};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tools, done, failed, instrument_of, refusal_text, track_of};
use crate::song::{self, Sf2Arg};
use crate::trackref::{TrackRef, id_text};
use meloet_core::ai::AiSong;

/// A strip that is not a track's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BusArg {
    Master,
    ReturnA,
    ReturnB,
}

pub(super) fn screen_effect(kind: EffectKind) -> ScreenEffect {
    match kind {
        EffectKind::Eq4 => ScreenEffect::Eq4,
        EffectKind::Compressor => ScreenEffect::Compressor,
        EffectKind::Reverb => ScreenEffect::Reverb,
        EffectKind::Delay => ScreenEffect::Delay,
        EffectKind::Chorus => ScreenEffect::Chorus,
        EffectKind::Limiter => ScreenEffect::Limiter,
        EffectKind::Gate => ScreenEffect::Gate,
        EffectKind::DeEsser => ScreenEffect::DeEsser,
        EffectKind::Saturation => ScreenEffect::Saturation,
    }
}

pub(super) fn strip_ref(strip: StripId) -> StripRef {
    match strip {
        StripId::Track(t) => StripRef::Track { id: t.0 },
        StripId::Return(Bus::A) => StripRef::Return { bus: SendBus::A },
        StripId::Return(Bus::B) => StripRef::Return { bus: SendBus::B },
        StripId::Master => StripRef::Master,
    }
}

/// The strip of a track or of `bus`, one of them.
pub(super) fn strip_of(
    read: &AiSong,
    track: Option<TrackRef>,
    bus: Option<BusArg>,
) -> Result<StripId, CallToolResult> {
    match (track, bus) {
        (Some(n), None) => Ok(StripId::Track(track_of(read, n)?.id)),
        (None, Some(BusArg::Master)) => Ok(StripId::Master),
        (None, Some(BusArg::ReturnA)) => Ok(StripId::Return(Bus::A)),
        (None, Some(BusArg::ReturnB)) => Ok(StripId::Return(Bus::B)),
        _ => Err(failed("give `track` or `bus`, one of them")),
    }
}

/// The slot from 1 to 4, as from 0.
pub(super) fn slot_of(slot: u32) -> Result<usize, CallToolResult> {
    match slot {
        1..=4 => Ok(slot as usize - 1),
        _ => Err(failed("an insert slot is 1 to 4")),
    }
}

pub(super) fn unit_name(unit: meloet_model::effects::Unit) -> String {
    format!("{unit:?}").to_lowercase()
}

fn insert_value(slot: usize, insert: &Insert) -> Value {
    match (insert.kind(), insert.plugin_ref()) {
        (Some(kind), _) => {
            let params: Vec<Value> = (kind.params().iter().zip(insert.values()).enumerate())
                .map(|(i, (p, v))| {
                    json!({ "index": i, "name": p.name, "value": v, "unit": unit_name(p.unit),
                        "min": p.min, "max": p.max })
                })
                .collect();
            json!({ "slot": slot + 1, "effect": kind.name(), "bypassed": insert.bypassed,
                "params": params })
        }
        (None, Some(plugin)) => json!({
            "slot": slot + 1, "plugin": { "key": &*plugin.key, "name": &*plugin.name },
            "bypassed": insert.bypassed,
            "params": "call get_plugin_params",
        }),
        (None, None) => json!({ "slot": slot + 1 }),
    }
}

fn channel_value(channel: &Channel) -> Value {
    let inserts: Vec<Value> = (channel.inserts.iter().enumerate())
        .filter_map(|(i, s)| s.as_ref().map(|insert| insert_value(i, insert)))
        .collect();
    json!({
        "volume_db": channel.gain.db(),
        "pan": channel.pan.get(),
        "inserts": inserts,
    })
}

pub(super) fn mixer(read: &AiSong) -> Value {
    let doc = &read.song.doc;
    let soloed = &read.song.session.soloed;
    let tracks: Vec<Value> = (doc.tracks.iter().enumerate())
        .map(|(i, t)| {
            let mut v = channel_value(&t.channel);
            v["number"] = json!(i + 1);
            v["id"] = json!(id_text(t.id));
            v["name"] = json!(t.name);
            v["muted"] = json!(t.muted);
            v["soloed"] = json!(soloed.contains(&t.id));
            v["send_a_db"] = json!(t.sends[0].db());
            v["send_b_db"] = json!(t.sends[1].db());
            v["instrument"] = match t.kind.is_vocal() {
                true => Value::Null,
                false => json!(song::sound_name(&t.instrument)),
            };
            super::outputs::add_track_outputs(read, t, &mut v);
            v
        })
        .collect();
    let returns: Vec<Value> = (doc.returns.iter().zip(["return_a", "return_b"]))
        .map(|(r, name)| {
            let mut v = channel_value(&r.channel);
            v["bus"] = json!(name);
            v["muted"] = json!(r.muted);
            v
        })
        .collect();
    let mut master = channel_value(&doc.master);
    master["bus"] = json!("master");
    json!({
        "volume_note": "dB from -60 to +6; null is silence (-inf). Pan -100 (left) to 100 (right).",
        "tracks": tracks,
        "returns": returns,
        "master": master,
    })
}

/// The insert at a slot, as read.
fn insert_at(read: &AiSong, strip: StripId, slot: usize) -> Option<&Insert> {
    read.song.doc.insert(strip, slot)
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct VolumeArgs {
    #[serde(default)]
    pub track: Option<TrackRef>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// dB, -60 to 6. Below -60 is silence (-inf).
    #[schemars(range(min = -200.0, max = 6.0))]
    pub db: f64,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct PanArgs {
    #[serde(default)]
    pub track: Option<TrackRef>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// -100 (all left) to 100 (all right); 0 is the middle.
    #[schemars(range(min = -100, max = 100))]
    pub pan: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SendArg {
    A,
    B,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SendArgs {
    pub track: TrackRef,
    /// "a" (to return A) or "b" (to return B).
    pub send: SendArg,
    /// dB, -60 to 6. Below -60 sends nothing.
    #[schemars(range(min = -200.0, max = 6.0))]
    pub db: f64,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MuteArgs {
    /// A track (number or id). Give this or `bus` ("return_a" or "return_b"; the master
    /// has no mute).
    #[serde(default)]
    pub track: Option<TrackRef>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    pub on: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SoloArgs {
    pub track: TrackRef,
    pub on: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct EffectArgs {
    #[serde(default)]
    pub track: Option<TrackRef>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// 1 to 4, top first.
    #[schemars(range(min = 1, max = 4))]
    pub slot: u32,
    /// A built-in effect (eq4, compressor, reverb, delay, chorus, limiter, gate, deEsser,
    /// saturation) or an effect plugin's key from list_effects.
    #[schemars(length(min = 1, max = 1024))]
    pub effect: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SlotArgs {
    #[serde(default)]
    pub track: Option<TrackRef>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    #[schemars(range(min = 1, max = 4))]
    pub slot: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MoveEffectArgs {
    #[serde(default)]
    pub track: Option<TrackRef>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    #[schemars(range(min = 1, max = 4))]
    pub slot: u32,
    /// The slot it goes to, 1 to 4.
    #[schemars(range(min = 1, max = 4))]
    pub to: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct BypassArgs {
    #[serde(default)]
    pub track: Option<TrackRef>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    #[schemars(range(min = 1, max = 4))]
    pub slot: u32,
    /// true: the sound passes the effect by.
    pub on: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct InstrumentArgs {
    pub track: TrackRef,
    /// A built-in sound's name from list_sounds.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub sound: Option<String>,
    /// An instrument plugin's key from list_instruments.
    #[serde(default)]
    #[schemars(length(min = 1, max = 1024))]
    pub plugin: Option<String>,
    /// A preset of an added SF2 file, from list_instruments.
    #[serde(default)]
    pub sf2: Option<Sf2Arg>,
}

fn db_of(db: f64) -> Result<Option<f32>, CallToolResult> {
    if !db.is_finite() || db > 6.0 {
        return Err(failed("give dB up to 6 (below -60 is silence)"));
    }
    Ok((db >= -60.0).then_some(db as f32))
}

fn effect_of(text: &str) -> AiEffect {
    match EffectKind::from_name(text.trim()) {
        Some(kind) => AiEffect::Builtin(kind),
        None => AiEffect::Plugin(text.trim().to_owned()),
    }
}

#[tool_router(router = mixer_router, vis = "pub(super)")]
impl Tools {
    /// Every strip of the mixer: the tracks (volume dB, pan, mute, solo, sends A and B, the
    /// instrument, an instrument plugin's outputs), returns A and B, and the master; their
    /// insert effects with bypass and parameter values.
    #[tool(annotations(title = "Get mixer", read_only_hint = true, open_world_hint = false))]
    async fn get_mixer(&self) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let value = mixer(&read);
        Ok(done(value.to_string(), value))
    }

    /// The sounds a track can have: meloet's own, the instrument plugins this PC has (by
    /// key), and the presets of the SF2 files added to meloet.
    #[tool(annotations(
        title = "List instruments",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn list_instruments(&self) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let plugins: Vec<Value> = (read.library.plugins.iter())
            .filter(|p| matches!(p.kind, PluginKindView::Instrument(_)))
            .map(|p| {
                json!({ "key": p.key, "name": p.name, "vendor": p.vendor,
                    "kind": p.kind, "formats": p.formats })
            })
            .collect();
        let value = json!({
            "builtin": song::sounds()["sounds"],
            "plugins": plugins,
            "sf2": read.library.sf2,
        });
        Ok(done(value.to_string(), value))
    }

    /// Gives a keys track another sound (built-in, instrument plugin or SF2 preset), as one
    /// undo step. Lanes of the old plugin's parameters go.
    #[tool(annotations(
        title = "Set instrument",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_instrument(
        &self,
        Parameters(args): Parameters<InstrumentArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let instrument = check!(instrument_of(
            args.sound.as_deref(),
            args.plugin.as_deref(),
            args.sf2.as_ref()
        ));
        let read = check!(self.read().await);
        let track = check!(track_of(&read, args.track)).id.0;
        Ok(self.write(AiAsk::SetInstrument { track, instrument }).await)
    }

    /// Sets a strip's volume, as one undo step.
    #[tool(annotations(
        title = "Set volume",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_volume(
        &self,
        Parameters(args): Parameters<VolumeArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let db = check!(db_of(args.db));
        let read = check!(self.read().await);
        let strip = strip_ref(check!(strip_of(&read, args.track, args.bus)));
        Ok(self
            .screen(Command::SetGain {
                strip,
                db,
                commit: true,
            })
            .await)
    }

    /// Sets a strip's pan, as one undo step.
    #[tool(annotations(
        title = "Set pan",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_pan(
        &self,
        Parameters(args): Parameters<PanArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if !(-100..=100).contains(&args.pan) {
            return Ok(failed("the pan is -100 to 100"));
        }
        let read = check!(self.read().await);
        let strip = strip_ref(check!(strip_of(&read, args.track, args.bus)));
        Ok(self
            .screen(Command::SetPan {
                strip,
                pan: args.pan,
                commit: true,
            })
            .await)
    }

    /// Sets how much of a track goes to return A or B, as one undo step.
    #[tool(annotations(
        title = "Set send",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_send(
        &self,
        Parameters(args): Parameters<SendArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let db = check!(db_of(args.db));
        let read = check!(self.read().await);
        let track = check!(track_of(&read, args.track)).id.0;
        let bus = match args.send {
            SendArg::A => SendBus::A,
            SendArg::B => SendBus::B,
        };
        Ok(self
            .screen(Command::SetSend {
                track,
                bus,
                db,
                commit: true,
            })
            .await)
    }

    /// Mutes or unmutes a track or a return, as one undo step.
    #[tool(annotations(
        title = "Set mute",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_mute(
        &self,
        Parameters(args): Parameters<MuteArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let doc = &read.song.doc;
        let (muted, command) = match check!(strip_of(&read, args.track, args.bus)) {
            StripId::Track(id) => {
                let t = doc.track(id).is_some_and(|t| t.muted);
                (t, Command::ToggleMute { track: id.0 })
            }
            StripId::Return(bus) => {
                let send = match bus {
                    Bus::A => SendBus::A,
                    Bus::B => SendBus::B,
                };
                (
                    doc.returns[bus.index()].muted,
                    Command::ToggleReturnMute { bus: send },
                )
            }
            StripId::Master => return Ok(failed("the master has no mute")),
        };
        if muted == args.on {
            return Ok(self.no_change().await);
        }
        Ok(self.screen(command).await)
    }

    /// Solos a track or ends its solo. Solo changes only what plays: it is no undo step and
    /// is not saved.
    #[tool(annotations(
        title = "Set solo",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_solo(
        &self,
        Parameters(args): Parameters<SoloArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let id = check!(track_of(&read, args.track)).id;
        if read.song.session.soloed.contains(&id) == args.on {
            return Ok(self.no_change().await);
        }
        Ok(self.screen(Command::ToggleSolo { track: id.0 }).await)
    }

    /// Puts an effect (built-in or plugin) into an empty insert slot, as one undo step.
    #[tool(annotations(
        title = "Insert effect",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn insert_effect(
        &self,
        Parameters(args): Parameters<EffectArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.put_effect(args, false).await
    }

    /// Puts an effect in place of what an insert slot holds, as one undo step. Lanes of the
    /// old effect's parameters go.
    #[tool(annotations(
        title = "Replace effect",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn replace_effect(
        &self,
        Parameters(args): Parameters<EffectArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.put_effect(args, true).await
    }

    /// Takes the effect out of an insert slot, as one undo step.
    #[tool(annotations(
        title = "Remove effect",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn remove_effect(
        &self,
        Parameters(args): Parameters<SlotArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let slot = check!(slot_of(args.slot));
        let read = check!(self.read().await);
        let strip = check!(strip_of(&read, args.track, args.bus));
        if insert_at(&read, strip, slot).is_none() {
            return Ok(failed(refusal_text(&meloet_core::ai::AiRefusal::SlotEmpty)));
        }
        let insert = InsertRef {
            strip: strip_ref(strip),
            slot: slot as u8,
        };
        Ok(self.screen(Command::RemoveEffect { insert }).await)
    }

    /// Moves an effect to another insert slot (the others shift), as one undo step.
    #[tool(annotations(
        title = "Move effect",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn move_effect(
        &self,
        Parameters(args): Parameters<MoveEffectArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let (slot, to) = (check!(slot_of(args.slot)), check!(slot_of(args.to)));
        let read = check!(self.read().await);
        let strip = check!(strip_of(&read, args.track, args.bus));
        let insert = InsertRef {
            strip: strip_ref(strip),
            slot: slot as u8,
        };
        Ok(self
            .screen(Command::MoveInsert {
                insert,
                to: to as u8,
            })
            .await)
    }

    /// Bypasses an effect (the sound passes it by; its values stay) or ends that, as one
    /// undo step.
    #[tool(annotations(
        title = "Bypass effect",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn bypass_effect(
        &self,
        Parameters(args): Parameters<BypassArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let slot = check!(slot_of(args.slot));
        let read = check!(self.read().await);
        let strip = check!(strip_of(&read, args.track, args.bus));
        let Some(insert) = insert_at(&read, strip, slot) else {
            return Ok(failed(refusal_text(&meloet_core::ai::AiRefusal::SlotEmpty)));
        };
        if insert.bypassed == args.on {
            return Ok(self.no_change().await);
        }
        let insert = InsertRef {
            strip: strip_ref(strip),
            slot: slot as u8,
        };
        Ok(self.screen(Command::ToggleBypass { insert }).await)
    }
}

impl Tools {
    async fn put_effect(
        &self,
        args: EffectArgs,
        replace: bool,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let slot = check!(slot_of(args.slot));
        let read = check!(self.read().await);
        let strip = check!(strip_of(&read, args.track, args.bus));
        Ok(self
            .write(AiAsk::PutEffect {
                strip,
                slot,
                effect: effect_of(&args.effect),
                replace,
            })
            .await)
    }
}

/// The insert id of a slot, for the lanes of its parameters.
pub(super) fn insert_id(read: &AiSong, strip: StripId, slot: usize) -> Option<InsertId> {
    insert_at(read, strip, slot).map(|i| i.id)
}
