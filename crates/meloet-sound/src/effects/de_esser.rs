//! De-esser: a high shelf at the frequency that goes down while the band above the frequency is
//! over the threshold, by what is over, at most by the reduction. With nothing to lower the
//! shelf is flat and the output is the input, sample for sample. Both sides share one gain.
//!
//! The watching side is a 24 dB/oct Butterworth high-pass (two sections), so the body of a
//! voice does not set it off. The shelf is the high shelf of the EQ (Simper's SVF mix).

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{EffectKind, de_esser as p};

use super::dsp::{self, LN10_OVER_20, Ramp, SMOOTH_S, Svf, SvfCoefs};

/// The cut covers 63 % of a step up in the first time, of a step down in the second.
pub const ATTACK_MS: f32 = 1.0;
pub const RELEASE_MS: f32 = 50.0;
/// The watched band's level is its peaks, falling to 37 % in this time between them.
const PEAK_FALL_MS: f32 = 5.0;

const SHELF_K: f32 = std::f32::consts::SQRT_2;
/// 1 / Q of the two sections of a 4th-order Butterworth filter.
const WATCH_K: [f32; 2] = [1.847_759, 0.765_366_9];
const FLOOR: f32 = 1.0e-6;

pub struct DeEsser {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,
    /// The natural log of the frequency.
    log_freq: Ramp,
    threshold_db: Ramp,
    most_db: Ramp,
    attack_coef: f32,
    release_coef: f32,
    peak_fall: f32,
    /// tan(pi * frequency / sample rate).
    g: f32,
    watch_coefs: [SvfCoefs; 2],
    flat: SvfCoefs,
    /// Per channel: the two sections of the watching high-pass.
    watch: [[Svf; 2]; 2],
    shelf: [Svf; 2],
    /// The level of the watched band (a sample value).
    peak: f32,
    /// How far the shelf is down now, in dB (0 or more).
    cut_db: f32,
}

impl DeEsser {
    pub fn new() -> DeEsser {
        let mut d = DeEsser {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            log_freq: Ramp::new(0.0),
            threshold_db: Ramp::new(0.0),
            most_db: Ramp::new(0.0),
            attack_coef: 0.0,
            release_coef: 0.0,
            peak_fall: 0.0,
            g: 0.0,
            watch_coefs: [SvfCoefs::default(); 2],
            flat: SvfCoefs::default(),
            watch: [[Svf::default(); 2]; 2],
            shelf: [Svf::default(); 2],
            peak: 0.0,
            cut_db: 0.0,
        };
        for (i, spec) in EffectKind::DeEsser.params().iter().enumerate() {
            d.set_param(i as u32, spec.default);
        }
        d.update_times();
        d
    }

    fn update_times(&mut self) {
        self.attack_coef = (-1000.0 / (ATTACK_MS * self.sample_rate)).exp();
        self.release_coef = (-1000.0 / (RELEASE_MS * self.sample_rate)).exp();
        self.peak_fall = (-1000.0 / (PEAK_FALL_MS * self.sample_rate)).exp();
    }

    fn tune(&mut self) {
        let freq = self.log_freq.get().exp().min(0.45 * self.sample_rate);
        self.g = (std::f32::consts::PI * freq / self.sample_rate).tan();
        self.watch_coefs = WATCH_K.map(|k| SvfCoefs::new(self.g, k));
        self.flat = SvfCoefs::new(self.g, SHELF_K);
    }
}

impl Default for DeEsser {
    fn default() -> DeEsser {
        DeEsser::new()
    }
}

impl Effect for DeEsser {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        self.update_times();
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::DeEsser, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        match index {
            p::FREQUENCY => self.log_freq.set(v.ln(), n),
            p::THRESHOLD => self.threshold_db.set(v, n),
            p::REDUCTION => self.most_db.set(v, n),
            _ => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        if self.fresh {
            self.fresh = false;
            self.log_freq.snap();
            self.threshold_db.snap();
            self.most_db.snap();
            self.tune();
        }
        let (mut peak, mut cut) = (self.peak, self.cut_db);
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if self.log_freq.active() {
                self.log_freq.next();
                self.tune();
            }
            let threshold = self.threshold_db.next();
            let most = self.most_db.next();

            let mut heard = [*l, *r];
            for (x, sections) in heard.iter_mut().zip(&mut self.watch) {
                for (section, coefs) in sections.iter_mut().zip(&self.watch_coefs) {
                    *x = section.high(coefs, *x);
                }
            }
            peak = heard[0]
                .abs()
                .max(heard[1].abs())
                .max(peak * self.peak_fall);
            let over = peak.max(FLOOR).ln() / LN10_OVER_20 - threshold;
            // A level that is not a number asks for nothing here; the end of the block sees it.
            let want = over.min(most).max(0.0);
            let coef = if want > cut {
                self.attack_coef
            } else {
                self.release_coef
            };
            cut = dsp::flush(want + coef * (cut - want));

            if cut == 0.0 {
                // The states go on, so the shelf starts from the sound when it next moves.
                self.shelf[0].tick(&self.flat, *l);
                self.shelf[1].tick(&self.flat, *r);
            } else {
                // The square root of the shelf's "A" (which is the square root of its gain).
                let root = (-cut * (LN10_OVER_20 / 4.0)).exp();
                let a = root * root;
                let coefs = SvfCoefs::new(self.g * root, SHELF_K);
                let (m0, m1, m2) = (a * a, SHELF_K * (1.0 - a) * a, 1.0 - a * a);
                for (x, state) in [&mut *l, &mut *r].into_iter().zip(&mut self.shelf) {
                    let (band, low) = state.tick(&coefs, *x);
                    *x = m0 * *x + m1 * band + m2 * low;
                }
            }
        }
        self.peak = dsp::flush(peak);
        self.cut_db = cut;
        for state in self.watch.iter_mut().flatten().chain(&mut self.shelf) {
            state.flush();
        }
        if !peak.is_finite() || !cut.is_finite() || !dsp::all_finite(left, right) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        self.watch = [[Svf::default(); 2]; 2];
        self.shelf = [Svf::default(); 2];
        self.peak = 0.0;
        self.cut_db = 0.0;
        self.fresh = true;
    }
}
