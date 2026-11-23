//! The audio thread's rule: processing and parameter changes never allocate or free.
//! One test only, so nothing else runs in this binary while the counts are taken.

#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;
mod support;

use meloet_engine::EffectContext;
use meloet_model::effects::EffectKind;
use support::*;

#[test]
fn processing_and_parameter_changes_do_not_allocate() {
    let mut effects: Vec<_> = EffectKind::ALL.into_iter().map(prepared).collect();
    let mut left = noise(0.5, BLOCK, 1);
    let mut right = noise(0.5, BLOCK, 2);
    let source = left.clone();
    assert_eq!(common::audio_block_counts(), (0, 0));

    let mut blocks = 0_u64;
    {
        let _block = meloet_engine::rt::enter();
        for (kind, fx) in EffectKind::ALL.into_iter().zip(&mut effects) {
            let params = kind.params();
            for round in 0..400_usize {
                let spec = &params[round % params.len()];
                let value = match round % 5 {
                    0 => spec.min,
                    1 => spec.max,
                    2 => spec.default,
                    3 => spec.min + (spec.max - spec.min) * 0.37,
                    _ => f32::NAN,
                };
                fx.set_param((round % params.len()) as u32, value);
                fx.set_param(99, 1.0);

                left.copy_from_slice(&source);
                right.copy_from_slice(&source);
                match round {
                    150 => left[3] = f32::NAN,
                    250 => right[9] = f32::INFINITY,
                    300 => fx.reset(),
                    _ => {}
                }
                let frames = [BLOCK, 1, 64, 200][round % 4];
                let ctx = EffectContext {
                    tempo_bpm: [120.0, 30.0, 300.0][round / 50 % 3],
                };
                fx.process(&mut left[..frames], &mut right[..frames], &ctx);
                blocks += 1;
            }
        }
    }
    assert_eq!(blocks, 400 * EffectKind::ALL.len() as u64);
    assert_eq!(common::audio_block_counts(), (0, 0));
}
