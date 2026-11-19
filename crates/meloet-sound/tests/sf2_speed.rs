//! How long one block of the player takes with the parts a SoundFont uses. Run in release:
//! `cargo test --release -p meloet-sound --test sf2_speed -- --ignored --nocapture`

mod support;

use std::time::Instant;

use meloet_engine::Instrument;
use support::pack::on;
use support::sf2::*;

const RATE: f32 = 48_000.0;
const BLOCK: usize = 256;
const BLOCKS: usize = 10_000;

/// `voices` looped voices of a 44.1 kHz file through `zone`, 64 keys with as many layers as
/// that takes.
fn time_blocks(name: &str, voices: usize, mut zone: Zone) -> f64 {
    let mut sample = Smp::new("tone", sine(441.0, 44_100, 44_100, 0.01), 44_100, 60);
    sample.looped = (4410, 39_690);
    zone.extend([(SAMPLE_MODES, 1), (SAMPLE_ID, 0)]);
    let font = Font::simple(vec![sample], vec![zone; voices / 64]);
    let file = TempFile::new(name, &font.bytes());
    let mut sampler = file.sampler(RATE, BLOCK);
    for key in 0..64 {
        sampler.event(on(30 + key, 100));
    }
    let (mut left, mut right) = (vec![0.0_f32; BLOCK], vec![0.0_f32; BLOCK]);
    let began = Instant::now();
    for _ in 0..BLOCKS {
        sampler.render(&mut left, &mut right);
        std::hint::black_box((&left, &right));
    }
    assert_eq!(sampler.active_voices(), voices);
    began.elapsed().as_secs_f64() * 1.0e6 / BLOCKS as f64
}

#[test]
#[ignore = "a measurement, not a check"]
fn time_per_block() {
    let block_us = BLOCK as f64 / f64::from(RATE) * 1.0e6;
    println!("48 kHz, {BLOCK} frames ({block_us:.0} us), {BLOCKS} blocks each, mono files");
    let still = vec![(FILTER_FC, 9000), (FILTER_Q, 60)];
    // Everything moving for the whole run: the envelope never settles and both LFOs turn.
    let moving = vec![
        (FILTER_FC, 8000),
        (FILTER_Q, 60),
        (MOD_ENV_TO_FILTER, 2400),
        (DECAY_MOD_ENV, timecents(90.0)),
        (SUSTAIN_MOD_ENV, 1000),
        (MOD_LFO_TO_FILTER, 600),
        (MOD_LFO_TO_VOLUME, 20),
        (MOD_LFO_TO_PITCH, 10),
        (VIB_LFO_TO_PITCH, 20),
        (FREQ_VIB_LFO, -500),
    ];
    for (name, voices, zone) in [
        ("envelope only", 64, Vec::new()),
        ("filter, not moving", 64, still.clone()),
        ("filter, LFOs, envelope moving", 64, moving.clone()),
        ("envelope only", 128, Vec::new()),
        ("filter, LFOs, envelope moving", 128, moving),
    ] {
        let us = time_blocks(&format!("speed-{voices}-{}", name.len()), voices, zone);
        println!(
            "{voices:>3} voices, {name:30} {us:>8.2} us {:>7.3} %",
            us / block_us * 100.0
        );
    }
}
