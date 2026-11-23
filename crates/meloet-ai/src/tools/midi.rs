//! Standard MIDI files: bringing one into a keys track, and writing a clip or a range of
//! the song out as one. The files are read and written here, on the blocking pool; the
//! core only gets the notes, as one step.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use meloet_core::ai::{AiAsk, AiNote, AiPlace, AiRefusal, AiSong};
use meloet_model::{
    Clip, InstrumentGroup, InstrumentRef, TICKS_PER_QUARTER, TempoMap, Timeline, Track,
};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::notes::notes_line;
use super::{Tools, at_or_tick, closing, done, failed, path_ok, refusal_text};
use crate::smf::{self, OutFile, OutNote, OutTrack, Smf, SmfNote, SmfTrack};
use crate::song::{self, Place};
use crate::trackref::{TrackRef, id_text};

/// The largest MIDI file read.
const MAX_FILE: u64 = 8 << 20;
/// Notes one import_midi brings in (one step).
pub const MAX_IMPORT_NOTES: usize = 50_000;
/// Tempo and meter events of a file listed in an answer.
const MAX_LISTED: usize = 32;
/// A tempo ramp is written as steps this long (a sixteenth note).
const RAMP_STEP: u64 = TICKS_PER_QUARTER / 4;
/// The General MIDI drum channel (10), from 0.
const DRUM_CHANNEL: u8 = 9;

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ImportMidiArgs {
    /// A standard MIDI file (.mid), an absolute path; up to 8 MiB, and up to 50000 notes
    /// in one call.
    #[schemars(length(min = 1, max = 4096))]
    pub path: String,
    /// The keys track (number from 1, or id) the notes go into: the clip that holds the first
    /// note, or a new clip as long as the notes need. Not needed with `list_only`.
    #[serde(default)]
    pub track: Option<TrackRef>,
    /// Where the file's start goes, as "bar.beat.tick" (default: the song's start). Give
    /// this or `tick`.
    #[serde(default)]
    pub at: Option<String>,
    /// Where the file's start goes, in ticks.
    #[serde(default)]
    pub tick: Option<u64>,
    /// Only this track of the file (from 1, as `list_only` numbers them). Default: all.
    #[serde(default)]
    #[schemars(range(min = 1, max = 65535))]
    pub midi_track: Option<u32>,
    /// Only this MIDI channel, 1 to 16. Default: all.
    #[serde(default)]
    #[schemars(range(min = 1, max = 16))]
    pub channel: Option<u32>,
    /// Keys up (down when below 0).
    #[serde(default)]
    #[schemars(range(min = -127, max = 127))]
    pub transpose: Option<i32>,
    /// Only say what the file holds; the song is not changed.
    #[serde(default)]
    pub list_only: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ExportMidiArgs {
    /// A keys clip's id: the file holds that clip. Give this, or `from` and `to`, or
    /// nothing for the whole song. Notes end where their clip or the range ends; drum kits
    /// go on channel 10, other tracks on channel 1; a tempo ramp becomes a step each
    /// sixteenth note.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub clip: Option<u32>,
    /// Where the range starts (default: the song's start).
    #[serde(default)]
    pub from: Option<Place>,
    /// Where the range ends (default: where the last clip ends).
    #[serde(default)]
    pub to: Option<Place>,
    /// Keys tracks (numbers from 1, or ids) of the range to write. Default: every keys track.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub tracks: Option<Vec<TrackRef>>,
    /// The file to write (.mid), an absolute path in a folder that is there. Leave out to
    /// get a file in the private temporary folder (deleted after an hour).
    #[serde(default)]
    #[schemars(length(min = 1, max = 4096))]
    pub path: Option<String>,
    /// Write over a file that is there (default false: that is an error).
    #[serde(default)]
    pub overwrite: bool,
    /// 480, 960 or 1920 (default: 1920, meloet's own ticks, so no note moves).
    #[serde(default)]
    pub ticks_per_quarter: Option<u32>,
}

fn read_file(path: &Path) -> Result<Smf, String> {
    let cannot = |e: &dyn std::fmt::Display| format!("{} cannot be read ({e})", path.display());
    let meta = std::fs::metadata(path).map_err(|e| cannot(&e))?;
    if !meta.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    let too_large = || {
        format!(
            "{} is larger than {} MiB; that is no MIDI file meloet reads",
            path.display(),
            MAX_FILE >> 20
        )
    };
    if meta.len() > MAX_FILE {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    let file = std::fs::File::open(path).map_err(|e| cannot(&e))?;
    (file.take(MAX_FILE + 1).read_to_end(&mut bytes)).map_err(|e| cannot(&e))?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(too_large());
    }
    smf::read(&bytes).map_err(|e| format!("{} cannot be read: {e}", path.display()))
}

fn bpm_of(us_per_quarter: u32) -> f64 {
    (6_000_000_000.0 / f64::from(us_per_quarter.max(1))).round() / 100.0
}

fn track_value(number: usize, track: &SmfTrack, division: u16) -> Value {
    let mut channels: Vec<u8> = track.notes.iter().map(|n| n.channel + 1).collect();
    channels.sort_unstable();
    channels.dedup();
    let length = smf::scaled(track.length, division);
    json!({
        "number": number + 1,
        "name": track.name,
        "channels": channels,
        "notes": track.notes.len(),
        "lowest_key": track.notes.iter().map(|n| n.key).min(),
        "highest_key": track.notes.iter().map(|n| n.key).max(),
        "length_ticks": length,
        "length_quarters": (length as f64 / TICKS_PER_QUARTER as f64 * 100.0).round() / 100.0,
        "hanging_notes": track.hanging,
    })
}

/// What a MIDI file holds; ticks are meloet's (1920 a quarter note), from the file's start.
fn file_value(path: &Path, file: &Smf) -> Value {
    let division = file.division;
    let mut tempos: Vec<(u64, u32)> = (file.tracks.iter())
        .flat_map(|t| t.tempos.iter().copied())
        .collect();
    tempos.sort_by_key(|t| t.0);
    let mut meters: Vec<(u64, u8, u8)> = (file.tracks.iter())
        .flat_map(|t| t.meters.iter().copied())
        .collect();
    meters.sort_by_key(|m| m.0);
    let tracks: Vec<Value> = (file.tracks.iter().enumerate())
        .map(|(i, t)| track_value(i, t, division))
        .collect();
    let tempo: Vec<Value> = (tempos.iter().take(MAX_LISTED))
        .map(|&(tick, us)| json!({ "tick": smf::scaled(tick, division), "bpm": bpm_of(us) }))
        .collect();
    let meter: Vec<Value> = (meters.iter().take(MAX_LISTED))
        .map(|&(tick, num, power)| {
            let den = 1u64 << power.min(32);
            json!({ "tick": smf::scaled(tick, division), "meter": format!("{num}/{den}") })
        })
        .collect();
    json!({
        "path": path.display().to_string(),
        "format": file.format,
        "ticks_per_quarter": division,
        "scaled_exactly": TICKS_PER_QUARTER.is_multiple_of(u64::from(division)),
        "notes": file.tracks.iter().map(|t| t.notes.len()).sum::<usize>(),
        "tracks": tracks,
        "tempo": tempo,
        "tempo_count": tempos.len(),
        "meters": meter,
        "meter_count": meters.len(),
    })
}

/// The notes of the file chosen by track and channel, in order of start.
fn chosen(
    file: &Smf,
    midi_track: Option<u32>,
    channel: Option<u32>,
) -> Result<(Vec<SmfNote>, u32), String> {
    let tracks: Vec<&SmfTrack> = match midi_track {
        None => file.tracks.iter().collect(),
        Some(n) => match n.checked_sub(1).and_then(|i| file.tracks.get(i as usize)) {
            Some(track) => vec![track],
            None => {
                return Err(format!(
                    "the file has {} tracks; there is no track {n}",
                    file.tracks.len()
                ));
            }
        },
    };
    let hanging = tracks.iter().map(|t| t.hanging).sum();
    let mut notes: Vec<SmfNote> = (tracks.iter())
        .flat_map(|t| t.notes.iter().copied())
        .filter(|n| channel.is_none_or(|c| u32::from(n.channel) + 1 == c))
        .collect();
    notes.sort_by_key(|n| (n.start, n.key));
    Ok((notes, hanging))
}

/// A note of a clip as it sounds: where it starts and ends in the song.
struct Heard {
    start: u64,
    end: u64,
    key: u8,
    velocity: u8,
}

/// The notes of a keys clip that start in `from..to`, each ending at the clip's end or at
/// `to` at the latest, as the clip plays them; and how many were cut at `to`.
fn heard(clip: &Clip, from: u64, to: u64) -> (Vec<Heard>, u32) {
    let shown = clip.active_take();
    let zero = clip.zero(shown);
    let mut cut = 0;
    let mut notes = Vec::new();
    for n in &shown.take.notes {
        let on = zero + n.start as i64;
        if on < clip.start as i64 || on >= clip.end as i64 {
            continue;
        }
        let on = on as u64;
        if on < from || on >= to {
            continue;
        }
        let off = (on + n.length).min(clip.end);
        if off > to {
            cut += 1;
        }
        notes.push(Heard {
            start: on,
            end: off.min(to),
            key: n.key,
            velocity: n.velocity,
        });
    }
    (notes, cut)
}

/// The song's tempo in `from..to` as steps: a tick and the tempo from there. A ramp
/// becomes a step each sixteenth note, with the tempo of the step's middle.
fn tempo_steps(timeline: &Timeline, map: &TempoMap, from: u64, to: u64) -> Vec<(u64, f64)> {
    let mut points = vec![(0, timeline.head_ramp)];
    points.extend(timeline.tempo_points.iter().map(|p| (p.tick, p.ramp)));
    let mut steps = Vec::new();
    for (i, &(start, ramp)) in points.iter().enumerate() {
        let next = points.get(i + 1).map(|p| p.0);
        let (a, b) = (start.max(from), next.unwrap_or(u64::MAX).min(to));
        if a >= b {
            continue;
        }
        if !ramp || next.is_none() {
            steps.push((a, map.tempo_at(a as f64)));
            continue;
        }
        let mut t = a;
        while t < b {
            let end = (start + ((t - start) / RAMP_STEP + 1) * RAMP_STEP).min(b);
            steps.push((t, map.tempo_at((t + end) as f64 / 2.0)));
            t = end;
        }
    }
    steps
}

fn write_file(path: &Path, bytes: &[u8], overwrite: bool) -> std::io::Result<()> {
    if !overwrite {
        let mut file = (std::fs::OpenOptions::new().write(true).create_new(true)).open(path)?;
        return file.write_all(bytes);
    }
    // Written beside the file and moved over it, so a failed write leaves the old file.
    let mut beside = path.as_os_str().to_owned();
    beside.push(format!(".{}.tmp", std::process::id()));
    let beside = PathBuf::from(beside);
    let written = std::fs::write(&beside, bytes).and_then(|()| std::fs::rename(&beside, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&beside);
    }
    written
}

/// What export_midi writes, made from the song as it was read.
struct Export {
    file: OutFile,
    /// Where the file's tick 0 is in the song.
    from: u64,
    to: u64,
    tracks: Vec<Value>,
    notes: usize,
    cut: u32,
    ramps: bool,
}

fn is_drums(track: &Track) -> bool {
    matches!(&track.instrument, InstrumentRef::Builtin(b) if b.group() == InstrumentGroup::Drums)
}

fn export_of(read: &AiSong, a: &ExportMidiArgs, division: u16) -> Result<Export, CallToolResult> {
    let doc = &read.song.doc;
    let map = TempoMap::new(&doc.timeline);
    let place = |p: &Place| p.tick(&map).map_err(failed);
    // Each track to write with its number, and the one clip of it when a clip was asked for.
    let mut chosen: Vec<(usize, &Track, Option<&Clip>)> = Vec::new();
    let (from, to) = match a.clip {
        Some(id) => {
            if a.from.is_some() || a.to.is_some() || a.tracks.is_some() {
                return Err(failed(
                    "give `clip`, or a range (`from`, `to`, `tracks`), not both",
                ));
            }
            let found = (doc.tracks.iter().enumerate())
                .find_map(|(i, t)| t.clips.iter().find(|c| c.id.0 == id).map(|c| (i, t, c)));
            let Some((i, track, clip)) = found else {
                return Err(failed(refusal_text(&AiRefusal::NoSuchClip(id))));
            };
            if !clip.is_keys() {
                return Err(failed(refusal_text(&AiRefusal::NotKeys)));
            }
            chosen.push((i, track, Some(clip)));
            (clip.start, clip.end)
        }
        None => {
            let song_end = (doc.tracks.iter().flat_map(|t| t.clips.iter()))
                .map(|c| c.end)
                .max()
                .unwrap_or(0);
            let from = match &a.from {
                Some(from) => place(from)?,
                None => 0,
            };
            let to = match &a.to {
                Some(to) => place(to)?,
                None => song_end,
            };
            if from >= to {
                return Err(failed(
                    "the range is empty: `from` must be before `to` (default: the song's end)",
                ));
            }
            match &a.tracks {
                Some(asked) => {
                    for &t in asked {
                        let (n, track) = t.find(read).map_err(failed)?;
                        if track.kind.is_vocal() {
                            return Err(failed(format!(
                                "track {n} is a vocal track: it holds audio, not notes"
                            )));
                        }
                        if !chosen.iter().any(|c| c.0 == n as usize - 1) {
                            chosen.push((n as usize - 1, track, None));
                        }
                    }
                    chosen.sort_by_key(|c| c.0);
                }
                None => chosen.extend(
                    (doc.tracks.iter().enumerate())
                        .filter(|(_, t)| !t.kind.is_vocal())
                        .map(|(i, t)| (i, &**t, None)),
                ),
            }
            (from, to)
        }
    };
    let ticks = |tick: u64| smf::unscaled(tick - from, division);
    if ticks(to) > smf::MAX_OUT_TICKS {
        return Err(failed(
            "that range is too long for a MIDI file; write it in parts",
        ));
    }
    let mut tracks = Vec::new();
    let mut listed = Vec::new();
    let (mut count, mut cut) = (0, 0);
    for (i, track, only) in chosen {
        let mut notes = Vec::new();
        for clip in track.clips.iter().filter(|c| c.is_keys()) {
            if only.is_some_and(|o| o.id != clip.id) {
                continue;
            }
            let (sounding, cut_here) = heard(clip, from, to);
            cut += cut_here;
            notes.extend(sounding.iter().map(|n| {
                let start = ticks(n.start);
                OutNote {
                    start,
                    length: ticks(n.end).saturating_sub(start).max(1),
                    key: n.key,
                    velocity: n.velocity,
                }
            }));
        }
        let channel = if is_drums(track) { DRUM_CHANNEL } else { 0 };
        count += notes.len();
        listed.push(json!({
            "number": i + 1,
            "id": id_text(track.id),
            "name": track.name,
            "notes": notes.len(),
            "channel": channel + 1,
        }));
        tracks.push(OutTrack {
            name: track.name.clone(),
            channel,
            notes,
        });
    }
    let timeline = &doc.timeline;
    let mut tempos: Vec<(u64, u32)> = Vec::new();
    for (tick, bpm) in tempo_steps(timeline, &map, from, to) {
        let us = (60_000_000.0 / bpm.max(1.0)).round() as u32;
        if tempos.last().is_none_or(|last| last.1 != us) {
            tempos.push((ticks(tick), us));
        }
    }
    let head = map.meter_at(from).meter;
    let mut meters = vec![(0, head.num(), head.den())];
    for change in &timeline.meter_changes {
        let tick = map.tick_of_bar(u64::from(change.bar));
        if tick > from && tick < to {
            meters.push((ticks(tick), change.meter.num(), change.meter.den()));
        }
    }
    let ramps = timeline.head_ramp || timeline.tempo_points.iter().any(|p| p.ramp);
    Ok(Export {
        file: OutFile {
            division,
            name: doc.name.clone(),
            tempos,
            meters,
            length: ticks(to),
            tracks,
        },
        from,
        to,
        tracks: listed,
        notes: count,
        cut,
        ramps,
    })
}

fn is_midi_name(path: &Path) -> bool {
    (path.extension().and_then(|e| e.to_str()))
        .is_some_and(|e| e.eq_ignore_ascii_case("mid") || e.eq_ignore_ascii_case("midi"))
}

#[tool_router(router = midi_router, vis = "pub(super)")]
impl Tools {
    /// Brings the notes of a standard MIDI file (format 0 or 1) into a keys track at a
    /// place, as one undo step. `list_only` only says what the file holds. The file's tempo
    /// and meter are not applied (the answer lists them). Refused, naming the clip, when
    /// another clip is in the way. Answers the clip written and `not_written`.
    #[tool(annotations(
        title = "Import MIDI file",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn import_midi(
        &self,
        Parameters(a): Parameters<ImportMidiArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let path = check!(path_ok(&a.path));
        let transpose = a.transpose.unwrap_or(0);
        if !(-127..=127).contains(&transpose) {
            return Ok(failed("transpose by -127 to 127 keys"));
        }
        if a.channel.is_some_and(|c| !(1..=16).contains(&c)) {
            return Ok(failed("the channel is 1 to 16"));
        }
        let reading = path.clone();
        let file = match tokio::task::spawn_blocking(move || read_file(&reading)).await {
            Ok(file) => check!(file.map_err(failed)),
            Err(_) => return Ok(closing()),
        };
        let mut listing = file_value(&path, &file);
        if a.list_only {
            let line = format!(
                "{}: format {}, {} ticks a quarter note, {} tracks, {} notes. Nothing was \
                 changed.",
                path.display(),
                file.format,
                file.division,
                file.tracks.len(),
                listing["notes"],
            );
            return Ok(done(format!("{line}\n{listing}"), listing));
        }
        let Some(track) = a.track else {
            return Ok(failed(
                "give the `track` the notes go into (or `list_only`)",
            ));
        };
        let (notes, hanging) = check!(chosen(&file, a.midi_track, a.channel).map_err(failed));
        if notes.is_empty() {
            return Ok(failed(format!(
                "the file has no notes there; it holds: {listing}"
            )));
        }
        if notes.len() > MAX_IMPORT_NOTES {
            return Ok(failed(format!(
                "that is {} notes; at most {MAX_IMPORT_NOTES} come in at once. Choose a \
                 `midi_track` or a `channel`",
                notes.len()
            )));
        }
        let read = check!(self.read().await);
        let track = check!(track.number(&read).map_err(failed));
        let map = TempoMap::new(&read.song.doc.timeline);
        let origin = match (&a.at, a.tick) {
            (None, None) => 0,
            (at, tick) => check!(at_or_tick(&map, at.as_deref(), tick)),
        };
        let in_song = |tick: u64| origin.saturating_add(smf::scaled(tick, file.division));
        let notes: Vec<AiNote> = (notes.iter())
            .map(|n| {
                let start = in_song(n.start);
                AiNote {
                    start: i64::try_from(start).unwrap_or(i64::MAX),
                    length: in_song(n.end).saturating_sub(start).max(1),
                    key: i32::from(n.key) + transpose,
                    velocity: n.velocity.clamp(1, 127),
                }
            })
            .collect();
        let asked = notes.len();
        let ask = AiAsk::PutNotes {
            at: AiPlace::Track(track),
            notes,
        };
        let (d, mut value) = check!(self.put(ask, &map, &read).await);
        // The tracks were told by `list_only`; the answer of an import stays small.
        listing["tracks"] = Value::Null;
        value["file"] = listing;
        value["asked"] = json!(asked);
        value["origin"] = song::place(&map, origin);
        value["hanging_notes"] = json!(hanging);
        value["tempo_and_meter_applied"] = json!(false);
        let mut line = format!("Imported: of {asked} notes, {}", notes_line(&d, &value));
        if hanging > 0 {
            line.push_str(&format!(
                " {hanging} notes never ended in the file: they end where their track ends."
            ));
        }
        line.push_str(" The file's tempo and meter were not applied to the song.");
        Ok(done(format!("{line}\n{value}"), value))
    }

    /// Writes a keys clip, or a range of the song, as a standard MIDI file (format 1; tempos
    /// and meters in the first track, then a track for each meloet track) to `path` or the
    /// private temporary folder. Notes are written as they sound. The song is not changed.
    /// import_midi at `origin` puts the same notes back.
    #[tool(annotations(
        title = "Export MIDI file",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn export_midi(
        &self,
        Parameters(a): Parameters<ExportMidiArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let division = match a.ticks_per_quarter.unwrap_or(1920) {
            d @ (480 | 960 | 1920) => d as u16,
            _ => return Ok(failed("ticks_per_quarter is 480, 960 or 1920")),
        };
        let path = match &a.path {
            None => None,
            Some(text) => {
                let path = check!(path_ok(text));
                if !is_midi_name(&path) {
                    return Ok(failed("the file's name must end in .mid"));
                }
                if !path.parent().is_some_and(Path::is_dir) {
                    return Ok(failed(format!(
                        "the folder of {} is not there",
                        path.display()
                    )));
                }
                Some(path)
            }
        };
        let read = check!(self.read().await);
        let export = check!(export_of(&read, &a, division));
        let map = TempoMap::new(&read.song.doc.timeline);
        let overwrite = a.overwrite && path.is_some();
        let scratch = self.scratch.clone();
        let what = match a.clip {
            Some(clip) => format!("midi-clip{clip}"),
            None => "midi-song".to_owned(),
        };
        let bytes = smf::write(&export.file);
        let size = bytes.len();
        let written = tokio::task::spawn_blocking(move || {
            let path = match path {
                Some(path) => path,
                None => {
                    scratch.clean(crate::scratch::KEEP);
                    let dir = scratch.dir().map_err(|e| {
                        format!("the private temporary folder is not usable ({e}); give a `path`")
                    })?;
                    scratch.name(&dir, &what, "mid")
                }
            };
            match write_file(&path, &bytes, overwrite) {
                Ok(()) => Ok(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    Err(refusal_text(&AiRefusal::Exists(path)))
                }
                Err(e) => Err(format!("{} cannot be written ({e})", path.display())),
            }
        })
        .await;
        let path = match written {
            Ok(path) => check!(path.map_err(|why| failed(format!("Could not export: {why}.")))),
            Err(_) => return Ok(closing()),
        };
        let value = json!({
            "file": path.display().to_string(),
            "bytes": size,
            "format": 1,
            "ticks_per_quarter": division,
            "origin": song::place(&map, export.from),
            "end": song::place(&map, export.to),
            "notes": export.notes,
            "notes_cut_at_range_end": export.cut,
            "tracks": export.tracks,
            "tempo_events": export.file.tempos.len(),
            "meter_events": export.file.meters.len(),
            "tempo_ramps_as_steps": export.ramps,
        });
        let line = format!(
            "Wrote {} notes of {} track(s), {} to {}, to {}",
            export.notes,
            export.file.tracks.len(),
            song::at(&map, export.from),
            song::at(&map, export.to),
            path.display()
        );
        Ok(done(format!("{line}\n{value}"), value))
    }
}
