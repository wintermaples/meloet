//! The fields of round 12, waves 3 and 4, in the song file (still version 7): each is left out
//! when it holds the default, and a song without them reads with the defaults, which sound as
//! before. A meloet of version 7 that does not know a field skips it.
//!
//! - track `"arp": {"on": true, "shape": "upDown", "rate": 16, "triplet": false,
//!   "octaves": 2, "hold": false}` (E2): `rate` is steps to a whole note (4, 8, 16, 32).
//! - track `"knobs": {"brightness": 62.5, "attack": 50, "release": 50, "vibrato": 0}` (E3):
//!   percent; a knob left out is at its default.
//! - vocal clip `"comp": [[0, 2], [3840, 0]]` (A8): `[ticks from the clip's start, the take's
//!   place in "takes"]`.
//! - vocal clip `"shape": {…, "tune": {"root": 9, "mode": "minor", "strength": 60}}` (M7).
//! - `"mixer": {"master": {…, "automation": [lane]}}` (M5): lanes as a track's, of
//!   `{"kind": "volume"}` only.
//!
//! What is not understood reads as the default (an arpeggio, a knob, a comp, a tune); a lane
//! of the master breaks the song as a track's lane does.

use std::sync::Arc;

use meloet_model::song::Clip;
use meloet_model::{
    ARP_OCTAVES_MAX, Arp, ArpRate, ArpShape, AutoPoint, AutoTarget, CompPiece, Lane, LaneId,
    MAX_TICK, SoundKnob, SoundKnobs, TUNE_STRENGTH_MAX, Tune,
};
use serde::{Deserialize, Serialize};

use super::{LaneFile, ScaleModeFile, scale_mode_file, scale_mode_of};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArpFile {
    #[serde(default)]
    on: bool,
    #[serde(default)]
    shape: String,
    #[serde(default)]
    rate: u16,
    #[serde(default)]
    triplet: bool,
    #[serde(default)]
    octaves: u8,
    #[serde(default)]
    hold: bool,
}

const ARP_SHAPES: [(ArpShape, &str); 4] = [
    (ArpShape::Up, "up"),
    (ArpShape::Down, "down"),
    (ArpShape::UpDown, "upDown"),
    (ArpShape::Random, "random"),
];

pub(super) fn arp_file(arp: &Arp) -> Option<ArpFile> {
    (*arp != Arp::default()).then(|| ArpFile {
        on: arp.on,
        shape: (ARP_SHAPES.iter())
            .find(|(s, _)| *s == arp.shape)
            .map_or("up", |(_, n)| n)
            .to_owned(),
        rate: arp.rate.division(),
        triplet: arp.triplet,
        octaves: arp.octaves,
        hold: arp.hold,
    })
}

/// Each setting that is not understood takes its default on its own.
pub(super) fn arp_of(file: Option<ArpFile>) -> Arp {
    let default = Arp::default();
    let Some(f) = file else {
        return default;
    };
    Arp {
        on: f.on,
        shape: (ARP_SHAPES.iter())
            .find(|(_, n)| *n == f.shape)
            .map_or(default.shape, |(s, _)| *s),
        rate: ArpRate::from_division(f.rate).unwrap_or(default.rate),
        triplet: f.triplet,
        octaves: if (1..=ARP_OCTAVES_MAX).contains(&f.octaves) {
            f.octaves
        } else {
            default.octaves
        },
        hold: f.hold,
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct KnobsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    brightness: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attack: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    release: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vibrato: Option<f32>,
}

pub(super) fn knobs_file(knobs: &SoundKnobs) -> Option<KnobsFile> {
    let at = |knob: SoundKnob| Some(knobs.get(knob));
    (!knobs.is_default()).then(|| KnobsFile {
        brightness: at(SoundKnob::Brightness),
        attack: at(SoundKnob::Attack),
        release: at(SoundKnob::Release),
        vibrato: at(SoundKnob::Vibrato),
    })
}

/// A value out of 0..=100 is put at the nearer end.
pub(super) fn knobs_of(file: Option<KnobsFile>) -> SoundKnobs {
    let Some(f) = file else {
        return SoundKnobs::default();
    };
    [
        (SoundKnob::Brightness, f.brightness),
        (SoundKnob::Attack, f.attack),
        (SoundKnob::Release, f.release),
        (SoundKnob::Vibrato, f.vibrato),
    ]
    .into_iter()
    .fold(SoundKnobs::default(), |knobs, (knob, value)| {
        value.and_then(|v| knobs.with(knob, v)).unwrap_or(knobs)
    })
}

pub(super) fn comp_file(clip: &Clip) -> Vec<(u64, usize)> {
    clip.comp.iter().map(|p| (p.from, p.take)).collect()
}

/// The comp of a vocal clip `length` long with `takes` takes. Empty when it is not one: the
/// pieces in the order of `from`, the first at 0, each inside the clip, naming a take there
/// is, and no two in a row of one take.
pub(super) fn comp_of(
    file: Vec<(u64, usize)>,
    vocal: bool,
    length: u64,
    takes: usize,
) -> Vec<CompPiece> {
    let pieces: Vec<CompPiece> = (file.into_iter())
        .map(|(from, take)| CompPiece { from, take })
        .collect();
    let first = pieces.first().is_none_or(|p| p.from == 0);
    let inside = pieces.iter().all(|p| p.from < length && p.take < takes);
    let ordered = (pieces.windows(2)).all(|w| w[0].from < w[1].from && w[0].take != w[1].take);
    if (vocal || pieces.is_empty()) && first && inside && ordered {
        pieces
    } else {
        Vec::new()
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct TuneFile {
    root: u8,
    mode: ScaleModeFile,
    strength: u8,
}

pub(super) fn tune_file(tune: Option<Tune>) -> Option<TuneFile> {
    tune.map(|t| TuneFile {
        root: t.root,
        mode: scale_mode_file(t.mode),
        strength: t.strength,
    })
}

/// `None` (strength 0) for a tune out of range.
pub(super) fn tune_of(file: Option<&TuneFile>) -> Option<Tune> {
    let f = file?;
    let tune = Tune {
        root: f.root,
        mode: scale_mode_of(&f.mode),
        strength: f.strength.min(TUNE_STRENGTH_MAX),
    };
    tune.is_valid().then_some(tune)
}

/// The master's folded lane in the session's list of folded lanes.
pub(super) fn master_fold_file() -> serde_json::Value {
    serde_json::json!({"master": true, "target": {"kind": "volume"}})
}

/// Whether `folded` of the session's list is the master's lane, which `doc` has.
pub(super) fn is_master_fold(folded: &serde_json::Value, doc: &meloet_model::Doc) -> bool {
    *folded == master_fold_file() && !doc.master_lanes.is_empty()
}

pub(super) fn master_lanes_file(lanes: &[Arc<Lane>]) -> Vec<serde_json::Value> {
    (lanes.iter())
        .filter(|l| l.target == AutoTarget::Volume)
        .map(|l| {
            let file = LaneFile {
                target: serde_json::json!({"kind": "volume"}),
                step: l.step,
                resting: l.resting,
                points: l.points.iter().map(|p| (p.tick, p.value)).collect(),
            };
            serde_json::to_value(file).expect("a lane always serializes")
        })
        .collect()
}

/// The master's lanes, each with an id from `next_id`. A lane of another target, or one not
/// understood, is skipped. `None` for two lanes of the volume, and for points out of order, on
/// one tick, past the end of the song, or with a value outside 0..=1.
pub(super) fn master_lanes_of(
    files: Vec<serde_json::Value>,
    next_id: &mut u32,
) -> Option<Vec<Arc<Lane>>> {
    let mut lanes: Vec<Arc<Lane>> = Vec::new();
    for value in files {
        let Ok(file) = serde_json::from_value::<LaneFile>(value) else {
            continue;
        };
        if file.target.get("kind").and_then(|k| k.as_str()) != Some("volume") {
            continue;
        }
        if !lanes.is_empty() {
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
            target: AutoTarget::Volume,
            step: false,
            points: points.into(),
            resting: file.resting,
        }));
    }
    Some(lanes)
}
