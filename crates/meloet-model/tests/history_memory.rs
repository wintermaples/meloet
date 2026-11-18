//! How much memory 1000 undo steps keep. Run with `--nocapture` to see the numbers.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};

use meloet_model::edit::{self, NewTake};
use meloet_model::{Doc, History, InstrumentRef, Note, Tempo, TrackColor, TrackId, song::Track};

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

const TRACKS: u32 = 8;

fn eight_tracks() -> Doc {
    let mut doc = Doc::new_default();
    for i in 2..=TRACKS {
        doc.tracks.push(Arc::new(Track {
            id: TrackId(i),
            name: format!("トラック {i}"),
            color: TrackColor::for_index(i as usize - 1),
            instrument: InstrumentRef::SINE,
            channel: Default::default(),
            sends: [meloet_model::Gain::SILENT; 2],
            muted: false,
            clips: Vec::new(),
        }));
    }
    doc
}

fn take(origin: u64, notes: usize) -> NewTake {
    NewTake {
        origin,
        length: 1920,
        notes: (0..notes)
            .map(|i| Note {
                from: None,
                start: i as u64 * 10,
                length: 5,
                key: 60,
                velocity: 90,
            })
            .collect(),
        controls: Vec::new(),
    }
}

/// Bytes the history alone keeps: measured with it, then without it.
fn history_bytes(doc: Doc, steps: impl Fn(&mut History, &mut Doc, usize)) -> isize {
    let mut doc = doc;
    let mut h = History::new();
    for i in 0..1000 {
        steps(&mut h, &mut doc, i);
    }
    let with = live();
    drop(h);
    let without = live();
    drop(doc);
    with - without
}

#[test]
fn a_thousand_steps() {
    // 1000 recordings, each a new clip of 20 notes, spread over 8 tracks.
    let record = history_bytes(eight_tracks(), |h, doc, i| {
        let track = TrackId(i as u32 % TRACKS + 1);
        let origin = (i as u64 / u64::from(TRACKS)) * 1920;
        let (e, _) = edit::add_recording(doc, track, vec![take(origin, 20)], 0).unwrap();
        h.apply(doc, e);
    });

    // 1000 tempo changes on a song with 8 tracks of 50 clips.
    let mut full = eight_tracks();
    for i in 0..50 * TRACKS as usize {
        let track = TrackId(i as u32 % TRACKS + 1);
        let origin = (i as u64 / u64::from(TRACKS)) * 1920;
        full = edit::add_recording(&full, track, vec![take(origin, 20)], 0)
            .unwrap()
            .0
            .doc;
    }
    let tempo = history_bytes(full.clone(), |h, doc, i| {
        let t = Tempo::from_centi(6_000 + (i as u32 % 2) * 100).unwrap();
        let e = edit::set_tempo(doc, t).unwrap();
        h.apply(doc, e);
    });

    // 1000 take switches on that song: each copies one track's list of 50 clips.
    let ids: Vec<_> = full.tracks[0].clips.iter().map(|c| c.id).collect();
    let full = edit::add_recording(&full, TrackId(1), vec![take(0, 20)], 0)
        .unwrap()
        .0
        .doc;
    let switch = history_bytes(full, |h, doc, i| {
        let to = if i % 2 == 0 { 0 } else { 1 };
        let e = edit::switch_take(doc, ids[0], to).unwrap();
        h.apply(doc, e);
    });

    println!("1000 steps keep: record {record} B, tempo {tempo} B, switch take {switch} B");
    // Generous bounds, to notice if steps stop sharing.
    assert!(record < 2_000_000, "{record}");
    assert!(tempo < 1_000_000, "{tempo}");
    assert!(switch < 2_000_000, "{switch}");
}
