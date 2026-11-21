//! Reads wav, mp3, flac and ogg files a packet at a time (symphonia 0.6.1). Never on the
//! audio thread, and never the whole file in memory.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;

/// What a file says of itself before it is decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    /// How many frames decoding it gives.
    pub frames: u64,
    pub sample_rate: u32,
    /// 1 or 2.
    pub channels: u8,
    /// The bits of a sample as the file has them; `None` for a kind without such a number
    /// (mp3, ogg).
    pub bits: Option<u8>,
}

/// A file being decoded.
pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track: u32,
    sample_rate: u32,
    channels: u8,
    bits: Option<u8>,
    /// What the file's head says of its length, when it says it in frames.
    header_frames: Option<u64>,
    block: Vec<f32>,
}

impl Decoder {
    /// Opens the file; an error says why it is not audio that reads.
    pub fn open(path: &Path) -> Result<Decoder, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(&extension.to_ascii_lowercase());
        }
        let format = symphonia::default::get_probe()
            .probe(&hint, stream, Default::default(), Default::default())
            .map_err(|e| format!("not a sound file this meloet reads: {e}"))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or("no sound in the file")?;
        let params = (track.codec_params.as_ref())
            .and_then(|p| p.audio())
            .ok_or("no sound in the file")?;
        let sample_rate = params.sample_rate.ok_or("no sample rate")?;
        let channels = params.channels.as_ref().map_or(0, |c| c.count());
        if sample_rate == 0 {
            return Err("no sample rate".into());
        }
        // The number of channels of an mp3 is known with its first packet; 0 here is
        // looked at again then.
        if channels > 2 {
            return Err(format!("{channels} channels; meloet reads mono and stereo"));
        }
        let bits = (params.bits_per_sample)
            .and_then(|b| u8::try_from(b).ok())
            .filter(|b| matches!(b, 8 | 16 | 24 | 32 | 64));
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| format!("no decoder: {e}"))?;
        // A wav whose head says "as long as the file" has a number here too; one that
        // ended early has more than is there, so the count is not taken from the head
        // alone (`facts`).
        let header_frames = track.num_frames;
        Ok(Decoder {
            track: track.id,
            format,
            decoder,
            sample_rate,
            channels: channels as u8,
            bits,
            header_frames,
            block: Vec::new(),
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 0 until the first block of a file that does not say it in its head.
    pub fn channels(&self) -> u8 {
        self.channels
    }

    pub fn header_frames(&self) -> Option<u64> {
        self.header_frames
    }

    /// The next frames, interleaved, with how many channels they have (the same for every
    /// block of a file); `None` at the end. A packet that does not decode is passed over,
    /// as a player does.
    pub fn next_block(&mut self) -> Result<Option<(u8, &[f32])>, String> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(None),
                // A file cut short ends where it ends.
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(format!("read: {e}")),
            };
            if packet.track_id != self.track {
                continue;
            }
            let buffer = match self.decoder.decode(&packet) {
                Ok(buffer) => buffer,
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(format!("decode: {e}")),
            };
            let channels = buffer.spec().channels().count();
            if !(1..=2).contains(&channels) {
                return Err(format!("{channels} channels; meloet reads mono and stereo"));
            }
            if self.channels == 0 {
                self.channels = channels as u8;
            } else if usize::from(self.channels) != channels {
                return Err("the number of channels changes inside the file".into());
            }
            if buffer.frames() == 0 {
                continue;
            }
            self.block.resize(buffer.samples_interleaved(), 0.0);
            buffer.copy_to_slice_interleaved(&mut self.block[..]);
            return Ok(Some((self.channels, &self.block)));
        }
    }
}

/// What the file at `path` is. A plain wav and a flac say how long they are in their
/// heads; the others are decoded once and counted (an mp3 without a tag only guesses its
/// length), the samples thrown away as they come.
pub fn facts(path: &Path) -> Result<Facts, String> {
    if let Ok(wav) = super::wav::WavPcm::open(path) {
        use crate::pcm::PcmFile;
        if wav.frames() == 0 {
            return Err("the file has no sound in it".into());
        }
        return Ok(Facts {
            frames: wav.frames(),
            sample_rate: wav.sample_rate(),
            channels: wav.channels(),
            bits: Some(wav.bits()),
        });
    }
    let mut decoder = Decoder::open(path)?;
    let flac =
        (path.extension().and_then(|e| e.to_str())).is_some_and(|e| e.eq_ignore_ascii_case("flac"));
    let said = decoder.header_frames().filter(|&f| f > 0 && flac);
    let frames = match said.filter(|_| decoder.channels() > 0) {
        Some(frames) => frames,
        None => {
            let mut frames = 0u64;
            while let Some((channels, block)) = decoder.next_block()? {
                frames += (block.len() / usize::from(channels)) as u64;
            }
            frames
        }
    };
    if frames == 0 || decoder.channels() == 0 {
        return Err("the file has no sound in it".into());
    }
    Ok(Facts {
        frames,
        sample_rate: decoder.sample_rate(),
        channels: decoder.channels(),
        bits: decoder.bits,
    })
}
