//! The waveform of an exported file for the dialog 「書き出しました」 (round 12, E9): the file
//! cut into [`EXPORT_WAVE_BARS`] parts of the same length, each the largest absolute sample
//! of its part as 0..=255 (1.0 and more is 255).

use meloet_protocol::EXPORT_WAVE_BARS;

#[derive(Debug)]
pub(crate) struct WaveBars {
    frames: u64,
    at: u64,
    bars: [f32; EXPORT_WAVE_BARS],
}

impl WaveBars {
    /// For a file of `frames` frames.
    pub(crate) fn new(frames: u64) -> WaveBars {
        WaveBars {
            frames: frames.max(1),
            at: 0,
            bars: [0.0; EXPORT_WAVE_BARS],
        }
    }

    /// Interleaved stereo, in the order of the file. Samples that are not finite count as 0.
    pub(crate) fn feed(&mut self, samples: &[f32]) {
        let bars = EXPORT_WAVE_BARS as u64;
        for frame in samples.as_chunks::<2>().0 {
            let bar = (self.at * bars / self.frames).min(bars - 1) as usize;
            let loud = frame[0].abs().max(frame[1].abs());
            if loud > self.bars[bar] && loud.is_finite() {
                self.bars[bar] = loud;
            }
            self.at += 1;
        }
    }

    pub(crate) fn finish(&self) -> Vec<u8> {
        (self.bars.iter())
            .map(|b| (b.min(1.0) * 255.0).round() as u8)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_as_many_bars_as_the_protocol_says_and_each_holds_the_largest_of_its_part() {
        // 400 frames: two frames for each bar; the first frame of a bar is quiet, the second
        // goes up with the bar, on the right channel.
        let mut samples = Vec::new();
        for bar in 0..EXPORT_WAVE_BARS {
            samples.extend_from_slice(&[0.01, -0.01]);
            samples.extend_from_slice(&[0.0, -(bar as f32) / 199.0]);
        }
        let mut wave = WaveBars::new(400);
        // In pieces that do not end on a bar, as `convert` reads.
        for piece in samples.chunks(6) {
            wave.feed(piece);
        }
        let bars = wave.finish();
        assert_eq!(bars.len(), EXPORT_WAVE_BARS);
        assert_eq!(bars[0], 3);
        assert_eq!(bars[199], 255);
        assert_eq!(bars[100], (100.0f32 / 199.0 * 255.0).round() as u8);
        assert!(bars.windows(2).skip(1).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn over_0_db_is_the_top_and_what_is_not_a_number_is_nothing() {
        let mut wave = WaveBars::new(2);
        wave.feed(&[f32::NAN, f32::INFINITY, 1.7, -3.0]);
        let bars = wave.finish();
        assert_eq!(bars.len(), EXPORT_WAVE_BARS);
        assert_eq!(bars[0], 0);
        assert_eq!(bars[EXPORT_WAVE_BARS / 2], 255);
        assert!(bars.iter().filter(|b| **b != 0).count() == 1);
    }

    #[test]
    fn a_file_shorter_than_the_bars_and_frames_past_the_count_stay_in_range() {
        let mut wave = WaveBars::new(3);
        wave.feed(&[0.5; 2 * 5]);
        let bars = wave.finish();
        assert_eq!(bars.len(), EXPORT_WAVE_BARS);
        assert_eq!(bars[EXPORT_WAVE_BARS - 1], 128);
    }
}
