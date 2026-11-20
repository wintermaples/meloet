//! The vocal clips of a track, without a device and without a disk: a feeder in the test
//! plays the read-ahead thread, and every sample it puts into the ring says which frame of
//! the song it is. So the output tells where each frame sounded and what it was multiplied
//! by (docs/architecture.md 22-8, 22-18 A1).

#![allow(clippy::needless_range_loop, reason = "a frame is told by its number")]

mod common;

use std::sync::Arc;

use meloet_engine::{
    BlockTiming, ClipRegion, ClipTable, Effect, EffectContext, Engine, EngineCommand, EngineConfig,
    EngineParts, MixStrip, StreamChunk, StreamLayout, StreamWant, StreamWriter, TrackSet,
    monotonic_us, song_frame, stream_ring,
};
use meloet_model::{TempoMap, fade_gain};
use rtrb::Consumer;

const RATE: u32 = 48_000;
const BLOCK: usize = 256;
const TRACK: usize = 3;
/// 10 ms and 20 ms.
const FADE: usize = 480;
const RAMP: usize = 960;

/// What the feeder puts on the left for a frame of the song; the right is its negative.
fn sample(song: u64) -> f32 {
    0.2 + (song % 1_000) as f32 / 5_000.0
}

struct Feeder {
    writer: StreamWriter,
    wants: Consumer<StreamWant>,
    map: Arc<TempoMap>,
    /// The generation it feeds, how it lies on the song, and the next frame to put in.
    now: Option<(u32, StreamLayout, u64)>,
    /// Every want that made a generation, and every one that only said where the engine is.
    made: Vec<StreamWant>,
    hints: Vec<StreamWant>,
    /// Off: it reads the wants and puts nothing in (a disk that stands still).
    on: bool,
}

impl Feeder {
    fn run(&mut self) {
        let mut fresh = false;
        while let Ok(want) = self.wants.pop() {
            assert_eq!(want.track, TRACK as u32);
            match &mut self.now {
                Some((generation, _, next)) if *generation == want.generation => {
                    *next = (*next).max(want.frame);
                    self.hints.push(want);
                }
                _ => {
                    let layout = StreamLayout::new(&want, &self.map, f64::from(RATE));
                    self.now = Some((want.generation, layout, want.frame));
                    fresh = true;
                    self.made.push(want);
                }
            }
        }
        let Some((generation, layout, next)) = &mut self.now else {
            return;
        };
        // A new generation goes in at once, behind what is left of the old one.
        while self.on && (fresh || self.writer.waiting_frames() < RATE as usize) {
            let (song, run) = layout.at(*next);
            let frames = run.min(1_024).min(self.writer.room_frames() as u64) as u32;
            if frames == 0 {
                break;
            }
            let data: Vec<f32> = (song..song + u64::from(frames))
                .flat_map(|s| [sample(s), -sample(s)])
                .collect();
            let chunk = StreamChunk {
                generation: *generation,
                first_frame: *next,
                frames,
            };
            assert!(self.writer.push(chunk, &data));
            *next += u64::from(frames);
        }
    }
}

struct Rig {
    parts: EngineParts,
    feeder: Feeder,
    out: Vec<f32>,
    frames: u64,
}

impl Rig {
    fn new(map: Arc<TempoMap>, regions: Vec<ClipRegion>) -> Rig {
        let mut parts = Engine::new(EngineConfig::default());
        assert!(parts.handle.set_tempo_map(map.clone()).is_ok());
        let (writer, reader) = stream_ring(RATE);
        assert!(parts.handle.set_stream(TRACK, Some(reader)).is_ok());
        let wants = parts.handle.take_stream_wants().unwrap();
        assert!(parts.handle.take_stream_wants().is_none());
        let mut rig = Rig {
            parts,
            feeder: Feeder {
                writer,
                wants,
                map,
                now: None,
                made: Vec::new(),
                hints: Vec::new(),
                on: true,
            },
            out: vec![0.0; BLOCK * 2],
            frames: 0,
        };
        rig.clips(regions, false);
        // A block while stopped: the engine says where it is, and the ring fills.
        rig.blocks(1);
        rig
    }

    fn clips(&mut self, regions: Vec<ClipRegion>, refeed: bool) {
        let table = ClipTable::new(regions).unwrap();
        assert!(
            (self.parts.handle)
                .set_clips(TRACK, Some(table), refeed)
                .is_ok()
        );
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok());
    }

    /// The left side of `count` blocks; the right must be its negative.
    fn blocks(&mut self, count: usize) -> Vec<f32> {
        let mut left = Vec::with_capacity(count * BLOCK);
        for _ in 0..count {
            self.feeder.run();
            let t_us = 1_000_000 + self.frames * 1_000_000 / u64::from(RATE);
            let timing = BlockTiming {
                callback_us: t_us,
                playback_us: t_us + 5_000,
            };
            self.parts.engine.process(&mut self.out, timing);
            self.frames += BLOCK as u64;
            for frame in self.out.as_chunks::<2>().0 {
                assert_eq!(frame[1], -frame[0]);
                left.push(frame[0]);
            }
            self.parts.handle.drop_retired();
        }
        left
    }

    fn missing(&self) -> u64 {
        self.parts.handle.meters().stream_missing()
    }
}

fn region(start: u64, end: u64, gain: f32) -> ClipRegion {
    ClipRegion {
        start,
        end,
        fade_in: 0,
        fade_out: 0,
        gain,
    }
}

fn no_memory_was_taken() {
    assert_eq!(common::audio_block_counts(), (0, 0));
}

#[test]
fn clips_sound_at_their_frames_and_the_volume_changes_at_once_where_they_meet() {
    let map = common::flat_map(12_000, 4, 4);
    let regions = vec![
        region(1_000, 5_000, 1.0),
        region(5_000, 9_000, 0.5),
        region(12_000, 13_000, 2.0),
    ];
    let mut rig = Rig::new(map, regions.clone());
    assert!(rig.blocks(4).iter().all(|&s| s == 0.0));
    rig.send(EngineCommand::Play { from: 0 });
    let out = rig.blocks(60);
    let table = ClipTable::new(regions).unwrap();
    for (n, &got) in out.iter().enumerate() {
        let n = n as u64;
        assert_eq!(got, sample(n) * table.gain_at(n), "frame {n}");
    }
    assert_eq!(out[999], 0.0);
    assert_eq!(out[1_000], sample(1_000));
    assert_eq!(out[4_999], sample(4_999));
    assert_eq!(out[5_000], sample(5_000) * 0.5);
    assert_eq!(out[9_000], 0.0);
    assert_eq!(out[12_000], sample(12_000) * 2.0);
    // The ring was filled while stopped, for the place the play started from.
    assert_eq!(rig.feeder.made.len(), 1);
    assert_eq!(rig.feeder.made[0].frame, 0);
    assert_eq!(rig.feeder.made[0].loop_ticks, None);
    assert_eq!(rig.missing(), 0);
    no_memory_was_taken();
}

#[test]
fn the_fades_have_the_shape_of_the_model() {
    let map = common::flat_map(12_000, 4, 4);
    let clip = ClipRegion {
        start: 512,
        end: 30_512,
        fade_in: 4_800,
        fade_out: 9_600,
        gain: 0.8,
    };
    let mut rig = Rig::new(map, vec![clip]);
    rig.send(EngineCommand::Play { from: 0 });
    let out = rig.blocks(130);
    for (n, &got) in out.iter().enumerate() {
        assert_eq!(got, sample(n as u64) * clip.gain_at(n as u64), "frame {n}");
    }
    let at = |n: usize| out[n] / sample(n as u64);
    assert_eq!(at(512), 0.0);
    assert!((at(512 + 2_400) - fade_gain(0.5) as f32 * 0.8).abs() < 1e-6);
    assert!((at(512 + 4_800) - 0.8).abs() < 1e-6);
    assert!((at(20_000) - 0.8).abs() < 1e-6);
    assert!((at(30_512 - 4_800) - fade_gain(0.5) as f32 * 0.8).abs() < 1e-6);
    assert!(at(30_511) > 0.0 && at(30_511) < 0.001);
    assert_eq!(at(30_512), 0.0);
    // Louder than a straight line all the way in.
    assert!((1..4_800).all(|k| at(512 + k) > 0.8 * k as f32 / 4_800.0));
    no_memory_was_taken();
}

#[test]
fn a_new_volume_for_the_clip_that_sounds_comes_over_twenty_ms_without_a_new_generation() {
    let map = common::flat_map(12_000, 4, 4);
    let mut rig = Rig::new(map, vec![region(0, 200_000, 1.0)]);
    rig.send(EngineCommand::Play { from: 0 });
    let before = rig.blocks(10);
    assert_eq!(before[2_559], sample(2_559));
    rig.clips(vec![region(0, 200_000, 0.5)], false);
    let out = rig.blocks(8);
    let at = |k: usize| out[k] / sample(2_560 + k as u64);
    for k in 0..RAMP {
        let want = 1.0 - 0.5 * k as f32 / RAMP as f32;
        assert!((at(k) - want).abs() < 1e-5, "{k}: {} {want}", at(k));
    }
    for k in RAMP..out.len() {
        assert_eq!(out[k], sample(2_560 + k as u64) * 0.5, "{k}");
    }
    assert_eq!(rig.feeder.made.len(), 1);
    assert_eq!(rig.missing(), 0);
    no_memory_was_taken();
}

#[test]
fn an_empty_ring_is_silence_that_is_counted_and_the_place_stays() {
    let map = common::flat_map(12_000, 4, 4);
    let mut rig = Rig::new(map, vec![region(0, 2_000_000, 1.0)]);
    rig.send(EngineCommand::Play { from: 0 });
    let fed = rig.blocks(20);
    assert_eq!(fed[5_119], sample(5_119));
    // The disk stands still: what is in the ring plays out, then nothing.
    rig.feeder.on = false;
    let waiting = rig.feeder.writer.waiting_frames();
    let out = rig.blocks(waiting / BLOCK + 40);
    let whole = waiting / BLOCK * BLOCK;
    for (k, &got) in out[..whole].iter().enumerate() {
        assert_eq!(got, sample(5_120 + k as u64));
    }
    assert!(out[whole + BLOCK..].iter().all(|&s| s == 0.0));
    let silent = (out.len() - whole) as u64;
    assert!(rig.missing() > silent - 2 * BLOCK as u64 && rig.missing() <= silent);
    // The engine said where it is, so the thread can go on from there.
    assert!(!rig.feeder.hints.is_empty());
    assert_eq!(rig.feeder.made.len(), 1);

    rig.feeder.on = true;
    let from = 5_120 + out.len() as u64;
    let back = rig.blocks(20);
    let first = back.iter().position(|&s| s != 0.0).unwrap();
    assert!(first < 3 * BLOCK, "{first}");
    // In over 10 ms, and every frame is the one the song is at.
    for k in first..first + FADE - 1 {
        let level = back[k] / sample(from + k as u64);
        let want = (k - first + 1) as f32 / FADE as f32;
        assert!((level - want).abs() < 1e-4, "{k}: {level} {want}");
    }
    for k in first + FADE..back.len() {
        assert_eq!(back[k], sample(from + k as u64), "{k}");
    }
    no_memory_was_taken();
}

#[test]
fn a_jump_while_playing_fades_out_and_the_new_place_comes_in() {
    let map = common::flat_map(12_000, 4, 4);
    let mut rig = Rig::new(map.clone(), vec![region(0, 5_000_000, 1.0)]);
    rig.send(EngineCommand::Play { from: 0 });
    rig.blocks(10);
    // Two bars on: 192000 frames.
    rig.send(EngineCommand::Locate { tick: 15_360 });
    let out = rig.blocks(10);
    for k in 0..FADE {
        let want = sample(2_560 + k as u64) * (1.0 - (k + 1) as f32 / FADE as f32);
        assert!((out[k] - want).abs() < 1e-4, "{k}");
    }
    assert!(out[FADE - 1].abs() < 1e-4);
    // The new place, counted from the block the jump was taken in.
    let level = |k: usize| out[k] / sample(192_000 + k as u64);
    let first = (FADE + 1..out.len()).find(|&k| out[k] != 0.0).unwrap();
    assert!(first <= 2 * BLOCK);
    for k in first..first + FADE {
        assert!(level(k) > 0.0 && level(k) <= 1.0 && level(k) >= level(k - 1));
    }
    for k in first + FADE..out.len() {
        assert_eq!(out[k], sample(192_000 + k as u64), "{k}");
    }
    let made = &rig.feeder.made;
    assert_eq!(made.len(), 2);
    assert_eq!(made[1].generation, made[0].generation + 1);
    assert_eq!((made[1].anchor_sec, made[1].frame), (4.0, 0));

    // Stopped: every place the song is put at is told, so its two seconds are there.
    rig.send(EngineCommand::Stop {
        t_us: monotonic_us(),
    });
    rig.blocks(4);
    rig.send(EngineCommand::Locate { tick: 1_920 });
    let silent = rig.blocks(4);
    assert!(silent.iter().all(|&s| s == 0.0));
    let told = rig.feeder.made.len();
    assert_eq!(rig.feeder.made[told - 1].anchor_sec, 0.5);
    assert!(rig.feeder.writer.waiting_frames() >= RATE as usize);
    // Playing from there needs nothing new, and starts on the first frame.
    rig.send(EngineCommand::Play { from: 1_920 });
    let out = rig.blocks(4);
    for (k, &got) in out.iter().enumerate() {
        assert_eq!(got, sample(24_000 + k as u64), "{k}");
    }
    assert_eq!(rig.feeder.made.len(), told);
    assert_eq!(rig.missing(), 0);
    no_memory_was_taken();
}

#[test]
fn the_loop_turns_in_the_ring_and_the_table_follows() {
    // One bar at 120 is 96000 frames; the loop is its second half.
    let map = common::flat_map(12_000, 4, 4);
    let regions = vec![region(0, 60_000, 1.0), region(60_000, 96_000, 0.5)];
    let mut rig = Rig::new(map, regions.clone());
    rig.send(EngineCommand::SetLoop {
        start: 3_840,
        end: 7_680,
        on: true,
    });
    rig.blocks(1);
    rig.send(EngineCommand::Locate { tick: 1_920 });
    rig.blocks(2);
    rig.send(EngineCommand::Play { from: 1_920 });
    let out = rig.blocks(900);
    let table = ClipTable::new(regions).unwrap();
    for (n, &got) in out.iter().enumerate() {
        let n = n as u64;
        let song = if n < 72_000 {
            24_000 + n
        } else {
            48_000 + (n - 72_000) % 48_000
        };
        assert_eq!(got, sample(song) * table.gain_at(song), "frame {n}");
    }
    let made = &rig.feeder.made;
    assert_eq!(made[made.len() - 1].loop_ticks, Some((3_840, 7_680)));
    assert_eq!(made[made.len() - 1].anchor_sec, 0.5);
    let told = made.len();
    rig.blocks(400);
    assert_eq!(rig.feeder.made.len(), told);
    assert_eq!(rig.missing(), 0);
    no_memory_was_taken();
}

#[test]
fn a_loop_whose_laps_are_not_whole_frames_stays_with_the_table() {
    // 123.45 a minute: a lap of the loop is not a whole number of frames, and a part of a
    // frame is let go at every turn (21-3). The table of the engine and the layout of the
    // thread must still mean the same frame of the song, lap after lap.
    let map = common::flat_map(12_345, 4, 4);
    let rate = f64::from(RATE);
    let frame = |tick: u64| song_frame(&map, tick as f64, rate);
    // A clip for every quarter note, each with its own volume.
    let regions: Vec<ClipRegion> = (0..16u64)
        .map(|q| {
            region(
                frame(q * 1_920),
                frame((q + 1) * 1_920),
                0.25 + q as f32 / 16.0,
            )
        })
        .collect();
    let mut rig = Rig::new(map.clone(), regions.clone());
    rig.send(EngineCommand::SetLoop {
        start: 1_920 + 7,
        end: 5 * 1_920 + 11,
        on: true,
    });
    rig.blocks(1);
    rig.send(EngineCommand::Locate { tick: 963 });
    rig.blocks(2);
    rig.send(EngineCommand::Play { from: 963 });
    let out = rig.blocks(2_000);
    let want = rig.feeder.made[rig.feeder.made.len() - 1];
    let layout = StreamLayout::new(&want, &map, rate);
    let table = ClipTable::new(regions).unwrap();
    for (n, &got) in out.iter().enumerate() {
        let (song, _) = layout.at(n as u64);
        assert_eq!(got, sample(song) * table.gain_at(song), "frame {n}");
    }
    assert!(layout.at(out.len() as u64 - 1).0 < frame(5 * 1_920 + 11));
    assert_eq!(rig.missing(), 0);
    no_memory_was_taken();
}

/// An effect that says it delays the sound, and does.
struct Delay {
    line: Vec<[f32; 2]>,
    at: usize,
}

impl Effect for Delay {
    fn prepare(&mut self, _: f32, _: usize) {}
    fn set_param(&mut self, _: u32, _: f32) {}
    fn process(&mut self, left: &mut [f32], right: &mut [f32], _: &EffectContext) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let old = std::mem::replace(&mut self.line[self.at], [*l, *r]);
            (*l, *r) = (old[0], old[1]);
            self.at = (self.at + 1) % self.line.len();
        }
    }
    fn reset(&mut self) {}
    fn latency(&self) -> u32 {
        self.line.len() as u32
    }
}

#[test]
fn a_track_whose_plugins_delay_the_sound_reads_its_clips_ahead() {
    let map = common::flat_map(12_000, 4, 4);
    let mut rig = Rig::new(map, vec![region(0, 5_000_000, 1.0)]);
    let delay = Delay {
        line: vec![[0.0; 2]; 300],
        at: 0,
    };
    assert!(
        (rig.parts.handle)
            .set_effect(MixStrip::Track(TRACK), 0, Some(Box::new(delay)))
            .is_ok()
    );
    rig.blocks(4);
    // Told again, 300 frames ahead.
    let want = rig.feeder.made[rig.feeder.made.len() - 1];
    assert_eq!(want.frame, 300);
    rig.send(EngineCommand::SetLoop {
        start: 0,
        end: 1_920,
        on: true,
    });
    rig.blocks(1);
    rig.send(EngineCommand::Play { from: 0 });
    let out = rig.blocks(300);
    // What comes out is in time with the song, from where the delay has run through; the
    // loop's turn (24000 frames) too.
    for (n, &got) in out.iter().enumerate().skip(300 + FADE) {
        assert_eq!(got, sample(n as u64 % 24_000), "frame {n}");
    }
    assert_eq!(rig.missing(), 0);
}

#[test]
fn a_muted_track_goes_on_reading_and_comes_back_in_time() {
    let map = common::flat_map(12_000, 4, 4);
    let mut rig = Rig::new(map, vec![region(0, 5_000_000, 1.0)]);
    rig.send(EngineCommand::Play { from: 0 });
    rig.blocks(4);
    rig.send(EngineCommand::SetSilentTracks {
        tracks: [TRACK].into_iter().collect::<TrackSet>(),
    });
    let out = rig.blocks(4);
    assert!(out[0] > 0.0 && out[FADE - 2] > 0.0);
    assert!(out[FADE..].iter().all(|&s| s == 0.0));
    rig.send(EngineCommand::SetSilentTracks {
        tracks: TrackSet::default(),
    });
    let out = rig.blocks(4);
    for (k, &got) in out.iter().enumerate().skip(FADE) {
        assert_eq!(got, sample(2_048 + k as u64));
    }
    assert_eq!(rig.feeder.made.len(), 1);
    no_memory_was_taken();
}

#[test]
fn audio_and_notes_of_one_tick_are_at_most_a_frame_apart() {
    // Audio is placed on the song's own frames (`song_frame`, the nearest); notes and
    // clicks on frames counted from where the play started (`frame_of`, the first at or
    // past). With a tempo whose beats are not on whole frames the two can differ by one.
    let map = common::flat_map(12_345, 4, 4);
    let rate = f64::from(RATE);
    let clip_tick = 4 * 1_920u64;
    let start = song_frame(&map, clip_tick as f64, rate);
    let (mut apart, mut same) = (0, 0);
    for from in [0u64, 1, 500, 963, 1_920, 2_001, 3_333, 5_000, 7_001] {
        let mut rig = Rig::new(map.clone(), vec![region(start, start + 48_000, 1.0)]);
        rig.send(EngineCommand::Locate { tick: from });
        rig.blocks(2);
        rig.send(EngineCommand::Play { from });
        let out = rig.blocks(400);
        let audio = out.iter().position(|&s| s != 0.0).unwrap() as i64;
        assert_eq!(out[audio as usize], sample(start));
        let note = map.frame_of(clip_tick as f64, map.seconds_at(from as f64), rate) as i64;
        assert!((audio - note).abs() <= 1, "from {from}: {audio} {note}");
        if audio == note {
            same += 1;
        } else {
            apart += 1;
        }
    }
    assert!(same > 0 && apart > 0, "{same} {apart}");
}
