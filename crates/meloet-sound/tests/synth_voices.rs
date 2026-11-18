//! The computed instruments: nothing allocated on the audio thread, no NaN, silence after the
//! last note, loudness, pedal, bend, wheel, the hi-hat group.

// The engine's counting allocator, shared instead of copied.
#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;

use meloet_engine::{Instrument, InstrumentEvent};
use meloet_model::drums::DrumPiece;
use meloet_sound::synth::{ElectroKit, PolySynth, SineSynth};

const RATE: f32 = 48_000.0;
const BLOCK: usize = 256;

fn all() -> Vec<(&'static str, Box<dyn Instrument>)> {
    let mut list: Vec<(&'static str, Box<dyn Instrument>)> = vec![
        ("lead", Box::new(PolySynth::lead())),
        ("bass", Box::new(PolySynth::bass())),
        ("pad", Box::new(PolySynth::pad())),
        ("sine", Box::new(SineSynth::new())),
        ("kit", Box::new(ElectroKit::new())),
    ];
    for (_, inst) in list.iter_mut() {
        inst.prepare(RATE, BLOCK);
    }
    list
}

fn pitched() -> Vec<(&'static str, PolySynth)> {
    let mut list = vec![
        ("lead", PolySynth::lead()),
        ("bass", PolySynth::bass()),
        ("pad", PolySynth::pad()),
    ];
    for (_, inst) in list.iter_mut() {
        inst.prepare(RATE, BLOCK);
    }
    list
}

fn on(note: u8, velocity: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOn { note, velocity }
}

fn off(note: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOff { note }
}

fn cc(controller: u8, value: u8) -> InstrumentEvent {
    InstrumentEvent::Control { controller, value }
}

/// Renders `seconds` in blocks; returns the left and right channels.
fn run(inst: &mut dyn Instrument, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    let total = (seconds * RATE) as usize;
    let (mut left, mut right) = (vec![0.0; total], vec![0.0; total]);
    for (l, r) in left.chunks_mut(BLOCK).zip(right.chunks_mut(BLOCK)) {
        inst.render(l, r);
    }
    (left, right)
}

fn peak(ch: &(Vec<f32>, Vec<f32>)) -> f32 {
    let top = |s: &[f32]| s.iter().fold(0.0_f32, |m, x| m.max(x.abs()));
    top(&ch.0).max(top(&ch.1))
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

fn db(x: f32) -> f32 {
    20.0 * x.max(1.0e-9).log10()
}

struct Lcg(u32);

impl Lcg {
    fn next(&mut self, below: u32) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) % below
    }
}

/// Keys, pedal, wheel, bend, all-notes-off and reset in no sensible order, with more keys down
/// than any instrument has voices. `check` sees every rendered block.
fn hammer(inst: &mut dyn Instrument, blocks: usize, mut check: impl FnMut(&[f32], &[f32])) {
    let mut rng = Lcg(7);
    let (mut left, mut right) = ([0.0_f32; BLOCK], [0.0_f32; BLOCK]);
    for block in 0..blocks {
        for _ in 0..rng.next(6) {
            let ev = match rng.next(12) {
                0..=4 => on(24 + rng.next(80) as u8, 1 + rng.next(127) as u8),
                5 => on(
                    DrumPiece::ROWS[rng.next(12) as usize].note(),
                    1 + rng.next(127) as u8,
                ),
                6 | 7 => off(24 + rng.next(80) as u8),
                8 => cc(64, rng.next(128) as u8),
                9 => cc(1, rng.next(128) as u8),
                10 => InstrumentEvent::PitchBend {
                    value: rng.next(16_384) as i16 - 8192,
                },
                _ => InstrumentEvent::AllNotesOff,
            };
            inst.event(ev);
        }
        if block % 97 == 96 {
            inst.reset();
        }
        let frames = 1 + rng.next(BLOCK as u32) as usize;
        inst.render(&mut left[..frames], &mut right[..frames]);
        check(&left[..frames], &right[..frames]);
    }
}

#[test]
fn nothing_is_allocated_or_freed_on_the_audio_thread() {
    for (name, mut inst) in all() {
        let before = common::audio_block_counts();
        {
            let _block = meloet_engine::rt::enter();
            hammer(inst.as_mut(), 3000, |_, _| {});
        }
        assert_eq!(common::audio_block_counts(), before, "{name}");
    }
}

#[test]
fn output_is_always_a_number_and_stays_in_reach() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for (name, mut inst) in all() {
            inst.prepare(rate, BLOCK);
            let mut loudest = 0.0_f32;
            hammer(inst.as_mut(), 3000, |l, r| {
                for x in l.iter().chain(r) {
                    assert!(x.is_finite(), "{name} at {rate}");
                    loudest = loudest.max(x.abs());
                }
            });
            assert!(loudest > 0.01, "{name} at {rate} made no sound");
            assert!(loudest < 8.0, "{name} at {rate}: {loudest}");
        }
    }
}

#[test]
fn silence_comes_back_after_the_last_note() {
    for (name, mut inst) in all() {
        for note in [36, 38, 46, 49, 51, 60, 64] {
            inst.event(on(note, 127));
        }
        assert!(peak(&run(inst.as_mut(), 0.5)) > 0.01, "{name}");
        for note in [36, 38, 46, 49, 51, 60, 64] {
            inst.event(off(note));
        }
        run(inst.as_mut(), 10.0);
        assert_eq!(peak(&run(inst.as_mut(), 0.1)), 0.0, "{name}");
    }
}

#[test]
fn reset_is_silent_at_once_and_forgets_pedal_and_bend() {
    for (name, mut inst) in all() {
        inst.event(cc(64, 127));
        inst.event(cc(1, 127));
        inst.event(InstrumentEvent::PitchBend { value: 8191 });
        for note in [36, 38, 46, 60, 64, 67] {
            inst.event(on(note, 127));
        }
        run(inst.as_mut(), 0.3);
        inst.reset();
        assert_eq!(peak(&run(inst.as_mut(), 0.05)), 0.0, "{name}");
        if name == "kit" {
            continue;
        }

        let mut fresh = all().into_iter().find(|(n, _)| *n == name).unwrap().1;
        fresh.event(on(57, 100));
        let want = pitch(&run(fresh.as_mut(), 0.8).0[28_800..]);
        inst.event(on(57, 100));
        let got = pitch(&run(inst.as_mut(), 0.8).0[28_800..]);
        assert!((1200.0 * (got / want).log2()).abs() < 25.0, "{name}");
        inst.event(off(57));
        run(inst.as_mut(), 6.0);
        assert_eq!(peak(&run(inst.as_mut(), 0.1)), 0.0, "{name}");
    }
}

#[test]
fn a_released_key_fades_instead_of_stopping_dead() {
    for (name, mut inst) in pitched() {
        inst.event(on(60, 100));
        let held = run(&mut inst, 0.6);
        inst.event(off(60));
        let after = run(&mut inst, 0.004);
        let before = rms(&held.0[held.0.len() - 2400..]);
        assert!(rms(&after.0) > 0.3 * before, "{name}");
    }
}

#[test]
fn a_taken_voice_fades_before_the_new_note_starts() {
    for (name, mut inst) in pitched() {
        // Low notes through a low-pass filter move slowly, so a cut-off voice would show as a jump.
        for note in 0..40 {
            inst.event(on(24 + note % 20, 110));
            let out = run(&mut inst, 0.011);
            let jump = out
                .0
                .windows(2)
                .fold(0.0_f32, |m, w| m.max((w[1] - w[0]).abs()));
            assert!(jump < 0.25, "{name}: step of {jump} at note {note}");
        }
    }
}

#[test]
fn the_pedal_holds_released_notes() {
    for (name, mut inst) in pitched() {
        inst.event(cc(64, 127));
        inst.event(on(60, 100));
        run(&mut inst, 0.5);
        inst.event(off(60));
        let held = run(&mut inst, 1.0);
        assert!(rms(&held.0[24_000..]) > 0.01, "{name}");
        inst.event(cc(64, 0));
        run(&mut inst, 6.0);
        assert_eq!(peak(&run(&mut inst, 0.1)), 0.0, "{name}");
    }
}

/// The pitch as the strongest period by autocorrelation, in Hz.
fn pitch(s: &[f32]) -> f32 {
    let best = (60..600)
        .max_by(|&a, &b| {
            let score = |lag: usize| -> f32 {
                s[..s.len() - lag]
                    .iter()
                    .zip(&s[lag..])
                    .map(|(x, y)| x * y)
                    .sum()
            };
            score(a).total_cmp(&score(b))
        })
        .unwrap();
    RATE / best as f32
}

#[test]
fn bend_moves_the_pitch_two_semitones_each_way() {
    for (name, mut inst) in pitched() {
        let mut heard = [0.0; 3];
        for (slot, value) in heard.iter_mut().zip([-8192, 0, 8191]) {
            inst.reset();
            inst.event(InstrumentEvent::PitchBend { value });
            inst.event(on(57, 100));
            let out = run(&mut inst, 0.8);
            *slot = pitch(&out.0[28_800..]);
        }
        // The bass has an oscillator an octave down, so its strongest period is 110 Hz.
        let centre = if name == "bass" { 110.0 } else { 220.0 };
        let cents = |f: f32| 1200.0 * (f / centre).log2();
        assert!((cents(heard[0]) + 200.0).abs() < 25.0, "{name}: {heard:?}");
        assert!(cents(heard[1]).abs() < 25.0, "{name}: {heard:?}");
        assert!((cents(heard[2]) - 200.0).abs() < 25.0, "{name}: {heard:?}");
    }
}

#[test]
fn the_wheel_changes_the_sound() {
    for (name, mut inst) in pitched() {
        inst.event(on(48, 100));
        let plain = run(&mut inst, 1.0);
        inst.reset();
        inst.event(cc(1, 127));
        inst.event(on(48, 100));
        let moved = run(&mut inst, 1.0);
        // Brightness as the size of the sample-to-sample steps against the size of the samples.
        let edge = |s: &[f32]| {
            let steps: Vec<f32> = s.windows(2).map(|w| w[1] - w[0]).collect();
            rms(&steps) / rms(s)
        };
        let (a, b) = (edge(&plain.0[24_000..]), edge(&moved.0[24_000..]));
        if name == "lead" {
            // Vibrato: the pitch moves, the tone colour stays.
            let diff: Vec<f32> = plain.0.iter().zip(&moved.0).map(|(x, y)| x - y).collect();
            assert!(rms(&diff[24_000..]) > 0.01, "{name}");
        } else {
            assert!(b > 1.3 * a, "{name}: {a} {b}");
        }
    }
}

#[test]
fn the_kit_plays_only_its_twelve_keys() {
    let mut kit = ElectroKit::new();
    kit.prepare(RATE, BLOCK);
    for note in 0..=127 {
        kit.reset();
        kit.event(on(note, 100));
        let loud = peak(&run(&mut kit, 0.1));
        match DrumPiece::from_note(note) {
            Some(piece) => assert!(loud > 0.02, "{}: {loud}", piece.id()),
            None => assert_eq!(loud, 0.0, "key {note}"),
        }
    }
}

#[test]
fn the_kit_ignores_key_up_pedal_and_bend() {
    let mut kit = ElectroKit::new();
    kit.prepare(RATE, BLOCK);
    kit.event(on(49, 100));
    let want = run(&mut kit, 0.5);
    kit.reset();
    kit.event(on(49, 100));
    kit.event(off(49));
    kit.event(cc(64, 127));
    kit.event(InstrumentEvent::PitchBend { value: 4000 });
    kit.event(InstrumentEvent::AllNotesOff);
    let got = run(&mut kit, 0.5);
    // The noise in a cymbal is not the same twice; the loudness is.
    assert!((db(rms(&want.0)) - db(rms(&got.0))).abs() < 1.0);
}

#[test]
fn closed_and_pedal_hi_hat_stop_the_open_one() {
    let mut kit = ElectroKit::new();
    kit.prepare(RATE, BLOCK);
    let tail = |kit: &mut ElectroKit, then: Option<u8>| {
        kit.reset();
        kit.event(on(46, 110));
        run(kit, 0.05);
        if let Some(note) = then {
            kit.event(on(note, 1));
        }
        run(kit, 0.1);
        rms(&run(kit, 0.1).0)
    };
    let ringing = tail(&mut kit, None);
    assert!(ringing > 0.001);
    assert!(tail(&mut kit, Some(42)) < 0.05 * ringing);
    assert!(tail(&mut kit, Some(44)) < 0.05 * ringing);
    // A snare is not in the group.
    assert!(tail(&mut kit, Some(38)) > 0.9 * ringing);
}

#[test]
fn a_harder_hit_is_louder_and_not_just_a_copy() {
    let mut kit = ElectroKit::new();
    kit.prepare(RATE, BLOCK);
    for piece in DrumPiece::ROWS {
        let mut hit = |velocity| {
            kit.reset();
            kit.event(on(piece.note(), velocity));
            run(&mut kit, 0.3)
        };
        let (soft, hard) = (hit(40), hit(120));
        assert!(peak(&hard) > 2.0 * peak(&soft), "{}", piece.id());
    }
    for (name, mut inst) in pitched() {
        let mut play = |velocity| {
            inst.reset();
            inst.event(on(60, velocity));
            run(&mut inst, 0.5)
        };
        let (soft, hard) = (play(40), play(120));
        assert!(peak(&hard) > 2.0 * peak(&soft), "{name}");
    }
}

#[test]
fn quick_hits_on_one_drum_do_not_pile_up() {
    let mut kit = ElectroKit::new();
    kit.prepare(RATE, BLOCK);
    kit.event(on(36, 127));
    let one = peak(&run(&mut kit, 0.5));
    kit.reset();
    let mut loudest = 0.0_f32;
    for _ in 0..40 {
        kit.event(on(36, 127));
        loudest = loudest.max(peak(&run(&mut kit, 0.013)));
    }
    assert!(loudest < 1.3 * one, "{loudest} against {one}");
}

/// One key (C4, or the kick) at velocity 100 peaks near -12 dBFS; four keys at velocity 127 stay
/// under full scale. Run with `--nocapture` to see the numbers.
#[test]
fn loudness_is_matched() {
    let chord = [60, 64, 67, 72];
    let mut rows = Vec::new();
    for (name, mut inst) in all() {
        let key = if name == "kit" { 36 } else { 60 };
        inst.event(on(key, 100));
        let one = peak(&run(inst.as_mut(), 2.0));
        // The pad and the lead start somewhere else in the wave every time: take the worst of many.
        let mut four = 0.0_f32;
        for _ in 0..40 {
            inst.reset();
            for note in if name == "kit" {
                [36, 38, 42, 49]
            } else {
                chord
            } {
                inst.event(on(note, 127));
            }
            four = four.max(peak(&run(inst.as_mut(), 2.0)));
        }
        rows.push((name, one, four));
    }
    println!("instrument   one key v100   four keys v127");
    for (name, one, four) in &rows {
        println!("{name:<10} {:>8.1} dBFS {:>10.1} dBFS", db(*one), db(*four));
    }
    for (name, one, four) in rows {
        let want = match name {
            "bass" | "kit" => Some(-12.0),
            "lead" | "pad" => Some(-14.5),
            _ => None,
        };
        if let Some(want) = want {
            assert!((db(one) - want).abs() < 0.5, "{name}: {} dBFS", db(one));
        }
        assert!(four < 1.0, "{name}: {} dBFS", db(four));
    }
}

/// Prints the peak of every drum at velocity 100 (`--nocapture`); the levels in kit.rs were set
/// from it.
#[test]
fn drum_levels() {
    let mut kit = ElectroKit::new();
    kit.prepare(RATE, BLOCK);
    let mut peaks = Vec::new();
    for piece in DrumPiece::ROWS {
        let mut worst = 0.0_f32;
        let mut power = 0.0;
        for _ in 0..20 {
            kit.reset();
            kit.event(on(piece.note(), 100));
            let out = run(&mut kit, 0.25);
            worst = worst.max(peak(&out));
            power = rms(&out.0).max(power);
        }
        println!(
            "{:<12} peak {:>6.1} dBFS   rms of 250 ms {:>6.1} dBFS",
            piece.id(),
            db(worst),
            db(power)
        );
        peaks.push((piece, db(worst)));
    }
    for (piece, top) in peaks {
        assert!((-24.0..-10.0).contains(&top), "{}: {top}", piece.id());
    }
}

/// `cargo test --release -p meloet-sound --test synth_voices -- --ignored --nocapture`
#[test]
#[ignore = "timing; run in release on a quiet machine"]
fn speed_with_sixteen_keys() {
    let mut list = all();
    println!("16 keys held, 48 kHz, blocks of {BLOCK} frames (5.33 ms)");
    for (name, inst) in list.iter_mut() {
        inst.prepare(RATE, BLOCK);
        let (mut left, mut right) = ([0.0_f32; BLOCK], [0.0_f32; BLOCK]);
        let mut times = Vec::new();
        for round in 0..2000 {
            // Drums are one-shots: hit all twelve again and again so they keep sounding.
            if *name == "kit" && round % 8 == 0 {
                for piece in DrumPiece::ROWS {
                    inst.event(on(piece.note(), 100));
                }
            } else if round == 0 {
                for i in 0..16 {
                    inst.event(on(36 + 3 * i, 100));
                }
            }
            let start = std::time::Instant::now();
            inst.render(&mut left, &mut right);
            times.push(start.elapsed().as_secs_f64() * 1.0e6);
            std::hint::black_box((&left, &right));
        }
        let times = &mut times[200..];
        times.sort_by(f64::total_cmp);
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        println!(
            "{name:<12} mean {mean:>6.1} us   median {:>6.1} us   worst {:>6.1} us   ({:.2} % of a block)",
            times[times.len() / 2],
            times[times.len() - 1],
            mean / (BLOCK as f64 / f64::from(RATE) * 1.0e6) * 100.0
        );
    }
}
