//! A track whose plugins delay the sound gets its notes early by that much, so that it reaches
//! the master in time with the others (docs/architecture.md 20-8). Keys played by hand are
//! not sent early. The master's plugins delay everything: the song reaches the ear that much
//! later, and the position heard, the keys and the metronome follow. Also: what plugins are
//! told of the song.

mod common;

use meloet_engine::{
    BlockTiming, ClickSound, Effect, EffectContext, Engine, EngineCommand, EngineConfig,
    EngineParts, HeardPosition, Instrument, InstrumentEvent, MidiIn, MixStrip, PluginBlock,
    RecentAt, Recorded, Sequence, SequenceEvent, TrackSet, TransportState,
};
use meloet_plugin::ipc::Transport;
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering::Relaxed};

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
    /// Keys not yet arrived, by `t_us`.
    keys: Vec<MidiIn>,
}

impl Rig {
    fn new() -> Rig {
        let mut rig = Rig {
            parts: Engine::new(EngineConfig::default()),
            frames: 0,
            says: Default::default(),
            keys: Vec::new(),
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
            while self.keys.first().is_some_and(|k| k.t_us <= callback_us) {
                let key = self.keys.remove(0);
                self.parts.midi_in.push(key).expect("midi ring full");
            }
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

/// When frame `n` (counted from the first render) reaches the DAC, as [`Rig::run`] says it.
fn dac_us(n: u64) -> f64 {
    let head = n / BLOCK as u64 * BLOCK as u64;
    (1_000_000 + head * 1_000_000 / RATE) as f64 + (n - head) as f64 * 1e6 / RATE as f64
}

impl Rig {
    /// A plugin on the master that delays everything by what the returned number says.
    fn master_lag(&mut self, frames: u32) -> Arc<AtomicU32> {
        let says = Arc::new(AtomicU32::new(frames));
        let lag = Box::new(Lag::new(says.clone()));
        let set = self.parts.handle.set_effect(MixStrip::Master, 0, Some(lag));
        assert!(set.is_ok());
        says
    }

    fn key(&mut self, t_us: f64, event: InstrumentEvent) {
        let t_us = t_us.round() as u64;
        self.keys.push(MidiIn { event, t_us });
        self.keys.sort_by_key(|k| k.t_us);
    }

    /// At the next whole microsecond: a moment on the first frame of a stretch is in it.
    fn heard(&self, t_us: f64) -> HeardPosition {
        self.parts
            .handle
            .heard_at(t_us.ceil() as u64)
            .expect("rendered")
    }

    fn recorded(&mut self) -> Vec<Recorded> {
        std::iter::from_fn(|| self.parts.handle.pop_recorded()).collect()
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

/// Stands for a plugin effect: where the song is, as its block is told when it runs.
struct Told {
    block: Arc<PluginBlock>,
    position_frames: Arc<AtomicI64>,
}

impl Effect for Told {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, _index: u32, _value: f32) {}
    fn process(&mut self, _left: &mut [f32], _right: &mut [f32], _ctx: &EffectContext) {
        let told = self.block.transport().position_frames;
        self.position_frames.store(told, Relaxed);
    }
    fn reset(&mut self) {}
}

/// The returns and the master are not cut into stretches: their plugins are told the first
/// frame of the piece, also when the tracks' stretches cut it.
#[test]
fn plugins_on_the_returns_and_the_master_are_told_the_head_of_the_piece() {
    let mut rig = Rig::new();
    let _tape = rig.tape(0);
    // A key or a key let go every 100 frames, 50 frames into the first block.
    let keys: Vec<_> = (0..64)
        .map(|n| (n * 8 + 4, if n % 2 == 0 { on(60) } else { off(60) }))
        .collect();
    rig.sequence(0, &keys);
    let block = rig.parts.handle.plugin_block().clone();
    let told: [Arc<AtomicI64>; 2] = Default::default();
    for (strip, position_frames) in [
        (MixStrip::Return(0), &told[0]),
        (MixStrip::Master, &told[1]),
    ] {
        let effect = Told {
            block: block.clone(),
            position_frames: position_frames.clone(),
        };
        let set = rig
            .parts
            .handle
            .set_effect(strip, 0, Some(Box::new(effect)));
        assert!(set.is_ok());
    }
    rig.send(EngineCommand::Play { from: 0 });
    for n in 0..20 {
        rig.run(1);
        let head = (n * BLOCK) as i64;
        let got = [told[0].load(Relaxed), told[1].load(Relaxed)];
        assert_eq!(got, [head, head], "block {n}");
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn a_sequence_grown_after_the_lane_went_round_ahead_sends_nothing_twice() {
    const AHEAD: u32 = 4800;
    let song = [
        (0, on(60)),
        (Q / 2, off(60)),
        (Q, on(62)),
        (Q + 100, off(62)),
    ];
    let mut rig = Rig::new();
    let mut tape = rig.tape(0);
    rig.sequence(0, &song);
    rig.lookahead(0, AHEAD);
    rig.send(EngineCommand::SetLoop {
        start: 0,
        end: 2 * Q,
        on: true,
    });
    rig.send(EngineCommand::Play { from: 0 });
    // Inside the lookahead of the loop's end: the next pass's first note is out already.
    rig.run(176);
    assert!(rig.frames + u64::from(AHEAD) > frame_of(2 * Q) && rig.frames < frame_of(2 * Q));
    assert_eq!(drain(&mut tape).last().map(|e| e.1), Some(on(60)));
    let mut grown = song.to_vec();
    grown.insert(1, (Q / 4, on(64)));
    let events = grown
        .iter()
        .map(|&(tick, event)| SequenceEvent { tick, event })
        .collect();
    assert!(
        rig.parts
            .handle
            .grow_sequence(0, Sequence::new(events))
            .is_ok()
    );
    rig.run(137);
    let got: Vec<_> = drain(&mut tape).into_iter().map(|e| e.1).collect();
    assert_eq!(got, [on(64), off(60), on(62), off(62)]);
    assert_eq!(common::audio_block_counts(), (0, 0));
}

const LAG_LINE: usize = 1 << 18;

/// Stands for a plugin on the master that looks ahead: what comes in comes out as many frames
/// later as it says.
struct Lag {
    says: Arc<AtomicU32>,
    line: [Box<[f32]>; 2],
    at: usize,
}

impl Lag {
    fn new(says: Arc<AtomicU32>) -> Lag {
        let line = || vec![0.0f32; LAG_LINE].into_boxed_slice();
        Lag {
            says,
            line: [line(), line()],
            at: 0,
        }
    }
}

impl Effect for Lag {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, _index: u32, _value: f32) {}
    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        let delay = (self.says.load(Relaxed) as usize).min(LAG_LINE - 1);
        let frames = left.len();
        for (side, samples) in [left, right].into_iter().enumerate() {
            let line = &mut self.line[side];
            for (i, x) in samples.iter_mut().enumerate() {
                let at = (self.at + i) % LAG_LINE;
                line[at] = *x;
                *x = line[(at + LAG_LINE - delay) % LAG_LINE];
            }
        }
        self.at = (self.at + frames) % LAG_LINE;
    }
    fn reset(&mut self) {}
    fn latency(&self) -> u32 {
        self.says.load(Relaxed)
    }
}

/// Delays a plugin on the master says it has, from none to more than a beat at 120.
const LAGS: [u32; 4] = [0, 100, 1_000, 30_000];

/// `frames` of the song in ticks at 120.
fn ticks(frames: f64) -> f64 {
    frames * TICKS_PER_S as f64 / RATE as f64
}

fn lag_us(frames: u32) -> f64 {
    f64::from(frames) * 1e6 / RATE as f64
}

/// Starts of sounds: a sound after at least 200 silent frames.
fn onsets(samples: &[f32]) -> Vec<usize> {
    let mut quiet = usize::MAX;
    let mut found = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        if s.abs() > 1e-6 {
            if quiet >= 200 {
                found.push(i);
            }
            quiet = 0;
        } else {
            quiet = quiet.saturating_add(1);
        }
    }
    found
}

/// Each beat comes out of the master as many frames after its own as the master's plugin
/// says, and the position heard when that frame reaches the DAC is the beat: on the first
/// pass, just after the loop went round (the end of the pass is still heard), and on the
/// next pass. The position moves on all the time.
#[test]
fn the_position_heard_is_behind_the_dac_by_the_masters_delay() {
    for lag in LAGS {
        let mut rig = Rig::new();
        rig.master_lag(lag);
        rig.late(0, 0);
        let beats: Vec<_> = (0..12).map(|b| (b * Q, on(60))).collect();
        rig.sequence(0, &beats);
        rig.send(EngineCommand::SetLoop {
            start: 4 * Q,
            end: 8 * Q,
            on: true,
        });
        rig.run(4);
        let start = rig.frames;
        rig.send(EngineCommand::Play { from: 0 });

        // (frame from the start, tick, lap) of each beat as the song plays it.
        let seam = frame_of(8 * Q);
        let played: Vec<(u64, u64, u32)> = (0..8)
            .map(|b| (frame_of(b * Q), b * Q, 0))
            .chain((0..4).map(|b| (seam + frame_of(b * Q), (4 + b) * Q, 1)))
            .collect();
        // The moments the beats after the first come out, and the one halfway through the
        // delay after the loop went round, where the end of the first pass is still heard.
        let mut checks: Vec<(f64, f64, u32)> = (played.iter().skip(1))
            .map(|&(frame, tick, lap)| (dac_us(start + frame) + lag_us(lag), tick as f64, lap))
            .collect();
        if lag > 0 {
            let half = dac_us(start + seam) + lag_us(lag) / 2.0;
            let tick = (8 * Q) as f64 - ticks(f64::from(lag) / 2.0);
            checks.insert(7, (half, tick, 0));
        }
        let mut checks = checks.into_iter().peekable();
        // The clock keeps the newest 512 stretches only: read as the song is rendered.
        let mut t = dac_us(start) + lag_us(lag) + 1.0;
        let mut last = None::<HeardPosition>;
        let mut out = Vec::new();
        for _ in 0..(frame_of(12 * Q) + u64::from(lag)) as usize / BLOCK {
            out.extend(rig.run(1));
            let end = dac_us(rig.frames);
            while let Some((at, tick, lap)) = checks.next_if(|c| c.0 < end) {
                let h = rig.heard(at);
                let label = format!("lag {lag}, tick {tick} of lap {lap}");
                assert!((h.tick - tick).abs() < 0.01, "{label}: {}", h.tick);
                let (state, run_lap) = (h.state, h.run_lap);
                assert_eq!((state, run_lap), (TransportState::Playing, lap), "{label}");
            }
            while t < end {
                let h = rig.heard(t);
                assert!(
                    h.ticks_per_us > 0.0,
                    "lag {lag}: at {t} the position stands"
                );
                if let Some(last) = last {
                    let went_round = h.run_lap == last.run_lap + 1 && h.tick < last.tick;
                    assert!(h.tick >= last.tick || went_round, "lag {lag}: back at {t}");
                }
                last = Some(h);
                t += 37.0;
            }
        }
        assert!(checks.next().is_none(), "lag {lag}: every moment was read");
        let sounded: Vec<usize> = clicks(&out).iter().map(|c| c.0).collect();
        let want: Vec<usize> = (played.iter())
            .map(|p| (p.0 + u64::from(lag)) as usize)
            .filter(|&f| f < out.len())
            .collect();
        assert_eq!(
            sounded, want,
            "lag {lag}: the master's plugin delays the song"
        );
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
}

/// A key played when a beat comes out of the master lands on that beat: in a recording
/// (with the count-in's last half beat), where a stop is heard, and as a key kept
/// afterwards, also one played before the play's first frame came out.
#[test]
fn keys_played_with_the_beat_that_comes_out_land_on_it() {
    for lag in LAGS {
        let mut rig = Rig::new();
        rig.master_lag(lag);
        rig.run(4);
        let start = rig.frames;
        // From bar 2 with a bar of count-in: the song is at tick 0 on `start`.
        rig.send(EngineCommand::Record {
            from: 4 * Q,
            count_in_beats: 4,
            muted: TrackSet::default(),
        });
        let out_us = |start: u64, tick: u64| dac_us(start + frame_of(tick)) + lag_us(lag);
        let keys = [(62, 3 * Q), (63, 7 * Q / 2 + 40)]
            .into_iter()
            .chain((4..=8).map(|b| (60, b * Q)));
        for (note, tick) in keys {
            let t = out_us(start, tick);
            rig.key(t, on(note));
        }
        let stop_us = out_us(start, 9 * Q).round();
        rig.run((frame_of(9 * Q) + u64::from(lag)) as usize / BLOCK + 2);
        rig.send(EngineCommand::Stop {
            t_us: stop_us as u64,
        });
        rig.run(1);
        let got = rig.recorded();
        let events: Vec<(u8, f64)> = (got.iter())
            .filter_map(|r| match *r {
                Recorded::Event {
                    event: InstrumentEvent::NoteOn { note, .. },
                    tick,
                    ..
                } => Some((note, tick)),
                _ => None,
            })
            .collect();
        let want = [(63, 7 * Q / 2 + 40)]
            .into_iter()
            .chain((4..=8).map(|b| (60, b * Q)));
        assert_eq!(events.len(), 6, "lag {lag}: {events:?}");
        for ((note, tick), (want_note, want_tick)) in events.iter().zip(want) {
            assert_eq!(*note, want_note, "lag {lag}");
            let off = tick - want_tick as f64;
            assert!(
                off.abs() < 0.01,
                "lag {lag}: key at {want_tick} lands {off} off"
            );
        }
        let stopped = got.iter().find_map(|r| match *r {
            Recorded::Stopped { tick, .. } => Some(tick),
            _ => None,
        });
        let off = stopped.expect("stopped") - (9 * Q) as f64;
        assert!(off.abs() < 0.01, "lag {lag}: the stop is {off} off");

        rig.run(4);
        let start = rig.frames;
        rig.send(EngineCommand::Play { from: 0 });
        let early = dac_us(start) + lag_us(lag) / 2.0;
        rig.key(early, on(70));
        for b in 1..=2 {
            let t = out_us(start, b * Q);
            rig.key(t, on(71));
        }
        rig.run((frame_of(3 * Q) + u64::from(lag)) as usize / BLOCK);
        let kept: Vec<(u8, f64)> = (rig.recorded().iter())
            .filter_map(|r| match *r {
                Recorded::Recent {
                    event: InstrumentEvent::NoteOn { note, .. },
                    at: Some(RecentAt { tick, .. }),
                    ..
                } => Some((note, tick)),
                _ => None,
            })
            .collect();
        let want = [
            (70, -ticks(f64::from(lag) / 2.0)),
            (71, Q as f64),
            (71, 2.0 * Q as f64),
        ];
        assert_eq!(kept.len(), 3, "lag {lag}: {kept:?}");
        for ((note, tick), (want_note, want_tick)) in kept.iter().zip(want) {
            assert_eq!(*note, want_note, "lag {lag}");
            assert!(
                (tick - want_tick).abs() < 0.01,
                "lag {lag}: {tick} {want_tick}"
            );
        }
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
}

/// The metronome joins the output after the master's plugins: its clicks wait as long as
/// those delay the song, so each click comes out on the frame its beat does, in the
/// count-in too, also when the delay is longer than a beat.
#[test]
fn the_metronome_clicks_with_the_song_behind_the_masters_delay() {
    for lag in LAGS {
        let blocks = (frame_of(7 * Q) + u64::from(lag) + 4_000) as usize / BLOCK;
        let render = |metronome: bool, record: bool| {
            let mut rig = Rig::new();
            rig.master_lag(lag);
            rig.late(0, 0);
            if !metronome {
                let beats: Vec<_> = (0..8).map(|b| (b * Q, on(60))).collect();
                rig.sequence(0, &beats);
            }
            rig.send(EngineCommand::SetMetronome { on: metronome });
            rig.send(EngineCommand::SetClick {
                sound: ClickSound::Tick,
                gain: 1.0,
                recording_only: false,
            });
            rig.run(4);
            rig.send(if record {
                EngineCommand::Record {
                    from: 4 * Q,
                    count_in_beats: 4,
                    muted: TrackSet::default(),
                }
            } else {
                EngineCommand::Play { from: 0 }
            });
            rig.run(blocks)
        };
        let want: Vec<usize> = (0..8)
            .map(|b| (frame_of(b * Q) + u64::from(lag)) as usize)
            .collect();
        let song: Vec<usize> = clicks(&render(false, false)).iter().map(|c| c.0).collect();
        assert_eq!(song, want, "lag {lag}: the song");
        assert_eq!(onsets(&render(true, false)), want, "lag {lag}: playing");
        assert_eq!(onsets(&render(true, true)), want, "lag {lag}: counting in");
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
}

/// The click of the latency measurement goes out after the master's plugins and is timed on
/// the line of the frames: the master's delay changes none of its numbers.
#[test]
fn the_latency_measurement_does_not_hear_the_masters_delay() {
    let measure = |lag: u32| {
        let mut rig = Rig::new();
        rig.master_lag(lag);
        rig.run(10);
        rig.send(EngineCommand::Probe);
        let out = rig.run(20);
        let sent = rig.parts.handle.pop_probe().expect("the click went out");
        let clock = rig.parts.handle.clock().clone();
        let left_us = clock.time_of_frame(sent.frame as f64);
        let frame = clock.frame_at(dac_us(sent.frame + 100));
        (out, sent, left_us, frame)
    };
    let plain = measure(0);
    let head = 10 * BLOCK as u64;
    assert_eq!(plain.1.frame, head);
    assert_eq!(
        onsets(&plain.0),
        [0],
        "on the first frame after the command"
    );
    let left_us = plain.2.expect("the clock knows the frame");
    assert!((left_us - (1_000_000 + head * 1_000_000 / RATE) as f64).abs() < 1.0);
    let frame = plain.3.expect("the clock knows the moment");
    assert!((frame - (head + 100) as f64).abs() < 0.1, "{frame}");
    for lag in LAGS {
        assert!(measure(lag) == plain, "lag {lag}");
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
}

/// The song of a frame comes out of the master as many frames later as its plugins delayed
/// it then: the moment it is heard is when that later frame reaches the DAC.
#[test]
fn the_song_of_a_frame_is_heard_when_it_comes_out_of_the_master() {
    for lag in LAGS {
        let mut rig = Rig::new();
        let says = rig.master_lag(lag);
        rig.run(100);
        let later = lag / 2 + 7;
        says.store(later, Relaxed);
        rig.run(100);
        let clock = rig.parts.handle.clock().clone();
        for (frame, ear) in [(3_000.0, lag), (3_000.5, lag), (30_000.0, later)] {
            let heard = clock.heard_time_of_frame(frame);
            let out = clock.time_of_frame(frame + f64::from(ear));
            assert_eq!(heard, out, "lag {lag}, frame {frame}");
        }
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
}

/// A plugin on the master that says another delay while the song plays: when it delays more,
/// the position heard stands until the song comes out again; when less, it jumps on. It never
/// goes back, and the clicks waiting move with the song, or are not heard where the song of
/// their beat is not.
#[test]
fn a_change_of_the_masters_delay_never_moves_the_position_heard_back() {
    let changes = [2_000u32, 500, 0, 30_000, 30_100, 1_000, 26_000, 0];
    // Where the delay changes, counted from the start of the play (the 4 blocks before it).
    let at: Vec<u64> = (0..changes.len() as u64)
        .map(|i| (40 + 150 * i) * BLOCK as u64)
        .collect();
    let start = 4 * BLOCK as u64;
    let pos = |frame: u64| ticks(frame as f64);
    let mut checks = Vec::new();
    // 0 → 2000: the song that came out last stands for 2000 frames, then goes on.
    let grew = dac_us(start + at[0]);
    for into in [5.0, lag_us(1_000), lag_us(2_000) - 5.0] {
        checks.push((grew + into, pos(at[0]), true));
    }
    checks.push((
        grew + lag_us(2_000) + 5.0,
        pos(at[0]) + 5.0 * TICKS_PER_S as f64 / 1e6,
        false,
    ));
    // 2000 → 500: 1500 frames of the song are not heard.
    let shrank = dac_us(start + at[1]) + lag_us(500);
    checks.push((
        shrank - 1.0,
        pos(at[1] - 1_500) - TICKS_PER_S as f64 / 1e6,
        false,
    ));
    checks.push((shrank + 1.0, pos(at[1]) + TICKS_PER_S as f64 / 1e6, false));
    let mut checks = checks.into_iter().peekable();

    let run = |metronome: bool| {
        let mut rig = Rig::new();
        let says = rig.master_lag(0);
        rig.send(EngineCommand::SetMetronome { on: metronome });
        rig.run(4);
        rig.send(EngineCommand::Play { from: 0 });
        let mut out = rig.run(40);
        for lag in changes {
            says.store(lag, Relaxed);
            out.extend(rig.run(150));
        }
        out
    };
    let mut rig = Rig::new();
    let says = rig.master_lag(0);
    rig.run(4);
    rig.send(EngineCommand::Play { from: 0 });
    let mut t = dac_us(start) + 1.0;
    let mut last = None::<HeardPosition>;
    for block in 0..40 + 150 * changes.len() {
        if let Some(i) = at.iter().position(|&a| a == (block * BLOCK) as u64) {
            says.store(changes[i], Relaxed);
        }
        rig.run(1);
        let end = dac_us(rig.frames);
        while let Some((when, tick, held)) = checks.next_if(|c| c.0 < end) {
            let h = rig.heard(when);
            assert!((h.tick - tick).abs() < 0.01, "at {when}: {} {tick}", h.tick);
            assert_eq!(h.ticks_per_us == 0.0, held, "at {when}");
        }
        while t < end {
            let h = rig.heard(t);
            if let Some(last) = last {
                assert!(
                    h.tick >= last.tick,
                    "back at {t}: {} after {}",
                    h.tick,
                    last.tick
                );
            }
            last = Some(h);
            t += 23.0;
        }
    }
    assert!(checks.next().is_none(), "every moment was read");

    // Each beat's click comes out on the first frame its song does, and none where the song
    // of its beat does not.
    let out = run(true);
    let delay = |frame: u64| match at.iter().rposition(|&a| a <= frame) {
        Some(i) => u64::from(changes[i]),
        None => 0,
    };
    let mut want = Vec::new();
    for b in 0.. {
        let beat = frame_of(b * Q);
        if beat >= out.len() as u64 {
            break;
        }
        let first = (beat..out.len() as u64).find(|&n| n.checked_sub(delay(n)) == Some(beat));
        want.extend(first.map(|n| n as usize));
    }
    assert!(want.len() > 6, "{want:?}");
    assert_eq!(onsets(&out), want);
    assert_eq!(common::audio_block_counts(), (0, 0));
}

/// A delay longer than the clock reaches back (512 stretches: 2.7 s at 256 frames a block):
/// in a steady play the position heard is counted back from the oldest stretch kept, so the
/// keys of a recording still land on the beats that came out.
#[test]
fn keys_land_on_the_beat_also_behind_a_delay_longer_than_the_clock_keeps() {
    const LAG: u32 = 140_000;
    assert!(u64::from(LAG) > 512 * BLOCK as u64);
    let mut rig = Rig::new();
    rig.master_lag(LAG);
    rig.run(4);
    let start = rig.frames;
    rig.send(EngineCommand::Record {
        from: 0,
        count_in_beats: 0,
        muted: TrackSet::default(),
    });
    for b in 1..=8 {
        let t = dac_us(start + frame_of(b * Q)) + lag_us(LAG);
        rig.key(t, on(60));
    }
    rig.run((frame_of(8 * Q) + u64::from(LAG)) as usize / BLOCK + 2);
    let landed: Vec<f64> = (rig.recorded().iter())
        .filter_map(|r| match *r {
            Recorded::Event {
                event: InstrumentEvent::NoteOn { .. },
                tick,
                ..
            } => Some(tick),
            _ => None,
        })
        .collect();
    assert_eq!(landed.len(), 8, "{landed:?}");
    for (b, tick) in (1..=8).zip(landed) {
        let off = tick - (b * Q) as f64;
        assert!(off.abs() < 0.01, "beat {b} lands {off} off");
    }
    let now = rig.heard(dac_us(rig.frames - 1));
    let want = ticks((rig.frames - 1 - start) as f64 - f64::from(LAG));
    assert!((now.tick - want).abs() < 0.01, "{} {want}", now.tick);
    assert_eq!(common::audio_block_counts(), (0, 0));
}
