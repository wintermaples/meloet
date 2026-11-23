//! Vocal tracks, the audio input and the latency (protocol 23; docs/architecture.md
//! 22-5 to 22-7 and 22-15, spec 3 「オーナーさんがえらんだ形」 23-A, 24-C, 25-B, 31-C, 33-A).

use serde::{Deserialize, Serialize};

use crate::{SoundKnobsView, SoundName, TrackRef};

/// Which channels of the input a vocal track records (spec 3 「歌のトラックの見出し」): the
/// three rows of the list under 「入口 1 ・ モノ ▾」.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum VocalInput {
    /// 「入口 1 ・ モノ」, what a new vocal track has.
    #[default]
    Mono1,
    /// 「入口 2 ・ モノ」.
    Mono2,
    /// 「入口 1 ・ 2 ・ ステレオ」.
    Stereo12,
}

impl VocalInput {
    pub const ALL: [VocalInput; 3] = [VocalInput::Mono1, VocalInput::Mono2, VocalInput::Stereo12];
}

/// The two kinds of track (spec 3, 23-A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TrackKindView {
    /// Clips of notes; it sounds with `TrackView::sound`. Its ● is lit when it is
    /// `SongView::keys_target`.
    Keys,
    /// Clips of audio. `TrackView::sound`, `stand_in` and `drum_keys` say nothing of it: the
    /// header and the instrument slot of the mixer show the input instead.
    Vocal {
        input: VocalInput,
        /// The PC's input has the channels `input` names. False: 「入口なし ▾」 (gray.8), also
        /// with no input at all (`InputsView::chosen` is `None`).
        input_ready: bool,
        /// The ● (`Command::ToggleVocalArm`; selecting the track turns it on too). R records
        /// to every vocal track that has it, whichever track is selected (spec 3, 24-C).
        armed: bool,
        /// 「モニター」 (`Command::ToggleMonitor`). The voice goes to the output only while
        /// `armed` is on as well.
        monitor: bool,
    },
}

/// A vocal track that is being recorded (`TransportView::recording_vocals`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct VocalRecordingView {
    /// `TrackView::id`.
    pub track: u32,
    /// Where recording starts in the song.
    pub from: u64,
    /// The number the take of the current lap gets in its clip, from 1 ("歌 テイク 1").
    pub take: u32,
    /// The clip the takes go into, when the recording overlaps one already.
    pub clip: Option<u32>,
    /// The number the file being written gets in the song's audio table when the recording
    /// ends: `peaks` answers for it already, with `Peaks::growing`, so the clip in
    /// `color.rec` can draw the waveform as it comes.
    pub audio: u32,
    /// The rate the file is written at.
    pub sample_rate: u32,
    /// The frame of the file that was sung at `from`; `None` until the song rolls (during
    /// the count-in). In a loop: of the lap going on.
    pub first_frame: Option<u64>,
}

/// 「録る形」 of the advanced sound settings: the samples of the files a recording writes.
/// 32 bits are floats, the other two integers (docs/architecture.md 22-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum RecordFormat {
    Bits16,
    #[default]
    Bits24,
    Bits32,
}

impl RecordFormat {
    pub const ALL: [RecordFormat; 3] = [
        RecordFormat::Bits16,
        RecordFormat::Bits24,
        RecordFormat::Bits32,
    ];

    pub fn bits(self) -> u8 {
        match self {
            RecordFormat::Bits16 => 16,
            RecordFormat::Bits24 => 24,
            RecordFormat::Bits32 => 32,
        }
    }
}

/// 「遅れの合わせ」 (spec 3, 25-B and 33-A): how long a sound takes from the output back into
/// the input, which is what recorded audio is moved back by. Every number is that whole
/// round trip in ms, as the dialogs show it. It belongs to the PC, not to the song.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LatencyView {
    /// 「PC が言う数」; `None` while the audio is not running.
    pub reported_ms: Option<f32>,
    /// 「測った数」 for the output, the input and the sample rate in use; `None`: not
    /// measured yet.
    pub measured_ms: Option<f32>,
    /// 「手で決める…」; `None`: no number was typed.
    pub manual_ms: Option<f32>,
    /// Which of the three is used: the typed one when there is one, else the measured one,
    /// else the PC's.
    pub used: LatencySource,
    /// The number of `used`; `None` only while `reported_ms` is used and is `None`.
    pub used_ms: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum LatencySource {
    #[default]
    Reported,
    Measured,
    Manual,
}

/// The dialog 「遅れを測る」 (mock F7-Latency), `WorkspaceView::latency_dialog`. 「PC が言う数」
/// is `AudioView::latency`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LatencyDialogView {
    pub measure: LatencyMeasure,
}

/// How many clicks one measuring plays (spec 3, 25-B).
pub const LATENCY_CLICKS: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LatencyMeasure {
    /// Nothing measured since the dialog opened: 「測る」.
    Idle,
    /// "測っています 3 / 5": `done` clicks came back, of `of`. `mean_ms`: the mean of those
    /// ("12.4 ms（まだ 3 回の平均）"), `None` before the first. 「やめる（Esc）」 stops it.
    Measuring {
        done: u8,
        of: u8,
        mean_ms: Option<f32>,
    },
    /// All clicks came back: 「この数を使う」 can be pressed.
    Done { measured_ms: f32 },
    /// 「もう一度」.
    Failed { reason: LatencyFailure },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum LatencyFailure {
    /// "音が小さくて測れませんでした。ヘッドホンの音量を上げて、もう一度測ってください。"
    /// (spec 6 「遅れを測れない」).
    TooQuiet,
    /// The clicks came back at times too far apart to trust.
    Unsteady,
    /// There is no input, or the audio is not running.
    NoInput,
}

/// "録っていた歌 1 つを、止まる直前（9. 3. 0000 ごろ）まで残しました。" (spec 3 「録っている
/// 途中で PC が終わったとき」), a line more in the hint about the recovery file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeptVocals {
    /// How many recordings were kept, each a clip on its vocal track now.
    pub count: u32,
    /// The song position the latest of them reaches.
    pub until: u64,
}

/// A sound with no parameters in the parameters panel (spec 3, 19-A): one that comes with
/// meloet ("最初から入っている") or a preset of an SF2 file ("SF2"). The panel says "この音色
/// には、動かせるパラメータがありません。" and offers 「音源を入れかえる…」
/// (`Command::OpenShelf` with `ShelfMode::Replace`). From round 12 (E3) a sound that comes
/// with meloet has the four rows of `knobs` instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SelectedSound {
    /// `TrackView::id`.
    pub track: u32,
    /// `SoundName::Builtin` or `SoundName::Sf2`, never a plugin.
    pub sound: SoundName,
    /// E3 「明るさ」「立ち上がり」「のび」「ゆれ」 (`Command::SetSoundKnob`). `None` for an SF2
    /// preset: no parameters.
    pub knobs: Option<SoundKnobsView>,
}

/// A vocal track in a sentence of the status line: "6 歌（入口 1）".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct VocalRef {
    pub track: TrackRef,
    pub input: VocalInput,
}
