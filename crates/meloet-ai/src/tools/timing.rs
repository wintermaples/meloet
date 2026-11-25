//! Time: meter changes, tempo changes, markers and the loop.

use meloet_model::{Meter, TempoMap};
use meloet_protocol::{Command, TimelineRef};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;

use super::{Tools, at_or_tick, failed, name_ok};

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MeterArgs {
    /// The bar it starts at (1 is the song's own meter).
    #[schemars(range(min = 1, max = 9999))]
    pub bar: u32,
    /// Beats in a bar, 1 to 32.
    #[schemars(range(min = 1, max = 32))]
    pub num: u32,
    /// The beat's note: 2, 4, 8 or 16.
    pub den: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct BarArgs {
    /// The bar of a meter change, 2 or more.
    #[schemars(range(min = 2, max = 9999))]
    pub bar: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct PlaceArgs {
    /// A tempo change's place as song_overview gives it, "bar.beat.tick". Give this or `tick`.
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub tick: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct RampArgs {
    /// The tempo change the ramp starts from; leave both out for the song's start.
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub tick: Option<u64>,
    /// true: the tempo moves smoothly from here to the next change.
    pub on: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MoveMarkerArgs {
    /// The marker's id from song_overview.
    #[schemars(range(min = 1))]
    pub marker: u32,
    /// Where to, as "bar.beat.tick" (snapped to the nearest beat). Give this or `tick`.
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub tick: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct RenameMarkerArgs {
    #[schemars(range(min = 1))]
    pub marker: u32,
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MarkerArgs {
    #[schemars(range(min = 1))]
    pub marker: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct LoopArgs {
    /// The first bar of the loop.
    #[schemars(range(min = 1, max = 9999))]
    pub from_bar: u32,
    /// The last bar, looped to its end.
    #[schemars(range(min = 1, max = 9999))]
    pub to_bar: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct OnArgs {
    pub on: bool,
}

#[tool_router(router = timing_router, vis = "pub(super)")]
impl Tools {
    /// Sets the meter from a bar on (a meter change there, or the song's meter at bar 1),
    /// as one undo step.
    #[tool(annotations(
        title = "Set meter",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_meter(
        &self,
        Parameters(a): Parameters<MeterArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let (Ok(num), Ok(den)) = (u8::try_from(a.num), u8::try_from(a.den)) else {
            return Ok(failed(
                "beats are 1 to 32 and the beat's note 2, 4, 8 or 16",
            ));
        };
        if Meter::new(num, den).is_none() || !(1..=9999).contains(&a.bar) {
            return Ok(failed(
                "beats are 1 to 32 and the beat's note 2, 4, 8 or 16",
            ));
        }
        let read = check!(self.read().await);
        let bar = a.bar - 1;
        let there = bar == 0 || (read.song.doc.timeline.meter_changes.iter()).any(|c| c.bar == bar);
        let command = match there {
            true => Command::SetMeterChange { bar, num, den },
            false => Command::AddMeterChange { bar, num, den },
        };
        Ok(self.screen(command).await)
    }

    /// Deletes the meter change at a bar, as one undo step.
    #[tool(annotations(
        title = "Delete meter change",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_meter_change(
        &self,
        Parameters(a): Parameters<BarArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.bar < 2 {
            return Ok(failed(
                "bar 1 has the song's own meter; set it with set_meter",
            ));
        }
        let target = TimelineRef::MeterChange { bar: a.bar - 1 };
        Ok(self.screen(Command::DeleteTimeline { target }).await)
    }

    /// Deletes a tempo change, as one undo step.
    #[tool(annotations(
        title = "Delete tempo change",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_tempo_change(
        &self,
        Parameters(a): Parameters<PlaceArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let tick = check!(at_or_tick(&map, a.at.as_deref(), a.tick));
        if tick == 0 {
            return Ok(failed(
                "the song's start tempo stays; set it with set_tempo",
            ));
        }
        let target = TimelineRef::TempoPoint { tick };
        Ok(self.screen(Command::DeleteTimeline { target }).await)
    }

    /// Makes the tempo move smoothly from a tempo change (or the start) to the next one, or
    /// stop doing so, as one undo step.
    #[tool(annotations(
        title = "Set tempo ramp",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_tempo_ramp(
        &self,
        Parameters(a): Parameters<RampArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let tick = match (&a.at, a.tick) {
            (None, None) => 0,
            (at, tick) => check!(at_or_tick(&map, at.as_deref(), tick)),
        };
        Ok(self.screen(Command::SetTempoRamp { tick, on: a.on }).await)
    }

    /// Moves a marker, as one undo step.
    #[tool(annotations(
        title = "Move marker",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn move_marker(
        &self,
        Parameters(a): Parameters<MoveMarkerArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let tick = check!(at_or_tick(&map, a.at.as_deref(), a.tick));
        Ok(self
            .screen(Command::MoveMarker { id: a.marker, tick })
            .await)
    }

    /// Renames a marker, as one undo step.
    #[tool(annotations(
        title = "Rename marker",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn rename_marker(
        &self,
        Parameters(a): Parameters<RenameMarkerArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let name = check!(name_ok(&a.name));
        Ok(self
            .screen(Command::RenameMarker { id: a.marker, name })
            .await)
    }

    /// Deletes a marker, as one undo step.
    #[tool(annotations(
        title = "Delete marker",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_marker(
        &self,
        Parameters(a): Parameters<MarkerArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let target = TimelineRef::Marker { id: a.marker };
        Ok(self.screen(Command::DeleteTimeline { target }).await)
    }

    /// Sets the loop's bars and turns the loop on. A new range is one undo step.
    #[tool(annotations(
        title = "Set loop",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_loop(
        &self,
        Parameters(a): Parameters<LoopArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.from_bar == 0 || a.to_bar < a.from_bar || a.to_bar > 9999 {
            return Ok(failed("give 1 <= from_bar <= to_bar <= 9999"));
        }
        Ok(self
            .screen(Command::SetLoop {
                from_bar: a.from_bar,
                to_bar: a.to_bar + 1,
            })
            .await)
    }

    /// Turns the loop on or off. Only how it plays changes: no undo step.
    #[tool(annotations(
        title = "Loop on or off",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_loop_on(
        &self,
        Parameters(a): Parameters<OnArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        if read.song.doc.loop_range.is_none() {
            return Ok(failed("the song has no loop range; set one with set_loop"));
        }
        if read.song.session.loop_on == a.on {
            return Ok(self.no_change().await);
        }
        Ok(self.screen_when(Command::ToggleLoop, a.on).await)
    }
}
