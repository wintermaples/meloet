//! The MCP tools. Each call checks its input here, then goes to the core on a thread of the
//! blocking pool, so the runtime's one thread never waits on the core.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use meloet_core::ai::{AiAnswer, AiAsk, AiBusy, AiDone, AiMeasure, AiPlace, AiRefusal, AiSong};
use meloet_model::{BuiltinInstrument, InstrumentRef, Note, TempoMap, TickRange};
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProgressNotificationParam, RequestMetaObject,
    ServerCapabilities, ServerConfig,
};
use rmcp::{
    ErrorData as McpError, Peer, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::song::{self, parse_at};
use crate::{MAX_BAR, Meloet};

/// Notes one `write_notes` takes, and one `get_notes` gives.
pub const MAX_NOTES: usize = 2000;
/// Characters of a clip's or a marker's name.
pub const MAX_NAME: usize = 64;
/// A note's length: 64 bars of 4/4.
pub const MAX_LENGTH: u64 = 1920 * 4 * 64;
/// What one `measure_mix` may render, in seconds of the song.
pub const MAX_MEASURE_S: f64 = 600.0;
/// Calls in a burst, and calls a second after it, over every connection together.
const BURST: f64 = 30.0;
const PER_SECOND: f64 = 10.0;

const INSTRUCTIONS: &str = "meloet is a DAW for people who have just bought a MIDI keyboard. \
These tools read and change the song open in meloet right now. Positions are \"bar.beat.tick\" \
(bar and beat from 1, the tick from 0; 1920 ticks are a quarter note) or plain ticks from the \
song's start. Keys are MIDI numbers (60 is C4, middle C). Every write is one step the person \
can see in meloet's undo history and take back; `undo` takes back the last step, whoever made it. \
Nothing can be written while meloet records, counts in or exports. Ask before changing the song \
when the person did not ask for it. You cannot hear audio: measure_mix gives numbers \
(peak, RMS, places over 0 dB) of a range rendered as the export would.";

/// Calls allowed over time (a token bucket).
#[derive(Debug)]
struct Bucket {
    tokens: f64,
    at: Instant,
}

impl Bucket {
    fn take(&mut self) -> bool {
        let now = Instant::now();
        let gained = now.duration_since(self.at).as_secs_f64() * PER_SECOND;
        self.tokens = (self.tokens + gained).min(BURST);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// One per connection; what they share is behind `Arc`s.
#[derive(Clone)]
pub struct Tools {
    meloet: Arc<dyn Meloet>,
    bucket: Arc<Mutex<Bucket>>,
    tool_router: ToolRouter<Tools>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ClipArgs {
    /// The clip's id, as song_overview gives it.
    #[schemars(range(min = 1))]
    pub clip: u32,
    /// The first note to give (index in the take, from 0).
    #[serde(default)]
    pub first: Option<u32>,
    /// How many notes to give at most (default and most: 2000).
    #[serde(default)]
    #[schemars(range(min = 1, max = 2000))]
    pub max: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct NoteArgs {
    /// Where the note starts, as "bar.beat.tick". Give this or `start`.
    #[serde(default)]
    pub at: Option<String>,
    /// Where the note starts, in ticks from the song's start. Give this or `at`.
    #[serde(default)]
    pub start: Option<u64>,
    /// In ticks (1920 is a quarter note).
    #[schemars(range(min = 1, max = 491520))]
    pub length: u64,
    /// MIDI key, 0 to 127 (60 is C4).
    #[schemars(range(min = 0, max = 127))]
    pub key: u32,
    /// 1 to 127.
    #[schemars(range(min = 1, max = 127))]
    pub velocity: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct WriteNotesArgs {
    /// The track's number (from 1): the notes go into the clip at the earliest start, or a
    /// new clip. Give this or `clip`.
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    /// A keys clip's id: it grows when notes go past its end. Give this or `track`.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub clip: Option<u32>,
    /// 1 to 2000 notes.
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Vec<NoteArgs>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct DeleteNotesArgs {
    /// The take's id, as get_notes gives it. When the clip shows another take now (the song
    /// changed since), nothing is deleted.
    #[schemars(range(min = 1))]
    pub take: u32,
    /// The notes' indexes in that take, as get_notes gives them.
    #[schemars(length(min = 1, max = 2000))]
    pub notes: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct CreateClipArgs {
    /// The keys track's number (from 1).
    #[schemars(range(min = 1, max = 64))]
    pub track: u32,
    /// Where, as "bar.beat.tick": the clip starts at the head of that bar and is 4 bars long,
    /// or up to the next clip.
    pub at: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct RenameClipArgs {
    #[schemars(range(min = 1))]
    pub clip: u32,
    /// 1 to 64 characters.
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SetTempoArgs {
    /// Quarter notes a minute, 30 to 300, in steps of 0.01.
    #[schemars(range(min = 30.0, max = 300.0))]
    pub bpm: f64,
    /// From where, as "bar.beat.tick" (snapped to the nearest beat). Leave out for the
    /// song's start, which is the tempo of the whole song when it has no tempo changes.
    #[serde(default)]
    pub at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct AddMarkerArgs {
    /// 1 to 64 characters.
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
    /// Where, as "bar.beat.tick" (snapped to the nearest beat).
    pub at: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct AddTrackArgs {
    /// A sound's name from list_sounds, like "grandPiano".
    #[schemars(length(min = 1, max = 64))]
    pub sound: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MeasureArgs {
    /// The first bar (from 1).
    #[schemars(range(min = 1, max = 9999))]
    pub from_bar: u32,
    /// The last bar, measured to its end.
    #[schemars(range(min = 1, max = 9999))]
    pub to_bar: u32,
    /// Track numbers (from 1) to measure, each alone; the mix then holds only these. Leave
    /// out for every track.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub tracks: Option<Vec<u32>>,
}

fn text(t: impl Into<String>) -> Vec<ContentBlock> {
    vec![ContentBlock::text(t.into())]
}

fn failed(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(text(message))
}

fn done(summary: String, value: Value) -> CallToolResult {
    let mut result = CallToolResult::success(text(summary));
    result.structured_content = Some(value);
    result
}

fn closing() -> CallToolResult {
    failed("meloet is closing or does not answer.")
}

fn busy_text(busy: AiBusy) -> String {
    format!(
        "meloet is {} now; the song cannot change until that ends.",
        song::busy_name(busy)
    )
}

pub fn refusal_text(r: &AiRefusal) -> String {
    match r {
        AiRefusal::Busy(busy) => busy_text(*busy),
        AiRefusal::Closing => "meloet is closing or does not answer.".into(),
        AiRefusal::NoSuchTrack(n) => {
            format!("there is no track {n}; call song_overview to see the tracks")
        }
        AiRefusal::NoSuchClip(c) => {
            format!("there is no clip {c}; call song_overview to see the clips")
        }
        AiRefusal::NoSuchTake(t) => format!(
            "no clip shows take {t} now (the song changed since it was read); nothing was done. \
             Call get_notes again"
        ),
        AiRefusal::NoSuchNote(i) => format!("the take has no note {i}; nothing was done"),
        AiRefusal::NotKeys => "that is a vocal track or clip, which holds audio, not notes".into(),
        AiRefusal::Overlap => "it would overlap another clip of the track; nothing was done".into(),
        AiRefusal::OffTheSong => "that is past the end of the song meloet can hold".into(),
        AiRefusal::BadNote => "a note is out of range".into(),
        AiRefusal::BadTempo => "the tempo must be 30 to 300".into(),
        AiRefusal::Unchanged => "that leaves the song as it is; nothing was done".into(),
        AiRefusal::TooManyTracks => "the song has as many tracks as meloet can hold".into(),
        AiRefusal::NothingToUndo => "there is no step to undo".into(),
        AiRefusal::Measuring => "another measurement is running; try again after it".into(),
        AiRefusal::ExportFailed(why) => format!("the rendering failed ({why})"),
    }
}

fn place(map: &TempoMap, text: &str) -> Result<u64, CallToolResult> {
    parse_at(map, text).map_err(failed)
}

fn name_ok(name: &str) -> Result<String, CallToolResult> {
    let name = name.trim();
    let count = name.chars().count();
    if count == 0 || count > MAX_NAME || name.chars().any(char::is_control) {
        return Err(failed(format!(
            "a name is 1 to {MAX_NAME} characters, with no control characters"
        )));
    }
    Ok(name.to_owned())
}

fn done_value(d: &AiDone) -> Value {
    json!({
        "step": d.step,
        "undo_steps": d.undo_count,
        "clip": d.clip,
        "take": d.take,
        "track": d.track,
    })
}

fn db(level: f32) -> Option<f64> {
    (level > 0.0).then(|| (f64::from(level).log10() * 2000.0).round() / 100.0)
}

fn levels_value(l: &meloet_core::ai::PassLevels, rate: u32) -> Value {
    json!({
        "peak_dbfs": db(l.peak),
        "rms_dbfs": db(l.rms),
        "over_0db_places": l.over,
        "seconds": if rate > 0 { (l.frames as f64 / f64::from(rate) * 100.0).round() / 100.0 } else { 0.0 },
    })
}

fn shown_db(v: &Value) -> String {
    v.as_f64()
        .map_or_else(|| "silent".to_owned(), |db| format!("{db:.1} dBFS"))
}

impl Tools {
    pub fn new(meloet: Arc<dyn Meloet>) -> Tools {
        Tools {
            meloet,
            bucket: Arc::new(Mutex::new(Bucket {
                tokens: BURST,
                at: Instant::now(),
            })),
            tool_router: Self::tool_router(),
        }
    }

    fn allowed(&self) -> Result<(), CallToolResult> {
        let mut bucket = self.bucket.lock().unwrap_or_else(|e| e.into_inner());
        if bucket.take() {
            Ok(())
        } else {
            Err(failed(
                "too many calls in a short time; wait a second and try again",
            ))
        }
    }

    async fn read(&self) -> Result<AiSong, CallToolResult> {
        let meloet = self.meloet.clone();
        match tokio::task::spawn_blocking(move || meloet.song()).await {
            Ok(Some(read)) => Ok(read),
            _ => Err(closing()),
        }
    }

    async fn write(&self, ask: AiAsk) -> CallToolResult {
        let what = ask.name();
        let meloet = self.meloet.clone();
        match tokio::task::spawn_blocking(move || meloet.ask(ask)).await {
            Ok(AiAnswer::Done(d)) => {
                let value = done_value(&d);
                done(format!("Done: {what}.\n{value}"), value)
            }
            Ok(AiAnswer::Refused(r)) => failed(format!("Could not {what}: {}.", refusal_text(&r))),
            Err(_) => closing(),
        }
    }
}

macro_rules! check {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(result) => return Ok(result),
        }
    };
}

#[tool_router]
impl Tools {
    /// The song open in meloet: name, tempo and meter changes, length, markers, loop, the
    /// scale lock, every track (number, name, keys or vocal, sound, mute, solo, volume) and
    /// every clip (id, track, start and end, name, takes, notes of the shown take).
    #[tool(annotations(
        title = "Song overview",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn song_overview(&self) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let value = song::overview(&read);
        let mut lines = vec![format!(
            "\"{}\": {} tracks, {} clips, ends at {}.",
            read.song.doc.name,
            read.song.doc.tracks.len(),
            value["clips"].as_array().map_or(0, Vec::len),
            value["length"]["end"]["at"].as_str().unwrap_or("1.1.0"),
        )];
        if let Some(busy) = read.busy {
            lines.push(busy_text(busy));
        }
        lines.push(value.to_string());
        Ok(done(lines.join("\n"), value))
    }

    /// The notes of the take a clip shows: the take's id and each note's index, start (ticks
    /// and "bar.beat.tick"), length, key (number and name), velocity, and whether it is inside
    /// the clip (a trimmed clip hides some). Use the take id and indexes with delete_notes.
    #[tool(annotations(title = "Get notes", read_only_hint = true, open_world_hint = false))]
    async fn get_notes(
        &self,
        Parameters(args): Parameters<ClipArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let max = args
            .max
            .unwrap_or(MAX_NOTES as u32)
            .clamp(1, MAX_NOTES as u32);
        let read = check!(self.read().await);
        let first = args.first.unwrap_or(0) as usize;
        let value = check!(song::notes(&read, args.clip, first, max as usize).map_err(failed));
        let summary = format!(
            "Clip {} shows take {} with {} notes.\n{}",
            args.clip, value["take"], value["total"], value
        );
        Ok(done(summary, value))
    }

    /// The sounds a new track can have (meloet's own; give the name to add_track).
    #[tool(annotations(title = "List sounds", read_only_hint = true, open_world_hint = false))]
    async fn list_sounds(&self) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let value = song::sounds();
        Ok(done(value.to_string(), value))
    }

    /// Writes notes, as one undo step. Into a track (the clip at the earliest start, or a new
    /// clip from the head of that bar, 4 bars or up to the next clip) or into a keys clip,
    /// which grows by whole bars when notes go past it. Nothing is written when a clip would
    /// overlap another. With the scale locked, keys outside it are written too.
    #[tool(annotations(
        title = "Write notes",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn write_notes(
        &self,
        Parameters(args): Parameters<WriteNotesArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let at = match (args.track, args.clip) {
            (Some(t), None) => AiPlace::Track(t),
            (None, Some(c)) => AiPlace::Clip(c),
            _ => return Ok(failed("give `track` or `clip`, one of them")),
        };
        if args.notes.is_empty() || args.notes.len() > MAX_NOTES {
            return Ok(failed(format!("give 1 to {MAX_NOTES} notes")));
        }
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let mut notes = Vec::with_capacity(args.notes.len());
        for (i, n) in args.notes.iter().enumerate() {
            let start = match (&n.at, n.start) {
                (Some(at), None) => check!(place(&map, at)),
                (None, Some(start)) => start,
                _ => {
                    return Ok(failed(format!(
                        "note {i}: give `at` or `start`, one of them"
                    )));
                }
            };
            if start > crate::MAX_TICK {
                return Ok(failed(format!("note {i}: the start is past bar {MAX_BAR}")));
            }
            if !(1..=MAX_LENGTH).contains(&n.length) {
                return Ok(failed(format!(
                    "note {i}: the length is 1 to {MAX_LENGTH} ticks"
                )));
            }
            let (Ok(key), Ok(velocity)) = (u8::try_from(n.key), u8::try_from(n.velocity)) else {
                return Ok(failed(format!(
                    "note {i}: the key or the velocity is out of range"
                )));
            };
            if key > 127 || !(1..=127).contains(&velocity) {
                return Ok(failed(format!(
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
        Ok(self.write(AiAsk::WriteNotes { at, notes }).await)
    }

    /// Deletes notes of a take, by the take's id and the notes' indexes from get_notes, as one
    /// undo step. When the clip shows another take now, nothing is deleted.
    #[tool(annotations(
        title = "Delete notes",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_notes(
        &self,
        Parameters(args): Parameters<DeleteNotesArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if args.notes.is_empty() || args.notes.len() > MAX_NOTES {
            return Ok(failed(format!("give 1 to {MAX_NOTES} note indexes")));
        }
        Ok(self
            .write(AiAsk::DeleteNotes {
                take: args.take,
                notes: args.notes,
            })
            .await)
    }

    /// Makes an empty keys clip from the head of a bar, 4 bars long or up to the next clip,
    /// named after its track, as one undo step.
    #[tool(annotations(
        title = "Create clip",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn create_clip(
        &self,
        Parameters(args): Parameters<CreateClipArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let tick = check!(place(&map, &args.at));
        Ok(self
            .write(AiAsk::CreateClip {
                track: args.track,
                tick,
            })
            .await)
    }

    /// Renames a clip, as one undo step.
    #[tool(annotations(
        title = "Rename clip",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn rename_clip(
        &self,
        Parameters(args): Parameters<RenameClipArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let name = check!(name_ok(&args.name));
        Ok(self
            .write(AiAsk::RenameClip {
                clip: args.clip,
                name,
            })
            .await)
    }

    /// Sets the tempo from a place on (a tempo change there, or the song's start tempo), as
    /// one undo step.
    #[tool(annotations(
        title = "Set tempo",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_tempo(
        &self,
        Parameters(args): Parameters<SetTempoArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if !(30.0..=300.0).contains(&args.bpm) {
            return Ok(failed("the tempo must be 30 to 300"));
        }
        let tick = match &args.at {
            None => 0,
            Some(at) => {
                let read = check!(self.read().await);
                check!(place(&TempoMap::new(&read.song.doc.timeline), at))
            }
        };
        Ok(self
            .write(AiAsk::SetTempo {
                tick,
                bpm: args.bpm,
            })
            .await)
    }

    /// Adds a marker, as one undo step.
    #[tool(annotations(
        title = "Add marker",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn add_marker(
        &self,
        Parameters(args): Parameters<AddMarkerArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let name = check!(name_ok(&args.name));
        let read = check!(self.read().await);
        let tick = check!(place(&TempoMap::new(&read.song.doc.timeline), &args.at));
        Ok(self.write(AiAsk::AddMarker { tick, name }).await)
    }

    /// Adds a keys track with a sound from list_sounds below the last track, as one undo
    /// step.
    #[tool(annotations(
        title = "Add track",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn add_track(
        &self,
        Parameters(args): Parameters<AddTrackArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let Some(sound) = BuiltinInstrument::from_name(args.sound.trim()) else {
            return Ok(failed(format!(
                "there is no sound \"{}\"; call list_sounds",
                args.sound
            )));
        };
        Ok(self
            .write(AiAsk::AddTrack {
                instrument: InstrumentRef::Builtin(sound),
            })
            .await)
    }

    /// Takes back the last step of meloet's undo history (whoever made it), as the person's
    /// undo does.
    #[tool(annotations(
        title = "Undo",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn undo(&self) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        Ok(self.write(AiAsk::Undo).await)
    }

    /// Renders bars of the song as the export would (without the tail after the range) and
    /// gives numbers, not audio: the mix's peak (dBFS, before the output's clip), RMS, places
    /// over 0 dB, and the same for each track played alone with its sends. It takes a while
    /// (each track is one more rendering); progress is sent when asked for.
    #[tool(annotations(title = "Measure mix", read_only_hint = true, open_world_hint = false))]
    async fn measure_mix(
        &self,
        Parameters(args): Parameters<MeasureArgs>,
        meta: RequestMetaObject,
        peer: Peer<RoleServer>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if args.from_bar == 0 || args.to_bar < args.from_bar || u64::from(args.to_bar) > MAX_BAR {
            return Ok(failed(format!("give 1 <= from_bar <= to_bar <= {MAX_BAR}")));
        }
        if args
            .tracks
            .as_ref()
            .is_some_and(|t| t.is_empty() || t.len() > 64)
        {
            return Ok(failed("give 1 to 64 track numbers, or leave `tracks` out"));
        }
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let start = map.tick_of_bar(u64::from(args.from_bar) - 1);
        let end = map.tick_of_bar(u64::from(args.to_bar));
        let seconds = map.seconds_at(end as f64) - map.seconds_at(start as f64);
        if seconds > MAX_MEASURE_S {
            return Ok(failed(format!(
                "that is {seconds:.0} s of the song; measure at most {MAX_MEASURE_S:.0} s at a time"
            )));
        }
        let Some(span) = TickRange::new(start, end) else {
            return Ok(failed("the range is empty"));
        };
        let ask = AiMeasure {
            span,
            tracks: args.tracks.clone(),
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u8>();
        let meloet = self.meloet.clone();
        let stop = ct.clone();
        let mut job = tokio::task::spawn_blocking(move || {
            let mut progress = |percent| {
                let _ = tx.send(percent);
            };
            meloet.measure(ask, &mut progress, &|| stop.is_cancelled())
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
        let levels = match answer {
            Ok(Ok(levels)) => levels,
            Ok(Err(r)) => return Ok(failed(format!("Could not measure: {}.", refusal_text(&r)))),
            Err(_) => return Ok(closing()),
        };
        let rate = levels.sample_rate;
        let tracks: Vec<Value> = (levels.tracks.iter())
            .map(|(n, name, l)| {
                let mut v = levels_value(l, rate);
                v["number"] = json!(n);
                v["name"] = json!(name);
                v
            })
            .collect();
        let master = levels_value(&levels.master, rate);
        let mut lines = vec![format!(
            "Bars {} to {}: the mix peaks at {}, RMS {}, {} places over 0 dB.",
            args.from_bar,
            args.to_bar,
            shown_db(&master["peak_dbfs"]),
            shown_db(&master["rms_dbfs"]),
            levels.master.over,
        )];
        for t in &tracks {
            lines.push(format!(
                "Track {} \"{}\": peak {}, RMS {}.",
                t["number"],
                t["name"].as_str().unwrap_or(""),
                shown_db(&t["peak_dbfs"]),
                shown_db(&t["rms_dbfs"]),
            ));
        }
        let value = json!({
            "from_bar": args.from_bar,
            "to_bar": args.to_bar,
            "sample_rate": rate,
            "master": master,
            "tracks": tracks,
        });
        lines.push(value.to_string());
        Ok(done(lines.join("\n"), value))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("meloet", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}
