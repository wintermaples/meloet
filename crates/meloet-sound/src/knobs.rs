//! The knobs of a sound that comes with meloet (round 12, E3). [`Knobbed`] goes around every
//! such instrument of a track: it holds 「明るさ」, a filter on what the instrument puts out,
//! and hands 「立ち上がり」, 「のび」 and 「ゆれ」 to the instrument's voices
//! (`meloet_engine::knobs`). The filter's state is the track's own: the samples of a sound
//! are shared by its tracks, and nothing is kept with them.
//!
//! 「明るさ」 below 50 % closes a low-pass (400 Hz at 0 %), above lifts a high shelf from
//! 4 kHz (+9 dB at 100 %). At 50 %, once it has got there, nothing is done to the sound.

use meloet_engine::knobs::{Glide, KNOB_BRIGHTNESS, KNOB_GLIDE_S, KNOBS, knob_place};
use meloet_engine::{Instrument, InstrumentEvent, SongPlace};

use crate::effects::dsp::{Svf, SvfCoefs, db_to_gain};

/// The filter's settings are worked out once per this many frames.
const CHUNK: usize = 32;
const CLOSED_HZ: f32 = 400.0;
const OPEN_HZ: f32 = 20_000.0;
const SHELF_HZ: f32 = 4_000.0;
const SHELF_DB: f32 = 9.0;
/// 1 / Q for a Q of 1/sqrt(2).
const K: f32 = std::f32::consts::SQRT_2;
/// Lane points of 「明るさ」 kept for one block; more take the place of the last.
const POINTS: usize = 16;

/// How much of the low-pass and of the shelf there is at a place: both 0 at 0.5.
fn weights(place: f32) -> (f32, f32) {
    if place < 0.5 {
        let closed = (0.5 - place) * 2.0;
        ((10.0 * closed).min(1.0), 0.0)
    } else if place > 0.5 {
        (0.0, db_to_gain(SHELF_DB * (place - 0.5) * 2.0) - 1.0)
    } else {
        (0.0, 0.0)
    }
}

fn svf_g(hz: f32, sample_rate: f32) -> f32 {
    (std::f32::consts::PI * hz.min(0.45 * sample_rate) / sample_rate).tan()
}

/// An instrument that comes with meloet, with its knobs.
pub struct Knobbed<I> {
    inner: I,
    bright: Glide,
    /// Lane points of the next block: frame and place.
    points: [(u32, f32); POINTS],
    count: usize,
    low: [Svf; 2],
    shelf: [Svf; 2],
    shelf_coefs: SvfCoefs,
    sample_rate: f32,
    glide_frames: u32,
    fresh: bool,
}

impl<I: Instrument> Knobbed<I> {
    pub fn new(inner: I) -> Knobbed<I> {
        let mut knobbed = Knobbed {
            inner,
            bright: Glide::at(0.5),
            points: [(0, 0.5); POINTS],
            count: 0,
            low: [Svf::default(); 2],
            shelf: [Svf::default(); 2],
            shelf_coefs: SvfCoefs::default(),
            sample_rate: 48_000.0,
            glide_frames: 960,
            fresh: true,
        };
        knobbed.set_rate(48_000.0);
        knobbed
    }

    pub fn inner(&self) -> &I {
        &self.inner
    }

    fn set_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.glide_frames = (KNOB_GLIDE_S * sample_rate).round().max(1.0) as u32;
        self.shelf_coefs = SvfCoefs::new(svf_g(SHELF_HZ, sample_rate), K);
    }

    fn settle(&mut self) {
        self.bright.snap();
        self.count = 0;
        self.low = [Svf::default(); 2];
        self.shelf = [Svf::default(); 2];
        self.fresh = true;
    }

    fn aim(&mut self, place: f32) {
        let frames = if self.fresh { 0 } else { self.glide_frames };
        self.bright.set(place, frames);
    }

    /// The lane points before frame `end` take effect.
    fn take_points(&mut self, end: usize) {
        let due = self.points[..self.count]
            .iter()
            .take_while(|p| (p.0 as usize) < end)
            .count();
        if let Some(&(_, place)) = self.points[..due].last() {
            self.aim(place);
        }
        self.points.copy_within(due..self.count, 0);
        self.count -= due;
    }

    fn filter(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len();
        let mut at = 0;
        while at < frames {
            let n = CHUNK.min(frames - at);
            self.take_points(at + n);
            let (low_from, shelf_from) = weights(self.bright.now());
            let place = self.bright.advance(n as u32);
            let (low_to, shelf_to) = weights(place);
            let inv = 1.0 / n as f32;
            let sides = [&mut left[at..at + n], &mut right[at..at + n]];
            if low_from > 0.0 || low_to > 0.0 {
                let cutoff = CLOSED_HZ * (OPEN_HZ / CLOSED_HZ).powf(1.0 - (0.5 - place) * 2.0);
                let coefs = SvfCoefs::new(svf_g(cutoff, self.sample_rate), K);
                let step = (low_to - low_from) * inv;
                for (side, svf) in sides.into_iter().zip(&mut self.low) {
                    let mut w = low_from;
                    for x in side.iter_mut() {
                        w += step;
                        let lp = svf.tick(&coefs, *x).1;
                        *x += w * (lp - *x);
                    }
                }
            } else {
                self.low = [Svf::default(); 2];
            }
            let sides = [&mut left[at..at + n], &mut right[at..at + n]];
            if shelf_from > 0.0 || shelf_to > 0.0 {
                let step = (shelf_to - shelf_from) * inv;
                for (side, svf) in sides.into_iter().zip(&mut self.shelf) {
                    let mut g = shelf_from;
                    for x in side.iter_mut() {
                        g += step;
                        *x += g * svf.high(&self.shelf_coefs, *x);
                    }
                }
            } else {
                self.shelf = [Svf::default(); 2];
            }
            at += n;
        }
        for svf in self.low.iter_mut().chain(&mut self.shelf) {
            svf.flush();
        }
        if !self.low.iter().chain(&self.shelf).all(Svf::is_finite) {
            self.low = [Svf::default(); 2];
            self.shelf = [Svf::default(); 2];
        }
    }

    fn set(&mut self, knob: u32, place: f32) {
        if knob == KNOB_BRIGHTNESS {
            if let Some(place) = knob_place(place) {
                self.aim(place);
            }
        } else if (knob as usize) < KNOBS {
            self.inner.set_knob(knob, place);
        }
    }
}

impl<I: Instrument> Instrument for Knobbed<I> {
    fn prepare(&mut self, sample_rate: f32, max_frames: usize) {
        self.inner.prepare(sample_rate, max_frames);
        self.set_rate(sample_rate);
        self.settle();
    }

    fn event(&mut self, ev: InstrumentEvent) {
        self.inner.event(ev);
    }

    fn song_note_on(&mut self, note: u8, velocity: u8, place: SongPlace<'_>) {
        self.inner.song_note_on(note, velocity, place);
    }

    fn song_pedal(&mut self, value: u8, place: SongPlace<'_>) {
        self.inner.song_pedal(value, place);
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.inner.render(left, right);
        if self.count > 0 || !self.bright.rests_at(0.5) {
            self.filter(left, right);
        } else {
            self.low = [Svf::default(); 2];
            self.shelf = [Svf::default(); 2];
        }
        self.count = 0;
        self.fresh = false;
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.settle();
    }

    fn ask(&mut self, frames: usize) {
        self.inner.ask(frames);
    }

    fn latency(&self) -> u32 {
        self.inner.latency()
    }

    fn set_param_at(&mut self, frame: u32, id: u32, value: f32) {
        if id != KNOB_BRIGHTNESS {
            self.set(id, value);
            return;
        }
        let Some(place) = knob_place(value) else {
            return;
        };
        if self.count == POINTS {
            self.count -= 1;
        }
        self.points[self.count] = (frame, place);
        self.count += 1;
    }

    fn set_knob(&mut self, knob: u32, place: f32) {
        self.set(knob, place);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_weights_are_nothing_at_the_middle_and_full_at_the_ends() {
        assert_eq!(weights(0.5), (0.0, 0.0));
        assert_eq!(weights(0.0), (1.0, 0.0));
        let (low, shelf) = weights(1.0);
        assert_eq!(low, 0.0);
        assert!((20.0 * (1.0 + shelf).log10() - SHELF_DB).abs() < 1e-3);
        assert!(weights(0.499).0 > 0.0 && weights(0.501).1 > 0.0);
    }
}
