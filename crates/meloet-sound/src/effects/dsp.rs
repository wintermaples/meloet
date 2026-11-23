//! Small parts the effects share: parameter ramps, delay lines, a tap that changes its delay by
//! crossfading, and the guards for values that are too small or not numbers.

use meloet_model::effects::EffectKind;

/// How long a parameter takes to reach a new target.
pub const SMOOTH_S: f32 = 0.02;

pub const LN10_OVER_20: f32 = std::f32::consts::LN_10 / 20.0;

#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    (db * LN10_OVER_20).exp()
}

/// The value to keep for `set_param`: `None` for an unknown position or a value that is not a
/// number.
pub fn accept(kind: EffectKind, index: u32, value: f32) -> Option<f32> {
    if value.is_nan() {
        return None;
    }
    kind.clamp(index, value)
}

/// Zero instead of a value so small that the next steps of a decaying loop would be denormal.
#[inline]
pub fn flush(x: f32) -> f32 {
    if x.abs() < 1.0e-18 { 0.0 } else { x }
}

/// False if any sample is NaN or infinite.
pub fn all_finite(left: &[f32], right: &[f32]) -> bool {
    let mut acc = 0.0_f32;
    for x in left.iter().chain(right) {
        acc += x * 0.0;
    }
    acc == 0.0
}

pub fn silence(left: &mut [f32], right: &mut [f32]) {
    left.fill(0.0);
    right.fill(0.0);
}

/// One-pole coefficient for a cutoff: `state += coef * (input - state)` is a 6 dB/oct low-pass.
#[inline]
pub fn one_pole(cutoff_hz: f32, sample_rate: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * cutoff_hz / sample_rate).exp()
}

/// The coefficients of [`Svf`]. `g` is `tan(pi * cutoff / sample_rate)`, `k` is `1 / Q`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SvfCoefs {
    a1: f32,
    a2: f32,
    a3: f32,
    pub k: f32,
}

impl SvfCoefs {
    #[inline]
    pub fn new(g: f32, k: f32) -> SvfCoefs {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        SvfCoefs {
            a1,
            a2,
            a3: g * a2,
            k,
        }
    }
}

/// One channel of a 12 dB/oct state variable filter (Andrew Simper's linear trapezoidal form,
/// cytomic.com "SvfLinearTrapOptimised2"), which stays well behaved while its coefficients move.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// The band-pass and the low-pass output for `v0`; the high-pass is `v0 - k * band - low`.
    #[inline]
    pub fn tick(&mut self, c: &SvfCoefs, v0: f32) -> (f32, f32) {
        let v3 = v0 - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v1, v2)
    }

    #[inline]
    pub fn high(&mut self, c: &SvfCoefs, v0: f32) -> f32 {
        let (band, low) = self.tick(c, v0);
        v0 - c.k * band - low
    }

    /// Once per block: zero instead of states too small to go on decaying.
    pub fn flush(&mut self) {
        self.ic1 = flush(self.ic1);
        self.ic2 = flush(self.ic2);
    }

    pub fn is_finite(&self) -> bool {
        self.ic1.is_finite() && self.ic2.is_finite()
    }
}

/// A value that moves to its target in a straight line.
#[derive(Clone, Copy, Debug)]
pub struct Ramp {
    cur: f32,
    target: f32,
    step: f32,
    left: u32,
}

impl Ramp {
    pub const fn new(value: f32) -> Ramp {
        Ramp {
            cur: value,
            target: value,
            step: 0.0,
            left: 0,
        }
    }

    pub fn set(&mut self, target: f32, frames: u32) {
        if target == self.target {
            return;
        }
        self.target = target;
        if frames == 0 {
            self.snap();
        } else {
            self.step = (target - self.cur) / frames as f32;
            self.left = frames;
        }
    }

    pub fn snap(&mut self) {
        self.cur = self.target;
        self.left = 0;
    }

    #[inline]
    pub fn active(&self) -> bool {
        self.left > 0
    }

    #[inline]
    pub fn get(&self) -> f32 {
        self.cur
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            if self.left == 0 {
                self.cur = self.target;
            } else {
                self.cur += self.step;
            }
        }
        self.cur
    }

    /// The same as calling `next` `frames` times.
    pub fn advance(&mut self, frames: u32) -> f32 {
        if self.left > frames {
            self.left -= frames;
            self.cur += self.step * frames as f32;
        } else {
            self.snap();
        }
        self.cur
    }
}

/// A ring of samples. Per frame: `read` the taps, then `write` once.
pub struct DelayLine {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
}

impl DelayLine {
    pub const fn empty() -> DelayLine {
        DelayLine {
            buf: Vec::new(),
            mask: 0,
            pos: 0,
        }
    }

    /// Makes room for delays up to `max_delay` frames. Allocates.
    pub fn resize(&mut self, max_delay: usize) {
        let len = (max_delay + 1).next_power_of_two();
        self.buf = vec![0.0; len];
        self.mask = len - 1;
        self.pos = 0;
    }

    /// The longest delay `read` can give.
    pub fn max_delay(&self) -> usize {
        self.mask
    }

    pub fn clear(&mut self) {
        self.buf.fill(0.0);
    }

    /// The sample written `delay` writes ago (1 is the last one written).
    #[inline]
    pub fn read(&self, delay: usize) -> f32 {
        self.buf[self.pos.wrapping_sub(delay) & self.mask]
    }

    /// Between the samples `delay` and `delay + 1` writes ago, `frac` of the way to the older
    /// one, on a curve through the two samples on each side (4-point Hermite).
    #[inline]
    pub fn read_frac(&self, delay: usize, frac: f32) -> f32 {
        let y0 = self.read(delay.wrapping_sub(1));
        let y1 = self.read(delay);
        let y2 = self.read(delay + 1);
        let y3 = self.read(delay + 2);
        let c1 = 0.5 * (y2 - y0);
        let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
        ((c3 * frac + c2) * frac + c1) * frac + y1
    }

    #[inline]
    pub fn write(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) & self.mask;
    }
}

/// A delay length that changes by fading from the old tap to the new one, so a jump in the
/// delay never jumps the signal (and never bends the pitch).
#[derive(Clone, Copy, Debug)]
pub struct FadeTap {
    cur: usize,
    next: usize,
    want: usize,
    left: u32,
    len: u32,
}

impl FadeTap {
    pub const fn new(delay: usize) -> FadeTap {
        FadeTap {
            cur: delay,
            next: delay,
            want: delay,
            left: 0,
            len: 1,
        }
    }

    pub fn set_fade_frames(&mut self, frames: u32) {
        self.len = frames.max(1);
    }

    pub fn set(&mut self, delay: usize) {
        self.want = delay;
    }

    pub fn snap(&mut self) {
        self.cur = self.want;
        self.next = self.want;
        self.left = 0;
    }

    /// For this frame: the old tap, the new tap, and how far the fade is (0 = only the old).
    #[inline]
    pub fn tick(&mut self) -> (usize, usize, f32) {
        if self.left == 0 {
            if self.want == self.cur {
                return (self.cur, self.cur, 0.0);
            }
            self.next = self.want;
            self.left = self.len;
        }
        self.left -= 1;
        let x = 1.0 - self.left as f32 / self.len as f32;
        let taps = (self.cur, self.next, x);
        if self.left == 0 {
            self.cur = self.next;
        }
        taps
    }
}

/// Reads a line through a [`FadeTap`]'s answer.
#[inline]
pub fn read_faded(line: &DelayLine, (old, new, x): (usize, usize, f32)) -> f32 {
    let a = line.read(old);
    if x == 0.0 {
        a
    } else {
        a + x * (line.read(new) - a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_arrives_exactly() {
        let mut r = Ramp::new(1.0);
        r.set(2.0, 4);
        let seen: Vec<f32> = (0..5).map(|_| r.next()).collect();
        assert_eq!(seen, [1.25, 1.5, 1.75, 2.0, 2.0]);
        r.set(0.0, 10);
        assert_eq!(r.advance(5), 1.0);
        assert_eq!(r.advance(5), 0.0);
        assert!(!r.active());
    }

    #[test]
    fn delay_line_reads_what_was_written() {
        let mut d = DelayLine::empty();
        d.resize(5);
        for i in 1..=20 {
            d.write(i as f32);
        }
        assert_eq!(d.read(1), 20.0);
        assert_eq!(d.read(5), 16.0);
        assert_eq!(d.read_frac(2, 0.5), 18.5);
    }

    #[test]
    fn fade_tap_moves_in_one_fade() {
        let mut t = FadeTap::new(10);
        t.set_fade_frames(4);
        assert_eq!(t.tick(), (10, 10, 0.0));
        t.set(20);
        let xs: Vec<f32> = (0..4).map(|_| t.tick().2).collect();
        assert_eq!(xs, [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(t.tick(), (20, 20, 0.0));
    }

    #[test]
    fn finite_check() {
        assert!(all_finite(&[0.0, 1.0], &[-1.0, 1.0e30]));
        assert!(!all_finite(&[0.0, f32::NAN], &[0.0, 0.0]));
        assert!(!all_finite(&[0.0, 0.0], &[f32::NEG_INFINITY, 0.0]));
    }
}
