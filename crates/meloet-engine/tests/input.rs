//! The input callback takes no memory: blocks of every length, both rings fed, the record
//! ring running full, with a reader on another thread.

mod common;

use meloet_engine::{InputEnds, input_ends};

#[test]
fn the_input_callback_never_allocates() {
    let InputEnds {
        mut tap,
        mut monitor,
        mut record,
        meters,
    } = input_ends(2, 48_000);
    meters.set_monitoring(true);
    meters.set_recording(true);

    // The disk writer's side: reads for a while, then stalls so that the ring runs full.
    let reader = std::thread::spawn(move || {
        let mut into = vec![0.0f32; 2 * 4_096];
        let mut frames = 0u64;
        while frames < 480_000 {
            match record.pop(&mut into) {
                Some(block) => frames += u64::from(block.frames),
                None => std::thread::yield_now(),
            }
        }
        frames
    });

    let data: Vec<f32> = (0..2 * 4_096).map(|i| (i as f32 * 0.01).sin()).collect();
    let mut into = vec![0.0f32; 2 * 4_096];
    let sizes = [256usize, 0, 1024, 37, 4_096, 64, 512, 1];
    let (mut given, mut calls) = (0u64, 0usize);
    let mut t_us = 1_000_000u64;
    // Thirty seconds of input: far more than the record ring holds once its reader is done.
    while given < 30 * 48_000 {
        let frames = sizes[calls % sizes.len()];
        calls += 1;
        tap.block(&data[..2 * frames], t_us, t_us + 400);
        given += frames as u64;
        t_us += frames as u64 * 1_000_000 / 48_000;
        // The output's side, once a block as it would.
        let _rt = meloet_engine::rt::enter();
        while monitor.pop(&mut into).is_some() {}
    }
    assert!(reader.join().unwrap() >= 480_000);

    assert_eq!(meters.frames(), given);
    assert!(meters.record_dropped() > 0, "the ring ran full");
    assert_eq!(meters.monitor_dropped(), 0);
    assert_eq!(common::audio_block_counts(), (0, 0));
}
