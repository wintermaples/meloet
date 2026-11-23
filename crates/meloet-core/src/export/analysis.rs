//! What the AI link hears in a rendering, as numbers (docs/architecture.md 24-12): loudness
//! as ITU-R BS.1770-4 measures it, the level in frequency bands by Welch's method, how much
//! two tracks sound in one band at the same time, and the levels of sections. The samples
//! come in as the export's thread takes them, block after block; nothing is kept but sums.

mod fft;
#[cfg(test)]
mod tests;

use meloet_model::TickRange;

use crate::ai::{
    AiAnalysis, AiBand, AiBandLayout, AiBandLevel, AiLoudness, AiOverlap, AiSection, AiShared,
};
use fft::{Complex, Fft};

/// BS.1770: a block's loudness is this plus 10·log10 of its K-weighted power.
const OFFSET: f64 = -0.691;
const ABSOLUTE_GATE: f64 = -70.0;
const RELATIVE_GATE: f64 = -10.0;
/// A gating block is 4 steps of 100 ms (400 ms, 75 % overlap); a short-term window 30 (3 s).
const BLOCK_STEPS: usize = 4;
const SHORT_STEPS: usize = 30;
/// Filter states below this are set to 0: they only fade, and slowly once denormal.
const TINY: f64 = 1e-30;
/// A window of the band analysis is the power of two of frames at or above this.
const WINDOW_S: f64 = 0.15;
/// −90 dBFS: a band this quiet in a window does not count as sounding when two are compared.
const SILENT_BAND: f64 = 1e-9;

const FIVE: [(&str, f64, f64); 5] = [
    ("low", 0.0, 120.0),
    ("low_mid", 120.0, 500.0),
    ("mid", 500.0, 2_000.0),
    ("high_mid", 2_000.0, 6_000.0),
    ("high", 6_000.0, f64::INFINITY),
];

const OCTAVES: [&str; 10] = [
    "31.5Hz", "63Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz",
];

/// The bands of `layout`, their edges cut at half the sample rate.
pub(crate) fn bands(layout: AiBandLayout, rate: u32) -> Vec<AiBand> {
    let nyquist = f64::from(rate) / 2.0;
    let band = |name, low: f64, high: f64| AiBand {
        name,
        low_hz: low.min(nyquist),
        high_hz: high.min(nyquist),
    };
    match layout {
        AiBandLayout::Five => (FIVE.iter())
            .map(|&(name, low, high)| band(name, low, high))
            .collect(),
        AiBandLayout::Octave => {
            // Base 10: an octave is 10^(3/10), mid-bands are 1 kHz times its powers.
            let g = 10f64.powf(0.3);
            (OCTAVES.iter().zip(-5..=4))
                .map(|(&name, x)| {
                    let mid = 1000.0 * g.powi(x);
                    band(name, mid / g.sqrt(), mid * g.sqrt())
                })
                .collect()
        }
    }
}

/// The frames of a window of the band analysis.
pub(crate) fn window_frames(rate: u32) -> usize {
    ((f64::from(rate) * WINDOW_S) as usize)
        .next_power_of_two()
        .clamp(256, 1 << 16)
}

/// The two stages of K-weighting at `rate`, each as [b0, b1, b2, a1, a2]: the shelf and the
/// high-pass of BS.1770-4, computed for any rate as libebur128 does; at 48 kHz they are the
/// standard's tables.
pub(crate) fn k_weighting(rate: f64) -> [[f64; 5]; 2] {
    use std::f64::consts::PI;
    let (f0, gain, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (PI * f0 / rate).tan();
    let vh = 10f64.powf(gain / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = [
        (vh + vb * k / q + k * k) / a0,
        2.0 * (k * k - vh) / a0,
        (vh - vb * k / q + k * k) / a0,
        2.0 * (k * k - 1.0) / a0,
        (1.0 - k / q + k * k) / a0,
    ];
    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (PI * f0 / rate).tan();
    let a0 = 1.0 + k / q + k * k;
    let high_pass = [
        1.0,
        -2.0,
        1.0,
        2.0 * (k * k - 1.0) / a0,
        (1.0 - k / q + k * k) / a0,
    ];
    [shelf, high_pass]
}

/// A biquad in transposed direct form II.
#[derive(Debug, Clone, Copy)]
struct Biquad {
    c: [f64; 5],
    s1: f64,
    s2: f64,
}

impl Biquad {
    fn new(c: [f64; 5]) -> Biquad {
        Biquad {
            c,
            s1: 0.0,
            s2: 0.0,
        }
    }

    fn run(&mut self, x: f64) -> f64 {
        let [b0, b1, b2, a1, a2] = self.c;
        let y = b0 * x + self.s1;
        self.s1 = b1 * x - a1 * y + self.s2;
        self.s2 = b2 * x - a2 * y;
        y
    }

    fn flush(&mut self) {
        for s in [&mut self.s1, &mut self.s2] {
            if s.abs() < TINY {
                *s = 0.0;
            }
        }
    }
}

fn loudness(power: f64) -> f64 {
    OFFSET + 10.0 * power.log10()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len().max(1) as f64
}

/// The K-weighted power (both channels added) of each window of `steps` steps, the window
/// moving a step at a time; only whole windows.
fn windows(sums: &[f64], steps: usize, step_frames: usize) -> impl Iterator<Item = f64> + '_ {
    let scale = 1.0 / (steps * step_frames) as f64;
    sums.windows(steps)
        .map(move |w| w.iter().sum::<f64>() * scale)
}

/// Integrated loudness in LUFS from the K-weighted squares summed over each 100 ms step:
/// gating blocks of 400 ms, the absolute gate at −70 LUFS and the relative one 10 LU below
/// what passes it (BS.1770-4, 2.8). `None`: no block passes.
pub(crate) fn integrated(sums: &[f64], step_frames: usize) -> Option<f32> {
    let gated: Vec<f64> = windows(sums, BLOCK_STEPS, step_frames)
        .filter(|&p| p > 0.0 && loudness(p) > ABSOLUTE_GATE)
        .collect();
    if gated.is_empty() {
        return None;
    }
    let relative = loudness(mean(&gated)) + RELATIVE_GATE;
    let kept: Vec<f64> = (gated.into_iter())
        .filter(|&p| loudness(p) > relative)
        .collect();
    (!kept.is_empty()).then(|| loudness(mean(&kept)) as f32)
}

/// The loudest short-term loudness (3 s, not gated, EBU Tech 3341) in LUFS. `None`: shorter
/// than 3 s, or silent.
pub(crate) fn short_term_max(sums: &[f64], step_frames: usize) -> Option<f32> {
    let most = windows(sums, SHORT_STEPS, step_frames).fold(0.0, f64::max);
    (most > 0.0).then(|| loudness(most) as f32)
}

/// K-weighted squares summed over steps of 100 ms.
#[derive(Debug)]
struct Steps {
    frames: usize,
    sum: f64,
    count: usize,
    sums: Vec<f64>,
}

impl Steps {
    fn new(frames: usize) -> Steps {
        Steps {
            frames,
            sum: 0.0,
            count: 0,
            sums: Vec::new(),
        }
    }

    /// True when a step ended.
    fn add(&mut self, power: f64) -> bool {
        self.sum += power;
        self.count += 1;
        if self.count < self.frames {
            return false;
        }
        self.sums.push(self.sum);
        self.sum = 0.0;
        self.count = 0;
        true
    }
}

/// One section as it is heard.
#[derive(Debug)]
struct Part {
    peak: f32,
    squares: f64,
    frames: u64,
    steps: Steps,
}

impl Part {
    fn new(step_frames: usize) -> Part {
        Part {
            peak: 0.0,
            squares: 0.0,
            frames: 0,
            steps: Steps::new(step_frames),
        }
    }

    fn add(&mut self, l: f64, r: f64, k: f64) {
        self.peak = self.peak.max(l.abs().max(r.abs()) as f32);
        self.squares += l * l + r * r;
        self.frames += 1;
        self.steps.add(k);
    }

    fn levels(&self) -> AiLoudness {
        let rms = match self.frames {
            0 => 0.0,
            n => (self.squares / (2 * n) as f64).sqrt() as f32,
        };
        AiLoudness {
            peak: self.peak,
            rms,
            lufs: integrated(&self.steps.sums, self.steps.frames),
            lufs_short_max: short_term_max(&self.steps.sums, self.steps.frames),
            frames: self.frames,
        }
    }
}

#[derive(Debug)]
struct Sections {
    /// Frames where sections after the first begin.
    cuts: Vec<u64>,
    done: Vec<AiLoudness>,
    now: Part,
}

impl Sections {
    fn reach(&mut self, frame: u64) {
        while (self.cuts.get(self.done.len())).is_some_and(|&cut| frame >= cut) {
            self.done.push(self.now.levels());
            self.now = Part::new(self.now.steps.frames);
        }
    }

    /// One for each section, those the range did not reach empty.
    fn finish(mut self) -> Vec<AiLoudness> {
        self.done.push(self.now.levels());
        self.done.resize(self.cuts.len() + 1, AiLoudness::default());
        self.done
    }
}

/// Band powers by Welch's method: Hann windows overlapping by half, the power of each bin of
/// both channels added and summed into the bands.
struct Spectrum {
    fft: Fft,
    window: Vec<f64>,
    left: Vec<f64>,
    right: Vec<f64>,
    filled: usize,
    work: Vec<Complex>,
    /// The band of each bin from 0 to n/2.
    band_of: Vec<Option<usize>>,
    /// The band powers of the window at hand.
    now: Vec<f64>,
    sums: Vec<f64>,
    total: f64,
    windows: u64,
    /// The band powers of every window, when kept for comparing.
    kept: Option<Vec<f64>>,
}

fn hann(len: usize) -> Vec<f64> {
    use std::f64::consts::TAU;
    (0..len)
        .map(|i| 0.5 - 0.5 * (TAU * i as f64 / len as f64).cos())
        .collect()
}

impl Spectrum {
    fn new(rate: u32, bands: &[AiBand], keep: bool) -> Spectrum {
        let n = window_frames(rate);
        let nyquist = f64::from(rate) / 2.0;
        let band_of = (0..=n / 2)
            .map(|k| {
                let hz = k as f64 * f64::from(rate) / n as f64;
                (bands.iter()).position(|b| {
                    hz >= b.low_hz && (hz < b.high_hz || (b.high_hz >= nyquist && hz <= nyquist))
                })
            })
            .collect();
        Spectrum {
            fft: Fft::new(n),
            window: hann(n),
            left: vec![0.0; n],
            right: vec![0.0; n],
            filled: 0,
            work: vec![Complex::default(); n],
            band_of,
            now: vec![0.0; bands.len()],
            sums: vec![0.0; bands.len()],
            total: 0.0,
            windows: 0,
            kept: keep.then(Vec::new),
        }
    }

    fn push(&mut self, l: f64, r: f64) {
        self.left[self.filled] = l;
        self.right[self.filled] = r;
        self.filled += 1;
        let n = self.left.len();
        if self.filled == n {
            self.analyse(None);
            let hop = n / 2;
            self.left.copy_within(hop.., 0);
            self.right.copy_within(hop.., 0);
            self.filled = n - hop;
        }
    }

    /// One window; `short`: a window of only the first frames, the rest silent.
    fn analyse(&mut self, short: Option<usize>) {
        let n = self.left.len();
        let own;
        let window = match short {
            None => &self.window,
            Some(len) => {
                own = hann(len);
                &own
            }
        };
        let energy: f64 = window.iter().map(|w| w * w).sum();
        if energy <= 0.0 {
            return;
        }
        for (i, z) in self.work.iter_mut().enumerate() {
            let w = window.get(i).copied().unwrap_or(0.0);
            *z = Complex {
                re: self.left[i] * w,
                im: self.right[i] * w,
            };
        }
        self.fft.forward(&mut self.work);
        // With z = left + i·right, |Z[k]|² + |Z[n−k]|² is the power of both channels at
        // +f and −f; over every bin that adds up to n·Σ(left² + right²)·w² (Parseval).
        let scale = 1.0 / (2.0 * n as f64 * energy);
        self.now.fill(0.0);
        let mut total = 0.0;
        for k in 0..=n / 2 {
            let mut power = self.work[k].norm_sqr();
            if k != 0 && k != n / 2 {
                power += self.work[n - k].norm_sqr();
            }
            let power = power * scale;
            total += power;
            if let Some(band) = self.band_of[k] {
                self.now[band] += power;
            }
        }
        for (sum, now) in self.sums.iter_mut().zip(&self.now) {
            *sum += now;
        }
        self.total += total;
        self.windows += 1;
        if let Some(kept) = &mut self.kept {
            kept.extend_from_slice(&self.now);
        }
    }

    fn finish(mut self) -> (Vec<AiBandLevel>, Vec<f64>) {
        if self.windows == 0 && self.filled >= 2 {
            let len = self.filled;
            self.left[len..].fill(0.0);
            self.right[len..].fill(0.0);
            self.analyse(Some(len));
        }
        let windows = self.windows.max(1) as f64;
        let levels = (self.sums.iter())
            .map(|&sum| AiBandLevel {
                power: sum / windows,
                share: if self.total > 0.0 {
                    sum / self.total
                } else {
                    0.0
                },
            })
            .collect();
        (levels, self.kept.unwrap_or_default())
    }
}

/// What one rendering was heard as.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Heard {
    pub lufs: Option<f32>,
    pub lufs_short_max: Option<f32>,
    /// Empty without bands.
    pub bands: Vec<AiBandLevel>,
    /// The band powers of each window, when kept.
    pub windows: Vec<f64>,
    /// Empty without cuts.
    pub sections: Vec<AiLoudness>,
}

/// Hears one rendering of the range, from its first frame on.
pub(crate) struct Listener {
    /// Of the left and the right channel, each the shelf and the high-pass.
    filters: [[Biquad; 2]; 2],
    whole: Steps,
    sections: Option<Sections>,
    spectrum: Option<Spectrum>,
    frame: u64,
}

fn finite(sample: f32) -> f64 {
    let sample = f64::from(sample);
    if sample.is_finite() { sample } else { 0.0 }
}

impl Listener {
    /// `bands`: the bands to measure, and whether to keep their power in every window for
    /// comparing. `cuts`: the frames where sections after the first begin, rising.
    pub fn new(rate: u32, bands: Option<(&[AiBand], bool)>, cuts: &[u64]) -> Listener {
        let [shelf, high_pass] = k_weighting(f64::from(rate));
        let channel = [Biquad::new(shelf), Biquad::new(high_pass)];
        // 100 ms to the nearest frame.
        let step_frames = ((rate as usize + 5) / 10).max(1);
        Listener {
            filters: [channel, channel],
            whole: Steps::new(step_frames),
            sections: (!cuts.is_empty()).then(|| Sections {
                cuts: cuts.to_vec(),
                done: Vec::new(),
                now: Part::new(step_frames),
            }),
            spectrum: bands.map(|(bands, keep)| Spectrum::new(rate, bands, keep)),
            frame: 0,
        }
    }

    /// Interleaved stereo, in the order of the rendering.
    pub fn feed(&mut self, samples: &[f32]) {
        for pair in samples.as_chunks::<2>().0 {
            let (l, r) = (finite(pair[0]), finite(pair[1]));
            if let Some(sections) = &mut self.sections {
                sections.reach(self.frame);
            }
            let [[l_shelf, l_high], [r_shelf, r_high]] = &mut self.filters;
            let yl = l_high.run(l_shelf.run(l));
            let yr = r_high.run(r_shelf.run(r));
            let k = yl * yl + yr * yr;
            if self.whole.add(k) {
                self.filters.iter_mut().flatten().for_each(Biquad::flush);
            }
            if let Some(sections) = &mut self.sections {
                sections.now.add(l, r, k);
            }
            if let Some(spectrum) = &mut self.spectrum {
                spectrum.push(l, r);
            }
            self.frame += 1;
        }
    }

    pub fn finish(mut self) -> Heard {
        if let Some(sections) = &mut self.sections {
            sections.reach(self.frame);
        }
        let (bands, windows) = self.spectrum.map(Spectrum::finish).unwrap_or_default();
        Heard {
            lufs: integrated(&self.whole.sums, self.whole.frames),
            lufs_short_max: short_term_max(&self.whole.sums, self.whole.frames),
            bands,
            windows,
            sections: self.sections.map(Sections::finish).unwrap_or_default(),
        }
    }
}

/// How much two renderings of one range sound in each band at the same time, from the band
/// powers of their windows (`bands` to a window). A band under −90 dBFS in a window is
/// silent there.
pub(crate) fn shared(a: &[f64], b: &[f64], bands: usize) -> Vec<AiShared> {
    let heard = |power: f64| if power < SILENT_BAND { 0.0 } else { power };
    let windows = a.len().min(b.len()) / bands.max(1);
    (0..bands)
        .map(|band| {
            let (mut both, mut of_a, mut of_b) = (0.0f64, 0.0, 0.0);
            for w in 0..windows {
                let (x, y) = (heard(a[w * bands + band]), heard(b[w * bands + band]));
                both += x.min(y);
                of_a += x;
                of_b += y;
            }
            let louder = of_a.max(of_b);
            AiShared {
                power: if windows > 0 {
                    both / windows as f64
                } else {
                    0.0
                },
                score: if louder > 0.0 { both / louder } else { 0.0 },
            }
        })
        .collect()
}

/// What the renderings were heard as, for the AI link. `heard`: the mix, then each track
/// alone; `compare`: places of tracks among those alone; `cuts`: ticks where sections begin.
pub(crate) fn gather(
    bands: Vec<AiBand>,
    rate: u32,
    heard: &[Heard],
    compare: &[usize],
    span: TickRange,
    cuts: &[u64],
) -> AiAnalysis {
    let mut overlaps = Vec::new();
    for (i, &a) in compare.iter().enumerate() {
        for &b in &compare[i + 1..] {
            let (Some(x), Some(y)) = (heard.get(a + 1), heard.get(b + 1)) else {
                continue;
            };
            overlaps.push(AiOverlap {
                a,
                b,
                bands: shared(&x.windows, &y.windows, bands.len()),
            });
        }
    }
    let sections = match cuts.is_empty() {
        true => Vec::new(),
        false => {
            let edges: Vec<u64> = (std::iter::once(span.start()))
                .chain(cuts.iter().copied())
                .chain(std::iter::once(span.end()))
                .collect();
            (edges.windows(2).enumerate())
                .map(|(s, edge)| AiSection {
                    start: edge[0],
                    end: edge[1],
                    passes: (heard.iter())
                        .map(|h| h.sections.get(s).copied().unwrap_or_default())
                        .collect(),
                })
                .collect()
        }
    };
    AiAnalysis {
        window_s: window_frames(rate) as f64 / f64::from(rate.max(1)),
        levels: heard.iter().map(|h| h.bands.clone()).collect(),
        bands,
        overlaps,
        sections,
    }
}
