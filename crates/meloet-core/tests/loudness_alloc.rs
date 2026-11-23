//! The loudness meters of M3 are fed on the analysis thread every 10 ms for as long as meloet
//! runs: once made, they take no memory however long the song plays (30 minutes here), and
//! reading them takes none either. In its own test binary because it counts with the global
//! allocator.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use meloet_core::analysis::loudness::Loudness;

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static WATCHED: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: every call is passed on to `System` unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCHED.try_with(Cell::get).unwrap_or(false) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if WATCHED.try_with(Cell::get).unwrap_or(false) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn thirty_minutes_of_play_take_no_memory() {
    let rate = 48_000u32;
    // One second of something like a song, heard again and again.
    let second: Vec<[f32; 2]> = (0..rate)
        .map(|n| {
            let t = f64::from(n) / f64::from(rate);
            let level = 0.2 + 0.15 * (std::f64::consts::TAU * 0.5 * t).sin();
            let l = level * (std::f64::consts::TAU * 220.0 * t).sin();
            let r = level * (std::f64::consts::TAU * 330.0 * t).sin();
            [l as f32, r as f32]
        })
        .collect();
    let mut meters = Loudness::new(rate).expect("48 kHz");

    WATCHED.with(|w| w.set(true));
    meters.start_play();
    for seconds in 0..30 * 60 {
        for (i, piece) in second.chunks(480).enumerate() {
            meters.feed(piece, seconds % 600 != 599);
            if i == 0 {
                black_box((
                    meters.short_term(),
                    meters.integrated(),
                    meters.true_peak_db(),
                ));
            }
        }
    }
    meters.stop_hearing();
    meters.start_play();
    meters.restart();
    WATCHED.with(|w| w.set(false));

    assert_eq!(ALLOCS.load(Ordering::Relaxed), 0);
}
