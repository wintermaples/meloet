//! What the built-in effects do to known signals.

mod support;

use meloet_engine::Effect;
use meloet_model::effects::{
    EffectKind, chorus, compressor, de_esser, delay, eq4, gate, limiter, reverb, saturation,
};
use support::*;

// ---------------------------------------------------------------------------------------------
// 1. Silence in, silence out.

#[test]
fn silence_gives_exact_silence() {
    for kind in EffectKind::ALL {
        let mut fx = prepared(kind);
        let mut l = vec![0.0; frames(1.0)];
        let mut r = l.clone();
        run(fx.as_mut(), &mut l, &mut r);
        assert_eq!(peak(&l), 0.0, "{}", kind.name());
        assert_eq!(peak(&r), 0.0, "{}", kind.name());
    }
}

#[test]
fn the_tail_dies_to_silence() {
    for kind in EffectKind::ALL {
        let mut fx = prepared(kind);
        let mut l = noise(0.5, frames(1.0), 1);
        let mut r = noise(0.5, frames(1.0), 2);
        run(fx.as_mut(), &mut l, &mut r);
        // Reverb 2.4 s to -60 dB is 5.6 s to -140 dB; delay 35 % each 250 ms is 3.9 s.
        let mut l = vec![0.0; frames(8.0)];
        let mut r = l.clone();
        run(fx.as_mut(), &mut l, &mut r);
        let last = frames(7.5);
        let tail = peak(&l[last..]).max(peak(&r[last..]));
        assert!(tail < 1.0e-7, "{}: {tail:e}", kind.name());
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Known shapes.

#[test]
fn eq_at_0_db_passes_the_input_through() {
    let mut fx = prepared(EffectKind::Eq4);
    let mut l = noise(0.8, frames(0.5), 3);
    l[0] = 1.0;
    let mut r = noise(0.8, frames(0.5), 4);
    let (in_l, in_r) = (l.clone(), r.clone());
    run(fx.as_mut(), &mut l, &mut r);
    let worst = l
        .iter()
        .zip(&in_l)
        .chain(r.iter().zip(&in_r))
        .fold(0.0_f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(worst < 1.0e-6, "{worst:e}");
}

/// The gain in dB the effect gives a steady sine.
fn sine_gain_db(fx: &mut dyn Effect, freq: f32) -> f32 {
    let mut l = sine(freq, 0.1, frames(1.5));
    let mut r = l.clone();
    let input = rms(&l[frames(1.0)..]);
    run(fx, &mut l, &mut r);
    db(rms(&l[frames(1.0)..]) / input)
}

#[test]
fn eq_peaks_are_as_high_as_set() {
    for (freq_at, gain_at, freq) in [
        (eq4::LOW_MID_FREQ, eq4::LOW_MID_GAIN, 400.0),
        (eq4::HIGH_MID_FREQ, eq4::HIGH_MID_GAIN, 2500.0),
        (eq4::HIGH_MID_FREQ, eq4::HIGH_MID_GAIN, 12000.0),
    ] {
        for gain_db in [9.0, -12.0, 18.0, -18.0] {
            let mut fx = prepared(EffectKind::Eq4);
            fx.set_param(freq_at, freq);
            fx.set_param(gain_at, gain_db);
            let got = sine_gain_db(fx.as_mut(), freq);
            assert!((got - gain_db).abs() < 0.5, "{freq} Hz {gain_db} dB: {got}");
            // Two octaves away, a 1-octave peak leaves the level almost alone.
            let away = sine_gain_db(fx.as_mut(), freq / 8.0);
            assert!(
                away.abs() < 1.0,
                "{freq} Hz {gain_db} dB, 3 oct below: {away}"
            );
        }
    }
}

#[test]
fn eq_peak_width_is_in_octaves() {
    // RBJ's width: the gain is half (in dB) at the edges of the band.
    for width in [0.5, 1.0, 2.0] {
        let mut fx = prepared(EffectKind::Eq4);
        fx.set_param(eq4::LOW_MID_FREQ, 1000.0);
        fx.set_param(eq4::LOW_MID_GAIN, 12.0);
        fx.set_param(eq4::LOW_MID_WIDTH, width);
        let edge = 1000.0 * 2.0_f32.powf(width / 2.0);
        let got = sine_gain_db(fx.as_mut(), edge);
        assert!(
            (got - 6.0).abs() < 0.75,
            "width {width}: {got} dB at {edge} Hz"
        );
    }
}

#[test]
fn eq_shelves_reach_the_set_gain() {
    for gain_db in [12.0, -12.0] {
        let mut fx = prepared(EffectKind::Eq4);
        fx.set_param(eq4::LOW_FREQ, 400.0);
        fx.set_param(eq4::LOW_GAIN, gain_db);
        let far = sine_gain_db(fx.as_mut(), 30.0);
        let at = sine_gain_db(fx.as_mut(), 400.0);
        let above = sine_gain_db(fx.as_mut(), 6000.0);
        assert!((far - gain_db).abs() < 0.5, "low shelf {gain_db}: {far}");
        assert!(
            (at - gain_db / 2.0).abs() < 0.5,
            "low shelf {gain_db} at corner: {at}"
        );
        assert!(above.abs() < 0.5, "low shelf {gain_db} above: {above}");

        let mut fx = prepared(EffectKind::Eq4);
        fx.set_param(eq4::HIGH_FREQ, 2000.0);
        fx.set_param(eq4::HIGH_GAIN, gain_db);
        let far = sine_gain_db(fx.as_mut(), 18000.0);
        let at = sine_gain_db(fx.as_mut(), 2000.0);
        let below = sine_gain_db(fx.as_mut(), 100.0);
        assert!((far - gain_db).abs() < 0.5, "high shelf {gain_db}: {far}");
        assert!(
            (at - gain_db / 2.0).abs() < 0.5,
            "high shelf {gain_db} at corner: {at}"
        );
        assert!(below.abs() < 0.5, "high shelf {gain_db} below: {below}");
    }
}

fn open_delay(interval_ms: f32, feedback: f32) -> Box<dyn Effect> {
    let mut fx = prepared(EffectKind::Delay);
    fx.set_param(delay::SYNC, 0.0);
    fx.set_param(delay::INTERVAL_MS, interval_ms);
    fx.set_param(delay::FEEDBACK, feedback);
    fx.set_param(delay::HIGH_CUT, 20000.0);
    fx.set_param(delay::LOW_CUT, 20.0);
    fx
}

fn first_sound(x: &[f32]) -> Option<usize> {
    x.iter().position(|v| *v != 0.0)
}

#[test]
fn delay_repeats_arrive_on_the_frame() {
    let mut fx = open_delay(100.0, 50.0);
    let mut l = vec![0.0; frames(0.5)];
    l[0] = 1.0;
    let mut r = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    assert_eq!(first_sound(&l), Some(4800));
    assert_eq!(first_sound(&r), Some(4800));
    // Between the repeats there is only the filters' short ringing.
    assert!(peak(&l[4800..4810]) > 0.5);
    let second = l[9600..9610].iter().fold(0.0_f32, |m, v| m.max(v.abs()));
    assert!(second > 0.2 && second < 0.5, "{second}");
    assert!(peak(&l[9000..9600]) < 1.0e-3);
}

#[test]
fn delay_follows_the_tempo_when_sync_is_on() {
    // The steps in order: 1/4, dotted 1/8, 1/8, 1/8 triplet, 1/16.
    for (step, tempo, want) in [
        (0.0, 120.0, 24000),
        (1.0, 120.0, 18000),
        (2.0, 120.0, 12000),
        (3.0, 120.0, 8000),
        (4.0, 120.0, 6000),
        (2.0, 96.0, 15000),
        (0.0, 30.0, 96000),
    ] {
        let mut fx = prepared(EffectKind::Delay);
        fx.set_param(delay::INTERVAL_STEP, step);
        let mut l = vec![0.0; frames(2.5)];
        l[0] = 1.0;
        let mut r = l.clone();
        run_at(fx.as_mut(), &mut l, &mut r, tempo);
        assert_eq!(first_sound(&l), Some(want), "step {step} at {tempo}");
    }
    // The default: sync on, 1/8.
    let mut fx = prepared(EffectKind::Delay);
    let mut l = vec![0.0; frames(0.5)];
    l[0] = 1.0;
    let mut r = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    assert_eq!(first_sound(&l), Some(12000));
}

#[test]
fn delay_repeats_shrink_by_the_feedback() {
    for feedback in [25.0, 50.0, 95.0] {
        let mut fx = open_delay(100.0, feedback);
        // A 20 ms burst of 1 kHz, where the loop's filters (20 Hz, 20 kHz) are flat.
        let burst = frames(0.02);
        let mut l = sine(1000.0, 0.5, frames(0.6));
        for (i, v) in l.iter_mut().enumerate() {
            let window = if i < burst {
                0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / burst as f32).cos()
            } else {
                0.0
            };
            *v *= window;
        }
        let mut r = l.clone();
        let input = rms(&l[..burst]);
        run(fx.as_mut(), &mut l, &mut r);
        let echo = |n: usize| rms(&l[n * 4800..n * 4800 + burst]);
        let first = db(echo(1) / input);
        assert!(first.abs() < 0.5, "first repeat: {first} dB");
        let want = db(feedback / 100.0);
        for n in 2..=5 {
            let step = db(echo(n) / echo(n - 1));
            assert!(
                (step - want).abs() < 0.5,
                "feedback {feedback}: repeat {n} is {step} dB, want {want}"
            );
        }
    }
}

#[test]
fn delay_cuts_inside_the_loop() {
    // With the high cut at 1 kHz, every pass takes more off a 4 kHz tone than the feedback.
    let mut fx = open_delay(100.0, 50.0);
    fx.set_param(delay::HIGH_CUT, 1000.0);
    let mut l = sine(4000.0, 0.5, frames(0.6));
    l[frames(0.05)..].fill(0.0);
    let mut r = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    let echo = |n: usize| rms(&l[n * 4800 + 480..n * 4800 + 1920]);
    let step = db(echo(3) / echo(2));
    assert!(step < -15.0, "{step} dB");
}

#[test]
fn delay_ping_pong_alternates_sides() {
    let mut fx = open_delay(100.0, 50.0);
    fx.set_param(delay::PING_PONG, 1.0);
    let mut l = vec![0.0; frames(0.5)];
    l[0] = 1.0;
    let mut r = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    let at = |x: &[f32], n: usize| peak(&x[n * 4800..n * 4800 + 20]);
    assert!(at(&l, 1) > 0.5 && at(&r, 1) < 1.0e-3);
    assert!(at(&r, 2) > 0.2 && at(&l, 2) < 1.0e-3);
    assert!(at(&l, 3) > 0.1 && at(&r, 3) < 1.0e-3);
}

/// Seconds for the response to fall 60 dB, from the slope between -5 and -35 dB of the energy
/// still to come (Schroeder's backward integration).
fn decay_time(response: &[f32]) -> f32 {
    let mut left = 0.0_f64;
    let energy: Vec<f64> = response
        .iter()
        .rev()
        .map(|v| {
            left += f64::from(*v) * f64::from(*v);
            left
        })
        .collect();
    let total = left;
    let at = |level_db: f64| {
        let want = total * 10.0_f64.powf(level_db / 10.0);
        // `energy` runs backwards in time: find the first frame (in time) below `want`.
        let from_end = energy.partition_point(|e| *e < want);
        (response.len() - from_end) as f32 / SR
    };
    2.0 * (at(-35.0) - at(-5.0))
}

fn reverb_response(fx: &mut dyn Effect, seconds: f32) -> Vec<f32> {
    let mut l = vec![0.0; frames(seconds)];
    l[0] = 1.0;
    let mut r = l.clone();
    run(fx, &mut l, &mut r);
    l.iter().zip(&r).map(|(a, b)| a + b).collect()
}

fn low_passed(x: &[f32], cutoff: f32) -> Vec<f32> {
    let coef = 1.0 - (-std::f32::consts::TAU * cutoff / SR).exp();
    let (mut a, mut b) = (0.0_f32, 0.0_f32);
    x.iter()
        .map(|v| {
            a += coef * (v - a);
            b += coef * (a - b);
            b
        })
        .collect()
}

#[test]
fn reverb_decay_is_the_time_to_minus_60_db() {
    for decay in [2.4, 0.5, 8.0, 0.2, 10.0] {
        // Everything else at its default: below the high cut (6 kHz) the tail takes `decay`.
        let mut fx = prepared(EffectKind::Reverb);
        fx.set_param(reverb::DECAY, decay);
        let response = low_passed(&reverb_response(fx.as_mut(), decay * 1.2 + 0.3), 1500.0);
        let got = decay_time(&response);
        println!("reverb decay {decay} s, defaults: measured {got:.2} s");
        assert!(
            (got / decay - 1.0).abs() < 0.15,
            "decay {decay} s, defaults: measured {got} s"
        );

        // With the cuts opened up, the whole band takes `decay`.
        for size in [0.0, 60.0, 100.0] {
            let mut fx = prepared(EffectKind::Reverb);
            fx.set_param(reverb::DECAY, decay);
            fx.set_param(reverb::SIZE, size);
            fx.set_param(reverb::HIGH_CUT, 20000.0);
            fx.set_param(reverb::LOW_CUT, 20.0);
            let got = decay_time(&reverb_response(fx.as_mut(), decay * 1.2 + 0.3));
            println!("reverb decay {decay} s, size {size}, open: measured {got:.2} s");
            assert!(
                (got / decay - 1.0).abs() < 0.15,
                "decay {decay} s, size {size}, open: measured {got} s"
            );
        }
    }
}

#[test]
fn reverb_pre_delay_width_and_mix() {
    let mut fx = prepared(EffectKind::Reverb);
    fx.set_param(reverb::PRE_DELAY, 100.0);
    let mut l = vec![0.0; frames(0.5)];
    l[0] = 1.0;
    let mut r = vec![0.0; frames(0.5)];
    run(fx.as_mut(), &mut l, &mut r);
    // Mix 100 %: no dry signal, and nothing before the pre-delay.
    assert_eq!(first_sound(&l), Some(4800));
    assert!(rms(&l[4800..]) > 1.0e-3);
    let apart = l.iter().zip(&r).any(|(a, b)| (a - b).abs() > 1.0e-4);
    assert!(apart, "width 100 %: the sides differ");

    let mut fx = prepared(EffectKind::Reverb);
    fx.set_param(reverb::WIDTH, 0.0);
    let mut l = noise(0.5, frames(0.5), 5);
    let mut r = noise(0.5, frames(0.5), 6);
    run(fx.as_mut(), &mut l, &mut r);
    assert!(l == r, "width 0 %: mono");

    let mut fx = prepared(EffectKind::Reverb);
    fx.set_param(reverb::MIX, 0.0);
    let mut l = noise(0.5, frames(0.5), 7);
    let mut r = noise(0.5, frames(0.5), 8);
    let (in_l, in_r) = (l.clone(), r.clone());
    run(fx.as_mut(), &mut l, &mut r);
    assert!(l == in_l && r == in_r, "mix 0 %: dry");
}

#[test]
fn reverb_is_about_as_loud_as_its_input() {
    // On a return at mix 100 %, steady noise comes back within a few dB of what was sent.
    for (decay, size) in [
        (2.4, 60.0),
        (2.4, 0.0),
        (2.4, 100.0),
        (0.5, 60.0),
        (8.0, 60.0),
    ] {
        let mut fx = prepared(EffectKind::Reverb);
        fx.set_param(reverb::DECAY, decay);
        fx.set_param(reverb::SIZE, size);
        let mut l = noise(0.25, frames(decay + 1.0), 9);
        let mut r = noise(0.25, frames(decay + 1.0), 10);
        let input = rms(&l);
        run(fx.as_mut(), &mut l, &mut r);
        let level = db(rms(&l[frames(decay)..]) / input);
        println!("reverb decay {decay} size {size}: {level:.1} dB");
        assert!(
            level > -18.0 && level < 6.0,
            "decay {decay} size {size}: {level} dB"
        );
    }
}

#[test]
fn compressor_follows_the_formula() {
    // Soft knee 6 dB wide around the threshold, as in Giannoulis et al.
    fn expected_gain_db(level: f32, threshold: f32, ratio: f32) -> f32 {
        let knee = 6.0;
        let over = level - threshold;
        if 2.0 * over < -knee {
            0.0
        } else if 2.0 * over.abs() <= knee {
            (1.0 / ratio - 1.0) * (over + knee / 2.0).powi(2) / (2.0 * knee)
        } else {
            (1.0 / ratio - 1.0) * over
        }
    }
    for (threshold, ratio, makeup) in [(-20.0, 4.0, 0.0), (-30.0, 20.0, 6.0), (-18.0, 1.0, 0.0)] {
        for level in [-50.0, -23.0, -20.0, -17.0, -10.0, 0.0] {
            let mut fx = prepared(EffectKind::Compressor);
            fx.set_param(compressor::THRESHOLD, threshold);
            fx.set_param(compressor::RATIO, ratio);
            fx.set_param(compressor::MAKEUP, makeup);
            // Fast attack and slow release: the detector sits on the sine's peaks.
            fx.set_param(compressor::ATTACK, 0.1);
            fx.set_param(compressor::RELEASE, 1000.0);
            let mut l = sine(100.0, gain(level), frames(1.0));
            let mut r = l.clone();
            run(fx.as_mut(), &mut l, &mut r);
            let got = db(peak(&l[frames(0.8)..])) - level;
            let want = expected_gain_db(level, threshold, ratio) + makeup;
            assert!(
                (got - want).abs() < 0.5,
                "threshold {threshold} ratio {ratio} level {level}: {got} dB, want {want}"
            );
            let told = meloet_sound::effects::static_gain_db(level, threshold, ratio) + makeup;
            assert!((told - want).abs() < 1.0e-3);
        }
    }
}

#[test]
fn compressor_links_the_sides() {
    let mut fx = prepared(EffectKind::Compressor);
    fx.set_param(compressor::THRESHOLD, -30.0);
    let mut l = sine(1000.0, 0.8, frames(0.5));
    let mut r = sine(1000.0, 0.01, frames(0.5));
    run(fx.as_mut(), &mut l, &mut r);
    let from = frames(0.4);
    let left_gain = db(peak(&l[from..]) / 0.8);
    let right_gain = db(peak(&r[from..]) / 0.01);
    assert!(left_gain < -10.0);
    assert!(
        (left_gain - right_gain).abs() < 0.1,
        "{left_gain} {right_gain}"
    );
}

#[test]
fn limiter_never_goes_over_the_ceiling() {
    let n = frames(1.0);
    let mut impulses = vec![0.0; n];
    for i in (0..n).step_by(997) {
        impulses[i] = if i % 2 == 0 { 100.0 } else { -100.0 };
    }
    let inputs: [(&str, Vec<f32>); 5] = [
        ("+24 dB sine", sine(220.0, gain(24.0), n)),
        ("+24 dB high sine", sine(9000.0, gain(24.0), n)),
        ("impulses", impulses),
        ("noise", noise(4.0, n, 11)),
        ("quiet noise", noise(0.2, n, 12)),
    ];
    for (name, input) in &inputs {
        for ceiling in [-0.3, -6.0, -12.0, 0.0] {
            for pre in [0.0, 12.0] {
                for release in [1.0, 100.0, 1000.0] {
                    let mut fx = prepared(EffectKind::Limiter);
                    fx.set_param(limiter::CEILING, ceiling);
                    fx.set_param(limiter::GAIN, pre);
                    fx.set_param(limiter::RELEASE, release);
                    let mut l = input.clone();
                    let mut r: Vec<f32> = input.iter().rev().copied().collect();
                    run(fx.as_mut(), &mut l, &mut r);
                    let limit = 10.0_f64.powf(f64::from(ceiling) / 20.0) as f32;
                    let top = peak(&l).max(peak(&r));
                    assert!(
                        top <= limit * (1.0 + 1.0e-6),
                        "{name}, ceiling {ceiling}, gain {pre}, release {release}: {top} > {limit}"
                    );
                }
            }
        }
    }
}

#[test]
fn limiter_holds_the_ceiling_while_parameters_move() {
    // The ceiling itself moves smoothly (20 ms), so while it goes down the bound is the place
    // it has reached; it is never above the higher of the old and the new value.
    let mut fx = prepared(EffectKind::Limiter);
    fx.set_param(limiter::CEILING, -12.0);
    let mut l = sine(220.0, 8.0, frames(1.0));
    let mut r = l.clone();
    let half = frames(0.5) / BLOCK * BLOCK;
    run(fx.as_mut(), &mut l[..half], &mut r[..half]);
    fx.set_param(limiter::GAIN, 12.0);
    fx.set_param(limiter::CEILING, -3.0);
    run(fx.as_mut(), &mut l[half..], &mut r[half..]);
    assert!(peak(&l[..half]) <= gain(-12.0) * (1.0 + 1.0e-6));
    assert!(peak(&l[half..]) <= gain(-3.0) * (1.0 + 1.0e-6));
}

#[test]
fn limiter_leaves_quiet_sound_alone_and_recovers() {
    let mut fx = prepared(EffectKind::Limiter);
    let mut l = sine(440.0, 0.5, frames(0.25));
    let mut r = l.clone();
    let input = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    assert!(l == input);

    // After a loud hit the gain comes back: 100 ms release, 1 s later it is within 0.01 dB.
    let mut l = sine(440.0, 0.5, frames(1.5));
    l[10] = 20.0;
    let mut r = l.clone();
    let input = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    let from = frames(1.2);
    let back = db(rms(&l[from..]) / rms(&input[from..]));
    assert!(back.abs() < 0.01, "{back} dB");
    let just_after = db(rms(&l[20..200]) / rms(&input[20..200]));
    assert!(just_after < -15.0, "{just_after} dB");
}

#[test]
fn chorus_mix_and_width() {
    let mut fx = prepared(EffectKind::Chorus);
    fx.set_param(chorus::MIX, 0.0);
    let mut l = noise(0.5, frames(0.5), 13);
    let mut r = l.clone();
    let input = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    assert!(l == input, "mix 0 %: dry");

    let mut fx = prepared(EffectKind::Chorus);
    fx.set_param(chorus::WIDTH, 0.0);
    let mut l = sine(440.0, 0.5, frames(1.0));
    let mut r = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    assert!(l == r, "width 0 %: both sides move together");

    let mut fx = prepared(EffectKind::Chorus);
    let mut l = sine(440.0, 0.5, frames(1.0));
    let mut r = l.clone();
    let input = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    assert!(
        l.iter().zip(&r).any(|(a, b)| (a - b).abs() > 0.01),
        "width 100 %"
    );
    assert!(
        l.iter().zip(&input).any(|(a, b)| (a - b).abs() > 0.01),
        "the sound moves"
    );
    assert!(
        peak(&l) <= 0.5 * 1.01,
        "half dry and half delayed never adds up over the input"
    );

    // Mix 100 %, depth 0: only the 7 ms delay.
    let mut fx = prepared(EffectKind::Chorus);
    fx.set_param(chorus::MIX, 100.0);
    fx.set_param(chorus::DEPTH, 0.0);
    let mut l = vec![0.0; frames(0.1)];
    l[0] = 1.0;
    let mut r = l.clone();
    run(fx.as_mut(), &mut l, &mut r);
    let at = l.iter().position(|v| v.abs() > 0.5);
    assert_eq!(at, Some(336));
}

// ---------------------------------------------------------------------------------------------
// 3. A sudden parameter change does not click.

/// Largest step between neighbouring frames.
fn steepest(x: &[f32]) -> f32 {
    x.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
}

/// A click is a step much larger than the signal's own steps. A 110 Hz sine moves at most
/// 1.44 % of its height per frame. The smooth changes here stay under 1.7 times the steepest
/// step of the steady sound before and after (the worst is the EQ's low shelf going from
/// +18 to -18 dB; the test prints the table). 2.5 leaves room for honest level changes while
/// a parameter moves, and still fails on a jump of 4 % of the height (0.04 / 0.0144 = 2.8).
const CLICK_FACTOR: f32 = 2.5;

const SETTLE_S: f32 = 2.5;
const MOVE_S: f32 = 0.15;
const AFTER_S: f32 = 1.0;

fn lively(kind: EffectKind) -> Box<dyn Effect> {
    let mut fx = prepared(kind);
    match kind {
        EffectKind::Eq4 => {
            for at in [
                eq4::LOW_GAIN,
                eq4::LOW_MID_GAIN,
                eq4::HIGH_MID_GAIN,
                eq4::HIGH_GAIN,
            ] {
                fx.set_param(at, 12.0);
            }
        }
        EffectKind::Compressor => fx.set_param(compressor::THRESHOLD, -30.0),
        EffectKind::Delay => {
            fx.set_param(delay::INTERVAL_MS, 180.0);
            fx.set_param(delay::FEEDBACK, 50.0);
        }
        EffectKind::Limiter => {
            fx.set_param(limiter::GAIN, 12.0);
            fx.set_param(limiter::CEILING, -6.0);
        }
        EffectKind::Reverb | EffectKind::Chorus => {}
        // Closed on the test's -12 dB sine, and not all the way down, so the range is heard.
        EffectKind::Gate => {
            fx.set_param(gate::THRESHOLD, -6.0);
            fx.set_param(gate::RANGE, -20.0);
        }
        EffectKind::DeEsser => {
            fx.set_param(de_esser::FREQUENCY, 3000.0);
            fx.set_param(de_esser::THRESHOLD, -60.0);
            fx.set_param(de_esser::REDUCTION, 18.0);
        }
        EffectKind::Saturation => {
            fx.set_param(saturation::DRIVE, 24.0);
            fx.set_param(saturation::TONE, 4000.0);
        }
    }
    fx
}

/// Runs a low sine, applies `change` once in the middle, and gives the steepest step around
/// the change over the steepest step of the steady parts and the input.
fn click_ratio(
    fx: &mut dyn Effect,
    tempo_before: f32,
    change: &mut dyn FnMut(&mut dyn Effect) -> f32,
) -> f32 {
    let total = frames(SETTLE_S + MOVE_S + AFTER_S);
    let mut l = sine(110.0, 0.25, total);
    let mut r: Vec<f32> = sine(110.0, 0.25, total + 100).split_off(100);
    let input = steepest(&l);
    let at = frames(SETTLE_S) / BLOCK * BLOCK;
    run_at(fx, &mut l[..at], &mut r[..at], tempo_before);
    let tempo = change(fx);
    run_at(fx, &mut l[at..], &mut r[at..], tempo);

    let moved = at + frames(MOVE_S);
    let mut worst = 0.0_f32;
    for x in [&l, &r] {
        // The floor keeps the ratio meaningful when the output is silent before and after.
        let steady = steepest(&x[at - frames(0.5)..at])
            .max(steepest(&x[moved..]))
            .max(input * 0.05);
        worst = worst.max(steepest(&x[at - 1..moved + 1]) / steady);
    }
    worst
}

#[test]
fn the_click_meter_sees_a_jump() {
    struct Jump(f32);
    impl Effect for Jump {
        fn prepare(&mut self, _: f32, _: usize) {}
        fn set_param(&mut self, _: u32, value: f32) {
            self.0 = value;
        }
        fn process(&mut self, l: &mut [f32], r: &mut [f32], _: &meloet_engine::EffectContext) {
            l.iter_mut().chain(r).for_each(|v| *v *= self.0);
        }
        fn reset(&mut self) {}
    }
    let mut jump = Jump(1.0);
    let ratio = click_ratio(&mut jump, TEMPO, &mut |fx| {
        fx.set_param(0, 0.9);
        TEMPO
    });
    assert!(
        ratio > CLICK_FACTOR,
        "a 10 % jump must count as a click: {ratio}"
    );
}

#[test]
fn sudden_parameter_changes_do_not_click() {
    let mut worst_of_all = 0.0_f32;
    for kind in EffectKind::ALL {
        for (index, spec) in kind.params().iter().enumerate() {
            for (from, to) in [(spec.min, spec.max), (spec.max, spec.min)] {
                let mut fx = lively(kind);
                fx.set_param(index as u32, from);
                let ratio = click_ratio(fx.as_mut(), TEMPO, &mut |fx| {
                    fx.set_param(index as u32, to);
                    TEMPO
                });
                println!(
                    "{:10} {:14} {from:>8} -> {to:<8} {ratio:.2}",
                    kind.name(),
                    spec.name
                );
                assert!(
                    ratio <= CLICK_FACTOR,
                    "{} {} {from} -> {to}: {ratio}",
                    kind.name(),
                    spec.name
                );
                worst_of_all = worst_of_all.max(ratio);
            }
        }
    }
    println!("worst: {worst_of_all:.2}");
}

#[test]
fn a_tempo_change_does_not_click_the_delay() {
    for (from, to) in [(120.0, 30.0), (30.0, 300.0), (300.0, 119.0), (120.0, 121.0)] {
        let mut fx = prepared(EffectKind::Delay);
        fx.set_param(delay::FEEDBACK, 50.0);
        let ratio = click_ratio(fx.as_mut(), from, &mut |_| to);
        println!("tempo {from} -> {to}: {ratio:.2}");
        assert!(ratio <= CLICK_FACTOR, "tempo {from} -> {to}: {ratio}");
    }
}

#[test]
fn many_changes_at_once_do_not_click() {
    for kind in EffectKind::ALL {
        let mut fx = lively(kind);
        let ratio = click_ratio(fx.as_mut(), TEMPO, &mut |fx| {
            for (index, spec) in kind.params().iter().enumerate() {
                fx.set_param(index as u32, spec.max);
                fx.set_param(index as u32, spec.min);
            }
            90.0
        });
        println!("{:10} everything to the minimum: {ratio:.2}", kind.name());
        assert!(ratio <= CLICK_FACTOR, "{}: {ratio}", kind.name());
    }
}

// ---------------------------------------------------------------------------------------------
// 4. NaN, infinity and denormals.

#[test]
fn bad_numbers_give_one_silent_block_and_then_the_effect_is_back() {
    for kind in EffectKind::ALL {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut fx = prepared(kind);
            let mut l = sine(440.0, 0.25, BLOCK * 40);
            let mut r = l.clone();
            run(fx.as_mut(), &mut l, &mut r);

            let mut l = sine(440.0, 0.25, BLOCK);
            let mut r = l.clone();
            l[17] = bad;
            r[200] = bad;
            run(fx.as_mut(), &mut l, &mut r);
            assert!(
                l.iter().chain(&r).all(|v| *v == 0.0),
                "{} with {bad}: the block is silent",
                kind.name()
            );

            let mut l = sine(440.0, 0.25, frames(1.0));
            let mut r = l.clone();
            run(fx.as_mut(), &mut l, &mut r);
            assert!(
                l.iter().chain(&r).all(|v| v.is_finite()),
                "{} after {bad}: finite",
                kind.name()
            );
            let back = rms(&l[frames(0.75)..]);
            assert!(
                back > 1.0e-3,
                "{} after {bad}: sounds again ({back})",
                kind.name()
            );
        }
    }
}

#[test]
fn bad_parameter_values_are_ignored_or_brought_into_range() {
    for kind in EffectKind::ALL {
        let mut fx = prepared(kind);
        for index in 0..12 {
            fx.set_param(index, f32::NAN);
            fx.set_param(index, f32::INFINITY);
            fx.set_param(index, f32::NEG_INFINITY);
            fx.set_param(index, 1.0e30);
        }
        fx.set_param(u32::MAX, 1.0);
        let mut l = sine(440.0, 0.25, frames(0.5));
        let mut r = l.clone();
        run_at(fx.as_mut(), &mut l, &mut r, f32::NAN);
        run_at(fx.as_mut(), &mut l, &mut r, 0.0);
        run_at(fx.as_mut(), &mut l, &mut r, f32::INFINITY);
        assert!(l.iter().chain(&r).all(|v| v.is_finite()), "{}", kind.name());
    }
}

#[test]
fn denormals_stay_finite_and_small() {
    for kind in EffectKind::ALL {
        let mut fx = prepared(kind);
        let mut l = sine(440.0, 0.25, frames(0.5));
        let mut r = l.clone();
        run(fx.as_mut(), &mut l, &mut r);

        let mut l = vec![1.0e-40_f32; frames(8.0)];
        let mut r = vec![-1.0e-41_f32; frames(8.0)];
        run(fx.as_mut(), &mut l, &mut r);
        let last = frames(7.5);
        assert!(l.iter().chain(&r).all(|v| v.is_finite()), "{}", kind.name());
        let tail = peak(&l[last..]).max(peak(&r[last..]));
        assert!(tail < 1.0e-7, "{}: {tail:e}", kind.name());

        let mut l = sine(440.0, 0.25, frames(1.0));
        let mut r = l.clone();
        run(fx.as_mut(), &mut l, &mut r);
        assert!(rms(&l[frames(0.75)..]) > 1.0e-3, "{}", kind.name());
    }
}

#[test]
fn reset_and_prepare_keep_the_parameters_and_drop_the_sound() {
    for kind in EffectKind::ALL {
        let mut a = lively(kind);
        let mut b = lively(kind);
        let mut l = noise(0.5, frames(0.5), 14);
        let mut r = noise(0.5, frames(0.5), 15);
        run(b.as_mut(), &mut l, &mut r);
        b.reset();

        let (mut la, mut ra) = (noise(0.3, frames(0.5), 16), noise(0.3, frames(0.5), 17));
        let (mut lb, mut rb) = (la.clone(), ra.clone());
        run(a.as_mut(), &mut la, &mut ra);
        run(b.as_mut(), &mut lb, &mut rb);
        assert!(
            la == lb && ra == rb,
            "{}: a reset effect is like a new one",
            kind.name()
        );

        b.prepare(SR, BLOCK);
        let mut l = vec![0.0; BLOCK];
        let mut r = vec![0.0; BLOCK];
        run(b.as_mut(), &mut l, &mut r);
        assert_eq!(
            peak(&l).max(peak(&r)),
            0.0,
            "{}: silent after prepare",
            kind.name()
        );
    }
}

#[test]
fn other_sample_rates_and_block_sizes() {
    for kind in EffectKind::ALL {
        for rate in [44_100.0, 96_000.0, 192_000.0] {
            let mut fx = meloet_sound::effects::create(kind);
            fx.prepare(rate, 1024);
            let ctx = meloet_engine::EffectContext { tempo_bpm: 30.0 };
            let mut seen = 0.0_f32;
            for block in [1, 7, 64, 1024, 333, 1024, 1024, 1024, 1024] {
                for _ in 0..40 {
                    let mut l = noise(0.25, block, 18);
                    let mut r = noise(0.25, block, 19);
                    fx.process(&mut l, &mut r, &ctx);
                    assert!(l.iter().chain(&r).all(|v| v.is_finite()));
                    seen = seen.max(peak(&l));
                }
            }
            assert!(
                seen > 0.0 || kind == EffectKind::Delay,
                "{} at {rate}",
                kind.name()
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 5. Gate, de-esser and saturation.

fn sine_at(rate: f32, freq: f32, amp: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| {
            let phase = std::f64::consts::TAU * f64::from(freq) * i as f64 / f64::from(rate);
            (f64::from(amp) * phase.sin()) as f32
        })
        .collect()
}

/// A new effect at `rate`, set up by `setup`, run over `input` on both sides.
fn at_rate(
    kind: EffectKind,
    rate: f32,
    setup: &dyn Fn(&mut dyn Effect),
    input: &[f32],
) -> Vec<f32> {
    let mut fx = meloet_sound::effects::create(kind);
    fx.prepare(rate, 1024);
    setup(fx.as_mut());
    let ctx = meloet_engine::EffectContext { tempo_bpm: TEMPO };
    let mut l = input.to_vec();
    let mut r = input.to_vec();
    for (l, r) in l.chunks_mut(1024).zip(r.chunks_mut(1024)) {
        fx.process(l, r, &ctx);
    }
    l
}

/// How loud the `freq` part of `x` is (the height of that sine). `x` must hold whole cycles.
fn tone_level(x: &[f32], rate: f32, freq: f32) -> f32 {
    let (mut s, mut c) = (0.0_f64, 0.0_f64);
    for (i, v) in x.iter().enumerate() {
        let phase = std::f64::consts::TAU * f64::from(freq) * i as f64 / f64::from(rate);
        s += f64::from(*v) * phase.sin();
        c += f64::from(*v) * phase.cos();
    }
    (2.0 * (s * s + c * c).sqrt() / x.len() as f64) as f32
}

fn gate_with(threshold: f32, range: f32, release: f32) -> impl Fn(&mut dyn Effect) {
    move |fx| {
        fx.set_param(gate::THRESHOLD, threshold);
        fx.set_param(gate::RANGE, range);
        fx.set_param(gate::RELEASE, release);
    }
}

const LOUD_DB: f32 = -10.0;

/// A 1 kHz sine at -10 dB for `loud_s`, then at `after_db` to the end. At every rate used
/// here the change falls where the sine passes zero.
fn loud_then(rate: f32, loud_s: f32, after_db: f32, total_s: f32) -> Vec<f32> {
    let change = (loud_s * rate) as usize;
    let mut x = sine_at(rate, 1000.0, 1.0, (total_s * rate) as usize);
    for (i, v) in x.iter_mut().enumerate() {
        *v *= gain(if i < change { LOUD_DB } else { after_db });
    }
    x
}

/// The gain in dB over the millisecond that starts at `at_s`.
fn gain_at(rate: f32, input: &[f32], output: &[f32], at_s: f32) -> f32 {
    let from = (at_s * rate) as usize;
    let to = from + (rate / 1000.0) as usize;
    db(peak(&output[from..to]) / peak(&input[from..to]))
}

#[test]
fn gate_brings_down_what_is_under_the_threshold() {
    for range in [-60.0, -24.0, -6.0] {
        let input = sine(1000.0, gain(-40.0), frames(0.5));
        let out = at_rate(
            EffectKind::Gate,
            SR,
            &gate_with(-30.0, range, 120.0),
            &input,
        );
        let got = gain_at(SR, &input, &out, 0.4);
        assert!((got - range).abs() < 0.01, "range {range}: {got} dB");
        // From the first frame: a gate starts closed.
        let got = gain_at(SR, &input, &out, 0.0);
        assert!(
            (got - range).abs() < 0.01,
            "range {range} at the start: {got} dB"
        );
    }
    // Over the threshold: open within a millisecond, and then the very same samples.
    let input = sine(1000.0, gain(-20.0), frames(0.5));
    let out = at_rate(
        EffectKind::Gate,
        SR,
        &gate_with(-30.0, -60.0, 120.0),
        &input,
    );
    assert!(out[60..] == input[60..]);
    assert!(
        out[..48]
            .iter()
            .zip(&input)
            .all(|(o, i)| o.abs() <= i.abs())
    );
}

#[test]
fn gate_with_no_range_gives_the_very_same_samples() {
    let quiet_and_loud: Vec<f32> = noise(0.9, frames(1.0), 31)
        .iter()
        .enumerate()
        .map(|(i, v)| if i / 4800 % 2 == 0 { *v } else { v * 1.0e-4 })
        .collect();
    let out = at_rate(
        EffectKind::Gate,
        SR,
        &gate_with(-20.0, 0.0, 5.0),
        &quiet_and_loud,
    );
    assert!(out == quiet_and_loud);
}

#[test]
fn gate_opens_in_a_millisecond_with_no_delay() {
    let change = frames(0.1);
    let mut input = sine(1000.0, 1.0, frames(0.3));
    for (i, v) in input.iter_mut().enumerate() {
        *v *= gain(if i < change { -50.0 } else { LOUD_DB });
    }
    let out = at_rate(
        EffectKind::Gate,
        SR,
        &gate_with(-30.0, -60.0, 120.0),
        &input,
    );
    assert!((gain_at(SR, &input, &out, 0.09) + 60.0).abs() < 0.01);
    // The first frame over the threshold is the frame after the change; 48 frames later the
    // gate is open, and half way there the gain is half.
    assert!(out[change + 50..] == input[change + 50..]);
    let half = out[change + 25] / input[change + 25];
    assert!((half - 0.5).abs() < 0.05, "{half}");
    let first = out[change + 1] / input[change + 1];
    assert!(first > 0.02 && first < 0.03, "{first}");
}

#[test]
fn gate_holds_30_ms_and_then_falls_60_db_in_the_release_time() {
    let loud_s = 0.2;
    for (release, range, checks) in [
        // (seconds after the last loud frame, the gain there, how close)
        (
            120.0,
            -60.0,
            vec![(0.028, 0.0, 0.01), (0.09, -30.0, 1.0), (0.155, -60.0, 0.01)],
        ),
        (120.0, -20.0, vec![(0.028, 0.0, 0.01), (0.075, -20.0, 0.01)]),
        (5.0, -60.0, vec![(0.028, 0.0, 0.01), (0.037, -60.0, 0.01)]),
        (
            1000.0,
            -60.0,
            vec![(0.028, 0.0, 0.01), (0.53, -30.0, 0.2), (1.04, -60.0, 0.01)],
        ),
    ] {
        let input = loud_then(SR, loud_s, -50.0, 1.5);
        let out = at_rate(
            EffectKind::Gate,
            SR,
            &gate_with(-30.0, range, release),
            &input,
        );
        for (after, want, close) in checks {
            let got = gain_at(SR, &input, &out, loud_s + after);
            println!("gate release {release} range {range}: {got:.2} dB after {after} s");
            assert!(
                (got - want).abs() <= close,
                "release {release}, range {range}, {after} s after: {got} dB, want {want}"
            );
        }
    }
}

#[test]
fn gate_closes_3_db_under_where_it_opens() {
    let setup = gate_with(-30.0, -60.0, 5.0);
    // 1.5 dB under the threshold: not enough to open.
    let input = sine(1000.0, gain(-31.5), frames(0.5));
    let out = at_rate(EffectKind::Gate, SR, &setup, &input);
    assert!((gain_at(SR, &input, &out, 0.4) + 60.0).abs() < 0.01);
    // But enough to stay open.
    let input = loud_then(SR, 0.1, -31.5, 1.0);
    let out = at_rate(EffectKind::Gate, SR, &setup, &input);
    assert!(out[frames(0.1)..] == input[frames(0.1)..]);
    // 4 dB under is closed.
    let input = loud_then(SR, 0.1, -34.0, 1.0);
    let out = at_rate(EffectKind::Gate, SR, &setup, &input);
    assert!((gain_at(SR, &input, &out, 0.9) + 60.0).abs() < 0.01);
}

#[test]
fn gate_moves_both_sides_together() {
    let mut fx = prepared(EffectKind::Gate);
    gate_with(-30.0, -60.0, 120.0)(fx.as_mut());
    let mut l = sine(1000.0, gain(LOUD_DB), frames(0.5));
    let mut r = noise(1.0e-3, frames(0.5), 32);
    let in_r = r.clone();
    run(fx.as_mut(), &mut l, &mut r);
    assert!(r[60..] == in_r[60..]);
}

fn de_esser_with(frequency: f32, threshold: f32, reduction: f32) -> impl Fn(&mut dyn Effect) {
    move |fx| {
        fx.set_param(de_esser::FREQUENCY, frequency);
        fx.set_param(de_esser::THRESHOLD, threshold);
        fx.set_param(de_esser::REDUCTION, reduction);
    }
}

/// The gain in dB a steady sine gets, after half a second to settle.
fn settled_gain_db(
    kind: EffectKind,
    rate: f32,
    setup: &dyn Fn(&mut dyn Effect),
    input: &[f32],
) -> f32 {
    let out = at_rate(kind, rate, setup, input);
    let from = input.len() / 2;
    db(rms(&out[from..]) / rms(&input[from..]))
}

#[test]
fn de_esser_lowers_7_khz_and_leaves_500_hz_alone() {
    let defaults = |_: &mut dyn Effect| {};
    let hiss = sine(7000.0, gain(-10.0), frames(1.0));
    let got = settled_gain_db(EffectKind::DeEsser, SR, &defaults, &hiss);
    println!("de-esser, defaults, 7 kHz at -10 dB: {got:.2} dB");
    assert!(got < -3.0 && got > -8.0, "{got} dB");

    // Nothing over the threshold above the frequency: the very same samples.
    let body = sine(500.0, gain(-10.0), frames(1.0));
    let out = at_rate(EffectKind::DeEsser, SR, &defaults, &body);
    assert!(out == body);
}

#[test]
fn de_esser_takes_off_what_is_over_up_to_the_reduction() {
    // Well above the frequency the shelf is all the way down.
    for reduction in [3.0, 8.0, 18.0] {
        let input = sine(12000.0, gain(-6.0), frames(1.0));
        let setup = de_esser_with(3000.0, -40.0, reduction);
        let got = settled_gain_db(EffectKind::DeEsser, SR, &setup, &input);
        println!("de-esser reduction {reduction}: {got:.2} dB at 12 kHz");
        assert!(
            (got + reduction).abs() < 0.5,
            "reduction {reduction}: {got} dB"
        );
    }
    // 4 dB over the threshold: 4 dB off, though 8 are allowed.
    // (10 kHz: at 12 kHz there are four frames a cycle, and the frames can miss the tops.)
    let input = sine(10000.0, gain(-16.0), frames(1.0));
    let setup = de_esser_with(3000.0, -20.0, 8.0);
    let got = settled_gain_db(EffectKind::DeEsser, SR, &setup, &input);
    println!("de-esser 4 dB over: {got:.2} dB");
    assert!((got + 4.0).abs() < 0.5, "{got} dB");
    // Under the threshold: untouched.
    let input = sine(10000.0, gain(-25.0), frames(1.0));
    assert!(at_rate(EffectKind::DeEsser, SR, &setup, &input) == input);
    // At the frequency itself the shelf is half way down (in dB).
    let input = sine(4000.0, gain(-6.0), frames(1.0));
    let setup = de_esser_with(4000.0, -60.0, 12.0);
    let got = settled_gain_db(EffectKind::DeEsser, SR, &setup, &input);
    println!("de-esser at its frequency, 12 dB: {got:.2} dB");
    assert!((got + 6.0).abs() < 0.5, "{got} dB");
}

#[test]
fn de_esser_with_no_reduction_gives_the_very_same_samples() {
    let input = noise(0.9, frames(1.0), 33);
    let setup = de_esser_with(3000.0, -60.0, 0.0);
    assert!(at_rate(EffectKind::DeEsser, SR, &setup, &input) == input);
}

#[test]
fn de_esser_lowers_only_the_band_above_while_it_works() {
    let n = frames(1.0);
    let low = sine(300.0, gain(-6.0), n);
    let high = sine(9000.0, gain(-12.0), n);
    let input: Vec<f32> = low.iter().zip(&high).map(|(a, b)| a + b).collect();
    let setup = de_esser_with(5000.0, -40.0, 12.0);
    let out = at_rate(EffectKind::DeEsser, SR, &setup, &input);
    let from = n / 2;
    let low_db = db(tone_level(&out[from..], SR, 300.0) / tone_level(&input[from..], SR, 300.0));
    let high_db = db(tone_level(&out[from..], SR, 9000.0) / tone_level(&input[from..], SR, 9000.0));
    println!("de-esser at work: 300 Hz {low_db:.3} dB, 9 kHz {high_db:.2} dB");
    assert!(low_db.abs() < 0.05, "300 Hz: {low_db} dB");
    assert!(high_db < -8.0, "9 kHz: {high_db} dB");
}

#[test]
fn de_esser_moves_both_sides_together() {
    let mut fx = prepared(EffectKind::DeEsser);
    de_esser_with(4000.0, -40.0, 12.0)(fx.as_mut());
    let mut l = sine(12000.0, gain(-6.0), frames(1.0));
    let mut r = sine(12000.0, gain(-50.0), frames(1.0));
    let (in_l, in_r) = (l.clone(), r.clone());
    run(fx.as_mut(), &mut l, &mut r);
    let from = frames(0.5);
    let left = db(rms(&l[from..]) / rms(&in_l[from..]));
    let right = db(rms(&r[from..]) / rms(&in_r[from..]));
    assert!(left < -10.0, "{left}");
    assert!((left - right).abs() < 0.1, "{left} {right}");
}

/// A 110 Hz sine and an 8 kHz sine through `fx`, with `change` once in the middle. Gives, for
/// the time around the change: the steepest step of what the effect did under 1 kHz, over the
/// steepest step of the 110 Hz sine itself; and the largest move in one frame of the 8 kHz
/// part (its height and phase, as a fraction of the height it goes in with).
fn band_moves(fx: &mut dyn Effect, change: &mut dyn FnMut(&mut dyn Effect)) -> (f32, f32) {
    const HIGH: f32 = 0.1;
    let total = frames(SETTLE_S + MOVE_S + AFTER_S);
    let low = sine(110.0, 0.25, total);
    let mut l: Vec<f32> = (low.iter().zip(&sine(8000.0, HIGH, total)))
        .map(|(a, b)| a + b)
        .collect();
    let mut r = l.clone();
    let at = frames(SETTLE_S) / BLOCK * BLOCK;
    run(fx, &mut l[..at], &mut r[..at]);
    change(fx);
    run(fx, &mut l[at..], &mut r[at..]);
    let moved = at + frames(MOVE_S);

    let coef = 1.0 - (-std::f32::consts::TAU * 1000.0 / SR).exp();
    let under = |x: &[f32]| -> Vec<f32> {
        let mut state = [0.0_f32; 4];
        x.iter()
            .map(|v| {
                let mut x = *v;
                for s in &mut state {
                    *s += coef * (x - *s);
                    x = *s;
                }
                x
            })
            .collect()
    };
    let (under_out, under_in) = (under(&l), under(&low));
    let done: Vec<f32> = under_out
        .iter()
        .zip(&under_in)
        .map(|(a, b)| a - b)
        .collect();
    let low_ratio = steepest(&done[at - 1..moved + 1]) / steepest(&under_in);

    // 8 kHz is six frames a cycle: the sum over six frames against a cosine and a sine is the
    // part's height and phase, with nothing left of anything steady.
    let part = |n: usize| {
        let (mut i, mut q) = (0.0_f32, 0.0_f32);
        for at in n..n + 6 {
            let phase = std::f32::consts::TAU * (at % 6) as f32 / 6.0;
            i += (l[at] - low[at]) * phase.cos();
            q += (l[at] - low[at]) * phase.sin();
        }
        (i / 3.0, q / 3.0)
    };
    let mut high_step = 0.0_f32;
    for n in at - 1..moved {
        let (a, b) = (part(n), part(n + 1));
        high_step = high_step.max((b.0 - a.0).hypot(b.1 - a.1) / HIGH);
    }
    (low_ratio, high_step)
}

/// The fastest the de-esser's own detector moves the band: 18 dB in its 1 ms attack is 4.4 %
/// of the height in one frame. A parameter that jumps must not move it faster than that.
const DE_ESSER_STEP: f32 = 0.06;

#[test]
fn de_esser_does_not_click_while_parameters_jump() {
    let kind = EffectKind::DeEsser;
    for (index, spec) in kind.params().iter().enumerate() {
        for (from, to) in [(spec.min, spec.max), (spec.max, spec.min)] {
            let mut fx = lively(kind);
            fx.set_param(index as u32, from);
            let (low, high) = band_moves(fx.as_mut(), &mut |fx| fx.set_param(index as u32, to));
            println!(
                "deEsser {:10} {from:>8} -> {to:<8} low {low:.3} high {high:.4}",
                spec.name
            );
            assert!(low <= 0.1, "{} {from} -> {to}: low {low}", spec.name);
            assert!(
                high <= DE_ESSER_STEP,
                "{} {from} -> {to}: high {high}",
                spec.name
            );
        }
    }
    // The meter sees a gain that jumps: by half (the low part), by 18 dB (the high part).
    struct Jump(f32);
    impl Effect for Jump {
        fn prepare(&mut self, _: f32, _: usize) {}
        fn set_param(&mut self, _: u32, value: f32) {
            self.0 = value;
        }
        fn process(&mut self, l: &mut [f32], r: &mut [f32], _: &meloet_engine::EffectContext) {
            l.iter_mut().chain(r).for_each(|v| *v *= self.0);
        }
        fn reset(&mut self) {}
    }
    let (low, _) = band_moves(&mut Jump(1.0), &mut |fx| fx.set_param(0, 0.5));
    assert!(low > 0.1, "a jump must show under the band: {low}");
    let mut working = lively(kind);
    let (_, high) = band_moves(working.as_mut(), &mut |fx| fx.reset());
    assert!(high > DE_ESSER_STEP, "a jump must show in the band: {high}");
}

fn saturation_with(drive: f32, tone: f32, mix: f32, output: f32) -> impl Fn(&mut dyn Effect) {
    move |fx| {
        fx.set_param(saturation::DRIVE, drive);
        fx.set_param(saturation::TONE, tone);
        fx.set_param(saturation::MIX, mix);
        fx.set_param(saturation::OUTPUT, output);
    }
}

#[test]
fn saturation_with_no_drive_gives_the_very_same_samples() {
    let input = noise(0.95, frames(1.0), 34);
    for mix in [100.0, 50.0, 0.0] {
        let setup = saturation_with(0.0, 20000.0, mix, 0.0);
        assert!(
            at_rate(EffectKind::Saturation, SR, &setup, &input) == input,
            "mix {mix}"
        );
    }
    // And with no mix, whatever the rest is.
    let setup = saturation_with(36.0, 1000.0, 0.0, 0.0);
    assert!(at_rate(EffectKind::Saturation, SR, &setup, &input) == input);
}

#[test]
fn saturation_drive_lifts_quiet_sound_and_keeps_the_reference_level() {
    for drive in [1.0, 6.0, 12.0, 24.0, 36.0] {
        let setup = saturation_with(drive, 20000.0, 100.0, 0.0);
        // 60 dB under the reference level the curve is still straight.
        let reference = meloet_sound::effects::SATURATION_REFERENCE;
        let quiet = sine(1000.0, reference * gain(-60.0), frames(0.2));
        let got = settled_gain_db(EffectKind::Saturation, SR, &setup, &quiet);
        assert!((got - drive).abs() < 0.05, "drive {drive}: {got} dB");
        // A sample at the reference level (-18 dBFS) stays there.
        let input = sine(1000.0, reference, frames(0.2));
        let out = at_rate(EffectKind::Saturation, SR, &setup, &input);
        assert!(
            (peak(&out) / reference - 1.0).abs() < 1.0e-3,
            "drive {drive}"
        );
        // Nothing comes out above the curve's top, however loud it goes in.
        let hot = noise(4.0, frames(0.2), 35);
        let out = at_rate(EffectKind::Saturation, SR, &setup, &hot);
        let bend = f64::from(meloet_sound::effects::saturation_bend(gain(drive)));
        let top = reference / bend.tanh() as f32;
        assert!(
            peak(&out) <= top * 1.001,
            "drive {drive}: {} > {top}",
            peak(&out)
        );
    }
}

#[test]
fn saturation_drive_does_not_make_ordinary_sound_much_louder() {
    // Noise at -18 dB RMS, about where the built-in sounds sit.
    let input = noise(0.125 * 3.0_f32.sqrt(), frames(1.0), 36);
    for drive in [0.0, 3.0, 6.0, 12.0, 18.0, 24.0, 30.0, 36.0] {
        let setup = saturation_with(drive, 20000.0, 100.0, 0.0);
        let out = at_rate(EffectKind::Saturation, SR, &setup, &input);
        let change = db(rms(&out) / rms(&input));
        println!("saturation drive {drive:>4} dB: loudness {change:+.2} dB");
        assert!(change.abs() < 3.0, "drive {drive}: {change} dB");
    }
}

#[test]
fn saturation_tone_mix_and_output() {
    // The tone is a 12 dB/oct low-pass after the curve.
    let setup = saturation_with(0.0, 1000.0, 100.0, 0.0);
    let got = settled_gain_db(
        EffectKind::Saturation,
        SR,
        &setup,
        &sine(1000.0, 0.1, frames(0.5)),
    );
    assert!((got + 3.0).abs() < 0.2, "at the tone: {got} dB");
    let got = settled_gain_db(
        EffectKind::Saturation,
        SR,
        &setup,
        &sine(4000.0, 0.1, frames(0.5)),
    );
    assert!(
        (got + 24.6).abs() < 1.0,
        "two octaves above the tone: {got} dB"
    );
    let got = settled_gain_db(
        EffectKind::Saturation,
        SR,
        &setup,
        &sine(100.0, 0.1, frames(0.5)),
    );
    assert!(got.abs() < 0.05, "well under the tone: {got} dB");
    // It takes the curve's overtones off: a driven 2 kHz sine keeps little above the tone.
    let input = sine(2000.0, 0.25, frames(0.5));
    let level = |tone: f32| {
        let out = at_rate(
            EffectKind::Saturation,
            SR,
            &saturation_with(30.0, tone, 100.0, 0.0),
            &input,
        );
        tone_level(&out[frames(0.25)..], SR, 6000.0)
    };
    let (bright, dark) = (level(20000.0), level(2000.0));
    assert!(bright > 0.02, "the third overtone is there: {bright}");
    assert!(db(dark / bright) < -15.0, "{dark} {bright}");

    // The mix is a straight line between the dry and the wet sound.
    let input = noise(0.5, frames(0.5), 37);
    let wet = at_rate(
        EffectKind::Saturation,
        SR,
        &saturation_with(24.0, 3000.0, 100.0, 0.0),
        &input,
    );
    let half = at_rate(
        EffectKind::Saturation,
        SR,
        &saturation_with(24.0, 3000.0, 50.0, 0.0),
        &input,
    );
    let worst = (half.iter().zip(&wet).zip(&input))
        .fold(0.0_f32, |m, ((h, w), d)| m.max((h - 0.5 * (w + d)).abs()));
    assert!(worst < 1.0e-6, "{worst:e}");

    // The output gain is the last step.
    for output in [-24.0, -12.0, 6.0] {
        let setup = saturation_with(0.0, 20000.0, 100.0, output);
        let got = settled_gain_db(EffectKind::Saturation, SR, &setup, &input);
        assert!((got - output).abs() < 0.01, "output {output}: {got} dB");
    }
}

#[test]
fn the_new_effects_add_no_delay() {
    for kind in [
        EffectKind::Gate,
        EffectKind::DeEsser,
        EffectKind::Saturation,
    ] {
        let mut fx = lively(kind);
        if kind == EffectKind::Gate {
            fx.set_param(gate::THRESHOLD, -45.0);
        }
        assert_eq!(fx.latency(), 0);
        let mut l = vec![0.0; BLOCK];
        l[0] = 0.5;
        let mut r = l.clone();
        run(fx.as_mut(), &mut l, &mut r);
        assert!(l[0] != 0.0 && r[0] != 0.0, "{}", kind.name());
    }
}

#[test]
fn the_new_effects_do_the_same_at_every_rate() {
    let rates = [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0];
    for rate in rates {
        // The gate: half way down (in dB) half a release after the 30 ms.
        let n = |seconds: f32| (seconds * rate) as usize;
        let input = loud_then(rate, 0.1, -50.0, 0.4);
        let out = at_rate(
            EffectKind::Gate,
            rate,
            &gate_with(-30.0, -60.0, 100.0),
            &input,
        );
        let got = gain_at(rate, &input, &out, 0.1 + 0.03 + 0.05);
        assert!((got + 30.0).abs() < 1.0, "gate at {rate}: {got} dB");
        assert!(
            (gain_at(rate, &input, &out, 0.128)).abs() < 0.01,
            "gate hold at {rate}"
        );
        assert!((gain_at(rate, &input, &out, 0.24) + 60.0).abs() < 0.01);

        // The de-esser: the same cut, in and above its band.
        let hiss = sine_at(rate, 7000.0, gain(-10.0), n(1.0));
        let got = settled_gain_db(EffectKind::DeEsser, rate, &|_| {}, &hiss);
        let at_48k = settled_gain_db(
            EffectKind::DeEsser,
            SR,
            &|_| {},
            &sine(7000.0, gain(-10.0), frames(1.0)),
        );
        println!("de-esser at {rate}: {got:.2} dB (48 kHz: {at_48k:.2})");
        assert!((got - at_48k).abs() < 0.5, "de-esser at {rate}: {got} dB");
        let body = sine_at(rate, 500.0, gain(-10.0), n(0.5));
        assert!(at_rate(EffectKind::DeEsser, rate, &|_| {}, &body) == body);

        // The saturation: the same lift, the same tone.
        let quiet = sine_at(rate, 1000.0, 0.001, n(0.2));
        let setup = saturation_with(12.0, 20000.0, 100.0, 0.0);
        let got = settled_gain_db(EffectKind::Saturation, rate, &setup, &quiet);
        assert!((got - 12.0).abs() < 0.05, "saturation at {rate}: {got} dB");
        let setup = saturation_with(0.0, 2000.0, 100.0, 0.0);
        let tone = sine_at(rate, 4000.0, 0.1, n(0.5));
        let got = settled_gain_db(EffectKind::Saturation, rate, &setup, &tone);
        assert!(
            (got + 12.4).abs() < 0.5,
            "saturation tone at {rate}: {got} dB"
        );
        let input = noise(0.9, n(0.2), 38);
        let setup = saturation_with(0.0, 20000.0, 100.0, 0.0);
        assert!(at_rate(EffectKind::Saturation, rate, &setup, &input) == input);
    }
}

#[test]
fn the_new_effects_do_the_same_in_every_block_size() {
    // Loud and quiet in turns, with a bright part, so all three have work to do.
    let n = frames(1.0);
    let bright = sine(8000.0, 0.2, n);
    let input: Vec<f32> = noise(0.3, n, 39)
        .iter()
        .zip(&bright)
        .enumerate()
        .map(|(i, (v, b))| if i / 7000 % 2 == 0 { v + b } else { v * 1.0e-3 })
        .collect();
    let ctx = meloet_engine::EffectContext { tempo_bpm: TEMPO };
    for kind in [
        EffectKind::Gate,
        EffectKind::DeEsser,
        EffectKind::Saturation,
    ] {
        let render = |sizes: &[usize]| {
            let mut fx = meloet_sound::effects::create(kind);
            fx.prepare(SR, 2048);
            match kind {
                EffectKind::Gate => gate_with(-20.0, -40.0, 50.0)(fx.as_mut()),
                EffectKind::DeEsser => de_esser_with(5000.0, -40.0, 12.0)(fx.as_mut()),
                _ => saturation_with(18.0, 5000.0, 80.0, -3.0)(fx.as_mut()),
            }
            let (mut l, mut r) = (input.clone(), input.clone());
            let (mut at, mut turn) = (0, 0);
            while at < n {
                let len = sizes[turn % sizes.len()].min(n - at);
                fx.process(&mut l[at..at + len], &mut r[at..at + len], &ctx);
                at += len;
                turn += 1;
            }
            l
        };
        let whole = render(&[2048]);
        assert!(whole != input, "{}: at work", kind.name());
        for sizes in [&[32][..], &[256], &[1, 7, 64, 333, 1024, 2048]] {
            assert!(
                render(sizes) == whole,
                "{} in blocks of {sizes:?}",
                kind.name()
            );
        }
    }
}
