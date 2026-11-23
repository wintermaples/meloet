//! The file an export writes, whatever its format (docs/architecture.md 22-11, 22-20).
//!
//! The export's thread hands over the finished audio, already at the file's sample rate, as
//! floats; an [`AudioSink`] makes the file's samples of them. Both formats go through the
//! same step from floats to whole numbers (with dither for 16 bits, once), so a wav and a
//! flac file of the same export hold the same samples. A sink writes where it is told to,
//! which is a file of the work place: moving the finished file to where the person wants it
//! is the caller's.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use flacenc::bitsink::MemSink;
use flacenc::component::{BitRepr, Stream, StreamInfo};
use flacenc::config;
use flacenc::error::{Verified, Verify};
use flacenc::source::{Context, Fill, FrameBuf};
use meloet_protocol::{ExportFormat, RecordFormat};

/// What the file is to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkSpec {
    pub format: ExportFormat,
    pub sample_rate: u32,
    /// 1 or 2.
    pub channels: u8,
    /// One of `format.bits_choices()`. 32 bits are floats.
    pub bits: RecordFormat,
}

/// A file being written. Not for the audio thread: every call may wait for the disk.
pub trait AudioSink: Send {
    /// Appends frames, interleaved, as many samples as a multiple of the channels.
    fn write(&mut self, interleaved: &[f32]) -> io::Result<()>;

    /// Ends the file so that it reads as a whole one. Returns the frames it holds.
    fn finish(self: Box<Self>) -> io::Result<u64>;

    /// Takes the half-written file away.
    fn abandon(self: Box<Self>);
}

/// Starts the file at `path`, over whatever is there: a wav as the recorder writes it
/// (`crate::vocalrec`), or a flac made with `flacenc`.
pub fn create(path: &Path, spec: SinkSpec) -> io::Result<Box<dyn AudioSink>> {
    if !(1..=2).contains(&spec.channels) || spec.sample_rate == 0 {
        return Err(invalid("an export has one or two channels and a rate"));
    }
    if !spec.format.bits_choices().contains(&spec.bits) {
        return Err(invalid("the format does not take these bits"));
    }
    Ok(match spec.format {
        ExportFormat::Wav => Box::new(WavSink::create(path, spec)?),
        ExportFormat::Flac => Box::new(FlacSink::create(path, spec)?),
        // round12 stub: F2
        ExportFormat::Mp3 => return Err(invalid("mp3 cannot be written yet")),
    })
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, what.to_owned())
}

/// The one step from floats to whole numbers, for both formats. The dither of 16 bits
/// starts from the same seed for every file: the same audio gives the same file.
struct Whole {
    bits: RecordFormat,
    seed: u32,
}

impl Whole {
    fn new(bits: RecordFormat) -> Whole {
        Whole {
            bits,
            seed: 0x9E37_79B9,
        }
    }

    fn noise(&mut self) -> f32 {
        // xorshift32; 24 bits of it make a number in [0, 1).
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// For 16 and 24 bits. A sample that is no number is silence.
    fn of(&mut self, sample: f32) -> i32 {
        let s = if sample.is_finite() { sample } else { 0.0 };
        match self.bits {
            RecordFormat::Bits16 => {
                // Triangular dither of one step each way, before rounding.
                let dither = self.noise() - self.noise();
                let v = (s.clamp(-1.0, 1.0) * 32_767.0 + dither).round();
                v.clamp(-32_768.0, 32_767.0) as i32
            }
            _ => (f64::from(s.clamp(-1.0, 1.0)) * 8_388_607.0).round() as i32,
        }
    }
}

const WAV_HEAD: u64 = 44;
/// The length a wav has while it is written, and when it is longer than a wav can say.
const UNKNOWN: u32 = u32::MAX;

fn wav_head(spec: SinkSpec, data: u32) -> [u8; 44] {
    crate::audiofiles::wav::wav_head(spec.sample_rate, spec.channels.into(), spec.bits, data)
}

struct WavSink {
    out: BufWriter<File>,
    path: PathBuf,
    spec: SinkSpec,
    whole: Whole,
    bytes: Vec<u8>,
    samples: u64,
}

impl WavSink {
    fn create(path: &Path, spec: SinkSpec) -> io::Result<WavSink> {
        let mut out = BufWriter::with_capacity(1 << 16, File::create(path)?);
        out.write_all(&wav_head(spec, UNKNOWN))?;
        Ok(WavSink {
            out,
            path: path.to_owned(),
            spec,
            whole: Whole::new(spec.bits),
            bytes: Vec::new(),
            samples: 0,
        })
    }
}

impl AudioSink for WavSink {
    fn write(&mut self, interleaved: &[f32]) -> io::Result<()> {
        self.bytes.clear();
        for &s in interleaved {
            match self.spec.bits {
                RecordFormat::Bits32 => {
                    let s = if s.is_finite() { s } else { 0.0 };
                    self.bytes.extend_from_slice(&s.to_le_bytes());
                }
                RecordFormat::Bits24 => {
                    let v = self.whole.of(s);
                    self.bytes.extend_from_slice(&v.to_le_bytes()[..3]);
                }
                RecordFormat::Bits16 => {
                    let v = self.whole.of(s) as i16;
                    self.bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        self.out.write_all(&self.bytes)?;
        self.samples += interleaved.len() as u64;
        Ok(())
    }

    fn finish(self: Box<Self>) -> io::Result<u64> {
        let WavSink {
            out, spec, samples, ..
        } = *self;
        let mut file = out.into_inner().map_err(|e| e.into_error())?;
        let bytes = samples * u64::from(spec.bits.bits() / 8);
        if let Ok(data) = u32::try_from(bytes)
            && data < UNKNOWN - WAV_HEAD as u32
        {
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&wav_head(spec, data))?;
        }
        file.sync_all()?;
        Ok(samples / u64::from(spec.channels))
    }

    fn abandon(self: Box<Self>) {
        let path = self.path.clone();
        drop(self);
        let _ = std::fs::remove_file(path);
    }
}

/// The frames of one flac frame: every frame but the last has this many.
const FLAC_BLOCK: usize = 4_096;

fn flac_failed(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("flac: {e}"))
}

/// A flac file written a frame at a time, so that a long song is never in memory whole
/// (the steps of `flacenc::encode_with_fixed_block_size`, coding.rs 648 to 692). The head
/// is written again at the end, when the length and the checksum are known.
struct FlacSink {
    out: BufWriter<File>,
    path: PathBuf,
    channels: usize,
    whole: Whole,
    config: Verified<config::Encoder>,
    info: StreamInfo,
    /// The samples that do not fill a frame yet, interleaved.
    waiting: Vec<i32>,
    buf: FrameBuf,
    /// Counts the frames and makes the checksum of the samples.
    context: Context,
    /// The smallest and the largest frame so far, in bytes.
    sizes: Option<(usize, usize)>,
    bits: MemSink<u8>,
}

fn flac_head(info: &StreamInfo) -> io::Result<Vec<u8>> {
    let mut sink = MemSink::<u8>::new();
    Stream::with_stream_info(info.clone())
        .write(&mut sink)
        .map_err(flac_failed)?;
    Ok(sink.into_inner())
}

impl FlacSink {
    fn create(path: &Path, spec: SinkSpec) -> io::Result<FlacSink> {
        let channels = usize::from(spec.channels);
        let bits = usize::from(spec.bits.bits());
        let mut info =
            StreamInfo::new(spec.sample_rate as usize, channels, bits).map_err(flac_failed)?;
        info.set_block_sizes(FLAC_BLOCK, FLAC_BLOCK)
            .map_err(flac_failed)?;
        let mut encoder = config::Encoder::default();
        encoder.block_size = FLAC_BLOCK;
        encoder.multithread = false;
        let config = encoder.into_verified().map_err(|(_, e)| flac_failed(e))?;
        let mut out = BufWriter::with_capacity(1 << 16, File::create(path)?);
        out.write_all(&flac_head(&info)?)?;
        Ok(FlacSink {
            out,
            path: path.to_owned(),
            channels,
            whole: Whole::new(spec.bits),
            config,
            info,
            waiting: Vec::with_capacity(FLAC_BLOCK * channels),
            buf: FrameBuf::with_size(channels, FLAC_BLOCK).map_err(flac_failed)?,
            context: Context::new(bits, channels),
            sizes: None,
            bits: MemSink::new(),
        })
    }

    /// Writes the first `frames` frames of `waiting` as one flac frame.
    fn frame(&mut self, frames: usize) -> io::Result<()> {
        let samples = &self.waiting[..frames * self.channels];
        let mut both = (&mut self.buf, &mut self.context);
        both.fill_interleaved(samples).map_err(flac_failed)?;
        let number = self.context.current_frame_number().unwrap_or(0);
        let frame = flacenc::encode_fixed_size_frame(&self.config, &self.buf, number, &self.info)
            .map_err(flac_failed)?;
        self.bits.clear();
        frame.write(&mut self.bits).map_err(flac_failed)?;
        let bytes = self.bits.as_slice();
        self.sizes = Some(match self.sizes {
            Some((min, max)) => (min.min(bytes.len()), max.max(bytes.len())),
            None => (bytes.len(), bytes.len()),
        });
        self.out.write_all(bytes)?;
        self.waiting.drain(..frames * self.channels);
        Ok(())
    }
}

impl AudioSink for FlacSink {
    fn write(&mut self, interleaved: &[f32]) -> io::Result<()> {
        for &s in interleaved {
            let v = self.whole.of(s);
            self.waiting.push(v);
            if self.waiting.len() == FLAC_BLOCK * self.channels {
                self.frame(FLAC_BLOCK)?;
            }
        }
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> io::Result<u64> {
        let left = self.waiting.len() / self.channels;
        if left > 0 {
            self.frame(left)?;
        }
        let FlacSink {
            out,
            mut info,
            context,
            sizes,
            ..
        } = *self;
        let frames = context.total_samples();
        info.set_total_samples(frames);
        info.set_md5_digest(&context.md5_digest());
        if let Some((min, max)) = sizes {
            info.set_frame_sizes(min, max).map_err(flac_failed)?;
        }
        let mut file = out.into_inner().map_err(|e| e.into_error())?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&flac_head(&info)?)?;
        file.sync_all()?;
        Ok(frames as u64)
    }

    fn abandon(self: Box<Self>) {
        let path = self.path.clone();
        drop(self);
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests;
