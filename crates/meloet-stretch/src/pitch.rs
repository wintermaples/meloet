//! The pitch of one voice, point by point: YIN (de Cheveigné and Kawahara, "YIN, a fundamental
//! frequency estimator for speech and music", JASA 111 (4), 2002; steps 1 to 5 of the paper),
//! on the channels mixed to one and brought down to about 24 kHz.
//!
//! Not for the audio thread.

use crate::render::RenderError;
use std::sync::atomic::{AtomicBool, Ordering};

/// Goes into the key of made sounds: another number when the same audio may give other points.
pub const PITCH_VERSION: u32 = 1;

/// The lowest and the highest pitch that is looked for, in Hz.
pub const PITCH_MIN_HZ: f64 = 70.0;
pub const PITCH_MAX_HZ: f64 = 1000.0;

/// Points per second (one every 10 ms; a little under at rates the step does not divide).
const POINTS_PER_SECOND: f64 = 100.0;
/// The integration window of the difference function, in seconds (the paper's 25 ms).
const WINDOW_SECONDS: f64 = 0.025;
/// The rate the detector works at is the file's rate over the largest whole number that keeps it
/// at or above this.
const WORK_RATE: f64 = 22_050.0;
/// The paper's absolute threshold (step 4): the first dip under it is the period.
const DIP: f32 = 0.15;
/// A point whose best dip is above this has no pitch we trust.
const UNSURE: f32 = 0.25;
/// Quieter than this (RMS, full scale 1.0) is silence: −60 dB.
const QUIET: f32 = 0.001;
/// A voice heard for fewer points than this in a row is not taken as one.
const SHORTEST_RUN: usize = 3;
/// Half the length of the low-pass before the rate is brought down, in output samples.
const LOWPASS_HALF: usize = 6;

/// The pitch of a stretch of a file.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchTrack {
    /// The file frame point 0 is about.
    pub first: i64,
    /// File frames from one point to the next.
    pub hop: usize,
    /// MIDI note numbers with their fraction (69.0 is 440 Hz); NaN where no single voice sounds.
    pub notes: Vec<f32>,
}

impl PitchTrack {
    /// The note at `frame` of the file (the nearest point); `None` where no voice sounds and
    /// outside the stretch that was looked at.
    pub fn note_at(&self, frame: f64) -> Option<f32> {
        let i = ((frame - self.first as f64) / self.hop as f64).round();
        if !(0.0..self.notes.len() as f64).contains(&i) {
            return None;
        }
        Some(self.notes[i as usize]).filter(|n| !n.is_nan())
    }
}

/// Frequency in Hz → MIDI note number with its fraction.
pub fn note_of_hz(hz: f64) -> f64 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

/// YIN on one window of samples at `rate`.
pub struct Yin {
    rate: f64,
    tau_min: usize,
    tau_max: usize,
    window: usize,
    diff: Vec<f32>,
}

impl Yin {
    pub fn new(rate: f64) -> Self {
        let tau_min = ((rate / PITCH_MAX_HZ).floor() as usize).max(2);
        let tau_max = (rate / PITCH_MIN_HZ).ceil() as usize;
        Yin {
            rate,
            tau_min,
            tau_max,
            window: (rate * WINDOW_SECONDS).round() as usize,
            // One lag past each end, for the parabola.
            diff: vec![0.0; tau_max + 2],
        }
    }

    /// How many samples [`Yin::detect`] takes.
    pub fn frame_len(&self) -> usize {
        self.window + self.tau_max + 1
    }

    /// Where in the window the answer is about: the middle of what a voice of 200 Hz compares.
    pub fn centre(&self) -> usize {
        (self.window + (self.rate / 200.0) as usize) / 2
    }

    /// The pitch of `x` (`frame_len()` samples) in Hz; `None` for silence, noise and anything
    /// that does not repeat clearly at a period inside the range.
    pub fn detect(&mut self, x: &[f32]) -> Option<f64> {
        assert_eq!(x.len(), self.frame_len(), "yin: window length");
        let w = self.window;
        let energy: f32 = x[..w].iter().map(|s| s * s).sum();
        if energy.is_nan() || energy <= QUIET * QUIET * w as f32 {
            return None;
        }
        // Step 2: the difference function.
        let head = &x[..w];
        for tau in 1..self.diff.len() {
            let lagged = &x[tau..tau + w];
            self.diff[tau] = (head.iter().zip(lagged))
                .map(|(a, b)| {
                    let d = a - b;
                    d * d
                })
                .sum();
        }
        // Steps 3 and 4: the cumulative mean normalised difference; the first dip under the
        // threshold, else the lowest of all.
        let mut sum = 0.0f32;
        let mut best: Option<(usize, f32)> = None;
        let mut found = None;
        let mut prev = 1.0f32;
        let mut falling: Option<(usize, f32)> = None;
        for tau in 1..=self.tau_max + 1 {
            sum += self.diff[tau];
            let norm = if sum > 0.0 {
                self.diff[tau] * tau as f32 / sum
            } else {
                1.0
            };
            if let Some((at, low)) = falling {
                if norm < low {
                    falling = Some((tau, norm));
                } else {
                    found = Some((at, low));
                    break;
                }
            } else if tau >= self.tau_min && tau <= self.tau_max {
                // Only a place the curve comes down to counts: under the threshold while
                // still rising from the lag before is the tail of a dip outside the range.
                if norm < DIP && norm < prev {
                    falling = Some((tau, norm));
                } else if best.is_none_or(|(_, low)| norm < low) {
                    best = Some((tau, norm));
                }
            }
            prev = norm;
        }
        let (mut tau, depth) = found.or(falling).or(best)?;
        if depth >= UNSURE || tau > self.tau_max {
            return None;
        }
        // Step 5: a parabola through the dip of the raw difference and its neighbours. The
        // dip of the raw difference may be one lag beside the normalised one.
        if tau < self.tau_max && self.diff[tau + 1] < self.diff[tau] {
            tau += 1;
        } else if tau > self.tau_min && self.diff[tau - 1] < self.diff[tau] {
            tau -= 1;
        }
        let (a, b, c) = (self.diff[tau - 1], self.diff[tau], self.diff[tau + 1]);
        if b > a || b > c {
            // Not a dip of its own: the true period is outside the range.
            return None;
        }
        let bend = a - 2.0 * b + c;
        let shift = if bend > 0.0 {
            (0.5 * (a - c) / bend).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let period = tau as f64 + f64::from(shift);
        let hz = self.rate / period;
        (PITCH_MIN_HZ..=PITCH_MAX_HZ).contains(&hz).then_some(hz)
    }
}

/// The file's rate over this is the rate the detector works at.
fn decimation(sample_rate: u32) -> usize {
    ((f64::from(sample_rate) / WORK_RATE).floor() as usize).max(1)
}

/// A low-pass (a sinc under a Hann window) for bringing the rate down by `factor`; the taps sum
/// to 1. One tap (no filter) for factor 1.
fn lowpass(factor: usize) -> Vec<f32> {
    if factor == 1 {
        return vec![1.0];
    }
    let half = (LOWPASS_HALF * factor) as isize;
    // 0.4 of the new rate, as a part of the old one.
    let cutoff = 0.4 / factor as f64;
    let mut taps: Vec<f64> = (-half..=half)
        .map(|n| {
            let x = n as f64;
            let sinc = if n == 0 {
                2.0 * cutoff
            } else {
                (2.0 * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
            };
            let hann = 0.5 + 0.5 * (std::f64::consts::PI * x / (half + 1) as f64).cos();
            sinc * hann
        })
        .collect();
    let sum: f64 = taps.iter().sum();
    for t in &mut taps {
        *t /= sum;
    }
    taps.into_iter().map(|t| t as f32).collect()
}

/// The pitch of the file from frame `from` to frame `to`, one point every 10 ms.
///
/// - `read(start, channels)`: as for [`crate::render`]: fills every channel with file frames
///   from `start` on, zeros outside the file. The reads go forward without gaps and start a
///   little before `from`.
/// - `progress(part)`: 0.0..=1.0, now and then.
/// - `cancel`: looked at between reads.
pub fn track_pitch<R, E>(
    sample_rate: u32,
    channels: usize,
    from: i64,
    to: i64,
    mut read: R,
    mut progress: impl FnMut(f64),
    cancel: &AtomicBool,
) -> Result<PitchTrack, RenderError<E>>
where
    R: FnMut(i64, &mut [&mut [f32]]) -> Result<(), E>,
{
    const CHUNK: usize = 16_384;
    let factor = decimation(sample_rate);
    let mut yin = Yin::new(f64::from(sample_rate) / factor as f64);
    let taps = lowpass(factor);
    let half = (taps.len() / 2) as i64;
    let hop_low =
        ((f64::from(sample_rate) / (POINTS_PER_SECOND * factor as f64)).round() as usize).max(1);
    let hop = hop_low * factor;
    let points = if to > from {
        ((to - from) as usize).div_ceil(hop) + 1
    } else {
        0
    };
    // Low-rate sample `m` is the file frame `origin + m * factor`, and point `i` looks at the
    // low-rate samples from `i * hop_low` on.
    let origin = from - (yin.centre() * factor) as i64;
    let mut track = PitchTrack {
        first: from,
        hop,
        notes: Vec::with_capacity(points),
    };
    if points == 0 || channels == 0 {
        return Ok(track);
    }
    let low_len = (points - 1) * hop_low + yin.frame_len();
    let file_end = origin + ((low_len - 1) * factor) as i64 + half + 1;

    let mut block: Vec<Vec<f32>> = vec![vec![0.0; CHUNK]; channels];
    // The file mixed to one channel, from frame `mono_at` on.
    let mut mono: Vec<f32> = Vec::with_capacity(CHUNK + taps.len());
    let mut mono_at = origin - half;
    let mut at = mono_at;
    // The low rate, from sample `low_at` on.
    let mut low: Vec<f32> = Vec::new();
    let mut low_at = 0usize;
    let mut made = 0usize;
    let all = (file_end - at).max(1) as f64;
    let start = at;

    while track.notes.len() < points {
        if cancel.load(Ordering::Relaxed) {
            return Err(RenderError::Cancelled);
        }
        let frames = CHUNK.min((file_end - at).max(0) as usize);
        if frames == 0 {
            // Cannot happen: `file_end` is the last frame the last point needs.
            break;
        }
        {
            let mut refs: Vec<&mut [f32]> = block.iter_mut().map(|c| &mut c[..frames]).collect();
            read(at, &mut refs).map_err(RenderError::Io)?;
        }
        let gain = 1.0 / channels as f32;
        mono.extend((0..frames).map(|i| {
            let s: f32 = block.iter().map(|c| c[i]).sum::<f32>() * gain;
            if s.is_finite() { s } else { 0.0 }
        }));
        at += frames as i64;

        // Every low-rate sample whose taps are all here.
        loop {
            let centre = origin + (made * factor) as i64;
            let first = (centre - half - mono_at) as usize;
            if made >= low_len || first + taps.len() > mono.len() {
                break;
            }
            let s: f32 = (taps.iter().zip(&mono[first..first + taps.len()]))
                .map(|(t, x)| t * x)
                .sum();
            low.push(s);
            made += 1;
        }
        let used =
            ((origin + (made * factor) as i64 - half - mono_at).max(0) as usize).min(mono.len());
        mono.drain(..used);
        mono_at += used as i64;

        loop {
            let i = track.notes.len();
            let first = i * hop_low - low_at;
            if i >= points || first + yin.frame_len() > low.len() {
                break;
            }
            let note = yin
                .detect(&low[first..first + yin.frame_len()])
                .map_or(f32::NAN, |hz| note_of_hz(hz) as f32);
            track.notes.push(note);
        }
        let used = (track.notes.len() * hop_low - low_at).min(low.len());
        low.drain(..used);
        low_at += used;
        progress(((at - start) as f64 / all).min(1.0));
    }
    drop_short_runs(&mut track.notes);
    Ok(track)
}

/// A few points of pitch among points without are more likely a consonant or a click than a voice.
fn drop_short_runs(notes: &mut [f32]) {
    let mut i = 0;
    while i < notes.len() {
        if notes[i].is_nan() {
            i += 1;
            continue;
        }
        let end = (i..notes.len())
            .find(|&j| notes[j].is_nan())
            .unwrap_or(notes.len());
        if end - i < SHORTEST_RUN {
            notes[i..end].fill(f32::NAN);
        }
        i = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    fn track_of(rate: u32, signal: &[f32]) -> PitchTrack {
        let read = |start: i64, ch: &mut [&mut [f32]]| -> Result<(), Infallible> {
            for c in ch.iter_mut() {
                for (i, s) in c.iter_mut().enumerate() {
                    let at = start + i as i64;
                    *s = if at >= 0 && (at as usize) < signal.len() {
                        signal[at as usize]
                    } else {
                        0.0
                    };
                }
            }
            Ok(())
        };
        track_pitch(
            rate,
            2,
            0,
            signal.len() as i64,
            read,
            |_| {},
            &AtomicBool::new(false),
        )
        .unwrap()
    }

    fn saw(rate: u32, hz: f64, seconds: f64) -> Vec<f32> {
        let n = (f64::from(rate) * seconds) as usize;
        // A few harmonics, falling: near enough to a sung vowel and without aliasing.
        (0..n)
            .map(|i| {
                let t = i as f64 / f64::from(rate);
                (1..=8)
                    .map(|h| {
                        (2.0 * std::f64::consts::PI * hz * f64::from(h) * t).sin() / f64::from(h)
                    })
                    .sum::<f64>() as f32
                    * 0.3
            })
            .collect()
    }

    fn noise(n: usize, seed: u64) -> Vec<f32> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.6
            })
            .collect()
    }

    #[test]
    fn steady_notes_are_found_within_a_few_cents_at_every_rate() {
        for rate in [22_050u32, 44_100, 48_000, 96_000] {
            for hz in [70.5, 82.4, 110.0, 196.0, 261.63, 440.0, 660.0, 987.8] {
                let track = track_of(rate, &saw(rate, hz, 0.5));
                let want = note_of_hz(hz) as f32;
                // The ends look into the silence around the file.
                let inner = &track.notes[5..track.notes.len() - 6];
                assert!(inner.len() > 30);
                for (i, n) in inner.iter().enumerate() {
                    assert!(
                        (n - want).abs() < 0.03,
                        "{hz} Hz at {rate}: point {i} is {n}, not {want}: {:?}",
                        track.notes
                    );
                }
            }
        }
    }

    #[test]
    fn a_sine_is_found_too() {
        let rate = 48_000;
        let signal: Vec<f32> = (0..24_000)
            .map(|i| (2.0 * std::f32::consts::PI * 311.1 * i as f32 / rate as f32).sin() * 0.5)
            .collect();
        let track = track_of(rate, &signal);
        let want = note_of_hz(311.1) as f32;
        assert!(track.notes[5..40].iter().all(|n| (n - want).abs() < 0.03));
    }

    #[test]
    fn noise_and_silence_have_no_pitch() {
        let track = track_of(48_000, &noise(48_000, 7));
        assert!(track.notes.len() >= 100);
        assert!(track.notes.iter().all(|n| n.is_nan()));
        let track = track_of(48_000, &vec![0.0; 48_000]);
        assert!(track.notes.iter().all(|n| n.is_nan()));
        // A voice far under the noise it sits in.
        let mut buried = noise(48_000, 9);
        for (b, s) in buried.iter_mut().zip(saw(48_000, 220.0, 1.0)) {
            *b += s * 0.05;
        }
        assert!(track_of(48_000, &buried).notes.iter().all(|n| n.is_nan()));
    }

    #[test]
    fn a_tone_above_the_range_is_not_given_a_wrong_note_in_it_when_it_is_a_sine() {
        // 3 kHz repeats every 8 samples at 24 kHz, so also every 24, 32, …: those dips are
        // real periods of it. What must not happen is a note it does not repeat at.
        let rate = 48_000;
        let signal: Vec<f32> = (0..24_000)
            .map(|i| (2.0 * std::f32::consts::PI * 3000.0 * i as f32 / rate as f32).sin() * 0.5)
            .collect();
        for n in track_of(rate, &signal).notes.iter().filter(|n| !n.is_nan()) {
            let hz = 440.0 * 2f64.powf((f64::from(*n) - 69.0) / 12.0);
            let times = 3000.0 / hz;
            assert!((times - times.round()).abs() < 0.02, "{hz}");
        }
    }

    #[test]
    fn the_points_are_where_the_sound_is() {
        // Silence, then a note from 0.5 s on: the first point with a pitch is within 20 ms.
        let rate = 48_000u32;
        let mut signal = vec![0.0; 24_000];
        signal.extend(saw(rate, 220.0, 0.5));
        let track = track_of(rate, &signal);
        assert_eq!(track.hop, 480);
        assert_eq!(track.first, 0);
        let first = track.notes.iter().position(|n| !n.is_nan()).unwrap();
        assert!((48..=52).contains(&first), "{first}");
        assert_eq!(track.note_at(0.0), None);
        assert!(track.note_at(30_000.0).is_some());
        assert_eq!(track.note_at(-1000.0), None);
        assert_eq!(track.note_at(1e9), None);
    }

    #[test]
    fn a_glide_is_followed() {
        let rate = 48_000u32;
        let n = 48_000usize;
        let mut phase = 0.0f64;
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                let note = 57.0 + 5.0 * i as f64 / n as f64;
                let hz = 440.0 * 2f64.powf((note - 69.0) / 12.0);
                phase += hz / f64::from(rate);
                ((2.0 * std::f64::consts::PI * phase).sin() * 0.4
                    + (4.0 * std::f64::consts::PI * phase).sin() * 0.2) as f32
            })
            .collect();
        let track = track_of(rate, &signal);
        for (i, got) in track.notes.iter().enumerate().take(95).skip(5) {
            let want = 57.0 + 5.0 * (i * track.hop) as f32 / n as f32;
            assert!((got - want).abs() < 0.1, "point {i}: {got} for {want}");
        }
    }

    #[test]
    fn the_same_audio_gives_the_same_points_however_it_is_cut_out() {
        // Points of a stretch that starts later sit on the same frames and say the same
        // (the low rate is laid from `from`, so only near enough, not to the bit).
        let rate = 44_100u32;
        let signal = saw(rate, 147.0, 1.0);
        let whole = track_of(rate, &signal);
        assert_eq!(whole.hop, 442);
        let read = |start: i64, ch: &mut [&mut [f32]]| -> Result<(), Infallible> {
            for c in ch.iter_mut() {
                for (i, s) in c.iter_mut().enumerate() {
                    let at = start + i as i64;
                    *s = signal.get(at as usize).copied().unwrap_or(0.0);
                }
            }
            Ok(())
        };
        let part =
            track_pitch(rate, 1, 4420, 30_000, read, |_| {}, &AtomicBool::new(false)).unwrap();
        for (i, n) in part.notes.iter().enumerate() {
            assert!((n - whole.notes[i + 10]).abs() < 0.02);
        }
    }

    #[test]
    fn it_stops_when_told() {
        let cancel = AtomicBool::new(true);
        let read = |_: i64, _: &mut [&mut [f32]]| -> Result<(), Infallible> { Ok(()) };
        let got = track_pitch(48_000, 1, 0, 48_000, read, |_| {}, &cancel);
        assert_eq!(got, Err(RenderError::Cancelled));
    }

    #[test]
    fn short_runs_go() {
        let mut notes = [60.0, f32::NAN, 61.0, 61.0, f32::NAN, 62.0, 62.0, 62.0];
        drop_short_runs(&mut notes);
        assert!(notes[..5].iter().all(|n| n.is_nan()));
        assert_eq!(&notes[5..], &[62.0, 62.0, 62.0]);
    }
}
