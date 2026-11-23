//! The MCP tools. Each call checks its input here, then goes to the core on a thread of the
//! blocking pool, so the runtime's one thread never waits on the core. The tools of each
//! group are in a module of their own; this one has the song, the notes and what they share.

macro_rules! check {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(result) => return Ok(result),
        }
    };
}

mod arrange;
mod automation;
mod batch;
mod files;
mod mixer;
mod notes;
mod render;
mod timing;

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meloet_core::ai::{
    AiAnswer, AiAsk, AiBusy, AiDone, AiInstrument, AiMeasure, AiOutcome, AiPlace, AiRefusal, AiSong,
};
use meloet_model::{BuiltinInstrument, Note, TempoMap, TickRange, Track};
use meloet_protocol::Command;
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

use crate::scratch::Scratch;
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

/// The answer to open_song and new_song while the song has unsaved changes (the lead's
/// words, 2026-11-23).
const UNSAVED: &str = "保存していない変更があります。先に save_song するか、人に聞いてください。";

/// Longest path a tool takes.
const MAX_PATH: usize = 4096;
/// How long a save, an open, a file brought in or a plugin's step is waited for.
const WAIT_FOR: Duration = Duration::from_secs(30);

const INSTRUCTIONS: &str = "meloet is a DAW for people who have just bought a MIDI keyboard. \
These tools read and change the song open in meloet right now. Positions are \"bar.beat.tick\" \
(bar and beat from 1, the tick from 0; 1920 ticks are a quarter note) or plain ticks from the \
song's start. Keys are MIDI numbers (60 is C4). Tracks are numbered from 1 as song_overview \
lists them; insert slots from 1 to 4.\n\
Groups: song and notes (song_overview, get_notes, write_notes, delete_notes, move_notes, \
resize_notes, set_note_length, set_velocity, quantize_notes, unquantize_notes); time \
(set_tempo, set_meter, markers, loop); tracks and clips (add_track, delete_track, clips, takes, \
import_audio, clip gain, fades, pitch and stretch); sounds (list_sounds, list_instruments, \
set_instrument); mixer (get_mixer, list_effects, volume, pan, sends, mute, solo, effects and \
their parameters, plugin parameters); automation (get_automation, lanes and points); files \
(save_song, save_song_as, open_song, new_song, export_song); listening (measure_mix, \
render_audio, render_note, get_clip_audio).\n\
Every change of the song is one step the person can see in meloet's undo history and take \
back; `undo` takes back the last step, whoever made it. Nothing can be changed while meloet \
records, counts in or exports. Ask before changing the song when the person did not ask for \
it, and before open_song or new_song when the song has unsaved changes. You cannot hear \
audio: measure_mix gives numbers; render_audio, render_note and get_clip_audio write WAV \
files into a private temporary folder and give their paths for a program of yours to read. \
Those files are deleted after an hour.";

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
    scratch: Arc<Scratch>,
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
    /// A built-in sound's name from list_sounds, like "grandPiano".
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub sound: Option<String>,
    /// An instrument plugin's key from list_instruments.
    #[serde(default)]
    #[schemars(length(min = 1, max = 1024))]
    pub plugin: Option<String>,
    /// A preset of an added SF2 file, from list_instruments.
    #[serde(default)]
    pub sf2: Option<song::Sf2Arg>,
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
        AiRefusal::Cannot(why) => why.clone(),
        AiRefusal::BatchOpen => {
            "a batch of another call is changing the song now; try again in a moment".into()
        }
        AiRefusal::NotInBatch => "that cannot be a part of a batch: it ends later (a save, an \
            open, a file brought in, a plugin's parameter), or it is undo or a read; call it by \
            itself"
            .into(),
        AiRefusal::BatchLost => "the batch is not open any more, or someone else changed the \
            song or its undo history while it ran"
            .into(),
        AiRefusal::TooManyTracks => "the song has as many tracks as meloet can hold".into(),
        AiRefusal::NothingToUndo => "there is no step to undo".into(),
        AiRefusal::Measuring => "another measurement is running; try again after it".into(),
        AiRefusal::ExportFailed(why) => format!("the rendering failed ({why})"),
        AiRefusal::Said(said) => format!("meloet did nothing ({said})"),
        AiRefusal::NoSuchSound => {
            "there is no such sound; call list_instruments to see them".into()
        }
        AiRefusal::NoSuchEffect => "there is no such effect; call list_effects to see them".into(),
        AiRefusal::NoSuchPlugin => "there is no plugin there; call get_mixer".into(),
        AiRefusal::PluginNotLoaded => {
            "the plugin is not loaded yet, or it stopped; try again in a moment".into()
        }
        AiRefusal::HasLane => {
            "that parameter has an automation lane; write the lane (or delete it) instead".into()
        }
        AiRefusal::NoSuchLane => {
            "the track has no lane for that; add it with add_lane first".into()
        }
        AiRefusal::NoSuchPoint => "there is no such point; call get_automation again".into(),
        AiRefusal::SlotTaken => "that insert slot holds an effect already".into(),
        AiRefusal::SlotEmpty => "that insert slot is empty".into(),
        AiRefusal::Unsaved => UNSAVED.into(),
        AiRefusal::NeverSaved => {
            "the song was never saved; call save_song_as with a folder and a name".into()
        }
        AiRefusal::FilesBusy => {
            "a save or an open is going on, or meloet asks the person something; try later".into()
        }
        AiRefusal::Exists(path) => format!(
            "{} is there already; give `overwrite: true` to write over it",
            path.display()
        ),
        AiRefusal::NothingToRender => "there is nothing to render".into(),
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
        "changed": true,
        "step": d.step,
        "undo_steps": d.undo_count,
        "clip": d.clip,
        "take": d.take,
        "track": d.track,
    })
}

/// A place given as "bar.beat.tick" or as ticks, one of them.
fn at_or_tick(map: &TempoMap, at: Option<&str>, tick: Option<u64>) -> Result<u64, CallToolResult> {
    let tick = match (at, tick) {
        (Some(at), None) => place(map, at)?,
        (None, Some(tick)) => tick,
        _ => return Err(failed("give `at` or `tick`, one of them")),
    };
    if tick > crate::MAX_TICK {
        return Err(failed(format!("that is past bar {MAX_BAR}")));
    }
    Ok(tick)
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
    pub fn new(meloet: Arc<dyn Meloet>, scratch: Arc<Scratch>) -> Tools {
        Tools {
            meloet,
            bucket: Arc::new(Mutex::new(Bucket {
                tokens: BURST,
                at: Instant::now(),
            })),
            scratch,
            tool_router: Self::tool_router()
                + Self::batch_router()
                + Self::mixer_router()
                + Self::notes_router()
                + Self::automation_router()
                + Self::timing_router()
                + Self::arrange_router()
                + Self::files_router()
                + Self::render_router(),
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

    async fn answer(&self, ask: AiAsk) -> Result<AiAnswer, CallToolResult> {
        let meloet = self.meloet.clone();
        tokio::task::spawn_blocking(move || meloet.ask(ask))
            .await
            .map_err(|_| closing())
    }

    async fn ask(&self, ask: AiAsk) -> Result<AiDone, CallToolResult> {
        let what = ask.name();
        match self.answer(ask).await? {
            AiAnswer::Done(d) => Ok(*d),
            AiAnswer::Refused(AiRefusal::Unchanged) => Err(self.no_change().await),
            AiAnswer::Refused(AiRefusal::Unsaved) => Err(failed(UNSAVED)),
            AiAnswer::Refused(r) => Err(failed(format!("Could not {what}: {}.", refusal_text(&r)))),
            AiAnswer::Params(_) => Err(closing()),
        }
    }

    /// The answer when the song is as asked already: not a failure, and no step.
    async fn no_change(&self) -> CallToolResult {
        let read = match self.read().await {
            Ok(read) => read,
            Err(result) => return result,
        };
        let value = json!({ "changed": false, "step": null, "undo_steps": read.undo_count });
        done(
            format!("No change: the song already is like that.\n{value}"),
            value,
        )
    }

    async fn write(&self, ask: AiAsk) -> CallToolResult {
        let what = ask.name();
        match self.ask(ask).await {
            Ok(d) => {
                let value = done_value(&d);
                done(format!("Done: {what}.\n{value}"), value)
            }
            Err(result) => result,
        }
    }

    /// One of the screen's intents, as one step.
    async fn screen(&self, command: Command) -> CallToolResult {
        self.write(AiAsk::Screen(command)).await
    }

    /// An ask answered with a ticket, and then how it ended.
    async fn ticketed(&self, ask: AiAsk) -> Result<(AiDone, Option<PathBuf>), CallToolResult> {
        let what = ask.name();
        let d = self.ask(ask).await?;
        let Some(ticket) = d.ticket else {
            return Ok((d, None));
        };
        let until = Instant::now() + WAIT_FOR;
        loop {
            let read = self.read().await?;
            let outcome = (read.outcomes.iter()).find(|(t, _)| *t == ticket);
            match outcome.map(|(_, o)| o) {
                Some(AiOutcome::Pending) if Instant::now() < until => {}
                Some(AiOutcome::Pending) => {
                    return Err(failed(format!(
                        "Could not {what}: meloet did not finish in {} s; call song_overview later",
                        WAIT_FOR.as_secs()
                    )));
                }
                Some(AiOutcome::Done(path)) => {
                    let mut d = d;
                    d.undo_count = read.undo_count;
                    return Ok((d, path.clone()));
                }
                Some(AiOutcome::Failed(why)) => {
                    return Err(failed(format!("Could not {what}: {why}.")));
                }
                None => return Err(closing()),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// An ask whose step comes later (a plugin's parameter): waits for it.
    async fn pending_step(&self, ask: AiAsk) -> CallToolResult {
        let what = ask.name();
        let d = match self.ask(ask).await {
            Ok(d) => d,
            Err(result) => return result,
        };
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let read = match self.read().await {
                Ok(read) => read,
                Err(result) => return result,
            };
            if read.undo_count > d.undo_count {
                let value = json!({ "undo_steps": read.undo_count });
                return done(format!("Done: {what}.\n{value}"), value);
            }
        }
        failed(format!(
            "Could not {what} as a step: the plugin was told, but no step came within 5 s. The \
             value may be the one it had, the plugin may not answer, or it may not keep that \
             value in its state (then the song does not hold it, and undo and saving miss it)."
        ))
    }
}

/// The track of number `number` (from 1) in what was read.
fn track_of(read: &AiSong, number: u32) -> Result<&Arc<Track>, CallToolResult> {
    (number.checked_sub(1))
        .and_then(|i| read.song.doc.tracks.get(i as usize))
        .ok_or_else(|| failed(refusal_text(&AiRefusal::NoSuchTrack(number))))
}

/// A path a tool may take: absolute, with no `..`, and not too long.
fn path_ok(text: &str) -> Result<PathBuf, CallToolResult> {
    let path = Path::new(text);
    if text.len() > MAX_PATH || text.contains('\0') {
        return Err(failed("that path is too long or holds a NUL"));
    }
    if !path.is_absolute() || path.components().any(|c| c == Component::ParentDir) {
        return Err(failed(
            "give an absolute path (from /) with no \"..\" in it",
        ));
    }
    Ok(path.to_path_buf())
}

/// A sound as list_sounds and list_instruments name it.
fn instrument_of(
    sound: Option<&str>,
    plugin: Option<&str>,
    sf2: Option<&song::Sf2Arg>,
) -> Result<AiInstrument, CallToolResult> {
    match (sound, plugin, sf2) {
        (Some(sound), None, None) => BuiltinInstrument::from_name(sound.trim())
            .map(AiInstrument::Builtin)
            .ok_or_else(|| failed(format!("there is no sound \"{sound}\"; call list_sounds"))),
        (None, Some(key), None) if !key.is_empty() && key.len() <= 1024 => {
            Ok(AiInstrument::Plugin(key.to_owned()))
        }
        (None, None, Some(s)) => {
            let (Ok(bank), Ok(program)) = (u16::try_from(s.bank), u8::try_from(s.program)) else {
                return Err(failed("the SF2 bank is 0 to 128 and the program 0 to 127"));
            };
            if bank > 128 || program > 127 || s.file.is_empty() || s.file.len() > 255 {
                return Err(failed("the SF2 bank is 0 to 128 and the program 0 to 127"));
            }
            Ok(AiInstrument::Sf2 {
                file: s.file.clone(),
                bank,
                program,
            })
        }
        _ => Err(failed("give one of `sound`, `plugin` or `sf2`")),
    }
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

    /// Writes notes, as one undo step: into a track (the clip at the earliest start, or a new
    /// 4-bar clip) or into a keys clip, which grows by bars when notes go past it. Nothing is
    /// written when a clip would overlap another.
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

    /// Adds a keys track below the last one with a built-in sound, an instrument plugin or
    /// an SF2 preset (see list_instruments). One undo step.
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
        let instrument = check!(instrument_of(
            args.sound.as_deref(),
            args.plugin.as_deref(),
            args.sf2.as_ref()
        ));
        Ok(self.write(AiAsk::AddTrack { instrument }).await)
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
