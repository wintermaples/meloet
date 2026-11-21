//! The path of the mixer without a device: inserts, fader, pan, sends, returns, master
//! (docs/architecture.md 19-11). The instruments and effects here are arithmetic, so that
//! every sample can be said in advance.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use meloet_engine::{
    BlockTiming, Effect, EffectContext, Engine, EngineCommand, EngineConfig, EngineParts, FADE_S,
    INSERT_SLOTS, Instrument, InstrumentEvent, MixStrip, RAMP_S, pan_gains,
};

const RATE: usize = 48_000;
const RAMP: usize = (RAMP_S * RATE as f32) as usize;
const FADE: usize = (FADE_S * RATE as f32) as usize;

/// The same value on every frame.
struct Dc(f32, f32);

impl Instrument for Dc {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn event(&mut self, _ev: InstrumentEvent) {}
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(self.0);
        right.fill(self.1);
    }
    fn reset(&mut self) {}
}

#[derive(Clone, Copy)]
enum Op {
    Add,
    /// Parameter 0 sets the factor.
    Scale,
    Ceiling,
}

struct Fx {
    op: Op,
    value: f32,
    prepared: Arc<AtomicUsize>,
    dropped_in_block: Arc<AtomicUsize>,
}

impl Effect for Fx {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {
        self.prepared.fetch_add(1, Relaxed);
    }
    fn set_param(&mut self, index: u32, value: f32) {
        if index == 0 {
            self.value = value;
        }
    }
    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        for s in left.iter_mut().chain(right.iter_mut()) {
            *s = match self.op {
                Op::Add => *s + self.value,
                Op::Scale => *s * self.value,
                Op::Ceiling => s.min(self.value),
            };
        }
    }
    fn reset(&mut self) {}
}

impl Drop for Fx {
    fn drop(&mut self) {
        if meloet_engine::rt::in_audio_block() {
            self.dropped_in_block.fetch_add(1, Relaxed);
        }
    }
}

/// Silences what goes through it, and counts the frames it was given.
struct Counting(Arc<AtomicUsize>);

impl Effect for Counting {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, _index: u32, _value: f32) {}
    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        self.0.fetch_add(left.len(), Relaxed);
        left.fill(0.0);
        right.fill(0.0);
    }
    fn reset(&mut self) {}
}

struct Rig {
    parts: EngineParts,
    prepared: Arc<AtomicUsize>,
    dropped_in_block: Arc<AtomicUsize>,
}

const A: MixStrip = MixStrip::Return(0);
const B: MixStrip = MixStrip::Return(1);

impl Rig {
    /// Track 0 gives 0.1 on the left and 0.2 on the right.
    fn new() -> Rig {
        let mut rig = Rig {
            parts: Engine::new(EngineConfig::default()),
            prepared: Arc::default(),
            dropped_in_block: Arc::default(),
        };
        rig.instrument(0, 0.1, 0.2);
        rig.run(256);
        rig
    }

    fn instrument(&mut self, track: usize, left: f32, right: f32) {
        let instrument = Box::new(Dc(left, right));
        assert!(self.parts.handle.swap_instrument(track, instrument).is_ok());
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    fn level(&mut self, strip: MixStrip, gain: f32, pan: f32) {
        self.send(EngineCommand::SetStrip { strip, gain, pan });
    }

    fn sends(&mut self, track: usize, a: f32, b: f32) {
        self.send(EngineCommand::SetSends {
            track,
            sends: [a, b],
        });
    }

    fn fx(&self, op: Op, value: f32) -> Option<Box<dyn Effect>> {
        Some(Box::new(Fx {
            op,
            value,
            prepared: self.prepared.clone(),
            dropped_in_block: self.dropped_in_block.clone(),
        }))
    }

    fn effect(&mut self, strip: MixStrip, slot: usize, effect: Option<Box<dyn Effect>>) {
        assert!(self.parts.handle.set_effect(strip, slot, effect).is_ok());
    }

    fn bypass(&mut self, strip: MixStrip, slot: usize, bypassed: bool) {
        self.send(EngineCommand::SetBypass {
            strip,
            slot,
            bypassed,
        });
    }

    fn move_effect(&mut self, strip: MixStrip, from: usize, to: usize) {
        self.send(EngineCommand::MoveEffect { strip, from, to });
    }

    /// Renders `frames` frames in blocks of 256 and returns left and right.
    fn run(&mut self, frames: usize) -> (Vec<f32>, Vec<f32>) {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        let mut out = [0.0f32; 512];
        while left.len() < frames {
            let n = 256.min(frames - left.len());
            let out = &mut out[..n * 2];
            self.parts.engine.process(out, BlockTiming::default());
            for frame in out.as_chunks::<2>().0 {
                left.push(frame[0]);
                right.push(frame[1]);
            }
        }
        (left, right)
    }

    /// What comes out once every ramp and fade is over.
    fn settled(&mut self) -> (f32, f32) {
        self.run(RAMP.max(FADE) + 256);
        let (left, right) = self.run(1);
        (left[0], right[0])
    }

    fn check_rules(&self) {
        assert_eq!(self.dropped_in_block.load(Relaxed), 0);
        assert_eq!(common::audio_block_counts(), (0, 0));
    }
}

fn near(got: (f32, f32), want: (f32, f32)) {
    assert!(
        (got.0 - want.0).abs() < 1e-6 && (got.1 - want.1).abs() < 1e-6,
        "got {got:?}, wanted {want:?}"
    );
}

fn largest_step(samples: &[f32]) -> f32 {
    samples
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max)
}

#[test]
fn inserts_run_from_the_first_slot_to_the_last() {
    let mut rig = Rig::new();
    let (add, scale) = (rig.fx(Op::Add, 0.1), rig.fx(Op::Scale, 2.0));
    rig.effect(MixStrip::Track(0), 1, add);
    rig.effect(MixStrip::Track(0), 3, scale);
    near(rig.settled(), (0.4, 0.6));
    let scale = rig.fx(Op::Scale, 2.0);
    rig.effect(MixStrip::Track(0), 0, scale);
    near(rig.settled(), (0.6, 1.0));
    rig.send(EngineCommand::SetEffectParam {
        strip: MixStrip::Track(0),
        slot: 0,
        param: 0,
        value: 1.0,
    });
    near(rig.settled(), (0.4, 0.6));
    rig.check_rules();
}

#[test]
fn the_pan_is_a_balance_that_never_makes_a_side_louder() {
    assert_eq!(pan_gains(0.0), [1.0, 1.0]);
    assert_eq!(pan_gains(1.0), [0.0, 1.0]);
    assert_eq!(pan_gains(-1.0), [1.0, 0.0]);
    assert_eq!(pan_gains(f32::NAN), [1.0, 1.0]);
    let mut before = 1.0;
    for step in 1..=100 {
        let [left, right] = pan_gains(step as f32 / 100.0);
        assert!(left < before && right == 1.0, "{step}: {left}");
        assert_eq!(pan_gains(-step as f32 / 100.0), [right, left]);
        before = left;
    }
    let half = pan_gains(0.5)[0];
    assert!((half - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
}

#[test]
fn sends_are_taken_after_the_fader_and_the_pan() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    rig.level(MixStrip::Track(0), 0.5, -1.0);
    rig.sends(0, 1.0, 0.0);
    // The track itself and what comes back from return A.
    near(rig.settled(), (0.1, 0.0));
    meters.take_return_peak(0);
    meters.take_track_peak(0);
    rig.run(256);
    assert_eq!(meters.take_track_peak(0), [0.05, 0.0]);
    assert_eq!(meters.take_return_peak(0), [0.05, 0.0]);
    assert_eq!(meters.take_return_peak(1), [0.0, 0.0]);

    rig.level(MixStrip::Track(0), 0.0, 0.0);
    near(rig.settled(), (0.0, 0.0));
    meters.take_return_peak(0);
    rig.run(256);
    assert_eq!(
        meters.take_return_peak(0),
        [0.0, 0.0],
        "fader down, send down"
    );
    rig.check_rules();
}

#[test]
fn a_return_runs_its_inserts_then_its_fader_and_pan() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    rig.sends(0, 1.0, 0.5);
    let scale = rig.fx(Op::Scale, 3.0);
    rig.effect(A, 0, scale);
    rig.level(A, 0.5, 1.0);
    // Track (0.1, 0.2), return A (0, 0.2 × 3 × 0.5), return B (0.05, 0.1).
    near(rig.settled(), (0.15, 0.6));
    meters.take_return_peak(0);
    meters.take_return_peak(1);
    rig.run(256);
    near(meters.take_return_peak(0).into(), (0.0, 0.3));
    near(meters.take_return_peak(1).into(), (0.05, 0.1));

    // A muted return: only that return is silent.
    rig.level(A, 0.0, 1.0);
    near(rig.settled(), (0.15, 0.3));
    rig.level(B, 0.0, 0.0);
    near(rig.settled(), (0.1, 0.2));
    rig.check_rules();
}

#[test]
fn the_master_runs_its_fader_and_pan_before_its_inserts() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    let ceiling = rig.fx(Op::Ceiling, 0.3);
    rig.effect(MixStrip::Master, 0, ceiling);
    near(rig.settled(), (0.1, 0.2));
    rig.level(MixStrip::Master, 2.0, 0.0);
    near(rig.settled(), (0.2, 0.3));
    meters.take_master_peak();
    rig.run(256);
    assert_eq!(meters.take_master_peak(), [0.2, 0.3], "what goes out");
    rig.level(MixStrip::Master, 2.0, -1.0);
    near(rig.settled(), (0.2, 0.0));
    let add = rig.fx(Op::Add, 0.05);
    rig.effect(MixStrip::Master, 1, add);
    near(rig.settled(), (0.25, 0.05));
    rig.check_rules();
}

#[test]
fn the_test_tone_does_not_go_through_the_masters_strip() {
    let mut rig = Rig::new();
    rig.level(MixStrip::Master, 0.0, 0.0);
    near(rig.settled(), (0.0, 0.0));
    rig.send(EngineCommand::TestTone);
    let (left, _) = rig.run(4800);
    assert!(largest_step(&left) > 0.001, "the tone sounds");
}

#[test]
fn fader_pan_and_sends_move_in_a_straight_line_over_20_ms() {
    let mut rig = Rig::new();
    rig.level(MixStrip::Track(0), 0.0, 0.0);
    let (left, right) = rig.run(RAMP + 64);
    let step = 0.1 / RAMP as f32;
    assert!((largest_step(&left) - step).abs() < step * 0.01);
    assert!((largest_step(&right) - step * 2.0).abs() < step * 0.02);
    assert!(left[RAMP - 2] > 0.0, "not there before 20 ms");
    assert_eq!(left[RAMP - 1], 0.0, "there after 20 ms");
    assert!(
        (left[RAMP / 2 - 1] - 0.05).abs() < 1e-4,
        "half way at 10 ms"
    );

    rig.level(MixStrip::Track(0), 1.0, 1.0);
    let (left, right) = rig.run(RAMP + 64);
    assert!(largest_step(&left) <= step * 1.01 && largest_step(&right) <= step * 2.02);
    near((left[RAMP - 1], right[RAMP - 1]), (0.0, 0.2));

    rig.level(MixStrip::Track(0), 1.0, 0.0);
    rig.settled();
    rig.sends(0, 1.0, 0.0);
    let (left, _) = rig.run(RAMP + 64);
    assert!((largest_step(&left) - step).abs() < step * 0.01);
    near((left[RAMP - 2], left[RAMP - 1]), (0.2 - step, 0.2));

    // A new target on the way starts from where the value is.
    rig.level(A, 0.0, 0.0);
    rig.run(RAMP / 2);
    rig.level(A, 1.0, 0.0);
    let (left, _) = rig.run(RAMP + 64);
    assert!(largest_step(&left) <= step * 0.51);
    assert_eq!(left[RAMP - 1], 0.2);
    rig.check_rules();
}

#[test]
fn a_strip_with_no_instrument_takes_its_values_at_once() {
    let mut rig = Rig::new();
    rig.level(MixStrip::Track(5), 0.5, 0.0);
    rig.sends(5, 1.0, 0.0);
    let scale = rig.fx(Op::Scale, 2.0);
    rig.effect(MixStrip::Track(5), 0, scale);
    rig.level(MixStrip::Track(0), 0.0, 0.0);
    rig.settled();
    rig.instrument(5, 0.1, 0.1);
    let (left, _) = rig.run(64);
    assert!(left.iter().all(|s| (s - 0.2).abs() < 1e-6), "{left:?}");
    rig.check_rules();
}

#[test]
fn an_effect_fades_in_and_out_over_10_ms_and_comes_back_on_the_ring() {
    let mut rig = Rig::new();
    let step = 0.1 / FADE as f32;
    let silence = rig.fx(Op::Scale, 0.0);
    rig.effect(MixStrip::Track(0), 2, silence);
    let (left, _) = rig.run(FADE + 32);
    assert!((largest_step(&left) - step).abs() < step * 0.01);
    assert_eq!(left[0], 0.1, "starts as it was");
    assert_eq!(left[FADE], 0.0);

    // Another effect in its place: the one before sounds on for the fade, then comes back.
    let double = rig.fx(Op::Scale, 2.0);
    rig.effect(MixStrip::Track(0), 2, double);
    let (left, _) = rig.run(256);
    assert!(largest_step(&left) <= step * 2.01);
    assert!(rig.parts.handle.take_retired_effect().is_none());
    let (left, _) = rig.run(FADE);
    assert_eq!(left[FADE - 1], 0.2);
    assert!(rig.parts.handle.take_retired_effect().is_some());
    assert!(rig.parts.handle.take_retired_effect().is_none());

    rig.effect(MixStrip::Track(0), 2, None);
    let (left, _) = rig.run(FADE + 256);
    assert!((largest_step(&left) - step).abs() < step * 0.01);
    assert_eq!(left[FADE], 0.1);
    assert!(rig.parts.handle.take_retired_effect().is_some());

    // Changed twice within the fade: the first one comes back at once.
    let (first, second) = (rig.fx(Op::Scale, 0.5), rig.fx(Op::Scale, 0.25));
    rig.effect(MixStrip::Track(0), 2, first);
    rig.run(64);
    rig.effect(MixStrip::Track(0), 2, second);
    rig.run(64);
    rig.effect(MixStrip::Track(0), 2, None);
    rig.run(64);
    assert_eq!(rig.parts.handle.drop_retired(), 1);
    near(rig.settled(), (0.1, 0.2));
    assert_eq!(rig.parts.handle.drop_retired(), 1);

    // A slot that does not exist gives the effect back.
    let lost = rig.fx(Op::Add, 1.0);
    rig.effect(MixStrip::Track(0), INSERT_SLOTS, lost);
    let lost = rig.fx(Op::Add, 1.0);
    rig.effect(MixStrip::Return(2), 0, lost);
    near(rig.settled(), (0.1, 0.2));
    assert_eq!(rig.parts.handle.drop_retired(), 2);
    rig.check_rules();
}

#[test]
fn a_slot_is_passed_by_over_10_ms_and_its_effect_keeps_running() {
    let mut rig = Rig::new();
    let track = MixStrip::Track(0);
    let step = 0.1 / FADE as f32;
    let counted = Arc::new(AtomicUsize::new(0));
    rig.effect(track, 1, Some(Box::new(Counting(counted.clone()))));
    near(rig.settled(), (0.0, 0.0));

    rig.bypass(track, 1, true);
    let before = counted.load(Relaxed);
    let (left, right) = rig.run(FADE + 256);
    assert!((largest_step(&left) - step).abs() < step * 0.01);
    assert!((largest_step(&right) - step * 2.0).abs() < step * 0.02);
    assert_eq!(left[0], 0.0, "starts as it was");
    assert_eq!((left[FADE], right[FADE]), (0.1, 0.2));
    assert_eq!(counted.load(Relaxed) - before, FADE + 256, "still running");

    rig.bypass(track, 1, false);
    let (left, _) = rig.run(FADE + 256);
    assert!((largest_step(&left) - step).abs() < step * 0.01);
    assert_eq!((left[0], left[FADE]), (0.1, 0.0));

    // Turned around within the fade: it goes back from where it was.
    rig.bypass(track, 1, true);
    let (out, _) = rig.run(FADE / 4);
    rig.bypass(track, 1, false);
    let (back, _) = rig.run(FADE);
    let all: Vec<f32> = out.into_iter().chain(back).collect();
    assert!(largest_step(&all) < step * 1.01, "{}", largest_step(&all));
    assert!(all[FADE / 4 - 1] > 0.02 && all[FADE / 2] == 0.0);

    // The mark is the slot's: an effect put into a slot that is passed by is not heard, and
    // one that changes while it is passed by is not heard either.
    rig.bypass(track, 3, true);
    rig.run(64);
    for value in [0.0, 5.0] {
        let fx = rig.fx(Op::Scale, value);
        rig.effect(track, 3, fx);
        let (left, _) = rig.run(FADE + 64);
        assert!(left.iter().all(|s| *s == 0.0));
    }
    rig.bypass(track, 1, true);
    near(rig.settled(), (0.1, 0.2));
    rig.bypass(track, 3, false);
    near(rig.settled(), (0.5, 1.0));

    // Passed by and given another effect in the same block: the two fades multiply, with
    // no jump.
    let quiet = rig.fx(Op::Scale, 1.0);
    rig.effect(track, 3, quiet);
    rig.bypass(track, 3, true);
    let (left, _) = rig.run(FADE + 64);
    assert!(largest_step(&left) < 0.8 / FADE as f32 * 1.01);
    assert_eq!(left[FADE], 0.1);

    // A track with no instrument takes it at once, and so does a slot that does not exist.
    rig.bypass(MixStrip::Track(3), 0, true);
    rig.bypass(track, INSERT_SLOTS, true);
    rig.bypass(MixStrip::Return(5), 0, true);
    near(rig.settled(), (0.1, 0.2));
    rig.parts.handle.drop_retired();
    rig.check_rules();
}

#[test]
fn an_effect_moves_to_another_slot_as_it_is_with_two_fades_of_10_ms() {
    let mut rig = Rig::new();
    let track = MixStrip::Track(0);
    let (add, double) = (rig.fx(Op::Add, 0.1), rig.fx(Op::Scale, 2.0));
    rig.effect(track, 0, add);
    rig.effect(track, 1, double);
    near(rig.settled(), (0.4, 0.6));
    let prepared = rig.prepared.load(Relaxed);
    rig.parts.handle.drop_retired();

    // (x + 0.1) × 2 becomes x × 2 + 0.1: the one that moves fades out of the chain where it
    // was, and into it where it is now. Swapped at once, the sound would jump by 0.1.
    rig.move_effect(track, 0, 1);
    let (left, right) = rig.run(2 * FADE + 512);
    let step = 0.2 / FADE as f32;
    assert!((largest_step(&left) - step).abs() < step * 0.01);
    assert!((largest_step(&right) - step).abs() < step * 0.01);
    assert_eq!(left[0], 0.4, "starts as it was");
    assert!(
        (left[FADE] - 0.2).abs() < 1e-6,
        "without it: {}",
        left[FADE]
    );
    assert!(left.iter().all(|s| (0.2 - 1e-6..=0.4 + 1e-6).contains(s)));
    near((left[2 * FADE + 511], right[2 * FADE + 511]), (0.3, 0.5));
    assert_eq!(rig.prepared.load(Relaxed), prepared, "nothing made anew");
    assert_eq!(rig.parts.handle.drop_retired(), 0, "nothing put away");

    // While it is on its way, the slots are numbered as they will be.
    rig.move_effect(track, 1, 0);
    rig.send(EngineCommand::SetEffectParam {
        strip: track,
        slot: 0,
        param: 0,
        value: 0.2,
    });
    let (left, _) = rig.run(2 * FADE + 512);
    assert!(largest_step(&left) < 0.4 / FADE as f32 * 1.01);
    near(rig.settled(), (0.6, 0.8));

    // What is passed by moves at once, and stays passed by where it lands.
    rig.bypass(track, 0, true);
    near(rig.settled(), (0.2, 0.4));
    rig.move_effect(track, 0, 3);
    let (left, _) = rig.run(512);
    assert!(left.iter().all(|s| (s - 0.2).abs() < 1e-6));
    rig.bypass(track, 3, false);
    near(rig.settled(), (0.4, 0.6));

    // Past empty slots only: the order of what is heard stays, so nothing fades.
    rig.move_effect(track, 3, 1);
    let (left, _) = rig.run(512);
    assert!(left.iter().all(|s| (s - 0.4).abs() < 1e-6));
    rig.move_effect(track, 0, 2);
    near(rig.settled(), (0.6, 0.8));

    // Moved again before it landed: the first move counts as done.
    rig.move_effect(track, 2, 1);
    rig.run(FADE / 2);
    rig.move_effect(track, 1, 2);
    let (left, _) = rig.run(2 * FADE + 512);
    assert!(largest_step(&left) < 0.4 / FADE as f32 * 1.01);
    near(rig.settled(), (0.6, 0.8));

    // No stream, or no instrument: at once.
    rig.move_effect(track, 2, 0);
    rig.parts.engine.settle();
    let (left, _) = rig.run(64);
    assert!(left.iter().all(|s| (s - 0.4).abs() < 1e-6), "{left:?}");
    rig.move_effect(track, 0, INSERT_SLOTS);
    rig.move_effect(MixStrip::Return(4), 0, 1);
    rig.move_effect(MixStrip::Track(5), 0, 1);
    near(rig.settled(), (0.4, 0.6));
    assert_eq!(rig.prepared.load(Relaxed), prepared);
    assert_eq!(rig.parts.handle.drop_retired(), 0);
    rig.check_rules();
}

#[test]
fn clearing_a_track_gives_its_effects_back_and_its_strip_is_new() {
    let mut rig = Rig::new();
    for slot in 0..INSERT_SLOTS {
        let fx = rig.fx(Op::Scale, 0.5);
        rig.effect(MixStrip::Track(0), slot, fx);
    }
    rig.level(MixStrip::Track(0), 0.5, 1.0);
    rig.sends(0, 1.0, 1.0);
    rig.run(64);
    rig.send(EngineCommand::ClearTrack { track: 0 });
    near(rig.settled(), (0.0, 0.0));
    let mut effects = 0;
    while rig.parts.handle.take_retired_effect().is_some() {
        effects += 1;
    }
    assert_eq!(effects, INSERT_SLOTS);
    assert_eq!(rig.parts.handle.drop_retired(), 1, "the instrument");
    rig.instrument(0, 0.1, 0.2);
    near(rig.settled(), (0.1, 0.2));
    rig.check_rules();
}

#[test]
fn a_new_stream_prepares_the_effects_again_and_ends_the_fades() {
    let mut rig = Rig::new();
    let (first, second) = (rig.fx(Op::Scale, 0.5), rig.fx(Op::Scale, 2.0));
    rig.effect(A, 0, first);
    rig.effect(MixStrip::Master, 0, second);
    rig.level(MixStrip::Track(0), 0.5, 0.0);
    rig.run(64);
    assert_eq!(rig.prepared.load(Relaxed), 2);
    let third = rig.fx(Op::Scale, 4.0);
    rig.effect(MixStrip::Master, 0, third);
    rig.run(64);
    rig.parts.engine.prepare(44_100);
    assert_eq!(
        rig.prepared.load(Relaxed),
        5,
        "the two that are in, not the old one"
    );
    assert_eq!(rig.parts.handle.drop_retired(), 1);
    let (left, right) = rig.run(64);
    assert!(left.iter().all(|s| (s - 0.2).abs() < 1e-6), "{left:?}");
    assert!(right.iter().all(|s| (s - 0.4).abs() < 1e-6));

    // Without a stream, commands are taken in at once.
    rig.level(MixStrip::Track(0), 1.0, 0.0);
    rig.effect(MixStrip::Master, 0, None);
    rig.parts.engine.settle();
    let (left, _) = rig.run(64);
    assert!(left.iter().all(|s| (s - 0.1).abs() < 1e-6), "{left:?}");
    assert_eq!(rig.parts.handle.drop_retired(), 1);
    rig.check_rules();
}

#[test]
fn what_is_not_a_number_stays_out_of_the_sums() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    rig.instrument(1, f32::NAN, 0.3);
    rig.instrument(2, f32::INFINITY, 0.3);
    rig.sends(1, 1.0, 1.0);
    rig.sends(2, 1.0, 1.0);
    rig.level(MixStrip::Track(2), 0.0, 0.0);
    near(rig.settled(), (0.1, 0.2));
    meters.take_track_peak(1);
    meters.take_master_peak();
    rig.run(256);
    assert_eq!(meters.take_track_peak(1), [0.0, 0.0]);
    assert_eq!(meters.take_return_peak(0), [0.0, 0.0]);
    assert_eq!(meters.take_master_peak(), [0.1, 0.2]);

    // Even from the master's own effect, nothing but numbers goes out.
    let bad = rig.fx(Op::Scale, f32::NAN);
    rig.effect(MixStrip::Master, 3, bad);
    near(rig.settled(), (0.0, 0.0));
    rig.check_rules();
}

#[test]
fn the_audition_track_has_a_strip_and_the_sample_seat_has_none() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    let audition = rig.parts.handle.audition_track();
    rig.level(MixStrip::Track(0), 0.0, 0.0);
    rig.instrument(audition, 0.1, 0.1);
    rig.sends(audition, 0.5, 0.25);
    near(rig.settled(), (0.175, 0.175));

    rig.send(EngineCommand::ClearTrack { track: audition });
    assert!(rig.parts.handle.play_sample(Box::new(Dc(0.1, 0.1))).is_ok());
    near(rig.settled(), (0.1, 0.1));
    meters.take_return_peak(0);
    rig.run(256);
    assert_eq!(meters.take_return_peak(0), [0.0, 0.0]);
    rig.check_rules();
}

#[test]
fn every_mixer_command_on_every_strip_never_touches_the_allocator() {
    let mut rig = Rig::new();
    for track in 1..16 {
        rig.instrument(track, 0.01, 0.01);
    }
    let strips: Vec<MixStrip> = (0..17)
        .map(MixStrip::Track)
        .chain([A, B, MixStrip::Master, MixStrip::Return(7)])
        .collect();
    let mut seed = 12345u32;
    let mut next = move |n: usize| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as usize % n
    };
    let mut out = vec![0.0f32; 2048 * 2];
    for block in 0..3000usize {
        let strip = strips[next(strips.len())];
        match next(11) {
            0 | 1 => rig.level(strip, next(21) as f32 / 10.0, next(21) as f32 / 10.0 - 1.0),
            2 => rig.sends(next(18), next(11) as f32 / 10.0, next(11) as f32 / 10.0),
            3 => {
                let fx = rig.fx([Op::Add, Op::Scale, Op::Ceiling][next(3)], 0.5);
                rig.effect(strip, next(INSERT_SLOTS + 1), fx);
            }
            4 => rig.effect(strip, next(INSERT_SLOTS + 1), None),
            5 => rig.send(EngineCommand::SetEffectParam {
                strip,
                slot: next(INSERT_SLOTS + 1),
                param: next(2) as u32,
                value: next(11) as f32 / 10.0,
            }),
            7 | 8 => rig.bypass(strip, next(INSERT_SLOTS + 1), next(2) == 0),
            9 | 10 => rig.move_effect(strip, next(INSERT_SLOTS + 1), next(INSERT_SLOTS + 1)),
            6 if block % 50 == 0 => {
                let track = next(16);
                rig.send(EngineCommand::ClearTrack { track });
                rig.instrument(track, 0.01, 0.01);
            }
            _ => {}
        }
        let frames = [64, 256, 1024, 2048, 333, 1][block % 6];
        let out = &mut out[..frames * 2];
        rig.parts.engine.process(out, BlockTiming::default());
        assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        rig.parts.handle.drop_retired();
    }
    rig.check_rules();
}

/// Passes the sound on and says it delays it.
struct Late(u32);

impl Effect for Late {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, _index: u32, _value: f32) {}
    fn process(&mut self, _left: &mut [f32], _right: &mut [f32], _ctx: &EffectContext) {}
    fn reset(&mut self) {}
    fn latency(&self) -> u32 {
        self.0
    }
}

#[test]
fn the_engine_says_how_much_the_masters_inserts_delay() {
    let mut parts = Engine::new(EngineConfig::default());
    assert_eq!(parts.engine.master_latency(), 0);
    let master = MixStrip::Master;
    for (slot, frames) in [(0, 64), (2, 1_000)] {
        let late: Box<dyn Effect> = Box::new(Late(frames));
        assert!(parts.handle.set_effect(master, slot, Some(late)).is_ok());
    }
    // What delays a track is not the master's.
    let late: Box<dyn Effect> = Box::new(Late(7));
    assert!(
        parts
            .handle
            .set_effect(MixStrip::Track(0), 0, Some(late))
            .is_ok()
    );
    parts.engine.settle();
    assert_eq!(parts.engine.master_latency(), 1_064);
    // A slot that is passed by delays nothing.
    let pass = EngineCommand::SetBypass {
        strip: master,
        slot: 2,
        bypassed: true,
    };
    assert!(parts.handle.send(pass).is_ok());
    parts.engine.settle();
    assert_eq!(parts.engine.master_latency(), 64);
}
