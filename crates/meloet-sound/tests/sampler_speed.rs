//! How long one block of the sample player takes. Run in release:
//! `cargo test --release -p meloet-sound --test sampler_speed -- --ignored --nocapture`

mod support;

use std::time::Instant;

use meloet_engine::{Instrument, InstrumentEvent};
use support::pack::{TestPack, on, region, tone};

const RATE: f32 = 48_000.0;
const BLOCK: usize = 256;
const BLOCKS: usize = 10_000;
const VOICES: usize = 64;

/// 64 looped voices of a 44.1 kHz file, each on its own key.
fn time_blocks(name: &str, stereo: bool, bending: bool) -> f64 {
    let pack = TestPack::new(name);
    let wave = tone(441.0, 44_100, 44_100, 0.01);
    let channels: &[&[f32]] = if stereo { &[&wave, &wave] } else { &[&wave] };
    pack.wav("tone", 44_100, channels);
    let looped = "\"loopStart\": 4410, \"loopEnd\": 39690";
    pack.instrument("\"polyphony\": 64", &[region("tone", 0, 127, 60, looped)]);
    let mut sampler = pack.sampler(RATE, BLOCK);
    for key in 0..VOICES {
        sampler.event(on(30 + key as u8, 100));
    }
    let (mut left, mut right) = (vec![0.0_f32; BLOCK], vec![0.0_f32; BLOCK]);
    let began = Instant::now();
    for block in 0..BLOCKS {
        if bending {
            let value = ((block % 64) as i16 - 32) * 250;
            sampler.event(InstrumentEvent::PitchBend { value });
        }
        sampler.render(&mut left, &mut right);
        std::hint::black_box((&left, &right));
    }
    assert_eq!(sampler.active_voices(), VOICES);
    began.elapsed().as_secs_f64() * 1.0e6 / BLOCKS as f64
}

#[test]
#[ignore = "a measurement, not a check"]
fn time_per_block() {
    let block_us = BLOCK as f64 / f64::from(RATE) * 1.0e6;
    println!("48 kHz, {BLOCK} frames ({block_us:.0} us), {VOICES} voices, {BLOCKS} blocks each");
    for (name, stereo, bending) in [
        ("mono", false, false),
        ("stereo", true, false),
        ("stereo, bend moving", true, true),
    ] {
        let us = time_blocks(&format!("speed-{stereo}-{bending}"), stereo, bending);
        println!("{name:22} {us:>8.2} us {:>7.3} %", us / block_us * 100.0);
    }
}
