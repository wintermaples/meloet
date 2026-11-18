//! Limiter without look-ahead: the gain drops at once, comes back smoothly, and a hard clip at
//! the ceiling is the last step. Design: the peak limiter as the limit case (zero attack) of the
//! gain smoothing in Giannoulis, Massberg, Reiss, JAES 2012.

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{EffectKind, limiter as p};

use super::dsp::{self, Ramp, SMOOTH_S, db_to_gain};

pub struct Limiter {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,
    ceiling: Ramp,
    pre_gain: Ramp,
    release_ms: f32,
    release_step: f32,
    /// The gain the limiter holds the signal at (0..=1).
    gain: f32,
}

impl Limiter {
    pub fn new() -> Limiter {
        let mut l = Limiter {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            ceiling: Ramp::new(1.0),
            pre_gain: Ramp::new(1.0),
            release_ms: 100.0,
            release_step: 0.0,
            gain: 1.0,
        };
        for (i, spec) in EffectKind::Limiter.params().iter().enumerate() {
            l.set_param(i as u32, spec.default);
        }
        l
    }

    fn update_release(&mut self) {
        self.release_step = 1.0 - (-1000.0 / (self.release_ms * self.sample_rate)).exp();
    }
}

impl Default for Limiter {
    fn default() -> Limiter {
        Limiter::new()
    }
}

impl Effect for Limiter {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        self.update_release();
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::Limiter, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        match index {
            p::CEILING => self.ceiling.set(db_to_gain(v), n),
            p::GAIN => self.pre_gain.set(db_to_gain(v), n),
            p::RELEASE => {
                self.release_ms = v;
                self.update_release();
            }
            _ => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        if self.fresh {
            self.fresh = false;
            self.ceiling.snap();
            self.pre_gain.snap();
        }
        let mut gain = self.gain;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let ceiling = self.ceiling.next();
            let pre = self.pre_gain.next();
            let xl = *l * pre;
            let xr = *r * pre;
            let peak = xl.abs().max(xr.abs());
            let want = if peak > ceiling { ceiling / peak } else { 1.0 };
            if want < gain {
                gain = want;
            } else {
                gain += self.release_step * (want - gain);
            }
            *l = (xl * gain).clamp(-ceiling, ceiling);
            *r = (xr * gain).clamp(-ceiling, ceiling);
        }
        self.gain = gain;
        if !gain.is_finite() || !dsp::all_finite(left, right) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        self.gain = 1.0;
        self.fresh = true;
    }
}
