//! M3: the meters of 「仕上げのメーター」 on known signals, against the export's measuring, and
//! the rule of what a play measures.

use super::*;
use crate::export::analysis::Listener;
use crate::export::loudness::TruePeak;

const RATE: u32 = 48_000;

/// A sine of `hz` at `db` dBFS (its peak), the same on both sides, `seconds` long.
fn sine(hz: f64, db: f64, seconds: f64) -> Vec<[f32; 2]> {
    let amplitude = 10f64.powf(db / 20.0);
    (0..(seconds * f64::from(RATE)) as usize)
        .map(|n| {
            let v = (amplitude * (std::f64::consts::TAU * hz * n as f64 / f64::from(RATE)).sin())
                as f32;
            [v, v]
        })
        .collect()
}

/// In pieces as the analysis thread takes them (480 frames, 10 ms).
fn feed(meters: &mut Loudness, frames: &[[f32; 2]], playing: bool) {
    for piece in frames.chunks(480) {
        meters.feed(piece, playing);
    }
}

fn near(got: Option<f64>, want: f64, within: f64) {
    let got = got.expect("a reading");
    assert!(
        (got - want).abs() <= within,
        "got {got}, wanted {want} ± {within}"
    );
}

#[test]
fn a_1_khz_sine_at_minus_23_dbfs_reads_minus_23_lufs() {
    // EBU Tech 3341, 2.9, case 1: a stereo 1 kHz sine at −23 dBFS is −23.0 LUFS ± 0.1.
    let mut meters = Loudness::new(RATE).unwrap();
    assert_eq!(
        (
            meters.short_term(),
            meters.integrated(),
            meters.true_peak_db()
        ),
        (None, None, None),
        "nothing heard yet"
    );
    feed(&mut meters, &sine(1000.0, -23.0, 20.0), true);
    near(meters.integrated(), -23.0, 0.1);
    near(meters.short_term(), -23.0, 0.1);
    near(meters.true_peak_db(), -23.0, 0.1);
}

#[test]
fn the_true_peak_hears_what_is_between_the_samples() {
    // A sine at a quarter of the rate, its samples at 45°: every sample is at 0.707 of the
    // height the wave reaches between them (−3 dB under it).
    let wave: Vec<[f32; 2]> = (0..RATE as usize)
        .map(|i| {
            let s = 0.5 * [1.0, 1.0, -1.0, -1.0][i % 4] * std::f32::consts::FRAC_1_SQRT_2;
            [s, s]
        })
        .collect();
    let mut meters = Loudness::new(RATE).unwrap();
    feed(&mut meters, &wave, true);
    near(meters.true_peak_db(), -6.02, 0.2);
    // The same as the export's true peak.
    let mut export = TruePeak::new(RATE).unwrap();
    export.feed(wave.as_flattened());
    near(meters.true_peak_db(), f64::from(export.db().unwrap()), 0.01);
}

/// Something like a song: two sines whose levels move, a loud part, and a quiet one under the
/// relative gate.
fn song() -> Vec<[f32; 2]> {
    let seconds = 24.0;
    (0..(seconds * f64::from(RATE)) as usize)
        .map(|n| {
            let t = n as f64 / f64::from(RATE);
            let level = match t {
                t if t < 6.0 => 0.25,
                t if t < 12.0 => 0.6 * (0.5 + 0.5 * (t * 1.3).sin().abs()),
                t if t < 16.0 => 0.02,
                _ => 0.4,
            };
            let l = level
                * ((std::f64::consts::TAU * 220.0 * t).sin()
                    + 0.5 * (std::f64::consts::TAU * 3300.0 * t).sin());
            let r = level
                * ((std::f64::consts::TAU * 330.0 * t).sin()
                    + 0.3 * (std::f64::consts::TAU * 70.0 * t).sin());
            [(0.6 * l) as f32, (0.6 * r) as f32]
        })
        .collect()
}

#[test]
fn the_play_reads_as_the_export_measures_the_same_sound() {
    let audio = song();
    let mut meters = Loudness::new(RATE).unwrap();
    feed(&mut meters, &audio, true);
    let mut export = Listener::new(RATE, None, &[]);
    export.feed(audio.as_flattened());
    let measured = export.finish().lufs.expect("measured");
    near(meters.integrated(), f64::from(measured), 0.1);
}

#[test]
fn the_play_measures_only_what_plays_from_its_start_and_keeps_it_after() {
    let mut meters = Loudness::new(RATE).unwrap();
    // Keys while stopped: heard now, not part of a play.
    feed(&mut meters, &sine(1000.0, -10.0, 4.0), false);
    near(meters.short_term(), -10.0, 0.1);
    assert_eq!((meters.integrated(), meters.true_peak_db()), (None, None));

    meters.start_play();
    feed(&mut meters, &sine(1000.0, -23.0, 10.0), true);
    near(meters.integrated(), -23.0, 0.1);
    near(meters.true_peak_db(), -23.0, 0.1);

    // Stopped again: louder keys show now, and the play keeps what it measured.
    feed(&mut meters, &sine(1000.0, -6.0, 4.0), false);
    near(meters.short_term(), -6.0, 0.1);
    near(meters.integrated(), -23.0, 0.1);
    near(meters.true_peak_db(), -23.0, 0.1);

    // The next play starts from nothing.
    meters.start_play();
    assert_eq!((meters.integrated(), meters.true_peak_db()), (None, None));
    feed(&mut meters, &sine(1000.0, -30.0, 5.0), true);
    near(meters.integrated(), -30.0, 0.1);
    near(meters.true_peak_db(), -30.0, 0.1);
}

#[test]
fn silence_reads_none_and_a_stopped_stream_forgets_only_what_is_heard_now() {
    let mut meters = Loudness::new(RATE).unwrap();
    feed(&mut meters, &[[0.0; 2]; 48_000], true);
    assert_eq!(
        (
            meters.short_term(),
            meters.integrated(),
            meters.true_peak_db()
        ),
        (None, None, None)
    );
    feed(&mut meters, &sine(1000.0, -20.0, 5.0), true);
    meters.stop_hearing();
    assert_eq!(meters.short_term(), None);
    near(meters.integrated(), -20.0, 0.1);
    meters.restart();
    assert_eq!(
        (
            meters.short_term(),
            meters.integrated(),
            meters.true_peak_db()
        ),
        (None, None, None)
    );
}

#[test]
fn a_rate_the_meters_do_not_take_gives_none() {
    assert!(Loudness::new(0).is_none());
    assert_eq!(Loudness::new(44_100).map(|m| m.rate()), Some(44_100));
}

/// The thread keeps numbers smaller than the smallest normal one (no flush-to-zero).
fn denormals_kept() -> bool {
    let tiny = std::hint::black_box(f32::MIN_POSITIVE);
    let half = std::hint::black_box(0.5f32);
    tiny * half != 0.0 && std::hint::black_box(tiny * half) * std::hint::black_box(1.0) != 0.0
}

#[test]
fn feeding_leaves_the_threads_float_settings_as_they_were() {
    assert!(denormals_kept());
    let mut meters = Loudness::new(RATE).unwrap();
    feed(&mut meters, &sine(1000.0, -20.0, 0.5), true);
    assert!(denormals_kept(), "flush-to-zero was left on");
}
