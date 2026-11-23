//! The arpeggio without a device: the frame every note starts and ends on, what is written
//! down as played, and that no note is left sounding.

mod common;

use std::sync::Arc;

use meloet_engine::{
    BlockTiming, Engine, EngineCommand, EngineConfig, EngineParts, Instrument, InstrumentEvent,
    MidiIn, PlaceholderSynth, RecentAt, Recorded, Sequence, SequenceEvent, TrackSet,
};
use meloet_model::{
    Arp, ArpRate, ArpShape, Meter, MeterChange, Tempo, TempoMap, TempoPoint, Timeline,
};
use rtrb::{Consumer, Producer, RingBuffer};

const RATE: u64 = 48_000;
const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
/// Ticks × this = the engine's exact position unit.
const SCALE: i128 = 6000 * RATE as i128;
const BLOCKS: [usize; 7] = [32, 64, 128, 256, 512, 1024, 2048];
const TEMPOS: [u32; 4] = [3_000, 9_600, 13_750, 30_000];

type Ev = (u64, InstrumentEvent);

/// Writes down every event with the frame (counted from the first render) it took effect on.
struct Tape {
    tx: Producer<Ev>,
    frames: u64,
}

impl Instrument for Tape {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn event(&mut self, ev: InstrumentEvent) {
        self.tx.push((self.frames, ev)).expect("tape full");
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        self.frames += left.len() as u64;
    }
    fn reset(&mut self) {}
}

struct Rig {
    parts: EngineParts,
    block: usize,
    /// Frames rendered so far.
    frames: u64,
    base_us: u64,
    delay_us: u64,
    /// Keys not yet arrived, by `t_us`.
    keys: Vec<MidiIn>,
    out: Vec<f32>,
}

impl Rig {
    fn new(block: usize) -> Self {
        Rig {
            parts: Engine::new(EngineConfig::default()),
            block,
            frames: 0,
            base_us: 1_000_000,
            delay_us: 13_000,
            keys: Vec::new(),
            out: vec![0.0; block * 2],
        }
    }

    /// A tape on `track` that counts frames from `self.frames` on.
    fn tape(&mut self, track: usize) -> Consumer<Ev> {
        let (tx, rx) = RingBuffer::new(1 << 16);
        let tape = Tape {
            tx,
            frames: self.frames,
        };
        let sent = self.parts.handle.swap_instrument(track, Box::new(tape));
        assert!(sent.is_ok());
        rx
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    fn arp(&mut self, track: usize, arp: Arp) {
        self.send(EngineCommand::SetArp { track, arp });
    }

    fn tempo(&mut self, bpm_centi: u32) {
        let map = common::flat_map(bpm_centi, 4, 4);
        assert!(self.parts.handle.set_tempo_map(map).is_ok());
    }

    fn now_us(&self) -> u64 {
        self.base_us + (self.frames as f64 * 1e6 / RATE as f64) as u64
    }

    /// When frame `n` (counted from the first render) reaches the DAC.
    fn dac_us(&self, n: u64) -> f64 {
        self.base_us as f64 + self.delay_us as f64 + n as f64 * 1e6 / RATE as f64
    }

    /// A key played now: the next block takes it.
    fn key(&mut self, event: InstrumentEvent) {
        let t_us = self.now_us();
        self.keys.push(MidiIn { event, t_us });
    }

    /// Renders whole device blocks until at least `frames` more frames are done.
    fn run(&mut self, frames: u64) {
        let end = self.frames + frames;
        while self.frames < end {
            let callback_us = self.now_us();
            while self.keys.first().is_some_and(|k| k.t_us <= callback_us) {
                let key = self.keys.remove(0);
                self.parts.midi_in.push(key).expect("midi ring full");
            }
            let timing = BlockTiming {
                callback_us,
                playback_us: callback_us + self.delay_us,
            };
            self.parts.engine.process(&mut self.out, timing);
            self.frames += self.block as u64;
        }
    }

    fn recorded(&mut self) -> Vec<Recorded> {
        std::iter::from_fn(|| self.parts.handle.pop_recorded()).collect()
    }
}

fn drain(tape: &mut Consumer<Ev>) -> Vec<Ev> {
    std::iter::from_fn(|| tape.pop().ok()).collect()
}

fn on(note: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOn {
        note,
        velocity: 100,
    }
}

fn off(note: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOff { note }
}

fn arp(shape: ArpShape, rate: ArpRate) -> Arp {
    Arp {
        on: true,
        shape,
        rate,
        ..Arp::default()
    }
}

fn step_of(set: &Arp) -> u64 {
    let step = 4 * Q / u64::from(set.rate.division());
    if set.triplet { step * 2 / 3 } else { step }
}

/// The first frame whose position is at or past `tick`, for a run that starts at `from` on
/// frame 0 (exact: the position moves by bpm_centi × ticks_per_beat / SCALE ticks a frame).
fn frame_of(tick: u64, from: u64, bpm_centi: u32) -> u64 {
    let step = i128::from(bpm_centi) * Q as i128;
    let gap = (tick as i128 - from as i128) * SCALE;
    if gap <= 0 {
        0
    } else {
        ((gap + step - 1) / step) as u64
    }
}

/// The notes that still sound at the end of `events`.
fn left_sounding(events: &[Ev]) -> Vec<u8> {
    let mut down = [false; 128];
    for (_, ev) in events {
        match *ev {
            InstrumentEvent::NoteOn { note, velocity } => down[usize::from(note)] = velocity > 0,
            InstrumentEvent::NoteOff { note } => down[usize::from(note)] = false,
            InstrumentEvent::AllNotesOff => down = [false; 128],
            _ => {}
        }
    }
    (0..128u8).filter(|&n| down[usize::from(n)]).collect()
}

fn note_ons(events: &[Ev]) -> Vec<(u64, u8)> {
    (events.iter())
        .filter_map(|&(frame, ev)| match ev {
            InstrumentEvent::NoteOn { note, .. } => Some((frame, note)),
            _ => None,
        })
        .collect()
}

/// What the tape must say for `steps` steps of a chord of C E G going up: the steps are at
/// `grid`, each note ends after 80 % of `step` or where the next step comes first.
fn up_tape(grid: &[u64], step: u64, steps: usize, frame: impl Fn(u64) -> u64) -> Vec<Ev> {
    let chord = [60, 64, 67];
    let mut want = Vec::new();
    for k in 0..steps {
        let note = chord[k % 3];
        want.push((frame(grid[k]), on(note)));
        want.push((
            frame((grid[k] + step * 80 / 100).min(grid[k + 1])),
            off(note),
        ));
    }
    want
}

#[test]
fn every_step_starts_on_the_frame_of_its_place_on_the_grid() {
    let rates: Vec<(ArpRate, bool)> = (ArpRate::ALL.into_iter())
        .flat_map(|r| [(r, false), (r, true)])
        .collect();
    let froms = [0, 700, BAR - 1, 3 * BAR + 480];
    let mut n = 0;
    for block in BLOCKS {
        for bpm in TEMPOS {
            // Every rate at one block size and tempo, a few of them at the others.
            let all = block == 256 && bpm == 13_750;
            for (i, &(rate, triplet)) in rates.iter().enumerate() {
                n += 1;
                if !all && (n + i) % 5 != 0 {
                    continue;
                }
                let from = froms[n % froms.len()];
                let set = Arp {
                    triplet,
                    ..arp(ArpShape::Up, rate)
                };
                let step = step_of(&set);
                let mut rig = Rig::new(block);
                let mut tape = rig.tape(0);
                rig.tempo(bpm);
                rig.arp(0, set);
                rig.send(EngineCommand::Play { from });
                for note in [64, 60, 67] {
                    rig.key(on(note));
                }
                let steps = 7;
                let first = from.div_ceil(step) * step;
                let grid: Vec<u64> = (0..=steps as u64).map(|k| first + k * step).collect();
                rig.run(frame_of(grid[steps], from, bpm) + 1);
                let got = drain(&mut tape);
                let want = up_tape(&grid, step, steps, |tick| frame_of(tick, from, bpm));
                assert_eq!(
                    got[..want.len()],
                    want[..],
                    "block {block}, {bpm}, {rate:?} triplet {triplet}, from {from}"
                );
            }
        }
    }
}

#[test]
fn the_grid_starts_again_at_every_bar_over_tempo_slopes_and_meter_changes() {
    // 3/4 for two bars, then 7/8; the tempo goes up in a line, then jumps down.
    let timeline = Timeline {
        tempo: Tempo::from_centi(9_000).unwrap(),
        meter: Meter::new(3, 4).unwrap(),
        head_ramp: true,
        tempo_points: vec![
            TempoPoint {
                tick: 2 * Q,
                tempo: Tempo::from_centi(14_000).unwrap(),
                ramp: true,
            },
            TempoPoint {
                tick: 7 * Q,
                tempo: Tempo::from_centi(7_000).unwrap(),
                ramp: false,
            },
        ],
        meter_changes: vec![MeterChange {
            bar: 2,
            meter: Meter::new(7, 8).unwrap(),
        }],
    };
    let map = Arc::new(TempoMap::new(&timeline));
    for (rate, triplet) in [
        (ArpRate::Quarter, true),
        (ArpRate::Quarter, false),
        (ArpRate::Sixteenth, true),
    ] {
        let set = Arp {
            triplet,
            ..arp(ArpShape::Up, rate)
        };
        let step = step_of(&set);
        // The steps of five bars, each bar counted from its own head.
        let mut grid = Vec::new();
        for bar in 0..5u64 {
            let (head, next) = (map.tick_of_bar(bar), map.tick_of_bar(bar + 1));
            grid.extend((0..).map(|k| head + k * step).take_while(|&t| t < next));
        }
        if rate == ArpRate::Quarter && triplet {
            // A step of 1280 does not fit 3/4 (5760) or 7/8 (6720): the last one is short.
            let bar2 = 2 * 5760;
            assert_eq!(grid[3..7], [3840, 5120, 5760, 7040]);
            assert!(grid.contains(&bar2) && grid.contains(&(bar2 + 6720)));
        }
        for block in [32, 333, 2048] {
            for from in [0u64, 2 * Q + 7] {
                let mut rig = Rig::new(block);
                let mut tape = rig.tape(0);
                assert!(rig.parts.handle.set_tempo_map(map.clone()).is_ok());
                rig.arp(0, set);
                rig.send(EngineCommand::Play { from });
                for note in [60, 64, 67] {
                    rig.key(on(note));
                }
                let grid: Vec<u64> = grid.iter().copied().filter(|&t| t >= from).collect();
                let steps = grid.len() - 1;
                let from_sec = map.seconds_at(from as f64);
                let frame = |tick: u64| map.frame_of(tick as f64, from_sec, RATE as f64);
                rig.run(frame(grid[steps]) + 1);
                let got = drain(&mut tape);
                let want = up_tape(&grid, step, steps, frame);
                assert_eq!(
                    got[..want.len()],
                    want[..],
                    "block {block}, {rate:?} triplet {triplet}, from {from}"
                );
            }
        }
    }
}

#[test]
fn the_shapes_and_the_octaves_reach_the_instrument() {
    let chord = [60u8, 64, 67];
    let cases: [(ArpShape, u8, &[u8]); 5] = [
        (ArpShape::Up, 1, &[60, 64, 67, 60, 64, 67, 60, 64]),
        (ArpShape::Down, 1, &[67, 64, 60, 67, 64, 60, 67, 64]),
        (ArpShape::UpDown, 1, &[60, 64, 67, 64, 60, 64, 67, 64]),
        (ArpShape::Up, 2, &[60, 64, 67, 72, 76, 79, 60, 64]),
        (ArpShape::Down, 3, &[91, 88, 84, 79, 76, 72, 67, 64]),
    ];
    for (shape, octaves, want) in cases {
        let mut rig = Rig::new(256);
        let mut tape = rig.tape(0);
        let set = Arp {
            octaves,
            ..arp(shape, ArpRate::ThirtySecond)
        };
        rig.arp(0, set);
        rig.send(EngineCommand::Play { from: 0 });
        for note in chord {
            rig.key(on(note));
        }
        rig.run(RATE);
        let got: Vec<u8> = note_ons(&drain(&mut tape)).iter().map(|n| n.1).collect();
        assert_eq!(got[..8], want[..], "{shape:?} {octaves}");
    }
    // 「ばらばら」: only notes of the chord and its octaves, never one twice in a row.
    let mut rig = Rig::new(256);
    let mut tape = rig.tape(0);
    let set = Arp {
        octaves: 2,
        ..arp(ArpShape::Random, ArpRate::ThirtySecond)
    };
    rig.arp(0, set);
    rig.send(EngineCommand::Play { from: 0 });
    for note in chord {
        rig.key(on(note));
    }
    rig.run(4 * RATE);
    let got: Vec<u8> = note_ons(&drain(&mut tape)).iter().map(|n| n.1).collect();
    assert!(got.len() >= 60);
    assert!(got.iter().all(|n| [60, 64, 67, 72, 76, 79].contains(n)));
    assert!(got.windows(2).all(|w| w[0] != w[1]));
    for note in [60, 64, 67, 72, 76, 79] {
        assert!(got.contains(&note), "{note} never came");
    }
}

#[test]
fn one_key_repeats_with_its_own_velocity_and_sixteen_keys_all_play() {
    let mut rig = Rig::new(128);
    let mut tape = rig.tape(0);
    rig.arp(0, arp(ArpShape::Up, ArpRate::Sixteenth));
    rig.send(EngineCommand::Play { from: 0 });
    rig.key(InstrumentEvent::NoteOn {
        note: 50,
        velocity: 37,
    });
    rig.run(RATE);
    let got = drain(&mut tape);
    let want: Vec<Ev> = (0..8u64)
        .flat_map(|k| {
            let hit = InstrumentEvent::NoteOn {
                note: 50,
                velocity: 37,
            };
            [(k * 6000, hit), (k * 6000 + 4800, off(50))]
        })
        .collect();
    assert_eq!(got[..want.len()], want[..]);

    let mut rig = Rig::new(128);
    let mut tape = rig.tape(0);
    rig.arp(0, arp(ArpShape::Up, ArpRate::ThirtySecond));
    rig.send(EngineCommand::Play { from: 0 });
    for k in 0..18u8 {
        rig.key(on(40 + k));
    }
    rig.run(2 * RATE);
    let got: Vec<u8> = note_ons(&drain(&mut tape)).iter().map(|n| n.1).collect();
    let want: Vec<u8> = (40..56).chain(40..56).collect();
    assert_eq!(got[..32], want[..]);
}

#[test]
fn standing_still_the_first_key_sounds_at_once_and_the_steps_follow_the_tempo() {
    for (block, bpm) in [(64, 9_600u32), (256, 13_750), (1024, 3_000), (32, 30_000)] {
        let mut rig = Rig::new(block);
        let mut tape = rig.tape(0);
        rig.tempo(bpm);
        let set = arp(ArpShape::Up, ArpRate::Eighth);
        rig.arp(0, set);
        rig.run(3 * block as u64);
        let start = rig.frames;
        rig.key(on(60));
        rig.key(on(64));
        // Frames of one step, with the fraction.
        let step = (Q / 2) as f64 / (f64::from(bpm) / 100.0 * 32.0) * RATE as f64;
        let steps = 6u64;
        rig.run((step * steps as f64) as u64 + 1);
        let got = drain(&mut tape);
        let at = |x: f64| start + (x - 1e-6).ceil() as u64;
        let want: Vec<Ev> = (0..steps)
            .flat_map(|k| {
                let note = [60, 64][k as usize % 2];
                let from = k as f64 * step;
                [(at(from), on(note)), (at(from + step * 0.8), off(note))]
            })
            .collect();
        assert_eq!(got[..want.len()], want[..], "block {block}, {bpm}");

        // Written down with the time each note is heard: kept afterwards, the notes are
        // half a tick or less from the steps.
        let items = rig.recorded();
        let per_us = f64::from(bpm) / 100.0 / 60.0 * Q as f64 / 1e6;
        let mut first = None;
        let mut k = 0;
        for item in &items {
            let Recorded::Recent {
                event, t_us, at, ..
            } = *item
            else {
                panic!("{item:?}");
            };
            assert_eq!(at, None);
            if let InstrumentEvent::NoteOn { .. } = event {
                let first = *first.get_or_insert(t_us);
                let tick = (t_us - first) as f64 * per_us;
                let want = k as f64 * (Q / 2) as f64;
                assert!((tick - want).abs() < 0.5, "{tick} for {want}");
                let heard = rig.dac_us(got[2 * k].0);
                assert!((t_us as f64 - heard).abs() <= 1.5);
                k += 1;
            }
        }
        assert!(k as u64 >= steps);
    }
}

#[test]
fn a_recording_gets_the_notes_at_their_grid_ticks_and_not_the_keys() {
    for block in [64, 480, 2048] {
        let mut rig = Rig::new(block);
        let mut tape = rig.tape(0);
        rig.arp(0, arp(ArpShape::Up, ArpRate::Sixteenth));
        rig.send(EngineCommand::Record {
            from: BAR,
            count_in_beats: 4,
            muted: TrackSet::default(),
        });
        rig.key(on(60));
        rig.key(on(67));
        // The count-in and two bars, at 120; the pedal goes down in the first bar.
        rig.run(3 * RATE);
        rig.key(InstrumentEvent::Control {
            controller: 64,
            value: 127,
        });
        rig.run(3 * RATE);
        let got = drain(&mut tape);
        // It sounds through the count-in, on the grid that leads to the start.
        let ons = note_ons(&got);
        assert_eq!(ons[..3], [(0, 60), (6000, 67), (12000, 60)]);
        assert_eq!(ons[16], (16 * 6000, 60));

        let items = rig.recorded();
        let mut notes = Vec::new();
        let mut pedal = 0;
        for item in &items {
            match *item {
                Recorded::Event {
                    event: InstrumentEvent::Control { .. },
                    ..
                } => pedal += 1,
                Recorded::Event {
                    event, tick, lap, ..
                } => notes.push((tick, lap, event)),
                _ => {}
            }
        }
        // The pedal goes through as it is; the keys held are in no item.
        assert_eq!(pedal, 1);
        let want: Vec<(f64, u32, InstrumentEvent)> = (0..32u64)
            .flat_map(|k| {
                let note = [60, 67][k as usize % 2];
                let tick = (BAR + k * 480) as f64;
                [(tick, 0, on(note)), (tick + 384.0, 0, off(note))]
            })
            .collect();
        assert_eq!(notes[..60], want[..60], "block {block}");
        // Each is said with the time it is heard.
        let heard: Vec<u64> = (items.iter())
            .filter_map(|r| match *r {
                Recorded::Event {
                    event: InstrumentEvent::NoteOn { .. },
                    t_us,
                    ..
                } => Some(t_us),
                _ => None,
            })
            .collect();
        let first_frame = 2 * RATE;
        assert!((heard[0] as f64 - rig.dac_us(first_frame)).abs() <= 1.5);
        assert!((heard[5] as f64 - rig.dac_us(first_frame + 5 * 6000)).abs() <= 1.5);
    }
}

#[test]
fn round_a_loop_the_note_is_cut_at_the_end_and_the_grid_starts_from_the_loop() {
    for recording in [false, true] {
        for block in [64, 1000] {
            let mut rig = Rig::new(block);
            let mut tape = rig.tape(0);
            rig.arp(0, arp(ArpShape::Up, ArpRate::Quarter));
            // Two and a half beats: the third step would sound past the loop's end.
            rig.send(EngineCommand::SetLoop {
                start: Q,
                end: 3 * Q + Q / 2,
                on: true,
            });
            if recording {
                rig.send(EngineCommand::Record {
                    from: Q,
                    count_in_beats: 0,
                    muted: TrackSet::default(),
                });
            } else {
                rig.send(EngineCommand::Play { from: Q });
            }
            rig.key(on(60));
            rig.run(3 * RATE);
            let got = drain(&mut tape);
            // A beat is 24000 frames, the loop 60000.
            let want = [
                (0, on(60)),
                (19_200, off(60)),
                (24_000, on(60)),
                (43_200, off(60)),
                (48_000, on(60)),
                (60_000, off(60)),
                (60_000, on(60)),
                (79_200, off(60)),
                (84_000, on(60)),
            ];
            assert_eq!(got[..want.len()], want[..], "block {block}");

            let places: Vec<(f64, u32, bool)> = (rig.recorded().iter())
                .filter_map(|r| match *r {
                    Recorded::Event {
                        event, tick, lap, ..
                    } if recording => Some((tick, lap, event)),
                    Recorded::Recent {
                        event,
                        at: Some(RecentAt { tick, lap }),
                        ..
                    } if !recording => Some((tick, lap, event)),
                    Recorded::Recent { .. } => panic!("no place: {r:?}"),
                    _ => None,
                })
                .map(|(tick, lap, event)| {
                    let is_on = matches!(event, InstrumentEvent::NoteOn { .. });
                    (tick, lap, is_on)
                })
                .collect();
            let q = Q as f64;
            let want = [
                (q, 0, true),
                (q + 1536.0, 0, false),
                (2.0 * q, 0, true),
                (2.0 * q + 1536.0, 0, false),
                (3.0 * q, 0, true),
                // Cut where the loop went round: said at the loop's start, a pass later.
                (q, 1, false),
                (q, 1, true),
                (q + 1536.0, 1, false),
            ];
            assert_eq!(places[..want.len()], want[..], "block {block}");
        }
    }
}

#[test]
fn keys_that_join_and_leave_change_the_pattern_at_the_next_step() {
    let mut rig = Rig::new(64);
    let mut tape = rig.tape(0);
    rig.arp(0, arp(ArpShape::Up, ArpRate::Sixteenth));
    rig.send(EngineCommand::Play { from: 0 });
    rig.key(on(60));
    // Steps are 6000 frames apart.
    rig.run(13_000);
    rig.key(on(64));
    rig.key(on(67));
    rig.run(18_000);
    rig.key(off(60));
    rig.run(12_000);
    rig.key(off(64));
    rig.key(off(67));
    rig.run(12_000);
    let got = drain(&mut tape);
    assert_eq!(
        note_ons(&got),
        [
            (0, 60),
            (6000, 60),
            (12000, 60),
            // Step 3 of three keys, then on.
            (18000, 60),
            (24000, 64),
            (30000, 67),
            // Step 6 of two keys.
            (36000, 64),
            (42000, 67),
        ]
    );
    // The last note still ends after its 80 %.
    assert_eq!(got.last(), Some(&(46_800, off(67))));
    assert_eq!(left_sounding(&got), [] as [u8; 0]);
}

#[test]
fn hold_keeps_the_chord_until_a_new_one_or_until_it_is_turned_off() {
    let held = Arp {
        hold: true,
        ..arp(ArpShape::Up, ArpRate::Sixteenth)
    };
    let mut rig = Rig::new(64);
    let mut tape = rig.tape(0);
    rig.arp(0, held);
    rig.send(EngineCommand::Play { from: 0 });
    rig.key(on(60));
    rig.key(on(64));
    rig.run(640);
    rig.key(off(60));
    rig.run(640);
    // One key is still down: this one joins the chord.
    rig.key(on(67));
    rig.run(640);
    rig.key(off(64));
    rig.key(off(67));
    // Whole blocks, up to just before the seventh step.
    rig.run(35_840 - 3 * 640);
    let got = drain(&mut tape);
    let ons: Vec<u8> = note_ons(&got).iter().map(|n| n.1).collect();
    assert_eq!(ons, [60, 64, 67, 60, 64, 67]);

    // Every key was let go: the next key starts a new chord, from its first note.
    rig.key(on(72));
    rig.run(640);
    rig.key(on(76));
    rig.key(off(72));
    rig.run(640);
    rig.key(off(76));
    rig.run(24_000 - 2 * 640);
    let ons = note_ons(&drain(&mut tape));
    assert_eq!(
        ons,
        [(36_000, 72), (42_000, 76), (48_000, 72), (54_000, 76)]
    );

    // 「続ける」 off with a key still down: only that key goes on.
    rig.key(on(79));
    rig.run(640);
    rig.arp(
        0,
        Arp {
            hold: false,
            ..held
        },
    );
    rig.run(12_000 - 640);
    let ons: Vec<u8> = note_ons(&drain(&mut tape)).iter().map(|n| n.1).collect();
    assert_eq!(ons, [79, 79]);
    rig.key(off(79));
    rig.run(12_000);
    assert_eq!(drain(&mut tape), []);

    // 「続ける」 off with no key down: silent at once.
    rig.arp(0, held);
    rig.key(on(60));
    rig.run(640);
    rig.key(off(60));
    rig.run(640);
    let at = rig.frames;
    rig.arp(
        0,
        Arp {
            hold: false,
            ..held
        },
    );
    rig.run(24_000);
    let got = drain(&mut tape);
    assert_eq!(got.last(), Some(&(at, off(60))));
    assert_eq!(left_sounding(&got), [] as [u8; 0]);
}

/// A held chord that runs, on track 0, with the song playing or not.
fn running(playing: bool) -> (Rig, Consumer<Ev>) {
    let mut rig = Rig::new(64);
    let tape = rig.tape(0);
    let set = Arp {
        hold: true,
        ..arp(ArpShape::Up, ArpRate::Sixteenth)
    };
    rig.arp(0, set);
    if playing {
        rig.send(EngineCommand::Play { from: 0 });
    }
    rig.key(on(60));
    rig.key(on(64));
    rig.run(640);
    rig.key(off(60));
    rig.key(off(64));
    // Into the second note.
    rig.run(6400);
    (rig, tape)
}

#[test]
fn nothing_is_left_sounding_whatever_ends_the_arpeggio() {
    type End = fn(&mut Rig);
    let ends: [(&str, End); 7] = [
        ("off", |rig| rig.arp(0, Arp::default())),
        ("another target", |rig| {
            rig.send(EngineCommand::SetPlayTarget { track: 1 });
        }),
        ("no target", |rig| rig.send(EngineCommand::ClearPlayTarget)),
        ("all notes off", |rig| rig.send(EngineCommand::AllNotesOff)),
        ("cc 123", |rig| {
            rig.key(InstrumentEvent::Control {
                controller: 123,
                value: 0,
            });
        }),
        ("cc 121", |rig| {
            rig.key(InstrumentEvent::Control {
                controller: 121,
                value: 0,
            });
        }),
        ("new stream", |rig| rig.parts.engine.prepare(RATE as u32)),
    ];
    for playing in [false, true] {
        for (name, end) in ends {
            let (mut rig, mut tape) = running(playing);
            // Track 1 has an arpeggio too: it must not take over what track 0 held.
            let mut other = rig.tape(1);
            rig.arp(1, arp(ArpShape::Up, ArpRate::Sixteenth));
            rig.run(64);
            let before = drain(&mut tape);
            assert_eq!(left_sounding(&before), [64], "{name}");
            let at = rig.frames;
            end(&mut rig);
            rig.run(48_000);
            let after = drain(&mut tape);
            if name == "new stream" {
                // `prepare` stops every instrument by itself; the arpeggio says no more.
                assert_eq!(after, [], "{name}");
            } else {
                let all: Vec<Ev> = before.iter().chain(&after).copied().collect();
                assert_eq!(left_sounding(&all), [] as [u8; 0], "{name}");
                assert!(after.iter().all(|e| e.0 == at), "{name}: {after:?}");
                assert_eq!(note_ons(&after), [], "{name}");
            }
            assert_eq!(note_ons(&drain(&mut other)), [], "{name}");
            // A key afterwards plays as the track it goes to says.
            rig.key(on(70));
            rig.run(640);
            rig.key(off(70));
            rig.run(12_000);
            rig.arp(0, Arp::default());
            rig.arp(1, Arp::default());
            rig.run(640);
            let (mine, theirs) = (drain(&mut tape), drain(&mut other));
            assert_eq!(left_sounding(&mine), [] as [u8; 0], "{name}");
            assert_eq!(left_sounding(&theirs), [] as [u8; 0], "{name}");
        }
    }
}

#[test]
fn a_track_emptied_or_given_another_instrument_leaves_no_note() {
    for playing in [false, true] {
        // Another instrument: the chord goes on, on the new one.
        let (mut rig, mut tape) = running(playing);
        let at = rig.frames;
        let mut new = rig.tape(0);
        rig.run(24_000);
        let old = drain(&mut tape);
        assert!(old.iter().all(|e| e.0 <= at), "{old:?}");
        let got = drain(&mut new);
        assert!(note_ons(&got).len() >= 3);
        rig.arp(0, Arp::default());
        rig.run(640);
        let got: Vec<Ev> = got.into_iter().chain(drain(&mut new)).collect();
        assert_eq!(left_sounding(&got), [] as [u8; 0]);

        // The track is gone: nothing more is made, also after an instrument comes back.
        let (mut rig, _tape) = running(playing);
        rig.send(EngineCommand::ClearTrack { track: 0 });
        rig.run(640);
        let mut new = rig.tape(0);
        rig.run(24_000);
        assert_eq!(drain(&mut new), []);
    }
}

#[test]
fn turning_it_on_lets_go_of_the_keys_that_sound_by_themselves() {
    let mut rig = Rig::new(64);
    let mut tape = rig.tape(0);
    rig.key(on(60));
    rig.key(on(64));
    rig.run(640);
    rig.arp(0, arp(ArpShape::Up, ArpRate::Sixteenth));
    rig.run(640);
    let got = drain(&mut tape);
    assert_eq!(
        got,
        [(0, on(60)), (0, on(64)), (640, off(60)), (640, off(64))]
    );
    // Their releases are no one's now; a key pressed anew is the arpeggio's.
    rig.key(off(60));
    rig.key(off(64));
    rig.key(on(72));
    rig.run(640);
    assert_eq!(drain(&mut tape), [(1280, on(72))]);
    // What was written down: the keys as played, their release, then the arpeggio's note.
    let said: Vec<InstrumentEvent> = (rig.recorded().iter())
        .filter_map(|r| match *r {
            Recorded::Recent { event, .. } => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(said, [on(60), on(64), off(60), off(64), on(72)]);
}

#[test]
fn play_and_stop_under_a_running_chord_cut_the_note_and_find_the_next_step() {
    let mut rig = Rig::new(64);
    let mut tape = rig.tape(0);
    let set = Arp {
        hold: true,
        ..arp(ArpShape::Up, ArpRate::Eighth)
    };
    rig.arp(0, set);
    rig.key(on(60));
    rig.run(640);
    rig.key(off(60));
    // Standing still at 120: a step every 12000 frames.
    rig.run(12_800 - 640);
    assert_eq!(
        drain(&mut tape),
        [(0, on(60)), (9600, off(60)), (12_000, on(60))]
    );
    // Play from 700: the note ends there, the next is on the grid (960, 1920 ...).
    let play = rig.frames;
    rig.send(EngineCommand::Play { from: 700 });
    rig.run(28_800);
    let f = |tick: u64| play + frame_of(tick, 700, 12_000);
    assert_eq!(
        drain(&mut tape),
        [
            (play, off(60)),
            (f(960), on(60)),
            (f(960 + 768), off(60)),
            (f(1920), on(60)),
            (f(1920 + 768), off(60)),
            (f(2880), on(60)),
        ]
    );
    let _ = rig.recorded();
    // Stop: the note ends, and the chord goes on from that moment by the tempo.
    let stop = rig.frames;
    let t_us = rig.now_us();
    rig.send(EngineCommand::Stop { t_us });
    rig.run(12_800);
    assert_eq!(
        drain(&mut tape),
        [
            (stop, off(60)),
            (stop, on(60)),
            (stop + 9600, off(60)),
            (stop + 12_000, on(60)),
        ]
    );
    // The run that played is closed before anything of the next one is said.
    let items = rig.recorded();
    let end = (items.iter())
        .position(|r| matches!(r, Recorded::RecentEnd { .. }))
        .expect("the run ended");
    assert!(
        items[end + 1..]
            .iter()
            .all(|r| matches!(r, Recorded::Recent { at: None, .. }))
    );
    assert!(items.len() > end + 2);
}

#[test]
fn the_notes_of_the_song_and_auditions_do_not_go_through_it() {
    let mut rig = Rig::new(64);
    let mut tape = rig.tape(0);
    rig.arp(0, arp(ArpShape::Up, ArpRate::Sixteenth));
    let events = vec![
        SequenceEvent {
            tick: 100,
            event: on(48),
        },
        SequenceEvent {
            tick: 1000,
            event: off(48),
        },
    ];
    let sent = (rig.parts.handle).swap_sequence(0, Sequence::new(events));
    assert!(sent.is_ok());
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(640);
    let keys = meloet_engine::Chord::new(&[70, 74]);
    rig.send(EngineCommand::Audition {
        track: 0,
        keys,
        velocity: 90,
    });
    rig.run(24_000);
    rig.send(EngineCommand::AuditionOff);
    rig.run(640);
    let hit = |note| InstrumentEvent::NoteOn { note, velocity: 90 };
    assert_eq!(
        drain(&mut tape),
        [
            (640, hit(70)),
            (640, hit(74)),
            (frame_of(100, 0, 12_000), on(48)),
            (frame_of(1000, 0, 12_000), off(48)),
            (640 + 24_000, off(70)),
            (640 + 24_000, off(74)),
        ]
    );
    // Neither is written down as played.
    assert!(
        rig.recorded()
            .iter()
            .all(|r| !matches!(r, Recorded::Recent { .. }))
    );
}

#[test]
fn a_change_of_the_rate_is_taken_at_the_next_step_of_the_new_grid() {
    let mut rig = Rig::new(64);
    let mut tape = rig.tape(0);
    let set = arp(ArpShape::Up, ArpRate::Quarter);
    rig.arp(0, set);
    rig.send(EngineCommand::Play { from: 0 });
    rig.key(on(60));
    rig.run(6400);
    rig.arp(
        0,
        Arp {
            rate: ArpRate::Sixteenth,
            ..set
        },
    );
    rig.run(24_000);
    let got = drain(&mut tape);
    // The note that sounds keeps its length; the steps are sixteenths from the next one on.
    assert_eq!(
        got[..6],
        [
            (0, on(60)),
            (12_000, off(60)),
            (12_000, on(60)),
            (16_800, off(60)),
            (18_000, on(60)),
            (22_800, off(60)),
        ]
    );
}

#[test]
fn the_arpeggio_stays_off_the_allocator_on_every_path() {
    let mut rig = Rig::new(256);
    for track in [0, 1] {
        let synth = Box::new(PlaceholderSynth::new());
        assert!(rig.parts.handle.swap_instrument(track, synth).is_ok());
    }
    rig.send(EngineCommand::SetLoop {
        start: Q,
        end: 5 * Q,
        on: true,
    });
    let shapes = ArpShape::ALL;
    let rates = ArpRate::ALL;
    let mut blocks = [64usize, 333, 1024, 2048, 32].into_iter().cycle();
    let mut said = [0usize; 3];
    for round in 0..48u64 {
        rig.block = blocks.next().unwrap();
        rig.out.resize(rig.block * 2, 0.0);
        let r = round as usize;
        rig.arp(
            r % 2,
            Arp {
                on: round % 11 != 10,
                shape: shapes[r % 4],
                rate: rates[(r / 2) % 4],
                triplet: round % 3 == 0,
                octaves: 1 + (round % 3) as u8,
                hold: round % 5 < 2,
            },
        );
        match round % 8 {
            0 => rig.send(EngineCommand::Play { from: round * 10 }),
            1 => rig.send(EngineCommand::SetPlayTarget { track: r / 8 % 2 }),
            2 => rig.send(EngineCommand::Locate { tick: 700 }),
            3 => rig.send(EngineCommand::RecordHere {
                muted: TrackSet::default(),
            }),
            4 => rig.tempo(7_000 + round as u32 * 400),
            5 => rig.send(EngineCommand::Stop { t_us: rig.now_us() }),
            6 => rig.send(EngineCommand::Record {
                from: 2 * Q,
                count_in_beats: (round % 3) as u32,
                muted: TrackSet::default(),
            }),
            _ => rig.send(EngineCommand::AllNotesOff),
        }
        let now = rig.now_us();
        for k in 0..20u64 {
            let t_us = now + k * 9_000;
            rig.keys.push(MidiIn {
                event: on(50 + k as u8),
                t_us,
            });
            rig.keys.push(MidiIn {
                event: off(50 + k as u8),
                t_us: t_us + 30_000 + k * 7_000,
            });
        }
        rig.keys.sort_by_key(|k| k.t_us);
        rig.run(RATE / 2);
        // Off the audio thread: empty what came back.
        for item in rig.recorded() {
            match item {
                Recorded::Event { .. } => said[0] += 1,
                Recorded::Recent { at: Some(_), .. } => said[1] += 1,
                Recorded::Recent { at: None, .. } => said[2] += 1,
                _ => {}
            }
        }
        rig.parts.handle.drop_retired();
    }
    // In a recording, while playing, and standing still.
    assert!(said.iter().all(|&n| n > 30), "{said:?}");
    assert_eq!(rig.parts.handle.meters().record_overflows(), 0);
    assert_eq!(
        common::audio_block_counts(),
        (0, 0),
        "(allocations, frees) inside audio blocks"
    );
}

#[test]
fn the_keys_of_the_pc_keyboard_go_through_it_as_the_midi_keys_do() {
    let mut rig = Rig::new(64);
    let mut tape = rig.tape(0);
    rig.arp(0, arp(ArpShape::Up, ArpRate::Sixteenth));
    rig.send(EngineCommand::Play { from: 0 });
    let t_us = rig.now_us();
    let typed = rig.parts.handle.typed_in();
    typed
        .push(MidiIn {
            event: on(64),
            t_us,
        })
        .unwrap();
    rig.key(on(60));
    rig.run(17_920);
    let t_us = rig.now_us();
    let typed = rig.parts.handle.typed_in();
    typed
        .push(MidiIn {
            event: off(64),
            t_us,
        })
        .unwrap();
    rig.run(12_000);
    rig.key(off(60));
    rig.run(6400);
    let got = drain(&mut tape);
    assert_eq!(
        note_ons(&got),
        [
            (0, 60),
            (6000, 64),
            (12_000, 60),
            (18_000, 60),
            (24_000, 60)
        ]
    );
    assert_eq!(left_sounding(&got), [] as [u8; 0]);
}

#[test]
fn with_no_stream_the_settings_are_taken_and_nothing_is_said() {
    let mut rig = Rig::new(64);
    let mut tape = rig.tape(0);
    rig.key(on(60));
    rig.run(640);
    let _ = rig.recorded();
    // The stream is gone with the key still down; the arpeggio is turned on meanwhile.
    rig.arp(0, arp(ArpShape::Up, ArpRate::Sixteenth));
    rig.parts.engine.settle();
    assert_eq!(rig.recorded(), []);
    rig.parts.engine.prepare(RATE as u32);
    let _ = drain(&mut tape);
    rig.key(off(60));
    rig.key(on(62));
    rig.run(640);
    assert_eq!(note_ons(&drain(&mut tape)), [(640, 62)]);
}
