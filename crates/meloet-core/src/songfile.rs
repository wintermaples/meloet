//! The song file: its JSON shape, and how it is written so that a crash at any moment leaves
//! `<name>.meloet` whole, either as it was or as it is now.
//!
//! The shape lives here and not in `meloet-model`, which stays free of serde and files.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use meloet_model::effects::{EffectKind, ParamShape};
use meloet_model::{
    BuiltinInstrument, Bus, Channel, Clip, ClipId, ClipTake, ControlPoint, ControlValue, Doc, Gain,
    Grid, Insert, Inserts, InstrumentRef, MAX_TICK, Meter, MusicalScale, Note, OriginIndex, Pan,
    QuantizeAlign, QuantizeSettings, Return, RollSettings, ScaleMode, Session, Song, Take, TakeId,
    TakeOrigin, Tempo, TempoChange, TickRange, Track, TrackColor, TrackId, mixer,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

pub const FORMAT: &str = "meloet-song";
/// The newest shape this meloet reads and the one it writes. 2: quantized takes, with their
/// origin take and the origin index of each note. 3: instruments by name, a song may have no
/// tracks, the session has no armed track, and the mixer (pan, sends, inserts, the returns
/// and the master).
pub const VERSION: u32 = 3;
pub const EXTENSION: &str = "meloet";
/// Tracks the engine has room for.
const MAX_TRACKS: usize = crate::play::MAX_TRACKS;

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
    /// The returns and the master. Missing (versions 1 and 2, and version 3 before the mixer)
    /// or not understood: the ones of a new song.
    #[serde(
        default,
        deserialize_with = "or_default",
        skip_serializing_if = "Option::is_none"
    )]
    mixer: Option<MixerFile>,
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
    /// The fader; `null` is −∞.
    #[serde(deserialize_with = "db")]
    gain_db: Option<f64>,
    /// -100..=100. Missing or not understood: the middle.
    #[serde(default, deserialize_with = "or_default")]
    pan: Option<i64>,
    /// Missing or not understood: nothing is sent, so a song from before the mixer sounds
    /// as it did.
    #[serde(default, deserialize_with = "or_default")]
    sends: Option<SendsFile>,
    /// One entry for each insert slot, top first.
    #[serde(default, deserialize_with = "or_default")]
    inserts: Option<Vec<SlotFile>>,
    /// Left out when off: a song with no muted track is written as before, and a meloet of
    /// the same version that does not know the field skips it (an unknown field).
    #[serde(default, skip_serializing_if = "is_false")]
    muted: bool,
    clips: Vec<ClipFile>,
    #[serde(default)]
    plugins: Vec<serde_json::Value>,
    #[serde(default)]
    automation: Vec<serde_json::Value>,
}

/// `{"kind": "builtin", "name": "grandPiano"}`. Versions 1 and 2 wrote
/// `{"kind": "placeholder"}`, the sound now named "sine".
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstrumentFile {
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// `null` is −∞.
#[derive(Serialize, Deserialize)]
struct SendsFile {
    #[serde(default)]
    a: Option<f64>,
    #[serde(default)]
    b: Option<f64>,
}

/// An insert slot: `null` when empty. An entry that is not an effect is read as empty.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SlotFile {
    Insert(InsertFile),
    Other(serde_json::Value),
}

/// `{"effect": "reverb", "params": {"decay": 2.4, …}}`. The parameters go by the names of
/// their tables (`meloet_model::effects`), so that a table can grow or change its order.
#[derive(Serialize, Deserialize)]
struct InsertFile {
    effect: String,
    #[serde(default)]
    params: BTreeMap<String, ParamFile>,
}

/// A number in the parameter's unit, the name of a step, or a switch.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ParamFile {
    Switch(bool),
    Number(f32),
    Step(String),
    Other(serde_json::Value),
}

#[derive(Serialize, Deserialize)]
struct MixerFile {
    returns: ReturnsFile,
    master: MasterFile,
}

#[derive(Serialize, Deserialize)]
struct ReturnsFile {
    a: ReturnFile,
    b: ReturnFile,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReturnFile {
    #[serde(default = "unity")]
    gain_db: Option<f64>,
    #[serde(default)]
    pan: i64,
    #[serde(default, skip_serializing_if = "is_false")]
    muted: bool,
    #[serde(default)]
    inserts: Vec<SlotFile>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MasterFile {
    #[serde(default = "unity")]
    gain_db: Option<f64>,
    #[serde(default)]
    inserts: Vec<SlotFile>,
}

fn unity() -> Option<f64> {
    Some(0.0)
}

/// A number or `null`. A field read with this must be there.
fn db<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Option::<f64>::deserialize(d)
}

fn gain_of(db: Option<f64>) -> Gain {
    db.map_or(Gain::SILENT, Gain::from_db)
}

fn pan_of(pan: i64) -> Pan {
    Pan::new(pan.clamp(-100, 100) as i32)
}

fn slots_file(inserts: &Inserts) -> Vec<SlotFile> {
    inserts
        .iter()
        .map(|slot| match slot {
            None => SlotFile::Other(serde_json::Value::Null),
            Some(insert) => {
                let params = insert.kind().params().iter().zip(insert.values());
                SlotFile::Insert(InsertFile {
                    effect: insert.kind().name().into(),
                    params: params
                        .map(|(spec, &v)| {
                            let value = match spec.shape {
                                ParamShape::Continuous => ParamFile::Number(v),
                                ParamShape::Steps(names) => {
                                    ParamFile::Step(names[v as usize].into())
                                }
                                ParamShape::Switch => ParamFile::Switch(v >= 0.5),
                            };
                            (spec.name.to_owned(), value)
                        })
                        .collect(),
                })
            }
        })
        .collect()
}

/// An effect this meloet does not know leaves its slot empty; a parameter it does not know
/// is skipped, one that is missing or of another shape has its default, and a value out of
/// range is brought into range. Slots past the last one are dropped.
fn inserts_of(slots: Vec<SlotFile>) -> Inserts {
    let mut inserts = Inserts::default();
    for (place, slot) in inserts.iter_mut().zip(slots) {
        let SlotFile::Insert(file) = slot else {
            continue;
        };
        let Some(kind) = EffectKind::from_name(&file.effect) else {
            continue;
        };
        let mut insert = Insert::new(kind);
        for (name, value) in file.params {
            let Some(index) = kind.param_index(&name) else {
                continue;
            };
            let shape = kind.params()[index as usize].shape;
            let value = match (shape, value) {
                (ParamShape::Continuous, ParamFile::Number(v)) => Some(v),
                (ParamShape::Steps(names), ParamFile::Step(step)) => {
                    names.iter().position(|n| *n == step).map(|i| i as f32)
                }
                (ParamShape::Switch, ParamFile::Switch(on)) => Some(f32::from(u8::from(on))),
                _ => None,
            };
            if let Some(v) = value {
                insert.set(index, v);
            }
        }
        *place = Some(insert);
    }
    inserts
}

fn mixer_file(doc: &Doc) -> MixerFile {
    let ret = |bus: Bus| {
        let r = &doc.returns[bus.index()];
        ReturnFile {
            gain_db: r.channel.gain.db(),
            pan: i64::from(r.channel.pan.get()),
            muted: r.muted,
            inserts: slots_file(&r.channel.inserts),
        }
    };
    MixerFile {
        returns: ReturnsFile {
            a: ret(Bus::A),
            b: ret(Bus::B),
        },
        master: MasterFile {
            gain_db: doc.master.gain.db(),
            inserts: slots_file(&doc.master.inserts),
        },
    }
}

fn return_of(file: ReturnFile) -> Arc<Return> {
    Arc::new(Return {
        channel: Channel {
            gain: gain_of(file.gain_db),
            pan: pan_of(file.pan),
            inserts: inserts_of(file.inserts),
        },
        muted: file.muted,
    })
}

const KIND_BUILTIN: &str = "builtin";

fn instrument_file(instrument: &InstrumentRef) -> InstrumentFile {
    match instrument {
        InstrumentRef::Builtin(b) => InstrumentFile {
            kind: KIND_BUILTIN.into(),
            name: Some(b.name().into()),
        },
    }
}

/// An instrument this meloet does not know (a sound of a later meloet, a name spelled by
/// hand) becomes the sine: the notes of a song are worth more than the name of a sound, so
/// the song is not refused for it.
fn instrument_of(file: &InstrumentFile) -> InstrumentRef {
    file.name
        .as_deref()
        .filter(|_| file.kind == KIND_BUILTIN)
        .and_then(BuiltinInstrument::from_name)
        .map_or(InstrumentRef::SINE, InstrumentRef::Builtin)
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
    /// Below 0 when the head of the take is trimmed off.
    offset: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakeFile {
    id: u32,
    length: u64,
    notes: Vec<NoteFile>,
    /// In tick order, both kinds in one list so that the order at the same tick stays.
    #[serde(default)]
    controls: Vec<ControlFile>,
    /// A quantized take: the recorded take it was made from, and how it was quantized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<OriginFile>,
}

/// `[start, length, key, velocity]`, and the index of the note in the origin take when it came
/// from there.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum NoteFile {
    Plain(u64, u64, u8, u8),
    From(u64, u64, u8, u8, u32),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OriginFile {
    take: u32,
    division: u16,
    triplet: bool,
    /// Percent.
    strength: u8,
    /// Percent.
    swing: u8,
    align: AlignFile,
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
    /// `null` in a song with no tracks. Versions 1 and 2 also wrote `armed`, which is skipped:
    /// a recording goes to the selected track.
    #[serde(default)]
    play_target: Option<u32>,
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

fn is_false(b: &bool) -> bool {
    !b
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
            instrument: instrument_file(&t.instrument),
            gain_db: t.channel.gain.db(),
            pan: Some(i64::from(t.channel.pan.get())),
            sends: Some(SendsFile {
                a: t.sends[Bus::A.index()].db(),
                b: t.sends[Bus::B.index()].db(),
            }),
            inserts: Some(slots_file(&t.channel.inserts)),
            muted: t.muted,
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
                            if let Some(o) = &ct.take.origin
                                && seen.insert(o.take.id)
                            {
                                takes.push(take_file(&o.take));
                            }
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
            mixer: Some(mixer_file(doc)),
            audio: Vec::new(),
        },
        takes,
        session: SessionFile {
            play_target: s.play_target.map(|t| t.0),
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
                QuantizeFile {
                    division: q.grid.division(),
                    triplet: q.grid.triplet,
                    strength: q.strength(),
                    swing: q.swing(),
                    align: align_file(q.align),
                }
            }),
        },
    };
    let mut bytes = serde_json::to_vec(&file).expect("a song always serializes");
    bytes.push(b'\n');
    bytes
}

fn align_file(align: QuantizeAlign) -> AlignFile {
    match align {
        QuantizeAlign::Start => AlignFile::Start,
        QuantizeAlign::StartAndEnd => AlignFile::StartAndEnd,
    }
}

fn align_of(align: &AlignFile) -> QuantizeAlign {
    match align {
        AlignFile::Start => QuantizeAlign::Start,
        AlignFile::StartAndEnd => QuantizeAlign::StartAndEnd,
    }
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
                let align = align_of(&q.align);
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
            .map(|n| match n.from {
                None => NoteFile::Plain(n.start, n.length, n.key, n.velocity),
                Some(f) => NoteFile::From(n.start, n.length, n.key, n.velocity, f.get()),
            })
            .collect(),
        controls: take
            .controls
            .iter()
            .map(|c| match c.value {
                ControlValue::Cc { number, value } => ControlFile::Cc(c.tick, number, value),
                ControlValue::PitchBend(v) => ControlFile::Bend(c.tick, BendTag::Bend, v),
            })
            .collect(),
        origin: take.origin.as_ref().map(|o| OriginFile {
            take: o.take.id.0,
            division: o.settings.grid.division(),
            triplet: o.settings.grid.triplet,
            strength: o.settings.strength(),
            swing: o.settings.swing(),
            align: align_file(o.settings.align),
        }),
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
    // Origin takes first: they have no origin themselves.
    let (derived, recorded): (Vec<TakeFile>, Vec<TakeFile>) =
        file.takes.into_iter().partition(|t| t.origin.is_some());
    for t in recorded.into_iter().chain(derived) {
        if takes.contains_key(&t.id) {
            return None;
        }
        let origin = match &t.origin {
            None => None,
            Some(o) => {
                let take: &Arc<Take> = takes.get(&o.take)?;
                if take.origin.is_some() {
                    return None;
                }
                let grid = Grid::new(o.division, o.triplet)?;
                let settings =
                    QuantizeSettings::new(grid, o.strength, o.swing, align_of(&o.align))?;
                Some(TakeOrigin {
                    take: take.clone(),
                    settings,
                })
            }
        };
        takes.insert(t.id, Arc::new(to_take(t, origin)?));
    }
    let d = file.song;
    let (returns, master) = d.mixer.map(|m| (m.returns, m.master)).unzip();
    let tracks_n = d.tracks.len();
    if tracks_n > MAX_TRACKS {
        return None;
    }
    let mut track_ids = HashSet::new();
    let mut clip_ids = HashSet::new();
    let mut tracks = Vec::with_capacity(tracks_n);
    for t in d.tracks {
        if !track_ids.insert(t.id) {
            return None;
        }
        let mut clips: Vec<Arc<Clip>> = Vec::with_capacity(t.clips.len());
        for c in t.clips {
            if !clip_ids.insert(c.id) {
                return None;
            }
            let range = TickRange::new(c.start, c.end).filter(|_| c.end <= MAX_TICK)?;
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
                    let take = takes.get(&ct.take)?;
                    let zero = (c.start as i64).checked_add(ct.offset)?;
                    (zero.unsigned_abs() <= MAX_TICK && take.length <= MAX_TICK).then_some(())?;
                    Some(ClipTake {
                        take: take.clone(),
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
            instrument: instrument_of(&t.instrument),
            channel: Channel {
                gain: gain_of(t.gain_db),
                pan: pan_of(t.pan.unwrap_or(0)),
                inserts: inserts_of(t.inserts.unwrap_or_default()),
            },
            sends: (t.sends).map_or([Gain::SILENT; 2], |s| [gain_of(s.a), gain_of(s.b)]),
            muted: t.muted,
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
        returns: match returns {
            Some(r) => [return_of(r.a), return_of(r.b)],
            None => mixer::new_song_returns(),
        },
        master: match master {
            Some(m) => Arc::new(Channel {
                gain: gain_of(m.gain_db),
                pan: Pan::CENTER,
                inserts: inserts_of(m.inserts),
            }),
            None => mixer::new_song_master(),
        },
        next_id: d.next_id,
    };
    let s = file.session;
    let known = |id: u32| track_ids.contains(&id).then_some(TrackId(id));
    let roll = roll_settings(&s);
    let session = Session {
        // A target that is gone falls back to the first track rather than failing the song.
        play_target: s
            .play_target
            .and_then(known)
            .or(doc.tracks.first().map(|t| t.id)),
        loop_range: match s.loop_range {
            Some(r) => Some(TickRange::new(r.start, r.end)?),
            None => None,
        },
        loop_on: s.loop_on,
        roll,
        soloed: BTreeSet::new(),
    };
    Some(Song { doc, session })
}

fn to_take(t: TakeFile, origin: Option<TakeOrigin>) -> Option<Take> {
    let from_len = origin.as_ref().map_or(0, |o| o.take.notes.len());
    let notes = t
        .notes
        .into_iter()
        .map(|n| {
            let (start, length, key, velocity, from) = match n {
                NoteFile::Plain(s, l, k, v) => (s, l, k, v, None),
                // An index into the origin take, so only in a take that has one.
                NoteFile::From(s, l, k, v, f) => {
                    ((f as usize) < from_len).then_some((s, l, k, v, OriginIndex::new(f)))?
                }
            };
            (length >= 1 && key <= 127 && (1..=127).contains(&velocity)).then_some(Note {
                start,
                length,
                key,
                velocity,
                from,
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
        notes: notes.into(),
        controls: controls.into(),
        origin,
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
