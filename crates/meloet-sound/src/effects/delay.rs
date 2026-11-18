//! Delay: a stereo feedback delay line with a low-pass and a high-pass in the loop and an
//! optional ping-pong routing. Design: the textbook feedback comb (J. O. Smith, "Physical Audio
//! Signal Processing", public); a change of the interval crossfades between two taps.

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{DELAY_INTERVALS, EffectKind, delay as p};

use super::dsp::{self, DelayLine, FadeTap, Ramp, SMOOTH_S};

/// The longest interval: 2000 ms, which is also a quarter note at the slowest tempo (30).
const MAX_DELAY_S: f32 = 2.0;
const FALLBACK_TEMPO: f32 = 120.0;

pub struct Delay {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,
    step: usize,
    interval_ms: f32,
    sync: bool,
    high_cut_hz: f32,
    low_cut_hz: f32,
    tap: FadeTap,
    feedback: Ramp,
    high_cut: Ramp,
    low_cut: Ramp,
    ping_pong: Ramp,
    mix: Ramp,
    lines: [DelayLine; 2],
    /// Per channel: the low-pass state and the state of the low-pass the high-pass subtracts.
    filters: [[f32; 2]; 2],
}

impl Delay {
    pub fn new() -> Delay {
        let mut d = Delay {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            step: 2,
            interval_ms: 250.0,
            sync: true,
            high_cut_hz: 5000.0,
            low_cut_hz: 200.0,
            tap: FadeTap::new(1),
            feedback: Ramp::new(0.0),
            high_cut: Ramp::new(1.0),
            low_cut: Ramp::new(0.0),
            ping_pong: Ramp::new(0.0),
            mix: Ramp::new(1.0),
            lines: [DelayLine::empty(), DelayLine::empty()],
            filters: [[0.0; 2]; 2],
        };
        for (i, spec) in EffectKind::Delay.params().iter().enumerate() {
            d.set_param(i as u32, spec.default);
        }
        d
    }

    fn delay_frames(&self, tempo_bpm: f32) -> usize {
        let seconds = if self.sync {
            let tempo = if tempo_bpm.is_finite() && tempo_bpm > 0.0 {
                tempo_bpm
            } else {
                FALLBACK_TEMPO
            };
            DELAY_INTERVALS[self.step].quarters * 60.0 / tempo
        } else {
            self.interval_ms / 1000.0
        };
        let frames = (seconds * self.sample_rate).round() as usize;
        frames.clamp(1, self.lines[0].max_delay().max(1))
    }

    fn update_filters(&mut self) {
        let n = self.ramp_frames;
        self.high_cut
            .set(dsp::one_pole(self.high_cut_hz, self.sample_rate), n);
        self.low_cut
            .set(dsp::one_pole(self.low_cut_hz, self.sample_rate), n);
    }
}

impl Default for Delay {
    fn default() -> Delay {
        Delay::new()
    }
}

impl Effect for Delay {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        self.tap.set_fade_frames(self.ramp_frames);
        let max = (MAX_DELAY_S * sample_rate).ceil() as usize;
        for line in &mut self.lines {
            line.resize(max);
        }
        self.update_filters();
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::Delay, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        match index {
            p::INTERVAL_STEP => self.step = v as usize,
            p::INTERVAL_MS => self.interval_ms = v,
            p::SYNC => self.sync = v >= 0.5,
            p::FEEDBACK => self.feedback.set(v / 100.0, n),
            p::HIGH_CUT => {
                self.high_cut_hz = v;
                self.update_filters();
            }
            p::LOW_CUT => {
                self.low_cut_hz = v;
                self.update_filters();
            }
            p::PING_PONG => self.ping_pong.set(v, n),
            p::MIX => self.mix.set(v / 100.0, n),
            _ => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], ctx: &EffectContext) {
        if self.lines[0].max_delay() == 0 {
            return;
        }
        self.tap.set(self.delay_frames(ctx.tempo_bpm));
        if self.fresh {
            self.fresh = false;
            self.tap.snap();
            self.feedback.snap();
            self.high_cut.snap();
            self.low_cut.snap();
            self.ping_pong.snap();
            self.mix.snap();
        }
        let [line_l, line_r] = &mut self.lines;
        let [fl, fr] = &mut self.filters;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let taps = self.tap.tick();
            let feedback = self.feedback.next();
            let lp = self.high_cut.next();
            let hp = self.low_cut.next();
            let pp = self.ping_pong.next();
            let mix = self.mix.next();

            let (xl, xr) = (*l, *r);
            let dl = dsp::read_faded(line_l, taps);
            let dr = dsp::read_faded(line_r, taps);

            // Ping-pong: the input goes to the left line only, and each line feeds the other.
            let mono = 0.5 * (xl + xr);
            let in_l = xl + pp * (mono - xl) + feedback * (dl + pp * (dr - dl));
            let in_r = xr * (1.0 - pp) + feedback * (dr + pp * (dl - dr));

            fl[0] = dsp::flush(fl[0] + lp * (in_l - fl[0]));
            fl[1] = dsp::flush(fl[1] + hp * (fl[0] - fl[1]));
            fr[0] = dsp::flush(fr[0] + lp * (in_r - fr[0]));
            fr[1] = dsp::flush(fr[1] + hp * (fr[0] - fr[1]));
            line_l.write(fl[0] - fl[1]);
            line_r.write(fr[0] - fr[1]);

            *l = xl + mix * (dl - xl);
            *r = xr + mix * (dr - xr);
        }
        if !dsp::all_finite(left, right) || !dsp::all_finite(fl, fr) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.filters = [[0.0; 2]; 2];
        self.fresh = true;
    }
}
