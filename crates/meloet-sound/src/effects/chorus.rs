//! Chorus: one delay per channel whose length a slow sine moves; the right channel's sine runs
//! up to a quarter turn ahead. Design: the textbook modulated delay line (J. O. Smith, "Physical
//! Audio Signal Processing", public), read with a 4-point Hermite curve.

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{EffectKind, chorus as p};

use super::dsp::{self, DelayLine, Ramp, SMOOTH_S};

const CENTRE_S: f32 = 0.007;
/// How far the delay swings each way at depth 100 %.
const SWING_S: f32 = 0.004;

pub struct Chorus {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,
    rate_hz: f32,
    /// The turn one frame makes: (cos, sin).
    turn: (f32, f32),
    /// The sine's place on the unit circle: (cos, sin).
    phase: (f32, f32),
    /// 0..=1 of the full swing.
    depth: Ramp,
    /// The right channel's lead: cos and sin of the angle.
    lead_cos: Ramp,
    lead_sin: Ramp,
    mix: Ramp,
    lines: [DelayLine; 2],
}

impl Chorus {
    pub fn new() -> Chorus {
        let mut c = Chorus {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            rate_hz: 0.8,
            turn: (1.0, 0.0),
            phase: (1.0, 0.0),
            depth: Ramp::new(0.0),
            lead_cos: Ramp::new(1.0),
            lead_sin: Ramp::new(0.0),
            mix: Ramp::new(0.0),
            lines: [DelayLine::empty(), DelayLine::empty()],
        };
        for (i, spec) in EffectKind::Chorus.params().iter().enumerate() {
            c.set_param(i as u32, spec.default);
        }
        c
    }

    fn update_rate(&mut self) {
        let angle = std::f32::consts::TAU * self.rate_hz / self.sample_rate;
        self.turn = (angle.cos(), angle.sin());
    }
}

impl Default for Chorus {
    fn default() -> Chorus {
        Chorus::new()
    }
}

impl Effect for Chorus {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        let max = ((CENTRE_S + SWING_S) * sample_rate).ceil() as usize + 4;
        for line in &mut self.lines {
            line.resize(max);
        }
        self.update_rate();
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::Chorus, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        match index {
            p::RATE => {
                self.rate_hz = v;
                self.update_rate();
            }
            p::DEPTH => self.depth.set(v / 100.0, n),
            p::WIDTH => {
                let angle = v / 100.0 * std::f32::consts::FRAC_PI_2;
                self.lead_cos.set(angle.cos(), n);
                self.lead_sin.set(angle.sin(), n);
            }
            p::MIX => self.mix.set(v / 100.0, n),
            _ => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        if self.lines[0].max_delay() == 0 {
            return;
        }
        if self.fresh {
            self.fresh = false;
            self.depth.snap();
            self.lead_cos.snap();
            self.lead_sin.snap();
            self.mix.snap();
        }
        let centre = CENTRE_S * self.sample_rate;
        let swing = SWING_S * self.sample_rate;
        let (turn_cos, turn_sin) = self.turn;
        let (mut cos, mut sin) = self.phase;
        let [line_l, line_r] = &mut self.lines;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let depth = self.depth.next() * swing;
            let lead_cos = self.lead_cos.next();
            let lead_sin = self.lead_sin.next();
            let mix = self.mix.next();

            (cos, sin) = (
                cos * turn_cos - sin * turn_sin,
                sin * turn_cos + cos * turn_sin,
            );
            let wave_l = sin;
            let wave_r = sin * lead_cos + cos * lead_sin;

            let (xl, xr) = (*l, *r);
            let read = |line: &DelayLine, wave: f32| {
                let delay = centre + depth * wave;
                let whole = delay as usize;
                line.read_frac(whole, delay - whole as f32)
            };
            *l = xl + mix * (read(line_l, wave_l) - xl);
            *r = xr + mix * (read(line_r, wave_r) - xr);
            line_l.write(xl);
            line_r.write(xr);
        }
        // The turning point drifts off the unit circle by rounding; pull it back.
        let pull = 1.5 - 0.5 * (cos * cos + sin * sin);
        self.phase = (cos * pull, sin * pull);
        if !dsp::all_finite(left, right) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.phase = (1.0, 0.0);
        self.fresh = true;
    }
}
