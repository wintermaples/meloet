//! The bands of E7 are worked on the analysis thread every 10 ms: once made, they take no
//! memory however many frames come. In its own test binary because it counts with the global
//! allocator.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use meloet_core::analysis::spectrum::{HOP_FRAMES, Spectrum};

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
fn the_bands_take_no_memory_while_frames_come() {
    let rate = 96_000;
    let frames: Vec<[f32; 2]> = (0..rate)
        .map(|n| {
            let t = f64::from(n) / f64::from(rate);
            let v = (0.5 * (std::f64::consts::TAU * 440.0 * t).sin()) as f32;
            [v, -v]
        })
        .collect();
    let mut spectrum = Spectrum::new(48_000);

    WATCHED.with(|w| w.set(true));
    spectrum.restart(rate);
    for piece in frames.chunks(480) {
        spectrum.push(piece);
        black_box(spectrum.bars());
    }
    spectrum.fall_for(Duration::from_millis(10));
    spectrum.push(&frames[..HOP_FRAMES]);
    WATCHED.with(|w| w.set(false));

    assert!(spectrum.ffts() > 90);
    assert_eq!(ALLOCS.load(Ordering::Relaxed), 0);
}
