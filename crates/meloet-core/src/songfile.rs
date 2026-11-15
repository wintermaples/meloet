//! The song file: its JSON shape, and how it is written so that a crash at any moment leaves
//! `<name>.meloet` whole, either as it was or as it is now.
//!
//! The shape lives here and not in `meloet-model`, which stays free of serde and files.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use meloet_model::{
    Clip, ClipId, ClipTake, ControlPoint, ControlValue, Doc, Grid, InstrumentRef, Meter,
    MusicalScale, Note, QuantizeAlign, QuantizeSettings, RollSettings, ScaleMode, Session, Song,
    Take, TakeId, Tempo, TempoChange, TickRange, Track, TrackColor, TrackId,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

pub const FORMAT: &str = "meloet-song";
/// The newest shape this meloet reads and the one it writes.
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "meloet";
/// Tracks the engine has room for.
const MAX_TRACKS: usize = 64;

#[derive(Debug)]
pub enum ReadError {
    Io(io::Error),
    /// Not JSON, not a song, or a song that breaks a rule of the model.
    Broken,
    /// Written by a newer meloet.
    TooNew {
        version: u32,
    },
}

// ---- the shape ----

#[derive(Deserialize)]
struct Head {
    format: String,
    version: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SongFile {
    format: String,
    version: u32,
    /// The meloet that wrote it. Only for people.
    #[serde(default)]
    app: String,
    song: DocFile,
    takes: Vec<TakeFile>,
    session: SessionFile,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocFile {
    name: String,
    /// Hundredths: 9600 is 96.00.
    tempo: u32,
    /// `[num, den]`.
    meter: [u8; 2],
    #[serde(default)]
    tempo_changes: Vec<TempoChangeFile>,
    next_id: u32,
    tracks: Vec<TrackFile>,
    /// Voice recordings, later.
    #[serde(default)]
    audio: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TempoChangeFile {
    tick: u64,
    tempo: u32,
    meter: [u8; 2],
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackFile {
    id: u32,
    name: String,
    color: u8,
    instrument: InstrumentFile,
    gain_db: f64,
    clips: Vec<ClipFile>,
    #[serde(default)]
    plugins: Vec<serde_json::Value>,
    #[serde(default)]
    automation: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum InstrumentFile {
    Placeholder,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipFile {
    id: u32,
    name: String,
    start: u64,
    end: u64,
    /// Index into `takes` of the take shown and played.
    active: usize,
    takes: Vec<ClipTakeFile>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipTakeFile {
    take: u32,
    offset: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakeFile {
    id: u32,
    length: u64,
    /// `[start, length, key, velocity]`.
    notes: Vec<(u64, u64, u8, u8)>,
    /// In tick order, both kinds in one list so that the order at the same tick stays.
    #[serde(default)]
    controls: Vec<ControlFile>,
}

/// `[tick, number, value]` for a CC, `[tick, "bend", value]` for the pitch bend.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ControlFile {
    Cc(u64, u8, u8),
    Bend(u64, BendTag, i16),
}

#[derive(Serialize, Deserialize)]
enum BendTag {
    #[serde(rename = "bend")]
    Bend,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionFile {
    play_target: u32,
    armed: Option<u32>,
    #[serde(rename = "loop")]
    loop_range: Option<SpanFile>,
    loop_on: bool,
    /// The piano roll's settings. Missing or not understood: the default.
    #[serde(default, deserialize_with = "or_default")]
    grid: Option<GridFile>,
    #[serde(default, deserialize_with = "or_default")]
    scale: Option<ScaleFile>,
    #[serde(default, deserialize_with = "or_default")]
    quantize: Option<QuantizeFile>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GridFile {
    /// Steps to a whole note; 0 is one bar.
    division: u16,
    triplet: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScaleFile {
    /// 0..=11 is C..B.
    root: u8,
    mode: ScaleModeFile,
    lock: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ScaleModeFile {
    Major,
    Minor,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuantizeFile {
    division: u16,
    triplet: bool,
    /// Percent.
    strength: u8,
    /// Percent.
    swing: u8,
    align: AlignFile,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum AlignFile {
    Start,
    StartAndEnd,
}

/// A value of a shape this meloet does not know is read as `None`, not as a broken song.
fn or_default<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Option<T>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).ok())
}

#[derive(Serialize, Deserialize)]
struct SpanFile {
    start: u64,
    end: u64,
}

// ---- song ↔ bytes ----

pub fn encode(song: &Song) -> Vec<u8> {
    let doc = &song.doc;
    let mut takes: Vec<TakeFile> = Vec::new();
    let mut seen = HashSet::new();
    let tracks = doc
        .tracks
        .iter()
        .map(|t| TrackFile {
            id: t.id.0,
            name: t.name.clone(),
            color: t.color.get(),
            instrument: match t.instrument {
                InstrumentRef::Placeholder => InstrumentFile::Placeholder,
            },
            gain_db: t.gain_db,
            clips: t
                .clips
                .iter()
                .map(|c| ClipFile {
                    id: c.id.0,
                    name: c.name.clone(),
                    start: c.start,
                    end: c.end,
                    active: c.active,
                    takes: c
                        .takes
                        .iter()
                        .map(|ct| {
                            if seen.insert(ct.take.id) {
                                takes.push(take_file(&ct.take));
                            }
                            ClipTakeFile {
                                take: ct.take.id.0,
                                offset: ct.offset,
                            }
                        })
                        .collect(),
                })
                .collect(),
            plugins: Vec::new(),
            automation: Vec::new(),
        })
        .collect();
    let s = &song.session;
    let file = SongFile {
        format: FORMAT.into(),
        version: VERSION,
        app: concat!("meloet ", env!("CARGO_PKG_VERSION")).into(),
        song: DocFile {
            name: doc.name.clone(),
            tempo: doc.tempo.centi(),
            meter: [doc.meter.num(), doc.meter.den()],
            tempo_changes: doc
                .tempo_changes
                .iter()
                .map(|c| TempoChangeFile {
                    tick: c.tick,
                    tempo: (c.tempo_bpm * 100.0).round() as u32,
                    meter: [c.meter.num(), c.meter.den()],
                })
                .collect(),
            next_id: doc.next_id,
            tracks,
            audio: Vec::new(),
        },
        takes,
        session: SessionFile {
            play_target: s.play_target.0,
            armed: s.armed.map(|t| t.0),
            loop_range: s.loop_range.map(|r| SpanFile {
                start: r.start(),
                end: r.end(),
            }),
            loop_on: s.loop_on,
            grid: Some(grid_file(s.roll.grid)),
            scale: Some(ScaleFile {
                root: s.roll.scale.root(),
                mode: match s.roll.scale.mode {
                    ScaleMode::Major => ScaleModeFile::Major,
                    ScaleMode::Minor => ScaleModeFile::Minor,
                },
                lock: s.roll.scale.lock,
            }),
            quantize: Some({
                let q = s.roll.quantize;
                let grid = grid_file(q.grid);
                QuantizeFile {
                    division: grid.division,
                    triplet: grid.triplet,
                    strength: q.strength(),
                    swing: q.swing(),
                    align: match q.align {
                        QuantizeAlign::Start => AlignFile::Start,
                        QuantizeAlign::StartAndEnd => AlignFile::StartAndEnd,
                    },
                }
            }),
        },
    };
    let mut bytes = serde_json::to_vec(&file).expect("a song always serializes");
    bytes.push(b'\n');
    bytes
}

fn grid_file(grid: Grid) -> GridFile {
    GridFile {
        division: grid.division(),
        triplet: grid.triplet,
    }
}

/// Values out of range fall back to the default, one setting at a time.
fn roll_settings(s: &SessionFile) -> RollSettings {
    let grid = |division, triplet| Grid::new(division, triplet);
    RollSettings {
        grid: s
            .grid
            .as_ref()
            .and_then(|g| grid(g.division, g.triplet))
            .unwrap_or_default(),
        scale: s
            .scale
            .as_ref()
            .and_then(|sc| {
                let mode = match sc.mode {
                    ScaleModeFile::Major => ScaleMode::Major,
                    ScaleModeFile::Minor => ScaleMode::Minor,
                };
                MusicalScale::new(sc.root, mode, sc.lock)
            })
            .unwrap_or_default(),
        quantize: s
            .quantize
            .as_ref()
            .and_then(|q| {
                let align = match q.align {
                    AlignFile::Start => QuantizeAlign::Start,
                    AlignFile::StartAndEnd => QuantizeAlign::StartAndEnd,
                };
                QuantizeSettings::new(grid(q.division, q.triplet)?, q.strength, q.swing, align)
            })
            .unwrap_or_default(),
    }
}

fn take_file(take: &Take) -> TakeFile {
    TakeFile {
        id: take.id.0,
        length: take.length,
        notes: take
            .notes
            .iter()
            .map(|n| (n.start, n.length, n.key, n.velocity))
            .collect(),
        controls: take
            .controls
            .iter()
            .map(|c| match c.value {
                ControlValue::Cc { number, value } => ControlFile::Cc(c.tick, number, value),
                ControlValue::PitchBend(v) => ControlFile::Bend(c.tick, BendTag::Bend, v),
            })
            .collect(),
    }
}

pub fn decode(bytes: &[u8]) -> Result<Song, ReadError> {
    let head: Head = serde_json::from_slice(bytes).map_err(|_| ReadError::Broken)?;
    if head.format != FORMAT {
        return Err(ReadError::Broken);
    }
    if head.version > VERSION {
        return Err(ReadError::TooNew {
            version: head.version,
        });
    }
    let file: SongFile = serde_json::from_slice(bytes).map_err(|_| ReadError::Broken)?;
    to_song(file).ok_or(ReadError::Broken)
}

/// `None` when anything breaks a rule the model keeps.
fn to_song(file: SongFile) -> Option<Song> {
    let mut takes = HashMap::new();
    for t in file.takes {
        if takes.contains_key(&t.id) {
            return None;
        }
        takes.insert(t.id, Arc::new(to_take(t)?));
    }
    let d = file.song;
    let tracks_n = d.tracks.len();
    if tracks_n == 0 || tracks_n > MAX_TRACKS {
        return None;
    }
    let mut track_ids = HashSet::new();
    let mut clip_ids = HashSet::new();
    let mut tracks = Vec::with_capacity(tracks_n);
    for t in d.tracks {
        if !track_ids.insert(t.id) || !t.gain_db.is_finite() {
            return None;
        }
        let mut clips: Vec<Arc<Clip>> = Vec::with_capacity(t.clips.len());
        for c in t.clips {
            if !clip_ids.insert(c.id) {
                return None;
            }
            let range = TickRange::new(c.start, c.end)?;
            if clips
                .last()
                .is_some_and(|before| before.end > range.start())
                || c.active >= c.takes.len()
            {
                return None;
            }
            let clip_takes = c
                .takes
                .iter()
                .map(|ct| {
                    Some(ClipTake {
                        take: takes.get(&ct.take)?.clone(),
                        offset: ct.offset,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            clips.push(Arc::new(Clip {
                id: ClipId(c.id),
                name: c.name,
                start: c.start,
                end: c.end,
                takes: clip_takes,
                active: c.active,
            }));
        }
        tracks.push(Arc::new(Track {
            id: TrackId(t.id),
            name: t.name,
            color: TrackColor::new(t.color)?,
            instrument: match t.instrument {
                InstrumentFile::Placeholder => InstrumentRef::Placeholder,
            },
            gain_db: t.gain_db,
            clips,
        }));
    }
    if clip_ids
        .iter()
        .chain(takes.keys())
        .any(|&id| id >= d.next_id)
    {
        return None;
    }
    let tempo_changes = d
        .tempo_changes
        .into_iter()
        .map(|c| {
            Some(TempoChange {
                tick: c.tick,
                tempo_bpm: Tempo::from_centi(c.tempo)?.bpm(),
                meter: Meter::new(c.meter[0], c.meter[1])?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let doc = Doc {
        name: d.name,
        tempo: Tempo::from_centi(d.tempo)?,
        meter: Meter::new(d.meter[0], d.meter[1])?,
        tempo_changes,
        tracks,
        next_id: d.next_id,
    };
    let s = file.session;
    let known = |id: u32| track_ids.contains(&id).then_some(TrackId(id));
    let roll = roll_settings(&s);
    let session = Session {
        // A target that is gone falls back to the first track rather than failing the song.
        play_target: known(s.play_target).unwrap_or(doc.tracks[0].id),
        armed: s.armed.and_then(known),
        loop_range: match s.loop_range {
            Some(r) => Some(TickRange::new(r.start, r.end)?),
            None => None,
        },
        loop_on: s.loop_on,
        roll,
    };
    Some(Song { doc, session })
}

fn to_take(t: TakeFile) -> Option<Take> {
    let notes = t
        .notes
        .into_iter()
        .map(|(start, length, key, velocity)| {
            (length >= 1 && key <= 127 && (1..=127).contains(&velocity)).then_some(Note {
                start,
                length,
                key,
                velocity,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let controls = t
        .controls
        .into_iter()
        .map(|c| match c {
            ControlFile::Cc(tick, number, value) => {
                (number <= 127 && value <= 127).then_some(ControlPoint {
                    tick,
                    value: ControlValue::Cc { number, value },
                })
            }
            ControlFile::Bend(tick, _, v) => (-8192..=8191).contains(&v).then_some(ControlPoint {
                tick,
                value: ControlValue::PitchBend(v),
            }),
        })
        .collect::<Option<Vec<_>>>()?;
    if !notes.is_sorted_by_key(|n| n.start) || !controls.is_sorted_by_key(|c| c.tick) {
        return None;
    }
    Some(Take {
        id: TakeId(t.id),
        length: t.length,
        notes,
        controls,
    })
}

// ---- files ----

/// Where songs go when nothing else is said: `$MELOET_SONGS_DIR`, the PC's music folder, or
/// `~/Music`.
pub fn songs_dir() -> Option<PathBuf> {
    std::env::var_os("MELOET_SONGS_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::audio_dir)
        .or_else(|| dirs::home_dir().map(|h| h.join("Music")))
}

/// `<name>.meloet.prev`: the file as it was before the last save.
pub fn previous_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(".prev");
    path.with_file_name(name)
}

pub fn read(path: &Path) -> Result<Song, ReadError> {
    decode(&fs::read(path).map_err(ReadError::Io)?)
}

/// Writes the song over `path`. Before that, the file there (when it reads as a song) becomes
/// `<name>.meloet.prev`. Every file is synced before it is renamed into place, and the folder
/// after, so `path` is always one whole song: the one before or this one.
pub fn save(path: &Path, song: &Song) -> io::Result<()> {
    let bytes = encode(song);
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
    let dir = dir.unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?
        .to_string_lossy()
        .into_owned();
    let fresh = dir.join(format!(".{name}.new"));
    let result = (|| {
        write_synced(&fresh, &bytes)?;
        // A file that does not read as a song is not kept: it would push out a good copy.
        if let Ok(old) = fs::read(path)
            && decode(&old).is_ok()
        {
            let kept = dir.join(format!(".{name}.prev.new"));
            write_synced(&kept, &old)?;
            fs::rename(&kept, previous_path(path))?;
        }
        fs::rename(&fresh, path)?;
        sync_dir(dir)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&fresh);
        let _ = fs::remove_file(dir.join(format!(".{name}.prev.new")));
    }
    result
}

/// The first save: makes `<base>/<name>/<name>.meloet`. When `<base>/<name>` is there already
/// (another song, or anything), the name gets " 2", " 3", … and so does the song. Returns the
/// path and the name the song has now.
pub fn save_new(base: &Path, song: &Song) -> io::Result<(PathBuf, String)> {
    fs::create_dir_all(base)?;
    let stem = folder_name(&song.doc.name);
    for n in 1u32.. {
        let name = if n == 1 {
            stem.clone()
        } else {
            format!("{stem} {n}")
        };
        let dir = base.join(&name);
        match fs::create_dir(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
        let path = dir.join(format!("{name}.{EXTENSION}"));
        let mut named = song.clone();
        named.doc.name = name.clone();
        return match save(&path, &named) {
            Ok(()) => Ok((path, name)),
            Err(e) => {
                let _ = fs::remove_dir(&dir);
                Err(e)
            }
        };
    }
    unreachable!("u32 names run out")
}

/// The song's name with what a folder name cannot hold made into "_".
fn folder_name(song: &str) -> String {
    let name: String = song
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim_matches('.').to_owned();
    if name.is_empty() {
        "meloet".into()
    } else {
        name
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The renames in `dir` reach the disk. Windows has no way to open a folder for this.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests;
