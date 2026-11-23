//! Automation: the lanes of a track and their points.

use meloet_core::ai::{AiAsk, AiSong};
use meloet_model::{AutoPoint, AutoTarget, Bus, Gain, Pan, StripId, TempoMap};
use meloet_protocol::{AutoTarget as ScreenTarget, Command, SendBus};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::mixer::{insert_id, slot_of};
use super::{MAX_NOTES, Tools, at_or_tick, done, failed, track_of};
use crate::song;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Volume,
    Pan,
    SendA,
    SendB,
    Mute,
    /// A parameter of the track's instrument plugin (`param`: its id).
    Instrument,
    /// A parameter of an insert effect (`slot`, and `param`: a built-in effect's index or a
    /// plugin's id).
    Insert,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct LaneArgs {
    #[schemars(range(min = 1, max = 64))]
    pub track: u32,
    /// What the lane moves.
    pub target: TargetKind,
    /// For "insert": the slot, 1 to 4.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// For "instrument" and "insert": the parameter.
    #[serde(default)]
    pub param: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct PointArgs {
    /// Where, as "bar.beat.tick". Give this or `tick`.
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub tick: Option<u64>,
    /// 0 to 1, the place of the knob: volume 0.8 is 0 dB and 1 is +6 dB, pan 0.5 is the
    /// middle, mute 1 is muted, a parameter from its least to its most.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub value: f32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct WriteArgs {
    #[schemars(range(min = 1, max = 64))]
    pub track: u32,
    pub target: TargetKind,
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    #[serde(default)]
    pub param: Option<u32>,
    /// 1 to 2000 points; one at the tick of a point there takes its place.
    #[schemars(length(min = 1, max = 2000))]
    pub points: Vec<PointArgs>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MovePointsArgs {
    #[schemars(range(min = 1, max = 64))]
    pub track: u32,
    pub target: TargetKind,
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    #[serde(default)]
    pub param: Option<u32>,
    /// The points' indexes from get_automation.
    #[schemars(length(min = 1, max = 2000))]
    pub points: Vec<u32>,
    /// Ticks later (earlier below 0).
    #[serde(default)]
    pub ticks: i64,
    /// Added to each value (-1 to 1); they stop together at 0 and 1.
    #[serde(default)]
    #[schemars(range(min = -1.0, max = 1.0))]
    pub value: f32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct DeletePointsArgs {
    #[schemars(range(min = 1, max = 64))]
    pub track: u32,
    pub target: TargetKind,
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    #[serde(default)]
    pub param: Option<u32>,
    #[schemars(length(min = 1, max = 2000))]
    pub points: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct TrackArgs {
    #[schemars(range(min = 1, max = 64))]
    pub track: u32,
}

/// The track's id and the target, as the song and as the screen name it.
fn target_of(
    read: &AiSong,
    track: u32,
    kind: TargetKind,
    slot: Option<u32>,
    param: Option<u32>,
) -> Result<(u32, AutoTarget, ScreenTarget), CallToolResult> {
    let id = track_of(read, track)?.id;
    let need = |p: Option<u32>| p.ok_or_else(|| failed("give the `param`"));
    Ok(match kind {
        TargetKind::Volume => (id.0, AutoTarget::Volume, ScreenTarget::Volume),
        TargetKind::Pan => (id.0, AutoTarget::Pan, ScreenTarget::Pan),
        TargetKind::SendA => (
            id.0,
            AutoTarget::Send(Bus::A),
            ScreenTarget::Send { bus: SendBus::A },
        ),
        TargetKind::SendB => (
            id.0,
            AutoTarget::Send(Bus::B),
            ScreenTarget::Send { bus: SendBus::B },
        ),
        TargetKind::Mute => (id.0, AutoTarget::Mute, ScreenTarget::Mute),
        TargetKind::Instrument => {
            let param = need(param)?;
            (
                id.0,
                AutoTarget::Instrument { param },
                ScreenTarget::Instrument { param },
            )
        }
        TargetKind::Insert => {
            let slot = slot_of(slot.ok_or_else(|| failed("give the insert `slot`"))?)?;
            let param = need(param)?;
            let insert = insert_id(read, StripId::Track(id), slot)
                .ok_or_else(|| failed("that insert slot is empty"))?;
            (
                id.0,
                AutoTarget::Insert { insert, param },
                ScreenTarget::Insert {
                    insert: insert.0,
                    param,
                },
            )
        }
    })
}

/// What a lane's value means, where it is plain.
fn shown(track: &meloet_model::Track, target: AutoTarget, value: f32) -> Value {
    match target {
        AutoTarget::Volume | AutoTarget::Send(_) => {
            json!({ "db": Gain::from_position(value).db() })
        }
        AutoTarget::Pan => json!({ "pan": (Pan::unit_at(value) * 100.0).round() }),
        AutoTarget::Mute => json!({ "muted": value >= 0.5 }),
        AutoTarget::Insert { insert, param } => {
            let found = (track.channel.inserts.iter().flatten()).find(|i| i.id == insert);
            match found.and_then(|i| i.kind()).and_then(|k| k.param(param)) {
                Some(spec) => json!({ spec.name: spec.value_at(value) }),
                None => Value::Null,
            }
        }
        AutoTarget::Instrument { .. } => Value::Null,
    }
}

fn target_value(track: &meloet_model::Track, target: AutoTarget) -> Value {
    match target {
        AutoTarget::Volume => json!({ "target": "volume" }),
        AutoTarget::Pan => json!({ "target": "pan" }),
        AutoTarget::Send(Bus::A) => json!({ "target": "send_a" }),
        AutoTarget::Send(Bus::B) => json!({ "target": "send_b" }),
        AutoTarget::Mute => json!({ "target": "mute" }),
        AutoTarget::Instrument { param } => json!({ "target": "instrument", "param": param }),
        AutoTarget::Insert { insert, param } => {
            let slot = (track.channel.inserts.iter())
                .position(|i| i.as_ref().is_some_and(|i| i.id == insert))
                .map(|s| s + 1);
            json!({ "target": "insert", "slot": slot, "param": param })
        }
    }
}

#[tool_router(router = automation_router, vis = "pub(super)")]
impl Tools {
    /// The automation lanes of a track: what each moves, whether it rests, and its points
    /// (index, tick, "bar.beat.tick", value 0 to 1 and what it means).
    #[tool(annotations(
        title = "Get automation",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn get_automation(
        &self,
        Parameters(a): Parameters<TrackArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let track = check!(track_of(&read, a.track));
        let map = TempoMap::new(&read.song.doc.timeline);
        let lanes: Vec<Value> = (track.automation.iter())
            .map(|lane| {
                let points: Vec<Value> = (lane.points.iter().enumerate().take(MAX_NOTES))
                    .map(|(i, p)| {
                        json!({ "index": i, "tick": p.tick, "at": song::at(&map, p.tick),
                            "value": p.value, "means": shown(track, lane.target, p.value) })
                    })
                    .collect();
                let mut v = target_value(track, lane.target);
                v["resting"] = json!(lane.resting);
                v["steps"] = json!(lane.step);
                v["total"] = json!(lane.points.len());
                v["points"] = json!(points);
                v
            })
            .collect();
        let value = json!({ "track": a.track, "lanes": lanes });
        Ok(done(value.to_string(), value))
    }

    /// Adds an empty automation lane to a track, as one undo step.
    #[tool(annotations(
        title = "Add lane",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn add_lane(
        &self,
        Parameters(a): Parameters<LaneArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let (track, model, target) = check!(target_of(&read, a.track, a.target, a.slot, a.param));
        let t = check!(track_of(&read, a.track));
        if t.lane(model).is_some() {
            return Ok(failed("the track has that lane already"));
        }
        Ok(self.screen(Command::AddLane { track, target }).await)
    }

    /// Deletes an automation lane with its points, as one undo step.
    #[tool(annotations(
        title = "Delete lane",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_lane(
        &self,
        Parameters(a): Parameters<LaneArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let (track, _, target) = check!(target_of(&read, a.track, a.target, a.slot, a.param));
        Ok(self.screen(Command::DeleteLane { track, target }).await)
    }

    /// Writes points into a lane (add it first with add_lane), as one undo step.
    #[tool(annotations(
        title = "Write automation",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn write_automation(
        &self,
        Parameters(a): Parameters<WriteArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.points.is_empty() || a.points.len() > MAX_NOTES {
            return Ok(failed(format!("give 1 to {MAX_NOTES} points")));
        }
        let read = check!(self.read().await);
        let (track, target, _) = check!(target_of(&read, a.track, a.target, a.slot, a.param));
        let map = TempoMap::new(&read.song.doc.timeline);
        let mut points = Vec::with_capacity(a.points.len());
        for p in &a.points {
            if !(0.0..=1.0).contains(&p.value) {
                return Ok(failed("a value is 0 to 1"));
            }
            let tick = check!(at_or_tick(&map, p.at.as_deref(), p.tick));
            points.push(AutoPoint {
                tick,
                value: p.value,
            });
        }
        Ok(self
            .write(AiAsk::WriteAutomation {
                track,
                target,
                points,
            })
            .await)
    }

    /// Moves points of a lane along and up or down, as one undo step.
    #[tool(annotations(
        title = "Move automation points",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn move_automation(
        &self,
        Parameters(a): Parameters<MovePointsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.points.is_empty() || a.points.len() > MAX_NOTES || !(-1.0..=1.0).contains(&a.value) {
            return Ok(failed(format!(
                "give 1 to {MAX_NOTES} points, and a value change of -1 to 1"
            )));
        }
        let read = check!(self.read().await);
        let (track, target, _) = check!(target_of(&read, a.track, a.target, a.slot, a.param));
        Ok(self
            .write(AiAsk::MoveAutomation {
                track,
                target,
                points: a.points,
                ticks: a
                    .ticks
                    .clamp(-(crate::MAX_TICK as i64), crate::MAX_TICK as i64),
                value: a.value,
            })
            .await)
    }

    /// Deletes points of a lane, as one undo step.
    #[tool(annotations(
        title = "Delete automation points",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_automation(
        &self,
        Parameters(a): Parameters<DeletePointsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.points.is_empty() || a.points.len() > MAX_NOTES {
            return Ok(failed(format!("give 1 to {MAX_NOTES} points")));
        }
        let read = check!(self.read().await);
        let (track, target, _) = check!(target_of(&read, a.track, a.target, a.slot, a.param));
        Ok(self
            .write(AiAsk::DeleteAutomation {
                track,
                target,
                points: a.points,
            })
            .await)
    }
}
