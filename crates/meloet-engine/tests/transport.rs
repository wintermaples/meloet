//! Transport without a device: sequences, loop, metronome, count-in, stop, and the heard
//! position given to recorded keys.

mod common;

use meloet_engine::{
    BlockTiming, Engine, EngineCommand, EngineConfig, EngineParts, HeardPosition, Instrument,
    InstrumentEvent, MidiIn, PlaceholderSynth, Recorded, Sequence, SequenceEvent, TrackSet,
    TransportState,
};
use rtrb::{Consumer, Producer, RingBuffer};

const RATE: u64 = 48_000;
const Q: u64 = meloet_model::TICKS_PER_QUARTER;
/// A bar of 4/4.
const BAR: u64 = 4 * Q;
/// Ticks × this = the engine's exact position unit.
const SCALE: i128 = 6000 * RATE as i128;

/// Writes down every event with the frame (counted from the first render) it took effect on.
struct Tape {
    tx: Producer<(u64, InstrumentEvent)>,
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
    fn new(block: usize, delay_us: u64) -> Self {
        Rig {
            parts: Engine::new(EngineConfig::default()),
            block,
            frames: 0,
            base_us: 1_000_000,
            delay_us,
            keys: Vec::new(),
            out: vec![0.0; block * 2],
        }
    }

    fn tape(&mut self, track: usize) -> Consumer<(u64, InstrumentEvent)> {
        let (tx, rx) = RingBuffer::new(1 << 16);
        let tape = Tape { tx, frames: 0 };
        assert!(
            self.parts
                .handle
                .swap_instrument(track, Box::new(tape))
                .is_ok()
        );
        rx
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    fn tempo(&mut self, bpm_centi: u32) {
        let map = common::flat_map(bpm_centi, 4, 4);
        assert!(self.parts.handle.set_tempo_map(map).is_ok());
    }

    fn sequence(&mut self, track: usize, events: &[(u64, InstrumentEvent)]) {
        let events = events
            .iter()
            .map(|&(tick, event)| SequenceEvent { tick, event })
            .collect();
        assert!(
            self.parts
                .handle
                .swap_sequence(track, Sequence::new(events))
                .is_ok()
        );
    }

    /// When frame `n` (counted from the first render) reaches the DAC.
    fn dac_us(&self, n: u64) -> f64 {
        self.base_us as f64 + self.delay_us as f64 + n as f64 * 1e6 / RATE as f64
    }

    fn key(&mut self, t_us: u64, event: InstrumentEvent) {
        self.keys.push(MidiIn { event, t_us });
        self.keys.sort_by_key(|k| k.t_us);
    }

    /// Renders whole device blocks until at least `frames` more frames are done. Returns the
    /// left channel.
    fn run(&mut self, frames: u64) -> Vec<f32> {
        let mut left = Vec::new();
        let end = self.frames + frames;
        while self.frames < end {
            let callback_us = self.base_us + (self.frames as f64 * 1e6 / RATE as f64) as u64;
            while self.keys.first().is_some_and(|k| k.t_us <= callback_us) {
                let key = self.keys.remove(0);
                self.parts.midi_in.push(key).expect("midi ring full");
            }
            let timing = BlockTiming {
                callback_us,
                playback_us: callback_us + self.delay_us,
            };
            self.parts.engine.process(&mut self.out, timing);
            left.extend(self.out.as_chunks::<2>().0.iter().map(|f| f[0]));
            self.frames += self.block as u64;
        }
        left
    }

    /// Without the `Rolling` items: [`Rig::recorded_all`] has them.
    fn recorded(&mut self) -> Vec<Recorded> {
        let mut all = self.recorded_all();
        all.retain(|r| !matches!(r, Recorded::Rolling { .. }));
        all
    }

    fn recorded_all(&mut self) -> Vec<Recorded> {
        std::iter::from_fn(|| self.parts.handle.pop_recorded()).collect()
    }
}

fn drain(tape: &mut Consumer<(u64, InstrumentEvent)>) -> Vec<(u64, InstrumentEvent)> {
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

fn cc(controller: u8, value: u8) -> InstrumentEvent {
    InstrumentEvent::Control { controller, value }
}

fn bend(value: i16) -> InstrumentEvent {
    InstrumentEvent::PitchBend { value }
}

/// The first frame whose position is at or past `tick`, for a run that starts at `from` on
/// frame 0 (exact: the position moves by bpm_centi × ticks_per_beat / SCALE ticks a frame).
fn frame_of(tick: u64, from: i128, bpm_centi: u32) -> u64 {
    let step = i128::from(bpm_centi) * Q as i128;
    let gap = (tick as i128 - from) * SCALE;
    if gap <= 0 {
        0
    } else {
        ((gap + step - 1) / step) as u64
    }
}

fn ticks_per_us(bpm_centi: u32) -> f64 {
    f64::from(bpm_centi) / 100.0 / 60.0 * Q as f64 / 1e6
}

/// Starts of clicks: a sound after at least 200 silent frames.
fn onsets(samples: &[f32]) -> Vec<(usize, f32)> {
    let mut quiet = usize::MAX;
    let mut found = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        if s.abs() > 1e-6 {
            if quiet >= 200 {
                found.push((i, s.abs()));
            }
            quiet = 0;
        } else {
            quiet = quiet.saturating_add(1);
        }
    }
    found
}

const BLOCKS: [usize; 7] = [32, 64, 128, 256, 512, 1024, 2048];
const TEMPOS: [u32; 4] = [3_000, 9_600, 13_750, 30_000];

#[test]
fn sequence_events_land_on_their_frame() {
    let end = 20 * Q + 400;
    let ticks = [
        0u64,
        1,
        7,
        Q / 2,
        Q - 1,
        Q,
        2 * Q + 40,
        BAR - 1,
        BAR,
        10 * Q + 1,
        end - 1,
    ];
    for block in BLOCKS {
        for bpm in TEMPOS {
            for from in [0u64, 100] {
                let mut rig = Rig::new(block, 0);
                let mut tape = rig.tape(0);
                rig.tempo(bpm);
                let events: Vec<_> = ticks
                    .iter()
                    .enumerate()
                    .map(|(i, &t)| (t, on(40 + i as u8)))
                    .collect();
                rig.sequence(0, &events);
                rig.run(block as u64);
                assert!(drain(&mut tape).is_empty());
                let start = rig.frames;
                rig.send(EngineCommand::Play { from });
                rig.run(frame_of(end, from as i128, bpm) + 1);
                let got = drain(&mut tape);
                let want: Vec<_> = events
                    .iter()
                    .filter(|(t, _)| *t >= from)
                    .map(|&(t, e)| (start + frame_of(t, from as i128, bpm), e))
                    .collect();
                assert_eq!(got, want, "block {block}, tempo {bpm}, from {from}");
                for (frame, ev) in &got {
                    let InstrumentEvent::NoteOn { note, .. } = ev else {
                        unreachable!()
                    };
                    let tick = ticks[usize::from(note - 40)];
                    let seconds = (tick - from) as f64 / (f64::from(bpm) / 100.0 / 60.0 * Q as f64);
                    let ideal = seconds * RATE as f64;
                    let at = (frame - start) as f64;
                    assert!(at >= ideal - 1e-6 && at < ideal + 1.0, "{at} vs {ideal}");
                }
            }
        }
    }
}

#[test]
fn same_tick_releases_then_controls_then_presses() {
    let mut rig = Rig::new(256, 0);
    let mut tape = rig.tape(0);
    rig.sequence(
        0,
        &[(0, on(60)), (Q, on(62)), (Q, cc(64, 127)), (Q, off(60))],
    );
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(48_000);
    let got: Vec<_> = drain(&mut tape).into_iter().map(|(_, e)| e).collect();
    assert_eq!(got, vec![on(60), off(60), cc(64, 127), on(62)]);
}

#[test]
fn recorded_keys_get_the_position_that_was_heard() {
    for block in BLOCKS {
        for delay_ms in [3u64, 13, 53, 120] {
            for bpm in [9_600u32, 13_750] {
                let mut rig = Rig::new(block, delay_ms * 1000);
                rig.tempo(bpm);
                rig.send(EngineCommand::SetMetronome { on: true });
                rig.run(block as u64);
                let start = rig.frames;
                // From bar 2 with a one-bar count-in: the run starts at tick 0 on `start`.
                rig.send(EngineCommand::Record {
                    from: BAR,
                    count_in_beats: 4,
                    muted: TrackSet::default(),
                });
                let dac = |rig: &Rig, tick: u64| rig.dac_us(start + frame_of(tick, 0, bpm));
                // On every recorded beat, 5 ms late on one, two in the count-in (one too early
                // to keep, one just inside the last half beat).
                let late_us = 5_000.0;
                let beats: Vec<u64> = (4..=8).map(|b| b * Q).collect();
                for &tick in &beats {
                    let t = dac(&rig, tick).round() as u64;
                    rig.key(t, on(60));
                }
                let t_late = (dac(&rig, 9 * Q) + late_us).round() as u64;
                rig.key(t_late, on(61));
                let t_early = dac(&rig, 3 * Q).round() as u64;
                rig.key(t_early, on(62));
                let inside = 7 * Q / 2 + 40;
                let t_inside = dac(&rig, inside).round() as u64;
                rig.key(t_inside, on(63));
                rig.run(frame_of(10 * Q, 0, bpm) + RATE / 5);

                let label = format!("block {block}, delay {delay_ms} ms, tempo {bpm}");
                let got = rig.recorded();
                assert!(
                    matches!(
                        got[0],
                        Recorded::Started {
                            from: BAR,
                            count_in_beats: 4,
                            ..
                        }
                    ),
                    "{label}"
                );
                let events: Vec<_> = got[1..]
                    .iter()
                    .map(|r| match *r {
                        Recorded::Event {
                            event: InstrumentEvent::NoteOn { note, .. },
                            tick,
                            lap,
                            ..
                        } => (note, tick, lap),
                        other => panic!("{label}: {other:?}"),
                    })
                    .collect();
                assert_eq!(events.len(), 7, "{label}: {events:?}");
                assert_eq!(events[0].0, 63, "{label}: the early key is dropped");
                assert!(
                    (events[0].1 - inside as f64).abs() <= 0.5,
                    "{label}: {events:?}"
                );
                for (i, tick) in beats.iter().enumerate() {
                    let (note, got_tick, lap) = events[i + 1];
                    assert_eq!((note, lap), (60, 0));
                    assert!(
                        (got_tick - *tick as f64).abs() <= 0.5,
                        "{label}: beat {tick} recorded at {got_tick}"
                    );
                }
                let (note, tick, _) = events[6];
                assert_eq!(note, 61);
                let want = (9 * Q) as f64 + late_us * ticks_per_us(bpm);
                assert!(
                    (tick - want).abs() <= 0.5,
                    "{label}: late key at {tick}, want {want}"
                );
            }
        }
    }
}

#[test]
fn clicks_reach_the_dac_on_the_beat() {
    // The click starts exactly on the frame of the beat, so a key stamped with that frame's DAC
    // time lands on the beat (checked above); here the sound itself is checked.
    for block in [32usize, 256, 2048] {
        for bpm in TEMPOS {
            let mut rig = Rig::new(block, 0);
            rig.tempo(bpm);
            rig.send(EngineCommand::SetMetronome { on: true });
            rig.send(EngineCommand::Play { from: 0 });
            let sound = rig.run(frame_of(7 * Q, 0, bpm) + 100);
            let got = onsets(&sound);
            let want: Vec<_> = (0..8)
                .map(|beat| frame_of(beat * Q, 0, bpm) as usize)
                .collect();
            let frames: Vec<_> = got.iter().map(|o| o.0).collect();
            assert_eq!(frames, want, "block {block}, tempo {bpm}");
            assert!(got[0].1 > got[1].1, "bar head is louder");
            assert!(got[4].1 > got[5].1, "bar head is louder");
            assert!(
                (0.1..0.2).contains(&got[0].1),
                "a little under the test tone: {}",
                got[0].1
            );
            let meters = rig.parts.handle.meters();
            assert_eq!(meters.take_track_peak(0), [0.0, 0.0], "not on a track");
            assert_eq!(
                meters.take_master_peak(),
                [0.0, 0.0],
                "not on the master's meter"
            );
        }
    }
}

#[test]
fn metronome_off_is_silent_while_playing() {
    let mut rig = Rig::new(256, 0);
    rig.send(EngineCommand::Play { from: 0 });
    assert!(onsets(&rig.run(RATE * 2)).is_empty());
}

/// The metronome is a switch and not a level that glides: turned off while the song stands
/// still, it is off at the first frame the song rolls again.
#[test]
fn a_metronome_turned_off_while_stopped_is_silent_from_the_first_frame() {
    for block in [64usize, 256, 1024] {
        let mut rig = Rig::new(block, 0);
        rig.send(EngineCommand::SetMetronome { on: true });
        rig.send(EngineCommand::Play { from: 0 });
        assert!(!onsets(&rig.run(RATE)).is_empty(), "it clicks while on");
        rig.send(EngineCommand::Stop { t_us: 0 });
        // What still sounds of the last click ends.
        rig.run(RATE / 4);
        rig.send(EngineCommand::SetMetronome { on: false });
        rig.run(RATE / 4);
        rig.send(EngineCommand::Play { from: 0 });
        let sound = rig.run(RATE);
        assert!(sound.iter().all(|s| *s == 0.0), "block {block}: play");
        rig.send(EngineCommand::Stop { t_us: 0 });
        rig.run(block as u64);
        // On, and off again in the block the recording starts in.
        rig.send(EngineCommand::SetMetronome { on: true });
        rig.run(block as u64);
        rig.send(EngineCommand::SetMetronome { on: false });
        rig.send(EngineCommand::Record {
            from: 0,
            count_in_beats: 0,
            muted: TrackSet::default(),
        });
        let sound = rig.run(RATE);
        assert!(sound.iter().all(|s| *s == 0.0), "block {block}: record");
        rig.recorded_all();
    }
    assert_eq!(
        common::audio_block_counts(),
        (0, 0),
        "(allocations, frees) inside audio blocks"
    );
}

#[test]
fn count_in_clicks_then_records_without_a_gap() {
    for metronome in [false, true] {
        for block in [64usize, 1024] {
            let bpm = 12_000;
            let mut rig = Rig::new(block, 0);
            let mut tape = rig.tape(0);
            rig.send(EngineCommand::SetMetronome { on: metronome });
            rig.sequence(0, &[(BAR - 1, on(50)), (BAR, on(60)), (5 * Q, on(62))]);
            rig.run(block as u64);
            let start = rig.frames as usize;
            rig.send(EngineCommand::Record {
                from: BAR,
                count_in_beats: 3,
                muted: TrackSet::default(),
            });
            let sound = rig.run(RATE * 2 + 1000);
            let from = (BAR - 3 * Q) as i128;
            let mut want: Vec<_> = (0..3)
                .map(|b| start + frame_of(from as u64 + b * Q, from, bpm) as usize)
                .collect();
            let switch = start + frame_of(BAR, from, bpm) as usize;
            if metronome {
                want.extend([switch, switch + 24_000]);
            }
            let got: Vec<_> = onsets(&sound).iter().map(|o| o.0 + start).collect();
            assert_eq!(got, want, "metronome {metronome}, block {block}");
            let notes = drain(&mut tape);
            assert_eq!(
                notes,
                vec![(switch as u64, on(60)), (switch as u64 + 24_000, on(62))],
                "the song starts on the switch frame; nothing plays during the count-in"
            );
            if block < 1024 {
                // 512 blocks of 64 frames do not reach back to the start of the count-in.
                continue;
            }
            let clock = &rig.parts.handle;
            let at = |n: usize| clock.heard_at(rig.dac_us(n as u64).ceil() as u64).unwrap();
            let before = at(switch - 1);
            assert_eq!(before.state, TransportState::CountIn);
            assert_eq!(before.count_in_beats_left, 1);
            assert_eq!(at(start + 1).count_in_beats_left, 3);
            let after = at(switch);
            assert_eq!(after.state, TransportState::Recording);
            assert!((after.tick - BAR as f64).abs() < 0.05, "{after:?}");
            assert_eq!(after.count_in_beats_left, 0);
        }
    }
}

#[test]
fn zero_count_in_records_at_once() {
    let mut rig = Rig::new(256, 10_000);
    rig.run(256);
    rig.send(EngineCommand::Record {
        from: 2 * Q,
        count_in_beats: 0,
        muted: TrackSet::default(),
    });
    let first = rig.frames;
    rig.run(256);
    let heard = rig
        .parts
        .handle
        .heard_at(rig.dac_us(first).round() as u64)
        .unwrap();
    assert_eq!(heard.state, TransportState::Recording);
    assert!((heard.tick - (2 * Q) as f64).abs() < 0.05);
    assert!(
        onsets(&rig.run(RATE)).is_empty(),
        "no count-in, metronome off"
    );
}

#[test]
fn loop_lets_go_resets_and_chases() {
    for block in [32usize, 1024] {
        let bpm = 13_750;
        let mut rig = Rig::new(block, 0);
        let mut tape = rig.tape(0);
        rig.tempo(bpm);
        rig.sequence(
            0,
            &[
                (0, cc(64, 127)),
                (0, cc(1, 64)),
                (0, bend(4000)),
                (100, on(60)),
                (3 * Q + 60, on(64)),
                (3 * Q + 260, off(64)),
                (15 * Q / 4, cc(1, 90)),
            ],
        );
        rig.send(EngineCommand::SetLoop {
            start: 2 * Q,
            end: BAR,
            on: true,
        });
        rig.run(block as u64);
        let start = rig.frames;
        rig.send(EngineCommand::Play { from: 0 });
        let wrap = start + frame_of(BAR, 0, bpm);
        let lap = frame_of(BAR, 2 * Q as i128, bpm);
        rig.run(wrap - start + 2 * lap + 10);
        let got = drain(&mut tape);
        let at_wrap: Vec<_> = got.iter().filter(|e| e.0 == wrap).map(|e| e.1).collect();
        assert_eq!(
            at_wrap,
            vec![
                off(60),
                cc(64, 0),
                cc(1, 0),
                bend(0),
                cc(64, 127),
                cc(1, 64),
                bend(4000)
            ],
            "block {block}"
        );
        let second_lap: Vec<_> = got
            .iter()
            .filter(|e| e.0 > wrap && e.0 < wrap + lap)
            .copied()
            .collect();
        assert_eq!(
            second_lap,
            vec![
                (wrap + frame_of(3 * Q + 60, 2 * Q as i128, bpm), on(64)),
                (wrap + frame_of(3 * Q + 260, 2 * Q as i128, bpm), off(64)),
                (wrap + frame_of(15 * Q / 4, 2 * Q as i128, bpm), cc(1, 90)),
            ]
        );
        let at_second_wrap: Vec<_> = got
            .iter()
            .filter(|e| e.0 == wrap + lap)
            .map(|e| e.1)
            .collect();
        assert_eq!(
            at_second_wrap,
            vec![
                cc(64, 0),
                cc(1, 0),
                bend(0),
                cc(64, 127),
                cc(1, 64),
                bend(4000)
            ],
            "the chase takes the last values before the loop start"
        );
    }
}

#[test]
fn keys_at_the_loop_seam_go_to_the_right_lap() {
    let bpm = 9_600;
    for block in [64usize, 2048] {
        for delay_ms in [13u64, 120] {
            let mut rig = Rig::new(block, delay_ms * 1000);
            rig.tempo(bpm);
            rig.send(EngineCommand::SetLoop {
                start: 2 * Q,
                end: BAR,
                on: true,
            });
            rig.run(block as u64);
            let start = rig.frames;
            rig.send(EngineCommand::Record {
                from: 2 * Q,
                count_in_beats: 0,
                muted: TrackSet::default(),
            });
            let lap = frame_of(BAR, 2 * Q as i128, bpm);
            let wrap = start + lap;
            let just_before = rig.dac_us(wrap - 1).round() as u64;
            let just_after = rig.dac_us(wrap).ceil() as u64;
            rig.key(just_before, on(70));
            rig.key(just_after, on(71));
            rig.key(rig.dac_us(wrap + lap).round() as u64 + 1, on(72));
            rig.run(3 * lap);
            let label = format!("block {block}, delay {delay_ms}");
            let got = rig.recorded();
            let events: Vec<_> = got
                .iter()
                .filter_map(|r| match *r {
                    Recorded::Event { tick, lap, .. } => Some((tick, lap)),
                    _ => None,
                })
                .collect();
            assert_eq!(events.len(), 3, "{label}: {got:?}");
            assert_eq!(events[0].1, 0, "{label}");
            assert!(
                events[0].0 > (BAR - 1) as f64 && events[0].0 < BAR as f64,
                "{label}: {events:?}"
            );
            assert_eq!(events[1].1, 1, "{label}");
            assert!(
                events[1].0 >= (2 * Q) as f64 && events[1].0 < (2 * Q + 1) as f64,
                "{label}: {events:?}"
            );
            assert_eq!(events[2].1, 2, "{label}");
            let laps: Vec<_> = got
                .iter()
                .filter_map(|r| match *r {
                    Recorded::LapChanged { lap, t_us, .. } => Some((lap, t_us)),
                    _ => None,
                })
                .collect();
            assert_eq!(laps.len(), 3, "{label}: {laps:?}");
            assert_eq!(laps[0].0, 1);
            assert!((laps[0].1 as f64 - rig.dac_us(wrap)).abs() <= 1.0);
            assert_eq!(laps[1].0, 2);
        }
    }
}

#[test]
fn play_after_the_loop_end_does_not_go_round() {
    let mut rig = Rig::new(256, 0);
    let mut tape = rig.tape(0);
    rig.sequence(0, &[(2 * Q, on(60)), (5 * Q + 100, on(61))]);
    rig.send(EngineCommand::SetLoop {
        start: 0,
        end: BAR,
        on: true,
    });
    rig.send(EngineCommand::Play { from: BAR + 80 });
    rig.run(RATE * 2);
    let got: Vec<_> = drain(&mut tape).into_iter().map(|e| e.1).collect();
    assert_eq!(got, vec![on(61)]);
}

#[test]
fn stop_with_the_pedal_down_leaves_nothing_sounding_but_the_keyboard() {
    let mut rig = Rig::new(256, 0);
    for track in [0, 1] {
        assert!(
            rig.parts
                .handle
                .swap_instrument(track, Box::new(PlaceholderSynth::new()))
                .is_ok()
        );
    }
    rig.send(EngineCommand::SetPlayTarget { track: 1 });
    rig.sequence(
        0,
        &[
            (0, cc(64, 127)),
            (0, on(60)),
            (0, on(64)),
            (Q / 2, off(60)),
            (Q / 2, bend(3000)),
        ],
    );
    rig.send(EngineCommand::Play { from: 0 });
    rig.key(0, cc(64, 127));
    rig.key(0, on(72));
    rig.key(1, off(72));
    rig.run(RATE);
    let meters = rig.parts.handle.meters().clone();
    meters.take_track_peak(0);
    rig.send(EngineCommand::Stop { t_us: 0 });
    rig.run(RATE / 2);
    meters.take_track_peak(0);
    meters.take_track_peak(1);
    rig.run(RATE / 10);
    assert_eq!(
        meters.take_track_peak(0),
        [0.0, 0.0],
        "the song's track is silent"
    );
    assert!(
        meters.take_track_peak(1)[0] > 0.01,
        "the key held by the person's own pedal keeps sounding"
    );
}

#[test]
fn stop_cuts_the_recording_at_its_time() {
    let mut rig = Rig::new(256, 13_000);
    rig.run(256);
    rig.send(EngineCommand::Record {
        from: 0,
        count_in_beats: 0,
        muted: TrackSet::default(),
    });
    rig.run(RATE);
    let stop_us = rig.base_us + 1_000_000;
    // Both keys reach the engine before the stop command does.
    rig.key(stop_us - 2_000, on(60));
    rig.key(stop_us + 2_000, on(61));
    rig.run(RATE / 10);
    rig.send(EngineCommand::Stop { t_us: stop_us });
    // These two reach the engine in the same block as the stop command.
    rig.key(stop_us - 1_000, on(62));
    rig.key(stop_us + 500, on(63));
    rig.run(RATE / 2);
    let got = rig.recorded();
    let stopped = got
        .iter()
        .find_map(|r| match *r {
            Recorded::Stopped {
                t_us, tick, lap, ..
            } => Some((t_us, tick, lap)),
            _ => None,
        })
        .expect("a stop mark");
    assert_eq!(stopped.0, stop_us);
    assert_eq!(stopped.2, 0);
    let want = (stop_us as f64 - rig.dac_us(256)) * ticks_per_us(12_000);
    assert!((stopped.1 - want).abs() < 0.5, "{} vs {want}", stopped.1);
    let kept: Vec<_> = got
        .iter()
        .filter_map(|r| match *r {
            Recorded::Event { event, t_us, .. } if t_us <= stop_us => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(kept, vec![on(60), on(62)]);
    // A key after the stop time that was stamped before the stop command came in is sent; the
    // receiver drops it by its time. One that comes in with the command is not sent.
    let late: Vec<_> = got
        .iter()
        .filter_map(|r| match *r {
            Recorded::Event { event, t_us, .. } if t_us > stop_us => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(late, vec![on(61)]);
    let after_stop = got
        .iter()
        .position(|r| matches!(r, Recorded::Stopped { .. }));
    assert_eq!(
        after_stop,
        Some(got.len() - 1),
        "nothing comes after the stop mark"
    );
    let heard = rig.parts.handle.heard_now();
    assert!(heard.is_none_or(|h| h.state == TransportState::Stopped));
}

#[test]
fn recording_while_playing_keeps_the_position_and_the_heard_ticks() {
    let bpm = 9_600u32;
    let tpu = ticks_per_us(bpm);
    for block in [64usize, 256, 1024] {
        for delay_ms in [13u64, 53] {
            let label = format!("block {block}, delay {delay_ms} ms");
            let mut rig = Rig::new(block, delay_ms * 1000);
            let mut armed = rig.tape(0);
            let mut other = rig.tape(1);
            rig.tempo(bpm);
            let every = |step: u64, note: u8| -> Vec<(u64, InstrumentEvent)> {
                (0..80).map(|i| (i * step, on(note))).collect()
            };
            rig.sequence(0, &every(Q / 2, 50));
            rig.sequence(1, &every(Q / 4, 60));
            rig.run(block as u64);
            let start = rig.frames;
            rig.send(EngineCommand::Play { from: 0 });
            rig.run(frame_of(BAR + 80, 0, bpm));

            // The command is taken at the next callback; the listener is then behind the
            // frames already rendered by the delay.
            let callback_us = rig.base_us as f64 + rig.frames as f64 * 1e6 / RATE as f64;
            let heard = (callback_us - rig.dac_us(start)) * tpu;
            let switch = rig.frames;
            rig.send(EngineCommand::RecordHere {
                muted: [0].into_iter().collect(),
            });
            // One key still in the stretch that was rendered while playing, one later.
            let at = |rig: &Rig, tick: f64| (rig.dac_us(start) + tick / tpu).round() as u64;
            let soon = heard.floor() + 5.0;
            let later = heard.floor() + (25 * Q / 4) as f64;
            let (t_soon, t_later) = (at(&rig, soon), at(&rig, later));
            rig.key(t_soon, on(70));
            rig.key(t_later, on(71));
            rig.run(frame_of(later as u64 + Q, 0, bpm) - (switch - start));

            let got = rig.recorded();
            let Recorded::Started {
                from,
                count_in_beats: 0,
                ..
            } = got[0]
            else {
                panic!("{label}: {got:?}");
            };
            assert_eq!(from, heard.floor() as u64, "{label}");
            let keys: Vec<_> = got[1..]
                .iter()
                .map(|r| match *r {
                    Recorded::Event {
                        event: InstrumentEvent::NoteOn { note, .. },
                        tick,
                        lap,
                        ..
                    } => (note, tick, lap),
                    other => panic!("{label}: {other:?}"),
                })
                .collect();
            assert_eq!(keys.len(), 2, "{label}: {keys:?}");
            for ((note, tick, lap), (want_note, want)) in
                keys.into_iter().zip([(70, soon), (71, later)])
            {
                assert_eq!((note, lap), (want_note, 0), "{label}");
                assert!(
                    (tick - want).abs() <= 0.5,
                    "{label}: {note} at {tick}, want {want}"
                );
            }

            // The other track goes on frame for frame: nothing jumped.
            let played = drain(&mut other);
            let want: Vec<_> = every(Q / 4, 60)
                .into_iter()
                .map(|(t, e)| (start + frame_of(t, 0, bpm), e))
                .filter(|(f, _)| *f < rig.frames)
                .collect();
            assert_eq!(played, want, "{label}");
            // The armed track presses nothing from the switch on.
            let muted = drain(&mut armed);
            assert!(
                muted
                    .iter()
                    .filter(|(_, e)| matches!(e, InstrumentEvent::NoteOn { note: 50, .. }))
                    .all(|(f, _)| *f < switch),
                "{label}: {muted:?}"
            );
            assert!(!muted.is_empty());
            let clock = &rig.parts.handle;
            let h = clock
                .heard_at(rig.dac_us(rig.frames - 1).round() as u64)
                .unwrap();
            assert_eq!(h.state, TransportState::Recording, "{label}");
            let want = (rig.dac_us(rig.frames - 1).round() - rig.dac_us(start)) * tpu;
            assert!(
                (h.tick - want).abs() < 0.05,
                "{label}: {} vs {want}",
                h.tick
            );
        }
    }
}

#[test]
fn recording_here_when_not_playing_ends_at_once() {
    let mut rig = Rig::new(256, 0);
    rig.send(EngineCommand::Locate { tick: 2 * Q });
    rig.send(EngineCommand::RecordHere {
        muted: TrackSet::default(),
    });
    rig.run(256);
    let got = rig.recorded();
    assert_eq!(
        got,
        [
            Recorded::Started {
                from: 2 * Q,
                count_in_beats: 0,
                frame: 0
            },
            Recorded::Stopped {
                t_us: rig.base_us,
                tick: (2 * Q) as f64,
                lap: 0,
                frame: 0
            }
        ]
    );
}

#[test]
fn recording_mutes_the_armed_tracks() {
    let mut rig = Rig::new(256, 0);
    let mut armed = rig.tape(0);
    let mut other = rig.tape(1);
    rig.sequence(0, &[(0, on(60))]);
    rig.sequence(1, &[(0, on(61))]);
    rig.send(EngineCommand::Record {
        from: 0,
        count_in_beats: 0,
        muted: [0].into_iter().collect(),
    });
    rig.run(4096);
    assert!(drain(&mut armed).is_empty());
    assert_eq!(drain(&mut other).len(), 1);
}

#[test]
fn silent_tracks_do_not_play_their_sequence_but_keys_still_sound() {
    let mut rig = Rig::new(256, 0);
    let meters = rig.parts.handle.meters().clone();
    for track in [0, 1] {
        assert!(
            rig.parts
                .handle
                .swap_instrument(track, Box::new(PlaceholderSynth::new()))
                .is_ok()
        );
    }
    let song: Vec<_> = (0..16u64)
        .flat_map(|i| [(i * Q, on(60)), (i * Q + 5 * Q / 6, off(60))])
        .collect();
    rig.sequence(0, &song);
    rig.sequence(1, &song);
    rig.send(EngineCommand::SetSilentTracks {
        tracks: [0].into_iter().collect(),
    });
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(RATE);
    assert_eq!(
        meters.take_track_peak(0),
        [0.0, 0.0],
        "silent in the render"
    );
    assert!(meters.take_track_peak(1)[0] > 0.01);
    // The keyboard plays track 0 (the play target) all the same.
    let now = rig.base_us + (rig.frames as f64 * 1e6 / RATE as f64) as u64;
    rig.key(now, on(72));
    rig.run(RATE / 4);
    assert!(meters.take_track_peak(0)[0] > 0.01, "a key played sounds");
    rig.key(now + 300_000, off(72));
    rig.run(RATE);
    meters.take_track_peak(0);
    rig.run(256);
    assert_eq!(meters.take_track_peak(0), [0.0, 0.0]);
}

#[test]
fn going_silent_lets_go_and_coming_back_picks_up() {
    let mut rig = Rig::new(256, 0);
    let mut tape = rig.tape(0);
    rig.sequence(
        0,
        &[
            (0, cc(64, 127)),
            (0, on(60)),
            (BAR + 80, on(62)),
            (20 * Q, off(60)),
        ],
    );
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(1024);
    drain(&mut tape);
    let at = rig.frames;
    rig.send(EngineCommand::SetSilentTracks {
        tracks: [0].into_iter().collect(),
    });
    // Past the silent track's key: it is not played.
    rig.run(frame_of(5 * Q, 0, 12_000) - at);
    assert_eq!(
        drain(&mut tape),
        vec![(at, off(60)), (at, cc(64, 0))],
        "let go at once"
    );
    let back = rig.frames;
    rig.send(EngineCommand::SetSilentTracks {
        tracks: TrackSet::default(),
    });
    rig.run(256);
    assert_eq!(
        drain(&mut tape),
        vec![(back, cc(64, 127))],
        "the pedal is chased where the song is"
    );
    // A stream that opens again starts with no silent track.
    rig.send(EngineCommand::SetSilentTracks {
        tracks: [0].into_iter().collect(),
    });
    rig.run(256);
    drain(&mut tape);
    rig.parts.engine.prepare(RATE as u32);
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(256);
    assert!(drain(&mut tape).contains(&(rig.frames - 256, on(60))));
}

#[test]
fn swapping_the_sequence_while_playing_lets_go_and_returns_the_old_one() {
    let mut rig = Rig::new(256, 0);
    let mut tape = rig.tape(0);
    rig.sequence(
        0,
        &[
            (0, cc(64, 127)),
            (0, on(60)),
            (0, bend(-100)),
            (20 * Q, off(60)),
        ],
    );
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(4096);
    drain(&mut tape);
    rig.sequence(0, &[(0, cc(64, 127)), (2 * BAR + 160, on(70))]);
    rig.run(256);
    let got: Vec<_> = drain(&mut tape).into_iter().map(|e| e.1).collect();
    assert_eq!(
        got,
        vec![off(60), cc(64, 0), bend(0), cc(64, 127)],
        "release and reset, then the new sequence is chased"
    );
    let old = rig
        .parts
        .handle
        .take_retired_sequence()
        .expect("the old sequence comes back");
    assert_eq!(old.events().len(), 4);
    // A track that does not exist hands the sequence straight back.
    rig.sequence(10_000, &[(0, on(1))]);
    rig.run(256);
    assert_eq!(rig.parts.handle.drop_retired(), 1);
}

#[test]
fn locate_while_playing_moves_and_chases() {
    let mut rig = Rig::new(256, 0);
    let mut tape = rig.tape(0);
    rig.sequence(0, &[(0, on(60)), (Q + 20, cc(1, 30)), (BAR + 80, on(62))]);
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(1024);
    drain(&mut tape);
    let at = rig.frames;
    rig.send(EngineCommand::Locate { tick: 3 * Q + 60 });
    rig.run(RATE);
    let got = drain(&mut tape);
    assert_eq!(
        got,
        vec![
            (at, off(60)),
            (at, cc(1, 30)),
            (
                at + frame_of(BAR + 80, (3 * Q + 60) as i128, 12_000),
                on(62)
            )
        ]
    );
}

#[test]
fn the_heard_position_follows_the_loop_and_the_count_in() {
    let bpm = 12_000;
    // 512 blocks of 512 frames: the clock still remembers the start of the count-in at the end.
    let mut rig = Rig::new(512, 53_000);
    rig.send(EngineCommand::SetLoop {
        start: BAR,
        end: 2 * BAR,
        on: true,
    });
    rig.run(512);
    let start = rig.frames;
    rig.send(EngineCommand::Record {
        from: BAR,
        count_in_beats: 4,
        muted: TrackSet::default(),
    });
    rig.run(RATE * 5);
    let clock = &rig.parts.handle;
    let heard = |n: u64| -> HeardPosition { clock.heard_at(rig.dac_us(n).ceil() as u64).unwrap() };
    let switch = start + frame_of(BAR, 0, bpm);
    let wrap = switch + frame_of(2 * BAR, BAR as i128, bpm);
    let rate = ticks_per_us(bpm);

    let h = heard(start);
    assert_eq!(
        (h.state, h.count_in_beats_left),
        (TransportState::CountIn, 4)
    );
    assert!(h.tick.abs() < 0.05);
    assert!((h.ticks_per_us - rate).abs() < 1e-12);
    let h = heard(switch - 1);
    assert_eq!(
        (h.state, h.count_in_beats_left),
        (TransportState::CountIn, 1)
    );
    let h = heard(switch);
    assert_eq!((h.state, h.lap), (TransportState::Recording, 0));
    assert!((h.tick - BAR as f64).abs() < 0.05);
    let h = heard(wrap - 1);
    assert_eq!(h.lap, 0);
    assert!(
        h.tick > (2 * BAR) as f64 - 0.1 && h.tick < (2 * BAR) as f64,
        "{h:?}"
    );
    let h = heard(wrap);
    assert_eq!(h.lap, 1, "{h:?}");
    assert!((h.tick - BAR as f64).abs() < 0.05, "{h:?}");
    let h = heard(start - 10);
    assert_eq!(h.state, TransportState::Stopped);
    assert_eq!(h.ticks_per_us, 0.0);
}

#[test]
fn another_thread_reads_the_position_while_the_engine_runs() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

    let mut rig = Rig::new(64, 0);
    rig.send(EngineCommand::Play { from: 0 });
    let clock = rig.parts.handle.clock().clone();
    let (map, map_gen) = rig.parts.handle.tempo_map();
    let map = map.clone();
    let done = Arc::new(AtomicBool::new(false));
    let reader = {
        let done = done.clone();
        let base = rig.base_us;
        std::thread::spawn(move || {
            let mut reads = 0u64;
            let mut t = base;
            while !done.load(Relaxed) {
                if let Some(h) = clock.heard_at(t, &map, map_gen) {
                    // Playing from 0 at 120 bpm: two quarter notes a second, from the base time on.
                    if h.state == TransportState::Playing {
                        let want = (t - base) as f64 * (2 * Q) as f64 / 1e6;
                        assert!((h.tick - want).abs() < 0.1, "torn read: {h:?} at {t}");
                        reads += 1;
                    }
                }
                t = base + (t - base + 777) % 2_000_000;
            }
            reads
        })
    };
    for _ in 0..200 {
        rig.run(RATE / 10);
    }
    done.store(true, Relaxed);
    assert!(reader.join().unwrap() > 0);
}

#[test]
fn every_transport_path_stays_off_the_allocator() {
    use meloet_model::{Meter, MeterChange, Tempo, TempoMap, TempoPoint, Timeline};
    use std::sync::Arc;

    let mut rig = Rig::new(256, 13_000);
    for track in [0, 1, 2] {
        assert!(
            rig.parts
                .handle
                .swap_instrument(track, Box::new(PlaceholderSynth::new()))
                .is_ok()
        );
    }
    let song: Vec<_> = (0..64u64)
        .flat_map(|i| {
            [
                (i * Q / 4, on(48 + (i % 24) as u8)),
                (i * Q / 4 + 90, off(48 + (i % 24) as u8)),
                (i * Q / 2, cc(64, if i % 2 == 0 { 127 } else { 0 })),
                (i * 200, bend((i as i16 % 9) * 900 - 4000)),
                (i * 300, cc(1, (i * 3 % 128) as u8)),
            ]
        })
        .collect();
    for track in [0, 1, 2] {
        rig.sequence(track, &song);
    }
    rig.send(EngineCommand::SetMetronome { on: true });
    rig.send(EngineCommand::SetLoop {
        start: Q,
        end: 5 * Q,
        on: true,
    });
    let mut out_blocks = [64usize, 333, 1024, 2048, 32].into_iter().cycle();
    for round in 0..40u64 {
        rig.block = out_blocks.next().unwrap();
        rig.out.resize(rig.block * 2, 0.0);
        match round % 8 {
            0 => rig.send(EngineCommand::Play { from: round * 10 }),
            1 => rig.send(EngineCommand::Record {
                from: 2 * Q,
                count_in_beats: (round % 5) as u32,
                muted: [1].into_iter().collect(),
            }),
            2 => rig.sequence(round as usize % 3, &song),
            3 => rig.send(EngineCommand::Locate { tick: 700 }),
            4 => {
                // Made here, off the audio thread; the engine only swaps it in.
                let timeline = Timeline {
                    tempo: Tempo::from_centi(7_000 + round as u32 * 500).unwrap(),
                    meter: Meter::new(3, 8).unwrap(),
                    head_ramp: round % 16 == 4,
                    tempo_points: vec![
                        TempoPoint {
                            tick: 600,
                            tempo: Tempo::from_centi(14_000).unwrap(),
                            ramp: true,
                        },
                        TempoPoint {
                            tick: 1500,
                            tempo: Tempo::from_centi(9_000).unwrap(),
                            ramp: false,
                        },
                    ],
                    meter_changes: vec![MeterChange {
                        bar: 2,
                        meter: Meter::FOUR_FOUR,
                    }],
                };
                let map = Arc::new(TempoMap::new(&timeline));
                assert!(rig.parts.handle.set_tempo_map(map).is_ok());
            }
            5 => rig.send(EngineCommand::Stop {
                t_us: rig.base_us + (rig.frames as f64 * 1e6 / RATE as f64) as u64,
            }),
            6 => rig.send(EngineCommand::SetLoop {
                start: 0,
                end: 2 * Q + round * 7,
                on: round % 3 != 0,
            }),
            _ if round % 16 == 15 => rig.send(EngineCommand::RecordHere {
                muted: [2].into_iter().collect(),
            }),
            7 if round % 32 == 7 => rig.send(EngineCommand::SetSilentTracks {
                tracks: [0, 2].into_iter().collect(),
            }),
            7 if round % 32 == 23 => rig.send(EngineCommand::SetSilentTracks {
                tracks: TrackSet::default(),
            }),
            _ => rig.send(EngineCommand::SetMetronome { on: round % 2 == 0 }),
        }
        let now = rig.base_us + (rig.frames as f64 * 1e6 / RATE as f64) as u64;
        for k in 0..20u64 {
            rig.key(now + k * 3_000, on(60 + k as u8));
            rig.key(now + k * 3_000 + 1_500, off(60 + k as u8));
        }
        rig.key(now, cc(64, 127));
        rig.run(RATE / 2);
        // Off the audio thread: empty what came back.
        rig.recorded();
        rig.parts.handle.drop_retired();
        rig.parts.handle.heard_now();
    }
    assert_eq!(rig.parts.handle.meters().record_overflows(), 0);
    assert_eq!(
        common::audio_block_counts(),
        (0, 0),
        "(allocations, frees) inside audio blocks"
    );
}

/// The frames a recording of the input is placed by (docs/architecture.md 22-6): where the
/// song starts rolling after the count-in, where each lap begins, and that the clock says
/// which frame reaches the DAC when.
#[test]
fn a_recording_says_the_stream_frames_it_rolls_and_goes_round_at() {
    for block in [64usize, 256, 1024] {
        let mut rig = Rig::new(block, 5_000);
        rig.tempo(12_000);
        // Something before, so that the stream's frames and the song's are not the same count.
        rig.run(10 * block as u64);
        let before = rig.frames;
        rig.send(EngineCommand::SetLoop {
            start: BAR,
            end: 2 * BAR,
            on: true,
        });
        rig.send(EngineCommand::Record {
            from: BAR,
            count_in_beats: 4,
            muted: TrackSet::default(),
        });
        // The count-in (2 s), the bar (2 s), and into the second lap.
        rig.run(5 * RATE);
        let stop_frame = rig.frames;
        let stop_us = rig.dac_us(stop_frame) as u64;
        rig.send(EngineCommand::Stop { t_us: stop_us });
        rig.run(block as u64);

        let got = rig.recorded_all();
        let label = format!("block {block}: {got:?}");
        assert_eq!(
            got[0],
            Recorded::Started {
                from: BAR,
                count_in_beats: 4,
                frame: before
            },
            "{label}"
        );
        // A beat at 120 is 24000 frames: the count-in ends 96000 frames on, whatever the block.
        assert_eq!(
            got[1],
            Recorded::Rolling {
                frame: before + 4 * 24_000,
                tick: BAR as f64
            },
            "{label}"
        );
        let Recorded::LapChanged { lap: 1, frame, .. } = got[2] else {
            panic!("{label}");
        };
        assert_eq!(frame, before + 8 * 24_000, "{label}");
        let Recorded::Stopped { frame, .. } = got[3] else {
            panic!("{label}");
        };
        assert_eq!(frame, stop_frame, "{label}");

        // The clock: frame n reaches the DAC at dac_us(n), and the other way round.
        let clock = rig.parts.handle.clock().clone();
        for n in [stop_frame - 3 * block as u64, stop_frame - 7] {
            let at = clock.frame_at(rig.dac_us(n)).expect("rendered");
            assert!((at - n as f64).abs() < 0.1, "{label}: {at} for {n}");
            let t_us = clock.time_of_frame(n as f64).expect("rendered");
            assert!((t_us - rig.dac_us(n)).abs() < 2.0, "{label}");
        }
    }
}

#[test]
fn recording_at_once_and_from_where_it_plays_roll_in_the_block_that_took_them() {
    let mut rig = Rig::new(256, 5_000);
    rig.tempo(12_000);
    rig.run(512);
    rig.send(EngineCommand::Record {
        from: 2 * Q,
        count_in_beats: 0,
        muted: TrackSet::default(),
    });
    rig.run(256);
    let got = rig.recorded_all();
    assert_eq!(
        got[..2],
        [
            Recorded::Started {
                from: 2 * Q,
                count_in_beats: 0,
                frame: 512
            },
            Recorded::Rolling {
                frame: 512,
                tick: (2 * Q) as f64
            }
        ]
    );

    let mut rig = Rig::new(256, 5_000);
    rig.tempo(12_000);
    rig.send(EngineCommand::Play { from: 0 });
    rig.run(48_000 + 128);
    rig.send(EngineCommand::RecordHere {
        muted: TrackSet::default(),
    });
    rig.run(256);
    let got = rig.recorded_all();
    let frames = 48_000 + 128;
    let (Recorded::Started { frame: started, .. }, Recorded::Rolling { frame, tick }) =
        (got[0], got[1])
    else {
        panic!("{got:?}");
    };
    assert_eq!((started, frame), (frames, frames));
    // The song's own position at that frame (ahead of what is heard): 2 beats a second.
    let expected = frames as f64 / 24_000.0 * Q as f64;
    assert!((tick - expected).abs() < 1e-6, "{tick} vs {expected}");
}

/// A cycle the stream missed: the frames go on counting, the time has jumped. The clock
/// answers for each side of the gap from that side's own blocks.
#[test]
fn the_clock_keeps_frames_and_times_apart_on_each_side_of_a_missed_cycle() {
    let mut rig = Rig::new(256, 5_000);
    rig.run(40 * 256);
    let before = rig.frames - 3 * 256;
    let before_us = rig.dac_us(before);
    // Three cycles never came.
    rig.base_us += 3 * 256 * 1_000_000 / RATE;
    rig.run(10 * 256);
    let after = rig.frames - 2 * 256 + 17;
    let clock = rig.parts.handle.clock().clone();

    let said = clock.time_of_frame(before as f64).expect("rendered");
    assert!((said - before_us).abs() < 2.0, "{said} for {before_us}");
    let said = clock.time_of_frame(after as f64).expect("rendered");
    assert!((said - rig.dac_us(after)).abs() < 2.0);
    let said = clock.frame_at(rig.dac_us(after)).expect("rendered");
    assert!((said - after as f64).abs() < 0.1, "{said} for {after}");
    let said = clock.frame_at(before_us).expect("rendered");
    assert!((said - before as f64).abs() < 0.1, "{said} for {before}");
}
