//! How long one block of a song takes with automation and without (docs/architecture.md
//! 21-7). Run in release, on a quiet PC:
//! `cargo test --release -p meloet-sound --test automation_speed -- --ignored --nocapture`
//!
//! Sixteen computed instruments with a compressor each, returns and master as in
//! `mixer_speed`; with lanes, every track has four: volume, pan, a parameter of its
//! instrument (as a plugin's would be: the engine works out the points, the computed
//! instrument passes them over) and the compressor's threshold. Each lane bends every
//! 120 ticks, over the four bars the song goes round.

mod support;

use std::time::Instant;

use meloet_engine::AutoLane;
use meloet_model::effects::{EffectKind, compressor};
use meloet_model::{AutoTarget, InsertId};
use support::mix::{Song, TRACKS};
use support::{BLOCK, SR};

const BLOCKS: usize = 20_000;

fn lanes(track: usize) -> Vec<AutoLane> {
    let line = |shift: u64| -> Vec<(u64, f32)> {
        (0..=64u64)
            .map(|i| {
                let v = ((i * 37 + shift + track as u64 * 11) % 100) as f32 / 100.0;
                (i * 120, 0.3 + v * 0.5)
            })
            .collect()
    };
    let lane = |target, points| AutoLane {
        target,
        step: false,
        points,
        slot: 0,
        effect: None,
    };
    vec![
        lane(AutoTarget::Volume, line(0)),
        lane(AutoTarget::Pan, line(13)),
        lane(AutoTarget::Instrument { param: 7 }, line(29)),
        AutoLane {
            effect: Some(EffectKind::Compressor),
            ..lane(
                AutoTarget::Insert {
                    insert: InsertId(1),
                    param: compressor::THRESHOLD,
                },
                line(41),
            )
        },
    ]
}

fn time_blocks(automated: bool) -> (f64, f64, f64, usize) {
    let mut song = Song::new(&[EffectKind::Compressor], true);
    if automated {
        for track in 0..TRACKS {
            assert!(
                song.parts
                    .handle
                    .set_automation(track, lanes(track))
                    .is_ok()
            );
        }
    }
    for _ in 0..1000 {
        song.block(BLOCK);
    }
    let mut times = Vec::with_capacity(BLOCKS);
    for _ in 0..BLOCKS {
        let began = Instant::now();
        song.block(BLOCK);
        times.push(began.elapsed().as_secs_f64() * 1.0e6);
    }
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / BLOCKS as f64;
    let slow = times.iter().filter(|t| **t > 1000.0).count();
    (mean, times[BLOCKS * 99 / 100], times[BLOCKS - 1], slow)
}

#[test]
#[ignore = "a measurement, not a check"]
fn time_per_block() {
    let block_us = BLOCK as f64 / f64::from(SR) * 1.0e6;
    println!("48 kHz, {BLOCK} frames ({block_us:.0} us), 16 tracks, {BLOCKS} blocks each");
    println!(
        "{:40} {:>8} {:>6} {:>8} {:>6} {:>8} {:>6} {:>7}",
        "", "mean us", "%", "p99 us", "%", "max us", "%", "> 1 ms"
    );
    for (name, automated) in [
        ("no lanes", false),
        ("4 lanes on every track", true),
        ("no lanes", false),
        ("4 lanes on every track", true),
    ] {
        let (mean, p99, max, slow) = time_blocks(automated);
        let pct = |us: f64| us / block_us * 100.0;
        println!(
            "{name:40} {mean:>8.1} {:>6.2} {p99:>8.1} {:>6.2} {max:>8.1} {:>6.2} {slow:>7}",
            pct(mean),
            pct(p99),
            pct(max)
        );
    }
}
