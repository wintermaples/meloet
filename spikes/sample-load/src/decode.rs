//! Whole-file decode through symphonia into interleaved samples.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::sample::Sample;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::TrackType;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;

pub struct Decoded<S> {
    pub rate: u32,
    pub channels: usize,
    /// Interleaved.
    pub samples: Vec<S>,
}

impl<S> Decoded<S> {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1)
    }
}

pub trait Target: Sample + symphonia::core::audio::conv::ConvertibleSample + Send {}
impl Target for f32 {}
impl Target for i16 {}

pub fn decode<S: Target>(path: &Path) -> Result<Decoded<S>, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, Default::default(), Default::default())
        .map_err(|e| format!("{}: probe: {e}", path.display()))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| format!("{}: no audio track", path.display()))?;
    let track_id = track.id;
    let expected = track.num_frames;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| format!("{}: no audio parameters", path.display()))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| format!("{}: decoder: {e}", path.display()))?;

    let mut rate = 0;
    let mut channels = 0;
    let mut samples: Vec<S> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(e) => return Err(format!("{}: read: {e}", path.display())),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                if channels == 0 {
                    rate = buf.spec().rate();
                    channels = buf.spec().channels().count();
                    // One exact allocation when the container says how long the file is.
                    if let Some(n) = expected {
                        samples.reserve_exact(n as usize * channels);
                    }
                }
                let at = samples.len();
                samples.resize(at + buf.samples_interleaved(), S::MID);
                buf.copy_to_slice_interleaved(&mut samples[at..]);
            }
            Err(Error::DecodeError(e)) => return Err(format!("{}: decode: {e}", path.display())),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    Ok(Decoded {
        rate,
        channels,
        samples,
    })
}
