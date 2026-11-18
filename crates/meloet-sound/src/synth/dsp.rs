//! Small pieces the computed instruments share.

use std::f32::consts::PI;

/// The correction that rounds off a step in a naive saw or pulse wave, so the step does not fold
/// back as aliasing (PolyBLEP). `t` is the phase in 0..1, `dt` the phase step per sample.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

#[inline]
pub(super) fn saw(t: f32, dt: f32) -> f32 {
    2.0 * t - 1.0 - poly_blep(t, dt)
}

/// A pulse that is high for the first `width` of its cycle. A width other than 0.5 leaves a DC
/// offset of `2 * width - 1`.
#[inline]
pub(super) fn pulse(t: f32, dt: f32, width: f32) -> f32 {
    let naive = if t < width { 1.0 } else { -1.0 };
    let mut fall = t + 1.0 - width;
    if fall >= 1.0 {
        fall -= 1.0;
    }
    naive + poly_blep(t, dt) - poly_blep(fall, dt)
}

/// Coefficient of [`Svf`] for a cutoff, kept under Nyquist.
#[inline]
pub(super) fn svf_g(cutoff_hz: f32, sample_rate: f32) -> f32 {
    (PI * cutoff_hz.clamp(10.0, 0.45 * sample_rate) / sample_rate).tan()
}

/// State variable filter, trapezoidal form (Andrew Simper, "Linear Trapezoidal Integrated SVF").
/// Stable while its cutoff moves every sample.
#[derive(Clone, Copy)]
pub(super) struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    pub(super) const ZERO: Svf = Svf { ic1: 0.0, ic2: 0.0 };

    /// One sample in; (low, band, high) out. `g` from [`svf_g`], `k` is 1 / Q.
    #[inline]
    pub(super) fn tick(&mut self, x: f32, g: f32, k: f32) -> (f32, f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - k * v1 - v2)
    }
}

/// xorshift32: start phases and noise. Not random in any strong sense, and it need not be.
#[derive(Clone, Copy)]
pub(super) struct Rng(u32);

impl Rng {
    pub(super) const fn new(seed: u32) -> Rng {
        Rng(if seed == 0 { 0x9E37_79B9 } else { seed })
    }

    #[inline]
    pub(super) fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// 0..1
    #[inline]
    pub(super) fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// -1..1
    #[inline]
    pub(super) fn bipolar(&mut self) -> f32 {
        2.0 * self.unit() - 1.0
    }
}

/// Per-sample factor of a decay that falls to 1/e in `tau_s` seconds.
#[inline]
pub(super) fn decay_coef(tau_s: f32, sample_rate: f32) -> f32 {
    (-1.0 / (tau_s * sample_rate)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saw_and_pulse_stay_in_range_and_have_the_right_mean() {
        for dt in [0.001_f32, 0.01, 0.1, 0.3] {
            let (mut t, mut saw_sum, mut pulse_sum) = (0.0_f32, 0.0_f64, 0.0_f64);
            let n = 200_000;
            for _ in 0..n {
                let (s, p) = (saw(t, dt), pulse(t, dt, 0.3));
                assert!(s.abs() <= 1.01 && p.abs() <= 2.01, "dt {dt}: {s} {p}");
                saw_sum += f64::from(s);
                pulse_sum += f64::from(p);
                t += dt;
                if t >= 1.0 {
                    t -= 1.0;
                }
            }
            assert!((saw_sum / f64::from(n)).abs() < 0.01);
            assert!((pulse_sum / f64::from(n) - (2.0 * 0.3 - 1.0)).abs() < 0.01);
        }
    }

    #[test]
    fn low_pass_keeps_dc_and_drops_the_top() {
        let g = svf_g(1000.0, 48_000.0);
        let mut f = Svf::ZERO;
        let mut low = 0.0;
        for _ in 0..4800 {
            low = f.tick(1.0, g, 1.0).0;
        }
        assert!((low - 1.0).abs() < 1e-3);

        let mut f = Svf::ZERO;
        let mut peak = 0.0_f32;
        for i in 0..4800 {
            let x = if i % 2 == 0 { 1.0 } else { -1.0 };
            let low = f.tick(x, g, 1.0).0;
            if i > 480 {
                peak = peak.max(low.abs());
            }
        }
        assert!(peak < 0.01, "{peak}");
    }
}
