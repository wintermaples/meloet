//! A track whose plugins delay the sound gets its notes early by that much, so that it reaches
//! the master in time with the others (docs/architecture.md 20-8). Keys played by hand are
//! not sent early. Also: what plugins are told of the song.

mod common;

use meloet_engine::{
    BlockTiming, Engine, EngineCommand, EngineConfig, EngineParts, Instrument, InstrumentEvent,
    MidiIn, Sequence, SequenceEvent,
};
use meloet_plugin::ipc::Transport;
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

const RATE: u64 = 48_000;
const Q: u64 = meloet_model::TICKS_PER_QUARTER;
/// At 120 beats a minute: two quarter notes a second.
const TICKS_PER_S: u64 = 2 * Q;
const BLOCK: usize = 256;
const LEVEL: f32 = 0.25;

/// Stands for a plugin that delays: every key pressed is one sample of `LEVEL`, `delay` frames
/// after the key arrived.
struct Late {
    delay: usize,
    /// Frames until each click still to come; 0 is a free place.
    due: [usize; 32],
    /// The delay it tells the engine of.
    says: Arc<AtomicU32>,
}

impl Instrument for Late {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}

    fn event(&mut self, ev: InstrumentEvent) {
        if matches!(ev, InstrumentEvent::NoteOn { velocity, .. } if velocity > 0)
            && let Some(free) = self.due.iter_mut().find(|d| **d == 0)
        {
            *free = self.delay + 1;
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let mut sample = 0.0;
            for due in self.due.iter_mut().filter(|d| **d > 0) {
                *due -= 1;
                if *due == 0 {
                    sample += LEVEL;
                }
            }
            *l = sample;
            *r = sample;
        }
    }

    fn reset(&mut self) {
        self.due = [0; 32];
    }

    fn latency(&self) -> u32 {
        self.says.load(Relaxed)
    }
}

/// Writes down every event with the frame (counted from the first render) it took effect on.
struct Tape {
    tx: Producer<(u64, InstrumentEvent)>,
    frames: u64,
    says: Arc<AtomicU32>,
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
        self.says.load(Relaxed)
    }
}

struct Rig {
    parts: EngineParts,
    frames: u64,
    /// The delay the instrument of each track says it has.
    says: [Arc<AtomicU32>; 2],
}

impl Rig {
    fn new() -> Rig {
        let mut rig = Rig {
            parts: Engine::new(EngineConfig::default()),
            frames: 0,
            says: Default::default(),
        };
        let map = common::flat_map(12_000, 4, 4);
        assert!(rig.parts.handle.set_tempo_map(map).is_ok());
        rig
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    fn instrument(&mut self, track: usize, instrument: Box<dyn Instrument>) {
        assert!(self.parts.handle.swap_instrument(track, instrument).is_ok());
    }

    fn tape(&mut self, track: usize) -> Consumer<(u64, InstrumentEvent)> {
        let (tx, rx) = RingBuffer::new(1 << 16);
        let tape = Tape {
            tx,
            frames: 0,
            says: self.says[track].clone(),
        };
        self.instrument(track, Box::new(tape));
        rx
    }

    /// An instrument that delays by `delay` frames; what it says of that is `lookahead`'s.
    fn late(&mut self, track: usize, delay: usize) {
        let late = Late {
            delay,
            due: [0; 32],
            says: self.says[track].clone(),
        };
        self.instrument(track, Box::new(late));
    }

    fn sequence(&mut self, track: usize, events: &[(u64, InstrumentEvent)]) {
        let events = events
            .iter()
            .map(|&(tick, event)| SequenceEvent { tick, event })
            .collect();
        let sequence = Sequence::new(events);
        assert!(self.parts.handle.swap_sequence(track, sequence).is_ok());
    }

    /// What the track's instrument says its delay is, from the next block on.
    fn lookahead(&mut self, track: usize, frames: u32) {
        self.says[track].store(frames, Relaxed);
    }

    /// Renders `blocks` device blocks; the left channel.
    fn run(&mut self, blocks: usize) -> Vec<f32> {
        let mut left = Vec::new();
        let mut out = [0.0f32; BLOCK * 2];
        for _ in 0..blocks {
            let callback_us = 1_000_000 + self.frames * 1_000_000 / RATE;
            let timing = BlockTiming {
                callback_us,
                playback_us: callback_us,
            };
            self.parts.engine.process(&mut out, timing);
            left.extend(out.as_chunks::<2>().0.iter().map(|f| f[0]));
            self.frames += BLOCK as u64;
        }
        left
    }
}

/// The frame of `tick` at 120 (a whole one for every tick used here).
fn frame_of(tick: u64) -> u64 {
    tick * RATE / TICKS_PER_S
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

/// Every frame that is not silent, with its level.
fn clicks(samples: &[f32]) -> Vec<(usize, f32)> {
    (samples.iter().enumerate())
        .filter(|(_, x)| **x != 0.0)
        .map(|(i, x)| (i, *x))
        .collect()
}

fn drain(tape: &mut Consumer<(u64, InstrumentEvent)>) -> Vec<(u64, InstrumentEvent)> {
    std::iter::from_fn(|| tape.pop().ok()).collect()
}

#[test]
fn a_track_that_delays_starts_its_notes_with_the_others() {
    const DELAY: usize = 300;
    let notes = [
        (0, on(60)),
        (Q, on(62)),
        (2 * Q, on(64)),
        (2 * Q + 40, on(65)),
    ];

    let at = |tick: u64| frame_of(tick) as usize;
    let play = |ahead: u32| {
        let mut rig = Rig::new();
        rig.late(0, 0);
        rig.late(1, DELAY);
        rig.sequence(0, &notes);
        rig.sequence(1, &notes);
        rig.lookahead(1, ahead);
        rig.run(8);
        rig.send(EngineCommand::Play { from: 0 });
        clicks(&rig.run(250))
    };

    // A plugin that does not say its delay is late by it.
    let apart: Vec<(usize, f32)> = [0, Q, 2 * Q, 2 * Q + 40]
        .iter()
        .flat_map(|&tick| [(at(tick), LEVEL), (at(tick) + DELAY, LEVEL)])
        .collect();
    assert_eq!(play(0), apart);

    // When it says it, every note after the first frames of the play comes out of both tracks on the
    // same frame. The note at the very start cannot be sent early: it is late, not lost.
    assert_eq!(
        play(DELAY as u32),
        [
            (at(0), LEVEL),
            (at(0) + DELAY, LEVEL),
            (at(Q), 2.0 * LEVEL),
            (at(2 * Q), 2.0 * LEVEL),
            (at(2 * Q + 40), 2.0 * LEVEL),
        ]
    );
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn every_event_of_the_sequence_goes_out_early_by_the_lookahead_also_round_a_loop() {
    const AHEAD: u64 = 1000;
    let song = [
        (Q, on(60)),
        (
            5 * Q / 4,
            InstrumentEvent::Control {
                controller: 64,
                value: 127,
            },
        ),
        (35 * Q / 24, off(60)),
        // Held over the loop's end: let go where the loop goes round.
        (15 * Q / 8, on(72)),
        (25 * Q / 6, off(72)),
    ];
    let play = |ahead: u64| {
        let mut rig = Rig::new();
        let mut tape = rig.tape(0);
        rig.sequence(0, &song);
        rig.lookahead(0, ahead as u32);
        rig.send(EngineCommand::SetLoop {
            start: Q,
            end: 2 * Q,
            on: true,
        });
        rig.send(EngineCommand::Play { from: Q / 2 });
        rig.run(600);
        drain(&mut tape)
    };
    let (plain, early) = (play(0), play(AHEAD));
    assert!(plain.len() > 20, "{}", plain.len());
    // Three times round the loop and more, the same events in the same order, each exactly
    // the lookahead earlier.
    let moved: Vec<_> = plain.iter().map(|&(f, e)| (f - AHEAD, e)).collect();
    assert_eq!(early[..moved.len() - 8], moved[..moved.len() - 8]);
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn keys_played_by_hand_are_not_sent_early() {
    let played = |ahead: u32| {
        let mut rig = Rig::new();
        let mut tape = rig.tape(0);
        rig.send(EngineCommand::SetPlayTarget { track: 0 });
        rig.lookahead(0, ahead);
        rig.send(EngineCommand::Play { from: 0 });
        rig.run(20);
        let key = MidiIn {
            event: on(60),
            t_us: 1_000_000 + rig.frames * 1_000_000 / RATE,
        };
        rig.parts.midi_in.push(key).expect("midi ring full");
        rig.run(20);
        drain(&mut tape)
    };
    let (plain, ahead) = (played(0), played(4000));
    assert_eq!(plain, [(20 * BLOCK as u64, on(60))]);
    assert_eq!(ahead, plain);
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn a_lookahead_that_changes_while_playing_loses_and_doubles_nothing() {
    let notes: Vec<(u64, InstrumentEvent)> = (0..200).map(|n| (n * Q / 8, on(60))).collect();
    let mut rig = Rig::new();
    let mut tape = rig.tape(0);
    rig.sequence(0, &notes);
    rig.send(EngineCommand::Play { from: 0 });
    let mut heard = Vec::new();
    for ahead in [0, 700, 64, 5000, 0, 333] {
        rig.lookahead(0, ahead);
        rig.run(40);
        heard.extend(drain(&mut tape));
    }
    // 240 blocks are 2.56 quarter notes, and the last lookahead does not reach the next note:
    // the 21 notes up to there are out, each once.
    let (first, last) = (heard[0], heard[heard.len() - 1]);
    assert_eq!(first, (0, on(60)));
    assert_eq!(heard.len(), 21);
    assert!(heard.windows(2).all(|w| w[0].0 <= w[1].0));
    assert!(last.0 <= rig.frames);
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn plugins_are_told_where_the_song_is() {
    let mut rig = Rig::new();
    rig.run(2);
    let told = |rig: &Rig| rig.parts.handle.plugin_block().transport();
    let stopped = told(&rig);
    assert_eq!(stopped.flags, 0);
    assert_eq!((stopped.meter_num, stopped.meter_den), (4, 4));
    assert_eq!(stopped.tempo_bpm, 120.0);
    assert_eq!(stopped.position_quarters, 0.0);

    rig.send(EngineCommand::SetLoop {
        start: 4 * Q,
        end: 8 * Q,
        on: true,
    });
    rig.send(EngineCommand::Play { from: 6 * Q });
    rig.run(10);
    // The last stretch rendered began at the start of the tenth block.
    let playing = told(&rig);
    assert_eq!(playing.flags, Transport::PLAYING | Transport::LOOPING);
    let frames = 9 * BLOCK as u64;
    let quarters = 6.0 + frames as f64 / (RATE / 2) as f64;
    assert!((playing.position_quarters - quarters).abs() < 1e-9);
    assert_eq!(playing.bar_start_quarters, 4.0);
    assert_eq!(
        (playing.loop_start_quarters, playing.loop_end_quarters),
        (4.0, 8.0)
    );
    assert_eq!(playing.position_frames, (frame_of(6 * Q) + frames) as i64);

    // Six eighths in a bar: the tempo is still quarter notes.
    let map = common::flat_map(9_000, 6, 8);
    assert!(rig.parts.handle.set_tempo_map(map).is_ok());
    rig.run(1);
    let eighths = told(&rig);
    assert_eq!((eighths.meter_num, eighths.meter_den), (6, 8));
    assert_eq!(eighths.tempo_bpm, 90.0);
    assert_eq!(common::audio_block_counts(), (0, 0));
}
