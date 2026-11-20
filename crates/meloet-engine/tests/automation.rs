//! Automation on the audio thread (docs/architecture.md 21-5 to 21-7), frame by frame, with
//! instruments and effects that are arithmetic, at a tempo that puts 50 frames in a tick.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use meloet_engine::{
    AutoLane, BlockTiming, Effect, EffectContext, Engine, EngineCommand, EngineConfig, EngineParts,
    Instrument, InstrumentEvent, MIX_STEP, MixStrip, PLUGIN_POINTS, RAMP_S, Sequence,
    SequenceEvent, place_at,
};
use meloet_model::effects::EffectKind;
use meloet_model::{AutoTarget, Bus, Gain, InsertId};
use rtrb::{Consumer, Producer, RingBuffer};

const RATE: usize = 48_000;
const Q: usize = meloet_model::TICKS_PER_QUARTER as usize;
const TEMPO_CENTI: usize = 12_000 * 480 / Q;
const PER_TICK: usize = RATE * 6000 / (TEMPO_CENTI * Q);
const _: () = assert!(PER_TICK == 50 && PER_TICK * TEMPO_CENTI * Q == RATE * 6000);
const BLOCK: usize = 256;
const RAMP: usize = (RAMP_S * RATE as f32) as usize;

/// The same value on every frame.
struct Dc(f32);

impl Instrument for Dc {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn event(&mut self, _ev: InstrumentEvent) {}
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(self.0);
        right.fill(self.0);
    }
    fn reset(&mut self) {}
}

/// `LEVEL` while a key is down.
struct Held(u32);

impl Instrument for Held {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn event(&mut self, ev: InstrumentEvent) {
        match ev {
            InstrumentEvent::NoteOn { velocity, .. } if velocity > 0 => self.0 += 1,
            InstrumentEvent::NoteOn { .. } | InstrumentEvent::NoteOff { .. } => {
                self.0 = self.0.saturating_sub(1)
            }
            InstrumentEvent::AllNotesOff => self.0 = 0,
            _ => {}
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let level = if self.0 > 0 { 0.5 } else { 0.0 };
        left.fill(level);
        right.fill(level);
    }
    fn reset(&mut self) {
        self.0 = 0;
    }
}

/// Stands for a plugin that delays the sound by `latency` frames and multiplies what comes
/// in by its parameter 0, which moves at the frames of its points (each holds from its frame).
struct Late {
    line: Vec<f32>,
    at: usize,
    gain: f32,
    points: [(u32, f32); 64],
    count: usize,
}

impl Late {
    fn new(latency: usize) -> Late {
        Late {
            line: vec![0.0; latency.max(1)],
            at: 0,
            gain: 1.0,
            points: [(0, 0.0); 64],
            count: 0,
        }
    }
}

impl Effect for Late {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, index: u32, value: f32) {
        self.set_param_at(0, index, value);
    }
    fn takes_points(&self) -> bool {
        true
    }
    fn set_param_at(&mut self, frame: u32, index: u32, value: f32) {
        if index == 0 && self.count < self.points.len() {
            self.points[self.count] = (frame, value);
            self.count += 1;
        }
    }
    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        for i in 0..left.len() {
            for &(frame, value) in &self.points[..self.count] {
                if frame as usize == i {
                    self.gain = value;
                }
            }
            let out = self.line[self.at];
            self.line[self.at] = left[i] * self.gain;
            self.at = (self.at + 1) % self.line.len();
            left[i] = out;
            right[i] = out;
        }
        self.count = 0;
    }
    fn reset(&mut self) {}
    fn latency(&self) -> u32 {
        self.line.len() as u32
    }
}

/// Multiplies by its parameter 1 ÷ 12 at once, as the limiter's gain (0 to 12 dB) reads it.
struct Scale(f32);

impl Effect for Scale {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, index: u32, value: f32) {
        if index == 1 {
            self.0 = value / 12.0;
        }
    }
    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        for s in left.iter_mut().chain(right.iter_mut()) {
            *s *= self.0;
        }
    }
    fn reset(&mut self) {}
}

/// Writes down every point a plugin would get: the frame of the device's stream, the
/// parameter, the value.
struct Tape {
    tx: Producer<(u64, u32, f32)>,
    frames: u64,
    pending: [(u32, u32, f32); 256],
    count: usize,
}

impl Effect for Tape {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, _index: u32, _value: f32) {}
    fn takes_points(&self) -> bool {
        true
    }
    fn set_param_at(&mut self, frame: u32, index: u32, value: f32) {
        if self.count < self.pending.len() {
            self.pending[self.count] = (frame, index, value);
            self.count += 1;
        }
    }
    fn process(&mut self, left: &mut [f32], _right: &mut [f32], _ctx: &EffectContext) {
        for &(frame, id, value) in &self.pending[..self.count] {
            let _ = self.tx.push((self.frames + u64::from(frame), id, value));
        }
        // Where each stretch ends: id u32::MAX.
        let end = self.frames + left.len() as u64 - 1;
        let _ = self.tx.push((end, u32::MAX, 0.0));
        self.count = 0;
        self.frames += left.len() as u64;
    }
    fn reset(&mut self) {}
}

struct Rig {
    parts: EngineParts,
}

impl Rig {
    fn new() -> Rig {
        let mut rig = Rig {
            parts: Engine::new(EngineConfig::default()),
        };
        let map = common::flat_map(TEMPO_CENTI as u32, 4, 4);
        assert!(rig.parts.handle.set_tempo_map(map).is_ok());
        rig
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    fn instrument(&mut self, track: usize, instrument: Box<dyn Instrument>) {
        assert!(self.parts.handle.swap_instrument(track, instrument).is_ok());
    }

    fn effect(&mut self, track: usize, slot: usize, effect: Box<dyn Effect>) {
        let strip = MixStrip::Track(track);
        assert!(
            self.parts
                .handle
                .set_effect(strip, slot, Some(effect))
                .is_ok()
        );
    }

    fn lanes(&mut self, track: usize, lanes: Vec<AutoLane>) {
        assert!(self.parts.handle.set_automation(track, lanes).is_ok());
    }

    fn sequence(&mut self, track: usize, events: &[(u64, InstrumentEvent)]) {
        let events = (events.iter())
            .map(|&(tick, event)| SequenceEvent { tick, event })
            .collect();
        assert!(
            self.parts
                .handle
                .swap_sequence(track, Sequence::new(events))
                .is_ok()
        );
    }

    /// Renders `frames` frames in device blocks of `block`; the left and the right channel.
    fn run_in(&mut self, frames: usize, block: usize) -> (Vec<f32>, Vec<f32>) {
        let mut out = vec![0.0f32; block * 2];
        let (mut left, mut right) = (Vec::new(), Vec::new());
        while left.len() < frames {
            self.parts.engine.process(&mut out, BlockTiming::default());
            left.extend(out.as_chunks::<2>().0.iter().map(|f| f[0]));
            right.extend(out.as_chunks::<2>().0.iter().map(|f| f[1]));
            self.parts.handle.drop_retired();
        }
        left.truncate(frames);
        right.truncate(frames);
        (left, right)
    }

    fn run(&mut self, frames: usize) -> Vec<f32> {
        self.run_in(frames, BLOCK).0
    }
}

fn lane(target: AutoTarget, step: bool, points: &[(u64, f32)]) -> AutoLane {
    AutoLane {
        target,
        step,
        points: points.to_vec(),
        slot: 0,
        effect: None,
    }
}

fn insert(slot: usize, param: u32, effect: Option<EffectKind>, points: &[(u64, f32)]) -> AutoLane {
    AutoLane {
        target: AutoTarget::Insert {
            insert: InsertId(slot as u32 + 1),
            param,
        },
        step: false,
        points: points.to_vec(),
        slot,
        effect,
    }
}

fn gain_at(points: &[(u64, f32)], tick: f64) -> f32 {
    Gain::from_position(place_at(points, false, tick)).linear()
}

#[test]
fn the_line_between_points() {
    let points = [(100, 0.2), (200, 0.6), (300, 0.6), (301, 0.0)];
    assert_eq!(place_at(&points, false, 0.0), 0.2);
    assert_eq!(place_at(&points, false, 100.0), 0.2);
    assert!((place_at(&points, false, 150.0) - 0.4).abs() < 1e-6);
    assert_eq!(place_at(&points, false, 250.0), 0.6);
    assert!((place_at(&points, false, 300.5) - 0.3).abs() < 1e-6);
    assert_eq!(place_at(&points, false, 301.0), 0.0);
    assert_eq!(place_at(&points, false, 1e9), 0.0);
    assert_eq!(place_at(&points, true, 199.9), 0.2);
    assert_eq!(place_at(&points, true, 200.0), 0.6);
    assert_eq!(place_at(&points, true, 300.9), 0.6);
    assert_eq!(place_at(&[(5, 0.7)], true, 0.0), 0.7);
}

/// The fader's line and the gain of what comes out, frame by frame: right at the end of
/// every step of 32 frames from the start of a stretch, a straight line in gain between.
/// Stretches here start at every device block and every beat (24 000 frames).
#[test]
fn the_fader_follows_its_line_frame_by_frame() {
    let points = [(0, 0.8), (1000, 0.4), (2000, 0.95), (3000, 0.1)];
    let frames = 3200 * PER_TICK;
    let want = |f: usize| 0.25 * gain_at(&points, f as f64 / PER_TICK as f64);
    for block in [64, 256, 1000] {
        let mut rig = Rig::new();
        rig.instrument(0, Box::new(Dc(0.25)));
        rig.lanes(0, vec![lane(AutoTarget::Volume, false, &points)]);
        rig.send(EngineCommand::Play { from: 0 });
        let (out, _) = rig.run_in(frames, block);
        let mut starts: Vec<usize> = (0..frames).step_by(block).collect();
        starts.extend((0..frames).step_by(24_000));
        starts.sort();
        starts.dedup();
        starts.push(frames);
        let (mut before, mut worst, mut far) = (0.25f32, 0.0f32, 0.0f32);
        for stretch in starts.windows(2) {
            for step in (stretch[0]..stretch[1]).step_by(MIX_STEP) {
                let end = (step + MIX_STEP).min(stretch[1]);
                let target = want(end - 1);
                for (f, x) in out.iter().enumerate().take(end).skip(step) {
                    let line =
                        before + (target - before) * (f + 1 - step) as f32 / (end - step) as f32;
                    worst = worst.max((x - line).abs() / line);
                    far = far.max((x - want(f)).abs() / want(f));
                }
                before = target;
            }
        }
        assert!(worst < 2e-5, "block {block}: {worst}");
        // Off the curve by no more than what a step of 32 frames cuts across at a bend.
        assert!(far < 0.007, "block {block}: {far}");
    }
}

/// A jump is one tick from point to point; it comes out as a slope of 20 ms for the whole
/// way, no faster.
#[test]
fn a_jump_of_the_fader_is_a_slope_of_twenty_ms() {
    let points = [(0, 0.8), (2399, 0.8), (2400, 0.0)];
    let mut rig = Rig::new();
    rig.instrument(0, Box::new(Dc(0.25)));
    rig.lanes(0, vec![lane(AutoTarget::Volume, false, &points)]);
    rig.send(EngineCommand::Play { from: 0 });
    let out = rig.run(2500 * PER_TICK);
    let begins = 2399 * PER_TICK;
    let first_down = out.iter().position(|x| *x < 0.25 - 1e-6).unwrap();
    let silent = out.iter().position(|x| *x == 0.0).unwrap();
    assert!(
        first_down + MIX_STEP >= begins && first_down <= begins + MIX_STEP,
        "{first_down}"
    );
    // 0.8 of the whole way at the whole way in 20 ms.
    let slope = (0.8 * RAMP as f32) as usize;
    assert!(
        silent.abs_diff(first_down + slope) <= MIX_STEP,
        "{first_down} {silent}"
    );
    assert!(out[first_down..silent].windows(2).all(|w| w[1] <= w[0]));
    assert!(out[silent..].iter().all(|x| *x == 0.0));
}

#[test]
fn pan_and_send_lanes_play() {
    let mut rig = Rig::new();
    rig.instrument(0, Box::new(Dc(0.25)));
    rig.lanes(
        0,
        vec![
            lane(AutoTarget::Pan, false, &[(0, 0.0)]),
            lane(AutoTarget::Send(Bus::A), false, &[(0, 0.8)]),
        ],
    );
    let (left, right) = rig.run_in(RATE / 10, BLOCK);
    // All left; return A gets the track at 0 dB on top.
    assert!(
        (left[RATE / 10 - 1] - 0.5).abs() < 1e-3,
        "{}",
        left[RATE / 10 - 1]
    );
    assert!(right[RATE / 10 - 1].abs() < 1e-6);
    // The lanes go away: the hand's values come back (the middle, no send).
    rig.lanes(0, Vec::new());
    let (left, right) = rig.run_in(RATE / 10, BLOCK);
    assert!((left[RATE / 10 - 1] - 0.25).abs() < 1e-6);
    assert!((right[RATE / 10 - 1] - 0.25).abs() < 1e-6);
}

/// While stopped a lane plays its value where the song is.
#[test]
fn a_lane_plays_while_stopped_where_the_song_is() {
    let points = [(0, 0.8), (1000, 0.4)];
    let mut rig = Rig::new();
    rig.instrument(0, Box::new(Dc(0.25)));
    rig.lanes(0, vec![lane(AutoTarget::Volume, false, &points)]);
    rig.send(EngineCommand::Locate { tick: 500 });
    let out = rig.run(RATE / 10);
    let want = 0.25 * gain_at(&points, 500.0);
    assert!((out[RATE / 10 - 1] - want).abs() < 1e-4);
    // Held: the hand's value (0 dB here).
    rig.send(EngineCommand::HoldLane {
        track: 0,
        target: AutoTarget::Volume,
        on: true,
    });
    let out = rig.run(RATE / 10);
    assert!((out[RATE / 10 - 1] - 0.25).abs() < 1e-6);
    rig.send(EngineCommand::HoldLane {
        track: 0,
        target: AutoTarget::Volume,
        on: false,
    });
    let out = rig.run(RATE / 10);
    assert!((out[RATE / 10 - 1] - want).abs() < 1e-4);
}

/// An effect that comes with meloet gets `set_param` at the head of every 64 frames: a step
/// of its parameter is heard from the first of them at the step or after it.
#[test]
fn a_step_of_a_built_in_effect_lands_within_a_piece() {
    for at_tick in [100u64, 101, 333] {
        let mut rig = Rig::new();
        rig.instrument(0, Box::new(Dc(0.25)));
        rig.effect(0, 0, Box::new(Scale(1.0)));
        let points = [(0, 0.0), (at_tick, 1.0)];
        let mut l = insert(0, 1, Some(EffectKind::Limiter), &points);
        l.step = true;
        rig.lanes(0, vec![l]);
        rig.run(BLOCK * 4);
        rig.send(EngineCommand::Play { from: 0 });
        let out = rig.run(400 * PER_TICK);
        let heard = out.iter().position(|x| *x > 0.0).unwrap();
        let due = at_tick as usize * PER_TICK;
        assert!(heard >= due && heard < due + 64, "{at_tick}: {heard} {due}");
        assert!(out[heard..].iter().all(|x| (*x - 0.25).abs() < 1e-6));
    }
}

/// Two plugins that each delay by 100 frames: the step of the first one's parameter is read
/// 200 frames ahead, the second one's 100, and both meet the sound that comes in at that
/// place of the song: what comes out changes at the frame of the step, to the frame.
#[test]
fn a_parameter_is_read_as_far_ahead_as_the_slots_from_it_delay() {
    const STEP: u64 = 100;
    for slot in [0, 1] {
        for bypass_first in [false, true] {
            if slot == 0 && bypass_first {
                continue;
            }
            let mut rig = Rig::new();
            rig.instrument(0, Box::new(Dc(0.25)));
            rig.effect(0, 0, Box::new(Late::new(100)));
            rig.effect(0, 1, Box::new(Late::new(100)));
            if bypass_first {
                rig.send(EngineCommand::SetBypass {
                    strip: MixStrip::Track(0),
                    slot: 0,
                    bypassed: true,
                });
            }
            let mut step = insert(slot, 0, None, &[(0, 0.0), (STEP, 1.0)]);
            step.step = true;
            rig.lanes(0, vec![step]);
            rig.run(BLOCK * 4);
            rig.send(EngineCommand::Play { from: 0 });
            let out = rig.run(200 * PER_TICK);
            let heard = out.iter().position(|x| *x > 0.0).unwrap();
            assert_eq!(
                heard,
                STEP as usize * PER_TICK,
                "slot {slot} bypass {bypass_first}"
            );
        }
    }
}

/// The mute lane is read where the sequence is: on a track whose plugins delay by 200
/// frames, its step is heard at the frame of the step.
#[test]
fn a_mute_step_is_heard_at_its_place_on_a_track_that_delays() {
    for (step, block) in [(100u64, 256), (101, 64), (250, 1000)] {
        let mut rig = Rig::new();
        rig.instrument(0, Box::new(Held(0)));
        rig.effect(0, 0, Box::new(Late::new(100)));
        rig.effect(0, 1, Box::new(Late::new(100)));
        rig.sequence(
            0,
            &[
                (
                    0,
                    InstrumentEvent::NoteOn {
                        note: 60,
                        velocity: 100,
                    },
                ),
                (9000, InstrumentEvent::NoteOff { note: 60 }),
            ],
        );
        rig.lanes(
            0,
            vec![lane(AutoTarget::Mute, true, &[(0, 0.0), (step, 1.0)])],
        );
        rig.run_in(BLOCK * 4, block);
        rig.send(EngineCommand::Play { from: 0 });
        let (out, _) = rig.run_in(400 * PER_TICK, block);
        let sounding = out.iter().position(|x| *x > 0.0).unwrap();
        let gone = sounding + out[sounding..].iter().position(|x| *x == 0.0).unwrap();
        assert_eq!(gone, step as usize * PER_TICK, "step {step} block {block}");
    }
}

fn tape(rig: &mut Rig, slot: usize) -> Consumer<(u64, u32, f32)> {
    let (tx, rx) = RingBuffer::new(1 << 18);
    rig.effect(
        0,
        slot,
        Box::new(Tape {
            tx,
            frames: 0,
            pending: [(0, 0, 0.0); 256],
            count: 0,
        }),
    );
    rx
}

/// A plugin gets the value of the last frame of every stretch, a point at a bend of the
/// line inside one, nothing it was sent last.
#[test]
fn a_plugin_gets_its_points_in_the_stretch() {
    let mut rig = Rig::new();
    rig.instrument(0, Box::new(Dc(0.25)));
    let mut rx = tape(&mut rig, 0);
    let points = [(0, 0.0), (1010, 0.8), (1930, 1.0), (2400, 1.0)];
    rig.lanes(0, vec![insert(0, 7, None, &points)]);
    rig.run(BLOCK);
    while rx.pop().is_ok() {}
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(2600 * PER_TICK);
    let tape: Vec<(u64, u32, f32)> = std::iter::from_fn(|| rx.pop().ok()).collect();
    let mut last_end = None;
    let mut bends = 0;
    let mut sent = f32::NAN;
    for &(frame, id, value) in &tape {
        let frame = frame - BLOCK as u64;
        if id == u32::MAX {
            let tick = frame as f64 / PER_TICK as f64;
            let want = place_at(&points, false, tick);
            assert!((sent - want).abs() < 1e-6, "end {frame}: {sent} {want}");
            last_end = Some(frame);
            continue;
        }
        assert_eq!(id, 7);
        assert_ne!(value, sent, "the same value twice at {frame}");
        let tick = frame as f64 / PER_TICK as f64;
        if tick == 1010.0 || tick == 1930.0 {
            bends += 1;
        }
        assert!(
            (value - place_at(&points, false, tick)).abs() < 1e-6,
            "{frame}"
        );
        sent = value;
    }
    assert!(last_end.is_some());
    assert_eq!(bends, 2, "a point at each bend");
    // After 2400 the line is flat: nothing more is sent.
    let after = (tape.iter()).filter(|p| p.1 == 7 && p.0 > 2400 * PER_TICK as u64);
    assert_eq!(after.count(), 0);
}

/// More lanes on one plugin than fit in a stretch: the last value of every one of them
/// gets there, those left over in the stretches after.
#[test]
fn a_plugin_with_too_many_lanes_gets_every_last_value() {
    const LANES: u32 = 200;
    let mut rig = Rig::new();
    rig.instrument(0, Box::new(Dc(0.25)));
    let mut rx = tape(&mut rig, 0);
    let lanes = (0..LANES)
        .map(|i| {
            let mut l = insert(0, i, None, &[(0, i as f32 / LANES as f32)]);
            l.target = AutoTarget::Insert {
                insert: InsertId(1),
                param: i,
            };
            l
        })
        .collect();
    rig.lanes(0, lanes);
    let mut got = vec![None; LANES as usize];
    let mut stretches = 0;
    for _ in 0..4 {
        rig.run(BLOCK);
        let mut this = 0;
        while let Ok((_, id, value)) = rx.pop() {
            if id == u32::MAX {
                stretches += 1;
                continue;
            }
            this += 1;
            got[id as usize] = Some(value);
        }
        assert!(this <= PLUGIN_POINTS);
    }
    assert!(stretches >= 2);
    for (i, value) in got.iter().enumerate() {
        assert_eq!(*value, Some(i as f32 / LANES as f32), "lane {i}");
    }
}

/// An instrument's lanes reach it before it is asked for the stretch, read as far ahead
/// as the track's delays.
#[test]
fn an_instrument_gets_its_points_ahead() {
    struct Points(Producer<(u64, u32, f32)>, u64, Arc<AtomicU32>);
    impl Instrument for Points {
        fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
        fn event(&mut self, _ev: InstrumentEvent) {}
        fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
            left.fill(0.0);
            right.fill(0.0);
            self.1 += left.len() as u64;
        }
        fn reset(&mut self) {}
        fn latency(&self) -> u32 {
            self.2.load(Relaxed)
        }
        fn set_param_at(&mut self, frame: u32, id: u32, value: f32) {
            let _ = self.0.push((self.1 + u64::from(frame), id, value));
        }
    }
    let (tx, mut rx) = RingBuffer::new(1 << 16);
    let mut rig = Rig::new();
    let says = Arc::new(AtomicU32::new(300));
    rig.instrument(0, Box::new(Points(tx, 0, says)));
    let points = [(0, 0.0), (100, 0.0), (101, 1.0)];
    rig.lanes(
        0,
        vec![lane(AutoTarget::Instrument { param: 3 }, false, &points)],
    );
    rig.run(BLOCK);
    while rx.pop().is_ok() {}
    let start = (BLOCK) as u64;
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(200 * PER_TICK);
    let tape: Vec<_> = std::iter::from_fn(|| rx.pop().ok()).collect();
    // The jump: the point at tick 100 (0.0) and the one at 101 (1.0), each 300 frames early.
    let jump = tape.iter().find(|p| p.2 == 1.0).unwrap();
    assert_eq!(jump.0 - start, 101 * PER_TICK as u64 - 300);
    assert!(tape.iter().all(|p| p.1 == 3));
}

/// Inserts that change places take their lanes with them.
#[test]
fn lanes_go_with_their_slot() {
    let mut rig = Rig::new();
    rig.instrument(0, Box::new(Dc(0.25)));
    let mut rx = tape(&mut rig, 0);
    rig.effect(0, 1, Box::new(Scale(12.0)));
    rig.lanes(0, vec![insert(0, 5, None, &[(0, 0.5)])]);
    rig.run(BLOCK);
    assert!(std::iter::from_fn(|| rx.pop().ok()).any(|p| p.1 == 5));
    rig.send(EngineCommand::MoveEffect {
        strip: MixStrip::Track(0),
        from: 0,
        to: 2,
    });
    rig.send(EngineCommand::SetAutomation {
        track: 0,
        set: None,
    });
    let mut lane = insert(2, 5, None, &[(0, 0.75)]);
    lane.slot = 2;
    rig.lanes(0, vec![lane]);
    rig.run(BLOCK * 8);
    let got: Vec<_> = std::iter::from_fn(|| rx.pop().ok())
        .filter(|p| p.1 == 5)
        .collect();
    assert_eq!(got.last().map(|p| p.2), Some(0.75));
}

/// Nothing on the audio thread allocates or frees with 200 lanes and 10 000 points, sets
/// swapped while playing, lanes held, slots moved, the loop going round.
#[test]
fn automation_allocates_nothing_on_the_audio_thread() {
    let mut rig = Rig::new();
    for track in 0..8 {
        rig.instrument(track, Box::new(Held(0)));
        rig.effect(track, 0, Box::new(Scale(12.0)));
        rig.effect(track, 1, Box::new(Late::new(64)));
        let notes: Vec<(u64, InstrumentEvent)> = (0..40)
            .flat_map(|i| {
                [
                    (
                        i * 240,
                        InstrumentEvent::NoteOn {
                            note: 60,
                            velocity: 90,
                        },
                    ),
                    (i * 240 + 200, InstrumentEvent::NoteOff { note: 60 }),
                ]
            })
            .collect();
        rig.sequence(track, &notes);
    }
    let set = |seed: u64| -> Vec<AutoLane> {
        let points = |n: u64, step: bool| -> Vec<(u64, f32)> {
            let n = n + 2;
            (0..n)
                .map(|i| {
                    let v = ((i * 37 + seed) % 100) as f32 / 100.0;
                    (i * 7 + u64::from(step), v)
                })
                .collect()
        };
        let mut lanes = vec![
            lane(AutoTarget::Volume, false, &points(50, false)),
            lane(AutoTarget::Pan, false, &points(50, false)),
            lane(AutoTarget::Send(Bus::A), false, &points(50, false)),
            lane(AutoTarget::Send(Bus::B), false, &points(50, false)),
            lane(AutoTarget::Mute, true, &points(25, true)),
        ];
        for p in 0..10 {
            let mut l = insert(0, 1, Some(EffectKind::Limiter), &points(50, false));
            l.target = AutoTarget::Insert {
                insert: InsertId(1),
                param: if p == 0 { 1 } else { 100 + p },
            };
            lanes.push(l);
            let mut l = insert(1, p, None, &points(50, false));
            l.slot = 1;
            lanes.push(l);
        }
        lanes
    };
    for track in 0..8 {
        rig.lanes(track, set(track as u64));
    }
    let lanes: usize = 8 * set(0).len();
    let points: usize = 8 * set(0).iter().map(|l| l.points.len()).sum::<usize>();
    assert!(lanes >= 200 && points >= 10_000, "{lanes} {points}");
    rig.send(EngineCommand::SetLoop {
        start: 0,
        end: 1920,
        on: true,
    });
    rig.send(EngineCommand::Play { from: 0 });
    for round in 0..60u64 {
        let track = (round % 8) as usize;
        match round % 5 {
            0 => rig.lanes(track, set(round)),
            1 => rig.send(EngineCommand::HoldLane {
                track,
                target: AutoTarget::Volume,
                on: round % 2 == 1,
            }),
            2 => rig.send(EngineCommand::MoveEffect {
                strip: MixStrip::Track(track),
                from: 0,
                to: 1,
            }),
            3 => rig.send(EngineCommand::SetAutomation { track, set: None }),
            _ => rig.send(EngineCommand::Locate {
                tick: round * 37 % 1900,
            }),
        }
        rig.run_in(BLOCK * 3, [64, 256, 1000][round as usize % 3]);
    }
    rig.send(EngineCommand::Stop { t_us: 0 });
    rig.run(BLOCK * 4);
    assert_eq!(common::audio_block_counts(), (0, 0));
}
