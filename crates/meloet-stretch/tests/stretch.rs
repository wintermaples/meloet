//! What the stretcher and the follower must keep: pitch, length, where things land, clean numbers.
//! The measured values are printed (`-- --nocapture`); the limits in the asserts are wider.

mod common;

use common::*;
use meloet_stretch::{Preset, RENDER_STEP, RenderError, render};
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};

const PRESETS: [Preset; 2] = [Preset::Default, Preset::Cheaper];
/// 1 ms at 48 kHz.
const ONE_MS: f64 = 48.0;

fn middle(x: &[f32]) -> &[f32] {
    &x[x.len() / 2 - 24_000..x.len() / 2 + 24_000]
}

#[test]
fn a_stretched_sine_keeps_its_pitch_and_level() {
    let frames = 3 * 48_000usize;
    for preset in PRESETS {
        let mut st = stretcher(1, preset);
        for ratio in [1.25, 0.8, 1.5, 0.75] {
            let out_frames = (frames as f64 * ratio).round() as u64;
            let out = render_all(&mut st, &[sine(440.0, frames, 0.5)], out_frames, |j| {
                j as f64 / ratio
            });
            assert_eq!(out[0].len() as u64, out_frames);
            let freq = peak_freq(middle(&out[0]), 300.0, 600.0);
            let level = rms(middle(&out[0])) / (0.5 / 2f64.sqrt());
            println!("{preset:?} x{ratio}: {freq:.2} Hz, level {level:.4}");
            assert!((freq - 440.0).abs() < 0.5, "{preset:?} x{ratio}: {freq}");
            assert!((level - 1.0).abs() < 0.05, "{preset:?} x{ratio}: {level}");
        }
    }
}

#[test]
fn transpose_moves_the_pitch_by_semitones_and_cents() {
    // The library moves whole spectral peaks by a step it finds from smoothed energy, so a pure
    // tone lands within a few Hz of the exact pitch, not on it.
    let frames = 3 * 48_000usize;
    for (preset, limit) in [(Preset::Default, 2.0), (Preset::Cheaper, 5.0)] {
        let mut worst = (0.0f64, 0.0f64);
        let mut st = stretcher(1, preset);
        for base in [220.0, 440.0, 1000.0] {
            for semitones in [12.0, -12.0, 3.0, -3.0, 0.5, -0.5] {
                st.set_transpose_semitones(semitones as f32, None);
                let want = base * 2f64.powf(semitones / 12.0);
                let out = render_all(&mut st, &[sine(base, frames, 0.5)], frames as u64, |j| {
                    j as f64
                });
                let freq = peak_freq(middle(&out[0]), want * 0.8, want * 1.25);
                println!(
                    "{preset:?} {base} Hz {semitones:+}: {freq:.2} Hz, want {want:.2} ({:+.1} cents)",
                    1200.0 * (freq / want).log2()
                );
                worst = (
                    worst.0.max((freq - want).abs()),
                    worst.1.max((1200.0 * (freq / want).log2()).abs()),
                );
                assert!(
                    (freq - want).abs() < limit,
                    "{preset:?} {base} {semitones}: {freq} vs {want}"
                );
            }
        }
        println!("{preset:?} worst: {:.2} Hz, {:.1} cents", worst.0, worst.1);
    }
}

#[test]
fn a_click_lands_where_p_says() {
    let frames = 3 * 48_000usize;
    for preset in PRESETS {
        let mut worst = (0.0f64, String::new());
        let mut st = stretcher(1, preset);
        for (ratio, semitones) in [
            (1.0, 0.0),
            (1.25, 0.0),
            (0.8, 0.0),
            (1.5, 0.0),
            (0.75, 0.0),
            (1.0, 12.0),
            (1.0, -3.0),
            (1.25, 3.0),
            (0.8, -12.0),
        ] {
            st.set_transpose_semitones(semitones as f32, None);
            st.set_formant_factor(1.0, semitones != 0.0);
            let out_frames = (frames as f64 * ratio).round() as u64;
            for at in [48_000usize, 60_000, 100_001] {
                // Alone in silence, and on top of a quiet tone (silence takes another path in the library).
                for bed in [0.0, 0.05] {
                    let mut file = sine(220.0, frames, bed);
                    add_click(&mut file, at);
                    let want = click_at(&file[at - 100..at + 400]) + (at - 100) as f64;
                    let out = render_all(&mut st, &[file], out_frames, |j| j as f64 / ratio);
                    let off = click_at(&out[0]) - want * ratio;
                    if off.abs() > worst.0 {
                        worst = (
                            off.abs(),
                            format!("x{ratio} {semitones:+}, click at {at}, bed {bed}"),
                        );
                    }
                    assert!(
                        off.abs() < ONE_MS,
                        "{preset:?} x{ratio} {semitones:+} click at {at} bed {bed}: off by {off:.1} frames"
                    );
                }
            }
        }
        println!(
            "{preset:?} worst click offset: {:.1} frames ({:.2} ms) at {}",
            worst.0,
            worst.0 / 48.0,
            worst.1
        );
    }
}

#[test]
fn a_ramp_is_followed_whatever_the_step() {
    // The rate (file frames per output frame) goes from 0.8 to 1.25 over four seconds.
    let out_frames = 4 * 48_000u64;
    let n = out_frames as f64;
    let p = move |x: f64| 0.8 * x + 0.45 * x * x / (2.0 * n);
    let frames = p(n).ceil() as usize;
    let clicks = [20_000usize, 80_000, 150_000, 190_000];
    let mut file = sine(220.0, frames, 0.05);
    for at in clicks {
        add_click(&mut file, at);
    }
    let want: Vec<f64> = clicks
        .iter()
        .map(|&at| {
            inverse(
                p,
                click_at(&file[at - 100..at + 400]) + (at - 100) as f64,
                n,
            )
        })
        .collect();
    let found = |out: &[f32]| -> Vec<f64> {
        want.iter()
            .map(|&w| {
                click_at(&out[w as usize - 2000..w as usize + 2000]) + (w as usize - 2000) as f64
            })
            .collect()
    };

    let mut st = stretcher(1, Preset::Default);
    let file = [file];
    let reference = render_all(&mut st, &file, out_frames, |j| p(j as f64));
    assert_eq!(reference[0].len() as u64, out_frames);
    // `render` is the follower at RENDER_STEP, bit for bit, and the same again the second time.
    assert_eq!(
        render_with_step(&mut st, &file, out_frames, |j| p(j as f64), RENDER_STEP),
        reference
    );

    for step in [64usize, 1000, 4096] {
        let out = render_with_step(&mut st, &file, out_frames, |j| p(j as f64), step);
        assert_eq!(out[0].len() as u64, out_frames);
        let at = found(&out[0]);
        let diff = out[0]
            .iter()
            .zip(&reference[0])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        println!(
            "step {step}: clicks off by {:.1?} frames; largest sample difference to step {RENDER_STEP}: {diff:.4}",
            at.iter().zip(&want).map(|(a, w)| a - w).collect::<Vec<_>>()
        );
        for (a, w) in at.iter().zip(&want) {
            assert!(
                (a - w).abs() < ONE_MS,
                "step {step}: click at {a:.1}, want {w:.1}"
            );
        }
    }
}

#[test]
fn rate_one_without_transpose_gives_the_file_back() {
    // meloet never renders such a clip (it plays the file), but it shows that nothing is shifted.
    let frames = 2 * 48_000usize;
    let mut file = sine(330.0, frames, 0.4);
    add_click(&mut file, 30_000);
    let mut st = stretcher(1, Preset::Default);
    let out = render_all(&mut st, std::slice::from_ref(&file), frames as u64, |j| {
        j as f64
    });
    let error: Vec<f32> = out[0].iter().zip(&file).map(|(a, b)| a - b).collect();
    println!(
        "rate 1: error {:.1} dB below the signal",
        20.0 * (rms(&file) / rms(&error)).log10()
    );
    assert!(rms(&error) < 0.01 * rms(&file));
}

fn noise(frames: usize, seed: u32) -> Vec<f32> {
    let mut s = seed;
    (0..frames)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 8) as f32 / (1 << 23) as f32 - 1.0
        })
        .collect()
}

#[test]
fn the_numbers_stay_finite_and_silence_stays_silent() {
    let frames = 48_000usize;
    for preset in PRESETS {
        for channels in [1usize, 2] {
            let mut st = stretcher(channels, preset);
            let loud: Vec<Vec<f32>> = (0..channels).map(|c| noise(frames, 7 + c as u32)).collect();
            let quiet = vec![vec![0.0f32; frames]; channels];
            for (ratio, semitones, formant) in [
                (1.0, 0.0, false),
                (4.0, 24.0, true),
                (0.25, -24.0, true),
                (1.3, 7.0, false),
                (0.1, 0.0, false),
            ] {
                st.set_transpose_semitones(semitones, None);
                st.set_formant_factor(1.0, formant);
                let out_frames = (frames as f64 * ratio) as u64;
                let out = render_all(&mut st, &loud, out_frames, |j| j as f64 / ratio);
                assert_eq!(out.len(), channels);
                for c in &out {
                    assert_eq!(c.len() as u64, out_frames);
                    assert!(
                        c.iter().all(|v| v.is_finite()),
                        "{preset:?} x{ratio} {semitones}"
                    );
                    assert!(
                        peak(c) < 8.0,
                        "{preset:?} x{ratio} {semitones}: peak {}",
                        peak(c)
                    );
                }
                let out = render_all(&mut st, &quiet, out_frames, |j| j as f64 / ratio);
                assert!(
                    out.iter().all(|c| peak(c) < 1e-6),
                    "{preset:?} x{ratio} {semitones}: silence became {}",
                    peak(&out[0])
                );
            }
        }
    }
}

#[test]
fn stereo_channels_stay_apart() {
    let frames = 2 * 48_000usize;
    let mut st = stretcher(2, Preset::Default);
    let out = render_all(
        &mut st,
        &[sine(440.0, frames, 0.5), sine(660.0, frames, 0.5)],
        (frames as f64 * 1.25) as u64,
        |j| j as f64 / 1.25,
    );
    let (left, right) = (middle(&out[0]), middle(&out[1]));
    assert!((peak_freq(left, 300.0, 900.0) - 440.0).abs() < 0.5);
    assert!((peak_freq(right, 300.0, 900.0) - 660.0).abs() < 0.5);
    assert!(band_energy(left, 640.0, 680.0) < 1e-4 * band_energy(left, 420.0, 460.0));
    assert!(band_energy(right, 420.0, 460.0) < 1e-4 * band_energy(right, 640.0, 680.0));
}

/// A voice-like tone: harmonics of 150 Hz under one resonance at 1 kHz.
fn vowel(frames: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames];
    for h in 1..=40 {
        let f = 150.0 * h as f64;
        let gain = 0.1 / (1.0 + ((f - 1000.0) / 150.0).powi(2));
        for (o, s) in out.iter_mut().zip(sine(f, frames, gain as f32)) {
            *o += s;
        }
    }
    out
}

#[test]
fn formant_compensation_keeps_the_resonance_in_place() {
    let frames = 3 * 48_000usize;
    let file = [vowel(frames)];
    let factor = 2f64.powf(7.0 / 12.0);
    let mut st = stretcher(1, Preset::Default);
    st.set_transpose_semitones(7.0, None);
    // How much of the energy sits around the old resonance (1 kHz) against the moved one (1.5 kHz).
    let stays = |out: &[f32]| {
        band_energy(out, 800.0, 1200.0)
            / band_energy(out, 1000.0 * factor - 200.0, 1000.0 * factor + 200.0)
    };

    st.set_formant_factor(1.0, false);
    let moved = render_all(&mut st, &file, frames as u64, |j| j as f64);
    st.set_formant_factor(1.0, true);
    let kept_auto = render_all(&mut st, &file, frames as u64, |j| j as f64);
    st.set_formant_base(150.0 / SR as f32);
    let kept_base = render_all(&mut st, &file, frames as u64, |j| j as f64);

    let (moved, kept_auto, kept_base) = (
        stays(middle(&moved[0])),
        stays(middle(&kept_auto[0])),
        stays(middle(&kept_base[0])),
    );
    println!(
        "energy at 1 kHz against 1.5 kHz after +7: off {moved:.3}, on (base found) {kept_auto:.3}, on (base given) {kept_base:.3}"
    );
    assert!(
        moved < 0.5,
        "without compensation the resonance moves up: {moved}"
    );
    assert!(
        kept_auto > 2.0 * moved && kept_base > 2.0 * moved,
        "with compensation it stays: {kept_auto}, {kept_base}"
    );
}

#[test]
fn progress_goes_up_to_100_and_cancel_stops() {
    let frames = 48_000usize;
    let file = [sine(440.0, frames, 0.5)];
    let mut st = stretcher(1, Preset::Default);
    let mut seen = Vec::new();
    let mut made = 0usize;
    let done: Result<(), RenderError<Infallible>> = render(
        &mut st,
        frames as u64,
        |j| j as f64,
        read_from(&file),
        |b| {
            made += b[0].len();
            assert!(b[0].len() <= RENDER_STEP);
            Ok(())
        },
        |p| seen.push(p),
        &AtomicBool::new(false),
    );
    assert_eq!(done, Ok(()));
    assert_eq!(made, frames);
    assert_eq!((seen[0], *seen.last().unwrap()), (0, 100));
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");

    let cancel = AtomicBool::new(false);
    let mut written = 0usize;
    let done: Result<(), RenderError<Infallible>> = render(
        &mut st,
        frames as u64,
        |j| j as f64,
        read_from(&file),
        |b| {
            written += b[0].len();
            Ok(())
        },
        |p| {
            if p >= 50 {
                cancel.store(true, Ordering::Relaxed);
            }
        },
        &cancel,
    );
    assert_eq!(done, Err(RenderError::Cancelled));
    assert!(written < frames);

    let failed: Result<(), RenderError<&str>> = render(
        &mut st,
        frames as u64,
        |j| j as f64,
        |_, _| Err("no file"),
        |_| Ok(()),
        |_| {},
        &AtomicBool::new(false),
    );
    assert_eq!(failed, Err(RenderError::Io("no file")));
}

#[test]
fn bad_sizes_are_refused() {
    use meloet_stretch::{Error, Stretcher};
    assert_eq!(
        Stretcher::new(0, 48_000.0, Preset::Default).err(),
        Some(Error::BadConfig)
    );
    assert_eq!(
        Stretcher::new(2, 100.0, Preset::Default).err(),
        Some(Error::BadConfig)
    );
    assert_eq!(
        Stretcher::new(
            2,
            48_000.0,
            Preset::Blocks {
                block: 100,
                interval: 80
            }
        )
        .err(),
        Some(Error::BadConfig)
    );
    let st = Stretcher::new(
        2,
        48_000.0,
        Preset::Blocks {
            block: 3840,
            interval: 960,
        },
    )
    .unwrap();
    assert_eq!(
        (st.block_samples(), st.interval_samples(), st.channels()),
        (3840, 960, 2)
    );
}

/// `cargo test -p meloet-stretch --release -- --ignored --nocapture speed`
#[test]
#[ignore = "a measurement: run it alone, in release"]
fn speed_of_sixty_seconds_of_stereo() {
    let seconds = 60usize;
    let frames = seconds * 48_000;
    let file: Vec<Vec<f32>> = (0..2)
        .map(|c| {
            let mut ch = noise(frames, 3 + c);
            for (o, s) in ch
                .iter_mut()
                .zip(sine(220.0 + 110.0 * c as f64, frames, 0.3))
            {
                *o = *o * 0.1 + s;
            }
            ch
        })
        .collect();
    for preset in PRESETS {
        let mut st = stretcher(2, preset);
        for (name, ratio, semitones, formant) in [
            ("stretch x1.25", 1.25, 0.0, false),
            ("stretch x0.8", 0.8, 0.0, false),
            ("x1.25, +3, formant kept", 1.25, 3.0, true),
        ] {
            st.set_transpose_semitones(semitones, None);
            st.set_formant_factor(1.0, formant);
            let out_frames = (frames as f64 * ratio) as u64;
            let start = std::time::Instant::now();
            let out = render_all(&mut st, &file, out_frames, |j| j as f64 / ratio);
            let took = start.elapsed().as_secs_f64();
            let made = out[0].len() as f64 / 48_000.0;
            println!(
                "{preset:?}, {name}: {made:.0} s of sound in {took:.2} s = {:.0} times real time ({:.0} times the {seconds} s read)",
                made / took,
                seconds as f64 / took
            );
        }
    }
}
