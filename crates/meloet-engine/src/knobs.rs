//! The knobs of a sound that comes with meloet (round 12, E3): what 「立ち上がり」, 「のび」 and
//! 「ゆれ」 do to the voices of an instrument. 「明るさ」 is a filter around the instrument
//! (`meloet_sound::knobs`). A knob is a place 0..=1 (percent / 100), as a lane has it; its
//! number is its place in the parameters panel ([`KNOB_BRIGHTNESS`] …).
//!
//! At the place each knob starts ([`KNOB_DEFAULTS`]) nothing here changes a voice: the
//! instruments take their own times as they did before the knobs, to the sample.

use std::f32::consts::TAU;

pub const KNOB_BRIGHTNESS: u32 = 0;
pub const KNOB_ATTACK: u32 = 1;
pub const KNOB_RELEASE: u32 = 2;
pub const KNOB_VIBRATO: u32 = 3;
pub const KNOBS: usize = 4;
/// Where each knob starts: 50 %, 50 %, 50 %, 0 %.
pub const KNOB_DEFAULTS: [f32; KNOBS] = [0.5, 0.5, 0.5, 0.0];

/// A knob moves to a new place in a straight line over this time.
pub const KNOB_GLIDE_S: f32 = 0.02;
/// 「立ち上がり」 at 100 % adds this to the attack.
const ATTACK_ADD_S: f32 = 0.4;
const VIBRATO_HZ: f64 = 5.5;
/// 「ゆれ」 at 100 %.
const VIBRATO_CENTS: f32 = 50.0;

/// A place that goes to its target in a straight line over a fixed number of frames, and
/// lands on it exactly.
#[derive(Clone, Copy, Debug)]
pub struct Glide {
    now: f32,
    target: f32,
    step: f32,
    left: u32,
}

impl Glide {
    pub const fn at(place: f32) -> Glide {
        Glide {
            now: place,
            target: place,
            step: 0.0,
            left: 0,
        }
    }

    pub fn set(&mut self, target: f32, frames: u32) {
        if target == self.target {
            return;
        }
        self.target = target;
        if frames == 0 {
            self.snap();
        } else {
            self.step = (target - self.now) / frames as f32;
            self.left = frames;
        }
    }

    pub fn snap(&mut self) {
        self.now = self.target;
        self.left = 0;
    }

    pub fn now(&self) -> f32 {
        self.now
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    /// Still and at `place`.
    pub fn rests_at(&self, place: f32) -> bool {
        self.now == place && self.target == place
    }

    /// The place `frames` later.
    pub fn advance(&mut self, frames: u32) -> f32 {
        if self.left > frames {
            self.left -= frames;
            self.now += self.step * frames as f32;
        } else {
            self.snap();
        }
        self.now
    }
}

/// A place from outside: `None` for one that is not a number.
pub fn knob_place(place: f32) -> Option<f32> {
    (!place.is_nan()).then(|| place.clamp(0.0, 1.0))
}

/// What an instrument keeps of 「立ち上がり」, 「のび」 and 「ゆれ」. The attack and the
/// release are read when a voice starts and when it lets go; the depth of the vibrato glides.
/// A value set before the first block after [`VoiceKnobs::settle`] is taken at once, so that
/// an instrument made anew sounds at its knobs from its first frame.
#[derive(Clone, Copy, Debug)]
pub struct VoiceKnobs {
    attack: f32,
    release: f32,
    vibrato: Glide,
    glide_frames: u32,
    fresh: bool,
}

impl Default for VoiceKnobs {
    fn default() -> Self {
        VoiceKnobs::new()
    }
}

impl VoiceKnobs {
    pub const fn new() -> VoiceKnobs {
        VoiceKnobs {
            attack: KNOB_DEFAULTS[KNOB_ATTACK as usize],
            release: KNOB_DEFAULTS[KNOB_RELEASE as usize],
            vibrato: Glide::at(KNOB_DEFAULTS[KNOB_VIBRATO as usize]),
            glide_frames: 960,
            fresh: true,
        }
    }

    /// In `prepare`: the glide's length at this rate, and [`VoiceKnobs::settle`].
    pub fn prepare(&mut self, sample_rate: f32) {
        self.glide_frames = (KNOB_GLIDE_S * sample_rate).round().max(1.0) as u32;
        self.settle();
    }

    /// In `prepare` and `reset`: the knobs are where they are going, and the next values set
    /// before a block is rendered are taken at once.
    pub fn settle(&mut self) {
        self.vibrato.snap();
        self.fresh = true;
    }

    /// `knob`: [`KNOB_ATTACK`], [`KNOB_RELEASE`] or [`KNOB_VIBRATO`]; others are passed over.
    pub fn set(&mut self, knob: u32, place: f32) {
        let Some(place) = knob_place(place) else {
            return;
        };
        match knob {
            KNOB_ATTACK => self.attack = place,
            KNOB_RELEASE => self.release = place,
            KNOB_VIBRATO => {
                let frames = if self.fresh { 0 } else { self.glide_frames };
                self.vibrato.set(place, frames);
            }
            _ => {}
        }
    }

    /// The attack time of a voice that starts now, for the instrument's own `base_s`.
    pub fn attack_s(&self, base_s: f32) -> f32 {
        let v = self.attack;
        if v == 0.5 {
            base_s
        } else if v < 0.5 {
            base_s * 2.0 * v
        } else {
            base_s + (v - 0.5) * 2.0 * ATTACK_ADD_S
        }
    }

    /// Whether the attack is the instrument's own.
    pub fn attack_as_is(&self) -> bool {
        self.attack == 0.5
    }

    /// By how much the release time of a voice that lets go now is multiplied: 1/4 at 0 %, 1
    /// at 50 %, 4 at 100 %.
    pub fn release_scale(&self) -> f32 {
        let v = self.release;
        if v == 0.5 {
            1.0
        } else {
            (2.0 * (2.0 * v - 1.0)).exp2()
        }
    }

    /// Whether the release is the instrument's own.
    pub fn release_as_is(&self) -> bool {
        self.release == 0.5
    }

    /// Whether any voice is to waver in the block about to be rendered.
    pub fn vibrato_on(&self) -> bool {
        !self.vibrato.rests_at(0.0)
    }

    /// The first block after a settle has begun: from now on values glide.
    pub fn rendering(&mut self) {
        self.fresh = false;
    }

    /// The vibrato's depth in cents `frames` on.
    pub fn advance_vibrato(&mut self, frames: usize) -> f32 {
        self.vibrato.advance(frames as u32) * VIBRATO_CENTS
    }
}

/// The pitch ratio of a voice `age` frames after it started, with a vibrato `cents` deep: a
/// sine of 5.5 Hz that starts at 0 with the voice. `follow`: the share of a bend the voice
/// follows.
pub fn vibrato_ratio(cents: f32, age: u64, sample_rate: f32, follow: f32) -> f32 {
    if cents == 0.0 || follow == 0.0 {
        return 1.0;
    }
    let phase = (VIBRATO_HZ * age as f64 / f64::from(sample_rate)).fract() as f32;
    (cents * follow * (TAU * phase).sin() / 1200.0).exp2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_the_start_the_times_are_the_instruments_own() {
        let k = VoiceKnobs::new();
        for base in [0.0_f32, 0.003, 0.45, 1.7] {
            assert_eq!(k.attack_s(base).to_bits(), base.to_bits());
        }
        assert_eq!(k.release_scale(), 1.0);
        assert!(k.attack_as_is() && k.release_as_is() && !k.vibrato_on());
    }

    #[test]
    fn the_ends_of_the_knobs() {
        let mut k = VoiceKnobs::new();
        k.set(KNOB_ATTACK, 0.0);
        assert_eq!(k.attack_s(0.2), 0.0);
        k.set(KNOB_ATTACK, 0.25);
        assert!((k.attack_s(0.2) - 0.1).abs() < 1e-6);
        k.set(KNOB_ATTACK, 1.0);
        assert!((k.attack_s(0.2) - 0.6).abs() < 1e-6);
        k.set(KNOB_RELEASE, 0.0);
        assert!((k.release_scale() - 0.25).abs() < 1e-6);
        k.set(KNOB_RELEASE, 1.0);
        assert!((k.release_scale() - 4.0).abs() < 1e-6);
        k.set(KNOB_RELEASE, f32::NAN);
        assert!((k.release_scale() - 4.0).abs() < 1e-6);
        k.set(KNOB_RELEASE, 7.0);
        assert!((k.release_scale() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn the_depth_is_taken_at_once_when_fresh_and_glides_after() {
        let mut k = VoiceKnobs::new();
        k.prepare(48_000.0);
        k.set(KNOB_VIBRATO, 1.0);
        assert_eq!(k.advance_vibrato(1), VIBRATO_CENTS);
        k.rendering();
        k.set(KNOB_VIBRATO, 0.0);
        let half = k.advance_vibrato(480);
        assert!((half - VIBRATO_CENTS / 2.0).abs() < 1e-3, "{half}");
        assert_eq!(k.advance_vibrato(480), 0.0);
        assert!(!k.vibrato_on());
    }

    #[test]
    fn the_vibrato_starts_at_its_middle_with_the_voice() {
        assert_eq!(vibrato_ratio(50.0, 0, 48_000.0, 1.0), 1.0);
        let quarter = (48_000.0 / 5.5 / 4.0) as u64;
        let top = vibrato_ratio(50.0, quarter, 48_000.0, 1.0);
        assert!((top - (50.0_f32 / 1200.0).exp2()).abs() < 1e-4, "{top}");
        assert_eq!(vibrato_ratio(0.0, quarter, 48_000.0, 1.0), 1.0);
    }
}
