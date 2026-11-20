//! Counting allocator: counts every allocation and every free made while the calling thread is
//! inside an audio block (`meloet_engine::rt`).
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

struct Counting;

// SAFETY: every call is passed on to `System` unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if meloet_engine::rt::in_audio_block() {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if meloet_engine::rt::in_audio_block() {
            FREES.fetch_add(1, Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if meloet_engine::rt::in_audio_block() {
            ALLOCS.fetch_add(1, Relaxed);
            FREES.fetch_add(1, Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// One tempo and one meter for the whole song.
pub fn flat_map(bpm_centi: u32, num: u8, den: u8) -> std::sync::Arc<meloet_model::TempoMap> {
    use meloet_model::{Meter, Tempo, TempoMap, Timeline};
    let tempo = Tempo::from_centi(bpm_centi).expect("a tempo of 30.00 to 300.00");
    let meter = Meter::new(num, den).expect("a meter");
    std::sync::Arc::new(TempoMap::new(&Timeline::new(tempo, meter)))
}

/// (allocations, frees) seen inside audio blocks, on any thread, since the process started.
pub fn audio_block_counts() -> (u64, u64) {
    (ALLOCS.load(Relaxed), FREES.load(Relaxed))
}
