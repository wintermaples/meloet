//! The voice of a vocal track, without a device: the input's channels come through the
//! monitor ring to the track's strip, and nothing on the way takes memory.

mod common;

use std::sync::Arc;

use meloet_engine::{
    BlockTiming, Engine, EngineCommand, EngineConfig, EngineParts, InputEnds, InputFeed,
    InputMeters, InputTap, MixStrip, PlaceholderSynth, Voice, input_ends,
};

const RATE: u32 = 48_000;
const BLOCK: usize = 256;

struct Rig {
    parts: EngineParts,
    tap: InputTap,
    meters: Arc<InputMeters>,
    out: Vec<f32>,
    frames: u64,
    /// What the input gives on its two channels, frame by frame.
    given: u64,
}

fn left_of(n: u64) -> f32 {
    0.25 + (n % 1_000) as f32 / 4_000.0
}

fn right_of(n: u64) -> f32 {
    -left_of(n) / 2.0
}

impl Rig {
    fn new() -> Rig {
        let mut parts = Engine::new(EngineConfig::default());
        let InputEnds {
            tap,
            monitor,
            record: _,
            meters,
        } = input_ends(2, RATE);
        let feed = InputFeed::new(monitor, meters.clone());
        assert!(parts.handle.set_input(Some(feed)).is_ok());
        Rig {
            parts,
            tap,
            meters,
            out: vec![0.0; BLOCK * 2],
            frames: 0,
            given: 0,
        }
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok());
    }

    /// One block of the input, as its callback would bring it.
    fn input(&mut self, frames: usize) {
        let data: Vec<f32> = (0..frames as u64)
            .flat_map(|i| [left_of(self.given + i), right_of(self.given + i)])
            .collect();
        self.given += frames as u64;
        self.tap.block(&data, 0, 0);
    }

    fn output(&mut self) -> Vec<[f32; 2]> {
        let t_us = 1_000_000 + self.frames * 1_000_000 / u64::from(RATE);
        let timing = BlockTiming {
            callback_us: t_us,
            playback_us: t_us + 5_000,
        };
        self.parts.engine.process(&mut self.out, timing);
        self.frames += BLOCK as u64;
        self.out.as_chunks::<2>().0.to_vec()
    }
}

fn voice(left: u8, right: u8, on: bool) -> EngineCommand {
    EngineCommand::SetVoice {
        track: 1,
        voice: Some(Voice { left, right, on }),
    }
}

#[test]
fn the_input_is_heard_on_the_track_that_monitors_it_and_comes_in_over_twenty_ms() {
    let mut rig = Rig::new();
    // An instrument on the same track: a vocal track's is never heard.
    let synth = Box::new(PlaceholderSynth::new());
    assert!(rig.parts.handle.swap_instrument(1, synth).is_ok());
    rig.send(voice(0, 1, false));
    rig.output();
    assert!(
        !rig.meters.monitoring(),
        "no voice is on: the callback does not feed the ring"
    );
    rig.input(BLOCK);
    assert!(rig.output().iter().all(|f| *f == [0.0, 0.0]));

    rig.send(voice(0, 1, true));
    // The block that takes the command turns the ring on; the input's next block is its first.
    assert!(rig.output().iter().all(|f| *f == [0.0, 0.0]));
    assert!(rig.meters.monitoring());
    let first = rig.given;
    let mut heard = Vec::new();
    for _ in 0..8 {
        rig.input(BLOCK);
        heard.extend(rig.output());
    }
    // 20 ms are 960 frames: a straight line up, then the input as it is, left and right
    // at the middle of the pan.
    let ramp = 960.0;
    for (i, frame) in heard.iter().enumerate() {
        let gain = ((i + 1) as f32 / ramp).min(1.0);
        let n = first + i as u64;
        assert!(
            (frame[0] - left_of(n) * gain).abs() < 1e-4
                && (frame[1] - right_of(n) * gain).abs() < 1e-4,
            "frame {i}: {frame:?}"
        );
    }

    // Off: down over 20 ms, then silence, and the ring is let go of.
    rig.send(voice(0, 1, false));
    let mut tail = Vec::new();
    for _ in 0..6 {
        rig.input(BLOCK);
        tail.extend(rig.output());
    }
    assert!(tail[0][0] > 0.2 && tail[900][0].abs() > 0.0);
    assert!(tail[961..].iter().all(|f| *f == [0.0, 0.0]));
    assert!(!rig.meters.monitoring());
}

#[test]
fn a_mono_input_goes_to_both_sides_and_through_the_strip() {
    let mut rig = Rig::new();
    rig.send(voice(1, 1, true));
    rig.send(EngineCommand::SetStrip {
        strip: MixStrip::Track(1),
        gain: 0.5,
        pan: 0.0,
    });
    rig.output();
    let first = rig.given;
    let mut heard = Vec::new();
    for _ in 0..8 {
        rig.input(BLOCK);
        heard.extend(rig.output());
    }
    for (i, frame) in heard.iter().enumerate().skip(1_200) {
        let want = right_of(first + i as u64) * 0.5;
        assert!(
            (frame[0] - want).abs() < 1e-4 && (frame[1] - want).abs() < 1e-4,
            "frame {i}: {frame:?} for {want}"
        );
    }
}

#[test]
fn the_output_waits_a_block_behind_an_input_that_comes_after_it_and_catches_up_after_a_stall() {
    let mut rig = Rig::new();
    rig.send(voice(0, 0, true));
    rig.output();
    // The output's block comes first in every cycle: it finds nothing once, then always the
    // block of the cycle before.
    let mut heard = Vec::new();
    for _ in 0..12 {
        heard.extend(rig.output());
        rig.input(BLOCK);
    }
    assert!(heard[..BLOCK].iter().all(|f| *f == [0.0, 0.0]));
    let at = 6 * BLOCK;
    let want = left_of((at - BLOCK) as u64);
    assert!(
        (heard[at][0] - want).abs() < 1e-4,
        "{} for {want}",
        heard[at][0]
    );

    // The output stalls for five cycles of the input. Afterwards the voice is one block
    // late again, not six.
    for _ in 0..5 {
        rig.input(BLOCK);
    }
    let given = rig.given;
    let block = rig.output();
    let want = left_of(given - BLOCK as u64);
    assert!(
        (block[0][0] - want).abs() < 1e-4,
        "{} for {want}",
        block[0][0]
    );
}

#[test]
fn the_voice_never_allocates_whatever_the_input_does() {
    let mut rig = Rig::new();
    rig.send(voice(0, 1, true));
    rig.send(EngineCommand::SetVoice {
        track: 2,
        voice: Some(Voice {
            left: 1,
            right: 1,
            on: true,
        }),
    });
    let data: Vec<f32> = (0..2 * 8_192)
        .map(|i| (i as f32 * 0.01).sin() * 0.5)
        .collect();
    let sizes = [256usize, 0, 1024, 37, 4_096, 64, 512, 1, 8_192, 300];
    let before = common::audio_block_counts();
    for round in 0..2_000 {
        let frames = sizes[round % sizes.len()];
        rig.tap.block(&data[..2 * frames], 0, 0);
        rig.output();
        if round == 700 {
            rig.send(voice(0, 1, false));
        }
        if round == 900 {
            rig.send(voice(1, 0, true));
        }
        if round == 1_500 {
            // The input closes: the feed comes back to this side, and is dropped here.
            assert!(rig.parts.handle.set_input(None).is_ok());
        }
    }
    assert_eq!(common::audio_block_counts(), before);
    assert_eq!(rig.parts.handle.drop_retired(), 1, "the feed came back");
    assert!(!rig.meters.monitoring());
}
