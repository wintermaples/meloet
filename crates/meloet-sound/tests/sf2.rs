//! The SoundFont reader and the parts of the player it uses, on little files made here: the
//! zone rules, the errors, broken files, envelopes, LFOs, filter, loops, classes, controllers.

mod support;

use meloet_engine::{Instrument, InstrumentEvent};
use meloet_sound::sampler::Sampler;
use meloet_sound::sf2::{Sf2Error, Sf2File};
use support::pack::{Storm, off, on, play};
use support::sf2::*;

const RATE: f32 = 48_000.0;
const BLOCK: usize = 256;

fn control(controller: u8, value: u8) -> InstrumentEvent {
    InstrumentEvent::Control { controller, value }
}

fn db(ratio: f32) -> f32 {
    20.0 * ratio.max(1.0e-9).log10()
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn at(seconds: f32) -> usize {
    (seconds * RATE) as usize
}

/// Times of the upward zero crossings, each placed between its two frames.
fn crossings(x: &[f32]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| (i - 1) as f64 + f64::from(-x[i - 1]) / f64::from(x[i] - x[i - 1]))
        .collect()
}

fn frequency(x: &[f32]) -> f64 {
    let times = crossings(x);
    (times.len() - 1) as f64 * f64::from(RATE) / (times[times.len() - 1] - times[0])
}

fn cents(got: f64, want: f64) -> f64 {
    1200.0 * (got / want).log2()
}

/// A looped file of one value, so that what comes out is the loudness.
fn flat_sample() -> Smp {
    Smp::new("flat", flat(16_384, 4800), 48_000, 60)
}

/// One instrument of `zones` on preset 0:0, with the volume controller fully up.
fn player(name: &str, samples: Vec<Smp>, zones: Vec<Zone>) -> (TempFile, Sampler) {
    let file = TempFile::new(name, &Font::simple(samples, zones).bytes());
    let mut sampler = file.sampler(RATE, BLOCK);
    sampler.event(control(7, 127));
    // The controller moves smoothly; let it arrive.
    play(&mut sampler, at(0.5), BLOCK);
    (file, sampler)
}

fn generator(file: &Sf2File, zone: usize, name: &str) -> i32 {
    let zones = file.zones(0, 0).unwrap();
    zones[zone]
        .generators
        .iter()
        .find(|g| g.0 == name)
        .unwrap()
        .1
}

#[test]
fn zones_follow_the_rules_of_the_two_levels() {
    let inst = Inst {
        name: "inst",
        zones: vec![
            vec![(ATTENUATION, 100), (PAN, 50)],
            vec![(KEY_RANGE, range(0, 63)), (PAN, -200), (SAMPLE_ID, 0)],
            vec![
                (KEY_RANGE, range(64, 127)),
                (VEL_RANGE, range(20, 90)),
                (SAMPLE_ID, 0),
            ],
            // No sampleID: ignored.
            vec![(KEY_RANGE, range(0, 127)), (PAN, 400)],
        ],
    };
    let preset = Preset {
        name: "Two levels",
        bank: 0,
        program: 0,
        zones: vec![
            vec![(FINE_TUNE, 5), (ATTENUATION, 7)],
            vec![
                (KEY_RANGE, range(10, 100)),
                (VEL_RANGE, range(50, 127)),
                (ATTENUATION, 30),
                // Means nothing in a preset.
                (SAMPLE_MODES, 1),
                (ROOT_KEY, 40),
                (INSTRUMENT, 0),
            ],
            // Outside the instrument's keys: plays nothing.
            vec![
                (KEY_RANGE, range(120, 127)),
                (VEL_RANGE, range(0, 10)),
                (INSTRUMENT, 0),
            ],
        ],
    };
    let font = Font::new(vec![flat_sample()], vec![inst], vec![preset]);
    let file = TempFile::new("levels", &font.bytes());
    let sf = file.open();
    let zones = sf.zones(0, 0).unwrap();
    assert_eq!(zones.len(), 2);
    assert_eq!((zones[0].keys, zones[0].velocities), ((10, 63), (50, 127)));
    assert_eq!((zones[1].keys, zones[1].velocities), ((64, 100), (50, 90)));
    assert_eq!(zones[0].instrument, "inst");
    assert_eq!(zones[0].sample, "flat");
    // Local over global at each level, then preset added to instrument.
    assert_eq!(generator(&sf, 0, "initialAttenuation"), 130);
    assert_eq!(generator(&sf, 0, "pan"), -200);
    assert_eq!(generator(&sf, 1, "pan"), 50);
    assert_eq!(generator(&sf, 0, "fineTune"), 5);
    assert_eq!(generator(&sf, 0, "sampleModes"), 0);
    assert_eq!(generator(&sf, 0, "overridingRootKey"), -1);
    // Defaults.
    assert_eq!(generator(&sf, 0, "initialFilterFc"), 13_500);
    assert_eq!(generator(&sf, 0, "decayVolEnv"), -12_000);
    assert_eq!(generator(&sf, 0, "scaleTuning"), 100);
    assert_eq!(sf.load(0, 0, RATE).unwrap().region_count(), 2);
}

#[test]
fn presets_come_by_bank_and_program_and_the_first_of_two_counts() {
    let preset = |name, bank, program, inst| Preset {
        name,
        bank,
        program,
        zones: vec![vec![(INSTRUMENT, inst)]],
    };
    let insts = vec![
        Inst {
            name: "one",
            zones: vec![vec![(SAMPLE_ID, 0)]],
        },
        Inst {
            name: "two",
            zones: vec![
                vec![(KEY_RANGE, range(0, 60)), (SAMPLE_ID, 0)],
                vec![(SAMPLE_ID, 0)],
            ],
        },
    ];
    let presets = vec![
        preset("Kit", 128, 0, 0),
        preset("Second", 0, 5, 0),
        preset("First", 0, 1, 0),
        preset("Second again", 0, 5, 1),
    ];
    let file = TempFile::new(
        "list",
        &Font::new(vec![flat_sample()], insts, presets).bytes(),
    );
    let sf = file.open();
    let list: Vec<_> = sf
        .presets()
        .into_iter()
        .map(|p| (p.bank, p.program, p.name))
        .collect();
    let want: [(u16, u16, &str); 3] = [(0, 1, "First"), (0, 5, "Second"), (128, 0, "Kit")];
    assert_eq!(list.len(), want.len());
    for (got, want) in list.iter().zip(want) {
        assert_eq!((got.0, got.1, got.2.as_str()), want);
    }
    assert_eq!(sf.load(0, 5, RATE).unwrap().region_count(), 1);
    assert_eq!(sf.load(0, 5, RATE).unwrap().kind(), "Second");
    assert_eq!(
        sf.load(1, 1, RATE).err(),
        Some(Sf2Error::NoPreset {
            bank: 1,
            program: 1
        })
    );
}

#[test]
fn what_cannot_be_read_gets_its_own_error() {
    let open = |name: &str, bytes: &[u8]| Sf2File::open(&TempFile::new(name, bytes).0).err();
    let font = || Font::simple(vec![flat_sample()], vec![vec![(SAMPLE_ID, 0)]]);

    let mut version_3 = font();
    version_3.version = (3, 1);
    assert_eq!(open("v3", &version_3.bytes()), Some(Sf2Error::Sf3));
    let mut compressed = font();
    compressed.samples[0].kind = 0x11;
    assert_eq!(open("flag", &compressed.bytes()), Some(Sf2Error::Sf3));
    let mut ogg = font();
    ogg.samples[0].data[..2].copy_from_slice(&[0x674F, 0x5367]);
    assert_eq!(open("ogg", &ogg.bytes()), Some(Sf2Error::Sf3));

    let mut version_1 = font();
    version_1.version = (1, 0);
    assert!(matches!(
        open("v1", &version_1.bytes()),
        Some(Sf2Error::NotSf2(_))
    ));
    assert!(matches!(
        open("text", b"not a sound font at all"),
        Some(Sf2Error::NotSf2(_))
    ));
    assert!(matches!(open("empty", b""), Some(Sf2Error::NotSf2(_))));
    let missing = std::path::Path::new("/nonexistent/meloet.sf2");
    assert!(matches!(
        Sf2File::open(missing).err(),
        Some(Sf2Error::Io(_))
    ));

    // A ROM sample, and a zone cut to nothing: the preset is there but plays nothing.
    let mut rom = font();
    rom.samples[0].kind = 0x8001;
    let file = TempFile::new("rom", &rom.bytes());
    let none = Some(Sf2Error::NoSound {
        bank: 0,
        program: 0,
    });
    assert_eq!(file.open().load(0, 0, RATE).err(), none);
    let cut = Font::simple(
        vec![flat_sample()],
        vec![
            vec![(START_OFFSET, 4799), (SAMPLE_ID, 0)],
            vec![(SAMPLE_ID, 9)],
        ],
    );
    let file = TempFile::new("cut", &cut.bytes());
    assert_eq!(file.open().load(0, 0, RATE).err(), none);
}

/// Opens the bytes as a file and, when that works, lists, loads and plays what is there.
fn survive(name: &str, bytes: &[u8]) -> bool {
    let file = TempFile::new(name, bytes);
    let Ok(sf) = Sf2File::open(&file.0) else {
        return false;
    };
    let mut played = false;
    for preset in sf.presets().iter().take(4) {
        let _ = sf.zones(preset.bank, preset.program);
        let Ok(bank) = sf.load(preset.bank, preset.program, RATE) else {
            continue;
        };
        let mut sampler = Sampler::new(bank.into());
        sampler.prepare(RATE, BLOCK);
        for key in [0, 36, 60, 61, 72, 96, 98, 127] {
            sampler.event(on(key, 100));
            let (left, right) = play(&mut sampler, 2 * BLOCK, BLOCK);
            sampler.event(off(key));
            assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        }
        played = true;
    }
    played
}

#[test]
fn broken_files_never_stop_the_program() {
    let good = rich().bytes();
    assert!(survive("good", &good));
    let pdta = good.windows(4).rposition(|w| w == b"pdta").unwrap();
    let mut seed = 0x2545_F491_u32;
    let mut random = |below: usize| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as usize % below
    };
    let (mut opened, mut played) = (0, 0);
    for round in 0..600 {
        let mut bytes = good.clone();
        match round % 6 {
            // Cut anywhere.
            0 => bytes.truncate(random(good.len())),
            // A few bytes anywhere.
            1 => {
                for _ in 0..1 + random(8) {
                    bytes[random(good.len())] = random(256) as u8;
                }
            }
            // A few bytes in the tables.
            2 | 3 => {
                for _ in 0..1 + random(6) {
                    bytes[pdta + random(good.len() - pdta)] = random(256) as u8;
                }
            }
            // Numbers in the tables turned into the biggest or into zero.
            4 => {
                for _ in 0..1 + random(4) {
                    let place = pdta + random(good.len() - pdta - 4);
                    let value = [0xFF, 0x00, 0x7F, 0x80][random(4)];
                    let width = [2, 4][random(2)];
                    bytes[place..place + width].fill(value);
                }
            }
            // Noise after a header that looks right.
            _ => {
                let keep = [12, 24, pdta + 4, pdta + 12][random(4)];
                for b in &mut bytes[keep..] {
                    *b = random(256) as u8;
                }
            }
        }
        if Sf2File::open(&TempFile::new("fuzz-open", &bytes).0).is_ok() {
            opened += 1;
        }
        played += usize::from(survive("fuzz", &bytes));
    }
    // The broken files did get past the first checks, into the tables and the player.
    assert!(opened > 200 && played > 100, "{opened} {played}");
}

#[test]
fn the_volume_envelope_has_six_stages() {
    let zone = vec![
        (DELAY_VOL_ENV, timecents(0.1)),
        (ATTACK_VOL_ENV, timecents(0.1)),
        (HOLD_VOL_ENV, timecents(0.1)),
        (DECAY_VOL_ENV, timecents(1.0)),
        (SUSTAIN_VOL_ENV, 240),
        (RELEASE_VOL_ENV, timecents(0.5)),
        (SAMPLE_MODES, 1),
        (SAMPLE_ID, 0),
    ];
    let (_file, mut sampler) = player("six", vec![flat_sample()], vec![zone]);
    sampler.event(on(60, 127));
    let (held, _) = play(&mut sampler, at(1.0), BLOCK);
    sampler.event(off(60));
    let (released, _) = play(&mut sampler, at(0.5), BLOCK);

    let full = held[at(0.25)];
    assert!(full > 0.1, "{full}");
    assert_eq!(held[at(0.05)], 0.0);
    // The attack is a straight line in amplitude.
    assert!((held[at(0.15)] / full - 0.5).abs() < 0.02);
    assert!((held[at(0.175)] / full - 0.75).abs() < 0.02);
    // The decay is a straight line in dB, 96 dB in the decay time, down to the sustain level.
    assert!((db(held[at(0.3 + 0.125)] / full) + 12.0).abs() < 0.5);
    assert!((db(held[at(0.3 + 0.2)] / full) + 19.2).abs() < 0.5);
    assert!((db(held[at(0.8)] / full) + 24.0).abs() < 0.2);
    assert!((db(held[at(0.99)] / full) + 24.0).abs() < 0.2);
    // The release too, 96 dB in the release time, from where the envelope was.
    assert!((db(released[at(0.125)] / full) + 48.0).abs() < 1.0);
    assert!((db(released[at(0.25)] / full) + 72.0).abs() < 1.0);
    assert_eq!(released[at(0.4)], 0.0);
    assert_eq!(sampler.active_voices(), 0);
}

#[test]
fn a_key_let_go_in_the_attack_falls_from_where_it_was() {
    let zone = vec![
        (ATTACK_VOL_ENV, timecents(0.2)),
        (RELEASE_VOL_ENV, timecents(0.96)),
        (SAMPLE_MODES, 1),
        (SAMPLE_ID, 0),
    ];
    let (_file, mut sampler) = player("early", vec![flat_sample()], vec![zone]);
    sampler.event(on(60, 127));
    let (rising, _) = play(&mut sampler, at(0.1), BLOCK);
    sampler.event(off(60));
    let (falling, _) = play(&mut sampler, at(0.3), BLOCK);
    let top = rising[rising.len() - 1];
    // 100 dB a second from half loudness; no jump at the turn.
    assert!((falling[0] - top).abs() < top * 0.02);
    assert!((db(falling[at(0.1)] / top) + 10.0).abs() < 0.5);
    assert!(falling.windows(2).all(|w| w[1] <= w[0] * 1.0001));
}

#[test]
fn higher_keys_hold_and_decay_shorter() {
    let zone = vec![
        (HOLD_VOL_ENV, timecents(0.1)),
        (KEY_TO_VOL_ENV_HOLD, 100),
        (DECAY_VOL_ENV, timecents(1.0)),
        (KEY_TO_VOL_ENV_DECAY, 100),
        (SUSTAIN_VOL_ENV, 1440),
        (SCALE_TUNING, 0),
        (SAMPLE_MODES, 1),
        (SAMPLE_ID, 0),
    ];
    let (_file, mut sampler) = player("keyed", vec![flat_sample()], vec![zone]);
    let mut fall_at = |key: u8, seconds: f32| {
        sampler.reset();
        sampler.event(on(key, 127));
        let (out, _) = play(&mut sampler, at(seconds) + 1, BLOCK);
        db(out[at(seconds)] / out[at(0.01)])
    };
    // Key 60: hold 0.1 s, then 96 dB a second. Key 72: half the hold, twice the rate.
    assert!((fall_at(60, 0.35) + 24.0).abs() < 0.5);
    assert!((fall_at(72, 0.3) + 48.0).abs() < 0.5);
    assert!((fall_at(48, 0.45) + 12.0).abs() < 0.5);
}

#[test]
fn attenuation_velocity_and_controllers_set_the_loudness() {
    let zones = vec![
        vec![(SAMPLE_MODES, 1)],
        vec![(KEY_RANGE, range(0, 60)), (SAMPLE_ID, 0)],
        vec![
            (KEY_RANGE, range(61, 61)),
            (ATTENUATION, 100),
            (SAMPLE_ID, 0),
        ],
        vec![(KEY_RANGE, range(62, 62)), (PAN, -500), (SAMPLE_ID, 0)],
        vec![(KEY_RANGE, range(63, 63)), (PAN, 250), (SAMPLE_ID, 0)],
    ];
    let (_file, mut sampler) = player("loud", vec![flat_sample()], zones);
    let mut level = |events: &[InstrumentEvent]| {
        sampler.reset();
        sampler.event(control(7, 127));
        for ev in events {
            sampler.event(*ev);
        }
        let (left, right) = play(&mut sampler, at(0.4), BLOCK);
        (left[at(0.39)], right[at(0.39)])
    };
    let (full, right) = level(&[on(60, 127)]);
    assert!(full > 0.1 && (full - right).abs() < 1.0e-6);
    let close = |got: f32, want: f32| (got / want - 1.0).abs() < 0.01;

    // 0.4 of the file's attenuation, as FluidSynth plays it: 100 cB is 4 dB.
    assert!(close(
        level(&[on(61, 127)]).0,
        full * 10.0_f32.powf(-4.0 / 20.0)
    ));
    // Velocity, volume and expression each as (value / 127)^2.
    assert!(close(
        level(&[on(60, 64)]).0,
        full * (64.0_f32 / 127.0).powi(2)
    ));
    assert!(close(
        level(&[control(7, 64), on(60, 127)]).0,
        full * (64.0_f32 / 127.0).powi(2)
    ));
    assert!(close(
        level(&[control(11, 90), on(60, 127)]).0,
        full * (90.0_f32 / 127.0).powi(2)
    ));
    // Equal power: the middle is 3 dB down on each side.
    let (left, right) = level(&[on(62, 127)]);
    assert!(close(left, full * std::f32::consts::SQRT_2) && right.abs() < 1.0e-6);
    let (left, right) = level(&[on(63, 127)]);
    let angle = 0.75 * std::f32::consts::FRAC_PI_2;
    assert!(close(left / right, angle.cos() / angle.sin()));
    // Controller 10 is added to the zone's place and stops at the sides.
    let (left, right) = level(&[control(10, 127), on(60, 127)]);
    assert!(left.abs() < 1.0e-6 && close(right, full * std::f32::consts::SQRT_2));
    let (left, right) = level(&[control(10, 0), on(63, 127)]);
    assert!(close(
        right / left,
        (0.25 * std::f32::consts::FRAC_PI_2).tan()
    ));

    // Without the volume controller moved, it is at 100.
    sampler.reset();
    sampler.event(on(60, 127));
    let (left, _) = play(&mut sampler, BLOCK, BLOCK);
    assert!(close(left[100], full * (100.0_f32 / 127.0).powi(2)));
}

#[test]
fn controllers_move_without_a_jump() {
    let zone = vec![(SAMPLE_MODES, 1), (SAMPLE_ID, 0)];
    let (_file, mut sampler) = player("smooth", vec![flat_sample()], vec![zone]);
    sampler.event(on(60, 127));
    play(&mut sampler, BLOCK, BLOCK);
    sampler.event(control(7, 0));
    sampler.event(control(10, 127));
    let (left, right) = play(&mut sampler, at(0.3), BLOCK);
    let step = |x: &[f32]| {
        x.windows(2)
            .fold(0.0_f32, |m, w| m.max((w[1] - w[0]).abs()))
    };
    assert!(step(&left) < 0.002 && step(&right) < 0.002);
    assert!(left[left.len() - 1].abs() < 1.0e-5 && right[right.len() - 1].abs() < 1.0e-5);
}

#[test]
fn pitch_follows_root_tuning_and_scale() {
    let mut corrected = Smp::new("corrected", sine(441.0, 44_100, 44_100, 0.4), 44_100, 255);
    corrected.correction = 10;
    let samples = vec![
        Smp::new("tone", sine(441.0, 44_100, 44_100, 0.4), 44_100, 60),
        corrected,
    ];
    let zones = vec![
        vec![(SAMPLE_MODES, 1)],
        vec![(KEY_RANGE, range(0, 72)), (SAMPLE_ID, 0)],
        vec![
            (KEY_RANGE, range(73, 73)),
            (ROOT_KEY, 73),
            (COARSE_TUNE, 1),
            (FINE_TUNE, 50),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(74, 80)),
            (SCALE_TUNING, 0),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(81, 81)),
            (SCALE_TUNING, 50),
            (ROOT_KEY, 79),
            (SAMPLE_ID, 0),
        ],
        // An original pitch of 255 is key 60; the sample's own correction is added.
        vec![(KEY_RANGE, range(82, 127)), (SAMPLE_ID, 1)],
    ];
    let (_file, mut sampler) = player("pitch", samples, zones);
    let mut hz = |key: u8, bend: i16| {
        sampler.reset();
        sampler.event(InstrumentEvent::PitchBend { value: bend });
        sampler.event(on(key, 100));
        play(&mut sampler, at(0.1), BLOCK);
        frequency(&play(&mut sampler, at(0.5), BLOCK).0)
    };
    let up = |semitones: f64| 441.0 * (semitones / 12.0).exp2();
    assert!(cents(hz(60, 0), 441.0).abs() < 0.5);
    assert!(cents(hz(72, 0), 882.0).abs() < 0.5);
    assert!(cents(hz(48, 0), 220.5).abs() < 0.5);
    assert!(cents(hz(73, 0), up(1.5)).abs() < 0.5);
    assert!(cents(hz(77, 0), 441.0).abs() < 0.5);
    assert!(cents(hz(81, 0), up(1.0)).abs() < 0.5);
    assert!(cents(hz(84, 0), up(24.1)).abs() < 0.5);
    assert!(cents(hz(60, 8191), up(2.0)).abs() < 1.0);
    assert!(cents(hz(77, -8192), up(-2.0)).abs() < 1.0);
}

/// A looped 4 kHz sine; key 60 plays it as it is.
fn high_tone() -> Smp {
    Smp::new("high", sine(4000.0, 48_000, 4800, 0.4), 48_000, 60)
}

/// Absolute cents of a frequency.
fn filter_cents(hz: f64) -> i16 {
    (1200.0 * (hz / 8.176).log2()).round() as i16
}

#[test]
fn the_filter_cuts_what_is_above_its_cutoff() {
    let zones = vec![
        vec![(SAMPLE_MODES, 1), (SCALE_TUNING, 0)],
        vec![
            (KEY_RANGE, range(0, 60)),
            (FILTER_FC, filter_cents(1000.0)),
            (SAMPLE_ID, 0),
        ],
        vec![(KEY_RANGE, range(61, 61)), (SAMPLE_ID, 0)],
        vec![
            (KEY_RANGE, range(62, 62)),
            (FILTER_FC, filter_cents(4000.0)),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(63, 63)),
            (FILTER_FC, filter_cents(4000.0)),
            (FILTER_Q, 120),
            (SAMPLE_ID, 0),
        ],
    ];
    let (_file, mut sampler) = player("filter", vec![high_tone()], zones);
    let mut level = |key: u8| {
        sampler.reset();
        sampler.event(control(7, 127));
        sampler.event(on(key, 127));
        let (left, _) = play(&mut sampler, at(0.3), BLOCK);
        rms(&left[at(0.2)..])
    };
    let open = level(61);
    // Two octaves over the cutoff of a two-pole filter: 24 dB down.
    let far = db(level(60) / open);
    assert!((-25.5..=-23.5).contains(&far), "{far}");
    // At the cutoff the flat response is 3 dB down.
    let at_cutoff = db(level(62) / open);
    assert!((at_cutoff + 3.0).abs() < 0.3, "{at_cutoff}");
    // 12 dB of resonance: the peak is 12 dB over the flat response's -3 dB, and the voice is
    // half of the 12 dB quieter.
    let peak = db(level(63) / open);
    assert!((peak - 3.0).abs() < 0.3, "{peak}");
}

#[test]
fn the_modulation_envelope_and_the_lfo_move_the_filter() {
    let swept = vec![
        (FILTER_FC, filter_cents(500.0)),
        (MOD_ENV_TO_FILTER, 4800),
        (DECAY_MOD_ENV, timecents(0.2)),
        (SUSTAIN_MOD_ENV, 1000),
        (KEY_RANGE, range(0, 60)),
        (SAMPLE_MODES, 1),
        (SAMPLE_ID, 0),
    ];
    let wobbled = vec![
        (FILTER_FC, filter_cents(2000.0)),
        (MOD_LFO_TO_FILTER, 2400),
        (FREQ_MOD_LFO, filter_cents(5.0)),
        (KEY_RANGE, range(61, 127)),
        (ROOT_KEY, 61),
        (SAMPLE_MODES, 1),
        (SAMPLE_ID, 0),
    ];
    let (_file, mut sampler) = player("sweep", vec![high_tone()], vec![swept, wobbled]);
    sampler.event(on(60, 127));
    let (left, _) = play(&mut sampler, at(0.4), BLOCK);
    // Open at the start (the cutoff is four octaves up), closed when the envelope is down.
    let (start, end) = (rms(&left[at(0.002)..at(0.01)]), rms(&left[at(0.3)..]));
    assert!(db(end / start) < -30.0, "{}", db(end / start));
    // Going down all the way, with no jump.
    let slices: Vec<f32> = left[..at(0.2)].chunks(at(0.025)).map(rms).collect();
    assert!(slices.windows(2).all(|w| w[1] < w[0]), "{slices:?}");

    sampler.reset();
    sampler.event(on(61, 127));
    let (left, _) = play(&mut sampler, at(0.4), BLOCK);
    // 5 Hz: the cutoff is highest after 50 ms and lowest after 150 ms.
    let (high, low) = (
        rms(&left[at(0.045)..at(0.055)]),
        rms(&left[at(0.145)..at(0.155)]),
    );
    assert!(db(high / low) > 30.0, "{}", db(high / low));
}

#[test]
fn the_lfos_wait_and_then_move_loudness_and_pitch() {
    let tremolo = vec![
        (KEY_RANGE, range(0, 60)),
        (MOD_LFO_TO_VOLUME, 60),
        (FREQ_MOD_LFO, filter_cents(5.0)),
        (DELAY_MOD_LFO, timecents(0.2)),
        (SAMPLE_MODES, 1),
        (SAMPLE_ID, 0),
    ];
    let vibrato = vec![
        (KEY_RANGE, range(61, 127)),
        (ROOT_KEY, 61),
        (VIB_LFO_TO_PITCH, 100),
        (FREQ_VIB_LFO, filter_cents(5.0)),
        (SAMPLE_MODES, 1),
        (SAMPLE_ID, 1),
    ];
    let tone = Smp::new("tone", sine(1000.0, 48_000, 4800, 0.4), 48_000, 60);
    let (_file, mut sampler) = player("lfo", vec![flat_sample(), tone], vec![tremolo, vibrato]);
    sampler.event(on(60, 127));
    let (left, _) = play(&mut sampler, at(0.6), BLOCK);
    let span = |x: &[f32]| {
        let low = x.iter().fold(f32::MAX, |m, v| m.min(*v));
        let high = x.iter().fold(0.0_f32, |m, v| m.max(*v));
        db(high / low)
    };
    assert!(span(&left[at(0.01)..at(0.19)]) < 0.01);
    // 6 dB each way, and up first.
    let moved = span(&left[at(0.2)..]);
    assert!((moved - 12.0).abs() < 0.3, "{moved}");
    assert!((db(left[at(0.25)] / left[at(0.1)]) - 6.0).abs() < 0.3);

    sampler.reset();
    sampler.event(on(61, 127));
    let (left, _) = play(&mut sampler, at(0.6), BLOCK);
    let times = crossings(&left);
    let periods: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let longest = periods.iter().fold(0.0_f64, |m, p| m.max(*p));
    let shortest = periods.iter().fold(f64::MAX, |m, p| m.min(*p));
    // 100 cents each way.
    let moved = 1200.0 * (longest / shortest).log2();
    assert!((moved - 200.0).abs() < 12.0, "{moved}");
    // Up first: the first quarter of the LFO's turn is the sharp one.
    let early = frequency(&left[at(0.03)..at(0.07)]);
    assert!(cents(early, 1000.0) > 50.0);
}

#[test]
fn the_modulation_envelope_moves_the_pitch() {
    let zone = vec![
        (MOD_ENV_TO_PITCH, 1200),
        (HOLD_MOD_ENV, timecents(0.2)),
        (DECAY_MOD_ENV, timecents(0.05)),
        (SUSTAIN_MOD_ENV, 500),
        (SAMPLE_MODES, 1),
        (SAMPLE_ID, 0),
    ];
    let tone = Smp::new("tone", sine(1000.0, 48_000, 4800, 0.4), 48_000, 60);
    let (_file, mut sampler) = player("bendenv", vec![tone], vec![zone]);
    sampler.event(on(60, 127));
    let (left, _) = play(&mut sampler, at(0.6), BLOCK);
    // An octave up while the envelope holds, half of it at the sustain level.
    assert!(cents(frequency(&left[at(0.02)..at(0.18)]), 2000.0).abs() < 1.0);
    assert!(cents(frequency(&left[at(0.3)..]), 1000.0 * 2.0_f64.sqrt()).abs() < 1.0);
}

#[test]
fn voices_of_one_exclusive_class_stop_each_other() {
    let zones = vec![
        vec![(SAMPLE_MODES, 1)],
        vec![
            (KEY_RANGE, range(60, 60)),
            (EXCLUSIVE_CLASS, 3),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(61, 61)),
            (EXCLUSIVE_CLASS, 3),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(62, 62)),
            (EXCLUSIVE_CLASS, 4),
            (SAMPLE_ID, 0),
        ],
        vec![(KEY_RANGE, range(63, 63)), (SAMPLE_ID, 0)],
    ];
    let (_file, mut sampler) = player("class", vec![flat_sample()], zones);
    for key in [60, 62, 63] {
        sampler.event(on(key, 100));
    }
    play(&mut sampler, at(0.05), BLOCK);
    assert_eq!(sampler.active_voices(), 3);
    sampler.event(on(61, 100));
    play(&mut sampler, at(0.05), BLOCK);
    // Key 60 is gone within a few milliseconds; the others play on.
    assert_eq!(sampler.active_voices(), 3);
    sampler.event(off(61));
    sampler.event(on(60, 100));
    sampler.event(on(60, 100));
    play(&mut sampler, at(0.05), BLOCK);
    // The same key again stops its own earlier voice too.
    assert_eq!(sampler.active_voices(), 3);
}

#[test]
fn the_three_loop_modes() {
    // 0.1 s of one value, the loop in its first half, then 0.05 s of the opposite value.
    let mut data = flat(8000, 4800);
    data.extend(flat(-8000, 2400));
    let mut sample = Smp::new("tail", data, 48_000, 60);
    sample.looped = (480, 2400);
    let zones = vec![
        vec![(RELEASE_VOL_ENV, timecents(4.0))],
        vec![(KEY_RANGE, range(60, 60)), (SAMPLE_ID, 0)],
        vec![
            (KEY_RANGE, range(61, 61)),
            (SAMPLE_MODES, 1),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(62, 62)),
            (SAMPLE_MODES, 3),
            (SAMPLE_ID, 0),
        ],
        vec![
            (KEY_RANGE, range(63, 63)),
            (SAMPLE_MODES, 2),
            (SAMPLE_ID, 0),
        ],
    ];
    let (_file, mut sampler) = player("loops", vec![sample], zones);
    let mut run = |key: u8| {
        sampler.reset();
        sampler.event(on(key, 127));
        let (held, _) = play(&mut sampler, at(0.3), BLOCK);
        sampler.event(off(key));
        let (released, _) = play(&mut sampler, at(0.3), BLOCK);
        (held, released, sampler.active_voices())
    };
    // No loop: the file plays once, tail and all, and the voice ends.
    for key in [60, 63] {
        let (held, _, voices) = run(key);
        assert!(held[at(0.05)] > 0.0 && held[at(0.12)] < 0.0);
        assert_eq!((held[at(0.16)], voices), (0.0, 0));
    }
    // Looped for ever: the tail is never reached, key up or not.
    let (held, released, voices) = run(61);
    assert!(held.iter().chain(&released).all(|x| *x >= 0.0));
    assert!(released[at(0.29)] > 0.0 && voices == 1);
    // Looped until the key goes up: then the rest of the file plays and the voice ends.
    let (held, released, voices) = run(62);
    assert!(held[at(0.01)..].iter().all(|x| *x > 0.0));
    assert!(released[at(0.1)] < 0.0);
    assert_eq!((released[at(0.15)], voices), (0.0, 0));
}

#[test]
fn a_storm_of_events_gives_no_nan_and_reset_is_silent_at_once() {
    let file = TempFile::new("storm", &rich().bytes());
    let mut sampler = file.sampler(RATE, BLOCK);
    let mut storm = Storm::new(11);
    let mut most = 0;
    for block in 0..4000 {
        for _ in 0..block % 4 {
            sampler.event(storm.next());
        }
        if block % 50 == 0 {
            sampler.event(control([7, 10, 11][block / 50 % 3], (block % 128) as u8));
        }
        let (left, right) = play(&mut sampler, BLOCK, BLOCK);
        assert!(
            left.iter()
                .chain(&right)
                .all(|x| x.is_finite() && x.abs() < 64.0)
        );
        most = most.max(sampler.active_voices());
    }
    assert!(most > 16, "{most}");
    sampler.event(on(40, 127));
    sampler.reset();
    let (left, right) = play(&mut sampler, BLOCK, BLOCK);
    assert!(left.iter().chain(&right).all(|x| *x == 0.0));
    assert_eq!(sampler.active_voices(), 0);
}
