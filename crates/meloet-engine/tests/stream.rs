//! The audio thread's side of a vocal track's ring takes no memory: blocks of every length,
//! the ring running dry, generations that change, a writer on another thread; and neither
//! does looking a frame up in the clip table.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use meloet_engine::{ClipRegion, ClipTable, StreamChunk, stream_ring, want_ring};

#[test]
fn reading_the_stream_and_the_clip_table_never_allocates() {
    let (mut writer, mut reader) = stream_ring(48_000);
    let (mut wants_tx, mut wants_rx) = want_ring(64);
    const GENERATIONS: u32 = 40;
    const FRAMES: u64 = 20_000;

    // The newest generation the other thread has put a run of into the ring.
    let fed = Arc::new(AtomicU32::new(0));
    let fed_by_feeder = fed.clone();

    // The read-ahead thread's side: follows the generation the engine says, in runs of
    // uneven length.
    let feeder = std::thread::spawn(move || {
        let data = vec![0.25f32; 2 * 4_096];
        let (mut generation, mut next, mut pushed) = (0u32, 0u64, 0u64);
        let sizes = [4_096u32, 1, 333, 2_048, 64];
        let mut turn = 0usize;
        while !writer.is_abandoned() {
            if let Ok(want) = wants_rx.pop() {
                generation = want.generation;
                next = want.frame;
            }
            let frames = sizes[turn % sizes.len()];
            turn += 1;
            let chunk = StreamChunk {
                generation,
                first_frame: next,
                frames,
            };
            if generation > 0 && writer.push(chunk, &data[..2 * frames as usize]) {
                next += u64::from(frames);
                pushed += u64::from(frames);
                fed_by_feeder.store(generation, Relaxed);
            } else {
                std::thread::yield_now();
            }
        }
        pushed
    });

    let table = ClipTable::new(
        (0..1_000u64)
            .map(|i| ClipRegion {
                start: i * 1_000,
                end: i * 1_000 + 900,
                fade_in: 100,
                fade_out: 100,
                gain: 0.5,
            })
            .collect(),
    )
    .unwrap();

    let mut out = vec![0.0f32; 2 * 2_048];
    let sizes = [256usize, 64, 1_024, 37, 2_048, 1, 512];
    let (mut real, mut missing, mut calls) = (0u64, 0u64, 0usize);
    let mut loudest = 0.0f32;
    {
        // Before the engine has said where it is, nothing is in the ring.
        let _rt = meloet_engine::rt::enter();
        let found = reader.read(0, 0, &mut out[..2 * 256]);
        assert_eq!((found.frames, found.missing), (0, 256));
        missing += u64::from(found.missing);
    }
    for generation in 1..=GENERATIONS {
        let mut frame = u64::from(generation) * 7;
        {
            let _rt = meloet_engine::rt::enter();
            let _ = wants_tx.push(meloet_engine::StreamWant {
                track: 0,
                generation,
                map_gen: 1,
                anchor_sec: 0.0,
                frame,
                loop_ticks: None,
            });
        }
        // Every other generation the engine is ahead of the thread, as after a jump while
        // playing; the others it finds the audio waiting, as after a jump while stopped.
        // Reading while it waits lets what is left of the old generation go, so that the
        // thread has room.
        while generation % 2 == 0 && fed.load(Relaxed) != generation {
            let _rt = meloet_engine::rt::enter();
            missing += u64::from(reader.read(generation, frame, &mut out[..2]).missing);
            std::thread::yield_now();
        }
        let end = frame + FRAMES;
        while frame < end {
            let frames = sizes[calls % sizes.len()];
            calls += 1;
            {
                let _rt = meloet_engine::rt::enter();
                let found = reader.read(generation, frame, &mut out[..2 * frames]);
                real += u64::from(found.frames);
                missing += u64::from(found.missing);
                for (i, pair) in out[..2 * frames].as_chunks::<2>().0.iter().enumerate() {
                    let gain = table.gain_at(frame + i as u64);
                    loudest = loudest.max((pair[0] * gain).abs());
                }
            }
            frame += frames as u64;
            std::thread::yield_now();
        }
    }
    drop(reader);
    let pushed = feeder.join().unwrap();

    assert!(real > 0 && real <= pushed, "{real} of {pushed}");
    assert!(missing > 0, "the ring never ran dry");
    assert!(loudest > 0.12 && loudest <= 0.125, "{loudest}");
    assert_eq!(common::audio_block_counts(), (0, 0));
}
