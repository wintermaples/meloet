//! Writing the song out as an audio file (protocol 26; spec 3 「書き出し（34-B）」 and
//! 「書き出している間」, spec 2 「書き出しの進み」; mocks F7-Export, F7-Exporting, R11-Fit;
//! docs/architecture.md 22-11, 22-14, 22-20).

use serde::{Deserialize, Serialize};

use crate::{PluginSlotRef, RecordFormat, TickSpan, TrackRef};

/// 「しっぽ」 is written until the sound is gone, and at most this long after the range ends.
pub const EXPORT_TAIL_MAX_S: u32 = 10;

/// `AppState::export`. Nothing of it changes 60 times a second: the numbers of the check and
/// of the run come a few times a second at most, as `AudioWorkView` does.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ExportView {
    /// The dialog 「書き出し」. `None` again once 「書き出す」 was pressed.
    pub dialog: Option<ExportDialogView>,
    /// 「書き出しの進み」 at the left of the status line: from 「書き出す」 on, and after the
    /// end until the person does the next thing.
    pub run: Option<ExportRunView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ExportDialogView {
    pub range: ExportRange,
    /// What 「ループ 5〜9」 would write; `None`: the song has no loop, the choice is unusable.
    pub loop_range: Option<TickSpan>,
    /// How many files 「トラックごと」 would write; 0: the choice is unusable.
    pub tracks: u32,
    /// What `range` writes, before the tail: "曲ぜんぶ ・ 1. 1. 0000 〜 17. 1. 0000 と、音が
    /// 消えるまで". The whole song ends where its last clip ends.
    pub span: TickSpan,
    pub format: ExportFormat,
    /// What 「形」 lists, in its order. A format this meloet cannot write is not in it.
    pub formats: Vec<ExportFormat>,
    /// 「保存する所」 as it is shown: "~/音楽/はじめての曲.wav". For 「トラックごと」 the
    /// folder the files go into. Rust keeps the path itself.
    pub place: String,
    pub check: ExportCheck,
    /// Tracks are soloed now (spec 3, 42-C): the line of the check says "いま 2 ベース を
    /// ソロにしています。書き出しには入れません（曲ぜんぶを書き出します）。" The solo is not in
    /// what is written. `None`: no track is soloed.
    pub solo: Option<ExportSolo>,
    /// 「止まっているプラグイン」: they are passed by in the written audio. Empty:
    /// 「止まっているプラグインはありません」.
    pub stopped_plugins: Vec<ExportStoppedPlugin>,
    /// 「くわしく」 is unfolded.
    pub advanced_open: bool,
    pub sample_rate: u32,
    pub sample_rate_choices: Vec<u32>,
    pub bits: RecordFormat,
    /// What `format` can hold: 32 bits only as wav.
    pub bits_choices: Vec<RecordFormat>,
    pub tail: ExportTail,
    /// The audio is written as fast as it plays and no faster: the person's choice, or
    /// `realtime_forced`.
    pub realtime: bool,
    /// A plugin of the song says it cannot run faster than it plays: `realtime` stays on.
    pub realtime_forced: bool,
    /// 「書き出す」 can be pressed.
    pub can_export: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ExportRange {
    /// 「曲ぜんぶ」.
    #[default]
    Song,
    /// 「ループ 5〜9」.
    Loop,
    /// 「トラックごと」: a file for each track, named as the track.
    Tracks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    #[default]
    Wav,
    Flac,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 2] = [ExportFormat::Wav, ExportFormat::Flac];

    /// The ending of the file's name.
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Wav => "wav",
            ExportFormat::Flac => "flac",
        }
    }

    pub fn bits_choices(self) -> &'static [RecordFormat] {
        match self {
            ExportFormat::Wav => &RecordFormat::ALL,
            ExportFormat::Flac => &[RecordFormat::Bits16, RecordFormat::Bits24],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ExportTail {
    /// 「しっぽは音が消えるまで（最大 10 秒）」.
    #[default]
    UntilSilent,
    /// The file ends where the range ends.
    Off,
}

/// 「書き出す前のたしかめ」. Rust starts writing the range when the dialog opens and tells
/// what it has seen so far; a change of the range, the tail or `realtime` starts it anew, one
/// of the format, the sample rate or the bits does not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ExportCheck {
    /// Nothing is written yet: the song's audio is still being made, or its plugins are
    /// still being started.
    Waiting,
    /// `percent` (0..=99) of the range is written, and these are the numbers of that much.
    Counting {
        percent: u8,
        peak_db: Option<f32>,
        clips: bool,
        over: u32,
    },
    /// 「いちばん大きい所 −0.6 dB ・ 音割れなし」. `peak_db`: `None` for silence. `clips`: the
    /// file as it would be written with `bits` clips (spec 6 「書き出しで音が割れる」; the
    /// button is then 「このまま書き出す」). `over` (here and in `Counting`, protocol 27, spec
    /// 3 41-A): how many places of what was written go over 0 dB, counted as docs/history/design-notes.ja.md
    /// B-95-2 says (a run of samples over 0 dB is one place; 100 ms not over ends it):
    /// "0 dB をこえる所が 3 か所あります（いちばん大きい所 +1.2 dB）。音が割れます。" The
    /// highest of those places is `peak_db`. Counted whatever `bits` is; at 32 bits `clips`
    /// stays false.
    Done {
        peak_db: Option<f32>,
        clips: bool,
        over: u32,
    },
    /// The range cannot be written: `can_export` is false.
    Failed { reason: ExportFailure },
}

/// The soloed tracks, for the line of the check: "2 ベース", "2 ベース ほか 1 本".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ExportSolo {
    /// The first soloed track, in the order of the tracks.
    pub first: TrackRef,
    /// How many more are soloed.
    pub others: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ExportStoppedPlugin {
    pub name: String,
    /// The screen names the strip from it, as for `StatusMessage::PluginStopped`.
    pub slot: PluginSlotRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ExportFailure {
    /// A plugin did not load for the export, or answered no more while it ran.
    Plugin { name: String },
    /// The file cannot be written: the disk is full, or the place does not take it.
    CannotWrite,
}

/// 「書き出しの進み」 (spec 2; spec 6 「書き出している」 … 「書き出しが終わった」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ExportRunView {
    /// "書き出しています 46 % ・ はじめての曲.wav ・ 残り 約 40 秒 ・ 弾けます（書き出しの音は
    /// 聞こえません）", with 「やめる」 (`Command::CancelExport`).
    Running {
        /// 0..=99.
        percent: u8,
        /// The file being written, without its folder.
        file: String,
        /// `None` until there is something to guess from.
        remaining_s: Option<u32>,
        /// Written as fast as it plays: "実時間で書き出しています（3:02）。"
        realtime: bool,
        /// A plugin cannot be run faster than it plays (then `realtime` is true whatever was
        /// chosen): "速く書き出せないプラグインがあるので、実時間で書き出します（3:02）。"
        realtime_forced: bool,
        /// How long the audio being written is, the tail not counted: "3:02".
        length_s: u32,
    },
    /// "はじめての曲.wav を書き出しました（3:02 ・ 48000 Hz ・ 24 ビット）。" and "書き出し
    /// ました: ~/ミュージック/はじめての曲/はじめての曲.wav", with 「フォルダをひらく」
    /// (`Command::ShowExported`).
    Done {
        /// The file written; of several, the first.
        file: String,
        /// How many files were written: more than 1 only for `ExportRange::Tracks`.
        files: u32,
        /// The file as it is shown, with its folder; for several files their folder.
        place: String,
        /// How long the written audio is, the tail counted.
        length_s: u32,
        sample_rate: u32,
        bits: RecordFormat,
    },
    /// "書き出しをやめました。途中までのファイルは消しました。"
    Cancelled,
    /// Nothing was written; what was is taken away again.
    Failed { reason: ExportFailure },
}

/// What `Command::SetExport` sets. None is a step: the song does not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ExportChange {
    /// A choice that is unusable does nothing.
    Range {
        range: ExportRange,
    },
    /// The ending of the name in 「保存する所」 follows; bits the new format cannot hold
    /// become 24.
    Format {
        format: ExportFormat,
    },
    /// 「くわしく ▸」.
    AdvancedOpen {
        open: bool,
    },
    /// One of `ExportDialogView::sample_rate_choices`.
    SampleRate {
        hz: u32,
    },
    /// One of `ExportDialogView::bits_choices`.
    Bits {
        bits: RecordFormat,
    },
    Tail {
        tail: ExportTail,
    },
    /// Does nothing while `ExportDialogView::realtime_forced`.
    Realtime {
        on: bool,
    },
}
