//! The knobs of the sounds that come with meloet (round 12, E3): at the values they start at
//! nothing changes to the sample, and each one does what its name says.

mod support;

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use meloet_engine::{Instrument, InstrumentEvent, KNOB_DEFAULTS, SongPlace};
use meloet_sound::knobs::Knobbed;
use meloet_sound::synth::{ElectroKit, PolySynth, SineSynth};
use support::pack::{Storm, TestPack, everything, off, on, play, region, tone};

const RATE: f32 = 48_000.0;
const BLOCK: usize = 256;
const BRIGHTNESS: u32 = 0;
const ATTACK: u32 = 1;
const RELEASE: u32 = 2;
const VIBRATO: u32 = 3;

/// The same instrument as it is and with its knobs around it.
fn made(name: &str, pack: &TestPack, knobbed: bool) -> Box<dyn Instrument> {
    fn wrap<I: Instrument + 'static>(i: I, knobbed: bool) -> Box<dyn Instrument> {
        if knobbed {
            Box::new(Knobbed::new(i))
        } else {
            Box::new(i)
        }
    }
    let mut inst = match name {
        "lead" => wrap(PolySynth::lead(), knobbed),
        "bass" => wrap(PolySynth::bass(), knobbed),
        "pad" => wrap(PolySynth::pad(), knobbed),
        "sine" => wrap(SineSynth::new(), knobbed),
        "kit" => wrap(ElectroKit::new(), knobbed),
        _ => wrap(pack.sampler(RATE, BLOCK), knobbed),
    };
    inst.prepare(RATE, BLOCK);
    inst
}

const ALL: [&str; 6] = ["lead", "bass", "pad", "sine", "kit", "samples"];

/// Notes, pedal, bend, wheel and presses of the song, in blocks of changing length; with
/// `knobs`, the knobs are set to where they start, by hand and by lane, now and then.
fn busy(inst: &mut dyn Instrument, knobs: bool) -> Vec<u32> {
    let mut storm = Storm::new(11);
    let (mut l, mut r) = ([0.0_f32; BLOCK], [0.0_f32; BLOCK]);
    let mut bits = Vec::new();
    for block in 0..1500_usize {
        for _ in 0..block % 3 {
            inst.event(storm.next());
        }
        if block % 5 == 0 {
            let note = [36, 38, 42, 60, 64, 96][block % 6];
            inst.song_note_on(note, 90, SongPlace::new(block as u32, &[]));
        }
        if knobs && block % 7 == 0 {
            for (knob, &place) in KNOB_DEFAULTS.iter().enumerate() {
                inst.set_knob(knob as u32, place);
                inst.set_param_at((block % 100) as u32, knob as u32, place);
            }
        }
        let n = [256, 1, 64, 200, 33][block % 5];
        inst.render(&mut l[..n], &mut r[..n]);
        bits.extend(l[..n].iter().chain(&r[..n]).map(|x| x.to_bits()));
    }
    bits
}

#[test]
fn at_the_values_they_start_at_the_knobs_change_nothing() {
    let pack = everything("knobs-same");
    for name in ALL {
        let mut bare = made(name, &pack, false);
        let mut with = made(name, &pack, true);
        let first = (busy(bare.as_mut(), false), busy(with.as_mut(), true));
        assert!(
            first.0.iter().any(|&b| f32::from_bits(b).abs() > 0.01),
            "{name}"
        );
        let at = first.0.iter().zip(&first.1).position(|(a, b)| a != b);
        assert_eq!(at, None, "{name}: the first sample that differs");
    }
}

/// `frames` of one note held for `held` frames, with the knobs at `knobs` (percent / 100).
fn note(name: &str, key: u8, knobs: [f32; 4], held: usize, frames: usize) -> Vec<f32> {
    static PACKS: AtomicU32 = AtomicU32::new(0);
    let pack = one_shots(&format!("knobs-note-{}", PACKS.fetch_add(1, Relaxed)));
    let mut inst = made(name, &pack, true);
    for (knob, place) in knobs.into_iter().enumerate() {
        inst.set_knob(knob as u32, place);
    }
    inst.event(on(key, 110));
    let (mut left, _) = play(inst.as_mut(), held, BLOCK);
    inst.event(off(key));
    left.extend(play(inst.as_mut(), frames - held, BLOCK).0);
    left
}

/// A pack of one long one-shot, as a drum's sample is.
fn one_shots(name: &str) -> TestPack {
    let pack = TestPack::new(name);
    let fall: Vec<f32> = (tone(200.0, 48_000, 48_000, 0.5).into_iter().enumerate())
        .map(|(i, x)| x * (-(i as f32) / 24_000.0).exp())
        .collect();
    pack.wav("hit", 48_000, &[&fall]);
    pack.instrument("", &[region("hit", 0, 127, 60, "\"oneShot\": true")]);
    pack
}

fn knobs(brightness: f32, attack: f32, release: f32, vibrato: f32) -> [f32; 4] {
    [brightness, attack, release, vibrato]
}

const START: [f32; 4] = KNOB_DEFAULTS;

fn energy(x: &[f32]) -> f64 {
    x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum()
}

/// What changes fast: the energy of the step from frame to frame.
fn bright_energy(x: &[f32]) -> f64 {
    x.windows(2).map(|w| f64::from(w[1] - w[0]).powi(2)).sum()
}

#[test]
fn brightness_closes_a_low_pass_and_lifts_the_top() {
    let second = RATE as usize;
    let at = |b: f32| {
        let left = note("lead", 60, knobs(b, 0.5, 0.5, 0.0), second, second);
        bright_energy(&left[4800..])
    };
    let (closed, middle) = (at(0.0), at(0.5));
    assert!(closed < middle * 0.1, "{closed} {middle}");
    let high = |b: f32| energy(&note("sine", 120, knobs(b, 0.5, 0.5, 0.0), second, second)[4800..]);
    let lifted = 10.0 * (high(1.0) / high(0.5)).log10();
    assert!(
        (6.0..9.5).contains(&lifted),
        "8.4 kHz, an octave over the shelf: {lifted} dB"
    );
    let low = |b: f32| energy(&note("sine", 48, knobs(b, 0.5, 0.5, 0.0), second, second)[4800..]);
    let ratio = low(1.0) / low(0.5);
    assert!(
        (0.8..1.25).contains(&ratio),
        "the shelf leaves a low C as it is: {ratio}"
    );
}

/// The first frame from which every 10 ms window stays below `below` of the loudest.
fn falls_by(x: &[f32], below: f32) -> usize {
    let win = 480;
    let rms: Vec<f32> = x
        .chunks(win)
        .map(|c| (energy(c) / c.len() as f64).sqrt() as f32)
        .collect();
    let top = rms.iter().fold(0.0_f32, |m, v| m.max(*v));
    let last = rms.iter().rposition(|&v| v > top * below).unwrap_or(0);
    (last + 1) * win
}

#[test]
fn attack_and_release_follow_their_knobs() {
    let second = RATE as usize;
    let early = |a: f32| energy(&note("sine", 69, knobs(0.5, a, 0.5, 0.0), second, second)[..2400]);
    assert!(early(1.0) < early(0.5) * 0.1, "+400 ms to the attack");
    assert!(early(0.0) >= early(0.5), "no attack at all");
    let tail = |r: f32| {
        let left = note("lead", 60, knobs(0.5, 0.5, r, 0.0), second / 2, 2 * second);
        falls_by(&left, 0.01)
    };
    let (short, middle, long) = (tail(0.0), tail(0.5), tail(1.0));
    assert!(short < middle && middle < long, "{short} {middle} {long}");
    assert!(
        long - second / 2 > 3 * (middle - second / 2),
        "{long} {middle}"
    );
}

/// Frames between rising zero crossings, in order.
fn periods(x: &[f32]) -> Vec<f32> {
    let ups: Vec<f32> = (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| i as f32 - x[i] / (x[i] - x[i - 1]))
        .collect();
    ups.windows(2).map(|w| w[1] - w[0]).collect()
}

#[test]
fn vibrato_wavers_from_the_start_of_each_note() {
    let second = RATE as usize;
    let cents = |v: f32| {
        let left = note("sine", 69, knobs(0.5, 0.5, 0.5, v), second, second);
        let base = RATE / 440.0;
        let p: Vec<f32> = (periods(&left[..second]).into_iter())
            .map(|q| 1200.0 * (base / q).log2())
            .collect();
        // A quarter of a turn of 5.5 Hz is 20 turns of 440 Hz.
        let quarter = p[18..23].iter().sum::<f32>() / 5.0;
        let widest = p.iter().fold(0.0_f32, |m, &c| m.max(c.abs()));
        (p[0], quarter, widest)
    };
    let (_, _, none) = cents(0.0);
    assert!(none < 1.0, "{none}");
    let (first, quarter, widest) = cents(1.0);
    assert!(
        first.abs() < 10.0,
        "it starts on the note and goes up: {first}"
    );
    assert!(
        (35.0..55.0).contains(&quarter),
        "the top a quarter turn on: {quarter}"
    );
    assert!((40.0..60.0).contains(&widest), "50 cents deep: {widest}");
}

#[test]
fn a_drum_rings_shorter_or_longer() {
    let second = RATE as usize;
    let snare = 38;
    let tail = |name: &str, key: u8, r: f32| {
        let left = note(name, key, knobs(0.5, 0.5, r, 0.0), 480, second);
        energy(&left[second / 10..])
    };
    let (short, middle, long) = (
        tail("kit", snare, 0.0),
        tail("kit", snare, 0.5),
        tail("kit", snare, 1.0),
    );
    assert!(
        short < middle * 0.5 && long > middle * 1.5,
        "{short} {middle} {long}"
    );
    let (short, middle, long) = (
        tail("samples", 60, 0.0),
        tail("samples", 60, 0.5),
        tail("samples", 60, 1.0),
    );
    assert!(
        short < middle * 0.2,
        "a one-shot falls faster: {short} {middle}"
    );
    assert_eq!(long, middle, "and cannot ring longer than it was recorded");
    let attack = note("kit", snare, knobs(0.5, 1.0, 0.5, 1.0), 480, second);
    assert_eq!(
        attack,
        note("kit", snare, START, 480, second),
        "a drum has no attack or vibrato knob"
    );
}

#[test]
fn the_filter_leaves_nothing_behind_and_takes_no_number_that_is_not_one() {
    let pack = everything("knobs-tail");
    let mut inst = made("lead", &pack, true);
    inst.set_knob(BRIGHTNESS, 0.0);
    inst.set_knob(VIBRATO, 0.8);
    inst.event(on(60, 127));
    play(inst.as_mut(), 9600, BLOCK);
    inst.set_knob(BRIGHTNESS, f32::NAN);
    inst.set_knob(ATTACK, f32::NAN);
    inst.set_param_at(0, RELEASE, f32::NAN);
    inst.event(InstrumentEvent::AllNotesOff);
    let (left, right) = play(inst.as_mut(), 5 * RATE as usize, BLOCK);
    assert!(left.iter().chain(&right).all(|x| x.is_finite()));
    let end = left.len() - RATE as usize;
    assert!(left[end..].iter().chain(&right[end..]).all(|&x| x == 0.0));
    inst.set_knob(BRIGHTNESS, 1.0);
    let (left, _) = play(inst.as_mut(), 4800, BLOCK);
    assert!(
        left.iter().all(|&x| x == 0.0),
        "silence through the shelf is silence"
    );
}
