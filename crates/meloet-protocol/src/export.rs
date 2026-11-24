//! Writing the song out as an audio file (protocol 26; spec 3 「書き出し（34-B）」 and
//! 「書き出している間」, spec 2 「書き出しの進み」; mocks F7-Export, F7-Exporting, R11-Fit;
//! docs/architecture.md 22-11, 22-14, 22-20), or as a MIDI file (protocol 36; spec 3
//! "MIDI export", spec 7 A-31).

use serde::{Deserialize, Serialize};

use crate::{PluginSlotRef, RecordFormat, TickSpan, TrackRef};

/// 「しっぽ」 is written until the sound is gone, and at most this long after the range ends.
pub const EXPORT_TAIL_MAX_S: u32 = 10;

/// F1 「公開先に合わせる」: the loudness the whole file is brought to…
pub const EXPORT_MATCH_LUFS: f32 = -14.0;
/// …unless its true peak would go over this.
pub const EXPORT_MATCH_TRUE_PEAK_DB: f32 = -1.0;
/// F1: a song quieter than this (or whose loudness cannot be measured) cannot be matched.
pub const EXPORT_MATCH_QUIETEST_LUFS: f32 = -70.0;

/// E9: how many bars `ExportRunView::Done::wave` has.
pub const EXPORT_WAVE_BARS: usize = 200;

/// F2 「ビットレート」 of mp3, kbit/s, always constant, in the order of the list…
pub const EXPORT_MP3_KBPS: [u16; 4] = [128, 192, 256, 320];
/// …and the one chosen at first.
pub const EXPORT_MP3_DEFAULT_KBPS: u16 = 192;
/// F2: the sample rates an mp3 is written at.
pub const EXPORT_MP3_SAMPLE_RATES: [u32; 2] = [44_100, 48_000];
/// F2 「絵」 of 「曲の情報」: a png or a jpg of at most this many bytes (5 MiB).
pub const EXPORT_PICTURE_MAX_BYTES: u64 = 5 * 1024 * 1024;

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
    /// How many files 「トラックごと」 would write; 0: the choice is unusable. Under
    /// `ExportFormat::Midi` the length of `midi_taken` (「トラックごと ・ 3 つのファイル」).
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
    /// 「書き出す」 can be pressed. Under `ExportFormat::Midi` the check is not waited for
    /// (nothing is measured), and it is false while `midi_problem` is `Some`.
    pub can_export: bool,
    /// F1 「音の大きさ」. Kept as long as the format is.
    pub loudness: ExportLoudness,
    /// F1 「公開先に合わせる」 can be chosen: not for 「トラックごと」, nor for a song whose
    /// loudness cannot be measured or is below `EXPORT_MATCH_QUIETEST_LUFS`. While it cannot,
    /// what is written is as with `ExportLoudness::AsIs`.
    pub can_match: bool,
    /// F1 the line under the check: what 「公開先に合わせる」 does to this export. `Some` only
    /// once the check is `Done` and `can_match`.
    pub loudness_result: Option<ExportLoudnessResult>,
    /// F2 「ビットレート」 (in place of 「ビット」 while `format` is mp3), one of
    /// `mp3_kbps_choices`.
    pub mp3_kbps: u16,
    /// F2: `EXPORT_MP3_KBPS`, or empty while mp3 cannot be written.
    pub mp3_kbps_choices: Vec<u16>,
    /// F2 「曲の情報」 「題名」: the song's name at first. For 「トラックごと」 each file is
    /// titled 「題名 - トラックの名前」.
    pub title: String,
    /// F2 「作者」, kept with the PC's settings.
    pub artist: String,
    /// F2 「絵」: the name of the picture chosen (without its folder); Rust keeps the path.
    pub picture: Option<String>,
    /// F2 the row 「絵」 can be used: not for wav, which holds no picture, nor for MIDI.
    pub picture_usable: bool,
    /// 「入れるトラック」, shown only under `ExportFormat::Midi`. Kept while meloet runs.
    pub midi_tracks: ExportMidiTracks,
    /// The text of the number field of 「番号で」 as Rust holds it: what was sent last
    /// (`ExportChange::MidiNumbers`). Empty: the field shows 「1-3, 5」 in gray as the hint.
    /// Kept while meloet runs, whatever `midi_tracks` is.
    pub midi_numbers: String,
    /// 「選んでいるトラック」 can be chosen: the selected track (`SongView::play_target`) is a
    /// keys track.
    pub midi_selected_usable: bool,
    /// The tracks `midi_tracks` takes, in the order of the tracks: 「1 ピアノ ・ 2 ベース
    /// ほか 2 本」 (two names at most). Never a vocal track. For `Numbers` also while
    /// `midi_problem` is `NoSuchTrack` (the numbers that name keys tracks); empty while it is
    /// `Unreadable`.
    pub midi_taken: Vec<ExportMidiTrack>,
    /// Only for `ExportMidiTracks::Numbers`: the vocal tracks the numbers name, in the order
    /// of the tracks. Each says in gray 「3 ボーカル は声のトラックなので入りません」; the
    /// others are still written.
    pub midi_vocal: Vec<ExportMidiTrack>,
    /// Why the choice of 「入れるトラック」 cannot be written, in `color.rec`. While `Some`,
    /// `can_export` is false. Only under `ExportFormat::Midi`.
    pub midi_problem: Option<ExportMidiProblem>,
}

/// F1 「音の大きさ」.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ExportLoudness {
    /// 「そのまま」: the audio as it plays.
    #[default]
    AsIs,
    /// 「公開先に合わせる」: one gain for the whole file, to `EXPORT_MATCH_LUFS`, or less so
    /// that the true peak stays at `EXPORT_MATCH_TRUE_PEAK_DB`. No limiter.
    Match,
}

/// F1: "合わせると 4.6 dB 大きくなります", or at the true-peak limit "−1 dBTP を越えないよう、
/// −15.2 LUFS になります".
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ExportLoudnessResult {
    /// The gain, + louder, − quieter.
    pub gain_db: f32,
    /// The gain stopped short of `EXPORT_MATCH_LUFS` at the true-peak limit.
    pub limited: bool,
    /// The loudness of the file with the gain.
    pub lufs: f32,
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
    /// F2 (protocol 32): stereo, a constant bitrate (`ExportDialogView::mp3_kbps`), at one of
    /// `EXPORT_MP3_SAMPLE_RATES`. Not in `ExportDialogView::formats` while this meloet
    /// cannot write it.
    Mp3,
    /// Protocol 36 「MIDI」: a standard MIDI file of the keys tracks
    /// (`ExportDialogView::midi_tracks`), not audio. Always in `ExportDialogView::formats`,
    /// last. Nothing is measured, and the rows only for audio are not shown: 「音の大きさ」,
    /// 「書き出す前のたしかめ」, 「くわしく」 and 「曲の情報」.
    Midi,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 4] = [
        ExportFormat::Wav,
        ExportFormat::Flac,
        ExportFormat::Mp3,
        ExportFormat::Midi,
    ];

    /// The ending of the file's name.
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Wav => "wav",
            ExportFormat::Flac => "flac",
            ExportFormat::Mp3 => "mp3",
            ExportFormat::Midi => "mid",
        }
    }

    /// Empty for mp3, which has a bitrate instead, and for MIDI.
    pub fn bits_choices(self) -> &'static [RecordFormat] {
        match self {
            ExportFormat::Wav => &RecordFormat::ALL,
            ExportFormat::Flac => &[RecordFormat::Bits16, RecordFormat::Bits24],
            ExportFormat::Mp3 | ExportFormat::Midi => &[],
        }
    }
}

/// 「入れるトラック」 of a MIDI export. Vocal tracks hold no notes and are never in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ExportMidiTracks {
    /// 「ぜんぶ」: every keys track; vocal tracks are passed by without a word.
    #[default]
    All,
    /// 「選んでいるトラック」: the one selected track, while it is a keys track
    /// (`ExportDialogView::midi_selected_usable`).
    Selected,
    /// 「番号で」: the tracks `ExportDialogView::midi_numbers` names, as 「1-3, 5」: numbers
    /// and ranges split by commas, spaces not counted; a range written backwards (「5-3」) is
    /// read as 3 to 5.
    Numbers,
}

/// A track a MIDI export takes, or one of `ExportDialogView::midi_vocal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ExportMidiTrack {
    /// From 1, as the track heads show it (`TrackView::number`).
    pub number: u32,
    pub name: String,
}

/// Why 「入れるトラック」 cannot be written (spec 7 A-31), in `color.rec`; 「書き出す」 is
/// disabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ExportMidiProblem {
    /// 「8 番のトラックはありません」: the smallest number the text names that is no
    /// track's.
    NoSuchTrack { number: u32 },
    /// 「番号は「1-3, 5」のように書きます」: the text cannot be read as numbers.
    Unreadable,
    /// 「入れるトラックがありません」: the choice takes no track (also a song with only
    /// vocal tracks, and an empty number field).
    NoTrack,
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
        /// F1, as in `Done`, of what is written so far.
        lufs: Option<f32>,
        true_peak_db: Option<f32>,
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
        /// F1 「曲ぜんたいの大きさ −18.6 LUFS」: integrated loudness (ITU-R BS.1770) of the
        /// range, the tail not counted. `None` when it cannot be measured (silence).
        lufs: Option<f32>,
        /// F1 「トゥルーピーク −3.2 dBTP」: the tail counted. `None` for silence.
        true_peak_db: Option<f32>,
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

/// 「書き出しの進み」 (spec 2; spec 6 「書き出している」 … 「書き出しが終わった」). An export the
/// AI link asks for is not shown here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// (`Command::ShowExported`). A MIDI export (`midi_tracks` is `Some`) says
    /// "はじめての曲.mid を書き出しました（MIDI ・ 3 トラック）。" and opens no dialog
    /// 「書き出しました」; its `sample_rate` and `bits` are the dialog's and are not shown.
    Done {
        /// The file written; of several, the first.
        file: String,
        /// How many files were written: more than 1 only for `ExportRange::Tracks`.
        files: u32,
        /// The file as it is shown, with its folder; for several files their folder.
        place: String,
        /// How long the written audio is, the tail counted. For MIDI the range's length.
        length_s: u32,
        sample_rate: u32,
        bits: RecordFormat,
        /// E9 the dialog 「書き出しました」: the file's waveform as `EXPORT_WAVE_BARS` bars, each
        /// the largest absolute sample of its part, 0..=255. Empty for 「トラックごと」
        /// (`ExportRange::Tracks`), however many files that wrote (one track too), and for
        /// MIDI.
        wave: Vec<u8>,
        /// E9 the loudness and the true peak of the file as written (after F1's gain).
        /// `None` for several files, for silence, and for MIDI.
        lufs: Option<f32>,
        true_peak_db: Option<f32>,
        /// F2: an mp3 was written at this bitrate (E9 shows it in place of `bits`).
        mp3_kbps: Option<u16>,
        /// Protocol 36: MIDI was written, this many of the song's tracks (the first track of
        /// the tempos and the meters not counted). Of one file the tracks in it:
        /// "（MIDI ・ 3 トラック）"; of 「トラックごと」 one in each file, as many as `files`:
        /// "3 つのファイルを書き出しました（MIDI）。" `None`: audio.
        midi_tracks: Option<u32>,
    },
    /// "書き出しをやめました。途中までのファイルは消しました。"
    Cancelled,
    /// Nothing was written; what was is taken away again.
    Failed { reason: ExportFailure },
}

/// What `Command::SetExport` sets. None is a step: the song does not change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// become 24 (mp3 and MIDI have none, and keep the ones chosen for the others).
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
    /// F1 「そのまま」 / 「公開先に合わせる」. `Match` while not `ExportDialogView::can_match`
    /// does nothing.
    Loudness {
        loudness: ExportLoudness,
    },
    /// F2 one of `ExportDialogView::mp3_kbps_choices`.
    Mp3Kbps {
        kbps: u16,
    },
    /// F2 「題名」 as typed.
    Title {
        title: String,
    },
    /// F2 「作者」 as typed; kept with the PC's settings.
    Artist {
        artist: String,
    },
    /// F2 「えらぶ…」 of 「絵」: Rust shows the PC's window for a png or a jpg. Cancelling it
    /// changes nothing; a picture that cannot be used says so
    /// (`StatusMessage::ExportPictureTooLarge`, `ExportPictureUnreadable`).
    ChoosePicture,
    /// F2: no picture.
    RemovePicture,
    /// Protocol 36 「入れるトラック」. `Selected` while not
    /// `ExportDialogView::midi_selected_usable` does nothing.
    MidiTracks {
        tracks: ExportMidiTracks,
    },
    /// Protocol 36: the number field of 「番号で」 as typed, sent when the field is left or
    /// Enter is pressed. Kept whatever `ExportDialogView::midi_tracks` is.
    MidiNumbers {
        text: String,
    },
}
