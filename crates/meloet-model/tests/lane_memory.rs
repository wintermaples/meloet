//! How much memory writing automation keeps in the undo history (docs/architecture.md 21-5).
//! In its own test binary so the counts are not mixed with other tests. `--nocapture` shows the
//! numbers.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

use meloet_model::automation::{self, AutoPoint, AutoTarget};
use meloet_model::{ChunkedVec, Doc, History, TrackId};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn live() -> isize {
    LIVE.load(Ordering::Relaxed)
}

const TRACK: TrackId = TrackId(1);
const VOLUME: AutoTarget = AutoTarget::Volume;
/// A point every 22 ticks: about 44 a second at 120 beats a minute.
const GAP: u64 = 22;

/// `count` points from `tick` on, far enough from a straight line that thinning keeps them all.
fn pass(tick: u64, count: usize) -> Vec<AutoPoint> {
    (0..count as u64)
        .map(|i| {
            let tick = tick + i * GAP;
            let noise = (tick.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as f32 / 16_777_216.0;
            AutoPoint {
                tick,
                value: if i % 2 == 0 { 0.2 } else { 0.6 } + noise * 0.2,
            }
        })
        .collect()
}

fn points(doc: &Doc) -> &ChunkedVec<AutoPoint> {
    &doc.tracks[0].lane(VOLUME).unwrap().points
}

fn with_lane() -> Doc {
    automation::add_lane(&Doc::new_default(), TRACK, VOLUME, false)
        .unwrap()
        .0
        .doc
}

#[test]
fn writing_keeps_the_chunks_that_changed() {
    assert_eq!(size_of::<AutoPoint>(), 16);
    two_seconds_on_a_lane_of_twenty_five_thousand_points();
    ten_minutes_of_writing();
}

fn two_seconds_on_a_lane_of_twenty_five_thousand_points() {
    let doc = with_lane();
    let doc = automation::write(&doc, TRACK, VOLUME, &[pass(0, 25_000)], 0.5)
        .unwrap()
        .0
        .doc;
    assert_eq!(points(&doc).len(), 25_001);
    for (name, tick) in [
        ("in the middle", 12_000 * GAP + 7),
        ("at the end", 25_010 * GAP),
    ] {
        let new = automation::write(&doc, TRACK, VOLUME, &[pass(tick, 90)], 0.5)
            .unwrap()
            .0
            .doc;
        let fresh = points(&new).bytes_not_in(points(&doc));
        println!(
            "90 points written {name} of 25 000: {fresh} B not shared with the lane before \
             (a copy: {} B)",
            points(&new).len() * 16
        );
        assert!(fresh < 24_000, "{fresh}");
    }
    let id = doc.tracks[0].lane(VOLUME).unwrap().id;
    let added = automation::add_point(&doc, id, 9_000 * GAP + 5, Some(0.5), 0.5).unwrap();
    let moved = automation::move_points(&doc, id, &[15_000, 15_001], 3, 0.1).unwrap();
    let deleted = automation::delete_points(&doc, id, &[20_000]).unwrap();
    for (name, new) in [
        ("added", &added.0.doc),
        ("moved", &moved.0.doc),
        ("deleted", &deleted.0.doc),
    ] {
        let fresh = points(new).bytes_not_in(points(&doc));
        println!("a point {name} in 25 000: {fresh} B not shared with the lane before");
        assert!(fresh < 24_000, "{fresh}");
    }
}

/// A fader written for 10 minutes and let go every 2 seconds, as measured in the app
/// (docs/measurements/2026-11-20-round6-step2-3.md 4): 282 steps, the lane grows to 25 000.
fn ten_minutes_of_writing() {
    let mut doc = with_lane();
    let mut h = History::new();
    let start = live();
    let (mut fresh, mut copies) = (0, 0);
    for step in 0..282u64 {
        let e = automation::write(&doc, TRACK, VOLUME, &[pass(1 + step * 1920, 87)], 0.5)
            .unwrap()
            .0;
        fresh += points(&e.doc).bytes_not_in(points(&doc));
        copies += points(&doc).len() * 16;
        h.apply(&mut doc, e);
    }
    let with = live();
    drop(h);
    let kept = with - live();
    println!(
        "282 writes, the lane grown to {} points: the history keeps {kept} B ({} B a step); \
         the lanes share all but {fresh} B; a copy of the points in each step would be \
         {copies} B; the song itself changed by {} B",
        points(&doc).len(),
        kept / 282,
        with - start - kept
    );
    assert!(points(&doc).len() >= 25_000);
    assert!(kept < 4 << 20, "{kept}");
}
