//! A note of the song sounds the same every time (docs/architecture.md 22-25): the samples
//! that take turns and the start of the waves come from the note's place in the song, not
//! from what was played before. Keys someone plays still take turns as they come.

#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;
mod support;

use meloet_engine::{
    BlockTiming, Engine, EngineCommand, EngineConfig, EngineParts, Instrument, InstrumentEvent,
    Sequence, SequenceEvent,
};
use meloet_model::TICKS_PER_QUARTER as Q;
use meloet_model::drums::DrumPiece;
use meloet_sound::synth::{ElectroKit, PolySynth};
use support::pack::{TestPack, level, region};

const RATE: usize = 48_000;
const BLOCK: usize = 256;
const BAR: u64 = 4 * Q;
/// Frames of a bar at the engine's first tempo, 120 quarters a minute.
const BAR_FRAMES: usize = 2 * RATE;
const SIXTEENTH: u64 = Q / 4;
const HAT: u8 = 42;
/// Frames of each sample of the pack: shorter than a sixteenth (6000 frames).
const SAMPLE_FRAMES: usize = 2000;

/// The hi-hat of a sampled kit: below velocity 100 four soft samples take turns, from 100 up
/// four hard ones. Each sample is one level all through, so the level that comes out says
/// which one played.
fn hat_pack(name: &str) -> TestPack {
    let pack = TestPack::new(name);
    let mut regions = Vec::new();
    for (i, value) in [0.1, 0.15, 0.2, 0.25, 0.5, 0.6, 0.7, 0.8]
        .into_iter()
        .enumerate()
    {
        let sample = format!("hat{i}");
        pack.wav(&sample, RATE as u32, &[&level(value, SAMPLE_FRAMES)]);
        let velocities = if i < 4 {
            "\"velHi\": 99"
        } else {
            "\"velLo\": 100"
        };
        let more = format!(
            "{velocities}, \"oneShot\": true, \"ampVeltrack\": 0, \"rrIndex\": {}, \
             \"rrCount\": 4",
            i % 4 + 1
        );
        regions.push(region(&sample, HAT, HAT, HAT, &more));
    }
    pack.instrument("", &regions);
    pack
}

fn on(tick: u64, note: u8, velocity: u8) -> SequenceEvent {
    SequenceEvent {
        tick,
        event: InstrumentEvent::NoteOn { note, velocity },
    }
}

fn off(tick: u64, note: u8) -> SequenceEvent {
    SequenceEvent {
        tick,
        event: InstrumentEvent::NoteOff { note },
    }
}

/// Sixteenths on the hi-hat through `bars` from `from`: hard on the first three beats, soft
/// elsewhere. Neither count of a bar is a multiple of four.
fn hats(from: u64, bars: u64) -> Vec<SequenceEvent> {
    (0..bars * 16)
        .map(|i| {
            let hard = i % 16 < 12 && i.is_multiple_of(4);
            on(from + i * SIXTEENTH, HAT, if hard { 120 } else { 60 })
        })
        .collect()
}

struct Rig {
    parts: EngineParts,
    out: Vec<f32>,
}

impl Rig {
    fn new(instruments: Vec<(Box<dyn Instrument>, Vec<SequenceEvent>)>) -> Rig {
        let mut parts = Engine::new(EngineConfig::default());
        for (track, (instrument, events)) in instruments.into_iter().enumerate() {
            assert!(parts.handle.swap_instrument(track, instrument).is_ok());
            assert!(
                (parts.handle)
                    .swap_sequence(track, Sequence::new(events))
                    .is_ok()
            );
        }
        let mut rig = Rig {
            parts,
            out: vec![0.0; 2 * BLOCK],
        };
        rig.run(BLOCK);
        rig
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    /// The left channel of `frames` frames, in whole blocks.
    fn run(&mut self, frames: usize) -> Vec<f32> {
        let mut left = Vec::with_capacity(frames + BLOCK);
        while left.len() < frames {
            self.parts
                .engine
                .process(&mut self.out, BlockTiming::default());
            left.extend(self.out.as_chunks::<2>().0.iter().map(|f| f[0]));
        }
        left.truncate(frames);
        left
    }

    /// Stops, lets every sound die away, and plays `frames` frames from `from`.
    fn play(&mut self, from: u64, frames: usize) -> Vec<f32> {
        self.send(EngineCommand::Stop { t_us: 0 });
        let after = self.run(12 * RATE);
        assert!(after[after.len() - RATE..].iter().all(|&s| s == 0.0));
        self.send(EngineCommand::Play { from });
        self.run(frames)
    }
}

/// The level each hit plays at: the output in the middle of its sample.
fn levels(left: &[f32], first_frame: usize, hits: usize) -> Vec<f32> {
    let step = BAR_FRAMES / 16;
    (0..hits)
        .map(|i| left[first_frame + i * step + SAMPLE_FRAMES / 2])
        .collect()
}

fn differ(a: f32, b: f32) -> bool {
    (a - b).abs() > 1.0e-3 * a.abs().max(b.abs())
}

/// Hard and soft hits each go round their four samples, the hard ones too, though on the
/// key they are every fourth hit; a play from the middle hears what a play from the head does.
#[test]
fn hits_of_one_group_take_turns_by_their_place_in_the_song() {
    let pack = hat_pack("song-places-hat");
    let sampler = pack.sampler(RATE as f32, BLOCK);
    let mut rig = Rig::new(vec![(Box::new(sampler), hats(0, 4))]);
    let head = rig.play(0, 4 * BAR_FRAMES);
    let all = levels(&head, 0, 64);
    let is_hard = |i: usize| i % 16 < 12 && i.is_multiple_of(4);
    let pick = |hard: bool| -> Vec<f32> {
        (all.iter().enumerate())
            .filter(|&(i, _)| is_hard(i) == hard)
            .map(|(_, &l)| l)
            .collect()
    };
    let (hard, soft) = (pick(true), pick(false));
    for (name, hits) in [("hard", &hard), ("soft", &soft)] {
        for w in hits.windows(4) {
            for a in 0..4 {
                for b in a + 1..4 {
                    assert!(differ(w[a], w[b]), "{name}: {w:?}");
                }
            }
        }
        for w in hits.windows(5) {
            assert!(!differ(w[0], w[4]), "{name}: {w:?}");
        }
    }
    let softest_hard = hard.iter().fold(f32::MAX, |m, &l| m.min(l));
    assert!(soft.iter().all(|&l| l < softest_hard && l > 0.0));

    // From the third bar, twice: the hits are the ones the head gave there.
    for _ in 0..2 {
        let middle = rig.play(2 * BAR, 2 * BAR_FRAMES);
        assert_eq!(levels(&middle, 0, 32), all[32..]);
    }
}

/// The keys someone plays take turns as before: the same key again is another sample.
#[test]
fn keys_played_now_go_round_the_samples() {
    let pack = hat_pack("song-places-keys");
    let mut sampler = pack.sampler(RATE as f32, BLOCK);
    let mut heard = Vec::new();
    for _ in 0..8 {
        sampler.event(InstrumentEvent::NoteOn {
            note: HAT,
            velocity: 60,
        });
        let (left, _) = support::pack::play(&mut sampler, SAMPLE_FRAMES + 1000, BLOCK);
        heard.push(left[SAMPLE_FRAMES / 2]);
    }
    for (i, w) in heard.windows(2).enumerate() {
        assert!(differ(w[0], w[1]), "key {i}: {heard:?}");
    }
    assert!(!differ(heard[0], heard[4]), "{heard:?}");
}

/// A sampled kit, the lead, the pad and the electronic kit on one engine. Two bars of notes,
/// six bars of nothing (the pad's release dies away), two bars of notes.
fn band(pack: &TestPack) -> Vec<(Box<dyn Instrument>, Vec<SequenceEvent>)> {
    let mut hat = hats(0, 2);
    hat.extend(hats(8 * BAR, 2));
    let mut lead = Vec::new();
    let mut pad = Vec::new();
    let mut kit = Vec::new();
    for from in [0, 8 * BAR] {
        for beat in 0..8 {
            let tick = from + beat * Q;
            let note = 60 + (beat * 5 % 12) as u8;
            lead.push(on(tick, note, 90));
            lead.push(off(tick + Q / 2, note));
            // Two presses of the same key at once.
            lead.push(on(tick, note + 12, 70));
            lead.push(on(tick, note + 12, 80));
            lead.push(off(tick + Q / 3, note + 12));
            let piece = if beat % 2 == 0 {
                DrumPiece::Snare
            } else {
                DrumPiece::OpenHiHat
            };
            kit.push(on(tick, piece.note(), 100));
            kit.push(on(tick + Q / 2, DrumPiece::ClosedHiHat.note(), 70));
        }
        for (i, note) in [48, 55, 64].into_iter().enumerate() {
            pad.push(on(from + i as u64 * 60, note, 80));
            pad.push(off(from + 2 * BAR - Q, note));
        }
    }
    vec![
        (Box::new(pack.sampler(RATE as f32, BLOCK)), hat),
        (Box::new(PolySynth::lead()), lead),
        (Box::new(PolySynth::pad()), pad),
        (Box::new(ElectroKit::new()), kit),
    ]
}

/// The whole song twice on one engine is the same to the last bit, and so is the part from
/// the ninth bar played from there. Nothing is allocated on the audio thread meanwhile.
#[test]
fn the_song_sounds_the_same_from_the_head_twice_and_from_the_middle() {
    let pack = hat_pack("song-places-band");
    let mut rig = Rig::new(band(&pack));
    let before = common::audio_block_counts();
    let first = rig.play(0, 11 * BAR_FRAMES);
    assert!(first[..2 * BAR_FRAMES].iter().any(|s| s.abs() > 0.05));
    assert!(first[8 * BAR_FRAMES..].iter().any(|s| s.abs() > 0.05));
    assert!(
        first[6 * BAR_FRAMES..8 * BAR_FRAMES]
            .iter()
            .all(|&s| s == 0.0),
        "the gap is silent, so the second part is the notes of its own bars only"
    );
    let second = rig.play(0, 11 * BAR_FRAMES);
    let differs = |a: &[f32], b: &[f32]| {
        a.iter()
            .zip(b)
            .position(|(a, b)| a.to_bits() != b.to_bits())
    };
    assert_eq!(
        differs(&first, &second),
        None,
        "the frame where the second play differs"
    );
    let middle = rig.play(8 * BAR, 3 * BAR_FRAMES);
    assert_eq!(
        differs(&first[8 * BAR_FRAMES..], &middle),
        None,
        "the frame where the play from the ninth bar differs"
    );
    assert_eq!(common::audio_block_counts(), before);
}
