//! Compressor: feed-forward, level and gain in dB, soft knee, smooth branching peak detector.
//! Design: Giannoulis, Massberg, Reiss, "Digital Dynamic Range Compressor Design", JAES 2012.

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{EffectKind, compressor as p};

use super::dsp::{self, LN10_OVER_20, Ramp, SMOOTH_S};

/// Width of the soft knee, centred on the threshold.
pub const KNEE_DB: f32 = 6.0;

const FLOOR: f32 = 1.0e-6;

pub struct Compressor {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,
    threshold: Ramp,
    /// 1 - 1/ratio.
    slope: Ramp,
    makeup_db: Ramp,
    attack_ms: f32,
    release_ms: f32,
    attack_coef: f32,
    release_coef: f32,
    /// How far the gain is held down, in dB (0 or more).
    reduction_db: f32,
}

/// The gain the compressor settles at for a steady level, in dB, without the make-up gain.
pub fn static_gain_db(level_db: f32, threshold_db: f32, ratio: f32) -> f32 {
    -reduction(level_db, threshold_db, 1.0 - 1.0 / ratio)
}

#[inline]
fn reduction(level_db: f32, threshold_db: f32, slope: f32) -> f32 {
    let over = level_db - threshold_db;
    if 2.0 * over < -KNEE_DB {
        0.0
    } else if 2.0 * over <= KNEE_DB {
        let x = over + KNEE_DB * 0.5;
        slope * x * x / (2.0 * KNEE_DB)
    } else {
        slope * over
    }
}

impl Compressor {
    pub fn new() -> Compressor {
        let mut c = Compressor {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            threshold: Ramp::new(0.0),
            slope: Ramp::new(0.0),
            makeup_db: Ramp::new(0.0),
            attack_ms: 10.0,
            release_ms: 100.0,
            attack_coef: 0.0,
            release_coef: 0.0,
            reduction_db: 0.0,
        };
        for (i, spec) in EffectKind::Compressor.params().iter().enumerate() {
            c.set_param(i as u32, spec.default);
        }
        c
    }

    /// Time constants: the detector covers 63 % of a step in the given time.
    fn update_times(&mut self) {
        self.attack_coef = (-1000.0 / (self.attack_ms * self.sample_rate)).exp();
        self.release_coef = (-1000.0 / (self.release_ms * self.sample_rate)).exp();
    }
}

impl Default for Compressor {
    fn default() -> Compressor {
        Compressor::new()
    }
}

impl Effect for Compressor {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        self.update_times();
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::Compressor, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        match index {
            p::THRESHOLD => self.threshold.set(v, n),
            p::RATIO => self.slope.set(1.0 - 1.0 / v, n),
            p::ATTACK => {
                self.attack_ms = v;
                self.update_times();
            }
            p::RELEASE => {
                self.release_ms = v;
                self.update_times();
            }
            p::MAKEUP => self.makeup_db.set(v, n),
            _ => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        if self.fresh {
            self.fresh = false;
            self.threshold.snap();
            self.slope.snap();
            self.makeup_db.snap();
        }
        let mut held = self.reduction_db;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let threshold = self.threshold.next();
            let slope = self.slope.next();
            let makeup = self.makeup_db.next();

            let peak = l.abs().max(r.abs()).max(FLOOR);
            let level_db = peak.ln() / LN10_OVER_20;
            let want = reduction(level_db, threshold, slope);
            let coef = if want > held {
                self.attack_coef
            } else {
                self.release_coef
            };
            held = dsp::flush(want + coef * (held - want));

            let gain = ((makeup - held) * LN10_OVER_20).exp();
            *l *= gain;
            *r *= gain;
        }
        self.reduction_db = held;
        if !held.is_finite() || !dsp::all_finite(left, right) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        self.reduction_db = 0.0;
        self.fresh = true;
    }

    fn reduction_db(&self) -> f32 {
        self.reduction_db.max(0.0)
    }
}
