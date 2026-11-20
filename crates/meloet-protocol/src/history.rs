//! The history band (round 6, protocol 21; docs/architecture.md 21-12, 21-15).
//!
//! The section `history` has the counts only. The rows are the reply of the `history_rows`
//! invoke, fetched for what the band shows: a song may have thousands of steps, and a held
//! arrow key makes thirty a second.
//!
//! A row is "「（手の名前）」・ （何を）" and where (spec 3, Y19). Rust sends which name and
//! the values; the wording is the screen's (`ui/src/text/ja.ts`).

use serde::{Deserialize, Serialize};

use crate::{
    BarSpan, ControlKind, EffectKind, EffectName, LaneName, SendBus, SoundName, TrackRef,
    VocalInput,
};

/// The section `history`.
///
/// What the screen keeps of the rows: a row is fixed by its `index` and `step`. Rows below
/// `total` never change except the last, which is another row when `newest` is not its
/// `step`; rows above `total` are gone. A snapshot fetched again and another
/// `SongView::serial` let go of every row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    /// How many steps there are, those that can be redone too ("14 手").
    pub total: u32,
    /// How many of them are undone and can be redone: the rows `current + 1 ..= total`.
    pub redo: u32,
    /// The row that is 「いまここ」: `total - redo`; 0 when every step is undone, or there is
    /// none.
    pub current: u32,
    /// `HistoryRow::step` of row `total`; 0 with no steps.
    pub newest: u64,
}

/// A row of the band: the reply of `history_rows(from, to)` is the rows `from ..= to` that
/// there are, oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HistoryRow {
    /// From 1, the oldest step ("11 手目").
    pub index: u32,
    /// Names the step for as long as the history holds it as it is; `Command::HistoryJump`
    /// takes it. Never 0.
    pub step: u64,
    pub name: StepName,
    pub what: StepWhat,
    /// `None`: the row names no place (the steps of the recovery file, mock F-Recover2).
    pub place: Option<StepPlace>,
}

/// 「（手の名前）」: which name, and for the names that are made of others the values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StepName {
    WriteNote,
    DeleteNotes,
    CutNotes,
    MoveNotes,
    DuplicateNotes,
    ResizeNotes,
    ChangeVelocity,
    PasteNotes,
    Quantize,
    Unquantize,
    AddControl,
    MoveControls,
    DeleteControls,
    Record,
    SwitchTake,
    DeleteTake,
    MoveClips,
    DuplicateClips,
    CutClips,
    PasteClips,
    TrimClip,
    DeleteClips,
    AddTrack,
    DeleteTrack,
    RenameTrack,
    MoveTrack,
    SetTrackColor,
    SetInstrument,
    RemoveInstrument,
    RemoveMissing,
    AddEffect,
    RemoveEffect,
    MoveInsert,
    ReplaceEffect,
    BypassEffect,
    RestoreEffect,
    /// "コンプレッサーの「しきい値」を変える": the screen has both names.
    SetParam {
        effect: EffectKind,
        param: u32,
    },
    /// "Surge XT の「A Filter 1 Cutoff」を変える". `param`: the parameter's name as the plugin
    /// said it when the step was made; `None` when Rust does not have it.
    SetPluginParam {
        plugin: String,
        param: Option<String>,
    },
    SetGain,
    SetPan,
    SetSend,
    Mute,
    Unmute,
    /// "Surge XT の窓で変えた所" (spec 6).
    PluginWindow {
        plugin: String,
    },
    AddLane,
    DeleteLane,
    AddAutoPoint,
    MoveAutoPoints,
    DeleteAutoPoints,
    WriteAutomation,
    RestLane,
    RestoreLane,
    /// The tempo of the head or of a point.
    SetTempo,
    /// The meter of the head or of a change.
    SetMeter,
    /// A tempo point or a meter change added.
    AddTimelineChange,
    MoveTimelineChange,
    DeleteTimelineChange,
    SetTempoRamp,
    ClearTempoRamp,
    AddMarker,
    RenameMarker,
    MoveMarker,
    DeleteMarker,
    SetLoop,
    Recovered,
    DiscardRecovered,
    /// `Command::SetVocalInput`.
    SetVocalInput,
}

impl StepName {
    /// One of every kind, with sample values where a kind has any: what `ui/src/text/ja.ts`
    /// must have a name for.
    pub fn every_kind() -> Vec<StepName> {
        use StepName::*;
        let all = vec![
            WriteNote,
            DeleteNotes,
            CutNotes,
            MoveNotes,
            DuplicateNotes,
            ResizeNotes,
            ChangeVelocity,
            PasteNotes,
            Quantize,
            Unquantize,
            AddControl,
            MoveControls,
            DeleteControls,
            Record,
            SwitchTake,
            DeleteTake,
            MoveClips,
            DuplicateClips,
            CutClips,
            PasteClips,
            TrimClip,
            DeleteClips,
            AddTrack,
            DeleteTrack,
            RenameTrack,
            MoveTrack,
            SetTrackColor,
            SetInstrument,
            RemoveInstrument,
            RemoveMissing,
            AddEffect,
            RemoveEffect,
            MoveInsert,
            ReplaceEffect,
            BypassEffect,
            RestoreEffect,
            SetParam {
                effect: EffectKind::Compressor,
                param: 0,
            },
            SetPluginParam {
                plugin: "Surge XT".to_owned(),
                param: Some("A Filter 1 Cutoff".to_owned()),
            },
            SetGain,
            SetPan,
            SetSend,
            Mute,
            Unmute,
            PluginWindow {
                plugin: "Surge XT".to_owned(),
            },
            AddLane,
            DeleteLane,
            AddAutoPoint,
            MoveAutoPoints,
            DeleteAutoPoints,
            WriteAutomation,
            RestLane,
            RestoreLane,
            SetTempo,
            SetMeter,
            AddTimelineChange,
            MoveTimelineChange,
            DeleteTimelineChange,
            SetTempoRamp,
            ClearTempoRamp,
            AddMarker,
            RenameMarker,
            MoveMarker,
            DeleteMarker,
            SetLoop,
            Recovered,
            DiscardRecovered,
            SetVocalInput,
        ];
        // A kind added to the enum and not to the list above does not build.
        for name in &all {
            match name {
                WriteNote
                | DeleteNotes
                | CutNotes
                | MoveNotes
                | DuplicateNotes
                | ResizeNotes
                | ChangeVelocity
                | PasteNotes
                | Quantize
                | Unquantize
                | AddControl
                | MoveControls
                | DeleteControls
                | Record
                | SwitchTake
                | DeleteTake
                | MoveClips
                | DuplicateClips
                | CutClips
                | PasteClips
                | TrimClip
                | DeleteClips
                | AddTrack
                | DeleteTrack
                | RenameTrack
                | MoveTrack
                | SetTrackColor
                | SetInstrument
                | RemoveInstrument
                | RemoveMissing
                | AddEffect
                | RemoveEffect
                | MoveInsert
                | ReplaceEffect
                | BypassEffect
                | RestoreEffect
                | SetParam { .. }
                | SetPluginParam { .. }
                | SetGain
                | SetPan
                | SetSend
                | Mute
                | Unmute
                | PluginWindow { .. }
                | AddLane
                | DeleteLane
                | AddAutoPoint
                | MoveAutoPoints
                | DeleteAutoPoints
                | WriteAutomation
                | RestLane
                | RestoreLane
                | SetTempo
                | SetMeter
                | AddTimelineChange
                | MoveTimelineChange
                | DeleteTimelineChange
                | SetTempoRamp
                | ClearTempoRamp
                | AddMarker
                | RenameMarker
                | MoveMarker
                | DeleteMarker
                | SetLoop
                | Recovered
                | DiscardRecovered
                | SetVocalInput => {}
            }
        }
        all
    }
}

/// 「（何を）」: the values the screen writes after the name ("3 ノート", "サビ", "4 パッド
/// −10.5 → −12.0 dB", "5 小節目 96 → 120"; mock R10-History).
///
/// `bar` and `beat` are as shown (from 1). A level in dB is `None` for −∞.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StepWhat {
    /// Nothing follows the name.
    None,
    Notes {
        count: u32,
    },
    /// Points of a CC lane of the piano roll.
    Points {
        count: u32,
    },
    Clips {
        count: u32,
    },
    /// "テイク 1".
    Take {
        number: u32,
    },
    TakeSwitch {
        from: u32,
        to: u32,
    },
    Track {
        track: TrackRef,
    },
    /// A strip of the mixer (the M of a track or of a return).
    Strip {
        strip: StripName,
    },
    /// A place on the timeline, with nothing else to say of it (a ramp set or taken away).
    Place {
        bar: u32,
        beat: u32,
    },
    /// The name of a marker or of a clip.
    Name {
        name: String,
    },
    Renamed {
        from: String,
        to: String,
    },
    /// A tempo at a place: both for a changed value, `to_bpm` only for a point added,
    /// `from_bpm` only for one deleted.
    Tempo {
        bar: u32,
        beat: u32,
        from_bpm: Option<f64>,
        to_bpm: Option<f64>,
    },
    /// As `Tempo`, a meter at the head of a bar.
    Meter {
        bar: u32,
        from: Option<MeterValue>,
        to: Option<MeterValue>,
    },
    /// A tempo point or a meter change moved (`beat` and `to_beat` 1 for a meter change).
    Moved {
        bar: u32,
        beat: u32,
        to_bar: u32,
        to_beat: u32,
    },
    /// The loop's range, "5–9".
    Bars {
        span: BarSpan,
    },
    Gain {
        strip: StripName,
        from_db: Option<f32>,
        to_db: Option<f32>,
    },
    /// −100..=100.
    Pan {
        strip: StripName,
        from: i8,
        to: i8,
    },
    Send {
        track: TrackRef,
        bus: SendBus,
        from_db: Option<f32>,
        to_db: Option<f32>,
    },
    /// The values of `StepName::SetParam`, in the parameter's own unit.
    Param {
        from: f32,
        to: f32,
    },
    Effect {
        effect: EffectName,
    },
    Replaced {
        from: EffectName,
        to: EffectName,
    },
    /// The sound the track has after the step.
    Sound {
        sound: SoundName,
    },
    /// A lane, and for the steps on points how many.
    Lane {
        target: LaneName,
        points: Option<u32>,
    },
    /// 「曲から外す」: how many tracks got the sine wave and how many insert slots were
    /// emptied.
    Missing {
        tracks: u32,
        inserts: u32,
    },
    // Protocol 23 (spec 3 「履歴の行の「（何を）」」, 92-3).
    /// One note: "C4".
    Note {
        key: u8,
    },
    /// "1/16 ・ 12 ノート": the grid of the quantize, `division` steps to a whole note.
    Quantized {
        division: u16,
        triplet: bool,
        count: u32,
    },
    /// "3 ノート ・ 96 → 110" when every note had `from` before and has `to` now; "3 ノート"
    /// when they differ (`None`).
    Velocity {
        count: u32,
        from: Option<u8>,
        to: Option<u8>,
    },
    /// "サステイン ・ 4 点": points of one control. `control` is `None` when the step touched
    /// points of more than one.
    ControlPoints {
        control: Option<ControlKind>,
        count: u32,
    },
    /// "4 パッド ・ 色 3".
    Color {
        track: TrackRef,
        color: u8,
    },
    /// "1 ピアノ ・ グランドピアノ → オルガン".
    SoundReplaced {
        track: TrackRef,
        from: SoundName,
        to: SoundName,
    },
    /// "6 歌 ・ 入口 1 ・ モノ → 入口 2 ・ モノ" (`StepName::SetVocalInput`).
    VocalInput {
        track: TrackRef,
        from: VocalInput,
        to: VocalInput,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MeterValue {
    pub num: u8,
    pub den: u8,
}

/// A strip as a row names it. A track by its number and name when the step was made: it may
/// be gone by now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StripName {
    Track { track: TrackRef },
    Return { bus: SendBus },
    Master,
}

/// Where the step was made, the right column of a row ("ピアノロール", "ミキサー", "アレンジ",
/// "4 パッド"; mock R10-History).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StepPlace {
    Arrange,
    PianoRoll,
    Mixer,
    Track {
        track: TrackRef,
    },
    /// "5 Surge XT ・ プラグインの窓" (spec 6).
    PluginWindow {
        strip: StripName,
    },
}
