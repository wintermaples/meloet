//! Measuring the way round: a click goes out, comes back through the input, and is found
//! again in what was heard (docs/architecture.md 22-5).
//!
//! The click is a short sweep, so that what comes back lines up with what was sent at one
//! place only; where the two agree best is read finer than a frame with a parabola through
//! the best place and its two neighbours.

use std::f32::consts::TAU;

/// The click's largest sample: −12 dBFS. It lasts 4 ms, so in headphones it is a soft tick,
/// about as loud as the metronome; an input that hears it at a hundredth of that still
/// finds it.
pub const PROBE_PEAK: f32 = 0.25;
const PROBE_S: f32 = 0.004;
const PROBE_FROM_HZ: f32 = 1_000.0;
const PROBE_TO_HZ: f32 = 6_000.0;

/// What comes back must agree with the click at least this well to count as the click.
const CLEAR_ENOUGH: f32 = 0.5;
/// And be at least this loud, as a share of what was sent (−50 dB).
const LOUD_ENOUGH: f32 = 0.003;

/// The click at `sample_rate`: a sweep from 1 kHz to 6 kHz under a raised cosine.
pub fn probe_wave(sample_rate: u32) -> Vec<f32> {
    let rate = sample_rate.max(1) as f32;
    let len = ((PROBE_S * rate) as usize).max(8);
    let mut phase = 0.0f32;
    (0..len)
        .map(|i| {
            let along = i as f32 / len as f32;
            let hz = PROBE_FROM_HZ + (PROBE_TO_HZ - PROBE_FROM_HZ) * along;
            phase = (phase + hz / rate).fract();
            let window = 0.5 - 0.5 * (TAU * (i as f32 + 0.5) / len as f32).cos();
            PROBE_PEAK * window * (TAU * phase).sin()
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeFound {
    /// Where in `heard` the click begins, in frames, with the fraction.
    pub at: f64,
    /// How loud it came back, as a share of what was sent. Negative: the way round turns
    /// the wave over.
    pub level: f32,
    /// How well what is there agrees with the click, 0 to 1.
    pub clear: f32,
}

/// Looks for `wave` in `heard` (one channel). `None`: nothing there is clear and loud
/// enough to be it.
pub fn find_probe(heard: &[f32], wave: &[f32]) -> Option<ProbeFound> {
    if wave.is_empty() || heard.len() < wave.len() {
        return None;
    }
    let power: f64 = wave.iter().map(|w| f64::from(*w).powi(2)).sum();
    let places = heard.len() - wave.len() + 1;
    let agree = |k: usize| -> f64 {
        (heard[k..k + wave.len()].iter().zip(wave))
            .map(|(h, w)| f64::from(*h) * f64::from(*w))
            .sum()
    };
    let (mut best, mut best_at) = (0.0f64, 0usize);
    for k in 0..places {
        let a = agree(k);
        if a.abs() > best.abs() {
            (best, best_at) = (a, k);
        }
    }
    let heard_power: f64 = (heard[best_at..best_at + wave.len()].iter())
        .map(|h| f64::from(*h).powi(2))
        .sum();
    if power == 0.0 || heard_power == 0.0 {
        return None;
    }
    let clear = (best.abs() / (power * heard_power).sqrt()) as f32;
    let level = (best / power) as f32;
    if clear < CLEAR_ENOUGH || level.abs() < LOUD_ENOUGH {
        return None;
    }
    let mut at = best_at as f64;
    if best_at > 0 && best_at + 1 < places {
        let (before, here, after) = (
            agree(best_at - 1).abs(),
            best.abs(),
            agree(best_at + 1).abs(),
        );
        let bend = before - 2.0 * here + after;
        if bend < 0.0 {
            at += (0.5 * (before - after) / bend).clamp(-0.5, 0.5);
        }
    }
    Some(ProbeFound { at, level, clear })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbeVerdict {
    /// The middle one of the clicks' answers, in microseconds.
    Measured(f64),
    /// Too few clicks came back.
    TooQuiet,
    /// The answers are more than a frame from their middle.
    Unsteady,
}

/// What the clicks say together. `answers`: the way round of each click sent, in
/// microseconds, `None` for one that was not found.
pub fn judge_probes(answers: &[Option<f64>], sample_rate: u32) -> ProbeVerdict {
    let mut found: Vec<f64> = answers.iter().flatten().copied().collect();
    if found.len() * 2 <= answers.len() {
        return ProbeVerdict::TooQuiet;
    }
    found.sort_unstable_by(f64::total_cmp);
    let middle = found[found.len() / 2];
    let frame_us = 1e6 / f64::from(sample_rate.max(1));
    if found.iter().any(|x| (x - middle).abs() > frame_us) {
        return ProbeVerdict::Unsteady;
    }
    ProbeVerdict::Measured(middle)
}

/// When each frame of a stream was, from the times its blocks came with.
///
/// The time of one block has the jitter of the moment its cycle woke; the count of frames
/// has none. So every block says when a frame was, counting frames from its own time, and
/// the middle one of the answers is taken (docs/architecture.md 22-6: the head of a
/// recording is placed by the first 32 blocks, not by one).
#[derive(Debug, Clone)]
pub struct FrameClock {
    sample_rate: f64,
    /// (first frame, its time in microseconds) of the blocks kept.
    blocks: Vec<(u64, f64)>,
    keep: usize,
    next: usize,
}

impl FrameClock {
    /// Keeps the newest `keep` blocks.
    pub fn new(sample_rate: u32, keep: usize) -> FrameClock {
        FrameClock {
            sample_rate: f64::from(sample_rate.max(1)),
            blocks: Vec::with_capacity(keep.max(1)),
            keep: keep.max(1),
            next: 0,
        }
    }

    pub fn note(&mut self, first_frame: u64, t_us: f64) {
        if self.blocks.len() < self.keep {
            self.blocks.push((first_frame, t_us));
        } else {
            self.blocks[self.next] = (first_frame, t_us);
            self.next = (self.next + 1) % self.keep;
        }
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Which frame was at `t_us`, with the fraction: [`FrameClock::time_of`] the other way
    /// round. `None` before any block.
    pub fn frame_at(&self, t_us: f64) -> Option<f64> {
        let mut votes: Vec<f64> = (self.blocks.iter())
            .map(|(first, at_us)| *first as f64 + (t_us - at_us) * self.sample_rate / 1e6)
            .collect();
        votes.sort_unstable_by(f64::total_cmp);
        votes.get(votes.len() / 2).copied()
    }

    /// When `frame` was, in microseconds. `None` before any block.
    pub fn time_of(&self, frame: f64) -> Option<f64> {
        let mut votes: Vec<f64> = (self.blocks.iter())
            .map(|(first, t_us)| t_us + (frame - *first as f64) * 1e6 / self.sample_rate)
            .collect();
        votes.sort_unstable_by(f64::total_cmp);
        votes.get(votes.len() / 2).copied()
    }
}

/// How long after a click reaches the DAC it is still looked for in what the input heard.
pub const PROBE_LISTEN_S: f64 = 0.2;

/// What the input heard around one click: the blocks as their headers say, and one channel
/// of their frames, the first of them frame `first_frame` of the input's stream.
pub struct ProbeHeard<'a> {
    pub blocks: &'a [crate::InputBlock],
    pub first_frame: u64,
    pub heard: &'a [f32],
    pub sample_rate: u32,
}

impl ProbeHeard<'_> {
    /// The way round of a click that reaches the DAC at `left_us`
    /// ([`crate::TransportClock::time_of_frame`]), in microseconds: `c − p` of
    /// docs/architecture.md 22-5. `None`: it did not come back.
    ///
    /// `out_delay_us`: what the PC says lies between the engine writing a frame and the DAC
    /// ([`crate::Meters::out_delay_us`]). When the PC says more than there is (a monitor of
    /// the output, some Bluetooth outputs), the click is back before `left_us` and the
    /// answer is negative; it cannot be back before the engine wrote it.
    pub fn way_round(
        &self,
        wave: &[f32],
        left_us: f64,
        out_delay_us: f64,
    ) -> Option<(f64, ProbeFound)> {
        let mut clock = FrameClock::new(self.sample_rate, self.blocks.len().max(1));
        for block in self.blocks {
            clock.note(block.first_frame, block.first_frame_us(self.sample_rate));
        }
        let rate = f64::from(self.sample_rate.max(1));
        let zero_us = clock.time_of(self.first_frame as f64)?;
        let frames = |us: f64| (us * rate / 1e6).max(0.0) as usize;
        // A little room for the times' jitter.
        let from = frames(left_us - out_delay_us.max(0.0) - 2_000.0 - zero_us);
        let to = frames(left_us - zero_us + PROBE_LISTEN_S * 1e6).min(self.heard.len());
        let found = find_probe(self.heard.get(from..to)?, wave)?;
        let back_us = clock.time_of(self.first_frame as f64 + from as f64 + found.at)?;
        Some((back_us - left_us, found))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard_with(wave: &[f32], at: f64, level: f32, len: usize) -> Vec<f32> {
        // The wave moved by a fraction of a frame, by a straight line between its samples.
        let whole = at.floor() as usize;
        let part = (at - at.floor()) as f32;
        let mut heard = vec![0.0f32; len];
        for (i, w) in wave.iter().enumerate() {
            heard[whole + i] += level * w * (1.0 - part);
            heard[whole + i + 1] += level * w * part;
        }
        heard
    }

    #[test]
    fn the_click_is_short_and_soft() {
        for rate in [44_100, 48_000, 96_000] {
            let wave = probe_wave(rate);
            assert_eq!(wave.len(), (0.004 * rate as f32) as usize);
            let peak = wave.iter().fold(0.0f32, |p, w| p.max(w.abs()));
            assert!(peak <= PROBE_PEAK && peak > PROBE_PEAK * 0.8, "{peak}");
            assert!(wave[0].abs() < 0.001 && wave[wave.len() - 1].abs() < 0.001);
        }
    }

    #[test]
    fn a_click_is_found_where_it_is_finer_than_a_frame() {
        let wave = probe_wave(48_000);
        for (at, level) in [(100.0, 1.0), (731.25, 0.5), (2_000.5, 0.02), (40.75, -0.7)] {
            let heard = heard_with(&wave, at, level, 4_800);
            let found = find_probe(&heard, &wave).expect("found");
            assert!((found.at - at).abs() < 0.1, "{at}: {found:?}");
            assert!((found.level - level).abs() < level.abs() * 0.1, "{found:?}");
            assert!(found.clear > 0.9);
        }
    }

    #[test]
    fn silence_and_noise_are_not_a_click() {
        let wave = probe_wave(48_000);
        assert_eq!(find_probe(&vec![0.0; 4_800], &wave), None);
        assert_eq!(find_probe(&wave[..10], &wave), None);
        let mut seed = 1u32;
        let noise: Vec<f32> = (0..4_800)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1 << 24) as f32 - 0.5
            })
            .collect();
        assert_eq!(find_probe(&noise, &wave), None);
        // Far too soft: a thousandth of what was sent.
        let soft = heard_with(&wave, 300.0, 0.001, 4_800);
        assert_eq!(find_probe(&soft, &wave), None);
    }

    #[test]
    fn five_clicks_give_their_middle_or_say_why_not() {
        let frame = 1e6 / 48_000.0;
        let near = |d: f64| Some(4_000.0 + d * frame);
        assert_eq!(
            judge_probes(
                &[near(0.2), near(-0.3), near(0.0), near(0.9), near(-0.9)],
                48_000
            ),
            ProbeVerdict::Measured(4_000.0)
        );
        assert_eq!(
            judge_probes(
                &[near(0.0), near(0.1), near(0.0), near(1.5), near(0.0)],
                48_000
            ),
            ProbeVerdict::Unsteady
        );
        assert_eq!(
            judge_probes(&[near(0.0), None, None, near(0.0), None], 48_000),
            ProbeVerdict::TooQuiet
        );
        assert_eq!(
            judge_probes(&[near(0.0), None, near(0.1), near(0.0), None], 48_000),
            ProbeVerdict::Measured(4_000.0)
        );
        assert_eq!(judge_probes(&[], 48_000), ProbeVerdict::TooQuiet);
    }

    /// What an input of `quantum` frames a block hears of a click that is in it at frame
    /// `back_at`; the stream's frame 0 was captured at 1 s.
    fn way_round_of(quantum: u32, back_at: f64, left_us: f64, out_delay_us: f64) -> Option<f64> {
        let wave = probe_wave(48_000);
        let heard = heard_with(&wave, back_at, 0.5, 24_000);
        let block_us = f64::from(quantum) * 1e6 / 48_000.0;
        let blocks: Vec<crate::InputBlock> = (0..24_000 / u64::from(quantum))
            .map(|n| crate::InputBlock {
                first_frame: n * u64::from(quantum),
                frames: quantum,
                capture_us: (1e6 + (n + 1) as f64 * block_us).round() as u64,
                callback_us: 0,
                dropped: false,
            })
            .collect();
        let all = ProbeHeard {
            blocks: &blocks,
            first_frame: 0,
            heard: &heard,
            sample_rate: 48_000,
        };
        all.way_round(&wave, left_us, out_delay_us)
            .map(|(us, _)| us)
    }

    #[test]
    fn a_click_back_before_the_pc_says_it_left_is_measured() {
        let frame_us = 1e6 / 48_000.0;
        for quantum in [64u32, 256, 1024] {
            // The PC says three blocks from the engine to the DAC; the click is at the DAC,
            // it says, at frame 9600 of the input.
            let out_delay_us = 3.0 * f64::from(quantum) * frame_us;
            let left_us = 1e6 + 9_600.0 * frame_us;
            // One block early: a monitor of the output.
            let early = 9_600.0 - f64::from(quantum);
            let x = way_round_of(quantum, early, left_us, out_delay_us).expect("found");
            assert!(
                (x + f64::from(quantum) * frame_us).abs() < 0.2 * frame_us,
                "{quantum}: {x}"
            );
            // On time and late, as before.
            for late in [0.0, 212.5] {
                let x =
                    way_round_of(quantum, 9_600.0 + late, left_us, out_delay_us).expect("found");
                assert!(
                    (x - late * frame_us).abs() < 0.2 * frame_us,
                    "{quantum}: {x}"
                );
            }
            // Before the engine wrote it: not this click.
            let before = 9_600.0 - 3.0 * f64::from(quantum) - 400.0;
            assert_eq!(way_round_of(quantum, before, left_us, out_delay_us), None);
            // Later than is listened for.
            let after = 9_600.0 + PROBE_LISTEN_S * 48_000.0 + 10.0;
            assert_eq!(way_round_of(quantum, after, left_us, out_delay_us), None);
        }
    }

    #[test]
    fn the_frame_clock_takes_the_middle_of_what_the_blocks_say() {
        let mut clock = FrameClock::new(48_000, 4);
        assert_eq!(clock.time_of(0.0), None);
        // 480 frames are 10 ms. One block woke late.
        clock.note(0, 1_000_000.0);
        clock.note(480, 1_010_000.0);
        clock.note(960, 1_020_900.0);
        assert_eq!(clock.time_of(0.0), Some(1_000_000.0));
        assert_eq!(clock.time_of(1_440.0), Some(1_030_000.0));
        // Only the newest four are kept.
        for n in 3..8u64 {
            clock.note(n * 480, 2_000_000.0 + n as f64 * 10_000.0);
        }
        assert_eq!(clock.len(), 4);
        assert_eq!(clock.time_of(0.0), Some(2_000_000.0));
        assert_eq!(clock.frame_at(2_010_000.0), Some(480.0));
    }
}
