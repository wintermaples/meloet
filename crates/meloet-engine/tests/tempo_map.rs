//! The engine on a tempo map with ramps and meter changes (docs/architecture.md 21-3, 21-4):
//! every note and every click on the frame the map's formula gives, whatever the block size
//! and however long it runs, and every key at the tick the formula gives.

mod common;

use std::sync::Arc;

use meloet_engine::{
    BlockTiming, Effect, EffectContext, Engine, EngineCommand, EngineConfig, EngineParts,
    Instrument, InstrumentEvent, MidiIn, MixStrip, PluginBlock, Recorded, Sequence, SequenceEvent,
    TrackSet, TransportState,
};
use meloet_model::{Meter, MeterChange, Tempo, TempoMap, TempoPoint, Timeline};
use meloet_plugin::ipc::Transport;
use rtrb::{Consumer, Producer, RingBuffer};

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
const BAR_LEVEL: f32 = 0.17;
const BEAT_LEVEL: f32 = 0.13;

/// Writes down every event with the frame (counted from the first render) it took effect on,
/// and says nothing.
struct Tape {
    tx: Producer<(u64, InstrumentEvent)>,
    frames: u64,
    latency: u32,
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
    fn latency(&self) -> u32 {
        self.latency
    }
}

/// Stands for a plugin: writes down what plugins are told at the head of every stretch.
struct Listener {
    block: Arc<PluginBlock>,
    tx: Producer<(u64, Transport)>,
    frames: u64,
}

impl Instrument for Listener {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn event(&mut self, _ev: InstrumentEvent) {}
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.tx
            .push((self.frames, self.block.transport()))
            .expect("listener full");
        left.fill(0.0);
        right.fill(0.0);
        self.frames += left.len() as u64;
    }
    fn reset(&mut self) {}
}

/// Stands for the delay: writes down the tempo it is given, whenever it is another one.
struct Hears {
    tx: Producer<(u64, f32)>,
    frames: u64,
    last: f32,
}

impl Effect for Hears {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, _index: u32, _value: f32) {}
    fn process(&mut self, left: &mut [f32], _right: &mut [f32], ctx: &EffectContext) {
        if ctx.tempo_bpm != self.last {
            self.last = ctx.tempo_bpm;
            self.tx.push((self.frames, ctx.tempo_bpm)).expect("full");
        }
        self.frames += left.len() as u64;
    }
    fn reset(&mut self) {}
}

struct Rig {
    parts: EngineParts,
    rate: u32,
    /// Frames rendered so far.
    frames: u64,
    base_us: u64,
    delay_us: u64,
    /// Keys not yet arrived, by `t_us`.
    keys: Vec<MidiIn>,
    out: Vec<f32>,
    /// The first frame of every click heard, and whether it was the loud one.
    clicks: Vec<(u64, bool)>,
    quiet_until: u64,
    /// The first frame of every block, and when it reaches the DAC.
    starts: Vec<(u64, u64)>,
}

impl Rig {
    fn new(rate: u32, delay_us: u64) -> Rig {
        let config = EngineConfig {
            sample_rate: rate,
            max_frames: 1024,
            max_tracks: 4,
        };
        Rig {
            parts: Engine::new(config),
            rate,
            frames: 0,
            base_us: 1_000_000,
            delay_us,
            keys: Vec::new(),
            out: Vec::new(),
            clicks: Vec::new(),
            quiet_until: 0,
            starts: Vec::new(),
        }
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    fn map(&mut self, map: &Arc<TempoMap>) {
        assert!(self.parts.handle.set_tempo_map(map.clone()).is_ok());
    }

    fn tape(&mut self, track: usize, latency: u32) -> Consumer<(u64, InstrumentEvent)> {
        let (tx, rx) = RingBuffer::new(1 << 17);
        let tape = Tape {
            tx,
            frames: 0,
            latency,
        };
        assert!(
            self.parts
                .handle
                .swap_instrument(track, Box::new(tape))
                .is_ok()
        );
        rx
    }

    fn sequence(&mut self, track: usize, ticks: &[u64]) {
        let events = ticks.iter().map(|&tick| SequenceEvent {
            tick,
            event: on(60),
        });
        let sequence = Sequence::new(events.collect());
        assert!(self.parts.handle.swap_sequence(track, sequence).is_ok());
    }

    /// When frame `n` (counted from the first render) reaches the DAC.
    fn dac_us(&self, n: u64) -> f64 {
        self.base_us as f64 + self.delay_us as f64 + n as f64 * 1e6 / f64::from(self.rate)
    }

    /// The frame (with its fraction) that is at the DAC at `t_us`, on the line of the block
    /// that is there then. The blocks are given whole microseconds, as a device gives them, so
    /// this is up to a microsecond off `dac_us`.
    fn frame_at(&self, t_us: u64) -> f64 {
        let i = self.starts.partition_point(|&(_, start)| start <= t_us);
        let (frame, start) = self.starts[i - 1];
        frame as f64 + (t_us - start) as f64 * f64::from(self.rate) / 1e6
    }

    fn key(&mut self, t_us: u64, note: u8) {
        self.keys.push(MidiIn {
            event: on(note),
            t_us,
        });
        self.keys.sort_by_key(|k| k.t_us);
    }

    /// One device block of `block` frames.
    fn block(&mut self, block: usize) {
        let callback_us = self.base_us + (self.frames as f64 * 1e6 / f64::from(self.rate)) as u64;
        while self.keys.first().is_some_and(|k| k.t_us <= callback_us) {
            let key = self.keys.remove(0);
            self.parts.midi_in.push(key).expect("midi ring full");
        }
        let timing = BlockTiming {
            callback_us,
            playback_us: callback_us + self.delay_us,
        };
        self.starts.push((self.frames, timing.playback_us));
        self.out.resize(block * 2, 0.0);
        self.parts.engine.process(&mut self.out, timing);
        // Nothing but the metronome sounds here, and a click starts on its loudest sample.
        for (i, frame) in self.out.as_chunks::<2>().0.iter().enumerate() {
            let at = self.frames + i as u64;
            if frame[0] != 0.0 && at >= self.quiet_until {
                let bar = (frame[0] - BAR_LEVEL).abs() < 1e-4;
                assert!(
                    bar || (frame[0] - BEAT_LEVEL).abs() < 1e-4,
                    "{at}: {frame:?}"
                );
                self.clicks.push((at, bar));
                self.quiet_until = at + u64::from(self.rate) / 25;
            }
        }
        self.frames += block as u64;
        self.parts.handle.drop_retired();
    }

    /// Whole blocks, their sizes from `sizes` in turn, until at least `frames` more are done.
    fn run(&mut self, frames: u64, sizes: &mut dyn Iterator<Item = usize>) {
        let end = self.frames + frames;
        while self.frames < end {
            self.block(sizes.next().expect("block sizes go on"));
        }
    }

    fn recorded(&mut self) -> Vec<Recorded> {
        std::iter::from_fn(|| self.parts.handle.pop_recorded()).collect()
    }
}

/// The frames of the keys pressed, as the tape has them.
fn pressed(tape: &mut Consumer<(u64, InstrumentEvent)>) -> impl Iterator<Item = u64> {
    std::iter::from_fn(|| tape.pop().ok())
        .filter(|(_, event)| matches!(event, InstrumentEvent::NoteOn { .. }))
        .map(|(frame, _)| frame)
}

fn on(note: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOn { note, velocity: 90 }
}

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

fn point(tick: u64, centi: u32, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick,
        tempo: tempo(centi),
        ramp,
    }
}

fn flag(bar: u32, num: u8, den: u8) -> MeterChange {
    MeterChange {
        bar,
        meter: Meter::new(num, den).unwrap(),
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = (self.0)
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Block sizes with no pattern, 1 to 2048 frames (over the engine's 1024, too).
fn ragged() -> impl Iterator<Item = usize> {
    let mut lcg = Lcg(7);
    std::iter::repeat_with(move || 1 + lcg.below(2048) as usize)
}

fn fixed(block: usize) -> impl Iterator<Item = usize> {
    std::iter::repeat(block)
}

/// The spec's example first (96, then 104 at bar 5 going up to 120 at bar 9), then some 6 250
/// quarter notes of tempo points at odd places, half of them ramps, and meter changes every
/// few bars, with every meter the song may have.
fn winding() -> Timeline {
    let mut lcg = Lcg(2026);
    let mut tempo_points = vec![point(4 * BAR, 10_400, true), point(8 * BAR, 12_000, false)];
    let mut tick = 8 * BAR;
    while tick < 6_250 * Q {
        tick += 35 * Q / 24 + lcg.below(75 * Q / 4);
        let centi = 3_000 + lcg.below(27_001) as u32;
        tempo_points.push(point(tick, centi, lcg.below(2) == 0));
    }
    let mut meter_changes = Vec::new();
    let mut bar = 9;
    while bar < 4_000 {
        let den = Meter::DENS[lcg.below(4) as usize];
        meter_changes.push(flag(bar, 1 + lcg.below(12) as u8, den));
        bar += 1 + lcg.below(12) as u32;
    }
    let timeline = Timeline {
        tempo: tempo(9_600),
        meter: Meter::FOUR_FOUR,
        head_ramp: false,
        tempo_points,
        meter_changes,
    };
    assert_eq!(timeline.check(), Ok(()));
    timeline
}

/// Notes at ticks with no pattern to the frames, and on, just before and just after every
/// tempo point.
fn notes_of(timeline: &Timeline, until: u64) -> Vec<u64> {
    let mut ticks: Vec<u64> = (0..until).step_by(97).collect();
    for p in &timeline.tempo_points {
        ticks.extend([p.tick - 1, p.tick, p.tick + 1]);
    }
    ticks.retain(|&t| t < until);
    ticks.sort_unstable();
    ticks
}

/// Every beat from `from` until `until`, with the frame the map gives it counting from
/// `from_seconds`.
fn beats(map: &TempoMap, from: u64, until: u64, from_seconds: f64, rate: f64) -> Vec<(u64, bool)> {
    let mut out = Vec::new();
    let mut beat = map.next_beat(from as f64);
    while (beat.tick as u64) < until {
        let frame = map.frame_of(beat.tick as f64, from_seconds, rate);
        out.push((frame, beat.downbeat));
        beat = map.next_beat((beat.tick + 1) as f64);
    }
    out
}

/// Plays `seconds` of the winding map from the head and gives how many notes and clicks were
/// compared with the formula.
fn play_and_compare(
    rate: u32,
    seconds: u64,
    sizes: &mut dyn Iterator<Item = usize>,
) -> (usize, usize) {
    let timeline = winding();
    let map = Arc::new(TempoMap::new(&timeline));
    let total = seconds * u64::from(rate);
    let until = map.tick_at(seconds as f64).ceil() as u64 + 1000;
    let notes = notes_of(&timeline, until);

    let mut rig = Rig::new(rate, 0);
    let mut tape = rig.tape(0, 0);
    rig.map(&map);
    rig.sequence(0, &notes);
    rig.send(EngineCommand::SetMetronome { on: true });
    rig.send(EngineCommand::Play { from: 0 });
    let mut got = Vec::new();
    while rig.frames < total {
        rig.run(u64::from(rate), sizes);
        got.extend(pressed(&mut tape));
    }
    let rate_f = f64::from(rate);
    let want: Vec<u64> = (notes.iter())
        .map(|&tick| map.frame_of(tick as f64, 0.0, rate_f))
        .filter(|&frame| frame < total)
        .collect();
    got.retain(|&frame| frame < total);
    assert!(want.len() > 5_000);
    assert_eq!(got.len(), want.len(), "notes sent");
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w, "note {i} at tick {}", notes[i]);
    }

    let mut want: Vec<(u64, bool)> = beats(&map, 0, until, 0.0, rate_f);
    want.retain(|&(frame, _)| frame < total);
    rig.clicks.retain(|&(frame, _)| frame < total);
    assert!(want.len() > 500);
    assert_eq!(rig.clicks.len(), want.len(), "clicks heard");
    for (i, (g, w)) in rig.clicks.iter().zip(&want).enumerate() {
        assert_eq!(g, w, "click {i}");
    }
    assert!(want.iter().any(|c| c.1) && want.iter().any(|c| !c.1));
    (got.len(), want.len())
}

#[test]
fn ten_minutes_on_a_winding_map_every_note_and_click_is_on_the_frame_of_the_formula() {
    let mut compared = (0, 0);
    let runs: [(u32, Box<dyn Iterator<Item = usize>>); 5] = [
        (48_000, Box::new(fixed(64))),
        (48_000, Box::new(fixed(256))),
        (48_000, Box::new(fixed(1024))),
        (48_000, Box::new(ragged())),
        (44_100, Box::new(ragged())),
    ];
    for (rate, mut sizes) in runs {
        let (notes, clicks) = play_and_compare(rate, 600, &mut *sizes);
        compared = (compared.0 + notes, compared.1 + clicks);
    }
    println!("compared {} notes and {} clicks", compared.0, compared.1);
    assert_eq!(common::audio_block_counts(), (0, 0));
}

/// Where [`loop_map`]'s ramp starts, and where it ends.
const LOOP_RAMP: (u64, u64) = (25 * Q / 8, 35 * Q / 8);
/// The loop on [`loop_map`]: from inside the ramp, over the meter change, past the ramp's end.
const LOOP: (u64, u64) = (85 * Q / 24, 115 * Q / 24);

/// A loop over a ramp, a tempo point and a meter change: 150 going up from `LOOP_RAMP.0` to
/// 260 at `LOOP_RAMP.1`, 200 from there; 7/8 from bar 2 (`BAR`).
fn loop_map() -> Arc<TempoMap> {
    let timeline = Timeline {
        tempo: tempo(12_000),
        meter: Meter::FOUR_FOUR,
        head_ramp: false,
        tempo_points: vec![
            point(LOOP_RAMP.0, 15_000, true),
            point(LOOP_RAMP.1, 26_000, false),
            point(LOOP_RAMP.1 + 1, 20_000, false),
        ],
        meter_changes: vec![flag(1, 7, 8)],
    };
    assert_eq!(timeline.check(), Ok(()));
    Arc::new(TempoMap::new(&timeline))
}

#[test]
fn a_thousand_laps_of_a_loop_stay_on_the_frames_of_the_formula() {
    const LAPS: u64 = 1000;
    const AHEAD: u32 = 300;
    let (start, end) = LOOP;
    let rate = 48_000.0;
    let map = loop_map();
    let point = LOOP_RAMP.1;
    let notes: Vec<u64> = vec![
        3,
        Q + 20,
        start,
        start + 1,
        BAR - 1,
        BAR,
        BAR + 1,
        point - 1,
        point,
        point + 1,
        end - 1,
    ];
    for mut sizes in [
        Box::new(fixed(256)) as Box<dyn Iterator<Item = usize>>,
        Box::new(ragged()),
    ] {
        let mut rig = Rig::new(48_000, 0);
        let mut tape = rig.tape(0, 0);
        // A track whose instrument delays: its notes are sent that much early, at the loop's
        // start too.
        let mut early = rig.tape(1, AHEAD);
        rig.map(&map);
        rig.sequence(0, &notes);
        rig.sequence(1, &notes);
        rig.send(EngineCommand::SetMetronome { on: true });
        rig.send(EngineCommand::SetLoop {
            start,
            end,
            on: true,
        });
        rig.send(EngineCommand::Play { from: 0 });

        // The first pass counts from the head, every lap after it from the loop's start.
        let start_sec = map.seconds_at(start as f64);
        let first = map.frame_of(end as f64, 0.0, rate);
        let lap = map.frame_of(end as f64, start_sec, rate);
        let total = first + LAPS * lap;
        let mut want = Vec::new();
        let mut want_clicks = beats(&map, 0, end, 0.0, rate);
        want.extend(notes.iter().map(|&t| map.frame_of(t as f64, 0.0, rate)));
        for n in 0..LAPS {
            let head = first + n * lap;
            let in_lap = notes.iter().filter(|&&t| t >= start);
            want.extend(in_lap.map(|&t| head + map.frame_of(t as f64, start_sec, rate)));
            let clicks = beats(&map, start, end, start_sec, rate);
            want_clicks.extend(clicks.iter().map(|&(f, bar)| (head + f, bar)));
        }

        let (mut got, mut got_early) = (Vec::new(), Vec::new());
        while rig.frames < total {
            rig.run(48_000, &mut *sizes);
            got.extend(pressed(&mut tape));
            got_early.extend(pressed(&mut early));
        }
        let within = |frames: &mut Vec<u64>, ahead: u64| frames.retain(|&f| f + ahead < total);
        within(&mut got, 0);
        within(&mut want, 0);
        assert_eq!(got.len(), want.len());
        assert_eq!(got, want);
        // The lane that reads ahead: the same notes, `AHEAD` frames before (the very first
        // ones, closer to the start than that, at once).
        let ahead = u64::from(AHEAD);
        within(&mut got_early, ahead);
        let want_early: Vec<u64> = want.iter().map(|&f| f.saturating_sub(ahead)).collect();
        assert_eq!(got_early, want_early);

        rig.clicks.retain(|c| c.0 < total);
        want_clicks.retain(|c| c.0 < total);
        assert_eq!(rig.clicks, want_clicks);
        println!(
            "{} notes twice and {} clicks over {LAPS} laps",
            want.len(),
            want_clicks.len()
        );
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn a_map_put_in_while_playing_keeps_the_tick_and_loses_no_note_or_click() {
    let rate = 48_000.0;
    let a = common::flat_map(12_000, 4, 4);
    let b = Arc::new(TempoMap::new(&winding()));
    let c = loop_map();
    let notes: Vec<u64> = (0..250 * Q / 3).step_by((Q / 15) as usize).collect();
    // 250-frame blocks: at 120.00 the second beat (tick `Q`), a beat and a note, is the first
    // frame of block 96.
    let mut rig = Rig::new(48_000, 5_000);
    let mut tape = rig.tape(0, 0);
    rig.map(&a);
    rig.sequence(0, &notes);
    rig.send(EngineCommand::SetMetronome { on: true });
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(24_000, &mut fixed(250));

    let mut want = Vec::new();
    let mut want_clicks = Vec::new();
    // (the map, the frame it came in at, the tick there)
    let mut legs: Vec<(Arc<TempoMap>, u64, f64)> = vec![(a.clone(), 0, 0.0)];
    for (next, blocks) in [(&b, 333u64), (&c, 77), (&a, 1), (&b, 400)] {
        let (map, from_frame, from_tick) = legs.last().unwrap().clone();
        let at = rig.frames;
        let sec = map.seconds_at(from_tick) + (at - from_frame) as f64 / rate;
        let tick = map.tick_at(sec);
        let tick = if (tick - tick.round()).abs() < 1e-6 {
            tick.round()
        } else {
            tick
        };

        // Read a moment before the change with the old map, a moment after with the new:
        // one line of ticks goes through the change.
        let (old_map, old_gen) = rig.parts.handle.tempo_map();
        let (old_map, old_gen) = (old_map.clone(), old_gen);
        rig.map(next);
        rig.run(250 * blocks, &mut fixed(250));
        let clock = rig.parts.handle.clock().clone();
        let (t_before, t_after) = (rig.dac_us(at) as u64 - 200, rig.dac_us(at) as u64 + 200);
        let before = clock.heard_at(t_before, &old_map, old_gen).unwrap();
        let since = (rig.frame_at(t_before) - at as f64) / rate;
        assert!(
            (before.tick - map.tick_at(sec + since)).abs() < 1e-6,
            "{before:?} {tick}"
        );
        let (new_map, new_gen) = rig.parts.handle.tempo_map();
        let after = clock.heard_at(t_after, new_map, new_gen).unwrap();
        let since = (rig.frame_at(t_after) - at as f64) / rate;
        let want = next.tick_at(next.seconds_at(tick) + since);
        assert!((after.tick - want).abs() < 1e-6, "{after:?} {tick}");
        legs.push((next.clone(), at, tick));
    }
    let total = rig.frames;
    let mut next_note = 0;
    for (i, (map, from_frame, from_tick)) in legs.iter().enumerate() {
        let until_frame = legs.get(i + 1).map_or(total, |next| next.1);
        let until_tick = legs.get(i + 1).map_or(f64::MAX, |next| next.2);
        let from_sec = map.seconds_at(*from_tick);
        // A note still to come when the map changes is sent on the new map's frames.
        while let Some(&t) = notes.get(next_note) {
            let frame = from_frame + map.frame_of(t as f64, from_sec, rate);
            if frame >= until_frame {
                break;
            }
            want.push(frame);
            next_note += 1;
        }
        let first = from_tick.ceil() as u64;
        let last = if until_tick == f64::MAX {
            10_000_000
        } else {
            until_tick.ceil() as u64
        };
        for (frame, bar) in beats(map, first, last, from_sec, rate) {
            // A beat due on the very frame of the change still clicks there.
            if from_frame + frame <= until_frame && from_frame + frame < total {
                want_clicks.push((from_frame + frame, bar));
            }
        }
    }
    let got: Vec<u64> = pressed(&mut tape).collect();
    assert_eq!(legs[1].1, 24_000);
    assert_eq!(legs[1].2, Q as f64);
    assert!(got.contains(&24_000) && rig.clicks.contains(&(24_000, false)));
    assert_eq!(got, want);
    assert_eq!(rig.clicks, want_clicks);
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn a_beat_and_a_note_a_part_of_a_frame_before_the_change_of_map_still_sound() {
    // At 97.01 the first beat is at frame 29 687.66: frame 29 688 carries it, and 24 blocks
    // of 1237 frames end right there, with the position a third of a frame past the beat.
    let mut rig = Rig::new(48_000, 0);
    let mut tape = rig.tape(0, 0);
    rig.map(&common::flat_map(9_701, 4, 4));
    rig.sequence(0, &[Q, BAR + 80]);
    rig.send(EngineCommand::SetMetronome { on: true });
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(29_688, &mut fixed(1237));
    assert_eq!(rig.frames, 29_688);
    rig.map(&common::flat_map(6_000, 7, 8));
    rig.run(48_000, &mut fixed(1237));
    let got: Vec<u64> = pressed(&mut tape).collect();
    assert_eq!(got[0], 29_688);
    assert_eq!(rig.clicks[..2], [(0, true), (29_688, false)]);
}

/// [`keys_map`]: the point to 192, the ramp down to 40, the change to 6/8, the point to 300.
const KEYS_POINT: u64 = 2 * Q;
const KEYS_RAMP: (u64, u64) = (25 * Q / 4, 125 * Q / 12);
const KEYS_FLAG: u64 = 3 * BAR;
const KEYS_LAST: u64 = 175 * Q / 12;

/// 96 in 4/4; 192 from `KEYS_POINT` (frame 60 000 at 48 kHz, in the middle of a block); from
/// `KEYS_RAMP.0` down in a ramp to 40 at `KEYS_RAMP.1`; 6/8 from bar 4 (`KEYS_FLAG`); 300 from
/// `KEYS_LAST`.
fn keys_map() -> Arc<TempoMap> {
    let timeline = Timeline {
        tempo: tempo(9_600),
        meter: Meter::FOUR_FOUR,
        head_ramp: false,
        tempo_points: vec![
            point(KEYS_POINT, 19_200, false),
            point(KEYS_RAMP.0, 19_200, true),
            point(KEYS_RAMP.1, 4_000, false),
            point(KEYS_LAST, 30_000, false),
        ],
        meter_changes: vec![flag(3, 6, 8)],
    };
    assert_eq!(timeline.check(), Ok(()));
    Arc::new(TempoMap::new(&timeline))
}

fn events(recorded: &[Recorded]) -> Vec<(u64, f64, u32)> {
    let events = recorded.iter().filter_map(|r| match r {
        Recorded::Event {
            t_us, tick, lap, ..
        } => Some((*t_us, *tick, *lap)),
        _ => None,
    });
    events.collect()
}

#[test]
fn keys_get_the_tick_of_the_formula_around_points_ramps_and_meter_changes() {
    let map = keys_map();
    let mut worst = 0.0f64;
    for (block, delay_us) in [(64usize, 3_000u64), (256, 12_000), (1024, 40_000)] {
        let mut rig = Rig::new(48_000, delay_us);
        rig.map(&map);
        rig.send(EngineCommand::Record {
            from: 0,
            count_in_beats: 0,
            muted: TrackSet::default(),
        });
        let t0 = rig.dac_us(0);
        // Seconds into the song: around the first point (1.25 s), in the ramp, around its
        // end, around the meter change and the point after it, and in the tenth minute.
        let ramp_from = map.seconds_at(KEYS_RAMP.0 as f64);
        let ramp_to = map.seconds_at(KEYS_RAMP.1 as f64);
        let flag_at = map.seconds_at(KEYS_FLAG as f64);
        let last_point = map.seconds_at(KEYS_LAST as f64);
        let mut at: Vec<f64> = vec![0.000_3, 0.7, 1.249_9, 1.250_0, 1.250_1, 1.26, 2.0];
        for edge in [ramp_from, ramp_to, flag_at, last_point] {
            at.extend([edge - 0.000_2, edge + 0.000_2, edge + 0.005]);
        }
        at.extend((1..40).map(|i| ramp_from + (ramp_to - ramp_from) * f64::from(i) / 40.0));
        at.extend([599.0, 599.999_9, 600.0, 601.234_567]);
        let keys: Vec<u64> = at.iter().map(|s| (t0 + s * 1e6).round() as u64).collect();
        for (i, &t_us) in keys.iter().enumerate() {
            rig.key(t_us, (i % 100) as u8);
        }
        rig.run(48_000 * 603, &mut fixed(block));
        // The play line, read as the meter thread does, is on the same line.
        for back_us in [100.0, 9_000.0, 30_000.0] {
            let t_us = (rig.dac_us(rig.frames) - back_us) as u64;
            let h = rig.parts.handle.heard_at(t_us).unwrap();
            let want = map.tick_at(rig.frame_at(t_us) / 48_000.0);
            assert!((h.tick - want).abs() < 1e-6);
            assert!(
                (h.ticks_per_us - map.quarter_bpm_at(want) * Q as f64 / 60.0 / 1e6).abs() < 1e-12
            );
        }
        let got = events(&rig.recorded());
        assert_eq!(got.len(), keys.len(), "block {block}");
        for (t_us, tick, lap) in got {
            let want = map.tick_at(rig.frame_at(t_us) / 48_000.0);
            worst = worst.max((tick - want).abs());
            assert!(
                (tick - want).abs() < 0.01,
                "block {block}: key at {t_us} got {tick}, want {want}"
            );
            assert_eq!(lap, 0);
        }
    }
    println!("worst key: {worst:e} ticks off the formula");
    assert!(worst < 1e-6);
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn keys_around_the_turn_of_a_loop_get_the_tick_of_the_formula() {
    let map = loop_map();
    let (start, end) = LOOP;
    let rate = 48_000.0;
    let mut worst = 0.0f64;
    for (block, delay_us) in [(64usize, 3_000u64), (1024, 40_000)] {
        let mut rig = Rig::new(48_000, delay_us);
        rig.map(&map);
        rig.send(EngineCommand::SetLoop {
            start,
            end,
            on: true,
        });
        rig.send(EngineCommand::Record {
            from: 0,
            count_in_beats: 0,
            muted: TrackSet::default(),
        });
        let t0 = rig.dac_us(0);
        let start_sec = map.seconds_at(start as f64);
        let first = map.frame_of(end as f64, 0.0, rate);
        let lap = map.frame_of(end as f64, start_sec, rate);
        // (when, the lap it is in, the first frame of that lap, the map's seconds there)
        let mut want = Vec::new();
        for n in [0u64, 1, 2, 57, 499, 1499] {
            // The turn that ends lap `n`, and the anchor of the lap before it.
            let turn = first + n * lap;
            let turn_us = t0 + turn as f64 * 1e6 / rate;
            for (us, after) in [
                (-300.0, false),
                (-40.0, false),
                (40.0, true),
                (300.0, true),
                (90_000.0, true),
            ] {
                let t_us = (turn_us + us).round() as u64;
                let (lap_no, head, sec) = match (after, n) {
                    (true, _) => (n + 1, turn, start_sec),
                    (false, 0) => (0, 0, 0.0),
                    (false, _) => (n, turn - lap, start_sec),
                };
                want.push((t_us, lap_no as u32, head, sec));
            }
        }
        for (i, &(t_us, ..)) in want.iter().enumerate() {
            rig.key(t_us, (i % 100) as u8);
        }
        rig.run(first + 1501 * lap, &mut fixed(block));
        let got = events(&rig.recorded());
        assert_eq!(got.len(), want.len());
        for ((t_us, tick, lap), (want_us, want_lap, head, sec)) in got.into_iter().zip(want) {
            assert_eq!((t_us, lap), (want_us, want_lap), "block {block}");
            let want_tick = map.tick_at(sec + (rig.frame_at(t_us) - head as f64) / rate);
            worst = worst.max((tick - want_tick).abs());
            assert!(
                (tick - want_tick).abs() < 0.01,
                "block {block}: lap {lap} key at {t_us}: {tick}, want {want_tick}"
            );
        }
    }
    println!("worst key: {worst:e} ticks off the formula");
    assert!(worst < 1e-6);
}

#[test]
fn the_count_in_keeps_the_tempo_and_meter_of_where_the_recording_starts() {
    let map = keys_map();
    let rate = 48_000.0;
    // In the ramp (192 going down to 40), on a beat of 4/4. And in the 6/8 part, at 40: the
    // beats are eighths, and the bar's first one is the loud click.
    for (from, count_in) in [(9 * Q, 4u32), (KEYS_FLAG + 3 * (Q / 2), 6)] {
        let at = map.meter_at(from);
        let beat = at.meter.ticks_per_beat();
        let ticks_per_s = map.quarter_bpm_at(from as f64) * Q as f64 / 60.0;
        let frames_per_beat = beat as f64 / ticks_per_s * rate;
        let switch = (f64::from(count_in) * frames_per_beat - 1e-6).ceil() as u64;

        let mut rig = Rig::new(48_000, 10_000);
        rig.map(&map);
        rig.send(EngineCommand::Record {
            from,
            count_in_beats: count_in,
            muted: TrackSet::default(),
        });
        let t0 = rig.dac_us(0);
        let switch_us = t0 + switch as f64 * 1e6 / rate;
        // One key in the last half beat of the count-in, one before it, one after the start.
        let half_beat_us = frames_per_beat / 2.0 / rate * 1e6;
        let early = (switch_us - half_beat_us * 0.8).round() as u64;
        let too_early = (switch_us - half_beat_us * 1.2).round() as u64;
        let late = (switch_us + 30_000.0).round() as u64;
        for (i, t_us) in [too_early, early, late].into_iter().enumerate() {
            rig.key(t_us, 60 + i as u8);
        }
        rig.run(switch + 48_000, &mut fixed(256));

        // The clicks of the count-in are that meter's beats, one tempo; the song's own
        // follow from the start.
        let want: Vec<(u64, bool)> = (0..count_in)
            .map(|k| {
                let frame = (f64::from(k) * frames_per_beat - 1e-6).ceil() as u64;
                let beats_in = ((from - at.bar_tick) / beat) as i64 - i64::from(count_in - k);
                (frame, beats_in.rem_euclid(i64::from(at.meter.num())) == 0)
            })
            .collect();
        assert!(want.iter().any(|c| c.1));
        let got: Vec<(u64, bool)> = rig
            .clicks
            .iter()
            .copied()
            .filter(|c| c.0 < switch)
            .collect();
        assert_eq!(got, want, "from {from}");

        let got = events(&rig.recorded());
        assert_eq!(
            got.len(),
            2,
            "the key before the last half beat is not recorded"
        );
        // The count-in is one straight line from its first frame.
        let line =
            (from - u64::from(count_in) * beat) as f64 + rig.frame_at(early) / rate * ticks_per_s;
        assert!((got[0].1 - line).abs() < 1e-6, "{} {line}", got[0].1);
        let since = (rig.frame_at(late) - switch as f64) / rate;
        let on_map = map.tick_at(map.seconds_at(from as f64) + since);
        assert!((got[1].1 - on_map).abs() < 1e-6, "{} {on_map}", got[1].1);
        let h = rig.parts.handle.heard_at(early).unwrap();
        assert_eq!(h.state, TransportState::CountIn);
        assert_eq!(h.count_in_beats_left, 1);
    }
}

#[test]
fn plugins_and_effects_hear_the_tempo_and_meter_from_the_very_frame_of_a_change() {
    let map = keys_map();
    let rate = 48_000.0;
    for mut sizes in [
        Box::new(fixed(256)) as Box<dyn Iterator<Item = usize>>,
        Box::new(ragged()),
    ] {
        let mut rig = Rig::new(48_000, 0);
        let (tx, mut told) = RingBuffer::new(1 << 16);
        let listener = Listener {
            block: rig.parts.handle.plugin_block().clone(),
            tx,
            frames: 0,
        };
        assert!(
            rig.parts
                .handle
                .swap_instrument(0, Box::new(listener))
                .is_ok()
        );
        let (tx, mut heard) = RingBuffer::new(1 << 12);
        let hears = Hears {
            tx,
            frames: 0,
            last: 0.0,
        };
        let slot = rig
            .parts
            .handle
            .set_effect(MixStrip::Track(0), 0, Some(Box::new(hears)));
        assert!(slot.is_ok());
        rig.map(&map);
        rig.send(EngineCommand::Play { from: 0 });
        rig.run(48_000 * 17, &mut *sizes);

        let told: Vec<(u64, Transport)> = std::iter::from_fn(|| told.pop().ok()).collect();
        // Every head: what the formula says at that frame.
        for &(frame, t) in &told {
            let tick = map.tick_at(frame as f64 / rate);
            assert!(
                (t.position_quarters - tick / Q as f64).abs() < 1e-6,
                "{frame}"
            );
            assert_eq!(t.position_frames, frame as i64);
            let tick = t.position_quarters * Q as f64;
            assert!((t.tempo_bpm - map.quarter_bpm_at(tick)).abs() < 1e-9);
            assert_eq!(t.flags, Transport::PLAYING);
        }
        // A stretch begins on the frame of every change, and is told what holds from there.
        for (tick, bpm, meter, bar_start) in [
            (KEYS_POINT, 192.0, (4, 4), 0.0),
            (KEYS_RAMP.1, 40.0, (4, 4), 8.0),
            (KEYS_FLAG, 20.0, (6, 8), 12.0),
            (KEYS_LAST, 150.0, (6, 8), 12.0),
        ] {
            let tick = tick as f64;
            let frame = map.frame_of(tick, 0.0, rate);
            let (_, t) = told
                .iter()
                .find(|(f, _)| *f == frame)
                .unwrap_or_else(|| panic!("no stretch begins at {tick}, frame {frame}"));
            assert_eq!(t.tempo_bpm, bpm, "at {tick}");
            assert_eq!((t.meter_num, t.meter_den), meter, "at {tick}");
            assert_eq!(t.bar_start_quarters, bar_start, "at {tick}");
            assert!(t.position_quarters >= tick / Q as f64);
        }
        // Inside the ramp no stretch is cut for the tempo, and the tempo told moves with it.
        let in_ramp = |f: u64| {
            f > map.frame_of(KEYS_RAMP.0 as f64, 0.0, rate)
                && f < map.frame_of(KEYS_RAMP.1 as f64, 0.0, rate)
        };
        let ramp: Vec<f64> = told
            .iter()
            .filter(|(f, _)| in_ramp(*f))
            .map(|(_, t)| t.tempo_bpm)
            .collect();
        assert!(ramp.len() > 20 && ramp.windows(2).all(|w| w[1] < w[0]));

        // The effect on the track: the tempo at the head of each stretch, the same numbers.
        let heard: Vec<(u64, f32)> = std::iter::from_fn(|| heard.pop().ok()).collect();
        assert_eq!(heard[0], (0, 96.0));
        assert_eq!(
            heard[1],
            (map.frame_of(KEYS_POINT as f64, 0.0, rate), 192.0)
        );
        for (tick, bpm) in [(KEYS_RAMP.1, 40.0), (KEYS_FLAG, 20.0), (KEYS_LAST, 150.0)] {
            assert!(
                heard.contains(&(map.frame_of(tick as f64, 0.0, rate), bpm)),
                "at {tick}"
            );
        }
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
}
