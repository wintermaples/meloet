//! The tempo map is read on the audio thread: none of its functions may allocate. In its own
//! test binary because it counts with the global allocator.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use meloet_model::{Meter, MeterChange, Tempo, TempoMap, TempoPoint, Timeline};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static WATCHED: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCHED.try_with(Cell::get).unwrap_or(false) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn no_function_of_the_map_allocates() {
    let tempo = |centi| Tempo::from_centi(centi).unwrap();
    let timeline = Timeline {
        tempo: tempo(9600),
        meter: Meter::FOUR_FOUR,
        head_ramp: true,
        tempo_points: (1..200u64)
            .map(|i| TempoPoint {
                tick: i * 700,
                tempo: tempo(6000 + (i as u32 * 977) % 12000),
                ramp: i % 3 != 0,
            })
            .collect(),
        meter_changes: (1..40u32)
            .map(|i| MeterChange {
                bar: i * 3,
                meter: Meter::new((i % 12 + 1) as u8, Meter::DENS[i as usize % 4]).unwrap(),
            })
            .collect(),
    };
    let map = TempoMap::new(&timeline);
    assert!(map.piece_count() > 200);

    WATCHED.with(|w| w.set(true));
    let mut sum = 0.0;
    for i in -50..3000i64 {
        let tick = i as f64 * 61.7;
        let seconds = map.seconds_at(black_box(tick));
        sum += seconds;
        sum += map.tick_at(black_box(seconds));
        sum += map.tempo_at(tick) + map.quarter_bpm_at(tick);
        sum += map.next_beat(tick).tick as f64;
        sum += map.frame_of(tick, 0.25, 48_000.0) as f64;
        let whole = tick.max(0.0) as u64;
        sum += map.meter_at(whole).bar_tick as f64;
        sum += map.bar_beat_tick(whole).bar as f64;
        sum += map.tick_of_bar(whole / 100) as f64;
    }
    WATCHED.with(|w| w.set(false));

    assert!(black_box(sum).is_finite());
    assert_eq!(ALLOCS.load(Ordering::Relaxed), 0);
}
