//! Vocal clips, audio files and their waveforms (round 7, protocols 24 and 25;
//! docs/architecture.md 22-4, 22-8 to 22-10, 22-17 and 22-19, spec 3 26-A, 27-A, 28-A, 29-A,
//! 30-C).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::timeline::TempoPointView;

/// The range of a vocal clip's volume in dB (spec 3 「クリップの音量」): there is no −∞.
pub const CLIP_GAIN_MIN_DB: f32 = -60.0;
pub const CLIP_GAIN_MAX_DB: f32 = 12.0;

/// The ranges of 「音の高さ」 (spec 3, Y22): −12..=12 semitones and −50..=50 cents.
pub const CLIP_SEMITONES_MAX: i8 = 12;
pub const CLIP_CENTS_MAX: i8 = 50;
/// The range of a typed 「この音のテンポ」, in steps of 0.01: that of a tempo of the song.
pub const SOURCE_BPM_MIN: f64 = 30.0;
pub const SOURCE_BPM_MAX: f64 = 300.0;

/// The endings of the files that read as audio (spec 3 「読めるファイル」), as the status line
/// lists them: "読めるのは wav ・ mp3 ・ flac ・ ogg".
pub const AUDIO_EXTENSIONS: [&str; 4] = ["wav", "mp3", "flac", "ogg"];

/// What a clip of a vocal track has besides `ClipView` (`ClipView::audio`).
///
/// Where the waveform goes: the take's tick 0 is at `ClipView::start + take_offset` in the
/// song, and there the file is at `first_frame`. From there on (`meloet_model::FileClock`;
/// the screen's copy is `ui/src/components/arrange/follow.ts`):
///
/// - `source_tempo` is `None`: the audio plays at the file's own speed. The frame of the
///   file at a song tick `t` is `first_frame + (seconds(t) − seconds(tick 0 of the take)) ×
///   sample_rate`, with the screen's tempo map of the song.
/// - `source_tempo` is `Some`: the clip follows the song's tempo, its audio stuck to the
///   song's ticks. The frame is `first_frame + source_seconds(t − tick 0 of the take) ×
///   sample_rate`, where `source_seconds` is of a tempo map made from `source_tempo` alone
///   (ticks of the take from its tick 0; before it the head's tempo goes on backwards). The
///   song's tempo is not in it.
///
/// Frames before 0 and from `frames` on are silence. The waveform is always that of the
/// file, put where the audio is heard; while the stretched audio is being made
/// (`AudioJobKind::Rendering`) the clip still sounds unstretched, and the waveform is where
/// it will sound.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AudioClipView {
    /// The audio file of the shown take, by its number in the song's audio table: what
    /// `peaks` takes. A file never changes, so the screen keeps its peaks by this and
    /// `SongView::serial`.
    pub audio: u32,
    /// The file's name, without its folder: "歌 テイク 1.wav".
    pub file: String,
    /// Of the file, as it is on the disk.
    pub sample_rate: u32,
    /// 16, 24 or 32 ("24 ビット"); `None` for a file whose kind has no such number (mp3,
    /// ogg): the row leaves it out.
    pub bits: Option<u8>,
    /// 1 or 2.
    pub channels: u8,
    /// The file's rate is not the one the audio runs at: "44100 Hz（曲に合わせて鳴らします）".
    pub resampled: bool,
    /// The frame of the file at the take's tick 0, in frames of `sample_rate`.
    pub first_frame: u64,
    /// How many frames the take has from there.
    pub frames: u64,
    /// The fades, in ticks from the clip's start and before its end. Together never longer
    /// than the clip. The shape is the one of `ui/src/components/arrange/fade.ts`.
    pub fade_in: u64,
    pub fade_out: u64,
    /// `CLIP_GAIN_MIN_DB..=CLIP_GAIN_MAX_DB`, in steps of 0.01.
    pub gain_db: f32,
    pub state: AudioClipState,
    pub stretch: StretchView,
    /// `Some` exactly when the clip follows the song's tempo: `stretch.follow` is on and
    /// the take has a tempo. The tempo the audio was played at, as tempo points from the
    /// take's tick 0, for placing the waveform.
    pub source_tempo: Option<SourceTempoView>,
}

/// The tempo the audio of a take was played or sung at: `meloet_model::SourceTempo`. The
/// fields are named as those of `TimelineView`, so that the screen makes a tempo map of it
/// as it does of the song (with any meter and no meter changes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourceTempoView {
    /// At the take's tick 0.
    pub tempo_bpm: f64,
    /// The tempo goes in a straight line from tick 0 to the first point.
    pub head_ramp: bool,
    /// Ticks from the take's tick 0, in order, each 1 or more. None for a file brought in
    /// whose tempo was typed or found; recorded audio has the song's points as they were.
    pub tempo_points: Vec<TempoPointView>,
}

/// Whether the clip sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum AudioClipState {
    #[default]
    Ready,
    /// The file is being read (decoded, brought to the rate the audio runs at): the clip is
    /// there and silent, with "読みこんでいます 40 %" under it. How far is in
    /// `AudioWorkView::jobs`. Its peaks may not be there yet.
    Loading,
    /// 「音のファイルが見つかりません」: gray.2, the line on top gray.7, no waveform, silent.
    Missing,
}

/// The group 「のび縮み ・ 音の高さ」 of the panel 「選んだクリップ」, as the song has it (spec
/// 3, 28-A and Y22), and the marks beside the clip's name. `Command::SetClipStretch` and
/// `Command::FindSourceTempo` change it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StretchView {
    /// 「曲のテンポについていく」: on for recorded audio, off for a file brought in. On with
    /// no `source_bpm` the clip plays at the file's speed all the same.
    pub follow: bool,
    /// 「この音のテンポ」 at the head of the take; `None`: 「—」, with 「さがす」.
    pub source_bpm: Option<f64>,
    /// The mark "×1.25": the song's tempo over the tempo of the audio, both at the clip's
    /// start (inside a ramp of either it is another number further on). `None` for a clip
    /// that does not follow. Not rounded: the screen shows two places and no mark for what
    /// shows as 1.00. Rust sends the song again when a change of the song's tempo or of the
    /// clip's place makes it another number.
    pub ratio: Option<f64>,
    /// −12..=12.
    pub semitones: i8,
    /// −50..=50.
    pub cents: i8,
    /// 「声の感じを保つ」.
    pub keep_formant: bool,
}

/// The parameters panel shows 「選んだクリップ」 (spec 3, 93-4): exactly one clip is selected
/// and it is a vocal clip. Its name, position, length, volume and fades are in `song`
/// (`ClipView`, `AudioClipView`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SelectedClip {
    /// `TrackView::id`.
    pub track: u32,
    /// `ClipView::id`.
    pub clip: u32,
    /// The clip's file is not found (`AudioClipState::Missing`): the notice with 「さがす…」
    /// (`Command::FindAudio`) and 「曲から外す」 (`Command::RemoveMissing`) in place of the
    /// groups.
    pub missing: Option<MissingAudioView>,
}

/// The panel 「選んだクリップ」 for two or more vocal clips (spec 3, 40-A; `ParamsView::clips`):
/// "3 クリップを選んでいます", the names of their tracks, and the group 「まとめて変える」.
/// A value is `None` when the clips do not all have the same one: the field shows 「—」.
/// Typing into it gives every clip the value (`Command::SetClipGains`,
/// `Command::SetClipsStretch`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SelectedClips {
    /// `ClipView::id`, in the order of `ArrangeView::selected_clips`; 2 or more.
    pub clips: Vec<u32>,
    /// `TrackView::id` of their tracks, each once, in the order of the tracks: "6 歌 ・ 7 歌 2".
    pub tracks: Vec<u32>,
    /// 「音量」, `AudioClipView::gain_db`.
    pub gain_db: Option<f32>,
    /// 「曲のテンポについていく」, `StretchView::follow`.
    pub follow: Option<bool>,
    /// 「音の高さ」, `StretchView::semitones` and `cents`, each on its own.
    pub semitones: Option<i8>,
    pub cents: Option<i8>,
    /// 「声の感じを保つ」, `StretchView::keep_formant`.
    pub keep_formant: Option<bool>,
}

/// "歌 テイク 1.wav が見つかりません。前はここにありました: はじめての曲/音/歌 テイク 1.wav"
/// (spec 6 「音のファイルが見つからない（欄）」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MissingAudioView {
    /// The file's number in the song's audio table.
    pub audio: u32,
    pub file: String,
    /// Where it was looked for, as shown: the song's folder, its audio folder and the name.
    /// `None` for a song that was never saved.
    pub was_at: Option<String>,
}

/// The section `audioWork`: audio that is being made ready, apart from `song` so that a
/// percent does not send the song again. Rust sends a job's `percent` a few times a second
/// at most.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkView {
    /// In the order they were started. A job is gone when it is done.
    pub jobs: Vec<AudioJob>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AudioJob {
    pub kind: AudioJobKind,
    /// The file's number in the song's audio table: every clip that plays it shows the job.
    pub audio: u32,
    /// `Rendering`: the one clip the audio is made for; only that clip shows the job (under
    /// it, as the bar on top of the panel 「選んだクリップ」 when it is the selected one, and
    /// in the status line, spec 6 「音を作っている」). `None` for `Loading`.
    pub clip: Option<u32>,
    /// 0..=99.
    pub percent: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum AudioJobKind {
    /// "読みこんでいます 40 %" (spec 6 「音のファイルを読みこんでいる」).
    Loading,
    /// "音を作っています 34 %" (spec 6 「音を作っている」): the stretched or pitched audio of
    /// one clip. Until it is done the clip sounds as the file is; a clip whose audio would
    /// come out as the file is has no such job.
    Rendering,
}

/// What `Command::SetClipField` sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ClipField {
    /// 「位置」: where the clip starts, in ticks. The clip moves as it is.
    Start,
    /// 「長さ」, in ticks: the clip's end moves.
    Length,
    /// 「音量」 and the volume line, in dB.
    GainDb,
    /// The fade at the clip's start (「前」) and at its end (「後」), in ticks.
    FadeIn,
    FadeOut,
}

/// What `Command::SetClipStretch` changes: one row of the group 「のび縮み ・ 音の高さ」. A
/// number outside its range is put into it; one that is not a number does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StretchChange {
    /// 「曲のテンポについていく」. With it on, the audio is stuck to the song's ticks and
    /// stretches with every change of the song's tempo, ramps too. A clip whose audio has
    /// no tempo (`StretchView::source_bpm` is `None`) keeps the switch and plays at the
    /// file's speed until it gets one.
    Follow { on: bool },
    /// 「この音のテンポ」, typed: quarter notes a minute, to the nearest 0.01 inside
    /// `SOURCE_BPM_MIN..=SOURCE_BPM_MAX`. The audio then has that one tempo all along. A
    /// take is never changed, so the clip shows a new take (a new `ClipView::take_id`) of
    /// the same audio in the same place; the count of takes stays, and another clip cut
    /// from the same one keeps the old take. Whether the clip follows stays as it is.
    SourceBpm { bpm: f64 },
    /// 「音の高さ」, the semitones: to the nearest whole number inside
    /// `−CLIP_SEMITONES_MAX..=CLIP_SEMITONES_MAX`.
    Semitones { semitones: f64 },
    /// 「音の高さ」, the cents: to the nearest whole number inside
    /// `−CLIP_CENTS_MAX..=CLIP_CENTS_MAX`.
    Cents { cents: f64 },
    /// 「声の感じを保つ」.
    KeepFormant { on: bool },
}

/// Where files let go of over the window go when two or more of them are audio (spec 3,
/// 30-C): the two halves of the box in the middle of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum DropMode {
    /// 「別々のトラックに」: a new vocal track for each file, every clip from the same tick.
    /// Also what one file alone is sent with.
    #[default]
    Separate,
    /// 「1 本にならべる」: on one track, one after the other in the order of their names.
    InRow,
}

/// Blocks of a vocal recording that are not what came in (docs/architecture.md 22-7): of
/// the recording going on, or of the last one. Nothing shows it yet (the sentence is asked
/// for in design-notes B-94-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecordingFaults {
    /// Times the recording moved to stay where the song was heard: the input skipped and
    /// silence of that length was written, or the output skipped and that much of the
    /// input was left out (docs/architecture.md 22-26).
    pub gaps: u32,
    /// Blocks the ring to the disk had no room for, written as silence.
    pub dropped: u32,
}

// ---- The peaks of an audio file, binary ----

pub const PEAKS_KIND: u8 = 4;
pub const PEAKS_VERSION: u8 = 1;
pub const PEAKS_HEADER_LEN: usize = 24;
/// How many frames of the file one peak of each level covers: every level four times the
/// one before (docs/architecture.md 22-9).
pub const PEAK_LEVELS: [u32; 5] = [256, 1024, 4096, 16_384, 65_536];
/// The most peaks one `peaks` reply carries.
pub const PEAKS_MAX_COUNT: u32 = 65_536;

const FLAG_GROWING: u8 = 1;

/// Peaks of one audio file, version 1, little-endian: the reply of the `peaks` invoke
/// (`peaks(audio, level, first, count)`). The layout table is docs/architecture.md 22-17;
/// `ui/src/bridge/peaks.ts` reads it.
///
/// A peak is the smallest and the largest sample of `PEAK_LEVELS[level]` frames of the
/// file, as i16 (−32768..=32767 for −1.0..=1.0), for each channel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Peaks {
    /// The file's number in the song's audio table.
    pub audio: u32,
    /// An index into `PEAK_LEVELS`.
    pub level: u8,
    /// 1 or 2.
    pub channels: u8,
    /// The file is being recorded or read: `total` goes up, and the last peak may still
    /// change. Peaks before the last are final all the same.
    pub growing: bool,
    /// The index of the first peak in here: it starts at frame `first × PEAK_LEVELS[level]`.
    pub first: u32,
    /// How many peaks of this level the file has now.
    pub total: u32,
    /// `count × channels` pairs: for each peak, for each channel, the smallest then the
    /// largest sample.
    pub values: Vec<i16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeaksError {
    TooShort {
        len: usize,
    },
    WrongKind(u8),
    WrongVersion(u8),
    WrongLevel(u8),
    WrongChannels(u8),
    /// The length does not match the count in the header.
    WrongLength {
        len: usize,
        expected: usize,
    },
}

impl fmt::Display for PeaksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(f, "peaks are {len} bytes, shorter than the header"),
            Self::WrongKind(k) => write!(f, "peaks kind {k}, expected {PEAKS_KIND}"),
            Self::WrongVersion(v) => write!(f, "peaks version {v}, expected {PEAKS_VERSION}"),
            Self::WrongLevel(l) => write!(f, "peaks level {l}, of {}", PEAK_LEVELS.len()),
            Self::WrongChannels(c) => write!(f, "peaks of {c} channels"),
            Self::WrongLength { len, expected } => {
                write!(f, "peaks are {len} bytes, the header says {expected}")
            }
        }
    }
}

impl std::error::Error for PeaksError {}

impl Peaks {
    /// How many peaks are in here.
    pub fn count(&self) -> u32 {
        (self.values.len() / (2 * usize::from(self.channels.max(1)))) as u32
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(PEAKS_HEADER_LEN + 2 * self.values.len());
        out.extend_from_slice(&[
            PEAKS_KIND,
            PEAKS_VERSION,
            self.level,
            self.channels,
            if self.growing { FLAG_GROWING } else { 0 },
            0,
            0,
            0,
        ]);
        out.extend_from_slice(&self.audio.to_le_bytes());
        out.extend_from_slice(&self.first.to_le_bytes());
        out.extend_from_slice(&self.count().to_le_bytes());
        out.extend_from_slice(&self.total.to_le_bytes());
        for v in &self.values {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Peaks, PeaksError> {
        if bytes.len() < PEAKS_HEADER_LEN {
            return Err(PeaksError::TooShort { len: bytes.len() });
        }
        if bytes[0] != PEAKS_KIND {
            return Err(PeaksError::WrongKind(bytes[0]));
        }
        if bytes[1] != PEAKS_VERSION {
            return Err(PeaksError::WrongVersion(bytes[1]));
        }
        let (level, channels) = (bytes[2], bytes[3]);
        if usize::from(level) >= PEAK_LEVELS.len() {
            return Err(PeaksError::WrongLevel(level));
        }
        if !matches!(channels, 1 | 2) {
            return Err(PeaksError::WrongChannels(channels));
        }
        let word = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        let count = word(16) as usize;
        let expected = PEAKS_HEADER_LEN + count * usize::from(channels) * 4;
        if bytes.len() != expected {
            return Err(PeaksError::WrongLength {
                len: bytes.len(),
                expected,
            });
        }
        Ok(Peaks {
            audio: word(8),
            level,
            channels,
            growing: bytes[4] & FLAG_GROWING != 0,
            first: word(12),
            total: word(20),
            values: bytes[PEAKS_HEADER_LEN..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes(*b))
                .collect(),
        })
    }
}
