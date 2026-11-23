//! Changing notes that are there: by the take's id and the notes' indexes from get_notes.

use std::borrow::Cow;

use meloet_core::ai::{
    AiAnswer, AiAsk, AiDone, AiNoteEdit, AiNoteWhy, AiPick, AiPlace, AiRefusal, AiShift,
};
use meloet_model::{ClipEdge, Grid, Note, NoteField, QuantizeAlign, QuantizeSettings, TempoMap};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    MAX_LENGTH, MAX_NOTES, NoteArgs, Tools, closing, done, done_value, failed, refusal_text,
};
use crate::MAX_BAR;
use crate::song::{self, Place};

/// The velocity of a note written as an array without one.
const ARRAY_VELOCITY: u32 = 100;

/// A note as write_notes takes it: the object form, or the array
/// `[start, length, key, velocity]`.
#[derive(Debug, Clone)]
pub struct NoteForm(pub NoteArgs);

impl<'de> Deserialize<'de> for NoteForm {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<NoteForm, D::Error> {
        use serde::de::Error;
        let parts = match Value::deserialize(d)? {
            Value::Array(parts) => parts,
            object @ Value::Object(_) => {
                return NoteArgs::deserialize(object)
                    .map(NoteForm)
                    .map_err(D::Error::custom);
            }
            _ => {
                return Err(D::Error::custom(
                    "a note is an object or an array [start, length, key, velocity]",
                ));
            }
        };
        if !(3..=4).contains(&parts.len()) {
            return Err(D::Error::custom(
                "a note as an array is [start, length, key] or [start, length, key, velocity]",
            ));
        }
        let number = |i: usize, name: &str| {
            parts[i].as_u64().ok_or_else(|| {
                D::Error::custom(format!("a note's {name} is a whole number, 0 or more"))
            })
        };
        let (at, start) = match &parts[0] {
            Value::String(at) => (Some(at.clone()), None),
            _ => (None, Some(number(0, "start")?)),
        };
        let small = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
        Ok(NoteForm(NoteArgs {
            at,
            start,
            length: number(1, "length")?,
            key: small(number(2, "key")?),
            velocity: match parts.get(3) {
                Some(_) => small(number(3, "velocity")?),
                None => ARRAY_VELOCITY,
            },
        }))
    }
}

impl schemars::JsonSchema for NoteForm {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "Note".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let object = generator.subschema_for::<NoteArgs>();
        schemars::json_schema!({
            "description": "A note as an object, or shorter as the array [start, length, key, \
                velocity]: start in ticks (or \"bar.beat.tick\" text), length in ticks, key 0 to \
                127, velocity 1 to 127 (left out: 100).",
            "anyOf": [
                object,
                {
                    "type": "array",
                    "minItems": 3,
                    "maxItems": 4,
                    "items": { "type": ["integer", "string"] }
                }
            ]
        })
    }
}

/// The notes of a write_notes call, checked; with `origin` every start counts from it.
pub(super) fn notes_in(
    map: &TempoMap,
    forms: &[NoteForm],
    origin: Option<u64>,
) -> Result<Vec<Note>, CallToolResult> {
    let mut notes = Vec::with_capacity(forms.len());
    for (i, NoteForm(n)) in forms.iter().enumerate() {
        let start = match (&n.at, n.start, origin) {
            (Some(at), None, None) => song::parse_at(map, at).map_err(failed)?,
            (Some(_), None, Some(_)) => {
                return Err(failed(format!(
                    "note {i}: with `origin`, give the start in ticks from it"
                )));
            }
            (None, Some(start), origin) => start.saturating_add(origin.unwrap_or(0)),
            _ => {
                return Err(failed(format!(
                    "note {i}: give `at` or `start`, one of them"
                )));
            }
        };
        if start > crate::MAX_TICK {
            return Err(failed(format!("note {i}: the start is past bar {MAX_BAR}")));
        }
        if !(1..=MAX_LENGTH).contains(&n.length) {
            return Err(failed(format!(
                "note {i}: the length is 1 to {MAX_LENGTH} ticks"
            )));
        }
        let (Ok(key), Ok(velocity)) = (u8::try_from(n.key), u8::try_from(n.velocity)) else {
            return Err(failed(format!(
                "note {i}: the key or the velocity is out of range"
            )));
        };
        if key > 127 || !(1..=127).contains(&velocity) {
            return Err(failed(format!(
                "note {i}: the key is 0 to 127 and the velocity 1 to 127"
            )));
        }
        notes.push(Note {
            start,
            length: n.length,
            key,
            velocity,
            from: None,
        });
    }
    Ok(notes)
}

/// The fixed word for why a note was not written.
fn why_name(why: AiNoteWhy) -> &'static str {
    match why {
        AiNoteWhy::PastClipEnd => "past_new_clip_end",
        AiNoteWhy::PastSongEnd => "past_song_end",
        AiNoteWhy::BeforeSongStart => "before_song_start",
        AiNoteWhy::KeyOutOfRange => "key_out_of_range",
        AiNoteWhy::OutsideSourceClip => "outside_source_clip",
    }
}

fn why_text(why: AiNoteWhy) -> &'static str {
    match why {
        AiNoteWhy::PastClipEnd => "they start after the end of the clip made for them",
        AiNoteWhy::PastSongEnd => "they start past the end of the song meloet can hold",
        AiNoteWhy::BeforeSongStart => "they would start before the song's start",
        AiNoteWhy::KeyOutOfRange => "their keys would be outside 0 to 127",
        AiNoteWhy::OutsideSourceClip => "they do not sound in the clip (it was trimmed)",
    }
}

fn span(map: &TempoMap, (start, end): (u64, u64)) -> Value {
    json!({ "start": song::place(map, start), "end": song::place(map, end) })
}

/// What a write answers: the step, and for notes where they went, which were left out and
/// which were cut.
pub(super) fn notes_value(map: &TempoMap, d: &AiDone) -> Value {
    let mut value = done_value(d);
    let Some(n) = &d.notes else {
        return value;
    };
    let mut clip = span(map, n.now);
    clip["id"] = json!(n.clip);
    clip["track"] = json!(n.track);
    let not_written: Vec<Value> = (n.not_written.iter())
        .map(|w| {
            json!({
                "index": w.index,
                "start": w.note.start,
                "length": w.note.length,
                "key": w.note.key,
                "reason": why_name(w.why),
            })
        })
        .collect();
    let changed: Vec<Value> = (n.cut.iter())
        .map(|c| {
            json!({
                "index": c.index,
                "start": c.note.start,
                "key": c.note.key,
                "length_asked": c.note.length,
                "length": c.length,
                "reason": if c.at_clip_end { "cut_at_new_clip_end" } else { "cut_at_song_end" },
            })
        })
        .collect();
    value["written"] = json!(n.written);
    value["created_clip"] = if n.clip_made {
        clip.clone()
    } else {
        Value::Null
    };
    value["grew_to"] = match n.was {
        Some(was) if was != n.now => span(map, n.now),
        _ => Value::Null,
    };
    value["clip_place"] = clip;
    value["not_written_count"] = json!(n.not_written_count);
    value["not_written_more"] = json!(n.not_written_count as usize - not_written.len());
    value["not_written"] = json!(not_written);
    value["changed_count"] = json!(n.cut_count);
    value["changed_more"] = json!(n.cut_count as usize - changed.len());
    value["changed"] = json!(changed);
    value
}

/// One line on a write of notes, for the text of the answer.
pub(super) fn notes_line(d: &AiDone, value: &Value) -> String {
    let Some(n) = &d.notes else {
        return String::new();
    };
    let at = |v: &Value| v["at"].as_str().unwrap_or("?").to_owned();
    let place = &value["clip_place"];
    let mut line = format!(
        "{} notes written into {} {} on track {} ({} to {})",
        n.written,
        if n.clip_made { "the new clip" } else { "clip" },
        n.clip,
        n.track,
        at(&place["start"]),
        at(&place["end"]),
    );
    if !value["grew_to"].is_null() {
        line.push_str(", which grew");
    }
    line.push('.');
    if let Some(first) = n.not_written.first() {
        line.push_str(&format!(
            " {} NOT written: {} (see not_written).",
            n.not_written_count,
            why_text(first.why)
        ));
    }
    if n.cut_count > 0 {
        line.push_str(&format!(" {} written shorter (see changed).", n.cut_count));
    }
    line
}

/// A refusal with its places as "bar.beat.tick".
pub(super) fn refused_text(map: &TempoMap, r: &AiRefusal) -> String {
    match r {
        AiRefusal::OverlapsClip { clip, start, end } => format!(
            "clip {clip} of the track ({} to {}) is in the way; nothing was done. Write into \
             that clip, or choose another place or track",
            song::at(map, *start),
            song::at(map, *end)
        ),
        AiRefusal::NoNoteFits(why) => {
            format!("no note can be written: {}", why_text(*why))
        }
        r => refusal_text(r),
    }
}

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

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct CopyNotesArgs {
    /// The keys clip to copy from (its id, as song_overview gives it).
    #[schemars(range(min = 1))]
    pub clip: u32,
    /// The take's id from get_notes: when the clip shows another take now, nothing is done.
    /// Needed with `notes`.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub take: Option<u32>,
    /// Only these notes (indexes in the take, from get_notes). Leave out for every note.
    #[serde(default)]
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Option<Vec<u32>>,
    /// Only the notes that start from here on (with `to`).
    #[serde(default)]
    pub from: Option<Place>,
    /// Only the notes that start before here (with `from`).
    #[serde(default)]
    pub to: Option<Place>,
    /// The keys track to copy into (its number from 1): the clip there that holds the first
    /// copied note, or a new clip as long as the notes need. Give this or `to_clip`.
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub to_track: Option<u32>,
    /// The keys clip to copy into; it grows by bars to hold the notes. Give this or `to_track`.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub to_clip: Option<u32>,
    /// Move the copies by ticks (below 0: earlier). Give at most one of `shift_ticks`,
    /// `shift_bars` and `at`; with none the copies keep their places.
    #[serde(default)]
    pub shift_ticks: Option<i64>,
    /// Move the copies by whole bars (below 0: earlier).
    #[serde(default)]
    #[schemars(range(min = -9999, max = 9999))]
    pub shift_bars: Option<i32>,
    /// Where the start of the clip copied from goes (the place `from` when a range is
    /// given): rests before the first note are kept.
    #[serde(default)]
    pub at: Option<Place>,
    /// Keys up (down when below 0).
    #[serde(default)]
    #[schemars(range(min = -127, max = 127))]
    pub transpose: Option<i32>,
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

    /// A write whose answer names places: the step, and for notes where they went and
    /// which were left out.
    pub(super) async fn put(
        &self,
        ask: AiAsk,
        map: &TempoMap,
    ) -> Result<(AiDone, Value), CallToolResult> {
        let what = ask.name();
        match self.answer(ask).await? {
            AiAnswer::Done(d) => {
                let value = notes_value(map, &d);
                Ok((*d, value))
            }
            AiAnswer::Refused(r) => Err(failed(format!(
                "Could not {what}: {}.",
                refused_text(map, &r)
            ))),
            _ => Err(closing()),
        }
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

    /// Copies notes of a clip (all, `notes`, or those starting in `from`..`to`) into
    /// another keys track or clip, moved and transposed if asked, as one undo step. The
    /// source is not changed. Only notes that sound are copied. Refused, naming the clip,
    /// when another clip is in the way. Answers the clip written and `not_written` with
    /// reasons.
    #[tool(annotations(
        title = "Copy notes",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn copy_notes(
        &self,
        Parameters(a): Parameters<CopyNotesArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let to = match (a.to_track, a.to_clip) {
            (Some(track), None) => AiPlace::Track(track),
            (None, Some(clip)) => AiPlace::Clip(clip),
            _ => return Ok(failed("give `to_track` or `to_clip`, one of them")),
        };
        let transpose = a.transpose.unwrap_or(0);
        if !(-127..=127).contains(&transpose) {
            return Ok(failed("transpose by -127 to 127 keys"));
        }
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let tick = |p: &Place| p.tick(&map).map_err(failed);
        let pick = match (&a.notes, &a.from, &a.to) {
            (None, None, None) => AiPick::All,
            (Some(notes), None, None) => {
                check!(picked(notes));
                if a.take.is_none() {
                    return Ok(failed(
                        "with `notes`, give the `take` id from get_notes too",
                    ));
                }
                AiPick::Notes(notes.clone())
            }
            (None, Some(from), Some(to)) => {
                let (from, to) = (check!(tick(from)), check!(tick(to)));
                if from >= to {
                    return Ok(failed("`from` must be before `to`"));
                }
                AiPick::Range { from, to }
            }
            _ => {
                return Ok(failed(
                    "give `notes`, or `from` and `to`, or none of them for every note",
                ));
            }
        };
        let shift = match (a.shift_ticks, a.shift_bars, &a.at) {
            (None, None, None) => AiShift::Ticks(0),
            (Some(ticks), None, None) if ticks.unsigned_abs() <= crate::MAX_TICK => {
                AiShift::Ticks(ticks)
            }
            (None, Some(bars), None) if i64::from(bars).unsigned_abs() <= MAX_BAR => {
                AiShift::Bars(i64::from(bars))
            }
            (None, None, Some(at)) => AiShift::To(check!(tick(at))),
            _ => {
                return Ok(failed(
                    "give at most one of `shift_ticks`, `shift_bars` (up to 9999) and `at`",
                ));
            }
        };
        let ask = AiAsk::CopyNotes {
            clip: a.clip,
            take: a.take,
            pick,
            to,
            shift,
            transpose,
        };
        let (d, value) = check!(self.put(ask, &map).await);
        let line = notes_line(&d, &value);
        Ok(done(format!("Copied: {line}\n{value}"), value))
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
