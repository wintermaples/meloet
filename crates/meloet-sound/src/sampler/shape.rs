//! What a zone of a SoundFont asks of a voice beyond a plain region: two envelopes of six
//! stages, two LFOs and a low-pass filter (docs/architecture.md 20-7). A region with a
//! [`Shape`] is played through here; the regions of the packs never are.

use super::bank::SampleData;
use super::player::interpolate;

/// Decay and release fall in a straight line in dB over this range, then the voice ends.
const RANGE_DB: f32 = 96.0;
/// Frequency of absolute cent 0 (SoundFont 2.01, 8.1.3).
const CENT_0_HZ: f64 = 8.176;
/// The filter is open at this cutoff and is left out unless something moves it or it has a
/// resonance.
const OPEN_CENTS: f32 = 13_500.0;
/// Just over the Q of the flat response, 1/sqrt(2).
const FLAT_Q: f32 = 0.7072;
/// Under this a filter's memory is taken as silence, before it reaches denormal numbers.
const TINY: f32 = 1.0e-12;

/// Times in seconds. `sustain` is how far down the envelope stays while the key is held, as a
/// share of the whole range (0 stays at the top, 1 goes all the way down).
#[derive(Clone, Copy)]
pub(crate) struct EnvShape {
    pub delay_s: f32,
    pub attack_s: f32,
    pub hold_s: f32,
    /// The time to fall the whole range; the fall stops at `sustain`.
    pub decay_s: f32,
    pub sustain: f32,
    /// The time to fall the whole range.
    pub release_s: f32,
    /// Timecents by which hold and decay get shorter for each key above 60.
    pub key_hold: f32,
    pub key_decay: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct LfoShape {
    pub delay_s: f32,
    pub hz: f32,
}

pub(crate) struct Shape {
    pub volume: EnvShape,
    pub modulation: EnvShape,
    /// Cents at the top of the modulation envelope.
    pub mod_to_pitch: f32,
    pub mod_to_filter: f32,
    pub vibrato: LfoShape,
    pub vibrato_to_pitch: f32,
    pub lfo: LfoShape,
    pub lfo_to_pitch: f32,
    pub lfo_to_filter: f32,
    pub lfo_to_volume_db: f32,
    /// Cutoff in absolute cents.
    pub filter_cents: f32,
    pub filter_q: f32,
    /// -1..1
    pub pan: f32,
    /// The loop is left when the key goes up, and the rest of the file plays.
    pub loop_until_release: bool,
}

impl Shape {
    fn filtered(&self) -> bool {
        self.filter_cents < OPEN_CENTS
            || self.filter_q > FLAT_Q
            || self.mod_to_filter != 0.0
            || self.lfo_to_filter != 0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Seg {
    Delay,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

/// One envelope of a voice, counted in frames.
#[derive(Clone, Copy)]
struct Adsr {
    seg: Seg,
    /// Frames left of the delay, attack or hold.
    left: f32,
    attack: f32,
    hold: f32,
    /// 0..1 through the attack.
    rise: f32,
    /// How far down since the hold ended: 0..1 of the range.
    fall: f32,
    decay_step: f32,
    sustain: f32,
    release_step: f32,
}

impl Adsr {
    const IDLE: Adsr = Adsr {
        seg: Seg::Sustain,
        left: 0.0,
        attack: 0.0,
        hold: 0.0,
        rise: 1.0,
        fall: 1.0,
        decay_step: 0.0,
        sustain: 1.0,
        release_step: 1.0,
    };

    fn start(shape: &EnvShape, key: u8, rate: f32) -> Adsr {
        let by_key = |timecents: f32| ((60.0 - f32::from(key)) * timecents / 1200.0).exp2();
        let mut env = Adsr {
            seg: Seg::Delay,
            left: shape.delay_s * rate,
            attack: shape.attack_s * rate,
            hold: shape.hold_s * by_key(shape.key_hold) * rate,
            rise: 0.0,
            fall: 0.0,
            decay_step: 1.0 / (shape.decay_s * by_key(shape.key_decay) * rate).max(1.0),
            sustain: shape.sustain,
            release_step: 1.0 / (shape.release_s * rate).max(1.0),
        };
        if env.left < 1.0 {
            env.seg = Seg::Attack;
            env.left = env.attack;
            if env.left < 1.0 {
                env.seg = Seg::Hold;
                env.left = env.hold;
                env.rise = 1.0;
            }
        }
        env
    }

    fn advance(&mut self, frames: f32) {
        let mut n = frames;
        loop {
            match self.seg {
                Seg::Delay | Seg::Attack | Seg::Hold if self.left > n => {
                    self.left -= n;
                    if self.seg == Seg::Attack {
                        self.rise = 1.0 - self.left / self.attack;
                    }
                    return;
                }
                Seg::Delay => {
                    n -= self.left;
                    self.seg = Seg::Attack;
                    self.left = self.attack;
                }
                Seg::Attack => {
                    n -= self.left;
                    self.rise = 1.0;
                    self.seg = Seg::Hold;
                    self.left = self.hold;
                }
                Seg::Hold => {
                    n -= self.left;
                    self.seg = Seg::Decay;
                }
                Seg::Decay => {
                    self.fall += self.decay_step * n;
                    if self.fall >= self.sustain {
                        self.fall = self.sustain;
                        self.seg = Seg::Sustain;
                    }
                    return;
                }
                Seg::Sustain => return,
                Seg::Release => {
                    self.fall = (self.fall + self.release_step * n).min(1.0);
                    return;
                }
            }
        }
    }

    fn rising(&self) -> bool {
        matches!(self.seg, Seg::Delay | Seg::Attack)
    }

    /// Loudness: the attack is a straight line in amplitude, the rest a straight line in dB.
    fn level(&self) -> f32 {
        if self.rising() {
            self.rise
        } else if self.fall >= 1.0 {
            0.0
        } else {
            (-self.fall * (RANGE_DB / 20.0 * std::f32::consts::LN_10)).exp()
        }
    }

    /// 0..1 for pitch and cutoff: the attack is convex (SoundFont 2.01, 8.1.2 attackModEnv),
    /// the rest a straight line.
    fn amount(&self) -> f32 {
        if self.rising() {
            if self.rise <= 0.0 {
                0.0
            } else {
                (1.0 + 400.0 / 960.0 * self.rise.log10()).max(0.0)
            }
        } else {
            1.0 - self.fall
        }
    }

    fn ended(&self) -> bool {
        !self.rising() && self.fall >= 1.0
    }

    fn release_level(&mut self) {
        if self.rising() {
            // The release goes on from the loudness the attack had reached.
            self.fall = (-self.rise.ln() * (20.0 / std::f32::consts::LN_10 / RANGE_DB)).min(1.0);
        }
        self.seg = Seg::Release;
    }

    fn release_amount(&mut self) {
        if self.rising() {
            // The release goes on from the amount the attack had reached.
            self.fall = 1.0 - self.amount();
        }
        self.seg = Seg::Release;
    }
}

/// A triangle wave that starts at 0 on its way up, after its delay.
#[derive(Clone, Copy)]
struct Lfo {
    wait: f32,
    phase: f32,
    step: f32,
}

impl Lfo {
    fn start(shape: &LfoShape, rate: f32) -> Lfo {
        Lfo {
            wait: shape.delay_s * rate,
            phase: 0.0,
            step: shape.hz / rate,
        }
    }

    fn advance(&mut self, frames: f32) {
        let mut n = frames;
        if self.wait > 0.0 {
            if self.wait >= n {
                self.wait -= n;
                return;
            }
            n -= self.wait;
            self.wait = 0.0;
        }
        self.phase += self.step * n;
        self.phase -= self.phase.floor();
    }

    /// -1..1
    fn value(&self) -> f32 {
        let p = self.phase;
        if p < 0.25 {
            4.0 * p
        } else if p < 0.75 {
            2.0 - 4.0 * p
        } else {
            4.0 * p - 4.0
        }
    }
}

/// A two-pole low-pass (the cookbook form of Robert Bristow-Johnson). `b1` is twice `b0` and
/// `b2` is `b0`, so three numbers describe it.
#[derive(Clone, Copy)]
struct Filter {
    on: bool,
    /// The cutoff the numbers below were worked out for; NaN before the first chunk.
    cents: f32,
    b0: f32,
    a1: f32,
    a2: f32,
    s1: f32,
    s2: f32,
}

impl Filter {
    const OFF: Filter = Filter {
        on: false,
        cents: f32::NAN,
        b0: 1.0,
        a1: 0.0,
        a2: 0.0,
        s1: 0.0,
        s2: 0.0,
    };

    /// Filters a chunk in place while the cutoff moves to `cents` in a straight line of the
    /// three numbers, so a jump of the cutoff does not click.
    fn run(&mut self, cents: f32, q: f32, rate: f32, x: &mut [f32]) {
        let (mut b0, mut a1, mut a2) = (self.b0, self.a1, self.a2);
        let (mut b0_step, mut a1_step, mut a2_step) = (0.0, 0.0, 0.0);
        if cents != self.cents {
            let hz =
                (CENT_0_HZ * f64::from(cents / 1200.0).exp2()).clamp(5.0, 0.45 * f64::from(rate));
            let (sin, cos) = (std::f64::consts::TAU * hz / f64::from(rate)).sin_cos();
            let alpha = sin / (2.0 * f64::from(q));
            let a0 = 1.0 / (1.0 + alpha);
            let to_b0 = ((1.0 - cos) * 0.5 * a0) as f32;
            let to_a1 = (-2.0 * cos * a0) as f32;
            let to_a2 = ((1.0 - alpha) * a0) as f32;
            if self.cents.is_nan() {
                (b0, a1, a2) = (to_b0, to_a1, to_a2);
            } else {
                let n = x.len() as f32;
                b0_step = (to_b0 - b0) / n;
                a1_step = (to_a1 - a1) / n;
                a2_step = (to_a2 - a2) / n;
            }
            self.cents = cents;
            (self.b0, self.a1, self.a2) = (to_b0, to_a1, to_a2);
        }
        let (mut s1, mut s2) = (self.s1, self.s2);
        for x in x.iter_mut() {
            b0 += b0_step;
            a1 += a1_step;
            a2 += a2_step;
            let input = *x;
            let y = b0 * input + s1;
            s1 = 2.0 * b0 * input - a1 * y + s2;
            s2 = b0 * input - a2 * y;
            *x = y;
        }
        if !(s1.is_finite() && s2.is_finite()) {
            x.fill(0.0);
            (s1, s2) = (0.0, 0.0);
        }
        self.s1 = if s1.abs() < TINY { 0.0 } else { s1 };
        self.s2 = if s2.abs() < TINY { 0.0 } else { s2 };
    }
}

/// What one chunk of a voice plays with.
pub(super) struct Moved {
    pub alive: bool,
    /// Loudness at the end of the chunk, the envelope and the LFO together.
    pub level: f32,
    pub pitch_cents: f32,
}

/// The part of a voice that follows a [`Shape`].
#[derive(Clone, Copy)]
pub(super) struct Shaped {
    volume: Adsr,
    modulation: Adsr,
    vibrato: Lfo,
    lfo: Lfo,
    filter: Filter,
    filter_cents: f32,
    pub released: bool,
    /// The first chunk has nothing to come from.
    pub fresh: bool,
    /// Loudness of each side at the end of the last chunk.
    pub amp_left: f32,
    pub amp_right: f32,
    /// The place between left and right the two shares below were worked out for.
    pub pan: f32,
    pub pan_left: f32,
    pub pan_right: f32,
}

impl Shaped {
    pub const IDLE: Shaped = Shaped {
        volume: Adsr::IDLE,
        modulation: Adsr::IDLE,
        vibrato: Lfo {
            wait: 0.0,
            phase: 0.0,
            step: 0.0,
        },
        lfo: Lfo {
            wait: 0.0,
            phase: 0.0,
            step: 0.0,
        },
        filter: Filter::OFF,
        filter_cents: 0.0,
        released: false,
        fresh: true,
        amp_left: 0.0,
        amp_right: 0.0,
        pan: f32::NAN,
        pan_left: 0.0,
        pan_right: 0.0,
    };

    pub fn start(shape: &Shape, key: u8, rate: f32) -> Shaped {
        Shaped {
            volume: Adsr::start(&shape.volume, key, rate),
            modulation: Adsr::start(&shape.modulation, key, rate),
            vibrato: Lfo::start(&shape.vibrato, rate),
            lfo: Lfo::start(&shape.lfo, rate),
            filter: Filter {
                on: shape.filtered(),
                ..Filter::OFF
            },
            ..Shaped::IDLE
        }
    }

    /// True when the voice is at full loudness from its first frame.
    pub fn starts_full(&self) -> bool {
        !self.volume.rising()
    }

    pub fn release(&mut self) {
        self.released = true;
        self.volume.release_level();
        self.modulation.release_amount();
    }

    /// Moves the envelopes and LFOs on by a chunk.
    pub fn advance(&mut self, shape: &Shape, frames: usize) -> Moved {
        let n = frames as f32;
        self.volume.advance(n);
        self.modulation.advance(n);
        self.vibrato.advance(n);
        self.lfo.advance(n);
        let amount = self.modulation.amount();
        let (vibrato, lfo) = (self.vibrato.value(), self.lfo.value());
        let mut level = self.volume.level();
        if shape.lfo_to_volume_db != 0.0 {
            level *= (lfo * shape.lfo_to_volume_db * (std::f32::consts::LN_10 / 20.0)).exp();
        }
        self.filter_cents =
            shape.filter_cents + amount * shape.mod_to_filter + lfo * shape.lfo_to_filter;
        Moved {
            alive: !self.volume.ended(),
            level,
            pitch_cents: amount * shape.mod_to_pitch
                + vibrato * shape.vibrato_to_pitch
                + lfo * shape.lfo_to_pitch,
        }
    }

    /// Equal power between the two sides, 3 dB down each in the middle.
    pub fn set_pan(&mut self, pan: f32) {
        if pan != self.pan {
            self.pan = pan;
            let (sin, cos) = ((pan + 1.0) * std::f32::consts::FRAC_PI_4).sin_cos();
            (self.pan_left, self.pan_right) = (cos, sin);
        }
    }

    /// Adds a chunk of the voice to the buffers; loudness goes to `to` (left, right) in a
    /// straight line. Returns false when a file without a loop has run out.
    #[allow(clippy::too_many_arguments)]
    pub fn mix(
        &mut self,
        shape: &Shape,
        sample: &SampleData,
        looped: Option<(usize, usize)>,
        pos: &mut f64,
        step: f64,
        to: (f32, f32),
        rate: f32,
        left: &mut [f32],
        right: &mut [f32],
    ) -> bool {
        let mut chunk = [0.0_f32; super::player::CHUNK];
        let x = &mut chunk[..left.len()];
        let more = fetch(sample, looped, pos, step, x);
        if self.filter.on {
            self.filter.run(self.filter_cents, shape.filter_q, rate, x);
        }
        let n = x.len() as f32;
        let (mut amp_left, mut amp_right) = (self.amp_left, self.amp_right);
        let (step_left, step_right) = ((to.0 - amp_left) / n, (to.1 - amp_right) / n);
        for ((l, r), x) in left.iter_mut().zip(right.iter_mut()).zip(x.iter()) {
            amp_left += step_left;
            amp_right += step_right;
            *l += x * amp_left;
            *r += x * amp_right;
        }
        (self.amp_left, self.amp_right) = to;
        more
    }
}

/// Reads frames of a file's first channel from `pos` on, `step` apart, a loop's end joined to
/// its start. Returns false when a file without a loop has run out; the rest is silence.
fn fetch(
    sample: &SampleData,
    looped: Option<(usize, usize)>,
    pos: &mut f64,
    step: f64,
    out: &mut [f32],
) -> bool {
    let data = &sample.data[..];
    let stride = sample.channels;
    let limit = looped.map_or(sample.frames, |(_, end)| end);
    let first = *pos as usize;
    let last = (*pos + step * (out.len() - 1) as f64) as usize;
    let mut at = *pos;

    if stride == 1 && first >= 1 && last + 3 < limit {
        for x in out.iter_mut() {
            let i = at as usize;
            let t = (at - i as f64) as f32;
            let w = &data[i - 1..i + 3];
            *x = interpolate(w[0].into(), w[1].into(), w[2].into(), w[3].into(), t);
            at += step;
        }
        *pos = at;
        return true;
    }

    let tap = |frame: isize| -> f32 {
        let mut frame = frame;
        if let Some((start, end)) = looped
            && frame >= end as isize
        {
            frame = start as isize + (frame - end as isize) % (end - start) as isize;
        }
        if frame < 0 || frame as usize >= sample.frames {
            0.0
        } else {
            data[frame as usize * stride].into()
        }
    };
    let mut done = 0;
    let mut more = true;
    while done < out.len() && more {
        let i = at as isize;
        let t = (at - i as f64) as f32;
        out[done] = interpolate(tap(i - 1), tap(i), tap(i + 1), tap(i + 2), t);
        done += 1;
        at += step;
        match looped {
            Some((start, end)) if at >= end as f64 => {
                at = start as f64 + (at - end as f64) % (end - start) as f64;
            }
            None => more = at < sample.frames as f64,
            _ => {}
        }
    }
    out[done..].fill(0.0);
    *pos = at;
    more
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key let go in the attack of the modulation envelope: pitch and cutoff go on from where
    /// the attack had brought them, and fall from there.
    #[test]
    fn the_modulation_release_goes_on_from_the_attack() {
        let shape = EnvShape {
            delay_s: 0.0,
            attack_s: 1.0,
            hold_s: 0.0,
            decay_s: 1.0,
            sustain: 0.5,
            release_s: 1.0,
            key_hold: 0.0,
            key_decay: 0.0,
        };
        for frames in [4_800.0, 12_000.0, 24_000.0, 36_000.0] {
            let mut env = Adsr::start(&shape, 60, 48_000.0);
            env.advance(frames);
            let before = env.amount();
            env.release_amount();
            assert!((env.amount() - before).abs() < 1e-6, "{frames}: {before}");
            env.advance(32.0);
            assert!(env.amount() < before, "{frames}");
        }
    }
}
