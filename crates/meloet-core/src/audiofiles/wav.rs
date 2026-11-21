//! Wav files as the read-ahead thread reads them: straight from the disk, a stretch at a
//! time, never the whole file in memory. And the float wav the cache is written as.

use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::pcm::PcmFile;

/// How the samples of a wav file are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Coding {
    /// Unsigned, 8 bits.
    U8,
    I16,
    I24,
    I32,
    F32,
    F64,
}

impl Coding {
    fn bytes(self) -> usize {
        match self {
            Coding::U8 => 1,
            Coding::I16 => 2,
            Coding::I24 => 3,
            Coding::I32 | Coding::F32 => 4,
            Coding::F64 => 8,
        }
    }

    fn sample(self, b: &[u8]) -> f32 {
        match self {
            Coding::U8 => (f32::from(b[0]) - 128.0) / 128.0,
            Coding::I16 => f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0,
            Coding::I24 => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            Coding::I32 => {
                (f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])) / 2_147_483_648.0) as f32
            }
            Coding::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Coding::F64 => {
                f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f32
            }
        }
    }
}

/// A wav file open for reading.
pub struct WavPcm {
    file: File,
    coding: Coding,
    channels: u8,
    sample_rate: u32,
    /// Where the samples start in the file.
    data: u64,
    frames: u64,
    bytes: Vec<u8>,
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}

const TAG_PCM: u16 = 1;
const TAG_FLOAT: u16 = 3;
const TAG_EXTENSIBLE: u16 = 0xFFFE;
/// A length that says "to the end of the file" (a file still being written, or one that
/// ended early: docs/architecture.md 22-7).
const UNKNOWN: u32 = 0xFFFF_FFFF;

impl WavPcm {
    /// Opens a wav file of 1 or 2 channels of plain samples. An error for anything else:
    /// such a file is decoded into the cache first.
    pub fn open(path: &Path) -> io::Result<WavPcm> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        let mut head = [0u8; 12];
        file.read_exact(&mut head)?;
        if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
            return Err(bad("not a wav file"));
        }
        let mut format: Option<(Coding, u8, u32)> = None;
        let mut at = 12u64;
        loop {
            let mut chunk = [0u8; 8];
            file.seek(SeekFrom::Start(at))?;
            file.read_exact(&mut chunk)?;
            let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let body = at + 8;
            match &chunk[0..4] {
                b"fmt " => {
                    let mut f = [0u8; 40];
                    let n = (size as usize).min(40);
                    if n < 16 {
                        return Err(bad("the fmt chunk is too short"));
                    }
                    file.read_exact(&mut f[..n])?;
                    let word = |i: usize| u16::from_le_bytes([f[i], f[i + 1]]);
                    let mut tag = word(0);
                    if tag == TAG_EXTENSIBLE && n >= 26 {
                        // The first two bytes of the sub-format are the plain tag.
                        tag = word(24);
                    }
                    let channels = word(2);
                    let sample_rate = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
                    let block = usize::from(word(12));
                    let coding = match (tag, word(14)) {
                        (TAG_PCM, 8) => Coding::U8,
                        (TAG_PCM, 16) => Coding::I16,
                        (TAG_PCM, 24) => Coding::I24,
                        (TAG_PCM, 32) => Coding::I32,
                        (TAG_FLOAT, 32) => Coding::F32,
                        (TAG_FLOAT, 64) => Coding::F64,
                        _ => return Err(bad("samples of a kind that is not read as they are")),
                    };
                    if !(1..=2).contains(&channels)
                        || sample_rate == 0
                        || block != coding.bytes() * usize::from(channels)
                    {
                        return Err(bad("not 1 or 2 channels of plain samples"));
                    }
                    format = Some((coding, channels as u8, sample_rate));
                }
                b"data" => {
                    let (coding, channels, sample_rate) =
                        format.ok_or_else(|| bad("the samples come before their format"))?;
                    let frame = (coding.bytes() * usize::from(channels)) as u64;
                    let there = len.saturating_sub(body);
                    let bytes = if size == UNKNOWN {
                        there
                    } else {
                        u64::from(size).min(there)
                    };
                    return Ok(WavPcm {
                        file,
                        coding,
                        channels,
                        sample_rate,
                        data: body,
                        frames: bytes / frame,
                        bytes: Vec::new(),
                    });
                }
                _ => {}
            }
            // Chunks are padded to an even length.
            at = body + u64::from(size) + u64::from(size & 1);
            if at + 8 > len {
                return Err(bad("no samples in the wav file"));
            }
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The bits of a sample as the file has them.
    pub fn bits(&self) -> u8 {
        (self.coding.bytes() * 8) as u8
    }
}

impl PcmFile for WavPcm {
    fn channels(&self) -> u8 {
        self.channels
    }

    fn frames(&self) -> u64 {
        self.frames
    }

    fn read(&mut self, frame: u64, out: &mut [f32]) -> io::Result<usize> {
        let channels = usize::from(self.channels);
        let wanted = (out.len() / channels) as u64;
        let count = wanted.min(self.frames.saturating_sub(frame)) as usize;
        if count == 0 {
            return Ok(0);
        }
        let sample = self.coding.bytes();
        self.bytes.resize(count * channels * sample, 0);
        self.file.seek(SeekFrom::Start(
            self.data + frame * (channels * sample) as u64,
        ))?;
        self.file.read_exact(&mut self.bytes)?;
        for (o, b) in out.iter_mut().zip(self.bytes.chunks_exact(sample)) {
            *o = self.coding.sample(b);
        }
        Ok(count)
    }
}

/// A 32-bit float wav being written into the cache: under a temporary name until
/// [`CacheWav::finish`], so that a file with the final name is always whole.
pub struct CacheWav {
    out: BufWriter<File>,
    bytes: Vec<u8>,
    temp: PathBuf,
    path: PathBuf,
    channels: u8,
    sample_rate: u32,
    samples: u64,
}

const HEAD: u64 = 44;

fn float_head(sample_rate: u32, channels: u8, data: u32) -> [u8; 44] {
    let block = 4 * u32::from(channels);
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&data.saturating_add(HEAD as u32 - 8).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&TAG_FLOAT.to_le_bytes());
    h[22..24].copy_from_slice(&u16::from(channels).to_le_bytes());
    h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(sample_rate.saturating_mul(block)).to_le_bytes());
    h[32..34].copy_from_slice(&(block as u16).to_le_bytes());
    h[34..36].copy_from_slice(&32u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

impl CacheWav {
    pub fn create(path: &Path, sample_rate: u32, channels: u8) -> io::Result<CacheWav> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut temp = path.as_os_str().to_owned();
        temp.push(".part");
        let temp = PathBuf::from(temp);
        let mut out = BufWriter::with_capacity(1 << 16, File::create(&temp)?);
        out.write_all(&float_head(sample_rate, channels, UNKNOWN))?;
        Ok(CacheWav {
            out,
            bytes: Vec::new(),
            temp,
            path: path.to_owned(),
            channels,
            sample_rate,
            samples: 0,
        })
    }

    /// The next frames, interleaved.
    pub fn write(&mut self, interleaved: &[f32]) -> io::Result<()> {
        self.bytes.clear();
        self.bytes
            .extend(interleaved.iter().flat_map(|s| s.to_le_bytes()));
        self.out.write_all(&self.bytes)?;
        self.samples += interleaved.len() as u64;
        Ok(())
    }

    /// Puts the length into the head and gives the file its name. Not synced: the file can
    /// be made again. Returns how many frames it holds.
    pub fn finish(self) -> io::Result<u64> {
        let CacheWav {
            out,
            temp,
            path,
            channels,
            sample_rate,
            samples,
            ..
        } = self;
        let mut file = out.into_inner().map_err(|e| e.into_error())?;
        // Longer than a wav can say: the length stays "to the end of the file".
        if let Ok(data) = u32::try_from(samples * 4)
            && data < UNKNOWN - HEAD as u32
        {
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&float_head(sample_rate, channels, data))?;
        }
        drop(file);
        std::fs::rename(&temp, &path)?;
        Ok(samples / u64::from(channels.max(1)))
    }

    /// Lets go of a file that will not be finished.
    pub fn abandon(self) {
        let temp = self.temp.clone();
        drop(self);
        let _ = std::fs::remove_file(temp);
    }
}
