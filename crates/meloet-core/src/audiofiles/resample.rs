//! Brings audio to the rate the engine runs at (rubato 5.1.0, the FFT resampler: the ratio
//! is fixed).
//!
//! The steps are those of rubato's `Resampler::process_all_into_buffer` (lib.rs 325 to
//! 393): whole chunks, then the last one as a partial chunk, then silence until the delay
//! of the resampler is pushed out; the delay is cut off the head and the output is
//! `ceil(ratio × input)` frames long. Only that it is done a chunk at a time, so that a
//! long file is never in memory whole.

use std::io;

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

const CHUNK: usize = 1024;

pub struct Resample {
    fft: Fft<f32>,
    channels: usize,
    /// Input that does not fill a chunk yet, interleaved.
    waiting: Vec<f32>,
    out: Vec<f32>,
    /// Frames of the resampler's delay still to cut off the head.
    to_trim: usize,
    frames_in: u64,
    frames_out: u64,
    from: u32,
    to: u32,
}

fn failed(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("resampling: {e}"))
}

impl Resample {
    pub fn new(from: u32, to: u32, channels: u8) -> io::Result<Resample> {
        let channels = usize::from(channels);
        let fft = Fft::<f32>::new(
            from as usize,
            to as usize,
            CHUNK,
            channels,
            FixedSync::Input,
        )
        .map_err(failed)?;
        Ok(Resample {
            out: vec![0.0; fft.output_frames_max() * channels],
            to_trim: fft.output_delay(),
            fft,
            channels,
            waiting: Vec::new(),
            frames_in: 0,
            frames_out: 0,
            from,
            to,
        })
    }

    /// How many frames the output has for an input of `frames`: the ratio, rounded up.
    /// In whole numbers, so that one second is one second (rubato's own count goes through
    /// a float and gives 48001 frames for 44100 at times).
    pub fn frames_for(from: u32, to: u32, frames: u64) -> u64 {
        (u128::from(frames) * u128::from(to)).div_ceil(u128::from(from.max(1))) as u64
    }

    /// One chunk through the resampler; `valid` frames of `waiting` are audio, the rest of
    /// the chunk counts as silence. At most `keep` frames of the output go to `sink`.
    fn chunk(
        &mut self,
        valid: Option<usize>,
        keep: u64,
        sink: &mut dyn FnMut(&[f32]) -> io::Result<()>,
    ) -> io::Result<()> {
        let need = self.fft.input_frames_next();
        if self.waiting.len() < need * self.channels {
            self.waiting.resize(need * self.channels, 0.0);
        }
        let out_frames = self.out.len() / self.channels;
        let input =
            InterleavedSlice::new(&self.waiting[..], self.channels, need).map_err(failed)?;
        let mut output = InterleavedSlice::new_mut(&mut self.out[..], self.channels, out_frames)
            .map_err(failed)?;
        let indexing = valid.map(|n| Indexing::new().partial_len(n));
        let (_, made) = self
            .fft
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .map_err(failed)?;
        let skip = self.to_trim.min(made);
        self.to_trim -= skip;
        let frames = ((made - skip) as u64).min(keep) as usize;
        if frames > 0 {
            sink(&self.out[skip * self.channels..(skip + frames) * self.channels])?;
            self.frames_out += frames as u64;
        }
        Ok(())
    }

    /// The next frames of the input, interleaved; what is ready goes to `sink`.
    pub fn push(
        &mut self,
        interleaved: &[f32],
        sink: &mut dyn FnMut(&[f32]) -> io::Result<()>,
    ) -> io::Result<()> {
        self.frames_in += (interleaved.len() / self.channels) as u64;
        self.waiting.extend_from_slice(interleaved);
        loop {
            let need = self.fft.input_frames_next() * self.channels;
            if self.waiting.len() < need {
                return Ok(());
            }
            self.chunk(None, u64::MAX, sink)?;
            self.waiting.drain(..need);
        }
    }

    /// The input is over: the rest goes to `sink`. Returns how many frames went out in all.
    pub fn finish(mut self, sink: &mut dyn FnMut(&[f32]) -> io::Result<()>) -> io::Result<u64> {
        let expected = Resample::frames_for(self.from, self.to, self.frames_in);
        let left = self.waiting.len() / self.channels;
        if left > 0 {
            self.chunk(Some(left), expected.saturating_sub(self.frames_out), sink)?;
        }
        self.waiting.clear();
        while self.frames_out < expected {
            self.chunk(Some(0), expected - self.frames_out, sink)?;
        }
        Ok(self.frames_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn through(from: u32, to: u32, channels: u8, input: &[f32], piece: usize) -> Vec<f32> {
        let mut r = Resample::new(from, to, channels).unwrap();
        let mut out = Vec::new();
        let mut sink = |block: &[f32]| {
            out.extend_from_slice(block);
            Ok(())
        };
        for part in input.chunks(piece * usize::from(channels)) {
            r.push(part, &mut sink).unwrap();
        }
        let frames = r.finish(&mut sink).unwrap();
        assert_eq!(frames as usize * usize::from(channels), out.len());
        out
    }

    #[test]
    fn a_sine_stays_where_it_is_and_as_loud() {
        // One second of 441 Hz at 44.1 kHz: a period is 100 frames, and at 48 kHz it is
        // 48000 / 441 frames.
        let input: Vec<f32> = (0..44_100)
            .map(|i| (i as f32 * std::f32::consts::TAU / 100.0).sin() * 0.5)
            .collect();
        let out = through(44_100, 48_000, 1, &input, 333);
        assert_eq!(out.len(), 48_000);
        // Away from the ends the output is the same sine, in place to a hundredth of a
        // frame: the delay of the resampler is cut off.
        let worst = (2_000..46_000)
            .map(|i| {
                let want = (i as f64 * std::f64::consts::TAU * 441.0 / 48_000.0).sin() * 0.5;
                (f64::from(out[i]) - want).abs()
            })
            .fold(0.0, f64::max);
        assert!(worst < 2e-3, "worst {worst}");
    }

    #[test]
    fn the_length_is_the_ratio_whatever_the_pieces() {
        let input: Vec<f32> = (0..2 * 10_007)
            .map(|i| ((i % 97) as f32 / 97.0) - 0.5)
            .collect();
        let a = through(48_000, 44_100, 2, &input, 4_096);
        let b = through(48_000, 44_100, 2, &input, 1);
        assert_eq!(
            a.len() as u64,
            2 * Resample::frames_for(48_000, 44_100, 10_007)
        );
        assert_eq!(a, b);
        // Shorter than one chunk, and up to twice the rate.
        let short = through(48_000, 96_000, 1, &input[..300], 300);
        assert_eq!(short.len(), 600);
        assert_eq!(through(22_050, 48_000, 1, &[], 1).len(), 0);
    }

    #[test]
    fn a_click_lands_on_its_frame() {
        let mut input = vec![0.0f32; 44_100];
        input[22_050] = 1.0;
        let out = through(44_100, 48_000, 1, &input, 1_000);
        let peak = (0..out.len())
            .max_by(|&a, &b| out[a].abs().total_cmp(&out[b].abs()))
            .unwrap();
        assert_eq!(peak, 24_000);
    }
}
