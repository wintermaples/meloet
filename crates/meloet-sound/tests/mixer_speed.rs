//! How long one block of a song takes with the mixer and without. Run in release, on a quiet
//! PC: `cargo test --release -p meloet-sound --test mixer_speed -- --ignored --nocapture`

mod support;

use std::time::Instant;

use meloet_model::effects::EffectKind;
use support::mix::Song;
use support::{BLOCK, SR};

const BLOCKS: usize = 20_000;

/// Mean, 99th percentile and longest block in microseconds, and how many blocks took over
/// 1 ms (on a PC that is not quiet, those are the blocks another program got in the way of).
fn time_blocks(inserts: &[EffectKind], mixer: bool) -> (f64, f64, f64, usize) {
    let mut song = Song::new(inserts, mixer);
    // Past the fades of the effects that just went in, and into the song.
    for _ in 0..1000 {
        song.block(BLOCK);
    }
    let mut times = Vec::with_capacity(BLOCKS);
    let mut loudest = 0.0f32;
    for _ in 0..BLOCKS {
        let began = Instant::now();
        let out = song.block(BLOCK);
        times.push(began.elapsed().as_secs_f64() * 1.0e6);
        loudest = loudest.max(out[0].abs());
    }
    assert!(loudest > 0.0, "the song sounds");
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / BLOCKS as f64;
    let slow = times.iter().filter(|t| **t > 1000.0).count();
    (mean, times[BLOCKS * 99 / 100], times[BLOCKS - 1], slow)
}

#[test]
#[ignore = "a measurement, not a check"]
fn time_per_block() {
    use EffectKind as K;
    let block_us = BLOCK as f64 / f64::from(SR) * 1.0e6;
    println!("48 kHz, {BLOCK} frames ({block_us:.0} us), 16 tracks, {BLOCKS} blocks each");
    println!(
        "{:48} {:>8} {:>6} {:>8} {:>6} {:>8} {:>6} {:>7}",
        "", "mean us", "%", "p99 us", "%", "max us", "%", "> 1 ms"
    );
    let cases: [(&str, &[EffectKind], bool); 4] = [
        (
            "EQ + compressor on every track, returns, master",
            &[K::Eq4, K::Compressor],
            true,
        ),
        ("no mixer (fader and pan only)", &[], false),
        ("returns and master, no inserts on tracks", &[], true),
        (
            "EQ + compressor on every track, returns, master",
            &[K::Eq4, K::Compressor],
            true,
        ),
    ];
    for (name, inserts, mixer) in cases {
        let (mean, p99, max, slow) = time_blocks(inserts, mixer);
        let pct = |us: f64| us / block_us * 100.0;
        println!(
            "{name:48} {mean:>8.1} {:>6.2} {p99:>8.1} {:>6.2} {max:>8.1} {:>6.2} {slow:>7}",
            pct(mean),
            pct(p99),
            pct(max)
        );
    }
}
