//! A kit shaped like the studio kit (one-shot cymbal with layers and turns, a kick, hi-hats
//! that choke each other, 32 voices) through the engine: a short crash rings on after its key
//! goes up, played on the keys and from a sequence, alone and among kicks and hi-hats.

mod support;

use std::sync::Arc;

use meloet_engine::{
    BlockTiming, Engine, EngineCommand, EngineConfig, EngineParts, InstrumentEvent, MidiIn,
    Sequence, SequenceEvent,
};
use meloet_sound::sampler::{SampleBank, Sampler};
use support::pack::{TestPack, off, on, region};

const RATE: usize = 48_000;
const BLOCK: usize = 256;
const CRASH: u8 = 49;
/// A sixteenth at 120 a minute, in ticks and in frames.
const SIXTEENTH: u64 = 480;
const SIXTEENTH_FRAMES: usize = RATE / 8;

/// Cymbal on the left, the rest on the right: the left channel is the cymbal alone.
fn kit(name: &str) -> (TestPack, Arc<SampleBank>) {
    let pack = TestPack::new(name);
    let fall = |frames: usize, amp: f32| -> Vec<f32> {
        (0..frames)
            .map(|i| amp * (1.0 - i as f32 / frames as f32))
            .collect()
    };
    pack.wav("crash", 48_000, &[&fall(3 * RATE, 0.5)]);
    pack.wav("short", 48_000, &[&fall(RATE, 0.5)]);
    let mut regions = Vec::new();
    for (lo, hi) in [(1, 64), (65, 127)] {
        for rr in 1..=4 {
            regions.push(region(
                "crash",
                CRASH,
                CRASH,
                CRASH,
                &format!(
                    "\"velLo\": {lo}, \"velHi\": {hi}, \"ampVeltrack\": 0, \"oneShot\": true, \
                     \"rrIndex\": {rr}, \"rrCount\": 4, \"pan\": -1"
                ),
            ));
        }
    }
    let right = "\"ampVeltrack\": 0, \"oneShot\": true, \"pan\": 1";
    regions.push(region("short", 36, 36, 36, right));
    regions.push(region(
        "short",
        42,
        42,
        42,
        &format!("{right}, \"chokeGroup\": 1"),
    ));
    regions.push(region(
        "short",
        46,
        46,
        46,
        &format!("{right}, \"chokeGroup\": 2, \"chokedBy\": [1], \"releaseMs\": 200"),
    ));
    pack.instrument("\"polyphony\": 32, \"sustainPedal\": false", &regions);
    let bank = pack.load();
    (pack, bank)
}

fn engine(bank: &Arc<SampleBank>) -> EngineParts {
    let mut parts = Engine::new(EngineConfig::default());
    let sampler = Box::new(Sampler::new(bank.clone()));
    assert!(parts.handle.swap_instrument(0, sampler).is_ok());
    parts
}

/// Renders `blocks` blocks; `at(block)` gives the keys pressed (and let go) at its start.
fn left(
    parts: &mut EngineParts,
    blocks: usize,
    at: impl Fn(usize) -> Vec<InstrumentEvent>,
) -> Vec<f32> {
    let mut out = vec![0.0f32; BLOCK * 2];
    let mut left = Vec::with_capacity(blocks * BLOCK);
    for block in 0..blocks {
        for event in at(block) {
            (parts.midi_in.push(MidiIn { event, t_us: 0 })).expect("midi ring full");
        }
        parts.engine.process(&mut out, BlockTiming::default());
        left.extend(out.chunks(2).map(|f| f[0]));
    }
    left
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

/// The cymbal's loudness from 0.5 s to 1 s, and from 2 s to 2.5 s.
fn tail(left: &[f32]) -> (f32, f32) {
    (
        rms(&left[RATE / 2..RATE]),
        rms(&left[2 * RATE..5 * RATE / 2]),
    )
}

fn assert_rings(what: &str, got: (f32, f32), alone: (f32, f32)) {
    assert!(alone.0 > 0.05 && alone.1 > 0.02, "{what}: alone {alone:?}");
    assert!(
        (got.0 - alone.0).abs() < 1.0e-4 && (got.1 - alone.1).abs() < 1.0e-4,
        "{what}: {got:?}, alone {alone:?}"
    );
}

const BLOCKS: usize = 3 * RATE / BLOCK;
const BEAT_BLOCKS: usize = RATE / 4 / BLOCK;

#[test]
fn a_short_crash_on_the_keys_rings_on() {
    let (_pack, bank) = kit("kit-tails-keys");
    let held = |frames: usize| {
        let up = frames / BLOCK;
        move |block: usize| match block {
            0 => vec![on(CRASH, 100)],
            b if b == up => vec![off(CRASH)],
            _ => Vec::new(),
        }
    };
    let alone = tail(&left(&mut engine(&bank), BLOCKS, held(2 * RATE)));
    let short = tail(&left(&mut engine(&bank), BLOCKS, held(SIXTEENTH_FRAMES)));
    assert_rings("a sixteenth", short, alone);

    // Kicks and hi-hats every eighth, each let go after a sixteenth.
    let short_crash = held(SIXTEENTH_FRAMES);
    let busy = tail(&left(&mut engine(&bank), BLOCKS, |block| {
        let mut events = short_crash(block);
        let beat = block % BEAT_BLOCKS;
        if beat == 0 {
            events.extend([
                on(36, 100),
                on(
                    if block % (2 * BEAT_BLOCKS) == 0 {
                        42
                    } else {
                        46
                    },
                    100,
                ),
            ]);
        } else if beat == SIXTEENTH_FRAMES / BLOCK {
            events.extend([off(36), off(42), off(46)]);
        }
        events
    }));
    assert_rings("among kicks and hi-hats", busy, alone);
}

fn played(bank: &Arc<SampleBank>, crash_ticks: u64, others: bool) -> (f32, f32) {
    let mut parts = engine(bank);
    let mut events = vec![
        SequenceEvent {
            tick: 0,
            event: on(CRASH, 100),
        },
        SequenceEvent {
            tick: crash_ticks,
            event: off(CRASH),
        },
    ];
    if others {
        for eighth in 0..12 {
            let tick = eighth * 2 * SIXTEENTH;
            let hat = if eighth % 2 == 0 { 42 } else { 46 };
            for key in [36, hat] {
                events.push(SequenceEvent {
                    tick,
                    event: on(key, 100),
                });
                events.push(SequenceEvent {
                    tick: tick + SIXTEENTH,
                    event: off(key),
                });
            }
        }
    }
    assert!(parts.handle.swap_sequence(0, Sequence::new(events)).is_ok());
    assert!(parts.handle.send(EngineCommand::Play { from: 0 }).is_ok());
    tail(&left(&mut parts, BLOCKS, |_| Vec::new()))
}

#[test]
fn a_short_crash_of_the_song_rings_on() {
    let (_pack, bank) = kit("kit-tails-song");
    let alone = played(&bank, 16 * SIXTEENTH, false);
    assert_rings("a sixteenth", played(&bank, SIXTEENTH, false), alone);
    assert_rings(
        "among kicks and hi-hats",
        played(&bank, SIXTEENTH, true),
        alone,
    );
}
