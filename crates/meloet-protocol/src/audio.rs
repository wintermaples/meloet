//! Vocal clips, audio files and their waveforms (round 7, protocol 24; docs/architecture.md
//! 22-4, 22-8 to 22-10 and 22-17, spec 3 26-A, 27-A, 29-A, 30-C).

use std::fmt;

use serde::{Deserialize, Serialize};

/// The range of a vocal clip's volume in dB (spec 3 「クリップの音量」): there is no −∞.
pub const CLIP_GAIN_MIN_DB: f32 = -60.0;
pub const CLIP_GAIN_MAX_DB: f32 = 12.0;

/// The endings of the files that read as audio (spec 3 「読めるファイル」), as the status line
/// lists them: "読めるのは wav ・ mp3 ・ flac ・ ogg".
pub const AUDIO_EXTENSIONS: [&str; 4] = ["wav", "mp3", "flac", "ogg"];

/// What a clip of a vocal track has besides `ClipView` (`ClipView::audio`).
///
/// Where the waveform goes: the take's tick 0 is at `ClipView::start + take_offset` in the
/// song, and there the file is at `first_frame`. The audio plays at the file's own speed
/// (stretching comes later), so the frame of the file at a song tick `t` is
/// `first_frame + (seconds(t) − seconds(tick 0 of the take)) × sample_rate`, with the
/// screen's tempo map. Frames before 0 and from `frames` on are silence.
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

/// The group 「のび縮み ・ 音の高さ」 of the panel 「選んだクリップ」, as the song has it. Nothing
/// changes it yet and nothing sounds by it: the rows are shown unusable until the stretching
/// is made (spec 3, 28-A and Y22).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StretchView {
    /// 「曲のテンポについていく」: on for recorded audio, off for a file brought in.
    pub follow: bool,
    /// 「この音のテンポ」 at the head of the take; `None`: 「—」.
    pub source_bpm: Option<f64>,
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
    /// `Rendering`: the one clip the audio is made for. `None` for `Loading`.
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
    /// "音を作っています 34 %" (spec 6 「音を作っている」): the stretching, which nothing
    /// starts yet.
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
    /// Times the input skipped and silence of that length was written.
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
