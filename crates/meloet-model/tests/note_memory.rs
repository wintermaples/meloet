//! How much memory 1000 note moves keep in the undo history, on a take of 10 000 notes. In its
//! own test binary so the counts are not mixed with other tests. `--nocapture` shows the numbers.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

use meloet_model::edit::{self, NewTake};
use meloet_model::{Doc, History, Note, TrackId, notes};

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

const NOTES: usize = 10_000;

#[test]
fn a_thousand_moves_on_a_take_of_ten_thousand_notes() {
    let take = NewTake {
        origin: 0,
        length: NOTES as u64 * 120,
        notes: (0..NOTES)
            .map(|i| Note {
                from: None,
                start: i as u64 * 120,
                length: 100,
                key: 36 + (i % 48) as u8,
                velocity: 90,
            })
            .collect(),
        controls: Vec::new(),
    };
    let (e, added) = edit::add_recording(&Doc::new_default(), TrackId(1), vec![take], 0).unwrap();
    let mut doc = e.doc;
    let mut h = History::new();
    let start = live();
    for i in 0..1000usize {
        let ticks = if i % 2 == 0 { 10 } else { -10 };
        let e = notes::move_notes(&doc, added.clip, &[NOTES / 2], ticks, 0, false).unwrap();
        h.apply(&mut doc, e.edit);
    }
    let with = live();
    drop(h);
    let kept = with - live();
    println!(
        "1000 moves of 1 note in {NOTES}: the history keeps {kept} B ({} B a step); the song \
         itself changed by {} B",
        kept / 1000,
        with - start - kept
    );
    // Each step keeps the whole old take: 24 B a note.
    assert_eq!(std::mem::size_of::<Note>(), 24);
    assert!(kept / 1000 < 260_000, "{kept}");
}
