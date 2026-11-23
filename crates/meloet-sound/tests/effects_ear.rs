//! Short dry and wet WAV files of the gate, the de-esser and the saturation, to judge by ear:
//! `cargo test -p meloet-sound --test effects_ear -- --ignored --nocapture [folder]`
//! (the folder comes from MELOET_EAR_DIR, else `target/effects-ear` under the repository).
//! The material is made here: the built-in drum kit and synths, noise, and a voice-like buzz.

mod support;

use std::path::{Path, PathBuf};

use meloet_engine::{Effect, Instrument, InstrumentEvent};
use meloet_model::effects::{EffectKind, de_esser, gate, saturation};
use meloet_sound::synth::{ElectroKit, PolySynth};
use support::*;

type Stereo = (Vec<f32>, Vec<f32>);

/// Plays `notes` (start in seconds, length in seconds, key, velocity) on `inst`.
fn play(inst: &mut dyn Instrument, notes: &[(f32, f32, u8, u8)], seconds: f32) -> Stereo {
    inst.prepare(SR, BLOCK);
    let mut events: Vec<(usize, InstrumentEvent)> = Vec::new();
    for &(at, len, note, velocity) in notes {
        events.push((frames(at), InstrumentEvent::NoteOn { note, velocity }));
        events.push((frames(at + len), InstrumentEvent::NoteOff { note }));
    }
    events.sort_by_key(|(at, _)| *at);
    let total = frames(seconds) / BLOCK * BLOCK;
    let (mut left, mut right) = (vec![0.0; total], vec![0.0; total]);
    let mut next = 0;
    for (block, (l, r)) in left
        .chunks_mut(BLOCK)
        .zip(right.chunks_mut(BLOCK))
        .enumerate()
    {
        while next < events.len() && events[next].0 < (block + 1) * BLOCK {
            inst.event(events[next].1);
            next += 1;
        }
        inst.render(l, r);
    }
    (left, right)
}

fn wet(kind: EffectKind, values: &[(u32, f32)], dry: &Stereo) -> Stereo {
    let mut fx: Box<dyn Effect> = prepared(kind);
    for &(index, value) in values {
        fx.set_param(index, value);
    }
    let (mut l, mut r) = dry.clone();
    run(fx.as_mut(), &mut l, &mut r);
    (l, r)
}

fn write(dir: &Path, name: &str, (left, right): &Stereo) {
    let data_bytes = (left.len() * 4) as u32;
    let mut bytes = Vec::with_capacity(44 + data_bytes as usize);
    bytes.extend(b"RIFF");
    bytes.extend((36 + data_bytes).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend((SR as u32).to_le_bytes());
    bytes.extend((SR as u32 * 4).to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data_bytes.to_le_bytes());
    for (l, r) in left.iter().zip(right) {
        for v in [l, r] {
            bytes.extend(((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
    }
    std::fs::write(dir.join(name), bytes).unwrap();
    println!(
        "{name}: {:.1} s, peak {:.1} dB, rms {:.1} dB",
        left.len() as f32 / SR,
        db(peak(left).max(peak(right))),
        db(rms(left))
    );
}

/// A drum pattern with a steady hiss and hum under it, as from a noisy input.
fn drums_with_hiss() -> Stereo {
    let mut notes = Vec::new();
    for bar in 0..2 {
        let at = bar as f32 * 2.0;
        notes.extend([
            (at, 0.1, 36, 110),
            (at + 0.5, 0.1, 38, 100),
            (at + 1.0, 0.1, 36, 110),
            (at + 1.25, 0.1, 36, 90),
            (at + 1.5, 0.1, 38, 100),
        ]);
    }
    let (mut l, mut r) = play(&mut ElectroKit::new(), &notes, 5.0);
    let hiss_l = noise(gain(-50.0), l.len(), 41);
    let hiss_r = noise(gain(-50.0), l.len(), 42);
    let hum = sine(100.0, gain(-56.0), l.len());
    for i in 0..l.len() {
        l[i] += hiss_l[i] + hum[i];
        r[i] += hiss_r[i] + hum[i];
    }
    (l, r)
}

/// Not a voice, but shaped like one for this purpose: a buzz at 140 Hz through two
/// resonances ("ah"), with bursts of bright noise ("s") before and between.
fn buzz_with_s() -> Stereo {
    let n = frames(5.0);
    let mut out = vec![0.0_f32; n];
    let breath = noise(1.0, n, 43);
    let pole = |freq: f32, width: f32| {
        let r = (-std::f32::consts::PI * width / SR).exp();
        (2.0 * r * (std::f32::consts::TAU * freq / SR).cos(), -r * r)
    };
    let formants = [pole(700.0, 110.0), pole(1200.0, 130.0), pole(2600.0, 200.0)];
    let mut state = [[0.0_f32; 2]; 3];
    let (mut high, mut last) = (0.0_f32, 0.0_f32);
    let mut phase = 0.0_f32;
    for (i, v) in out.iter_mut().enumerate() {
        let t = i as f32 / SR % 1.25;
        // 0.00-0.18 s "s", 0.20-0.95 s "ah", 1.00-1.15 s "s".
        let fade = |from: f32, to: f32| {
            ((t - from) * 60.0).clamp(0.0, 1.0) * ((to - t) * 60.0).clamp(0.0, 1.0)
        };
        let s = fade(0.0, 0.18) + fade(1.0, 1.15);
        let ah = fade(0.2, 0.95);
        phase = (phase + 140.0 / SR) % 1.0;
        let buzz = (2.0 * phase - 1.0) * 0.02 * ah;
        let mut voice = 0.0;
        for ((a1, a2), st) in formants.iter().zip(&mut state) {
            let y = buzz + a1 * st[0] + a2 * st[1];
            st[1] = st[0];
            st[0] = y;
            voice += y;
        }
        // The "s": noise with its low part taken off twice (a 6 kHz one-pole high-pass).
        let coef = 1.0 - (-std::f32::consts::TAU * 6000.0 / SR).exp();
        high += coef * (breath[i] - high);
        let bright = breath[i] - high;
        let brighter = bright - 0.5 * last;
        last = bright;
        *v = 0.25 * voice + 0.9 * s * brighter;
    }
    let top = peak(&out);
    for v in &mut out {
        *v *= 0.5 / top;
    }
    (out.clone(), out)
}

/// A bass line and a chord of the built-in synths.
fn bass_and_pad() -> Stereo {
    let mut bass = Vec::new();
    for (step, key) in [36, 36, 43, 36, 39, 39, 46, 41].into_iter().enumerate() {
        bass.push((step as f32 * 0.5, 0.4, key, 105));
    }
    let (mut l, mut r) = play(&mut PolySynth::bass(), &bass, 5.0);
    let chord: Vec<_> = [60, 63, 67, 70]
        .into_iter()
        .map(|key| (0.0, 3.8, key, 85))
        .collect();
    let (pl, pr) = play(&mut PolySynth::pad(), &chord, 5.0);
    for i in 0..l.len() {
        l[i] = 0.7 * l[i] + 0.5 * pl[i];
        r[i] = 0.7 * r[i] + 0.5 * pr[i];
    }
    (l, r)
}

#[test]
#[ignore = "writes files to listen to, not a check"]
fn write_the_ear_demos() {
    let dir = std::env::var_os("MELOET_EAR_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/effects-ear"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();

    let dry = drums_with_hiss();
    write(&dir, "gate-dry.wav", &dry);
    write(
        &dir,
        "gate-wet.wav",
        &wet(
            EffectKind::Gate,
            &[
                (gate::THRESHOLD, -38.0),
                (gate::RANGE, -60.0),
                (gate::RELEASE, 120.0),
            ],
            &dry,
        ),
    );
    write(
        &dir,
        "gate-wet-range-12.wav",
        &wet(
            EffectKind::Gate,
            &[
                (gate::THRESHOLD, -38.0),
                (gate::RANGE, -12.0),
                (gate::RELEASE, 300.0),
            ],
            &dry,
        ),
    );

    let dry = buzz_with_s();
    write(&dir, "deesser-dry.wav", &dry);
    write(
        &dir,
        "deesser-wet-default.wav",
        &wet(EffectKind::DeEsser, &[], &dry),
    );
    write(
        &dir,
        "deesser-wet-strong.wav",
        &wet(
            EffectKind::DeEsser,
            &[
                (de_esser::FREQUENCY, 5000.0),
                (de_esser::THRESHOLD, -40.0),
                (de_esser::REDUCTION, 18.0),
            ],
            &dry,
        ),
    );

    let dry = bass_and_pad();
    write(&dir, "saturation-dry.wav", &dry);
    write(
        &dir,
        "saturation-wet-default.wav",
        &wet(EffectKind::Saturation, &[], &dry),
    );
    write(
        &dir,
        "saturation-wet-warm.wav",
        &wet(
            EffectKind::Saturation,
            &[(saturation::DRIVE, 12.0), (saturation::TONE, 6000.0)],
            &dry,
        ),
    );
    write(
        &dir,
        "saturation-wet-hard.wav",
        &wet(EffectKind::Saturation, &[(saturation::DRIVE, 30.0)], &dry),
    );
    println!("written to {}", dir.display());
}
