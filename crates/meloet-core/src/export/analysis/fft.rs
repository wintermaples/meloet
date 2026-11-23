//! A small radix-2 FFT of complex numbers, in place.

use std::f64::consts::TAU;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

pub(crate) struct Fft {
    n: usize,
    /// e^(−2πik/n) for k below n/2.
    twiddles: Vec<Complex>,
    /// Where each place goes in the bit-reversed order.
    reversed: Vec<usize>,
}

impl Fft {
    /// `n` is a power of two, at least 2.
    pub fn new(n: usize) -> Fft {
        assert!(n.is_power_of_two() && n >= 2, "an FFT of {n}");
        let bits = n.trailing_zeros();
        let twiddles = (0..n / 2)
            .map(|k| {
                let angle = -TAU * k as f64 / n as f64;
                Complex {
                    re: angle.cos(),
                    im: angle.sin(),
                }
            })
            .collect();
        let reversed = (0..n)
            .map(|i| i.reverse_bits() >> (usize::BITS - bits))
            .collect();
        Fft {
            n,
            twiddles,
            reversed,
        }
    }

    /// X[k] = Σ x[j]·e^(−2πijk/n), without scaling.
    pub fn forward(&self, data: &mut [Complex]) {
        let n = self.n;
        assert_eq!(data.len(), n);
        for (i, &j) in self.reversed.iter().enumerate() {
            if i < j {
                data.swap(i, j);
            }
        }
        let mut size = 2;
        while size <= n {
            let half = size / 2;
            let step = n / size;
            for start in (0..n).step_by(size) {
                for k in 0..half {
                    let w = self.twiddles[k * step];
                    let a = data[start + k];
                    let b = data[start + k + half];
                    let t = Complex {
                        re: b.re * w.re - b.im * w.im,
                        im: b.re * w.im + b.im * w.re,
                    };
                    data[start + k] = Complex {
                        re: a.re + t.re,
                        im: a.im + t.im,
                    };
                    data[start + k + half] = Complex {
                        re: a.re - t.re,
                        im: a.im - t.im,
                    };
                }
            }
            size *= 2;
        }
    }
}
