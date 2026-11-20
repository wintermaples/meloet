//! Tempo, meter and markers along the song (round 6; docs/architecture.md 21-1, 21-15).
//!
//! A tempo is beats per minute as an `f64` that is a whole number of 0.01 steps, the beat
//! being the lower number of the meter in force there (`meloet_model::Tempo::bpm`).
//!
//! What names a tempo point is its tick, a meter change its bar: there is one at most at each
//! place. Tick 0 and bar 0 are the song's head, which is always there, can be selected and
//! changed, and can not be moved or deleted. A command that names a place with nothing at it
//! does nothing, so one sent just before an undo never lands on another point.

use serde::{Deserialize, Serialize};

/// The section `timeline`. The screen makes its tempo map (`ui/src/time/tempoMap.ts`) from
/// the head, `tempo_points` and `meter_changes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TimelineView {
    /// The song's head.
    pub tempo_bpm: f64,
    pub meter_num: u8,
    pub meter_den: u8,
    /// The tempo goes in a straight line from the head to the first point.
    pub head_ramp: bool,
    /// In the order of `tick`; each tick 1 or more and only once.
    pub tempo_points: Vec<TempoPointView>,
    /// In the order of `bar`; each bar 1 or more and only once.
    pub meter_changes: Vec<MeterChangeView>,
    /// In the order of `tick`, those at one tick in the order of `id`.
    pub markers: Vec<MarkerView>,
    /// One thing at most among tempo points, meter changes and markers. Core keeps it and
    /// undo brings it back, as the selected notes.
    pub selected: Option<TimelineRef>,
    /// The marker whose name field is open: the one `AddMarker` just made, or the one of
    /// `OpenMarkerName`. `RenameMarker` and `CloseMarkerName` close it, and so does the
    /// marker going away.
    pub renaming: Option<u32>,
    /// The row "テンポ ・ 拍子の変更" of the arrange view is open (`SetTimelineOpen`). Not a
    /// part of the song; closed after a start, a new song and an open.
    pub open: bool,
}

/// A tempo from `tick` on. With `ramp` the tempo goes in a straight line over ticks to the
/// next point; on the last point `ramp` is kept and does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TempoPointView {
    pub tick: u64,
    pub tempo_bpm: f64,
    pub ramp: bool,
}

/// A meter from the head of bar `bar` on. Bars count from 0 here, as in the song's file: the
/// bar shown as "9" is 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MeterChangeView {
    pub bar: u32,
    pub meter_num: u8,
    pub meter_den: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MarkerView {
    /// Stays with the marker for as long as the song is open, and is never given to another
    /// one. Commands name markers by it.
    pub id: u32,
    pub tick: u64,
    pub name: String,
}

/// A tempo point, a meter change or a marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TimelineRef {
    /// 0: the head's tempo.
    TempoPoint { tick: u64 },
    /// From 0; 0: the head's meter.
    MeterChange { bar: u32 },
    /// `MarkerView::id`.
    Marker { id: u32 },
}
