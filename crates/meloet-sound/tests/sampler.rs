//! The sample player on little packs made here: pitch, velocity, loops, one-shots, turns, choke,
//! release and pedal sounds, sustain, stealing, no NaN, reset.

mod support;

use meloet_engine::{Instrument, InstrumentEvent};
use support::pack::{Storm, TestPack, everything, level, off, on, pedal, play, region, tone};

const RATE: f32 = 48_000.0;
const BLOCK: usize = 256;
const SQRT_2: f32 = std::f32::consts::SQRT_2;

/// From the times of the upward zero crossings, each placed between its two frames.
fn frequency(x: &[f32], rate: f32) -> f64 {
    let (mut first, mut last, mut count) = (None, 0.0, 0_u32);
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let at = (i - 1) as f64 + f64::from(-x[i - 1]) / f64::from(x[i] - x[i - 1]);
            match first {
                None => first = Some(at),
                Some(_) => count += 1,
            }
            last = at;
        }
    }
    f64::from(count) * f64::from(rate) / (last - first.expect("no zero crossing"))
}

fn cents(got: f64, want: f64) -> f64 {
    1200.0 * (got / want).log2()
}

fn semitones(hz: f64, up: f64) -> f64 {
    hz * (up / 12.0).exp2()
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn biggest_step(x: &[f32]) -> f32 {
    x.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
}

fn close(got: f32, want: f32) -> bool {
    (got - want).abs() <= 1.0e-4 + want.abs() * 1.0e-3
}

/// Starts fresh, plays one key and gives the level 100 frames in (left, right).
fn level_of(sampler: &mut dyn Instrument, key: u8, velocity: u8) -> (f32, f32) {
    sampler.reset();
    sampler.event(on(key, velocity));
    let (left, right) = play(sampler, BLOCK, BLOCK);
    (left[100], right[100])
}

#[test]
fn pitch_follows_key_tuning_file_rate_and_output_rate() {
    let pack = TestPack::new("pitch");
    pack.wav("a440", 44_100, &[&tone(440.0, 44_100, 4 * 44_100, 0.5)]);
    pack.wav("a440at48", 48_000, &[&tone(440.0, 48_000, 4 * 48_000, 0.5)]);
    pack.instrument(
        "",
        &[
            region("a440", 0, 90, 69, ""),
            region("a440at48", 91, 99, 95, ""),
            region("a440", 100, 100, 100, "\"tuneCents\": 50"),
            region("a440", 101, 110, 101, "\"pitchKeytrack\": 0"),
            region(
                "a440",
                111,
                120,
                111,
                "\"pitchKeytrack\": 50, \"tuneCents\": -30",
            ),
        ],
    );
    let bank = pack.load();
    for (key, rate, want) in [
        (69, 48_000.0, 440.0),
        (81, 48_000.0, 880.0),
        (57, 48_000.0, 220.0),
        (70, 48_000.0, semitones(440.0, 1.0)),
        (62, 48_000.0, semitones(440.0, -7.0)),
        (69, 44_100.0, 440.0),
        (76, 44_100.0, semitones(440.0, 7.0)),
        (69, 96_000.0, 440.0),
        (95, 48_000.0, 440.0),
        (93, 44_100.0, semitones(440.0, -2.0)),
        (100, 48_000.0, semitones(440.0, 0.5)),
        (108, 48_000.0, 440.0),
        (117, 48_000.0, semitones(440.0, 3.0 - 0.3)),
    ] {
        let mut sampler = meloet_sound::sampler::Sampler::new(bank.clone());
        sampler.prepare(rate, BLOCK);
        sampler.event(on(key, 100));
        let (left, _) = play(&mut sampler, rate as usize, BLOCK);
        let off_by = cents(frequency(&left[100..], rate), want);
        assert!(off_by.abs() < 1.0, "key {key} at {rate}: {off_by} cents");
        assert!(peak(&left) > 0.3 && peak(&left) < 0.32, "key {key}");
    }
}

#[test]
fn bend_is_two_semitones_each_way() {
    let pack = TestPack::new("bend");
    pack.wav("a440", 44_100, &[&tone(440.0, 44_100, 4 * 44_100, 0.5)]);
    pack.instrument("", &[region("a440", 0, 127, 69, "")]);
    for (value, up) in [(8191, 2.0 * 8191.0 / 8192.0), (-8192, -2.0), (0, 0.0)] {
        let mut sampler = pack.sampler(RATE, BLOCK);
        sampler.event(InstrumentEvent::PitchBend { value: 4000 });
        sampler.event(on(69, 100));
        play(&mut sampler, 4800, BLOCK);
        sampler.event(InstrumentEvent::PitchBend { value });
        // The bend glides for a few milliseconds.
        play(&mut sampler, 4800, BLOCK);
        let (left, _) = play(&mut sampler, 48_000, BLOCK);
        let off_by = cents(frequency(&left, RATE), semitones(440.0, up));
        assert!(off_by.abs() < 1.0, "bend {value}: {off_by} cents");
    }
}

#[test]
fn a_key_far_from_its_root_is_no_more_than_120_semitones_off() {
    let pack = TestPack::new("far");
    pack.wav("a4400", 48_000, &[&tone(4400.0, 48_000, 48_000, 0.5)]);
    pack.instrument(
        "",
        &[
            region("a4400", 0, 10, 127, ""),
            region(
                "a4400",
                100,
                127,
                0,
                "\"pitchKeytrack\": 1200, \"loopStart\": 200, \"loopEnd\": 1000",
            ),
        ],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    // 127 semitones down.
    sampler.event(on(0, 100));
    let (left, _) = play(&mut sampler, 3 * RATE as usize, BLOCK);
    let off_by = cents(frequency(&left, RATE), semitones(4400.0, -120.0));
    assert!(off_by.abs() < 5.0, "{off_by} cents");

    // 1524 semitones up, in a loop: it goes round the loop 120 semitones up.
    sampler.reset();
    sampler.event(on(127, 100));
    let (left, right) = play(&mut sampler, 4 * BLOCK, BLOCK);
    assert!(left.iter().chain(&right).all(|x| x.is_finite()));
    assert_eq!(sampler.active_voices(), 1);
}

#[test]
fn velocity_picks_the_layer_and_sets_the_loudness() {
    let pack = TestPack::new("velocity");
    pack.wav("soft", 48_000, &[&level(0.25, 4800)]);
    pack.wav("hard", 48_000, &[&level(0.5, 4800)]);
    pack.instrument(
        "",
        &[
            region("soft", 60, 60, 60, "\"velHi\": 64, \"ampVeltrack\": 0"),
            region("hard", 60, 60, 60, "\"velLo\": 65, \"ampVeltrack\": 0"),
            region("hard", 62, 62, 62, ""),
            region("hard", 64, 64, 64, "\"ampVeltrack\": 50"),
            region("hard", 65, 65, 65, "\"ampVeltrack\": -100"),
        ],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    for (velocity, want) in [(1, 0.25), (64, 0.25), (65, 0.5), (127, 0.5)] {
        let (left, right) = level_of(&mut sampler, 60, velocity);
        assert!(
            close(left, want) && close(right, want),
            "{velocity}: {left}"
        );
    }
    for velocity in [1_u8, 32, 64, 100, 127] {
        let curve = (f32::from(velocity) / 127.0).powi(2);
        let (full, _) = level_of(&mut sampler, 62, velocity);
        assert!(close(full, 0.5 * curve), "100 %, {velocity}: {full}");
        let (half, _) = level_of(&mut sampler, 64, velocity);
        assert!(close(half, 0.5 * (0.5 + 0.5 * curve)), "50 %, {velocity}");
        let (turned, _) = level_of(&mut sampler, 65, velocity);
        assert!(close(turned, 0.5 * (1.0 - curve)), "-100 %, {velocity}");
    }
}

#[test]
fn gain_pan_and_stereo_files() {
    let pack = TestPack::new("gain");
    pack.wav("mono", 48_000, &[&level(0.5, 4800)]);
    pack.wav("stereo", 48_000, &[&level(0.5, 4800), &level(-0.25, 4800)]);
    let flat = "\"ampVeltrack\": 0";
    pack.instrument(
        "\"gainDb\": -6",
        &[
            region("mono", 60, 60, 60, flat),
            region("mono", 61, 61, 61, &format!("{flat}, \"gainDb\": -6")),
            region("mono", 62, 62, 62, &format!("{flat}, \"pan\": -1")),
            region("mono", 63, 63, 63, &format!("{flat}, \"pan\": 1")),
            region("mono", 64, 64, 64, &format!("{flat}, \"pan\": 0.5")),
            region("stereo", 65, 65, 65, flat),
        ],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    let g = 0.5 * 10.0_f32.powf(-6.0 / 20.0);
    for (key, want_left, want_right) in [
        (60, g, g),
        (
            61,
            g * 10.0_f32.powf(-6.0 / 20.0),
            g * 10.0_f32.powf(-6.0 / 20.0),
        ),
        (62, g * SQRT_2, 0.0),
        (63, 0.0, g * SQRT_2),
        (
            64,
            g * SQRT_2 * (0.375 * std::f32::consts::PI).cos(),
            g * SQRT_2 * (0.375 * std::f32::consts::PI).sin(),
        ),
        (65, g, -0.5 * g),
    ] {
        let (left, right) = level_of(&mut sampler, key, 100);
        assert!(
            close(left, want_left) && close(right, want_right),
            "key {key}: {left} {right}"
        );
    }
}

#[test]
fn attack_fades_in_and_keys_without_a_region_are_silent() {
    let pack = TestPack::new("attack");
    pack.wav("flat", 48_000, &[&level(0.5, 48_000)]);
    pack.instrument(
        "",
        &[region(
            "flat",
            60,
            60,
            60,
            "\"ampVeltrack\": 0, \"attackMs\": 20",
        )],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    sampler.event(on(60, 100));
    let (left, _) = play(&mut sampler, 4800, BLOCK);
    assert!(left[0].abs() < 0.01);
    assert!((left[480] - 0.25).abs() < 0.02, "{}", left[480]);
    assert!(close(left[1100], 0.5));
    assert!(left.windows(2).all(|w| w[1] >= w[0] - 1.0e-6));

    sampler.reset();
    for key in [0, 59, 61, 127] {
        sampler.event(on(key, 100));
    }
    let (left, right) = play(&mut sampler, 4800, BLOCK);
    assert_eq!(sampler.active_voices(), 0);
    assert_eq!(peak(&left) + peak(&right), 0.0);
}

/// 1200 frames of 480 Hz at 48 kHz: a cycle is 100 frames, the loop is 8 cycles.
fn looped_pack(name: &str, top: &str, more: &str) -> TestPack {
    let pack = TestPack::new(name);
    pack.wav("tone", 48_000, &[&tone(480.0, 48_000, 1200, 0.5)]);
    let fields = format!("\"loopStart\": 200, \"loopEnd\": 1000, \"ampVeltrack\": 0{more}");
    pack.instrument(top, &[region("tone", 0, 127, 60, &fields)]);
    pack
}

#[test]
fn a_loop_goes_on_without_a_seam_and_ends_when_the_key_goes_up() {
    let pack = looped_pack("loop", "", ", \"releaseMs\": 100");
    for (key, slope) in [(60, 0.033), (61, 0.035), (55, 0.025)] {
        let mut sampler = pack.sampler(RATE, BLOCK);
        sampler.event(on(key, 100));
        let (left, _) = play(&mut sampler, 96_000, BLOCK);
        assert!(close(rms(&left[90_000..]), 0.5 / SQRT_2), "key {key}");
        // A seam at the loop point would be a jump or a slip in the phase.
        assert!(
            biggest_step(&left) < slope,
            "key {key}: {}",
            biggest_step(&left)
        );
        let want = semitones(480.0, f64::from(key) - 60.0);
        let off_by = cents(frequency(&left[100..], RATE), want);
        assert!(off_by.abs() < 1.0, "key {key}: {off_by} cents");

        sampler.event(off(key));
        let (tail, _) = play(&mut sampler, 9600, BLOCK);
        assert!(biggest_step(&tail) < slope);
        // The release time is the time to -60 dB.
        let at_release = peak(&tail[4700..4900]);
        assert!(
            at_release < 0.5 * 2.0e-3 && at_release > 0.5 * 5.0e-4,
            "{at_release}"
        );
        assert_eq!(peak(&tail[7200..]), 0.0);
        assert_eq!(sampler.active_voices(), 0);
    }
}

#[test]
fn a_loop_decays_at_its_own_rate() {
    let pack = looped_pack("loop-decay", "", ", \"loopDecayDbPerS\": 20");
    let mut sampler = pack.sampler(RATE, BLOCK);
    sampler.event(on(60, 100));
    let (left, _) = play(&mut sampler, 6 * 48_000, BLOCK);
    let db = |from: usize| 20.0 * rms(&left[from..from + 4800]).log10();
    assert!((db(24_000) - db(72_000) - 20.0).abs() < 0.2);
    assert!((db(72_000) - db(120_000) - 20.0).abs() < 0.2);
    // -80 dB is reached after four seconds: the voice ends by itself.
    assert_eq!(peak(&left[5 * 48_000..]), 0.0);
    assert_eq!(sampler.active_voices(), 0);
}

#[test]
fn a_file_without_a_loop_plays_once() {
    let pack = TestPack::new("once");
    pack.wav("tone", 48_000, &[&tone(480.0, 48_000, 1200, 0.5)]);
    pack.instrument("", &[region("tone", 60, 60, 60, "")]);
    let mut sampler = pack.sampler(RATE, BLOCK);
    sampler.event(on(60, 127));
    let (left, _) = play(&mut sampler, 4800, BLOCK);
    assert!(peak(&left[..1200]) > 0.49);
    assert_eq!(peak(&left[1210..]), 0.0);
    assert_eq!(sampler.active_voices(), 0);
}

#[test]
fn one_shot_rings_on_after_the_key_goes_up() {
    let pack = TestPack::new("one-shot");
    pack.wav("hit", 48_000, &[&level(0.5, 4800)]);
    let flat = "\"ampVeltrack\": 0, \"releaseMs\": 10";
    pack.instrument(
        "",
        &[
            region(
                "hit",
                36,
                36,
                36,
                &format!("{flat}, \"pan\": -1, \"oneShot\": true"),
            ),
            region("hit", 38, 38, 38, &format!("{flat}, \"pan\": 1")),
        ],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    for event in [on(36, 100), on(38, 100), off(36), off(38)] {
        sampler.event(event);
    }
    sampler.event(InstrumentEvent::AllNotesOff);
    let (left, right) = play(&mut sampler, 9600, BLOCK);
    assert!(close(left[4700], 0.5 * SQRT_2));
    assert_eq!(peak(&left[4810..]), 0.0);
    assert_eq!(peak(&right[1000..]), 0.0);
    assert_eq!(sampler.active_voices(), 0);
}

#[test]
fn takes_come_in_turn_for_each_layer() {
    let pack = TestPack::new("turns");
    for (name, value) in [("a", 0.1), ("b", 0.2), ("c", 0.3), ("d", 0.4), ("e", 0.5)] {
        pack.wav(name, 48_000, &[&level(value, 480)]);
    }
    let hard = |index: u32| {
        format!(
            "\"ampVeltrack\": 0, \"oneShot\": true, \"velLo\": 65, \"rrIndex\": {index}, \"rrCount\": 3"
        )
    };
    let soft = |index: u32| {
        format!(
            "\"ampVeltrack\": 0, \"oneShot\": true, \"velHi\": 64, \"rrIndex\": {index}, \"rrCount\": 2"
        )
    };
    pack.instrument(
        "",
        &[
            region("a", 39, 39, 39, &hard(1)),
            region("b", 39, 39, 39, &hard(2)),
            region("c", 39, 39, 39, &hard(3)),
            region("d", 39, 39, 39, &soft(1)),
            region("e", 39, 39, 39, &soft(2)),
        ],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    let mut heard = Vec::new();
    for velocity in [100, 100, 20, 100, 100, 20, 20, 127, 1] {
        sampler.event(on(39, velocity));
        sampler.event(off(39));
        let (left, _) = play(&mut sampler, 960, BLOCK);
        heard.push((left[100] * 10.0).round() as u32);
    }
    assert_eq!(heard, [1, 2, 4, 3, 1, 5, 4, 2, 5]);
    sampler.reset();
    sampler.event(on(39, 100));
    let (left, _) = play(&mut sampler, 960, BLOCK);
    assert!(close(left[100], 0.1));
}

#[test]
fn the_closed_hi_hat_stops_the_open_one() {
    let pack = TestPack::new("choke");
    pack.wav("long", 48_000, &[&level(0.5, 96_000)]);
    pack.wav("short", 48_000, &[&level(0.5, 2400)]);
    let hit = "\"ampVeltrack\": 0, \"oneShot\": true";
    pack.instrument(
        "",
        &[
            region(
                "short",
                42,
                42,
                42,
                &format!("{hit}, \"pan\": 1, \"chokeGroup\": 1"),
            ),
            region(
                "long",
                46,
                46,
                46,
                &format!("{hit}, \"pan\": -1, \"chokedBy\": [1, 5]"),
            ),
            region(
                "long",
                49,
                49,
                49,
                &format!("{hit}, \"pan\": -1, \"gainDb\": -20"),
            ),
        ],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    sampler.event(on(46, 100));
    sampler.event(on(49, 100));
    let (left, _) = play(&mut sampler, 4800, BLOCK);
    let both = 0.55 * SQRT_2;
    assert!(close(left[4799], both));

    sampler.event(on(42, 100));
    let (left, right) = play(&mut sampler, 960, BLOCK);
    assert!(close(right[100], 0.5 * SQRT_2));
    // Six milliseconds down to the cymbal alone, with no jump on the way.
    assert!(left.windows(2).all(|w| w[1] <= w[0] + 1.0e-6));
    assert!(biggest_step(&left) < 0.5 * SQRT_2 / 200.0);
    assert!(left[100] > 0.3 && left[100] < 0.6);
    assert!(close(left[400], 0.05 * SQRT_2), "{}", left[400]);
    assert_eq!(sampler.active_voices(), 2);

    // The open hi-hat can be hit again, and the closed one does not stop itself.
    sampler.event(on(46, 100));
    sampler.event(on(42, 100));
    sampler.event(on(46, 100));
    let (left, right) = play(&mut sampler, 960, BLOCK);
    assert!(close(left[900], both));
    assert!(close(right[900], SQRT_2));
}

/// Key 60: a long held sound on the left, its release sound on the right.
fn release_pack(name: &str, top: &str) -> TestPack {
    let pack = TestPack::new(name);
    pack.wav("held", 48_000, &[&level(0.5, 480_000)]);
    pack.wav("up", 48_000, &[&level(0.5, 4800)]);
    let held = "\"ampVeltrack\": 0, \"pan\": -1, \"releaseMs\": 50";
    let up = "\"trigger\": \"release\", \"pan\": 1, \"rtDecayDbPerS\": 6";
    pack.instrument(
        top,
        &[
            region("held", 60, 60, 60, held),
            region("up", 60, 60, 60, up),
        ],
    );
    pack
}

#[test]
fn the_release_sound_plays_on_key_up_and_fades_with_the_time_held() {
    let pack = release_pack("release", "");
    let mut sampler = pack.sampler(RATE, BLOCK);
    for (held_frames, velocity) in [(0, 127), (48_000, 127), (96_000, 127), (24_000, 64)] {
        sampler.reset();
        sampler.event(on(60, velocity));
        let (left, right) = play(&mut sampler, held_frames, BLOCK);
        assert_eq!(peak(&right), 0.0);
        assert!(held_frames == 0 || close(left[held_frames - 1], 0.5 * SQRT_2));

        sampler.event(off(60));
        let (_, right) = play(&mut sampler, 9600, BLOCK);
        let by_time = 10.0_f32.powf(-6.0 * held_frames as f32 / 48_000.0 / 20.0);
        let by_velocity = (f32::from(velocity) / 127.0).powi(2);
        let want = 0.5 * SQRT_2 * by_time * by_velocity;
        assert!(close(right[100], want), "{held_frames}: {}", right[100]);
        assert_eq!(peak(&right[4810..]), 0.0);
    }
    // A key that was never down makes no release sound.
    sampler.reset();
    sampler.event(off(60));
    let (_, right) = play(&mut sampler, 960, BLOCK);
    assert_eq!(peak(&right), 0.0);
}

#[test]
fn the_pedal_keeps_notes_and_their_release_sounds_wait_for_it() {
    let pack = release_pack("sustain", "\"sustainPedal\": true");
    let mut sampler = pack.sampler(RATE, BLOCK);
    sampler.event(pedal(true));
    sampler.event(on(60, 127));
    sampler.event(off(60));
    let (left, right) = play(&mut sampler, 48_000, BLOCK);
    assert!(close(left[47_999], 0.5 * SQRT_2));
    assert_eq!(peak(&right), 0.0);

    sampler.event(pedal(false));
    let (left, right) = play(&mut sampler, 9600, BLOCK);
    assert_eq!(peak(&left[4800..]), 0.0);
    assert!(close(right[100], 0.5 * SQRT_2 * 10.0_f32.powf(-6.0 / 20.0)));

    // A key that is down again when the pedal comes up keeps its new note.
    sampler.reset();
    sampler.event(pedal(true));
    sampler.event(on(60, 127));
    sampler.event(off(60));
    sampler.event(on(60, 127));
    sampler.event(pedal(false));
    let (left, right) = play(&mut sampler, 9600, BLOCK);
    assert!(close(left[9599], 0.5 * SQRT_2));
    assert_eq!(peak(&right), 0.0);
    sampler.event(InstrumentEvent::AllNotesOff);
    let (left, right) = play(&mut sampler, 9600, BLOCK);
    assert_eq!(peak(&left[4800..]), 0.0);
    assert_eq!(peak(&right), 0.0);
    assert_eq!(sampler.active_voices(), 0);
}

#[test]
fn an_instrument_without_a_sustain_pedal_ignores_it() {
    let pack = release_pack("no-sustain", "");
    let mut sampler = pack.sampler(RATE, BLOCK);
    sampler.event(pedal(true));
    sampler.event(on(60, 127));
    sampler.event(off(60));
    let (left, right) = play(&mut sampler, 9600, BLOCK);
    assert_eq!(peak(&left[4800..]), 0.0);
    assert!(close(right[100], 0.5 * SQRT_2));
}

#[test]
fn the_pedal_makes_its_own_sounds() {
    let pack = TestPack::new("pedal");
    pack.wav("thump", 48_000, &[&level(0.5, 2400)]);
    let quiet = "\"gainDb\": -6, \"ampVeltrack\": 100";
    pack.instrument(
        "\"sustainPedal\": true",
        &[
            region(
                "thump",
                0,
                0,
                60,
                &format!("\"trigger\": \"pedalDown\", \"pan\": -1, {quiet}"),
            ),
            region(
                "thump",
                0,
                0,
                60,
                &format!("\"trigger\": \"pedalUp\", \"pan\": 1, {quiet}"),
            ),
        ],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    let want = 0.5 * SQRT_2 * 10.0_f32.powf(-6.0 / 20.0);
    let cc = |value: u8| InstrumentEvent::Control {
        controller: 64,
        value,
    };
    for (value, want_left, want_right) in [
        (64, want, 0.0),
        (127, 0.0, 0.0),
        (63, 0.0, want),
        (0, 0.0, 0.0),
        (100, want, 0.0),
    ] {
        sampler.event(cc(value));
        let (left, right) = play(&mut sampler, 4800, BLOCK);
        assert!(close(left[100], want_left), "{value}: {}", left[100]);
        assert!(close(right[100], want_right), "{value}: {}", right[100]);
    }
    sampler.event(InstrumentEvent::Control {
        controller: 1,
        value: 127,
    });
    let (left, right) = play(&mut sampler, 480, BLOCK);
    assert_eq!(peak(&left) + peak(&right), 0.0);
}

#[test]
fn a_stolen_voice_fades_and_the_new_note_does_not_wait() {
    let pack = looped_pack("steal", "\"polyphony\": 2", "");
    let mut sampler = pack.sampler(RATE, BLOCK);
    sampler.event(on(60, 100));
    let (first, _) = play(&mut sampler, 4800, BLOCK);
    sampler.event(on(64, 100));
    // 25 frames more: key 60 is at the top of its wave when it is taken.
    let (second, _) = play(&mut sampler, 4825, BLOCK);
    assert_eq!(sampler.active_voices(), 2);

    sampler.event(on(67, 100));
    let (third, _) = play(&mut sampler, 4800, BLOCK);
    let all: Vec<f32> = [first, second, third].concat();
    // Three sines of 0.5 at up to 720 Hz move at most 0.14 a frame together; a voice cut at
    // its top would add a jump of 0.5.
    assert!(biggest_step(&all) < 0.16, "{}", biggest_step(&all));
    assert!(all[9625 + 10].abs() > 0.0);
    assert_eq!(sampler.active_voices(), 2);

    // What is left is keys 64 and 67.
    let (rest, _) = play(&mut sampler, 48_000, BLOCK);
    let both = (0.5_f32 * 0.5 / 2.0 * 2.0).sqrt();
    assert!(close(rms(&rest), both), "{}", rms(&rest));
}

#[test]
fn a_released_voice_gives_its_place_before_a_held_one() {
    let pack = TestPack::new("steal-order");
    pack.wav("flat", 48_000, &[&level(0.25, 480_000)]);
    let flat = "\"ampVeltrack\": 0, \"releaseMs\": 5000";
    pack.instrument(
        "\"polyphony\": 2",
        &[
            region("flat", 60, 60, 60, &format!("{flat}, \"pan\": -1")),
            region("flat", 64, 64, 64, &format!("{flat}, \"pan\": 1")),
            region("flat", 67, 67, 67, &format!("{flat}, \"pan\": -1")),
        ],
    );
    let mut sampler = pack.sampler(RATE, BLOCK);
    for event in [on(60, 100), on(64, 100), off(64), on(67, 100)] {
        sampler.event(event);
    }
    assert_eq!(sampler.active_voices(), 3);
    let (left, right) = play(&mut sampler, 960, BLOCK);
    assert!(close(left[959], 0.5 * SQRT_2));
    assert_eq!(peak(&right[200..]), 0.0);
    assert_eq!(sampler.active_voices(), 2);

    // With every voice held, the oldest goes.
    sampler.event(on(64, 100));
    let (left, right) = play(&mut sampler, 960, BLOCK);
    assert!(close(left[959], 0.25 * SQRT_2));
    assert!(close(right[959], 0.25 * SQRT_2));
}

#[test]
fn more_notes_at_once_than_there_are_voices() {
    let pack = looped_pack("burst", "\"polyphony\": 4", "");
    let mut sampler = pack.sampler(RATE, BLOCK);
    for round in 0..20 {
        for key in 40..80 {
            sampler.event(on(key + round, 100));
        }
        assert!(sampler.active_voices() <= 8);
        let (left, right) = play(&mut sampler, 100, BLOCK);
        assert!(
            left.iter()
                .chain(&right)
                .all(|x| x.is_finite() && x.abs() < 4.0)
        );
    }
    play(&mut sampler, 480, BLOCK);
    assert_eq!(sampler.active_voices(), 4);
}

#[test]
fn a_storm_of_events_gives_no_nan_and_reset_is_silent_at_once() {
    let pack = everything("storm");
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        let mut sampler = pack.sampler(rate, BLOCK);
        let mut storm = Storm::new(rate as u32);
        let (mut left, mut right) = ([0.0_f32; BLOCK], [0.0_f32; BLOCK]);
        let mut loudest = 0.0_f32;
        for block in 0..4000_usize {
            for _ in 0..block % 4 {
                sampler.event(storm.next());
            }
            let frames = [BLOCK, 1, 64, 200, 33][block % 5];
            sampler.render(&mut left[..frames], &mut right[..frames]);
            for x in left[..frames].iter().chain(&right[..frames]) {
                assert!(x.is_finite(), "block {block}");
                loudest = loudest.max(x.abs());
            }
        }
        assert!(loudest > 0.1 && loudest < 8.0, "{loudest}");

        for key in 0..104 {
            sampler.event(on(key, 100));
        }
        sampler.event(pedal(true));
        sampler.event(InstrumentEvent::PitchBend { value: 8191 });
        sampler.reset();
        assert_eq!(sampler.active_voices(), 0);
        // Fill with something else first: render must overwrite, not add.
        let (mut left, mut right) = ([1.0_f32; BLOCK], [-1.0_f32; BLOCK]);
        sampler.render(&mut left, &mut right);
        assert_eq!(peak(&left) + peak(&right), 0.0);
        // The pedal is forgotten too: a key let go now ends its note.
        sampler.event(on(30, 100));
        sampler.event(off(30));
        play(&mut sampler, rate as usize, BLOCK);
        assert_eq!(sampler.active_voices(), 0);
    }
}

#[test]
fn prepare_again_is_silent_and_takes_the_new_rate() {
    let pack = looped_pack("prepare", "", "");
    let mut sampler = pack.sampler(RATE, BLOCK);
    sampler.event(on(60, 100));
    play(&mut sampler, 4800, BLOCK);
    sampler.prepare(96_000.0, 1024);
    let (left, _) = play(&mut sampler, 2048, 1024);
    assert_eq!(peak(&left), 0.0);
    sampler.event(on(60, 100));
    let (left, _) = play(&mut sampler, 96_000, 1024);
    assert!(cents(frequency(&left[100..], 96_000.0), 480.0).abs() < 1.0);
}
