//! 「いいとこ取り」 of a vocal clip (protocol 33, round 12 A8) and 「音程を寄せる」 (M7).

use serde::{Deserialize, Serialize};

use crate::{AudioClipState, ScaleMode, SourceTempoView};

/// A part of a comp (`ClipView::comp`, `Command::SetComp`): from `from` to the `from` of the
/// next piece (the clip's end for the last), take `take` sounds. The pieces are in the order
/// of `from`, the first at 0, and two pieces next to each other have different takes. Seams
/// are crossfaded over 10 ms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CompPiece {
    /// Ticks from the clip's start.
    pub from: u64,
    /// `AudioTakeView::take_id` of a take of the clip.
    pub take: u32,
}

/// A take of a vocal clip (`AudioClipView::takes`), for the rows of the band 「いいとこ取り」:
/// its waveform is placed as `AudioClipView` says, with these values in place of the shown
/// take's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AudioTakeView {
    /// The take's id; `CompPiece::take` names it.
    pub take_id: u32,
    /// 「テイク 3」: its place in the clip's takes, from 1.
    pub number: u32,
    /// The audio file it plays, by its number in the song's audio table (`peaks`).
    pub audio: u32,
    pub first_frame: u64,
    pub frames: u64,
    /// As `ClipView::take_offset`.
    pub take_offset: i64,
    /// As `AudioClipView::source_tempo`, of this take.
    pub source_tempo: Option<SourceTempoView>,
    pub state: AudioClipState,
}

/// The band 「いいとこ取り」 shows the newest takes, this many rows at most.
pub const COMP_ROWS_MAX: u32 = 8;

/// 「音程を寄せる」 of a vocal clip (`StretchView::tune`): the scale it took when the strength
/// left 0, and the strength. `None` there: strength 0, and no scale kept; the panel shows the
/// song's scale (`PianoRollView::scale`) as where it would go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TuneView {
    /// 0..=11 is C..B, as `MusicalScale::root`.
    pub root: u8,
    pub mode: ScaleMode,
    /// 1..=`TUNE_STRENGTH_MAX` percent.
    pub strength: u8,
}

/// The strength of 「音程を寄せる」 at its end, in percent.
pub const TUNE_STRENGTH_MAX: u8 = 100;
