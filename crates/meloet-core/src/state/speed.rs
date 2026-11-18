//! How long a big song takes to view, to compare and to edit. Numbers only mean something in a
//! release build: `cargo test --release -p meloet-core speed -- --ignored --nocapture`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_model::edit::{NewTake, add_recording};
use meloet_model::song::Track;
use meloet_model::{InstrumentRef, TrackColor, TrackId};

use super::*;
use crate::play::tests::{cmd, ready};

const TRACKS: u32 = 16;
const CLIPS: u64 = 100;
const BIG: usize = 10_000;

/// 16 tracks of 100 clips of 20 notes; the first clip of track 1 has a take of 10 000 notes.
fn big_song() -> Song {
    let mut song = Song::new_default();
    for i in 2..=TRACKS {
        song.doc.tracks.push(Arc::new(Track {
            id: TrackId(i),
            name: format!("トラック {i}"),
            color: TrackColor::for_index(i as usize - 1),
            instrument: InstrumentRef::SINE,
            gain_db: 0.0,
            muted: false,
            clips: Vec::new(),
        }));
    }
    let take = |origin: u64, notes: usize, step: u64| NewTake {
        origin,
        length: notes as u64 * step,
        notes: (0..notes)
            .map(|i| Note {
                from: None,
                start: i as u64 * step,
                length: step / 2,
                key: 36 + (i % 48) as u8,
                velocity: 90,
            })
            .collect(),
        controls: Vec::new(),
    };
    let mut doc = song.doc;
    doc = add_recording(&doc, TrackId(1), vec![take(0, BIG, 12)], 0)
        .unwrap()
        .0
        .doc;
    let first_end = BIG as u64 * 12;
    for t in 1..=TRACKS {
        for c in 0..CLIPS {
            if t == 1 && c == 0 {
                continue;
            }
            let origin = first_end + c * 1920;
            doc = add_recording(&doc, TrackId(t), vec![take(origin, 20, 90)], 0)
                .unwrap()
                .0
                .doc;
        }
    }
    song.doc = doc;
    song
}

fn time<T>(runs: u32, mut f: impl FnMut() -> T) -> Duration {
    let start = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(f());
    }
    start.elapsed() / runs
}

#[test]
#[ignore = "a measurement; run in release with --nocapture"]
fn a_big_song() {
    let m = &mut ready();
    m.play.replace_song(big_song(), false, &mut Vec::new());
    let first = m.view().song.tracks[0].clips[0].id;
    cmd(m, Command::SelectClip { clip: Some(first) });
    let clips: usize = m.view().song.tracks.iter().map(|t| t.clips.len()).sum();
    assert_eq!(clips, (TRACKS as u64 * CLIPS) as usize);

    let view = time(200, || m.view());
    let a = m.view();
    let b = m.view();
    let same = time(200, || AppState::changes_from(&a, &b));
    let song_only = time(200, || a.song == b.song);

    let take = m.view().piano_roll.take.unwrap();
    // One intent, then what the core thread does with it: the view and the patch.
    let mut moves = Vec::new();
    for i in 0..50 {
        let before = m.view();
        let start = Instant::now();
        let effects = cmd(
            m,
            Command::MoveNotes {
                take: if i == 0 {
                    take
                } else {
                    m.view().piano_roll.take.unwrap()
                },
                notes: vec![(BIG / 2) as u32],
                ticks: if i % 2 == 0 { 3 } else { -3 },
                keys: 0,
                copy: false,
            },
        );
        let handled = start.elapsed();
        let after = m.view();
        let changes = AppState::changes_from(&before, &after);
        moves.push((handled, start.elapsed()));
        assert!(changes.len() >= 2, "the song and the piano roll");
        std::hint::black_box(effects);
    }
    let median = |mut v: Vec<Duration>| {
        v.sort();
        v[v.len() / 2]
    };
    let handled = median(moves.iter().map(|m| m.0).collect());
    let patched = median(moves.iter().map(|m| m.1).collect());

    let id = m.view().piano_roll.take.unwrap();
    let binary = time(50, || m.take_notes(id).unwrap());
    let size = m.take_notes(id).unwrap().len();
    let doc = m.play.song().doc.clone();
    let events = time(50, || {
        meloet_model::playback::track_events(&doc, TrackId(1))
    });

    println!(
        "{TRACKS} tracks × {CLIPS} clips, one take of {BIG} notes:\n\
         view {view:?}, compare two equal views {same:?} (song section alone {song_only:?})\n\
         move 1 note (median of 50): handled {handled:?}, with view and patch {patched:?}\n\
         take_notes of {BIG} notes: {binary:?}, {size} B\n\
         events of track 1 ({BIG} + 99 × 20 notes): {events:?}"
    );
}
