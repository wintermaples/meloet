//! Changing notes that are there: by the take's id and the notes' indexes from get_notes.

use meloet_core::ai::{AiAsk, AiNoteEdit};
use meloet_model::{ClipEdge, Grid, NoteField, QuantizeAlign, QuantizeSettings};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;

use super::{MAX_LENGTH, MAX_NOTES, Tools, failed};

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MoveNotesArgs {
    /// The take's id from get_notes. When the clip shows another take now, nothing is done.
    #[schemars(range(min = 1))]
    pub take: u32,
    /// The notes' indexes in that take.
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Vec<u32>,
    /// Ticks later (earlier when below 0); 1920 is a quarter note.
    #[serde(default)]
    pub ticks: i64,
    /// Keys up (down when below 0).
    #[serde(default)]
    #[schemars(range(min = -127, max = 127))]
    pub keys: i32,
    /// Copy them there, leaving these where they are.
    #[serde(default)]
    pub copy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EdgeArg {
    Start,
    End,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ResizeNotesArgs {
    #[schemars(range(min = 1))]
    pub take: u32,
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Vec<u32>,
    /// Which end moves: "end" (the length changes) or "start" (the start moves, the end stays).
    pub edge: EdgeArg,
    /// How far that end moves, in ticks (below 0: earlier).
    pub ticks: i64,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct NoteLengthArgs {
    #[schemars(range(min = 1))]
    pub take: u32,
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Vec<u32>,
    /// Every note gets this length, in ticks.
    #[schemars(range(min = 1, max = 491520))]
    pub length: u64,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct VelocityArgs {
    #[schemars(range(min = 1))]
    pub take: u32,
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Vec<u32>,
    /// Every note gets this velocity, 1 to 127. Give this or `delta`.
    #[serde(default)]
    #[schemars(range(min = 1, max = 127))]
    pub value: Option<u32>,
    /// Added to each note's velocity (kept in 1 to 127). Give this or `value`.
    #[serde(default)]
    #[schemars(range(min = -126, max = 126))]
    pub delta: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct QuantizeArgs {
    #[schemars(range(min = 1))]
    pub take: u32,
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Vec<u32>,
    /// The grid as a part of a whole note: 2, 4, 8, 16 or 32 (16 is a sixteenth); 0 is a bar.
    pub grid: u32,
    /// Triplets of that grid.
    #[serde(default)]
    pub triplet: bool,
    /// Percent of the way to the grid, 1 to 100 (default 100).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100))]
    pub strength: Option<u32>,
    /// Percent, 0 to 100 (default 0): every second grid step comes later.
    #[serde(default)]
    #[schemars(range(min = 0, max = 100))]
    pub swing: Option<u32>,
    /// Bring the notes' ends to the grid too (default: only their starts).
    #[serde(default)]
    pub ends: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct PickedArgs {
    #[schemars(range(min = 1))]
    pub take: u32,
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Vec<u32>,
}

fn picked(notes: &[u32]) -> Result<(), CallToolResult> {
    if notes.is_empty() || notes.len() > MAX_NOTES {
        return Err(failed(format!("give 1 to {MAX_NOTES} note indexes")));
    }
    Ok(())
}

impl Tools {
    async fn edit_notes(&self, take: u32, notes: Vec<u32>, edit: AiNoteEdit) -> CallToolResult {
        if let Err(result) = picked(&notes) {
            return result;
        }
        self.write(AiAsk::EditNotes { take, notes, edit }).await
    }
}

#[tool_router(router = notes_router, vis = "pub(super)")]
impl Tools {
    /// Moves notes along and up or down (or copies them), as one undo step. The clip grows
    /// by bars when they go past it.
    #[tool(annotations(
        title = "Move notes",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn move_notes(
        &self,
        Parameters(a): Parameters<MoveNotesArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.ticks.unsigned_abs() > crate::MAX_TICK || !(-127..=127).contains(&a.keys) {
            return Ok(failed("move by fewer ticks, and by -127 to 127 keys"));
        }
        let edit = AiNoteEdit::Move {
            ticks: a.ticks,
            keys: a.keys,
            copy: a.copy,
        };
        Ok(self.edit_notes(a.take, a.notes, edit).await)
    }

    /// Moves the start or the end of notes, as one undo step.
    #[tool(annotations(
        title = "Resize notes",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn resize_notes(
        &self,
        Parameters(a): Parameters<ResizeNotesArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.ticks.unsigned_abs() > MAX_LENGTH {
            return Ok(failed(format!("change by at most {MAX_LENGTH} ticks")));
        }
        let edge = match a.edge {
            EdgeArg::Start => ClipEdge::Start,
            EdgeArg::End => ClipEdge::End,
        };
        let edit = AiNoteEdit::Resize {
            edge,
            ticks: a.ticks,
        };
        Ok(self.edit_notes(a.take, a.notes, edit).await)
    }

    /// Gives notes one length, as one undo step.
    #[tool(annotations(
        title = "Set note length",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_note_length(
        &self,
        Parameters(a): Parameters<NoteLengthArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if !(1..=MAX_LENGTH).contains(&a.length) {
            return Ok(failed(format!("the length is 1 to {MAX_LENGTH} ticks")));
        }
        let edit = AiNoteEdit::Set {
            field: NoteField::Length,
            value: a.length as i64,
        };
        Ok(self.edit_notes(a.take, a.notes, edit).await)
    }

    /// Sets the velocity of notes (one value, or a change of each), as one undo step.
    #[tool(annotations(
        title = "Set velocity",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn set_velocity(
        &self,
        Parameters(a): Parameters<VelocityArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let edit = match (a.value, a.delta) {
            (Some(v), None) if (1..=127).contains(&v) => AiNoteEdit::Set {
                field: NoteField::Velocity,
                value: i64::from(v),
            },
            (None, Some(d)) if d != 0 && (-126..=126).contains(&d) => {
                AiNoteEdit::Velocity { delta: d }
            }
            _ => {
                return Ok(failed(
                    "give `value` (1 to 127) or `delta` (-126 to 126, not 0), one of them",
                ));
            }
        };
        Ok(self.edit_notes(a.take, a.notes, edit).await)
    }

    /// Brings notes toward a grid (with strength and swing), as one undo step. Where they
    /// were played is kept: unquantize_notes takes them back there.
    #[tool(annotations(
        title = "Quantize notes",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn quantize_notes(
        &self,
        Parameters(a): Parameters<QuantizeArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let grid = u16::try_from(a.grid)
            .ok()
            .and_then(|g| Grid::new(g, a.triplet));
        let align = match a.ends {
            true => QuantizeAlign::StartAndEnd,
            false => QuantizeAlign::Start,
        };
        let strength = a.strength.unwrap_or(100);
        let swing = a.swing.unwrap_or(0);
        let settings = grid
            .filter(|_| (1..=100).contains(&strength))
            .and_then(|grid| {
                QuantizeSettings::new(grid, strength as u8, swing.min(255) as u8, align)
            });
        let Some(settings) = settings else {
            return Ok(failed(
                "the grid is 0 (a bar), 2, 4, 8, 16 or 32; strength 1 to 100; swing 0 to 100",
            ));
        };
        Ok(self
            .edit_notes(a.take, a.notes, AiNoteEdit::Quantize(settings))
            .await)
    }

    /// Takes notes back to where they were played, as one undo step.
    #[tool(annotations(
        title = "Unquantize notes",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn unquantize_notes(
        &self,
        Parameters(a): Parameters<PickedArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        Ok(self
            .edit_notes(a.take, a.notes, AiNoteEdit::Unquantize)
            .await)
    }
}
