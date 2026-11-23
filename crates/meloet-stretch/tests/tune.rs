//! 「音程を寄せる」 end to end on made signals: the pitch is looked for, the curve is made and the
//! stretcher follows it. The measured values are printed (`-- --nocapture`).

mod common;

use common::*;
use meloet_stretch::{
    PitchTrack, Preset, RenderError, Stretcher, Tune, note_of_hz, render_tuned, track_pitch,
};
use std::convert::Infallible;
use std::f64::consts::TAU;
use std::sync::atomic::AtomicBool;

const RATE: u32 = 48_000;

fn preset() -> Preset {
    Preset::for_tuning(RATE as f32)
}

const C_MAJOR: [bool; 12] = [
    true, false, true, false, true, true, false, true, false, true, false, true,
];

fn tune(strength: f32) -> Tune {
    Tune {
        scale: C_MAJOR,
        strength,
        transpose: 0.0,
    }
}

fn hz_of(note: f64) -> f64 {
    440.0 * 2f64.powf((note - 69.0) / 12.0)
}

/// A voice-like tone (harmonics falling by 1/n, none above 5 kHz) whose note at frame `i` is
/// `note(i)`.
fn voice(frames: usize, note: impl Fn(usize) -> f64) -> Vec<f32> {
    let mut phase = 0.0f64;
    (0..frames)
        .map(|i| {
            let hz = hz_of(note(i));
            phase += hz / SR;
            let harmonics = ((5000.0 / hz) as usize).clamp(1, 12);
            let s: f64 = (1..=harmonics)
                .map(|h| (TAU * phase * h as f64).sin() / h as f64)
                .sum();
            (s * 0.25) as f32
        })
        .collect()
}

fn noise(frames: usize, seed: u32) -> Vec<f32> {
    let mut s = seed;
    (0..frames)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((s >> 8) as f32 / (1 << 23) as f32 - 1.0) * 0.3
        })
        .collect()
}

fn tuned(
    st: &mut Stretcher,
    file: &[Vec<f32>],
    out_frames: u64,
    p: impl Fn(u64) -> f64,
    tune: &Tune,
) -> Vec<Vec<f32>> {
    let mut out = vec![Vec::with_capacity(out_frames as usize); file.len()];
    let write = |block: &[&[f32]]| {
        for (o, b) in out.iter_mut().zip(block) {
            o.extend_from_slice(b);
        }
        Ok(())
    };
    let done: Result<(), RenderError<Infallible>> = render_tuned(
        st,
        out_frames,
        p,
        RATE,
        tune,
        read_from(file),
        write,
        |_| {},
        &AtomicBool::new(false),
    );
    done.expect("render");
    out
}

fn pitch_of(x: &[f32]) -> PitchTrack {
    let file = [x.to_vec()];
    let got: Result<_, RenderError<Infallible>> = track_pitch(
        RATE,
        1,
        0,
        x.len() as i64,
        read_from(&file),
        |_| {},
        &AtomicBool::new(false),
    );
    got.unwrap()
}

/// The notes of the points from `from` to `to` seconds that have one.
fn notes_between(track: &PitchTrack, from: f64, to: f64) -> Vec<f32> {
    let at = |s: f64| (s * SR / track.hop as f64) as usize;
    track.notes[at(from)..at(to).min(track.notes.len())]
        .iter()
        .copied()
        .filter(|n| !n.is_nan())
        .collect()
}

fn median(mut x: Vec<f32>) -> f32 {
    assert!(!x.is_empty());
    x.sort_by(f32::total_cmp);
    x[x.len() / 2]
}

/// The median distance from `note`, in cents.
fn median_off(track: &PitchTrack, note: f32, from: f64, to: f64) -> f32 {
    let notes = notes_between(track, from, to);
    assert!(notes.len() as f64 > (to - from) * 90.0, "{}", notes.len());
    median(notes.iter().map(|n| (n - note).abs() * 100.0).collect())
}

#[test]
fn a_flat_note_comes_to_the_note_at_full_strength_and_half_way_at_half() {
    let frames = 3 * 48_000usize;
    // A (57) and E (64) are notes of C major; each is sung 30 cents under.
    for note in [57.0f64, 64.0, 45.0, 72.0] {
        let saw = voice(frames, |_| note - 0.3);
        let sine = sine(hz_of(note - 0.3), frames, 0.4);
        for (name, file) in [("saw", saw), ("sine", sine)] {
            let mut st = stretcher(1, preset());
            let before = median_off(&pitch_of(&file), note as f32, 0.5, 2.5);
            let full = tuned(
                &mut st,
                std::slice::from_ref(&file),
                frames as u64,
                |j| j as f64,
                &tune(1.0),
            );
            let half = tuned(
                &mut st,
                std::slice::from_ref(&file),
                frames as u64,
                |j| j as f64,
                &tune(0.5),
            );
            let off_full = median_off(&pitch_of(&full[0]), note as f32, 0.5, 2.5);
            let off_half = median_off(&pitch_of(&half[0]), note as f32, 0.5, 2.5);
            println!(
                "{name} {note}: {before:.1} cents off before, {off_full:.1} at 100 %, {off_half:.1} at 50 %"
            );
            assert!((before - 30.0).abs() < 1.0);
            assert!(off_full < 10.0, "{name} {note}: {off_full}");
            assert!((off_half - 15.0).abs() < 6.0, "{name} {note}: {off_half}");
            // As loud as before.
            let level = rms(&full[0][24_000..120_000]) / rms(&file[24_000..120_000]);
            assert!((level - 1.0).abs() < 0.1, "{name} {note}: level {level}");
        }
    }
}

#[test]
fn a_note_of_the_scale_noise_and_strength_nothing_are_what_they_are_without() {
    // Nothing to move: every block is made as in a clip that is not tuned, to the bit.
    let frames = 2 * 48_000usize;
    let plain = |file: &[Vec<f32>]| {
        let mut st = stretcher(file.len(), preset());
        st.set_transpose_semitones(0.0, None);
        render_all(&mut st, file, frames as u64, |j| j as f64)
    };
    let in_scale = voice(frames, |_| 60.0);
    let hiss = noise(frames, 5);
    let flat = voice(frames, |_| 59.6);
    for (name, file, t) in [
        ("a note of the scale", vec![in_scale.clone()], tune(1.0)),
        ("noise", vec![hiss.clone(), noise(frames, 6)], tune(1.0)),
        ("strength 0", vec![flat], tune(0.0)),
    ] {
        let mut st = stretcher(file.len(), preset());
        let out = tuned(&mut st, &file, frames as u64, |j| j as f64, &t);
        assert!(out == plain(&file), "{name}");
    }
    // And that is the file itself but for what the stretcher does at rate 1.
    let out = plain(std::slice::from_ref(&hiss));
    let error: Vec<f32> = out[0].iter().zip(&hiss).map(|(a, b)| a - b).collect();
    println!(
        "noise at rate 1: error {:.1} dB below the signal",
        20.0 * (rms(&hiss) / rms(&error)).log10()
    );
    assert!(rms(&error) < 0.01 * rms(&hiss));
}

#[test]
fn noise_around_a_note_stays_and_the_note_moves() {
    // Half a second of noise, a second of a flat note, half a second of noise.
    let mut file = noise(24_000, 3);
    file.extend(voice(48_000, |_| 63.7));
    file.extend(noise(24_000, 4));
    let frames = file.len();
    let mut st = stretcher(1, preset());
    let out = tuned(
        &mut st,
        std::slice::from_ref(&file),
        frames as u64,
        |j| j as f64,
        &tune(1.0),
    );
    let mut plain = stretcher(1, preset());
    plain.set_transpose_semitones(0.0, None);
    let untuned = render_all(
        &mut plain,
        std::slice::from_ref(&file),
        frames as u64,
        |j| j as f64,
    );
    // More than 200 ms from the note, nothing is other than without tuning.
    assert!(out[0][..12_000] == untuned[0][..12_000]);
    // After the note the blocks are made the same way again, from phases that have been
    // somewhere else: other samples, the same sound.
    let (a, b) = (&out[0][frames - 9_000..], &untuned[0][frames - 9_000..]);
    assert!((rms(a) / rms(b) - 1.0).abs() < 0.02);
    for (lo, hi) in [(100.0, 1000.0), (1000.0, 4000.0), (4000.0, 12_000.0)] {
        let ratio = band_energy(a, lo, hi) / band_energy(b, lo, hi);
        println!("noise after the note, {lo}..{hi} Hz: energy x{ratio:.3}");
        assert!((ratio - 1.0).abs() < 0.1, "{lo}: {ratio}");
    }
    let off = median_off(&pitch_of(&out[0]), 64.0, 0.7, 1.3);
    println!("the note between noise: {off:.1} cents off");
    assert!(off < 10.0, "{off}");
}

#[test]
fn a_slide_stays_a_slide() {
    // From C (60) up to D (62) over a second, held before and after.
    let frames = 3 * 48_000usize;
    let file = voice(frames, |i| {
        60.0 + 2.0 * ((i as f64 - 48_000.0) / 48_000.0).clamp(0.0, 1.0)
    });
    let mut st = stretcher(1, preset());
    let out = tuned(
        &mut st,
        std::slice::from_ref(&file),
        frames as u64,
        |j| j as f64,
        &tune(1.0),
    );
    let track = pitch_of(&out[0]);
    let notes = notes_between(&track, 0.3, 2.7);
    assert!(notes.len() > 230);
    let most = notes
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    let low = notes.iter().position(|n| *n > 60.4).unwrap();
    let high = notes.iter().position(|n| *n > 61.6).unwrap();
    println!(
        "slide: the largest move in 10 ms is {:.0} cents; from 20 % to 80 % in {} ms",
        most * 100.0,
        (high - low) * 10
    );
    // A step would be 200 cents from one point to the next.
    assert!(most < 0.8, "{most}");
    assert!(high - low >= 3);
    // The held notes are on the notes.
    assert!(median_off(&track, 60.0, 0.3, 0.9) < 5.0);
    assert!(median_off(&track, 62.0, 2.1, 2.7) < 5.0);
}

#[test]
fn the_length_and_the_place_stay() {
    // A click in silence, then a flat note that starts at 1 s and ends at 2 s.
    let frames = 3 * 48_000usize;
    let mut file = vec![0.0f32; frames];
    add_click(&mut file, 20_000);
    for (i, s) in voice(48_000, |_| 66.7).into_iter().enumerate() {
        file[48_000 + i] = s;
    }
    let mut st = stretcher(1, preset());
    let out = tuned(
        &mut st,
        std::slice::from_ref(&file),
        frames as u64,
        |j| j as f64,
        &tune(1.0),
    );
    assert_eq!(out[0].len(), frames);
    let click = onset(&out[0][15_000..25_000], 0.2).unwrap() + 15_000;
    let start = onset(&out[0][40_000..], 0.05).unwrap() + 40_000;
    let end = frames - onset_back(&out[0], 0.05);
    println!("click at {click} (20000), the note from {start} (48000) to {end} (96000)");
    assert!((click as i64 - 20_000).abs() <= 48);
    assert!((start as i64 - 48_000).abs() <= 240);
    assert!((end as i64 - 96_000).abs() <= 240);
    // 66.7 is nearest to G (67).
    assert!(median_off(&pitch_of(&out[0]), 67.0, 1.2, 1.8) < 10.0);
}

/// The first frame louder than `level`.
fn onset(x: &[f32], level: f32) -> Option<usize> {
    x.iter().position(|s| s.abs() > level)
}

/// Frames from the end back to the last one louder than `level`.
fn onset_back(x: &[f32], level: f32) -> usize {
    x.iter().rev().position(|s| s.abs() > level).unwrap()
}

#[test]
fn stretched_transposed_and_tuned_in_one_run() {
    // The clip plays the file 1.25 times as long, two semitones up. The voice is at 57.7:
    // two up it is 59.7, 30 cents under C (60).
    let frames = 3 * 48_000usize;
    let file = voice(frames, |_| 57.7);
    let out_frames = (frames as f64 * 1.25) as u64;
    let t = Tune {
        transpose: 2.0,
        ..tune(1.0)
    };
    let mut st = stretcher(1, preset());
    st.set_formant_factor(1.0, true);
    let out = tuned(
        &mut st,
        std::slice::from_ref(&file),
        out_frames,
        |j| j as f64 / 1.25,
        &t,
    );
    assert_eq!(out[0].len() as u64, out_frames);
    let track = pitch_of(&out[0]);
    let off = median_off(&track, 60.0, 0.5, 3.2);
    println!("x1.25, +2 and tuned: {off:.1} cents off C");
    assert!(off < 10.0, "{off}");
    // Without the tuning it is where the transpose alone puts it.
    let mut plain = stretcher(1, preset());
    plain.set_transpose_semitones(2.0, None);
    plain.set_formant_factor(1.0, true);
    let untuned = render_all(&mut plain, &[file], out_frames, |j| j as f64 / 1.25);
    let off = median_off(&pitch_of(&untuned[0]), 59.7, 0.5, 3.2);
    println!("x1.25 and +2 alone: {off:.1} cents off 59.7");
    assert!(off < 10.0, "{off}");
}

#[test]
fn a_vibrato_is_softened_and_centred() {
    // ±40 cents at 5.5 Hz around 20 cents under E (64).
    let frames = 3 * 48_000usize;
    let file = voice(frames, |i| 63.8 + 0.4 * (TAU * 5.5 * i as f64 / SR).sin());
    let mut st = stretcher(1, preset());
    let out = tuned(
        &mut st,
        std::slice::from_ref(&file),
        frames as u64,
        |j| j as f64,
        &tune(1.0),
    );
    let depth = |x: &[f32]| {
        let notes = notes_between(&pitch_of(x), 0.5, 2.5);
        let mean = notes.iter().sum::<f32>() / notes.len() as f32;
        let mut off: Vec<f32> = notes.iter().map(|n| (n - mean).abs()).collect();
        off.sort_by(f32::total_cmp);
        (mean, off[off.len() * 9 / 10])
    };
    let (mean_in, depth_in) = depth(&file);
    let (mean_out, depth_out) = depth(&out[0]);
    println!(
        "vibrato: mean {mean_in:.2} → {mean_out:.2}, depth {:.0} → {:.0} cents",
        depth_in * 100.0,
        depth_out * 100.0
    );
    assert!((mean_out - 64.0).abs() < 0.08, "{mean_out}");
    assert!(depth_out < depth_in && depth_out > 0.05, "{depth_out}");
    let _ = note_of_hz(440.0);
}
