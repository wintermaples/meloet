//! The tempo map: ticks, seconds, frames, bars and beats. Also writes the table of answers
//! that the screen's own tempo map (TypeScript) is checked against.
//!
//! `cargo test -p meloet-model --release --test tempo_map` runs the full sweep of tempos;
//! a debug build runs a thinned one. `-- --ignored --nocapture measure` shows the cost.

use std::fmt::Write as _;
use std::path::PathBuf;

use meloet_model::{
    Beat, Meter, MeterChange, TICKS_PER_QUARTER, Tempo, TempoMap, TempoPoint, Timeline,
};

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
const QI: i64 = Q as i64;
const QF: f64 = Q as f64;
const BARF: f64 = BAR as f64;

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

fn meter(num: u8, den: u8) -> Meter {
    Meter::new(num, den).unwrap()
}

fn point(tick: u64, centi: u32, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick,
        tempo: tempo(centi),
        ramp,
    }
}

fn change(bar: u32, num: u8, den: u8) -> MeterChange {
    MeterChange {
        bar,
        meter: meter(num, den),
    }
}

/// Spec 5: 4/4, 96 at the head, 104 at bar 5 with a ramp, 120 at bar 9.
fn spec_example() -> Timeline {
    Timeline {
        tempo_points: vec![point(4 * BAR, 10400, true), point(8 * BAR, 12000, false)],
        ..Timeline::new(tempo(9600), Meter::FOUR_FOUR)
    }
}

fn four_meters() -> Timeline {
    Timeline {
        meter_changes: vec![change(2, 3, 4), change(4, 6, 8), change(5, 7, 8)],
        ..Timeline::new(tempo(9600), Meter::FOUR_FOUR)
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn random_timeline(rng: &mut Rng) -> Timeline {
    let span = [2_000, 200_000, 20_000_000][rng.below(3) as usize];
    let any_tempo = |rng: &mut Rng| tempo(3000 + rng.below(27_001) as u32);
    let any_meter =
        |rng: &mut Rng| meter(1 + rng.below(12) as u8, Meter::DENS[rng.below(4) as usize]);
    let mut ticks: Vec<u64> = (0..rng.below(40)).map(|_| 1 + rng.below(span)).collect();
    ticks.sort_unstable();
    ticks.dedup();
    let mut bars: Vec<u32> = (0..rng.below(12))
        .map(|_| 1 + rng.below(span / 500) as u32)
        .collect();
    bars.sort_unstable();
    bars.dedup();
    let timeline = Timeline {
        tempo: any_tempo(rng),
        meter: any_meter(rng),
        head_ramp: rng.below(2) == 0,
        tempo_points: ticks
            .into_iter()
            .map(|tick| TempoPoint {
                tick,
                tempo: any_tempo(rng),
                ramp: rng.below(3) != 0,
            })
            .collect(),
        meter_changes: bars
            .into_iter()
            .map(|bar| MeterChange {
                bar,
                meter: any_meter(rng),
            })
            .collect(),
    };
    assert_eq!(timeline.check(), Ok(()));
    timeline
}

/// Every tick where a piece of the map starts.
fn cuts(timeline: &Timeline, map: &TempoMap) -> Vec<u64> {
    let mut cuts: Vec<u64> = timeline.tempo_points.iter().map(|p| p.tick).collect();
    cuts.extend(
        timeline
            .meter_changes
            .iter()
            .map(|c| map.tick_of_bar(u64::from(c.bar))),
    );
    cuts.push(0);
    cuts.sort_unstable();
    cuts.dedup();
    cuts
}

#[test]
fn the_example_of_the_spec() {
    let map = TempoMap::new(&spec_example());
    assert_eq!(map.piece_count(), 3);
    assert_eq!(map.tempo_at(0.0), 96.0);
    assert_eq!(map.tempo_at(4.0 * BARF - 1.0), 96.0);
    assert_eq!(map.tempo_at(4.0 * BARF), 104.0);
    assert_eq!(map.tempo_at(6.0 * BARF), 112.0);
    assert_eq!(map.tempo_at(8.0 * BARF), 120.0);
    assert_eq!(map.tempo_at(1.0e9), 120.0);

    let ramp = map.seconds_at(8.0 * BARF) - map.seconds_at(4.0 * BARF);
    assert!(
        (ramp - 60.0 * (120.0f64 / 104.0).ln()).abs() < 1e-12,
        "{ramp}"
    );
    assert_eq!(format!("{ramp:.3}"), "8.586");

    // 16 beats at 96, the ramp, then 4 beats at 120.
    let head = 16.0 * 60.0 / 96.0;
    assert!((map.seconds_at(4.0 * BARF) - head).abs() < 1e-12);
    assert!((map.seconds_at(9.0 * BARF) - (head + ramp + 2.0)).abs() < 1e-12);
}

#[test]
fn before_the_head_the_head_goes_on_without_a_ramp() {
    let timeline = Timeline {
        head_ramp: true,
        tempo_points: vec![point(BAR, 19200, false)],
        ..Timeline::new(tempo(9600), meter(6, 8))
    };
    let map = TempoMap::new(&timeline);
    assert_eq!(map.tempo_at(-5000.0), 96.0);
    assert!(map.tempo_at(2.0 * QF) > 96.0);
    assert!((map.seconds_at(-QF) + 60.0 / 96.0).abs() < 1e-15);
    assert!((map.tick_at(-60.0 / 96.0) + QF).abs() < 1e-12);
    assert_eq!(map.seconds_at(0.0), 0.0);
    assert_eq!(map.tick_at(0.0), 0.0);
    assert_eq!(
        map.next_beat(-QF / 2.0 - 1.0),
        Beat {
            tick: -QI / 2,
            downbeat: false
        }
    );
    assert_eq!(
        map.next_beat(-3.0 * QF),
        Beat {
            tick: -3 * QI,
            downbeat: true
        }
    );
    assert_eq!(
        map.next_beat(-0.5),
        Beat {
            tick: 0,
            downbeat: true
        }
    );
    // The clicks of 6/8 (dotted quarters) go on backwards too.
    assert_eq!(
        map.next_click(-QF / 2.0 - 1.0),
        Beat {
            tick: 0,
            downbeat: true
        }
    );
    assert_eq!(
        map.next_click(-3.0 * QF / 2.0 - 1.0),
        Beat {
            tick: -3 * QI / 2,
            downbeat: false
        }
    );
    assert_eq!(
        map.next_click(-3.0 * QF),
        Beat {
            tick: -3 * QI,
            downbeat: true
        }
    );
}

/// `Transport::frames_between` of `crates/meloet-engine/src/engine.rs` as it was before the
/// tempo map: positions are ticks × `scale`, with `scale` = 6000 × sample rate, and one frame
/// moves the position by `step` = tempo in hundredths × ticks per quarter note.
fn frames_between(from: i128, to: i128, step: i128) -> i128 {
    let gap = to - from;
    if gap <= 0 { 0 } else { (gap + step - 1) / step }
}

fn integer_frame_of(tick: u64, from_frames: u64, centi: u32, rate: u32) -> u64 {
    let scale = 6000 * i128::from(rate);
    let step = i128::from(centi) * i128::from(Q);
    frames_between(
        i128::from(from_frames) * step,
        i128::from(tick) * scale,
        step,
    ) as u64
}

#[derive(Default, Clone, Copy)]
struct Sweep {
    checked: u64,
    /// Where `ceil` without the 1e-6 of a frame gives another frame than the integers.
    guard_needed: u64,
    example: Option<(u32, u64, u32, u64, u64)>,
}

impl Sweep {
    fn check(&mut self, map: &TempoMap, centi: u32, tpb: u64, rate: u32, tick: u64, from: u64) {
        let want = integer_frame_of(tick, from, centi, rate);
        let from_seconds = from as f64 / f64::from(rate);
        let got = map.frame_of(tick as f64, from_seconds, f64::from(rate));
        assert_eq!(
            got, want,
            "tempo {centi} tpb {tpb} rate {rate} tick {tick} from frame {from}"
        );
        self.checked += 1;
        let bare = ((map.seconds_at(tick as f64) - from_seconds) * f64::from(rate))
            .ceil()
            .max(0.0) as u64;
        if bare != want {
            self.guard_needed += 1;
            self.example.get_or_insert((centi, tpb, rate, tick, from));
        }
    }

    fn add(&mut self, other: Sweep) {
        self.checked += other.checked;
        self.guard_needed += other.guard_needed;
        self.example = self.example.or(other.example);
    }
}

fn sweep_one(centi: u32, den: u8, rate: u32, rng: &mut Rng, sweep: &mut Sweep) {
    let m = meter(4, den);
    let tpb = m.ticks_per_beat();
    let map = TempoMap::new(&Timeline::new(tempo(centi), m));
    // Ten minutes of beats, each from the head, from a frame on the way, and a tick off the beat.
    let beats = u64::from(centi) / 10 + 1;
    for k in 0..=beats {
        let tick = k * tpb;
        sweep.check(&map, centi, tpb, rate, tick, 0);
        sweep.check(
            &map,
            centi,
            tpb,
            rate,
            tick,
            (k * 7919) % (u64::from(rate) * 60),
        );
        sweep.check(&map, centi, tpb, rate, tick + (k * 37) % tpb, k * 211);
    }
    // Two hours in.
    let two_hours = u64::from(centi) * 120 / 100;
    let frames = u64::from(rate) * 7200;
    for k in two_hours - 8..=two_hours + 8 {
        sweep.check(&map, centi, tpb, rate, k * tpb, 0);
        sweep.check(&map, centi, tpb, rate, k * tpb, frames - 100_000);
    }
    for _ in 0..32 {
        let tick = rng.below(two_hours * tpb);
        sweep.check(&map, centi, tpb, rate, tick, rng.below(frames));
    }
}

#[test]
fn frame_of_is_the_integer_formula_when_the_tempo_stays() {
    // A debug build takes every 53rd tempo (and both ends), so that `cargo test` stays quick;
    // a release build takes all 27 001.
    let stride = if cfg!(debug_assertions) { 53 } else { 1 };
    let mut total = Sweep::default();
    let parts: Vec<Sweep> = std::thread::scope(|scope| {
        let handles: Vec<_> = [44_100u32, 48_000, 96_000]
            .into_iter()
            .map(|rate| {
                scope.spawn(move || {
                    let mut sweep = Sweep::default();
                    let mut rng = Rng(u64::from(rate));
                    let mut centi = Tempo::MIN_CENTI;
                    loop {
                        sweep_one(centi, 4, rate, &mut rng, &mut sweep);
                        // The other beats (half, 8th, 16th note) on a thinner set.
                        if centi % (stride * 16) == 8 || centi == Tempo::MAX_CENTI {
                            for den in [2, 8, 16] {
                                sweep_one(centi, den, rate, &mut rng, &mut sweep);
                            }
                        }
                        if centi == Tempo::MAX_CENTI {
                            break;
                        }
                        centi = (centi + stride).min(Tempo::MAX_CENTI);
                    }
                    sweep
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for part in parts {
        total.add(part);
    }
    println!(
        "frame_of against the integers: {} checked, the 1e-6 guard decided {} (first: {:?})",
        total.checked, total.guard_needed, total.example
    );
}

#[test]
fn a_ramp_is_the_sum_of_small_steps() {
    // (meter, tempo at the start, at the end, ticks)
    for (m, from, to, len) in [
        (Meter::FOUR_FOUR, 10400u32, 12000u32, 4 * BAR),
        (Meter::FOUR_FOUR, 30000, 3000, Q),
        (meter(6, 8), 3000, 24000, Q / 2),
        (meter(2, 2), 12000, 12001, 100_000),
        (meter(7, 16), 20000, 6000, 30_000),
    ] {
        let at = BAR * 3;
        let timeline = Timeline {
            tempo_points: vec![point(at, from, true), point(at + len, to, false)],
            ..Timeline::new(tempo(9600), m)
        };
        let map = TempoMap::new(&timeline);
        let (a, b) = (f64::from(from) / 100.0, f64::from(to) / 100.0);
        // Midpoint rule over a million steps, with a compensated sum.
        let steps = 1_000_000u32;
        let h = len as f64 / f64::from(steps);
        let (mut sum, mut lost) = (0.0f64, 0.0f64);
        let mut marks = Vec::new();
        for i in 0..steps {
            let u = (f64::from(i) + 0.5) * h;
            let bpm = a + (b - a) * u / len as f64;
            let y = 60.0 * h / (QF * bpm) - lost;
            let next = sum + y;
            lost = (next - sum) - y;
            sum = next;
            if (i + 1) % 100_000 == 0 {
                marks.push((f64::from(i + 1) * h, sum));
            }
        }
        let start = map.seconds_at(at as f64);
        for (u, want) in marks {
            let got = map.seconds_at(at as f64 + u) - start;
            assert!(
                (got - want).abs() < 1e-9,
                "{from}->{to} u {u}: {got} {want}"
            );
            let back = map.tick_at(start + want) - at as f64;
            assert!((back - u).abs() < 1e-6, "{from}->{to} u {u}: back {back}");
        }
    }
}

#[test]
fn a_ramp_that_hardly_moves_is_close_to_no_ramp() {
    // The smallest slope a song can have: 0.01 over a very long way.
    let len = 100_000_000u64;
    let ramp = TempoMap::new(&Timeline {
        head_ramp: true,
        tempo_points: vec![point(len, 12001, false)],
        ..Timeline::new(tempo(12000), Meter::FOUR_FOUR)
    });
    let flat = TempoMap::new(&Timeline::new(tempo(12000), Meter::FOUR_FOUR));
    for tick in [1.0, QF, 96_000.0] {
        let (r, f) = (ramp.seconds_at(tick), flat.seconds_at(tick));
        // The tempo has moved by 0.01 × tick / len; the time by half of that share.
        let share = 0.01 * tick / len as f64 / 120.0 / 2.0;
        assert!(
            r <= f && (f - r) <= f * share * 1.001 + f * 1e-15,
            "{tick}: {r} {f}"
        );
        assert!((ramp.tick_at(r) - tick).abs() < 1e-9);
    }
}

#[test]
fn every_whole_tick_comes_back() {
    let mut rng = Rng(7);
    let mut checked = 0u64;
    for round in 0..3000 {
        let timeline = match round {
            0 => spec_example(),
            1 => four_meters(),
            _ => random_timeline(&mut rng),
        };
        let map = TempoMap::new(&timeline);
        let cuts = cuts(&timeline, &map);
        let last = *cuts.last().unwrap();
        let mut ticks: Vec<u64> = Vec::new();
        for &cut in &cuts {
            ticks.extend(cut.saturating_sub(3)..=cut + 3);
        }
        ticks.extend((0..60).map(|_| rng.below(last + 10_000)));
        // Two hours and more at the fastest tempo, where an f64 of seconds is coarsest.
        ticks.extend((0..4).map(|_| last + rng.below(50_000_000)));
        for k in ticks {
            let seconds = map.seconds_at(k as f64);
            let back = map.tick_at(seconds);
            assert_eq!(back.round(), k as f64, "round {round} tick {k}: {back}");
            assert!(
                (back - k as f64).abs() < 1e-4,
                "round {round} tick {k}: {back}"
            );
            checked += 1;
        }
        // And before the head.
        for k in [-1.0, -(QF - 1.0), -10_000.0] {
            assert_eq!(map.tick_at(map.seconds_at(k)).round(), k);
        }
    }
    println!("round trips: {checked}");
}

#[test]
fn seconds_and_frames_never_go_back() {
    let mut rng = Rng(11);
    for round in 0..400 {
        let timeline = random_timeline(&mut rng);
        let map = TempoMap::new(&timeline);
        let cuts = cuts(&timeline, &map);
        let rate = [44_100.0, 48_000.0, 96_000.0][round % 3];
        let from = rng.unit() * 3.0;
        // Whole ticks and small steps over the first cuts, then long strides over the rest.
        let mut walks: Vec<(f64, f64, u32)> = cuts
            .iter()
            .take(6)
            .flat_map(|&cut| {
                let cut = cut as f64;
                [
                    (cut - 40.0, 1.0, 80),
                    (cut - 2.0, 0.01, 400),
                    (cut - 1e-6, 1e-8, 200),
                ]
            })
            .collect();
        let last = *cuts.last().unwrap() as f64;
        walks.push((-2000.0, (last + 5000.0) / 4000.0, 4100));
        for (start, step, count) in walks {
            let (mut seconds, mut frame) = (f64::NEG_INFINITY, 0u64);
            for i in 0..count {
                let tick = start + f64::from(i) * step;
                let s = map.seconds_at(tick);
                let f = map.frame_of(tick, from, rate);
                assert!(s >= seconds, "round {round} tick {tick}: {s} < {seconds}");
                assert!(f >= frame, "round {round} tick {tick}: {f} < {frame}");
                (seconds, frame) = (s, f);
            }
        }
    }
}

#[test]
fn frame_of_is_the_first_frame_at_or_past_the_tick() {
    let mut rng = Rng(13);
    for _ in 0..300 {
        let map = TempoMap::new(&random_timeline(&mut rng));
        let rate = 48_000.0;
        let from = rng.unit() * 2.0;
        for _ in 0..50 {
            let tick = rng.below(300_000) as f64;
            let frame = map.frame_of(tick, from, rate);
            if frame == 0 {
                assert!(map.seconds_at(tick) <= from + 1e-6 / rate);
                continue;
            }
            // Within the guard (1e-6 of a frame, a few 1e-9 of a tick).
            let at = map.tick_at(from + frame as f64 / rate);
            let before = map.tick_at(from + (frame - 1) as f64 / rate);
            assert!(
                at > tick - 1e-6 && before < tick + 1e-6,
                "{tick}: {before} {at}"
            );
        }
    }
}

#[test]
fn bars_and_beats_over_four_meters() {
    // 4/4 ×2, 3/4 ×2, 6/8 ×1, then 7/8.
    let map = TempoMap::new(&four_meters());
    let (three, six, seven) = (2 * BAR, 2 * BAR + 6 * Q, 2 * BAR + 9 * Q);
    let heads = [
        0,
        BAR,
        three,
        three + 3 * Q,
        six,
        seven,
        seven + 7 * Q / 2,
        seven + 7 * Q,
    ];
    for (bar, &tick) in heads.iter().enumerate() {
        assert_eq!(map.tick_of_bar(bar as u64), tick);
        let at = map.meter_at(tick);
        assert_eq!((at.bar, at.bar_tick), (bar as u64, tick));
        if tick > 0 {
            let before = map.meter_at(tick - 1);
            assert_eq!(
                (before.bar, before.bar_tick),
                (bar as u64 - 1, heads[bar - 1])
            );
        }
    }
    assert_eq!(map.meter_at(three - 1).meter, meter(4, 4));
    assert_eq!(map.meter_at(three).meter, meter(3, 4));
    assert_eq!(map.meter_at(six - 1).meter, meter(3, 4));
    assert_eq!(map.meter_at(six).meter, meter(6, 8));
    assert_eq!(map.meter_at(seven).meter, meter(7, 8));
    assert_eq!(map.meter_at(1_000_000).meter, meter(7, 8));

    let bbt = |tick| {
        let b = map.bar_beat_tick(tick);
        (b.bar, b.beat, b.tick)
    };
    assert_eq!(bbt(0), (1, 1, 0));
    assert_eq!(bbt(three - 1), (2, 4, (Q - 1) as u32));
    assert_eq!(bbt(three), (3, 1, 0));
    assert_eq!(bbt(three + 2 * Q + 123), (3, 3, 123));
    assert_eq!(bbt(six - 1), (4, 3, (Q - 1) as u32));
    assert_eq!(bbt(six), (5, 1, 0));
    // A beat of 6/8 is Q / 2 ticks.
    assert_eq!(bbt(six + Q / 2 + 10), (5, 2, 10));
    assert_eq!(bbt(seven - 1), (5, 6, (Q / 2 - 1) as u32));
    assert_eq!(bbt(seven), (6, 1, 0));
    assert_eq!(
        bbt(seven + 6 * (Q / 2) + Q / 2 - 1),
        (6, 7, (Q / 2 - 1) as u32)
    );
    assert_eq!(bbt(seven + 7 * Q / 2), (7, 1, 0));

    let beat = |tick: f64| {
        let b = map.next_beat(tick);
        (b.tick, b.downbeat)
    };
    assert_eq!(beat(0.0), (0, true));
    assert_eq!(beat(0.001), (QI, false));
    assert_eq!(beat(3.0 * QF + 0.5), (4 * QI, true));
    assert_eq!(beat(three as f64 - 0.5), (three as i64, true));
    assert_eq!(beat(three as f64 + 0.5), ((three + Q) as i64, false));
    // The last beat of the 3/4 bars leads into the first of 6/8.
    assert_eq!(beat(six as f64 - 0.1), (six as i64, true));
    assert_eq!(beat(six as f64 + 1.0), ((six + Q / 2) as i64, false));
    assert_eq!(beat((seven - Q / 2 + 1) as f64), (seven as i64, true));
    assert_eq!(beat(seven as f64 + 1.0), ((seven + Q / 2) as i64, false));
    assert_eq!(
        beat((seven + 7 * Q / 2 - 1) as f64),
        ((seven + 7 * Q / 2) as i64, true)
    );

    // The beats one after another are the heads of the bars exactly where a bar starts.
    let mut tick = 0.0;
    let mut downbeats = Vec::new();
    while tick < (seven + 7 * Q + 80) as f64 {
        let b = map.next_beat(tick);
        if b.downbeat {
            downbeats.push(b.tick as u64);
        }
        tick = b.tick as f64 + 0.5;
    }
    assert_eq!(downbeats, heads);
}

#[test]
fn a_change_put_in_front_moves_ticks_and_keeps_bar_numbers() {
    let before = four_meters();
    let mut after = before.clone();
    after.meter_changes.insert(0, change(1, 5, 4));
    let (old, new) = (TempoMap::new(&before), TempoMap::new(&after));
    // Bar 1 is now 5/4 (5 Q ticks) where it was 4/4 (4 Q).
    for bar in 2..12u64 {
        assert_eq!(new.tick_of_bar(bar), old.tick_of_bar(bar) + Q);
        assert_eq!(
            new.meter_at(new.tick_of_bar(bar)).meter,
            if bar == 1 {
                meter(5, 4)
            } else {
                old.meter_at(old.tick_of_bar(bar)).meter
            }
        );
        assert_eq!(new.meter_at(new.tick_of_bar(bar)).bar, bar);
    }
    // A tick in front of the new change is where it was in time.
    assert_eq!(new.seconds_at(1000.0), old.seconds_at(1000.0));
    // The same tick reads as another place.
    assert_eq!(old.bar_beat_tick(2 * BAR).bar, 3);
    let moved = new.bar_beat_tick(2 * BAR);
    assert_eq!((moved.bar, moved.beat, moved.tick), (2, 5, 0));
}

#[test]
fn the_same_tempo_number_keeps_the_length_of_a_quarter_note() {
    let timeline = Timeline {
        meter_changes: vec![change(1, 6, 8)],
        ..Timeline::new(tempo(9600), Meter::FOUR_FOUR)
    };
    let map = TempoMap::new(&timeline);
    let before = map.seconds_at(QF) - map.seconds_at(0.0);
    let after = map.seconds_at(BARF + QF) - map.seconds_at(BARF);
    assert!((before - 60.0 / 96.0).abs() < 1e-15);
    assert!((after - before).abs() < 1e-15);
    assert_eq!(map.tempo_at(BARF - 1.0), 96.0);
    assert_eq!(map.tempo_at(BARF), 96.0);
    assert_eq!(map.tempo_at(BARF + 80.0), 96.0);
    // The flag of the meter changes nothing in time: the same seconds as with no flag.
    let plain = TempoMap::new(&Timeline::new(tempo(9600), Meter::FOUR_FOUR));
    for tick in [BARF - 1.0, BARF, BARF + 1.0, 3.0 * BARF + 7.0, 1.0e7] {
        assert_eq!(map.seconds_at(tick), plain.seconds_at(tick));
    }
    // A ramp that runs over the change of meter stays one line over ticks.
    let ramp = TempoMap::new(&Timeline {
        head_ramp: true,
        tempo_points: vec![point(2 * BAR, 19200, false)],
        ..timeline
    });
    assert_eq!(ramp.piece_count(), 3);
    assert_eq!(ramp.tempo_at(BARF), 144.0);
    assert!((ramp.tempo_at(BARF - 0.001) - 144.0).abs() < 1e-4);
    assert_eq!(ramp.tempo_at(2.0 * BARF), 192.0);
    // And the time is the time of the same ramp with no flag in it.
    let unflagged = TempoMap::new(&Timeline {
        head_ramp: true,
        tempo_points: vec![point(2 * BAR, 19200, false)],
        ..Timeline::new(tempo(9600), Meter::FOUR_FOUR)
    });
    for tick in [BARF - 1.0, BARF, BARF + 1.0, 2.0 * BARF, 5.0 * BARF] {
        let (a, b) = (ramp.seconds_at(tick), unflagged.seconds_at(tick));
        assert!((a - b).abs() < 1e-12, "{tick}: {a} {b}");
    }
}

#[test]
fn a_bar_of_six_eight_at_120_is_a_second_and_a_half() {
    let map = TempoMap::new(&Timeline::new(tempo(12000), meter(6, 8)));
    for bar in 0..40u64 {
        let seconds = map.seconds_at(map.tick_of_bar(bar) as f64);
        assert!(
            (seconds - 1.5 * bar as f64).abs() < 1e-12,
            "{bar}: {seconds}"
        );
    }
    assert_eq!(map.tick_of_bar(1), 3 * Q);
    assert!((map.tick_at(1.5) - 3.0 * QF).abs() < 1e-9);
    assert_eq!(map.frame_of(3.0 * QF, 0.0, 48_000.0), 72_000);
}

/// Every click from `from` to before `until`: (tick, strong).
fn clicks(map: &TempoMap, from: i64, until: i64) -> Vec<(i64, bool)> {
    let mut out = Vec::new();
    let mut tick = from as f64;
    loop {
        let c = map.next_click(tick);
        if c.tick >= until {
            return out;
        }
        out.push((c.tick, c.downbeat));
        tick = c.tick as f64 + 0.5;
    }
}

#[test]
fn the_clicks_of_each_meter() {
    // (meter, ticks from click to click, clicks in a bar)
    for (num, den, step, per_bar) in [
        (4, 4, Q, 4),
        (3, 4, Q, 3),
        (6, 8, 3 * Q / 2, 2),
        (9, 8, 3 * Q / 2, 3),
        (12, 8, 3 * Q / 2, 4),
        (5, 8, Q / 2, 5),
        (7, 8, Q / 2, 7),
        (2, 2, 2 * Q, 2),
        (3, 8, Q / 2, 3),
    ] {
        let m = meter(num, den);
        assert_eq!((m.ticks_per_click(), m.clicks_per_bar()), (step, per_bar));
        assert_eq!(m.ticks_per_bar(), step * u64::from(per_bar));
        let map = TempoMap::new(&Timeline::new(tempo(12000), m));
        let (step, per_bar) = (step as i64, i64::from(per_bar));
        let want: Vec<(i64, bool)> = (-per_bar..3 * per_bar)
            .map(|k| (k * step, k.rem_euclid(per_bar) == 0))
            .collect();
        assert_eq!(
            clicks(&map, -per_bar * step, 3 * per_bar * step),
            want,
            "{num}/{den}"
        );
        // On a click: that click. Just past it: the next one.
        assert_eq!(map.next_click(step as f64).tick, step, "{num}/{den}");
        assert_eq!(map.next_click(step as f64 + 0.001).tick, 2 * step);
        assert_eq!(map.next_click(step as f64 - 0.001).tick, step);
        // The beats (for snapping and the position) stay the notes of the lower number.
        assert_eq!(map.next_beat(0.5).tick as u64, m.ticks_per_beat());
    }
}

#[test]
fn clicks_over_changes_of_meter_inside_a_ramp() {
    // 4/4, 6/8 ×2, 5/8, 12/8, 2/2; the tempo ramps from the head over all of it.
    let timeline = Timeline {
        head_ramp: true,
        tempo_points: vec![point(40 * Q, 18000, false)],
        meter_changes: vec![
            change(1, 6, 8),
            change(3, 5, 8),
            change(4, 12, 8),
            change(5, 2, 2),
        ],
        ..Timeline::new(tempo(6000), Meter::FOUR_FOUR)
    };
    let map = TempoMap::new(&timeline);
    let (six, five, twelve, two) = (
        BAR,
        BAR + 6 * Q,
        BAR + 6 * Q + 5 * Q / 2,
        4 * BAR + 5 * Q / 2,
    );
    assert_eq!(map.tick_of_bar(1), six);
    assert_eq!(map.tick_of_bar(3), five);
    assert_eq!(map.tick_of_bar(4), twelve);
    assert_eq!(map.tick_of_bar(5), two);
    let mut want: Vec<(u64, bool)> = (0..4).map(|k| (k * Q, k == 0)).collect();
    want.extend((0..4).map(|k| (six + k * 3 * Q / 2, k % 2 == 0)));
    want.extend((0..5).map(|k| (five + k * Q / 2, k == 0)));
    want.extend((0..4).map(|k| (twelve + k * 3 * Q / 2, k == 0)));
    want.extend((0..4).map(|k| (two + k * 2 * Q, k % 2 == 0)));
    let want: Vec<(i64, bool)> = want.into_iter().map(|(t, d)| (t as i64, d)).collect();
    assert_eq!(clicks(&map, 0, (two + 2 * BAR) as i64), want);
    // The last click of a meter leads into the first of the next.
    assert_eq!(map.next_click(five as f64 - 0.1).tick, five as i64);
    assert_eq!(map.next_click(twelve as f64 - 0.1).tick, twelve as i64);
    assert!(map.next_click(two as f64 - 0.1).downbeat);

    // The flags do not bend the line: the tempo and the time are those of the bare ramp.
    let bare = TempoMap::new(&Timeline {
        meter_changes: Vec::new(),
        ..timeline
    });
    for &(tick, _) in &want {
        let tick = tick as f64;
        assert!((map.tempo_at(tick) - bare.tempo_at(tick)).abs() < 1e-12);
        assert!((map.seconds_at(tick) - bare.seconds_at(tick)).abs() < 1e-12);
    }
    assert!((map.tempo_at(20.0 * QF) - 120.0).abs() < 1e-12);
    // Clicks come closer in time as the tempo rises, also where the step stays the same.
    let gap = |a: u64, b: u64| map.seconds_at(b as f64) - map.seconds_at(a as f64);
    assert!(gap(six, six + 3 * Q / 2) > gap(six + 3 * Q / 2, six + 3 * Q));
}

// ---- The table for the screen's tempo map ----

fn json_timeline(t: &Timeline) -> String {
    let points: Vec<String> = t
        .tempo_points
        .iter()
        .map(|p| {
            format!(
                "{{ \"tick\": {}, \"tempo\": {:?}, \"ramp\": {} }}",
                p.tick,
                p.tempo.bpm(),
                p.ramp
            )
        })
        .collect();
    let changes: Vec<String> = t
        .meter_changes
        .iter()
        .map(|c| {
            format!(
                "{{ \"bar\": {}, \"meter\": [{}, {}] }}",
                c.bar,
                c.meter.num(),
                c.meter.den()
            )
        })
        .collect();
    format!(
        "{{\n        \"tempo\": {:?},\n        \"meter\": [{}, {}],\n        \"headRamp\": {},\n        \
         \"tempoPoints\": [{}],\n        \"meterChanges\": [{}]\n      }}",
        t.tempo.bpm(),
        t.meter.num(),
        t.meter.den(),
        t.head_ramp,
        points.join(", "),
        changes.join(", ")
    )
}

fn table_maps() -> Vec<(&'static str, Timeline)> {
    vec![
        (
            "spec example: 96, 104 at bar 5 with a ramp, 120 at bar 9",
            spec_example(),
        ),
        (
            "meters 4/4, 3/4, 6/8, 7/8 with tempo points and a ramp over a change of meter",
            Timeline {
                head_ramp: false,
                tempo_points: vec![
                    point(2 * Q + 40, 12050, false),
                    point(6 * Q + 120, 8000, true),
                    point(14 * Q + 280, 16000, true),
                    point(18 * Q + 361, 6025, true),
                ],
                ..four_meters()
            },
        ),
        (
            "slopes close to zero: 120.00 to 120.01 over a long way, then 120.01 to 120.00",
            Timeline {
                head_ramp: true,
                tempo_points: vec![
                    point(50_000_000, 12001, true),
                    point(50_000_000 + Q, 12000, true),
                ],
                ..Timeline::new(tempo(12000), Meter::FOUR_FOUR)
            },
        ),
        (
            "4/4 to 6/8 to 2/2 with one tempo number, then a ramp down from the head of 2/2",
            Timeline {
                tempo_points: vec![
                    point(BAR + 3 * Q, 9600, true),
                    point(BAR + 3 * Q + 2 * BAR, 3000, false),
                ],
                meter_changes: vec![change(1, 6, 8), change(2, 2, 2)],
                ..Timeline::new(tempo(9600), Meter::FOUR_FOUR)
            },
        ),
        (
            "a ramp from the head in 12/16, fast to slow",
            Timeline {
                head_ramp: true,
                tempo_points: vec![point(3 * Q / 2 + 57, 3000, false)],
                ..Timeline::new(tempo(30000), meter(12, 16))
            },
        ),
        (
            "6/8, 9/8, 12/8, 5/8, 7/8, 2/2, 3/8, two bars each, under one ramp up and one down",
            Timeline {
                head_ramp: true,
                tempo_points: vec![
                    point(20 * Q + 77, 21000, true),
                    point(47 * Q + 1, 4550, false),
                ],
                meter_changes: vec![
                    change(2, 9, 8),
                    change(4, 12, 8),
                    change(6, 5, 8),
                    change(8, 7, 8),
                    change(10, 2, 2),
                    change(12, 3, 8),
                ],
                ..Timeline::new(tempo(12000), meter(6, 8))
            },
        ),
    ]
}

fn table_json() -> String {
    let mut out = String::from(
        "{\n  \"note\": \"Written by `cargo test -p meloet-model --test tempo_map`. Do not edit this file manually. \
         tempo: quarter notes per minute, whatever the meter. bar in meterChanges counts from 0; barBeatTick is as shown (bar and beat from 1, the tick as many ticks into the beat). clicks: the first click of the metronome at the tick or after it (next), and whether it is the strong one (downbeat).\",\n  \"maps\": [\n",
    );
    let maps = table_maps();
    for (i, (name, timeline)) in maps.iter().enumerate() {
        assert_eq!(timeline.check(), Ok(()));
        let map = TempoMap::new(timeline);
        let cuts = cuts(timeline, &map);
        let mut ticks: Vec<f64> = vec![
            -BARF,
            -0.5,
            0.0,
            0.25,
            1.0,
            QF - 1.0,
            QF,
            BARF - 0.5,
            6.0 * BARF,
            8.0 * BARF - 0.001,
            20000.0,
            123456.789,
            1.0e7,
            5.0e7,
            3.0e8,
        ];
        for &cut in &cuts {
            let cut = cut as f64;
            ticks.extend([cut - 1.0, cut, cut + 1.0]);
        }
        ticks.retain(|&t| t >= 0.0 || t == -BARF || t == -0.5);
        ticks.sort_by(f64::total_cmp);
        ticks.dedup();
        let seconds = [
            -2.0, -0.001, 0.0, 0.001, 0.5, 1.0, 2.5, 5.0, 10.0, 12.5, 18.586, 20.0, 33.3, 60.0,
            600.0, 7200.0,
        ];
        let _ = write!(
            out,
            "    {{\n      \"name\": \"{name}\",\n      \"timeline\": {},\n      \"atTicks\": [\n",
            json_timeline(timeline)
        );
        for (j, &tick) in ticks.iter().enumerate() {
            let _ = writeln!(
                out,
                "        {{ \"tick\": {:?}, \"seconds\": {:?}, \"tempo\": {:?} }}{}",
                tick,
                map.seconds_at(tick),
                map.tempo_at(tick),
                if j + 1 < ticks.len() { "," } else { "" }
            );
        }
        out.push_str("      ],\n      \"atSeconds\": [\n");
        for (j, &s) in seconds.iter().enumerate() {
            let _ = writeln!(
                out,
                "        {{ \"seconds\": {:?}, \"tick\": {:?} }}{}",
                s,
                map.tick_at(s),
                if j + 1 < seconds.len() { "," } else { "" }
            );
        }
        out.push_str("      ],\n      \"positions\": [\n");
        let whole: Vec<u64> = ticks
            .iter()
            .filter(|t| **t >= 0.0 && t.fract() == 0.0)
            .map(|t| *t as u64)
            .collect();
        for (j, &tick) in whole.iter().enumerate() {
            let b = map.bar_beat_tick(tick);
            let _ = writeln!(
                out,
                "        {{ \"tick\": {}, \"barBeatTick\": [{}, {}, {}] }}{}",
                tick,
                b.bar,
                b.beat,
                b.tick,
                if j + 1 < whole.len() { "," } else { "" }
            );
        }
        out.push_str("      ],\n      \"clicks\": [\n");
        let mut click_ticks = ticks.clone();
        for &cut in &cuts {
            click_ticks.extend((1..=12).map(|k| (cut + k * Q / 2) as f64 - 0.5));
        }
        click_ticks.sort_by(f64::total_cmp);
        click_ticks.dedup();
        for (j, &tick) in click_ticks.iter().enumerate() {
            let c = map.next_click(tick);
            let _ = writeln!(
                out,
                "        {{ \"tick\": {:?}, \"next\": {}, \"downbeat\": {} }}{}",
                tick,
                c.tick,
                c.downbeat,
                if j + 1 < click_ticks.len() { "," } else { "" }
            );
        }
        let _ = writeln!(
            out,
            "      ]\n    }}{}",
            if i + 1 < maps.len() { "," } else { "" }
        );
    }
    out.push_str("  ]\n}\n");
    out
}

/// Writes `ui/src/time/tempoMap.table.json`, as the protocol's tests write
/// `ui/src/bridge/generated/`: a table that is not the committed one shows up in `git status`.
#[test]
fn write_the_table_for_the_screen() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/time/tempoMap.table.json");
    let table = table_json();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(table.as_str()) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &table).unwrap();
    }
    // Every number came out as JSON can read it.
    assert!(!table.contains("NaN") && !table.contains("inf"));
}

// ---- Cost ----

fn many_pieces(count: u64) -> Timeline {
    Timeline {
        head_ramp: true,
        tempo_points: (1..count)
            .map(|i| point(i * Q, 6000 + (i as u32 * 977) % 18000, i % 2 == 0))
            .collect(),
        ..Timeline::new(tempo(9600), Meter::FOUR_FOUR)
    }
}

/// `cargo test -p meloet-model --release --test tempo_map -- --ignored --nocapture measure`
#[test]
#[ignore = "a measurement, not a check"]
fn measure_cost() {
    use std::hint::black_box;
    use std::time::Instant;
    const N: u32 = 5_000_000;
    for count in [1u64, 10, 1000] {
        let map = TempoMap::new(&many_pieces(count));
        assert_eq!(map.piece_count() as u64, count);
        let span = count * Q + 2000;
        let end = map.seconds_at(span as f64);
        let mut rng = Rng(count);
        let ticks: Vec<f64> = (0..4096).map(|_| rng.unit() * span as f64).collect();
        let secs: Vec<f64> = (0..4096).map(|_| rng.unit() * end).collect();
        let time = |name: &str, f: &dyn Fn(usize) -> f64| {
            let mut best = f64::INFINITY;
            for _ in 0..5 {
                let start = Instant::now();
                let mut sum = 0.0;
                for i in 0..N {
                    sum += f(i as usize & 4095);
                }
                black_box(sum);
                best = best.min(start.elapsed().as_secs_f64() * 1e9 / f64::from(N));
            }
            println!("{count:>5} pieces  {name:<11} {best:6.1} ns");
        };
        time("seconds_at", &|i| map.seconds_at(black_box(ticks[i])));
        time("tick_at", &|i| map.tick_at(black_box(secs[i])));
        time("frame_of", &|i| {
            map.frame_of(black_box(ticks[i]), 0.0, 48_000.0) as f64
        });
        time("tempo_at", &|i| map.tempo_at(black_box(ticks[i])));
        time("next_beat", &|i| {
            map.next_beat(black_box(ticks[i])).tick as f64
        });
    }
}
