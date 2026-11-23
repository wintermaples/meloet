//! The master's sound from low to high (E7): 24 bands on a log scale from 40 Hz to 16 kHz, each
//! −72..0 dB as 0..255, for the bars left of 「マスター」 in the top band.
//!
//! 2048-point FFTs of the frames with a Hann window, one every 1024 frames. A band's level is
//! the power of the two sides together; a full-scale sine (both sides) whose main lobe is all in
//! a band reads 0 dB. A band narrower than two bins (the lowest ones; more of them at 96 kHz)
//! is read by a DFT at its middle instead, so a sine still lands in its own band. The bars rise
//! at once and fall 30 dB a second (sound time: the frames that came).
//!
//! Everything is made in [`Spectrum::new`]: nothing is allocated while frames come in.

use std::f64::consts::TAU;
use std::time::Duration;

use meloet_protocol::SPECTRUM_BANDS;

use crate::export::analysis::fft::{Complex, Fft};

/// Frames of one FFT.
pub const FFT_FRAMES: usize = 2048;
/// Frames from one FFT to the next: half of one.
pub const HOP_FRAMES: usize = FFT_FRAMES / 2;
pub const LOWEST_HZ: f64 = 40.0;
pub const HIGHEST_HZ: f64 = 16_000.0;
/// The bottom of a bar (0); 0 dB is its top (255).
pub const FLOOR_DB: f64 = -72.0;
pub const FALL_DB_PER_S: f64 = 30.0;

/// A window with no sample louder than this reads 0 in every band, so its FFT is left out: a
/// band holds at most 4·x² of it (−74 dB), and anything under −71.86 dB is 0.
const QUIET: f32 = 1e-4;

/// One side's |X|² to the power of a band: with a sine of amplitude A, the bins of its main
/// lobe add up to A² (Hann: Σw² = 3N/8, half of the power on the positive bins).
const SIDE_SCALE: f64 = 32.0 / (3.0 * (FFT_FRAMES * FFT_FRAMES) as f64);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Band {
    /// The bins `from..to`, two or more: their power added.
    Bins { from: usize, to: usize },
    /// Fewer than two bins: the DFT at the band's middle (Goertzel), `coeff` = 2·cos(ω).
    Middle { coeff: f64 },
    /// Above half the rate: nothing.
    Out,
}

/// The lower edge of band `i` (`i` = 24: the upper edge of the last).
fn edge(i: usize) -> f64 {
    LOWEST_HZ * (HIGHEST_HZ / LOWEST_HZ).powf(i as f64 / SPECTRUM_BANDS as f64)
}

/// How each band is read at `rate`.
fn plan(rate: u32) -> [Band; SPECTRUM_BANDS] {
    let rate = f64::from(rate);
    let bin_hz = rate / FFT_FRAMES as f64;
    std::array::from_fn(|band| {
        let (low, high) = (edge(band), edge(band + 1));
        let from = (low / bin_hz).ceil() as usize;
        let to = ((high / bin_hz).ceil() as usize).min(FFT_FRAMES / 2);
        let middle = (low * high).sqrt();
        if to >= from + 2 {
            Band::Bins { from, to }
        } else if middle < rate / 2.0 {
            Band::Middle {
                coeff: 2.0 * (TAU * middle / rate).cos(),
            }
        } else {
            Band::Out
        }
    })
}

/// The bands of one stream of frames.
pub struct Spectrum {
    fft: Fft,
    window: Box<[f64]>,
    /// The last `FFT_FRAMES` frames; the oldest at `at`, where the next one goes.
    frames: Box<[[f32; 2]]>,
    at: usize,
    since_hop: usize,
    /// Frames since the last one louder than `QUIET`, up to `FFT_FRAMES`.
    quiet: usize,
    work: Box<[Complex]>,
    rate: u32,
    bands: [Band; SPECTRUM_BANDS],
    /// dB as shown, falling; −∞ below the floor.
    shown: [f64; SPECTRUM_BANDS],
    /// Frames that came since the bars last fell.
    unfallen: usize,
    ffts: u64,
}

impl Spectrum {
    pub fn new(rate: u32) -> Spectrum {
        let mut spectrum = Spectrum {
            fft: Fft::new(FFT_FRAMES),
            window: (0..FFT_FRAMES)
                .map(|n| 0.5 - 0.5 * (TAU * n as f64 / FFT_FRAMES as f64).cos())
                .collect(),
            frames: vec![[0.0; 2]; FFT_FRAMES].into_boxed_slice(),
            at: 0,
            since_hop: 0,
            quiet: FFT_FRAMES,
            work: vec![Complex::default(); FFT_FRAMES].into_boxed_slice(),
            rate: 0,
            bands: [Band::Out; SPECTRUM_BANDS],
            shown: [f64::NEG_INFINITY; SPECTRUM_BANDS],
            unfallen: 0,
            ffts: 0,
        };
        spectrum.restart(rate);
        spectrum
    }

    /// Silence before, bars at the bottom; frames from now on come at `rate`.
    pub fn restart(&mut self, rate: u32) {
        self.forget_frames();
        self.shown = [f64::NEG_INFINITY; SPECTRUM_BANDS];
        self.unfallen = 0;
        let rate = rate.max(1);
        if rate != self.rate {
            self.rate = rate;
            self.bands = plan(rate);
        }
    }

    /// The next frames, left and right, in order.
    pub fn push(&mut self, frames: &[[f32; 2]]) {
        for &frame in frames {
            self.frames[self.at] = frame;
            self.at = (self.at + 1) % FFT_FRAMES;
            let loud = frame[0].abs() > QUIET || frame[1].abs() > QUIET;
            self.quiet = if loud {
                0
            } else {
                (self.quiet + 1).min(FFT_FRAMES)
            };
            self.unfallen += 1;
            self.since_hop += 1;
            if self.since_hop == HOP_FRAMES {
                self.since_hop = 0;
                self.hop();
            }
        }
        self.fall_frames();
    }

    /// No frames came for this long (the stream stopped): the bars fall as in silence, and the
    /// frames that come next do not join those before.
    pub fn fall_for(&mut self, time: Duration) {
        if self.quiet < FFT_FRAMES || self.since_hop != 0 {
            self.forget_frames();
        }
        self.fall(FALL_DB_PER_S * time.as_secs_f64());
    }

    /// The bars, low to high, 0..255.
    pub fn bars(&self) -> [u8; SPECTRUM_BANDS] {
        std::array::from_fn(|band| bar(self.shown[band]))
    }

    /// FFTs done so far.
    pub fn ffts(&self) -> u64 {
        self.ffts
    }

    fn hop(&mut self) {
        self.fall_frames();
        if self.quiet >= FFT_FRAMES {
            return;
        }
        self.ffts += 1;
        for (n, z) in self.work.iter_mut().enumerate() {
            let [left, right] = self.frames[(self.at + n) % FFT_FRAMES];
            let w = self.window[n];
            *z = Complex {
                re: f64::from(left) * w,
                im: f64::from(right) * w,
            };
        }
        // Before the FFT works over them in place.
        for (band, shown) in self.bands.iter().zip(&mut self.shown) {
            if let Band::Middle { coeff } = *band {
                raise(shown, middle_power(&self.work, coeff));
            }
        }
        self.fft.forward(&mut self.work);
        for (band, shown) in self.bands.iter().zip(&mut self.shown) {
            if let Band::Bins { from, to } = *band {
                // Left in the real part, right in the imaginary: |X_L|² + |X_R|² of bin k is
                // (|Z_k|² + |Z_{N−k}|²) / 2.
                let both: f64 = (from..to)
                    .map(|k| self.work[k].norm_sqr() + self.work[FFT_FRAMES - k].norm_sqr())
                    .sum();
                raise(shown, both / 4.0 * SIDE_SCALE);
            }
        }
    }

    fn forget_frames(&mut self) {
        self.frames.fill([0.0; 2]);
        self.at = 0;
        self.since_hop = 0;
        self.quiet = FFT_FRAMES;
    }

    fn fall_frames(&mut self) {
        if self.unfallen > 0 {
            let seconds = self.unfallen as f64 / f64::from(self.rate);
            self.unfallen = 0;
            self.fall(FALL_DB_PER_S * seconds);
        }
    }

    fn fall(&mut self, db: f64) {
        for shown in &mut self.shown {
            *shown -= db;
            if *shown < FLOOR_DB {
                *shown = f64::NEG_INFINITY;
            }
        }
    }
}

/// The power of the two sides at the frequency of `coeff`, by Goertzel over the windowed
/// frames (left in `re`, right in `im`).
fn middle_power(work: &[Complex], coeff: f64) -> f64 {
    let (mut l1, mut l2, mut r1, mut r2) = (0.0, 0.0, 0.0, 0.0);
    for z in work {
        let l = z.re + coeff * l1 - l2;
        (l2, l1) = (l1, l);
        let r = z.im + coeff * r1 - r2;
        (r2, r1) = (r1, r);
    }
    let side = |s1: f64, s2: f64| s1 * s1 + s2 * s2 - coeff * s1 * s2;
    (side(l1, l2) + side(r1, r2)) / 2.0 * SIDE_SCALE
}

/// Never over the top: a bar over 0 dB would stay at 255 until it had fallen that far.
fn raise(shown: &mut f64, power: f64) {
    let db = 10.0 * power.log10();
    if db > *shown {
        *shown = db.min(0.0);
    }
}

fn bar(db: f64) -> u8 {
    if db.is_nan() || db <= FLOOR_DB {
        return 0;
    }
    ((db - FLOOR_DB) / -FLOOR_DB * 255.0).round().min(255.0) as u8
}

#[cfg(test)]
mod tests;
