//! EQ, 4 bands: low shelf, two peaks, high shelf. Design: Andrew Simper's linear trapezoidal
//! state variable filter (cytomic.com, "SvfLinearTrapOptimised2"); width to Q as in RBJ's cookbook.

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{EffectKind, eq4 as p};

use super::dsp::{self, Ramp, SMOOTH_S};

const SHELF_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    LowShelf,
    Peak,
    HighShelf,
}

#[derive(Clone, Copy, Default)]
struct Coefs {
    a1: f32,
    a2: f32,
    a3: f32,
    m0: f32,
    m1: f32,
    m2: f32,
}

struct Band {
    shape: Shape,
    /// The natural log of the frequency, so a sweep takes the same time per octave.
    log_freq: Ramp,
    gain_db: Ramp,
    width: Ramp,
    coefs: Coefs,
    /// Per channel: the two integrator states.
    state: [[f32; 2]; 2],
}

impl Band {
    fn new(shape: Shape) -> Band {
        Band {
            shape,
            log_freq: Ramp::new(0.0),
            gain_db: Ramp::new(0.0),
            width: Ramp::new(1.0),
            coefs: Coefs::default(),
            state: [[0.0; 2]; 2],
        }
    }

    fn ramping(&self) -> bool {
        self.log_freq.active() || self.gain_db.active() || self.width.active()
    }

    fn snap(&mut self) {
        self.log_freq.snap();
        self.gain_db.snap();
        self.width.snap();
    }

    fn update(&mut self, sample_rate: f32) {
        let freq = self.log_freq.get().exp().min(0.45 * sample_rate);
        let a = (self.gain_db.get() * (std::f32::consts::LN_10 / 40.0)).exp();
        let t = (std::f32::consts::PI * freq / sample_rate).tan();
        let (g, k, m0, m1, m2) = match self.shape {
            Shape::Peak => {
                let q = 1.0 / (2.0 * (std::f32::consts::LN_2 * 0.5 * self.width.get()).sinh());
                let k = 1.0 / (q * a);
                (t, k, 1.0, k * (a * a - 1.0), 0.0)
            }
            Shape::LowShelf => {
                let k = 1.0 / SHELF_Q;
                (t / a.sqrt(), k, 1.0, k * (a - 1.0), a * a - 1.0)
            }
            Shape::HighShelf => {
                let k = 1.0 / SHELF_Q;
                (t * a.sqrt(), k, a * a, k * (1.0 - a) * a, 1.0 - a * a)
            }
        };
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        self.coefs = Coefs {
            a1,
            a2,
            a3: g * a2,
            m0,
            m1,
            m2,
        };
    }

    #[inline]
    fn tick(c: &Coefs, s: &mut [f32; 2], v0: f32) -> f32 {
        let v3 = v0 - s[1];
        let v1 = c.a1 * s[0] + c.a2 * v3;
        let v2 = s[1] + c.a2 * s[0] + c.a3 * v3;
        s[0] = 2.0 * v1 - s[0];
        s[1] = 2.0 * v2 - s[1];
        c.m0 * v0 + c.m1 * v1 + c.m2 * v2
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], sample_rate: f32) {
        let mut i = 0;
        while i < left.len() && self.ramping() {
            self.log_freq.next();
            self.gain_db.next();
            self.width.next();
            self.update(sample_rate);
            left[i] = Self::tick(&self.coefs, &mut self.state[0], left[i]);
            right[i] = Self::tick(&self.coefs, &mut self.state[1], right[i]);
            i += 1;
        }
        let c = self.coefs;
        let [sl, sr] = &mut self.state;
        for (l, r) in left[i..].iter_mut().zip(&mut right[i..]) {
            *l = Self::tick(&c, sl, *l);
            *r = Self::tick(&c, sr, *r);
        }
        for s in self.state.iter_mut().flatten() {
            *s = dsp::flush(*s);
        }
    }
}

pub struct Eq4 {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,
    bands: [Band; 4],
}

impl Eq4 {
    pub fn new() -> Eq4 {
        let mut eq = Eq4 {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            bands: [
                Band::new(Shape::LowShelf),
                Band::new(Shape::Peak),
                Band::new(Shape::Peak),
                Band::new(Shape::HighShelf),
            ],
        };
        for (i, spec) in EffectKind::Eq4.params().iter().enumerate() {
            eq.set_param(i as u32, spec.default);
        }
        eq
    }
}

impl Default for Eq4 {
    fn default() -> Eq4 {
        Eq4::new()
    }
}

impl Effect for Eq4 {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::Eq4, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        let b = &mut self.bands;
        match index {
            p::LOW_FREQ => b[0].log_freq.set(v.ln(), n),
            p::LOW_GAIN => b[0].gain_db.set(v, n),
            p::LOW_MID_FREQ => b[1].log_freq.set(v.ln(), n),
            p::LOW_MID_GAIN => b[1].gain_db.set(v, n),
            p::LOW_MID_WIDTH => b[1].width.set(v, n),
            p::HIGH_MID_FREQ => b[2].log_freq.set(v.ln(), n),
            p::HIGH_MID_GAIN => b[2].gain_db.set(v, n),
            p::HIGH_MID_WIDTH => b[2].width.set(v, n),
            p::HIGH_FREQ => b[3].log_freq.set(v.ln(), n),
            p::HIGH_GAIN => b[3].gain_db.set(v, n),
            _ => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        if self.fresh {
            self.fresh = false;
            for band in &mut self.bands {
                band.snap();
                band.update(self.sample_rate);
            }
        }
        for band in &mut self.bands {
            band.process(left, right, self.sample_rate);
        }
        if !dsp::all_finite(left, right) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.state = [[0.0; 2]; 2];
        }
        self.fresh = true;
    }
}
