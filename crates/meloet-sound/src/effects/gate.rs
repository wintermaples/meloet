//! Gate: sound below the threshold is brought down by the range. No look-ahead: it opens in
//! 1 ms from the frame that passes the threshold, stays open for 30 ms after the last frame
//! above the closing level (3 dB under the threshold), then falls at 60 dB per release time.
//! Both sides share one gain.

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{EffectKind, gate as p};

use super::dsp::{self, Ramp, SMOOTH_S, db_to_gain};

/// Time to open from silence to full.
pub const OPEN_MS: f32 = 1.0;
/// How long the gate stays open after the last frame above the closing level.
pub const HOLD_MS: f32 = 30.0;
/// The closing level is this far under the threshold.
pub const HYSTERESIS_DB: f32 = 3.0;
/// The gain falls this far in one release time.
pub const RELEASE_FALL_DB: f32 = 60.0;

pub struct Gate {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,
    /// As a sample value, not dB.
    threshold: Ramp,
    /// The gain while closed (the range as a gain).
    floor: Ramp,
    release_ms: f32,
    /// Added to the gain per frame while open.
    rise: f32,
    /// The gain is multiplied by this per frame while closed.
    fall: f32,
    hold_frames: u32,
    /// Frames the gate stays open from here; 0 is closed.
    hold: u32,
    gain: f32,
}

impl Gate {
    pub fn new() -> Gate {
        let mut g = Gate {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            threshold: Ramp::new(0.0),
            floor: Ramp::new(1.0),
            release_ms: 100.0,
            rise: 0.0,
            fall: 0.0,
            hold_frames: 0,
            hold: 0,
            gain: 1.0,
        };
        for (i, spec) in EffectKind::Gate.params().iter().enumerate() {
            g.set_param(i as u32, spec.default);
        }
        g
    }

    fn update_times(&mut self) {
        let per_ms = 0.001 * self.sample_rate;
        self.rise = 1.0 / (OPEN_MS * per_ms);
        self.hold_frames = (HOLD_MS * per_ms) as u32;
        self.fall = db_to_gain(-RELEASE_FALL_DB / (self.release_ms * per_ms));
    }
}

impl Default for Gate {
    fn default() -> Gate {
        Gate::new()
    }
}

impl Effect for Gate {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        self.update_times();
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::Gate, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        match index {
            p::THRESHOLD => self.threshold.set(db_to_gain(v), n),
            p::RANGE => self.floor.set(db_to_gain(v), n),
            p::RELEASE => {
                self.release_ms = v;
                self.update_times();
            }
            _ => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        if self.fresh {
            self.fresh = false;
            self.threshold.snap();
            self.floor.snap();
            self.gain = self.floor.get();
        }
        let closing = db_to_gain(-HYSTERESIS_DB);
        let (mut hold, mut gain) = (self.hold, self.gain);
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let threshold = self.threshold.next();
            let floor = self.floor.next();
            let peak = l.abs().max(r.abs());
            if peak > threshold || (hold > 0 && peak > threshold * closing) {
                hold = self.hold_frames;
            } else {
                hold = hold.saturating_sub(1);
            }
            gain = if hold > 0 {
                (gain + self.rise).min(1.0)
            } else {
                (gain * self.fall).max(floor)
            };
            *l *= gain;
            *r *= gain;
        }
        self.hold = hold;
        self.gain = gain;
        if !dsp::all_finite(left, right) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        self.hold = 0;
        self.fresh = true;
    }
}
