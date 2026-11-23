//! The maths of the analysis against signals whose answer is known.

use std::f64::consts::TAU;

use super::*;

const RATE: u32 = 48_000;

fn db(power: f64) -> f64 {
    10.0 * power.log10()
}

fn amplitude(dbfs: f64) -> f64 {
    10f64.powf(dbfs / 20.0)
}

/// Interleaved stereo: a sine of `peak` dBFS on both channels.
fn sine(rate: u32, hz: f64, peak: f64, seconds: f64) -> Vec<f32> {
    let frames = (seconds * f64::from(rate)).round() as usize;
    let a = amplitude(peak);
    (0..frames)
        .flat_map(|i| {
            let s = (a * (TAU * hz * i as f64 / f64::from(rate)).sin()) as f32;
            [s, s]
        })
        .collect()
}

fn hear(rate: u32, samples: &[f32], bands: Option<(&[AiBand], bool)>, cuts: &[u64]) -> Heard {
    let mut listener = Listener::new(rate, bands, cuts);
    // In blocks of an odd size, as the export's blocks need not line up with anything.
    for block in samples.chunks(2 * 301) {
        listener.feed(block);
    }
    listener.finish()
}

fn close(got: f64, want: f64, within: f64, what: &str) {
    assert!(
        (got - want).abs() <= within,
        "{what}: {got} is not {want} ± {within}"
    );
}

/// A small xorshift, so that the noise is the same every time.
struct Noise(u64);

impl Noise {
    /// Even in −1..1.
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

#[test]
fn the_fft_is_the_dft() {
    let n = 64;
    let mut noise = Noise(7);
    let input: Vec<Complex> = (0..n)
        .map(|_| Complex {
            re: noise.next(),
            im: noise.next(),
        })
        .collect();
    let mut fast = input.clone();
    Fft::new(n).forward(&mut fast);
    for (k, got) in fast.iter().enumerate() {
        let (mut re, mut im) = (0.0, 0.0);
        for (j, x) in input.iter().enumerate() {
            let angle = -TAU * (j * k) as f64 / n as f64;
            re += x.re * angle.cos() - x.im * angle.sin();
            im += x.re * angle.sin() + x.im * angle.cos();
        }
        close(got.re, re, 1e-9, "re");
        close(got.im, im, 1e-9, "im");
    }
}

#[test]
fn k_weighting_at_48_khz_is_the_table_of_the_standard() {
    let [shelf, high_pass] = k_weighting(48_000.0);
    let table_1 = [
        1.53512485958697,
        -2.69169618940638,
        1.19839281085285,
        -1.69065929318241,
        0.73248077421585,
    ];
    let table_2 = [1.0, -2.0, 1.0, -1.99004745483398, 0.99007225036621];
    for (got, want) in shelf.iter().zip(table_1) {
        close(*got, want, 1e-9, "stage 1");
    }
    for (got, want) in high_pass.iter().zip(table_2) {
        close(*got, want, 1e-9, "stage 2");
    }
}

#[test]
fn a_sine_of_minus_23_dbfs_on_both_channels_reads_minus_23_lufs_at_any_rate() {
    for rate in [44_100, 48_000, 96_000] {
        for hz in [997.0, 1000.0] {
            let heard = hear(rate, &sine(rate, hz, -23.0, 20.0), None, &[]);
            let what = format!("{hz} Hz at {rate}");
            close(f64::from(heard.lufs.unwrap()), -23.0, 0.05, &what);
            close(f64::from(heard.lufs_short_max.unwrap()), -23.0, 0.05, &what);
        }
    }
    // BS.1770-4: 0 dBFS at 997 Hz on one channel is −3.01.
    let one: Vec<f32> = (sine(RATE, 997.0, 0.0, 10.0).chunks(2))
        .flat_map(|f| [f[0], 0.0])
        .collect();
    close(
        f64::from(hear(RATE, &one, None, &[]).lufs.unwrap()),
        -3.01,
        0.05,
        "one channel",
    );
}

/// Tones of 1 kHz one after another: (dBFS, seconds).
fn tones(parts: &[(f64, f64)]) -> Vec<f32> {
    (parts.iter())
        .flat_map(|&(peak, seconds)| sine(RATE, 1000.0, peak, seconds))
        .collect()
}

#[test]
fn the_gates_leave_out_what_ebu_tech_3341_says() {
    // Case 3: the quiet parts are under the relative gate.
    let three = tones(&[(-36.0, 10.0), (-23.0, 60.0), (-36.0, 10.0)]);
    close(
        f64::from(hear(RATE, &three, None, &[]).lufs.unwrap()),
        -23.0,
        0.1,
        "case 3",
    );
    // Case 4: and the very quiet ones under the absolute gate.
    let four = tones(&[
        (-72.0, 10.0),
        (-36.0, 10.0),
        (-23.0, 60.0),
        (-36.0, 10.0),
        (-72.0, 10.0),
    ]);
    close(
        f64::from(hear(RATE, &four, None, &[]).lufs.unwrap()),
        -23.0,
        0.1,
        "case 4",
    );
    // Case 5: loud parts are not gated, they are averaged as power.
    let five = tones(&[(-26.0, 20.0), (-20.0, 20.1), (-26.0, 20.0)]);
    close(
        f64::from(hear(RATE, &five, None, &[]).lufs.unwrap()),
        -23.0,
        0.1,
        "case 5",
    );
    let silent = vec![0.0f32; 2 * RATE as usize];
    assert_eq!(hear(RATE, &silent, None, &[]).lufs, None);
    // Shorter than one gating block, or than the short-term window.
    let short = hear(RATE, &sine(RATE, 1000.0, -10.0, 0.3), None, &[]);
    assert_eq!((short.lufs, short.lufs_short_max), (None, None));
    let two_s = hear(RATE, &sine(RATE, 1000.0, -10.0, 2.0), None, &[]);
    assert!(two_s.lufs.is_some() && two_s.lufs_short_max.is_none());
}

#[test]
fn the_bands_have_the_edges_they_say() {
    let five = bands(AiBandLayout::Five, RATE);
    let edges: Vec<(f64, f64)> = five.iter().map(|b| (b.low_hz, b.high_hz)).collect();
    assert_eq!(
        edges,
        [
            (0.0, 120.0),
            (120.0, 500.0),
            (500.0, 2000.0),
            (2000.0, 6000.0),
            (6000.0, 24000.0)
        ]
    );
    let octaves = bands(AiBandLayout::Octave, RATE);
    assert_eq!(octaves.len(), 10);
    close(octaves[0].low_hz, 22.387, 0.001, "31.5 Hz low");
    close(octaves[0].high_hz, 44.668, 0.001, "31.5 Hz high");
    close(octaves[5].low_hz * octaves[5].high_hz, 1e6, 1e-6, "1 kHz");
    for pair in octaves.windows(2) {
        close(pair[0].high_hz, pair[1].low_hz, 1e-9, "octaves touch");
    }
    // Above half the rate a band is empty.
    let low_rate = bands(AiBandLayout::Octave, 22_050);
    assert_eq!(low_rate[9].low_hz, low_rate[9].high_hz);
}

#[test]
fn a_sine_lands_in_its_band_at_its_level() {
    for rate in [44_100, 48_000] {
        let five = bands(AiBandLayout::Five, rate);
        for (band, hz) in [60.0, 250.0, 1000.0, 4000.0, 10_000.0]
            .into_iter()
            .enumerate()
        {
            let heard = hear(rate, &sine(rate, hz, -12.0, 3.0), Some((&five, false)), &[]);
            let what = format!("{hz} Hz at {rate}");
            let level = heard.bands[band];
            // A sine of peak a has the power a²/2.
            close(db(level.power), -12.0 - 3.0103, 0.05, &what);
            assert!(level.share > 0.999, "{what}: {heard:?}");
        }
        let octaves = bands(AiBandLayout::Octave, rate);
        for (band, b) in octaves.iter().enumerate() {
            let hz = (b.low_hz * b.high_hz).sqrt();
            let heard = hear(
                rate,
                &sine(rate, hz, -20.0, 3.0),
                Some((&octaves, false)),
                &[],
            );
            let what = format!("{hz} Hz at {rate}");
            close(db(heard.bands[band].power), -23.0103, 0.1, &what);
            assert!(heard.bands[band].share > 0.98, "{what}: {heard:?}");
        }
    }
}

#[test]
fn a_range_shorter_than_a_window_is_still_measured() {
    let five = bands(AiBandLayout::Five, RATE);
    let heard = hear(
        RATE,
        &sine(RATE, 1000.0, -6.0, 0.1),
        Some((&five, true)),
        &[],
    );
    close(db(heard.bands[2].power), -9.0103, 0.2, "0.1 s");
    assert!(heard.bands[2].share > 0.99, "{heard:?}");
    assert_eq!(heard.windows.len(), five.len());
}

#[test]
fn white_noise_fills_the_bands_as_wide_as_they_are() {
    let mut noise = Noise(1);
    let samples: Vec<f32> = (0..10 * 2 * RATE as usize)
        .map(|_| (noise.next() * 0.5) as f32)
        .collect();
    let mean_square =
        samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64;
    let five = bands(AiBandLayout::Five, RATE);
    let heard = hear(RATE, &samples, Some((&five, false)), &[]);
    let all: f64 = heard.bands.iter().map(|b| b.power).sum();
    close(db(all), db(mean_square), 0.05, "all bands");
    close(
        heard.bands.iter().map(|b| b.share).sum(),
        1.0,
        1e-9,
        "shares",
    );
    for (b, level) in five.iter().zip(&heard.bands) {
        let want = (b.high_hz - b.low_hz) / 24_000.0;
        close(level.share / want, 1.0, 0.1, b.name);
    }
}

/// Pink noise made in the frequency domain: every bin as strong as 1/√f, its phase from
/// the noise.
fn pink(frames: usize, seed: u64) -> Vec<f64> {
    let mut noise = Noise(seed);
    let mut spectrum = vec![Complex::default(); frames];
    for k in 1..frames / 2 {
        let (angle, size) = (noise.next() * std::f64::consts::PI, 1.0 / (k as f64).sqrt());
        spectrum[k] = Complex {
            re: size * angle.cos(),
            im: size * angle.sin(),
        };
        // Conjugate, so that the signal is real. The forward FFT of it is the signal backwards,
        // which is as pink.
        spectrum[frames - k] = Complex {
            re: spectrum[k].re,
            im: -spectrum[k].im,
        };
    }
    Fft::new(frames).forward(&mut spectrum);
    let peak = spectrum.iter().fold(0.0f64, |m, z| m.max(z.re.abs()));
    spectrum.iter().map(|z| z.re / peak * 0.5).collect()
}

#[test]
fn pink_noise_is_as_strong_in_every_octave() {
    let (left, right) = (pink(1 << 20, 3), pink(1 << 20, 4));
    let samples: Vec<f32> = (left.iter().zip(&right))
        .flat_map(|(l, r)| [*l as f32, *r as f32])
        .collect();
    let octaves = bands(AiBandLayout::Octave, RATE);
    let heard = hear(RATE, &samples, Some((&octaves, false)), &[]);
    let middle = db(heard.bands[5].power);
    for (b, level) in octaves.iter().zip(&heard.bands) {
        close(db(level.power), middle, 0.5, b.name);
    }
}

#[test]
fn two_tracks_share_a_band_only_when_they_sound_in_it_together() {
    let five = bands(AiBandLayout::Five, RATE);
    let windows = |samples: &[f32]| hear(RATE, samples, Some((&five, true)), &[]).windows;
    let a = windows(&sine(RATE, 1000.0, -12.0, 4.0));
    let same = shared(&a, &a, five.len());
    close(same[2].score, 1.0, 1e-9, "the same");
    close(db(same[2].power), -15.0103, 0.1, "the same, power");

    let quiet = windows(&sine(RATE, 1000.0, -32.0, 4.0));
    close(
        shared(&a, &quiet, five.len())[2].score,
        0.01,
        0.002,
        "20 dB under",
    );

    let half = |first: bool| -> Vec<f32> {
        let mut s = sine(RATE, 1000.0, -12.0, 4.0);
        let middle = s.len() / 2;
        let silent = if first { middle..s.len() } else { 0..middle };
        s[silent].fill(0.0);
        s
    };
    let apart = shared(&windows(&half(true)), &windows(&half(false)), five.len());
    assert!(apart[2].score < 0.05, "{apart:?}");

    let low = windows(&sine(RATE, 100.0, -12.0, 4.0));
    let far = shared(&a, &low, five.len());
    assert!(far.iter().all(|s| s.score < 0.01), "{far:?}");
}

#[test]
fn sections_have_their_own_levels() {
    let mut samples = sine(RATE, 997.0, -6.0, 1.0);
    samples.extend(sine(RATE, 997.0, -20.0, 4.0));
    let cuts = [u64::from(RATE), 100 * u64::from(RATE)];
    let heard = hear(RATE, &samples, None, &cuts);
    let [first, second, past] = heard.sections[..] else {
        panic!("{heard:?}");
    };
    assert_eq!(first.frames, u64::from(RATE));
    assert_eq!(second.frames, 4 * u64::from(RATE));
    assert_eq!(past, AiLoudness::default());
    close(f64::from(first.peak), amplitude(-6.0), 1e-3, "first peak");
    close(
        f64::from(first.rms),
        amplitude(-6.0) / 2f64.sqrt(),
        1e-3,
        "first rms",
    );
    close(
        f64::from(second.rms),
        amplitude(-20.0) / 2f64.sqrt(),
        1e-4,
        "second rms",
    );
    close(f64::from(second.lufs.unwrap()), -20.0, 0.1, "second lufs");
    close(f64::from(first.lufs.unwrap()), -6.0, 0.1, "first lufs");
    assert_eq!(first.lufs_short_max, None);
    close(
        f64::from(second.lufs_short_max.unwrap()),
        -20.0,
        0.1,
        "short",
    );

    let span = TickRange::new(0, 9600).unwrap();
    let analysis = gather(Vec::new(), RATE, &[heard], &[], span, &[1920, 7680]);
    let edges: Vec<(u64, u64)> = analysis.sections.iter().map(|s| (s.start, s.end)).collect();
    assert_eq!(edges, [(0, 1920), (1920, 7680), (7680, 9600)]);
}
