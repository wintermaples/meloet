//! Saturation: a tanh-shaped curve that rounds the tops, a low-pass after it, dry/wet mix and
//! an output gain. No oversampling (it would delay the sound).
//!
//! The curve is `REFERENCE * tanh(z * x / REFERENCE) / tanh(z)`: whatever the bend `z`, a
//! sample at the reference level stays where it is, so turning the drive up does not turn the
//! sound up with it. Quiet sound is lifted by `z / tanh(z)`, and that lift is what the drive
//! parameter sets. At drive 0 dB the bend is 0 and the curve is a straight line.

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{EffectKind, saturation as p};

use super::dsp::{self, Ramp, SMOOTH_S, Svf, SvfCoefs, db_to_gain};

/// The sample value the curve leaves where it is: -18 dBFS, about the RMS level of the built-in
/// sounds (one key peaks near -12 dBFS), so their loudness stays within a few dB at any drive.
pub const REFERENCE: f32 = 0.125;

/// Under this bend the curve is a straight line within f32's precision.
const STRAIGHT: f32 = 1.0e-4;
/// The tone filter fades out over the top of its range and is off at the maximum (the end is
/// a hair under 20 kHz, so the maximum is past it whatever `ln` and `exp` round to).
const TONE_OPEN_FROM_HZ: f32 = 18_000.0;
const TONE_OPEN_AT_HZ: f32 = 19_990.0;
/// Just under the place where the fraction in [`curve`] reaches 1.
const CURVE_LIMIT: f32 = 4.97;

/// tanh within 1e-4: Lambert's continued fraction cut after seven terms, flat from the place it
/// reaches 1. No call into libm, so the loop over a block can be vectorized.
#[inline]
pub fn curve(x: f32) -> f32 {
    let x = x.clamp(-CURVE_LIMIT, CURVE_LIMIT);
    let x2 = x * x;
    let top = x * (135_135.0 + x2 * (17_325.0 + x2 * (378.0 + x2)));
    let bottom = 135_135.0 + x2 * (62_370.0 + x2 * (3_150.0 + x2 * 28.0));
    top / bottom
}

/// The bend whose curve lifts quiet sound by `lift` (a gain, 1 or more): the positive root of
/// `z = lift * tanh(z)`, or 0 for no lift. Newton's method from the right of the root, where
/// the function is convex and rising, so every step lands between the root and the last guess.
pub fn bend_for(lift: f32) -> f32 {
    let lift = f64::from(lift);
    if lift < 1.0 + 1.0e-9 {
        return 0.0;
    }
    let mut z = lift;
    for _ in 0..40 {
        let t = z.tanh();
        let step = (z - lift * t) / (1.0 - lift * (1.0 - t * t));
        z -= step;
        if step.abs() < 1.0e-9 * z {
            break;
        }
    }
    z as f32
}

pub struct Saturation {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,
    bend: Ramp,
    /// The natural log of the tone's frequency.
    log_tone: Ramp,
    mix: Ramp,
    output: Ramp,
    /// From the bend: the sample goes in times `into`, through the curve, out times `back`.
    into: f32,
    back: f32,
    straight: bool,
    tone: SvfCoefs,
    /// How much of the sound goes around the tone filter (1 = the filter is off).
    open: f32,
    low: [Svf; 2],
}

impl Saturation {
    pub fn new() -> Saturation {
        let mut s = Saturation {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            bend: Ramp::new(0.0),
            log_tone: Ramp::new(0.0),
            mix: Ramp::new(1.0),
            output: Ramp::new(1.0),
            into: 1.0,
            back: 1.0,
            straight: true,
            tone: SvfCoefs::default(),
            open: 1.0,
            low: [Svf::default(); 2],
        };
        for (i, spec) in EffectKind::Saturation.params().iter().enumerate() {
            s.set_param(i as u32, spec.default);
        }
        s
    }

    fn shape(&mut self) {
        let z = self.bend.get();
        self.straight = z <= STRAIGHT;
        if !self.straight {
            self.into = z / REFERENCE;
            self.back = REFERENCE / curve(z);
        }
    }

    fn tune(&mut self) {
        let freq = self.log_tone.get().exp();
        let g = (std::f32::consts::PI * freq.min(0.45 * self.sample_rate) / self.sample_rate).tan();
        self.tone = SvfCoefs::new(g, std::f32::consts::SQRT_2);
        self.open =
            ((freq - TONE_OPEN_FROM_HZ) / (TONE_OPEN_AT_HZ - TONE_OPEN_FROM_HZ)).clamp(0.0, 1.0);
    }
}

impl Default for Saturation {
    fn default() -> Saturation {
        Saturation::new()
    }
}

impl Effect for Saturation {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::Saturation, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        match index {
            p::DRIVE => self.bend.set(bend_for(db_to_gain(v)), n),
            p::TONE => self.log_tone.set(v.ln(), n),
            p::MIX => self.mix.set(v * 0.01, n),
            p::OUTPUT => self.output.set(db_to_gain(v), n),
            _ => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        if self.fresh {
            self.fresh = false;
            self.bend.snap();
            self.log_tone.snap();
            self.mix.snap();
            self.output.snap();
            self.shape();
            self.tune();
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if self.bend.active() {
                self.bend.next();
                self.shape();
            }
            if self.log_tone.active() {
                self.log_tone.next();
                self.tune();
            }
            let mix = self.mix.next();
            let output = self.output.next();
            for (x, low) in [&mut *l, &mut *r].into_iter().zip(&mut self.low) {
                let bent = if self.straight {
                    *x
                } else {
                    self.back * curve(self.into * *x)
                };
                // The filter runs even while it is off, so it has the sound when it comes in.
                let (_, dark) = low.tick(&self.tone, bent);
                let wet = if self.open >= 1.0 {
                    bent
                } else {
                    dark + self.open * (bent - dark)
                };
                *x = (*x + mix * (wet - *x)) * output;
            }
        }
        for low in &mut self.low {
            low.flush();
        }
        if !dsp::all_finite(left, right) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        self.low = [Svf::default(); 2];
        self.fresh = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_is_tanh_and_never_passes_one() {
        let mut worst = 0.0_f32;
        let mut last = -2.0_f32;
        for i in -80_000..=80_000 {
            let x = i as f32 / 10_000.0;
            let y = curve(x);
            worst = worst.max((y - x.tanh()).abs());
            // Near the flat ends, rounding may take back the last bits.
            assert!(y >= last - 1.0e-6, "rising at {x}");
            last = y;
        }
        assert!(worst < 1.5e-4, "{worst:e}");
        for x in [1.0e9, f32::MAX, f32::INFINITY] {
            assert!(curve(x) <= 1.0 && curve(x) > 0.9999);
            assert_eq!(curve(-x), -curve(x));
        }
        assert_eq!(curve(0.0), 0.0);
    }

    #[test]
    fn the_bend_gives_the_lift_asked_for() {
        assert_eq!(bend_for(1.0), 0.0);
        for db in [0.01_f32, 0.1, 1.0, 6.0, 12.0, 24.0, 36.0] {
            let lift = db_to_gain(db);
            let z = f64::from(bend_for(lift));
            let got = z / z.tanh();
            assert!(
                (got / f64::from(lift) - 1.0).abs() < 1.0e-5,
                "{db} dB: {got} for {lift}"
            );
        }
        // More drive, more bend, all the way.
        let mut last = 0.0;
        for step in 1..=3600 {
            let z = bend_for(db_to_gain(step as f32 * 0.01));
            assert!(z > last, "{step}");
            last = z;
        }
    }
}
