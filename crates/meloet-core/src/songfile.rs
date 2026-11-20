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
    AutoPoint, AutoTarget, BuiltinInstrument, Bus, Channel, Clip, ClipId, ClipTake, ControlPoint,
    ControlValue, Doc, Gain, Grid, Insert, InsertId, Inserts, InstrumentRef, Kept, Lane, LaneId,
    MAX_TICK, Marker, MarkerId, Meter, MeterChange, MusicalScale, Note, OriginIndex, Pan,
    PluginRef, QuantizeAlign, QuantizeSettings, Return, RollSettings, ScaleMode, Session, Sf2Ref,
    Song, Take, TakeId, TakeOrigin, Tempo, TempoMap, TempoPoint, TickRange, Timeline, Track,
    TrackColor, TrackId, mixer,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

pub const FORMAT: &str = "meloet-song";
/// The newest shape this meloet reads and the one it writes. 2: quantized takes, with their
/// origin take and the origin index of each note. 3: instruments by name, a song may have no
/// tracks, the session has no armed track, and the mixer (pan, sends, inserts, the returns
/// and the master). 4: plugins and SF2 presets as instruments, plugins in insert slots, and
/// the mark of an insert that is passed by. 5: tempo points, meter changes and markers, the
/// loop's range in the song (what undo covers) and not in the session, the ids of the inserts,
/// the automation lanes of a track, and the folded lanes in the session.
pub const VERSION: u32 = 5;
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
    /// The tempo goes in a straight line from the head to the first tempo point.
    #[serde(default)]
    head_ramp: bool,
    /// In the order of `tick`; each tick 1 or more and only once.
    #[serde(default)]
    tempo_points: Vec<TempoPointFile>,
    /// In the order of `bar`; each bar 1 or more and only once.
    #[serde(default)]
    meter_changes: Vec<MeterChangeFile>,
    #[serde(default)]
    markers: Vec<MarkerFile>,
    /// The loop's range. Versions 1 to 4 had it in the session.
    #[serde(default, rename = "loop")]
    loop_range: Option<SpanFile>,
    /// A seat versions 1 to 4 kept and never filled. Read from those, never written.
    #[serde(default, skip_serializing)]
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TempoChangeFile {
    tick: u64,
    tempo: u32,
    meter: [u8; 2],
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TempoPointFile {
    tick: u64,
    /// Hundredths, as the head's.
    tempo: u32,
    /// To the next point in a straight line.
    ramp: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeterChangeFile {
    /// From 0: the bar shown as "9" is 8.
    bar: u32,
    /// `[num, den]`.
    meter: [u8; 2],
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkerFile {
    id: u32,
    tick: u64,
    name: String,
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
    /// Version 5: the lanes (`LaneFile`), in the order they were added. Read in `lanes_of`
    /// once the inserts have their ids.
    #[serde(default)]
    automation: Vec<serde_json::Value>,
}

/// `{"target": {"kind": "volume"}, "step": false, "resting": false, "points": [[0, 0.8],
/// [1920, 0.5]]}`. `target` is one of `{"kind": "volume" | "pan" | "mute"}`,
/// `{"kind": "send", "bus": "a" | "b"}`, `{"kind": "instrument", "param": 12}` (the plugin's
/// own number) and `{"kind": "insert", "insert": 7, "param": "threshold" | 12}` (the insert's
/// id; the parameter's name for an effect of meloet, the plugin's own number for a plugin).
/// The points are `[tick, value]`, the value the place of the knob, 0..=1. A lane's id is
/// not written: it is given when the song is read.
#[derive(Serialize, Deserialize)]
struct LaneFile {
    target: serde_json::Value,
    #[serde(default)]
    step: bool,
    #[serde(default)]
    resting: bool,
    points: Vec<(u64, f32)>,
}

/// `{"track": 1, "target": {…}}`, the target as in `LaneFile`.
#[derive(Serialize, Deserialize)]
struct LaneRefFile {
    track: u32,
    target: serde_json::Value,
}

/// The fields of an entry that is kept as it was read: the ones this meloet knows are taken
/// out, the others go back into the file untouched.
type Fields = serde_json::Map<String, serde_json::Value>;

/// One of (version 4; before it only the first)
/// - `{"kind": "builtin", "name": "grandPiano"}`. Versions 1 and 2 wrote
///   `{"kind": "placeholder"}`, the sound now named "sine".
/// - `{"kind": "plugin", "key": "vst3:…", "name": "Surge XT", "vendor": "Surge Synth Team",
///   "path": "/usr/lib/vst3/Surge XT.vst3", "state": "<base64>"}`. `path` and `state` are
///   left out when empty.
/// - `{"kind": "sf2", "file": "FluidR3_GM.sf2", "sha256": "<hex>", "bank": 0, "program": 27,
///   "name": "Clean Guitar"}`.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct InstrumentFile(Fields);

/// `null` is −∞.
#[derive(Serialize, Deserialize)]
struct SendsFile {
    #[serde(default)]
    a: Option<f64>,
    #[serde(default)]
    b: Option<f64>,
}

/// An insert slot: `null` when empty. An entry that is neither an effect nor a plugin is read
/// as empty.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SlotFile {
    Insert(InsertFile),
    Plugin(PluginSlotFile),
    Other(serde_json::Value),
}

/// `{"effect": "reverb", "params": {"decay": 2.4, …}}`. The parameters go by the names of
/// their tables (`meloet_model::effects`), so that a table can grow or change its order.
#[derive(Serialize, Deserialize)]
struct InsertFile {
    /// `Insert::id`. Missing in files written before inserts had ids: given when read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    effect: String,
    #[serde(default)]
    params: BTreeMap<String, ParamFile>,
    /// 「とばす」. Left out when off; not a switch, or missing: off.
    #[serde(
        default,
        deserialize_with = "bool_or_false",
        skip_serializing_if = "is_false"
    )]
    bypassed: bool,
}

/// `{"plugin": {"key": "lv2:…", "name": "MVerb", "vendor": "DISTRHO", "state": "<base64>"}}`,
/// with the fields of a plugin instrument.
#[derive(Serialize, Deserialize)]
struct PluginSlotFile {
    /// As `InsertFile::id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    plugin: Fields,
    #[serde(
        default,
        deserialize_with = "bool_or_false",
        skip_serializing_if = "is_false"
    )]
    bypassed: bool,
}

fn bool_or_false<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(serde_json::Value::deserialize(d)?
        .as_bool()
        .unwrap_or(false))
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
    /// Missing, or not a whole number: the middle.
    #[serde(default, deserialize_with = "pan_or_middle")]
    pan: i64,
    #[serde(default)]
    inserts: Vec<SlotFile>,
}

fn unity() -> Option<f64> {
    Some(0.0)
}

fn pan_or_middle<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(serde_json::Value::deserialize(d)?.as_i64().unwrap_or(0))
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
        .map(|slot| match (slot, slot.as_ref().and_then(Insert::kind)) {
            (None, _) => SlotFile::Other(serde_json::Value::Null),
            (Some(insert), None) => match insert.plugin_ref() {
                Some(plugin) => SlotFile::Plugin(PluginSlotFile {
                    id: Some(insert.id.0),
                    plugin: plugin_fields(plugin),
                    bypassed: insert.bypassed,
                }),
                None => SlotFile::Other(serde_json::Value::Null),
            },
            (Some(insert), Some(kind)) => {
                let params = kind.params().iter().zip(insert.values());
                SlotFile::Insert(InsertFile {
                    id: Some(insert.id.0),
                    effect: kind.name().into(),
                    bypassed: insert.bypassed,
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
/// range is brought into range. A plugin stays in its slot whether or not this PC has it.
/// Slots past the last one are dropped. An insert the file gives no id has `NO_INSERT_ID`
/// until `give_insert_ids`.
fn inserts_of(slots: Vec<SlotFile>) -> Inserts {
    let id = |id: Option<u32>| InsertId(id.unwrap_or(NO_INSERT_ID.0));
    let mut inserts = Inserts::default();
    for (place, slot) in inserts.iter_mut().zip(slots) {
        let file = match slot {
            SlotFile::Insert(file) => file,
            SlotFile::Plugin(file) => {
                *place = plugin_of(file.plugin).map(|plugin| {
                    let mut insert = Insert::plugin(id(file.id), Arc::new(plugin));
                    insert.bypassed = file.bypassed;
                    insert
                });
                continue;
            }
            SlotFile::Other(_) => continue,
        };
        let Some(kind) = EffectKind::from_name(&file.effect) else {
            continue;
        };
        let mut insert = Insert::new(id(file.id), kind);
        insert.bypassed = file.bypassed;
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

/// Stands for "the file gives no id" while a song is read. Ids start at 1.
const NO_INSERT_ID: InsertId = InsertId(0);

fn inserts_mut(doc: &mut Doc) -> impl Iterator<Item = &mut Insert> {
    let tracks = (doc.tracks.iter_mut()).map(|t| &mut Arc::make_mut(t).channel);
    let returns = (doc.returns.iter_mut()).map(|r| &mut Arc::make_mut(r).channel);
    tracks
        .chain(returns)
        .chain(std::iter::once(Arc::make_mut(&mut doc.master)))
        .flat_map(|c| c.inserts.iter_mut().flatten())
}

/// Checks the ids the file gave its inserts (below `next_id`, each once, none `taken` by a
/// clip, a take or a marker), and gives the inserts without one theirs from `next_id`.
fn give_insert_ids(doc: &mut Doc, taken: impl Fn(u32) -> bool) -> Option<()> {
    let mut next_id = doc.next_id;
    let mut seen = HashSet::new();
    for insert in inserts_mut(doc) {
        if insert.id == NO_INSERT_ID {
            insert.id = InsertId(next_id);
            next_id = next_id.checked_add(1)?;
        } else if insert.id.0 >= next_id || taken(insert.id.0) || !seen.insert(insert.id) {
            return None;
        }
    }
    doc.next_id = next_id;
    Some(())
}

/// Test help: gives every insert of `doc` an id from `doc.next_id`, as a file that has none is
/// read. For songs put together by hand around a new song, whose inserts have the ids 1 to 3.
#[cfg(test)]
pub(crate) fn renumber_inserts(doc: &mut Doc) {
    for insert in inserts_mut(doc) {
        insert.id = NO_INSERT_ID;
    }
    give_insert_ids(doc, |_| false).expect("ids are left");
}

fn target_json(track: &Track, target: AutoTarget) -> serde_json::Value {
    use serde_json::json;
    match target {
        AutoTarget::Volume => json!({"kind": "volume"}),
        AutoTarget::Pan => json!({"kind": "pan"}),
        AutoTarget::Mute => json!({"kind": "mute"}),
        AutoTarget::Send(bus) => json!({"kind": "send", "bus": bus_name(bus)}),
        AutoTarget::Instrument { param } => json!({"kind": "instrument", "param": param}),
        AutoTarget::Insert { insert, param } => {
            let kind = (track.channel.inserts.iter().flatten())
                .find(|i| i.id == insert)
                .and_then(Insert::kind);
            match kind.and_then(|k| k.param(param)) {
                Some(spec) => json!({"kind": "insert", "insert": insert.0, "param": spec.name}),
                None => json!({"kind": "insert", "insert": insert.0, "param": param}),
            }
        }
    }
}

fn bus_name(bus: Bus) -> &'static str {
    match bus {
        Bus::A => "a",
        Bus::B => "b",
    }
}

/// What a target of the file names on `track`; `None` when it is not understood or the track
/// does not have it (an effect this meloet does not know left its slot empty).
fn target_of(track: &Track, value: &serde_json::Value) -> Option<AutoTarget> {
    let number = |name: &str| u32::try_from(value.get(name)?.as_u64()?).ok();
    let target = match value.get("kind")?.as_str()? {
        "volume" => AutoTarget::Volume,
        "pan" => AutoTarget::Pan,
        "mute" => AutoTarget::Mute,
        "send" => AutoTarget::Send(match value.get("bus")?.as_str()? {
            "a" => Bus::A,
            "b" => Bus::B,
            _ => return None,
        }),
        "instrument" => AutoTarget::Instrument {
            param: number("param")?,
        },
        "insert" => {
            let insert = InsertId(number("insert")?);
            let held = (track.channel.inserts.iter().flatten()).find(|i| i.id == insert)?;
            let param = match (held.kind(), value.get("param")?) {
                (Some(kind), serde_json::Value::String(name)) => kind.param_index(name)?,
                (None, _) => number("param")?,
                (Some(_), _) => return None,
            };
            AutoTarget::Insert { insert, param }
        }
        _ => return None,
    };
    track.has_target(target).then_some(target)
}

/// Whether a lane of `target` holds steps: what the target itself says, and for a plugin's
/// parameter what the file says.
fn steps_of(track: &Track, target: AutoTarget, file: bool) -> bool {
    match target {
        AutoTarget::Volume | AutoTarget::Pan | AutoTarget::Send(_) => false,
        AutoTarget::Mute => true,
        AutoTarget::Instrument { .. } => file,
        AutoTarget::Insert { insert, param } => {
            let held = track
                .channel
                .inserts
                .iter()
                .flatten()
                .find(|i| i.id == insert);
            match held.and_then(Insert::kind).and_then(|k| k.param(param)) {
                Some(spec) => !matches!(spec.shape, ParamShape::Continuous),
                None => file,
            }
        }
    }
}

/// The lanes of a track, each with an id from `next_id`. A lane whose target is not
/// understood, or that the track does not have, is skipped. `None` for two lanes of one
/// target, and for points out of order, on one tick, past the end of the song, or with a
/// value outside 0..=1.
fn lanes_of(
    track: &Track,
    files: Vec<serde_json::Value>,
    next_id: &mut u32,
) -> Option<Vec<Arc<Lane>>> {
    let mut lanes: Vec<Arc<Lane>> = Vec::with_capacity(files.len());
    for value in files {
        let Ok(file) = serde_json::from_value::<LaneFile>(value) else {
            continue;
        };
        let Some(target) = target_of(track, &file.target) else {
            continue;
        };
        if lanes.iter().any(|l| l.target == target) {
            return None;
        }
        let points: Vec<AutoPoint> = (file.points.into_iter())
            .map(|(tick, value)| AutoPoint { tick, value })
            .collect();
        let ordered = points.windows(2).all(|w| w[0].tick < w[1].tick);
        let inside = (points.iter()).all(|p| p.tick <= MAX_TICK && (0.0..=1.0).contains(&p.value));
        if !ordered || !inside {
            return None;
        }
        let id = LaneId(*next_id);
        *next_id = next_id.checked_add(1)?;
        lanes.push(Arc::new(Lane {
            id,
            target,
            step: steps_of(track, target, file.step),
            points: points.into(),
            resting: file.resting,
        }));
    }
    Some(lanes)
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
            pan: i64::from(doc.master.pan.get()),
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
const KIND_PLUGIN: &str = "plugin";
const KIND_SF2: &str = "sf2";

fn text(text: &str) -> serde_json::Value {
    serde_json::Value::String(text.to_owned())
}

/// The field `name` when it is text. It is taken out either way.
fn take_text(fields: &mut Fields, name: &str) -> Option<String> {
    match fields.remove(name)? {
        serde_json::Value::String(text) => Some(text),
        _ => None,
    }
}

fn take_number<T: TryFrom<u64>>(fields: &mut Fields, name: &str) -> Option<T> {
    T::try_from(fields.remove(name)?.as_u64()?).ok()
}

/// What is left of an entry after the known fields were taken out.
fn kept_of(fields: Fields) -> Kept {
    if fields.is_empty() {
        return Kept::NONE;
    }
    Kept::new(&serde_json::to_string(&fields).expect("a map of JSON values serializes"))
}

fn kept_fields(kept: &Kept) -> Fields {
    (kept.text())
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default()
}

/// The fields of a plugin: the ones it was read with that this meloet does not know, and
/// over them the ones it knows.
fn plugin_fields(plugin: &PluginRef) -> Fields {
    let mut fields = kept_fields(&plugin.kept);
    fields.insert("key".into(), text(&plugin.key));
    fields.insert("name".into(), text(&plugin.name));
    fields.insert("vendor".into(), text(&plugin.vendor));
    if !plugin.path.is_empty() {
        fields.insert("path".into(), text(&plugin.path));
    }
    if !plugin.state.is_empty() {
        fields.insert("state".into(), text(&base64::encode(&plugin.state)));
    }
    fields
}

/// `None` without a key. Whether this PC has the plugin, or reads its format, is not asked
/// here: the song keeps it either way. A state that is not base64 is read as no state.
fn plugin_of(mut fields: Fields) -> Option<PluginRef> {
    let key = take_text(&mut fields, "key").filter(|key| !key.is_empty())?;
    let mut word = |name| take_text(&mut fields, name).unwrap_or_default();
    let (name, vendor, path) = (word("name"), word("vendor"), word("path"));
    let state = take_text(&mut fields, "state")
        .and_then(|state| base64::decode(&state))
        .unwrap_or_default();
    Some(PluginRef {
        key: key.into(),
        name: name.into(),
        vendor: vendor.into(),
        path: path.into(),
        state: state.into(),
        kept: kept_of(fields),
    })
}

fn sf2_fields(sf2: &Sf2Ref) -> Fields {
    let mut fields = kept_fields(&sf2.kept);
    fields.insert("file".into(), text(&sf2.file));
    fields.insert("sha256".into(), text(&sf2.sha256));
    fields.insert("bank".into(), sf2.bank.into());
    fields.insert("program".into(), sf2.program.into());
    fields.insert("name".into(), text(&sf2.name));
    fields
}

/// `None` without the name of the file, a bank and a program from 0 to 127.
fn sf2_of(mut fields: Fields) -> Option<Sf2Ref> {
    let file = take_text(&mut fields, "file").filter(|file| !file.is_empty())?;
    let bank = take_number::<u16>(&mut fields, "bank")?;
    let program = take_number::<u8>(&mut fields, "program").filter(|p| *p <= 127)?;
    let mut word = |name| take_text(&mut fields, name).unwrap_or_default();
    let (sha256, name) = (word("sha256"), word("name"));
    Some(Sf2Ref {
        file: file.into(),
        sha256: sha256.into(),
        bank,
        program,
        name: name.into(),
        kept: kept_of(fields),
    })
}

fn instrument_file(instrument: &InstrumentRef) -> InstrumentFile {
    let (kind, mut fields) = match instrument {
        InstrumentRef::Builtin(b) => {
            let mut fields = Fields::new();
            fields.insert("name".into(), text(b.name()));
            (KIND_BUILTIN, fields)
        }
        InstrumentRef::Plugin(plugin) => (KIND_PLUGIN, plugin_fields(plugin)),
        InstrumentRef::Sf2(sf2) => (KIND_SF2, sf2_fields(sf2)),
    };
    fields.insert("kind".into(), text(kind));
    InstrumentFile(fields)
}

/// An instrument this meloet does not know (a sound of a later meloet, a name spelled by
/// hand, a kind it has no word for) becomes the sine: the notes of a song are worth more than
/// the name of a sound, so the song is not refused for it. A plugin or an SF2 preset is not
/// such a one: it is kept as it was read, whether or not this PC has it.
fn instrument_of(file: InstrumentFile) -> InstrumentRef {
    let mut fields = file.0;
    let known = match take_text(&mut fields, "kind").as_deref() {
        Some(KIND_BUILTIN) => take_text(&mut fields, "name")
            .and_then(|name| BuiltinInstrument::from_name(&name))
            .map(InstrumentRef::Builtin),
        Some(KIND_PLUGIN) => plugin_of(fields).map(|p| InstrumentRef::Plugin(Arc::new(p))),
        Some(KIND_SF2) => sf2_of(fields).map(|s| InstrumentRef::Sf2(Arc::new(s))),
        _ => None,
    };
    known.unwrap_or(InstrumentRef::SINE)
}

/// Base64 with the usual alphabet and padding: how a plugin's state sits in the JSON.
mod base64 {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let n = chunk
                .iter()
                .enumerate()
                .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    /// `None` for text that `encode` does not write.
    pub fn decode(text: &str) -> Option<Vec<u8>> {
        let text = text.as_bytes();
        if !text.len().is_multiple_of(4) {
            return None;
        }
        let mut out = Vec::with_capacity(text.len() / 4 * 3);
        let last = text.len().saturating_sub(4);
        for (at, chunk) in text.chunks(4).enumerate() {
            let pad = chunk.iter().rev().take_while(|c| **c == b'=').count();
            if pad > 2 || (pad > 0 && at * 4 != last) {
                return None;
            }
            let mut n = 0u32;
            for c in &chunk[..4 - pad] {
                let six = ALPHABET.iter().position(|a| a == c)? as u32;
                n = n << 6 | six;
            }
            n <<= 6 * pad as u32;
            // The bits that padding leaves over are zero in what `encode` writes.
            if n & ((1 << (8 * pad)) - 1) != 0 {
                return None;
            }
            out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
        }
        Some(out)
    }
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
    /// Versions 1 to 4. From version 5 the range is in the song.
    #[serde(default, rename = "loop", skip_serializing)]
    loop_range: Option<SpanFile>,
    loop_on: bool,
    /// The piano roll's settings. Missing or not understood: the default.
    #[serde(default, deserialize_with = "or_default")]
    grid: Option<GridFile>,
    #[serde(default, deserialize_with = "or_default")]
    scale: Option<ScaleFile>,
    #[serde(default, deserialize_with = "or_default")]
    quantize: Option<QuantizeFile>,
    /// Version 5: the folded lanes. One that is not understood, or names no lane, is skipped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    folded_lanes: Vec<serde_json::Value>,
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
            automation: (t.automation.iter())
                .map(|l| {
                    let file = LaneFile {
                        target: target_json(t, l.target),
                        step: l.step,
                        resting: l.resting,
                        points: l.points.iter().map(|p| (p.tick, p.value)).collect(),
                    };
                    serde_json::to_value(file).expect("a lane always serializes")
                })
                .collect(),
        })
        .collect();
    let s = &song.session;
    let timeline = &doc.timeline;
    let file = SongFile {
        format: FORMAT.into(),
        version: VERSION,
        app: concat!("meloet ", env!("CARGO_PKG_VERSION")).into(),
        song: DocFile {
            name: doc.name.clone(),
            tempo: timeline.tempo.centi(),
            meter: [timeline.meter.num(), timeline.meter.den()],
            head_ramp: timeline.head_ramp,
            tempo_points: (timeline.tempo_points.iter())
                .map(|p| TempoPointFile {
                    tick: p.tick,
                    tempo: p.tempo.centi(),
                    ramp: p.ramp,
                })
                .collect(),
            meter_changes: (timeline.meter_changes.iter())
                .map(|c| MeterChangeFile {
                    bar: c.bar,
                    meter: [c.meter.num(), c.meter.den()],
                })
                .collect(),
            markers: (doc.markers.iter())
                .map(|m| MarkerFile {
                    id: m.id.0,
                    tick: m.tick,
                    name: m.name.clone(),
                })
                .collect(),
            loop_range: doc.loop_range.map(|r| SpanFile {
                start: r.start(),
                end: r.end(),
            }),
            tempo_changes: Vec::new(),
            next_id: doc.next_id,
            tracks,
            mixer: Some(mixer_file(doc)),
            audio: Vec::new(),
        },
        takes,
        session: SessionFile {
            play_target: s.play_target.map(|t| t.0),
            loop_range: None,
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
            folded_lanes: (s.folded_lanes.iter())
                .filter_map(|&(track, target)| {
                    let t = doc.track(track)?;
                    let file = LaneRefFile {
                        track: track.0,
                        target: target_json(t, target),
                    };
                    Some(serde_json::to_value(file).expect("a lane always serializes"))
                })
                .collect(),
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
    to_song(file, head.version).ok_or(ReadError::Broken)
}

/// `None` when anything breaks a rule the model keeps.
fn to_song(file: SongFile, version: u32) -> Option<Song> {
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
    let mut lane_files = Vec::with_capacity(tracks_n);
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
            instrument: instrument_of(t.instrument),
            channel: Channel {
                gain: gain_of(t.gain_db),
                pan: pan_of(t.pan.unwrap_or(0)),
                inserts: inserts_of(t.inserts.unwrap_or_default()),
            },
            sends: (t.sends).map_or([Gain::SILENT; 2], |s| [gain_of(s.a), gain_of(s.b)]),
            muted: t.muted,
            clips,
            automation: Vec::new(),
        }));
        lane_files.push(t.automation);
    }
    if clip_ids
        .iter()
        .chain(takes.keys())
        .any(|&id| id >= d.next_id)
    {
        return None;
    }
    let mut timeline = Timeline::new(
        Tempo::from_centi(d.tempo)?,
        Meter::new(d.meter[0], d.meter[1])?,
    );
    let mut s = file.session;
    let loop_range = if version >= 5 {
        timeline.head_ramp = d.head_ramp;
        for p in d.tempo_points {
            timeline.tempo_points.push(TempoPoint {
                tick: Some(p.tick).filter(|&t| t <= MAX_TICK)?,
                tempo: Tempo::from_centi(p.tempo)?,
                ramp: p.ramp,
            });
        }
        for c in d.meter_changes {
            timeline.meter_changes.push(MeterChange {
                bar: c.bar,
                meter: Meter::new(c.meter[0], c.meter[1])?,
            });
        }
        // Out of order, two at one place, or one at the head.
        timeline.check().ok()?;
        d.loop_range
    } else {
        old_tempo_changes(&mut timeline, d.tempo_changes)?;
        s.loop_range.take()
    };
    let mut markers = Vec::with_capacity(d.markers.len());
    let mut marker_ids = HashSet::new();
    for m in d.markers.into_iter().filter(|_| version >= 5) {
        // A marker's id is no clip's, no take's and no other marker's.
        if !marker_ids.insert(m.id)
            || m.id >= d.next_id
            || clip_ids.contains(&m.id)
            || takes.contains_key(&m.id)
            || m.tick > MAX_TICK
        {
            return None;
        }
        markers.push(Marker {
            id: MarkerId(m.id),
            tick: m.tick,
            name: m.name,
        });
    }
    markers.sort_by_key(|m| (m.tick, m.id));
    let (own_returns, own_master) = (returns.is_some(), master.is_some());
    let mut doc = Doc {
        name: d.name,
        timeline: Arc::new(timeline),
        markers: Arc::new(markers),
        loop_range: match loop_range {
            Some(r) => Some(TickRange::new(r.start, r.end)?),
            None => None,
        },
        tracks,
        returns: match returns {
            Some(r) => [return_of(r.a), return_of(r.b)],
            None => mixer::new_song_returns(),
        },
        master: match master {
            Some(m) => Arc::new(Channel {
                gain: gain_of(m.gain_db),
                pan: pan_of(m.pan),
                inserts: inserts_of(m.inserts),
            }),
            None => mixer::new_song_master(),
        },
        next_id: d.next_id,
    };
    // The returns and the master of a new song come with ids of their own, which this song
    // may have given to something else.
    if !own_returns {
        for r in &mut doc.returns {
            for insert in Arc::make_mut(r).channel.inserts.iter_mut().flatten() {
                insert.id = NO_INSERT_ID;
            }
        }
    }
    if !own_master {
        for insert in Arc::make_mut(&mut doc.master).inserts.iter_mut().flatten() {
            insert.id = NO_INSERT_ID;
        }
    }
    give_insert_ids(&mut doc, |id| {
        clip_ids.contains(&id) || takes.contains_key(&id) || marker_ids.contains(&id)
    })?;
    if version >= 5 {
        for (slot, files) in doc.tracks.iter_mut().zip(lane_files) {
            let track = Arc::make_mut(slot);
            track.automation = lanes_of(track, files, &mut doc.next_id)?;
        }
    }
    let known = |id: u32| track_ids.contains(&id).then_some(TrackId(id));
    let roll = roll_settings(&s);
    let session = Session {
        // A target that is gone falls back to the first track rather than failing the song.
        play_target: s
            .play_target
            .and_then(known)
            .or(doc.tracks.first().map(|t| t.id)),
        loop_on: s.loop_on,
        roll,
        soloed: BTreeSet::new(),
        folded_lanes: (s.folded_lanes.into_iter())
            .filter_map(|v| {
                let file: LaneRefFile = serde_json::from_value(v).ok()?;
                let t = doc.track(TrackId(file.track))?;
                let target = target_of(t, &file.target)?;
                t.lane(target).map(|_| (t.id, target))
            })
            .collect(),
    };
    Some(Song { doc, session })
}

/// The seat of versions 1 to 4 should be empty. When it is not: each entry's tempo is a tempo
/// point, and its meter a meter change when the entry is at the head of a bar (of the bars as
/// the entries before it left them). An entry at tick 0 is left out: the head has its own.
fn old_tempo_changes(timeline: &mut Timeline, mut changes: Vec<TempoChangeFile>) -> Option<()> {
    changes.sort_by_key(|c| c.tick);
    for c in changes {
        let tempo = Tempo::from_centi(c.tempo)?;
        let meter = Meter::new(c.meter[0], c.meter[1])?;
        if c.tick == 0 || c.tick > MAX_TICK {
            continue;
        }
        match timeline.tempo_points.last_mut() {
            Some(last) if last.tick == c.tick => last.tempo = tempo,
            _ => timeline.tempo_points.push(TempoPoint {
                tick: c.tick,
                tempo,
                ramp: false,
            }),
        }
        let at = TempoMap::new(timeline).meter_at(c.tick);
        let Ok(bar) = u32::try_from(at.bar) else {
            continue;
        };
        if at.bar_tick != c.tick || at.meter == meter {
            continue;
        }
        match timeline.meter_changes.last_mut() {
            Some(last) if last.bar == bar => last.meter = meter,
            _ => timeline.meter_changes.push(MeterChange { bar, meter }),
        }
    }
    Some(())
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
