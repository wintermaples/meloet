//! Reverb: a feedback delay network of 8 lines mixed by a Hadamard matrix, with a loss per line
//! that gives the asked decay time, behind a pre-delay and 4 all-pass diffusers per channel.
//! Design: Jot and Chaigne's FDN (AES 1991) as told in J. O. Smith, "Physical Audio Signal
//! Processing" (public); Schroeder all-passes as in Freeverb (public domain).

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::{EffectKind, reverb as p};

use super::dsp::{self, DelayLine, FadeTap, Ramp, SMOOTH_S};

const LINES: usize = 8;
/// Line lengths at size 100 %.
const BASE_MS: [f32; LINES] = [41.3, 47.9, 53.9, 59.3, 67.1, 73.3, 79.9, 88.7];
/// The lines shrink to this share at size 0 %.
const SMALLEST: f32 = 0.25;
const DIFFUSER_MS: [[f32; 4]; 2] = [[4.31, 6.13, 2.27, 3.19], [4.73, 5.59, 2.57, 3.61]];
const DIFFUSION: f32 = 0.5;
const MAX_PRE_DELAY_S: f32 = 0.2;
/// Above the high cut, the tail dies this much faster than below it.
const HIGH_DECAY_SHARE: f32 = 0.35;
const SIZE_FADE_S: f32 = 0.05;
/// Loop gains follow their parameters once per this many frames.
const CHUNK: usize = 32;
const WET_GAIN: f32 = 0.25;
const LN_60_DB: f32 = -6.907_755;

const GAIN_CUR: usize = 0;
const GAIN_NEXT: usize = 1;
const HIGH_SHARE: usize = 2;

fn next_prime(mut n: usize) -> usize {
    n = n.max(3) | 1;
    loop {
        let mut d = 3;
        let mut prime = true;
        while d * d <= n {
            if n.is_multiple_of(d) {
                prime = false;
                break;
            }
            d += 2;
        }
        if prime {
            return n;
        }
        n += 2;
    }
}

#[inline]
fn hadamard(v: &mut [f32; LINES]) {
    let mut h = 1;
    while h < LINES {
        let mut i = 0;
        while i < LINES {
            for j in i..i + h {
                let (a, b) = (v[j], v[j + h]);
                v[j] = a + b;
                v[j + h] = a - b;
            }
            i += 2 * h;
        }
        h *= 2;
    }
}

pub struct Reverb {
    sample_rate: f32,
    ramp_frames: u32,
    fresh: bool,

    decay_s: Ramp,
    size_scale: f32,
    pre_delay_ms: f32,
    high_cut_hz: f32,
    low_cut_hz: f32,
    pre_tap: FadeTap,
    in_low_pass: Ramp,
    in_high_pass: Ramp,
    width: Ramp,
    mix: Ramp,
    out_gain: Ramp,

    pre: [DelayLine; 2],
    /// Per channel: low-pass state and the state of the low-pass the high-pass subtracts.
    tone: [[f32; 2]; 2],
    diffusers: [[DelayLine; 4]; 2],
    diffuser_len: [[usize; 4]; 2],

    lines: [DelayLine; LINES],
    len_want: [usize; LINES],
    len_cur: [usize; LINES],
    len_next: [usize; LINES],
    fade_left: u32,
    fade_len: u32,
    /// Per line: loop gain at the current length, loop gain at the length fading in, and the
    /// extra loss the highs get.
    loop_gains: [[f32; LINES]; 3],
    /// What `loop_gains` moves by per frame until the end of the chunk.
    loop_steps: [[f32; LINES]; 3],
    loop_targets: [[f32; LINES]; 3],
    gliding: bool,
    damp: f32,
    low: [f32; LINES],
    stale: bool,
}

impl Reverb {
    pub fn new() -> Reverb {
        let mut r = Reverb {
            sample_rate: 48_000.0,
            ramp_frames: 960,
            fresh: true,
            decay_s: Ramp::new(1.0),
            size_scale: 1.0,
            pre_tap: FadeTap::new(0),
            in_low_pass: Ramp::new(1.0),
            in_high_pass: Ramp::new(0.0),
            width: Ramp::new(1.0),
            mix: Ramp::new(1.0),
            out_gain: Ramp::new(WET_GAIN),
            pre: [DelayLine::empty(), DelayLine::empty()],
            tone: [[0.0; 2]; 2],
            diffusers: std::array::from_fn(|_| std::array::from_fn(|_| DelayLine::empty())),
            diffuser_len: [[1; 4]; 2],
            lines: std::array::from_fn(|_| DelayLine::empty()),
            len_want: [1; LINES],
            len_cur: [1; LINES],
            len_next: [1; LINES],
            fade_left: 0,
            fade_len: 1,
            loop_gains: [[0.0; LINES]; 3],
            loop_steps: [[0.0; LINES]; 3],
            loop_targets: [[0.0; LINES]; 3],
            gliding: false,
            damp: 1.0,
            low: [0.0; LINES],
            stale: true,
            high_cut_hz: 6000.0,
            low_cut_hz: 120.0,
            pre_delay_ms: 20.0,
        };
        for (i, spec) in EffectKind::Reverb.params().iter().enumerate() {
            r.set_param(i as u32, spec.default);
        }
        r
    }

    fn update_size(&mut self) {
        for (want, base) in self.len_want.iter_mut().zip(BASE_MS) {
            let frames = base * 0.001 * self.size_scale * self.sample_rate;
            *want = next_prime(frames.round() as usize);
        }
        // A small room packs more passes into the same decay time; keep the loudness level.
        self.out_gain
            .set(WET_GAIN * self.size_scale.sqrt(), self.ramp_frames);
    }

    fn update_rates(&mut self) {
        let n = self.ramp_frames;
        self.in_low_pass
            .set(dsp::one_pole(self.high_cut_hz, self.sample_rate), n);
        self.in_high_pass
            .set(dsp::one_pole(self.low_cut_hz, self.sample_rate), n);
        let frames = (self.pre_delay_ms * 0.001 * self.sample_rate).round() as usize;
        self.pre_tap
            .set(frames.min(self.pre[0].max_delay().saturating_sub(1)));
    }

    /// Loop gain of a line: the level falls 60 dB in `decay_s` whatever the line's length.
    /// The gains glide there over the next `frames` frames (0: at once).
    fn update_gains(&mut self, frames: usize) {
        let per_frame = LN_60_DB / (self.decay_s.get() * self.sample_rate);
        for i in 0..LINES {
            let cur = per_frame * self.len_cur[i] as f32;
            self.loop_targets[GAIN_CUR][i] = cur.exp();
            self.loop_targets[GAIN_NEXT][i] = (per_frame * self.len_next[i] as f32).exp();
            self.loop_targets[HIGH_SHARE][i] = (cur * (1.0 / HIGH_DECAY_SHARE - 1.0)).exp();
        }
        self.damp = self.in_low_pass.get();
        if frames == 0 {
            self.loop_gains = self.loop_targets;
            self.gliding = false;
        } else {
            for k in 0..3 {
                for i in 0..LINES {
                    self.loop_steps[k][i] =
                        (self.loop_targets[k][i] - self.loop_gains[k][i]) / frames as f32;
                }
            }
            self.gliding = true;
        }
    }
}

impl Default for Reverb {
    fn default() -> Reverb {
        Reverb::new()
    }
}

impl Effect for Reverb {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.ramp_frames = (SMOOTH_S * sample_rate) as u32;
        self.fade_len = ((SIZE_FADE_S * sample_rate) as u32).max(1);
        self.pre_tap.set_fade_frames(self.ramp_frames);
        for line in &mut self.pre {
            line.resize((MAX_PRE_DELAY_S * sample_rate).ceil() as usize + 2);
        }
        for (ch, lens) in self.diffuser_len.iter_mut().enumerate() {
            for (i, len) in lens.iter_mut().enumerate() {
                *len = next_prime((DIFFUSER_MS[ch][i] * 0.001 * sample_rate) as usize);
                self.diffusers[ch][i].resize(*len);
            }
        }
        for (line, base) in self.lines.iter_mut().zip(BASE_MS) {
            // Room for the next prime above the longest length.
            line.resize((base * 0.001 * sample_rate) as usize + 256);
        }
        self.update_size();
        self.update_rates();
        self.reset();
    }

    fn set_param(&mut self, index: u32, value: f32) {
        let Some(v) = dsp::accept(EffectKind::Reverb, index, value) else {
            return;
        };
        let n = self.ramp_frames;
        match index {
            p::DECAY => self.decay_s.set(v, n),
            p::SIZE => {
                self.size_scale = SMALLEST + (1.0 - SMALLEST) * v / 100.0;
                self.update_size();
            }
            p::PRE_DELAY => {
                self.pre_delay_ms = v;
                self.update_rates();
            }
            p::HIGH_CUT => {
                self.high_cut_hz = v;
                self.update_rates();
            }
            p::LOW_CUT => {
                self.low_cut_hz = v;
                self.update_rates();
            }
            p::WIDTH => self.width.set(v / 100.0, n),
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
            self.decay_s.snap();
            self.pre_tap.snap();
            self.in_low_pass.snap();
            self.in_high_pass.snap();
            self.width.snap();
            self.mix.snap();
            self.out_gain.snap();
            self.len_cur = self.len_want;
            self.len_next = self.len_want;
            self.fade_left = 0;
            self.stale = false;
            self.update_gains(0);
        }

        for (left, right) in left.chunks_mut(CHUNK).zip(right.chunks_mut(CHUNK)) {
            if self.fade_left == 0 && self.len_cur != self.len_want {
                self.len_next = self.len_want;
                self.fade_left = self.fade_len;
                self.stale = true;
            }
            if self.decay_s.active() || self.in_low_pass.active() {
                self.decay_s.advance(left.len() as u32);
                self.stale = true;
            }
            if self.stale {
                self.stale = false;
                self.update_gains(left.len());
            }
            self.process_chunk(left, right);
            if self.gliding {
                self.gliding = false;
                self.loop_gains = self.loop_targets;
            }
        }

        if !dsp::all_finite(left, right) {
            self.reset();
            dsp::silence(left, right);
        }
    }

    fn reset(&mut self) {
        for line in self
            .pre
            .iter_mut()
            .chain(self.diffusers.iter_mut().flatten())
            .chain(&mut self.lines)
        {
            line.clear();
        }
        self.tone = [[0.0; 2]; 2];
        self.low = [0.0; LINES];
        self.fresh = true;
    }
}

impl Reverb {
    #[inline]
    fn process_chunk(&mut self, left: &mut [f32], right: &mut [f32]) {
        let damp = self.damp;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let pre = self.pre_tap.tick();
            let low_pass = self.in_low_pass.next();
            let high_pass = self.in_high_pass.next();
            let width = self.width.next();
            let mix = self.mix.next();
            let out_gain = self.out_gain.next();

            let dry = [*l, *r];
            let mut feed = [0.0_f32; 2];
            for ch in 0..2 {
                self.pre[ch].write(dry[ch]);
                let (old, new, x) = pre;
                let mut v = dsp::read_faded(&self.pre[ch], (old + 1, new + 1, x));
                let tone = &mut self.tone[ch];
                tone[0] = dsp::flush(tone[0] + low_pass * (v - tone[0]));
                tone[1] = dsp::flush(tone[1] + high_pass * (tone[0] - tone[1]));
                v = tone[0] - tone[1];
                for (line, &len) in self.diffusers[ch].iter_mut().zip(&self.diffuser_len[ch]) {
                    let delayed = line.read(len);
                    let w = dsp::flush(v + DIFFUSION * delayed);
                    line.write(w);
                    v = delayed - DIFFUSION * w;
                }
                feed[ch] = v;
            }

            let mut fade = 0.0;
            if self.fade_left > 0 {
                self.fade_left -= 1;
                fade = 1.0 - self.fade_left as f32 / self.fade_len as f32;
            }
            if self.gliding {
                for (gains, steps) in self.loop_gains.iter_mut().zip(&self.loop_steps) {
                    for (g, step) in gains.iter_mut().zip(steps) {
                        *g += step;
                    }
                }
            }
            let [gain_cur, gain_next, high_share] = &self.loop_gains;
            let mut y = [0.0_f32; LINES];
            for i in 0..LINES {
                let mut v = self.lines[i].read(self.len_cur[i]) * gain_cur[i];
                if fade > 0.0 {
                    let new = self.lines[i].read(self.len_next[i]) * gain_next[i];
                    v += fade * (new - v);
                }
                let low = dsp::flush(self.low[i] + damp * (v - self.low[i]));
                self.low[i] = low;
                y[i] = low + high_share[i] * (v - low);
            }
            if fade > 0.0 && self.fade_left == 0 {
                // The fade is over: the new length is the current one. Both the gains and
                // their targets swap, so a glide under way stays on course.
                self.len_cur = self.len_next;
                self.loop_gains[GAIN_CUR] = self.loop_gains[GAIN_NEXT];
                self.loop_targets[GAIN_CUR] = self.loop_targets[GAIN_NEXT];
                self.loop_steps[GAIN_CUR] = self.loop_steps[GAIN_NEXT];
                self.stale = true;
            }

            hadamard(&mut y);
            const NORM: f32 = 0.353_553_4;
            for i in 0..LINES {
                y[i] = dsp::flush(y[i] * NORM + feed[i & 1]);
                self.lines[i].write(y[i]);
            }
            // Taken where the lines are fed, so the diffused input sounds right after the
            // pre-delay and not one line length later. Left feeds the even lines, right the odd.
            let wet_l = (y[0] + y[2] + y[4] - y[6]) * out_gain;
            let wet_r = (y[1] + y[3] - y[5] + y[7]) * out_gain;

            let mid = 0.5 * (wet_l + wet_r);
            let side = 0.5 * (wet_l - wet_r) * width;
            *l = dry[0] * (1.0 - mix) + (mid + side) * mix;
            *r = dry[1] * (1.0 - mix) + (mid - side) * mix;
        }
    }
}
