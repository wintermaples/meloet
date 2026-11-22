//! Automation (protocol 21; docs/architecture.md 21-5, 21-8, 21-15).
//!
//! A lane belongs to a track and moves one thing of it. `song` has each lane's header only;
//! the points are the reply of the `lane` invoke, binary, kept by the screen by `lane_id` as
//! takes are: a lane whose points change gets a new `lane_id`.
//!
//! A value is always the place of the knob a person sees, 0.0..=1.0: the fader's and a
//! send's length (spec 2 「メーターとフェーダーの目盛」; 0 is −∞), `(pan + 100) / 200`, the
//! bar of a parameter in the parameters panel, a step as n / (steps − 1), a switch and the
//! mute 0 or 1. Between two points the value goes in a straight line over ticks; in a lane
//! with `step` it stays at the point before. Before the first point it is the first point's
//! value, after the last the last's.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{EffectKind, IndexRange, SendBus};

/// What a lane moves. The returns and the master have no lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AutoTarget {
    /// The fader.
    Volume,
    Pan,
    Send {
        bus: SendBus,
    },
    Mute,
    /// A parameter of the plugin that is the track's instrument: `ParamRow::id`.
    Instrument {
        param: u32,
    },
    /// A parameter of what sits in an insert slot of the track. `insert` is `InsertView::id`,
    /// not the slot: the lane goes with the effect when the slots are put in another order.
    /// `param`: the place in the effect's table (`EFFECT_PARAMS`) for an effect that comes
    /// with meloet, `ParamRow::id` for a plugin.
    Insert {
        insert: u32,
        param: u32,
    },
}

/// A lane as commands name it. `track` is `TrackView::id`. A track has one lane at most for
/// a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LaneRef {
    pub track: u32,
    pub target: AutoTarget,
}

/// What a lane moves, as its header ("オートメーション ・ 音量"), the status line and the
/// history band name it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LaneName {
    Volume,
    Pan,
    Send {
        bus: SendBus,
    },
    Mute,
    /// A parameter of an effect that comes with meloet: the screen has both names.
    Builtin {
        effect: EffectKind,
        param: u32,
    },
    /// A parameter of a plugin, instrument or insert. `param`: its name as the plugin says
    /// it; `None` while the plugin is not read (not found, stopped before it was read).
    Plugin {
        plugin: String,
        param: Option<String>,
    },
}

/// The header of a lane, in `TrackView::lanes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LaneView {
    pub target: AutoTarget,
    pub name: LaneName,
    /// Names the points as they are now; the `lane` invoke takes it. A new number whenever
    /// the points change, and the number it had when an undo brings them back. Commands on
    /// points name the lane by it, and one with a number that is no lane's now does nothing.
    pub lane_id: u32,
    /// How many points ("点 5").
    pub points: u32,
    /// The value stays at the point before until the next one (a stepped value, a switch,
    /// the mute).
    pub step: bool,
    /// 「休み」 (spec 3, 17-A): the lane moves nothing, the value is where the hand put it.
    /// Saved with the song; resting and 「もどす」 are steps.
    pub resting: bool,
}

/// The section `automation`: what of the lanes is no step. A lane that is in none of the
/// lists is unfolded, not armed and not being written.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AutomationView {
    /// The folded lanes (`Command::FoldLane`).
    pub folded: Vec<LaneRef>,
    /// The lanes whose 「● 書く」 is on (`Command::SetLaneWrite`). Stopping takes every
    /// lane out, and so does a lane coming to rest.
    pub armed: Vec<LaneRef>,
    /// The lanes written right now: the hand is on the knob while the song plays. The
    /// screen draws the rec frame from `from_tick` to the play line.
    pub writing: Vec<WritingLane>,
    /// Lanes a hand holds outside of any step, which show as resting until the step is made
    /// or given up: a parameter moved in the plugin's own window, which becomes a step when
    /// the person leaves the window (docs/architecture.md 21-10).
    pub held: Vec<LaneRef>,
    /// The selected points, of one lane at most. Core keeps it and undo brings it back, as
    /// the selected notes.
    pub selected: Option<AutoSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WritingLane {
    pub lane: LaneRef,
    /// Where the writing began, a song position. In a loop, where it began in this pass.
    pub from_tick: u64,
}

/// Points by their place in the lane (in the order of tick, from 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AutoSelection {
    /// `LaneView::lane_id`.
    pub lane_id: u32,
    pub points: Vec<IndexRange>,
}

pub const LANE_KIND: u8 = 3;
pub const LANE_VERSION: u8 = 1;
pub const LANE_HEADER_LEN: usize = 16;
pub const LANE_POINT_LEN: usize = 16;

const FLAG_STEP: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LanePoint {
    /// A song position.
    pub tick: u64,
    /// 0.0..=1.0.
    pub value: f32,
}

/// The points of one lane, binary, version 1, little-endian: the reply of the `lane` invoke.
/// The layout table is docs/architecture.md 21-15; `ui/src/bridge/lane.ts` reads it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LanePoints {
    /// `LaneView::lane_id`.
    pub id: u32,
    pub step: bool,
    /// In the order of tick, each tick once.
    pub points: Vec<LanePoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneError {
    TooShort {
        len: usize,
    },
    WrongKind(u8),
    WrongVersion(u8),
    /// The length does not match the count in the header.
    WrongLength {
        len: usize,
        expected: usize,
    },
}

impl fmt::Display for LaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(f, "lane is {len} bytes, shorter than the header"),
            Self::WrongKind(k) => write!(f, "lane kind {k}, expected {LANE_KIND}"),
            Self::WrongVersion(v) => write!(f, "lane version {v}, expected {LANE_VERSION}"),
            Self::WrongLength { len, expected } => {
                write!(f, "lane is {len} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for LaneError {}

impl LanePoints {
    pub fn encoded_len(&self) -> usize {
        LANE_HEADER_LEN + LANE_POINT_LEN * self.points.len()
    }

    /// # Panics
    /// When there are more than `u32::MAX` points.
    pub fn encode(&self) -> Vec<u8> {
        let count = u32::try_from(self.points.len()).expect("count fits in u32");
        let mut out = Vec::with_capacity(self.encoded_len());
        out.push(LANE_KIND);
        out.push(LANE_VERSION);
        out.push(if self.step { FLAG_STEP } else { 0 });
        out.push(0);
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        for p in &self.points {
            out.extend_from_slice(&p.tick.to_le_bytes());
            out.extend_from_slice(&p.value.to_le_bytes());
            out.extend_from_slice(&[0; 4]);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<LanePoints, LaneError> {
        if bytes.len() < LANE_HEADER_LEN {
            return Err(LaneError::TooShort { len: bytes.len() });
        }
        if bytes[0] != LANE_KIND {
            return Err(LaneError::WrongKind(bytes[0]));
        }
        if bytes[1] != LANE_VERSION {
            return Err(LaneError::WrongVersion(bytes[1]));
        }
        let u32_at = |pos: usize| u32::from_le_bytes(bytes[pos..pos + 4].try_into().expect("4"));
        let count = u32_at(8) as usize;
        let expected = count
            .checked_mul(LANE_POINT_LEN)
            .and_then(|n| n.checked_add(LANE_HEADER_LEN))
            .unwrap_or(usize::MAX);
        if bytes.len() != expected {
            return Err(LaneError::WrongLength {
                len: bytes.len(),
                expected,
            });
        }
        Ok(LanePoints {
            id: u32_at(4),
            step: bytes[2] & FLAG_STEP != 0,
            points: (0..count)
                .map(|i| {
                    let p = LANE_HEADER_LEN + LANE_POINT_LEN * i;
                    LanePoint {
                        tick: u64::from_le_bytes(bytes[p..p + 8].try_into().expect("8")),
                        value: f32::from_le_bytes(bytes[p + 8..p + 12].try_into().expect("4")),
                    }
                })
                .collect(),
        })
    }
}
