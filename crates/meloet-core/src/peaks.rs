//! The peaks of an audio file: what the waveform is drawn from (docs/architecture.md 22-9,
//! 22-17).
//!
//! Five levels, a peak for every 256, 1024, 4096, 16384 and 65536 frames
//! (`meloet_protocol::PEAK_LEVELS`), each the smallest and the largest sample of every
//! channel as i16. A [`PeakBuilder`] makes them a block at a time, while a recording is
//! written or a file is read; they are kept beside nothing of the song, in
//! `$XDG_CACHE_HOME/meloet/peaks/<uid>.peaks`, and made again when that is gone.
//!
//! The file, little-endian:
//!
//! ```text
//!  0  4  "mlpk"
//!  4  1  version (1)
//!  5  1  channels (1 or 2)
//!  6  2  0
//!  8  8  frames of the audio the peaks were made from
//! 16 20  how many peaks each of the five levels has (u32 each)
//! 36  …  level 0, then 1 … 4: for each peak, for each channel, smallest then largest (i16)
//! ```

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use meloet_protocol::{PEAK_LEVELS, PEAKS_MAX_COUNT, Peaks};

const MAGIC: &[u8; 4] = b"mlpk";
const FILE_VERSION: u8 = 1;
const FILE_HEADER_LEN: u64 = 36;
const LEVELS: usize = PEAK_LEVELS.len();
/// How many peaks of a level make one of the next.
const FAN: u32 = 4;

/// Where the peaks are kept: `$XDG_CACHE_HOME/meloet/peaks`. `None` when the PC has no
/// place for caches.
pub fn peaks_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("meloet").join("peaks"))
}

/// The peaks file of the audio with this `uid` (`AudioSource::uid`), in `dir`.
pub fn peaks_path(dir: &Path, uid: &[u8; 16]) -> PathBuf {
    let name: String = uid.iter().map(|b| format!("{b:02x}")).collect();
    dir.join(format!("{name}.peaks"))
}

fn to_i16(sample: f32) -> i16 {
    // What is not a number draws as silence.
    (sample.clamp(-1.0, 1.0) * 32_767.0) as i16
}

/// The smallest and the largest of each channel, of what was put in so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Part {
    /// [left smallest, left largest, right smallest, right largest].
    range: [i16; 4],
    /// How many went in: frames for level 0, peaks of the level below for the others.
    filled: u32,
}

impl Part {
    const EMPTY: Part = Part {
        range: [i16::MAX, i16::MIN, i16::MAX, i16::MIN],
        filled: 0,
    };

    fn take(&mut self, channel: usize, low: i16, high: i16) {
        self.range[2 * channel] = self.range[2 * channel].min(low);
        self.range[2 * channel + 1] = self.range[2 * channel + 1].max(high);
    }

    fn join(&mut self, other: &Part) {
        for channel in 0..2 {
            self.take(
                channel,
                other.range[2 * channel],
                other.range[2 * channel + 1],
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Level {
    /// The peaks that are whole.
    done: Vec<i16>,
    part: Part,
}

/// Makes the five levels from audio that comes a block at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeakBuilder {
    channels: u8,
    frames: u64,
    levels: [Level; LEVELS],
}

impl PeakBuilder {
    /// `channels`: 1 or 2 (anything else counts as 1).
    pub fn new(channels: u8) -> PeakBuilder {
        PeakBuilder {
            channels: if channels == 2 { 2 } else { 1 },
            frames: 0,
            levels: std::array::from_fn(|_| Level {
                done: Vec::new(),
                part: Part::EMPTY,
            }),
        }
    }

    pub fn channels(&self) -> u8 {
        self.channels
    }

    /// How many frames went in.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// The next frames of the audio, interleaved; what is left of a frame at the end is
    /// passed over.
    pub fn push(&mut self, interleaved: &[f32]) {
        let channels = usize::from(self.channels);
        for frame in interleaved.chunks_exact(channels) {
            for (channel, &sample) in frame.iter().enumerate() {
                let v = to_i16(sample);
                self.levels[0].part.take(channel, v, v);
            }
            self.levels[0].part.filled += 1;
            self.frames += 1;
            if self.levels[0].part.filled == PEAK_LEVELS[0] {
                self.close(0);
            }
        }
    }

    /// The part of `level` is whole: it becomes a peak, and goes into the level above.
    fn close(&mut self, level: usize) {
        let part = std::mem::replace(&mut self.levels[level].part, Part::EMPTY);
        let channels = usize::from(self.channels);
        self.levels[level]
            .done
            .extend_from_slice(&part.range[..2 * channels]);
        if level + 1 < LEVELS {
            let above = &mut self.levels[level + 1].part;
            above.join(&part);
            above.filled += 1;
            if above.filled == FAN {
                self.close(level + 1);
            }
        }
    }

    /// The peak of `level` that is not whole yet, from everything below it too.
    fn open_part(&self, level: usize) -> Option<Part> {
        let mut part = self.levels[level].part;
        if level > 0
            && let Some(below) = self.open_part(level - 1)
        {
            part.join(&below);
            part.filled += 1;
        }
        (part.filled > 0).then_some(part)
    }

    /// How many peaks `level` has, the one that is not whole yet too.
    pub fn count(&self, level: usize) -> u32 {
        let channels = usize::from(self.channels);
        (self.levels[level].done.len() / (2 * channels)) as u32
            + u32::from(self.open_part(level).is_some())
    }

    /// The peaks `first .. first + count` of `level` that there are: for each peak, for
    /// each channel, the smallest then the largest sample.
    pub fn values(&self, level: usize, first: u32, count: u32) -> Vec<i16> {
        let per_peak = 2 * usize::from(self.channels);
        let total = self.count(level);
        let end = first.saturating_add(count).min(total);
        let mut out = Vec::with_capacity(end.saturating_sub(first) as usize * per_peak);
        let done = &self.levels[level].done;
        for index in first..end {
            let at = index as usize * per_peak;
            match done.get(at..at + per_peak) {
                Some(peak) => out.extend_from_slice(peak),
                None => {
                    if let Some(part) = self.open_part(level) {
                        out.extend_from_slice(&part.range[..per_peak]);
                    }
                }
            }
        }
        out
    }

    /// The reply of the `peaks` invoke from what is in here. `growing`: more audio comes.
    pub fn reply(&self, audio: u32, level: u8, first: u32, count: u32, growing: bool) -> Peaks {
        let level = level.min(LEVELS as u8 - 1);
        let count = count.min(PEAKS_MAX_COUNT);
        Peaks {
            audio,
            level,
            channels: self.channels,
            growing,
            first,
            total: self.count(usize::from(level)),
            values: self.values(usize::from(level), first, count),
        }
    }

    /// Writes the peaks file whole: a temporary file beside it, then renamed into place, so
    /// a reader never finds half of one. Not synced: the file can be made again.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&[FILE_VERSION, self.channels, 0, 0]);
        bytes.extend_from_slice(&self.frames.to_le_bytes());
        for level in 0..LEVELS {
            bytes.extend_from_slice(&self.count(level).to_le_bytes());
        }
        for level in 0..LEVELS {
            for v in self.values(level, 0, u32::MAX) {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        let mut temp = path.as_os_str().to_owned();
        temp.push(".tmp");
        let temp = PathBuf::from(temp);
        File::create(&temp)?.write_all(&bytes)?;
        std::fs::rename(&temp, path)
    }
}

/// What the head of a peaks file says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeaksHead {
    pub channels: u8,
    /// Frames of the audio the peaks were made from: a file of another length is not this
    /// audio's.
    pub frames: u64,
    pub counts: [u32; LEVELS],
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}

fn read_head(file: &mut File) -> io::Result<PeaksHead> {
    let mut head = [0u8; FILE_HEADER_LEN as usize];
    file.read_exact(&mut head)?;
    if &head[..4] != MAGIC || head[4] != FILE_VERSION || !matches!(head[5], 1 | 2) {
        return Err(bad("not a peaks file of this version"));
    }
    let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    let head = PeaksHead {
        channels: head[5],
        frames: u64::from_le_bytes(head[8..16].try_into().expect("8 bytes")),
        counts: std::array::from_fn(|level| word(16 + 4 * level)),
    };
    let values: u64 = head.counts.iter().map(|&c| u64::from(c)).sum();
    let expected = FILE_HEADER_LEN + values * u64::from(head.channels) * 4;
    if file.metadata()?.len() != expected {
        return Err(bad("the peaks file is not as long as its head says"));
    }
    Ok(head)
}

/// The head of the peaks file at `path`; an error when it is not there or is no peaks
/// file (it is then made again).
pub fn read_peaks_head(path: &Path) -> io::Result<PeaksHead> {
    read_head(&mut File::open(path)?)
}

/// The reply of the `peaks` invoke from the peaks file at `path`: only the peaks asked for
/// are read.
pub fn read_peaks(path: &Path, audio: u32, level: u8, first: u32, count: u32) -> io::Result<Peaks> {
    let mut file = File::open(path)?;
    let head = read_head(&mut file)?;
    let level = usize::from(level).min(LEVELS - 1);
    let total = head.counts[level];
    let end = first.saturating_add(count.min(PEAKS_MAX_COUNT)).min(total);
    let per_peak = u64::from(head.channels) * 4;
    let before: u64 = head.counts[..level].iter().map(|&c| u64::from(c)).sum();
    let mut bytes = vec![0u8; (u64::from(end.saturating_sub(first)) * per_peak) as usize];
    if !bytes.is_empty() {
        file.seek(SeekFrom::Start(
            FILE_HEADER_LEN + (before + u64::from(first)) * per_peak,
        ))?;
        file.read_exact(&mut bytes)?;
    }
    Ok(Peaks {
        audio,
        level: level as u8,
        channels: head.channels,
        growing: false,
        first,
        total,
        values: bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect(),
    })
}

/// Peaks that are still being made (a recording, a file being read), shared by the thread
/// that makes them and the one that answers the screen. Neither is the audio thread.
pub type SharedPeaks = Arc<Mutex<PeakBuilder>>;

/// Where the peaks of an audio file are, for the `peaks` invoke: the core thread says
/// which, and the thread of the invoke reads them, so that the core thread touches no file.
#[derive(Debug, Clone)]
pub enum PeaksPlace {
    File(PathBuf),
    Growing(SharedPeaks),
}

impl PeaksPlace {
    /// The reply of `peaks(audio, level, first, count)`; `None` when the file is not there
    /// or does not read.
    pub fn reply(&self, audio: u32, level: u8, first: u32, count: u32) -> Option<Vec<u8>> {
        let peaks = match self {
            PeaksPlace::File(path) => read_peaks(path, audio, level, first, count).ok()?,
            PeaksPlace::Growing(builder) => builder
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .reply(audio, level, first, count, true),
        };
        Some(peaks.encode())
    }
}

#[cfg(test)]
mod tests;
