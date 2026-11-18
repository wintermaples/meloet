//! How long one block takes. Run in release:
//! `cargo test --release -p meloet-sound --test effects_speed -- --ignored --nocapture`

mod support;

use std::time::Instant;

use meloet_engine::EffectContext;
use meloet_model::effects::EffectKind;
use support::*;

const BLOCKS: usize = 10_000;

fn time_blocks(kind: EffectKind, moving: bool) -> f64 {
    let mut fx = prepared(kind);
    let source_l = noise(0.25, BLOCK, 1);
    let source_r = noise(0.25, BLOCK, 2);
    let mut left = source_l.clone();
    let mut right = source_r.clone();
    let ctx = EffectContext { tempo_bpm: TEMPO };
    let params = kind.params();
    let began = Instant::now();
    for block in 0..BLOCKS {
        if moving {
            // Every parameter gets a new target every block, so every ramp is always running.
            for (index, spec) in params.iter().enumerate() {
                let t = ((block + index) % 7) as f32 / 6.0;
                fx.set_param(index as u32, spec.min + (spec.max - spec.min) * t);
            }
        }
        left.copy_from_slice(&source_l);
        right.copy_from_slice(&source_r);
        fx.process(&mut left, &mut right, &ctx);
        std::hint::black_box((&left, &right));
    }
    began.elapsed().as_secs_f64() * 1.0e6 / BLOCKS as f64
}

#[test]
#[ignore = "a measurement, not a check"]
fn time_per_block() {
    let block_us = BLOCK as f64 / f64::from(SR) * 1.0e6;
    println!("48 kHz, {BLOCK} frames ({block_us:.0} us), {BLOCKS} blocks each");
    println!(
        "{:12} {:>12} {:>8} {:>16} {:>8}",
        "", "steady us", "%", "all moving us", "%"
    );
    for kind in EffectKind::ALL {
        let steady = time_blocks(kind, false);
        let moving = time_blocks(kind, true);
        println!(
            "{:12} {:>12.2} {:>8.3} {:>16.2} {:>8.3}",
            kind.name(),
            steady,
            steady / block_us * 100.0,
            moving,
            moving / block_us * 100.0
        );
    }
}
