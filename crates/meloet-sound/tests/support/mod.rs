//! Helpers the tests share.
#![allow(dead_code)]

pub mod mix;
pub mod pack;

use meloet_engine::{Effect, EffectContext};
use meloet_model::effects::EffectKind;
use meloet_sound::effects::create;

pub const SR: f32 = 48_000.0;
pub const BLOCK: usize = 256;
pub const TEMPO: f32 = 120.0;

pub fn prepared(kind: EffectKind) -> Box<dyn Effect> {
    let mut fx = create(kind);
    fx.prepare(SR, BLOCK);
    fx
}

/// Runs the buffers through the effect in blocks of `BLOCK`.
pub fn run(fx: &mut dyn Effect, left: &mut [f32], right: &mut [f32]) {
    run_at(fx, left, right, TEMPO);
}

pub fn run_at(fx: &mut dyn Effect, left: &mut [f32], right: &mut [f32], tempo_bpm: f32) {
    let ctx = EffectContext { tempo_bpm };
    for (l, r) in left.chunks_mut(BLOCK).zip(right.chunks_mut(BLOCK)) {
        fx.process(l, r, &ctx);
    }
}

pub fn sine(freq: f32, amp: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| {
            let phase = std::f64::consts::TAU * f64::from(freq) * i as f64 / f64::from(SR);
            (f64::from(amp) * phase.sin()) as f32
        })
        .collect()
}

/// White noise in -amp..amp, the same every run.
pub fn noise(amp: f32, frames: usize, seed: u32) -> Vec<f32> {
    let mut state = seed.wrapping_mul(2_654_435_761).max(1);
    (0..frames)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32 * 2.0 - 1.0) * amp
        })
        .collect()
}

pub fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>() / x.len() as f64).sqrt() as f32
}

pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

pub fn db(ratio: f32) -> f32 {
    20.0 * ratio.log10()
}

pub fn gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

pub fn frames(seconds: f32) -> usize {
    (seconds * SR) as usize
}
