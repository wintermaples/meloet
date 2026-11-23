//! The arpeggio of a keys track (protocol 33, round 12 E2): 「アルペジオ」 of the input band,
//! its small window, and the mark above 「音源」 on the mixer strip.

use serde::{Deserialize, Serialize};

/// The settings of a track's arpeggio (`TrackView::arp`, `Command::SetArp`). Part of the song.
/// Only what the keyboards play goes through it (MIDI keyboards and 「キーで弾く」); the notes
/// of the song, the auditions and the chord sheet's do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Arp {
    pub on: bool,
    pub shape: ArpShape,
    pub rate: ArpRate,
    /// 「三連符」: each step 2/3 as long.
    pub triplet: bool,
    /// 「オクターブ」, 1..=`ARP_OCTAVES_MAX`.
    pub octaves: u8,
    /// 「指を離しても続ける」.
    pub hold: bool,
}

/// Off, 「上へ」, 1/16, one octave: what a track starts with.
impl Default for Arp {
    fn default() -> Self {
        Arp {
            on: false,
            shape: ArpShape::Up,
            rate: ArpRate::Sixteenth,
            triplet: false,
            octaves: 1,
            hold: false,
        }
    }
}

/// 「形」, in the order of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ArpShape {
    /// 「上へ」.
    Up,
    /// 「下へ」.
    Down,
    /// 「上下」.
    UpDown,
    /// 「ばらばら」.
    Random,
}

impl ArpShape {
    pub const ALL: [ArpShape; 4] = [
        ArpShape::Up,
        ArpShape::Down,
        ArpShape::UpDown,
        ArpShape::Random,
    ];
}

/// 「速さ」: one step, in the order of the window ("1/4" … "1/32").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ArpRate {
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl ArpRate {
    pub const ALL: [ArpRate; 4] = [
        ArpRate::Quarter,
        ArpRate::Eighth,
        ArpRate::Sixteenth,
        ArpRate::ThirtySecond,
    ];
}

/// The most octaves an arpeggio goes up.
pub const ARP_OCTAVES_MAX: u8 = 3;
/// The most keys held that the arpeggio plays.
pub const ARP_KEYS_MAX: u32 = 16;
