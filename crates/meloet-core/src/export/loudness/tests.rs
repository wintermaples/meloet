//! F1: the true peak and the gain of 「公開先に合わせる」.

use super::*;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn the_gain_brings_the_loudness_to_minus_14_unless_the_true_peak_would_pass_minus_1() {
    // Quiet, with room: louder by what is missing.
    let m = match_gain(Some(-18.6), Some(-6.0)).unwrap();
    assert!(
        near(m.gain_db, 4.6) && !m.limited && near(m.lufs, -14.0),
        "{m:?}"
    );
    // Quiet, without that much room: up to −1 dBTP.
    let m = match_gain(Some(-16.0), Some(-2.0)).unwrap();
    assert!(
        near(m.gain_db, 1.0) && m.limited && near(m.lufs, -15.0),
        "{m:?}"
    );
    // Loud: quieter, and the true peak is then far below the limit.
    let m = match_gain(Some(-8.0), Some(0.5)).unwrap();
    assert!(
        near(m.gain_db, -6.0) && !m.limited && near(m.lufs, -14.0),
        "{m:?}"
    );
    // A peak far above the rest: quieter than −14, to keep the peak at −1.
    let m = match_gain(Some(-16.0), Some(2.0)).unwrap();
    assert!(
        near(m.gain_db, -3.0) && m.limited && near(m.lufs, -19.0),
        "{m:?}"
    );
    // Exactly at the limit is not stopped by it.
    let m = match_gain(Some(-10.0), Some(3.0)).unwrap();
    assert!(near(m.gain_db, -4.0) && !m.limited, "{m:?}");
}

#[test]
fn a_loudness_not_measured_or_below_minus_70_cannot_be_matched() {
    assert_eq!(match_gain(None, Some(-20.0)), None);
    assert_eq!(match_gain(None, None), None);
    assert_eq!(match_gain(Some(-70.5), Some(-50.0)), None);
    assert_eq!(match_gain(Some(f32::NAN), Some(-50.0)), None);
    assert!(match_gain(Some(-70.0), Some(-50.0)).is_some());
    // A true peak that is not known does not stop the gain.
    let m = match_gain(Some(-20.0), None).unwrap();
    assert!(near(m.gain_db, 6.0) && !m.limited);
}

#[test]
fn the_factor_is_the_gain_as_a_ratio() {
    assert_eq!(factor(0.0), 1.0);
    assert!((factor(-6.0) - 0.501_187_2).abs() < 1e-6);
    assert!((factor(20.0) - 10.0).abs() < 1e-5);
}

/// A sine at a quarter of the rate, its samples at 45°: every sample is at 0.707 of the
/// height the wave reaches between them.
fn between_samples(amplitude: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|i| {
            let s = amplitude * [1.0, 1.0, -1.0, -1.0][i % 4] * std::f32::consts::FRAC_1_SQRT_2;
            [s, s]
        })
        .collect()
}

#[test]
fn the_true_peak_hears_what_is_between_the_samples() {
    let mut tp = TruePeak::new(48_000).unwrap();
    assert_eq!(tp.db(), None);
    tp.feed(&[0.0; 512]);
    assert_eq!(tp.db(), None, "silence");

    let wave = between_samples(0.5, 48_000);
    let sample_peak = wave.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!((20.0 * sample_peak.log10() + 9.03).abs() < 0.01);
    for block in wave.chunks(2 * 256) {
        tp.feed(block);
    }
    let db = tp.db().unwrap();
    assert!((db + 6.02).abs() < 0.2, "{db} dBTP");

    // The louder channel counts.
    let mut tp = TruePeak::new(44_100).unwrap();
    let left_only: Vec<f32> = (wave.chunks(2)).flat_map(|f| [f[0], 0.0]).collect();
    tp.feed(&left_only);
    assert!((tp.db().unwrap() + 6.02).abs() < 0.2);
}

#[test]
fn a_sample_that_is_not_a_number_does_not_spoil_the_true_peak() {
    let mut tp = TruePeak::new(48_000).unwrap();
    tp.feed(&[0.25, 0.25, f32::NAN, f32::INFINITY, -0.25, -0.25]);
    tp.feed(&between_samples(0.5, 4_800));
    let db = tp.db().unwrap();
    assert!(db.is_finite() && (db + 6.02).abs() < 0.2, "{db}");
}

/// The thread keeps numbers smaller than the smallest normal one (no flush-to-zero).
fn denormals_kept() -> bool {
    let tiny = std::hint::black_box(f32::MIN_POSITIVE);
    let half = std::hint::black_box(0.5f32);
    tiny * half != 0.0 && std::hint::black_box(tiny * half) * std::hint::black_box(1.0) != 0.0
}

#[test]
fn the_true_peak_leaves_the_threads_float_settings_as_they_were() {
    assert!(denormals_kept());
    let mut tp = TruePeak::new(48_000).unwrap();
    tp.feed(&between_samples(0.5, 4_800));
    assert!(denormals_kept(), "flush-to-zero was left on");
}
