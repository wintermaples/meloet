use std::sync::Arc;

use super::*;
use crate::edit::{self, NewTake};
use crate::id_book::{IdBook, held};
use crate::song::Note;
use crate::time::TICKS_PER_QUARTER;

const T1: TrackId = TrackId(1);
const Q: u64 = TICKS_PER_QUARTER;

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

fn set_tempo(h: &mut History, doc: &mut Doc, centi: u32) {
    let e = edit::set_tempo(doc, tempo(centi)).unwrap();
    h.apply(doc, e);
}

fn take(origin: u64, length: u64, key: u8) -> NewTake {
    NewTake {
        origin,
        length,
        notes: vec![Note {
            from: None,
            start: 0,
            length: 1,
            key,
            velocity: 64,
        }],
        controls: Vec::new(),
    }
}

#[test]
fn undo_and_redo_walk_the_steps() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    let start = doc.clone();
    assert_eq!(h.undo(&mut doc), None);
    assert_eq!(h.redo(&mut doc), None);

    set_tempo(&mut h, &mut doc, 10_000);
    let a = doc.clone();
    let e = edit::set_meter(&doc, Meter::new(3, 4).unwrap()).unwrap();
    h.apply(&mut doc, e);
    let b = doc.clone();
    assert_eq!((h.undo_count(), h.redo_count()), (2, 0));

    assert!(matches!(h.undo(&mut doc), Some(StepKind::Meter { .. })));
    assert_eq!(doc, a);
    assert!(matches!(h.undo(&mut doc), Some(StepKind::Tempo { .. })));
    assert_eq!(doc, start);
    assert_eq!((h.undo_count(), h.redo_count()), (0, 2));
    assert_eq!(h.steps().count(), 2);
    assert!(matches!(h.steps().next(), Some(StepKind::Tempo { .. })));

    assert!(matches!(h.redo(&mut doc), Some(StepKind::Tempo { .. })));
    assert!(matches!(h.redo(&mut doc), Some(StepKind::Meter { .. })));
    assert_eq!(doc, b);
}

#[test]
fn a_new_step_drops_what_could_be_redone() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    set_tempo(&mut h, &mut doc, 10_000);
    set_tempo(&mut h, &mut doc, 11_000);
    set_tempo(&mut h, &mut doc, 12_000);
    h.undo(&mut doc);
    h.undo(&mut doc);
    assert_eq!((h.undo_count(), h.redo_count()), (1, 2));
    set_tempo(&mut h, &mut doc, 13_000);
    assert_eq!((h.undo_count(), h.redo_count()), (2, 0));
    assert_eq!(h.redo(&mut doc), None);
    let tempos: Vec<_> = h
        .steps()
        .map(|s| match s {
            StepKind::Tempo { to, .. } => to.centi(),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(tempos, [10_000, 13_000]);
}

#[test]
fn saved_mark() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    assert!(!h.has_unsaved_changes(), "a new song counts as saved");
    set_tempo(&mut h, &mut doc, 10_000);
    assert!(h.has_unsaved_changes());
    h.mark_saved();
    assert!(!h.has_unsaved_changes());
    set_tempo(&mut h, &mut doc, 11_000);
    assert!(h.has_unsaved_changes());
    h.undo(&mut doc);
    assert!(!h.has_unsaved_changes(), "back where it was saved");
    h.undo(&mut doc);
    assert!(h.has_unsaved_changes());
    h.redo(&mut doc);
    assert!(!h.has_unsaved_changes());

    // The same tempo again is still a step: not the saved Doc.
    h.undo(&mut doc);
    set_tempo(&mut h, &mut doc, 10_000);
    assert!(h.has_unsaved_changes());
}

#[test]
fn undo_keeps_untouched_parts_shared() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    let (e, a) = edit::add_recording(&doc, T1, vec![take(0, 2 * Q, 60)], 0).unwrap();
    h.apply(&mut doc, e);
    let clip_a = doc.tracks[0].clips[0].clone();
    let (e, _) = edit::add_recording(&doc, T1, vec![take(5000, 2 * Q, 62)], 0).unwrap();
    h.apply(&mut doc, e);
    assert!(Arc::ptr_eq(&doc.tracks[0].clips[0], &clip_a));
    h.undo(&mut doc);
    assert!(Arc::ptr_eq(&doc.tracks[0].clips[0], &clip_a));
    h.redo(&mut doc);
    assert!(Arc::ptr_eq(&doc.tracks[0].clips[0], &clip_a));
    assert_eq!(doc.clip(a.clip).map(|(_, c)| c.takes.len()), Some(1));
}

#[test]
fn an_amended_step_stays_one_step() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    let first = doc.clone();
    assert!(!h.amend(&mut doc, first.clone()), "no step to amend");
    let (e, id) = crate::timeline::add_marker(&doc, 0, "マーカー 1").unwrap();
    h.apply(&mut doc, e);
    h.mark_saved();
    let renamed = crate::timeline::rename_marker(&doc, id, "サビ")
        .unwrap()
        .doc;
    assert!(h.amend(&mut doc, renamed));
    assert_eq!(doc.marker(id).unwrap().name, "サビ");
    assert_eq!((h.undo_count(), h.redo_count()), (1, 0));
    assert!(h.has_unsaved_changes(), "what was saved had the other name");
    assert_eq!(h.undo(&mut doc), Some(StepKind::AddMarker { marker: id }));
    assert_eq!(held(&doc), held(&first));
    // Not while a step could be redone: the step it would change is not the last one.
    let now = doc.clone();
    assert!(!h.amend(&mut doc, now));
    h.redo(&mut doc);
    assert_eq!(doc.marker(id).unwrap().name, "サビ");
    assert!(doc.next_id > id.0);
}

#[test]
fn ids_undone_are_not_given_again() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    let (e, a) = edit::add_recording(&doc, T1, vec![take(0, 2 * Q, 60)], 0).unwrap();
    h.apply(&mut doc, e);
    let (e, b) = edit::add_recording(&doc, T1, vec![take(5000, 2 * Q, 62)], 0).unwrap();
    h.apply(&mut doc, e);
    let given = doc.next_id;
    let b_take = doc.clip(b.clip).unwrap().1.keys()[0].take.id;
    h.undo(&mut doc);
    assert_eq!(doc.next_id, given, "undo keeps the ids given out");
    assert!(doc.clip(b.clip).is_none());
    let (e, c) = edit::add_recording(&doc, T1, vec![take(5000, 2 * Q, 64)], 0).unwrap();
    h.apply(&mut doc, e);
    assert!(c.clip.0 >= given);
    assert!(doc.clip(c.clip).unwrap().1.keys()[0].take.id.0 >= given);
    assert_ne!(doc.clip(c.clip).unwrap().1.keys()[0].take.id, b_take);
    // All the way back, then a new step: still past every id given out.
    h.undo(&mut doc);
    h.undo(&mut doc);
    assert!(doc.clip(a.clip).is_none());
    let past = doc.next_id;
    assert!(past > c.clip.0);
    let (e, d) = edit::add_recording(&doc, T1, vec![take(0, 2 * Q, 65)], 0).unwrap();
    h.apply(&mut doc, e);
    assert!(d.clip.0 >= past);
}

/// xorshift64*, so the test needs nothing outside std.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random change, or `None` when the one drawn cannot be made.
fn random_edit(rng: &mut Rng, doc: &Doc) -> Option<edit::Edit> {
    let clips = &doc.tracks[0].clips;
    match rng.below(5) {
        0 => edit::set_tempo(doc, tempo(3_000 + rng.below(27_001) as u32)).ok(),
        1 => {
            let num = 1 + rng.below(12) as u8;
            let den = Meter::DENS[rng.below(4) as usize];
            edit::set_meter(doc, Meter::new(num, den).unwrap()).ok()
        }
        2 => {
            let n = 1 + rng.below(3) as usize;
            let origin = rng.below(20) * Q;
            let takes = (0..n)
                .map(|_| take(origin, Q + rng.below(4) * Q, rng.below(128) as u8))
                .collect();
            edit::add_recording(doc, T1, takes, rng.below(n as u64) as usize)
                .ok()
                .map(|(e, _)| e)
        }
        3 => {
            let c = clips.get(rng.below(clips.len().max(1) as u64) as usize)?;
            edit::switch_take(doc, c.id, rng.below(c.takes.len() as u64) as usize).ok()
        }
        _ => {
            let c = clips.get(rng.below(clips.len().max(1) as u64) as usize)?;
            edit::delete_clips(doc, &[c.id], false).ok()
        }
    }
}

#[test]
fn random_walks_come_back_to_the_same_docs() {
    let mut applied = [0usize; 5];
    for seed in 1..=40u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut h = History::new();
        let mut doc = Doc::new_default();
        let mut book = IdBook::new(&doc);
        // What the history should be: every Doc in order, with a mark for each.
        let mut line = vec![(doc.clone(), 0u32)];
        let mut at = 0usize;
        let mut marks = 0u32;
        let mut saved = 0u32;
        for round in 0..400 {
            match rng.below(10) {
                0..=3 => {
                    if let Some(e) = random_edit(&mut rng, &doc) {
                        applied[kind_index(&e.step)] += 1;
                        let before = doc.clone();
                        h.apply(&mut doc, e);
                        book.saw(&doc, Some(&before))
                            .unwrap_or_else(|e| panic!("seed {seed}, round {round}: {e}"));
                        marks += 1;
                        line.truncate(at + 1);
                        line.push((doc.clone(), marks));
                        at += 1;
                    }
                }
                4..=6 => {
                    let did = h.undo(&mut doc).is_some();
                    assert_eq!(did, at > 0);
                    at = at.saturating_sub(1);
                }
                7..=8 => {
                    let did = h.redo(&mut doc).is_some();
                    assert_eq!(did, at + 1 < line.len());
                    at = (at + 1).min(line.len() - 1);
                }
                _ => {
                    h.mark_saved();
                    saved = line[at].1;
                }
            }
            let ctx = format!("seed {seed}, round {round}");
            book.saw(&doc, None)
                .unwrap_or_else(|e| panic!("{ctx}: {e}"));
            assert_eq!(held(&doc), held(&line[at].0), "{ctx}");
            assert_eq!(h.undo_count(), at, "{ctx}");
            assert_eq!(h.redo_count(), line.len() - 1 - at, "{ctx}");
            assert_eq!(h.has_unsaved_changes(), saved != line[at].1, "{ctx}");
        }
        // All the way back and all the way forward.
        while h.undo(&mut doc).is_some() {}
        assert_eq!(held(&doc), held(&line[0].0));
        while h.redo(&mut doc).is_some() {}
        assert_eq!(held(&doc), held(&line.last().unwrap().0));
    }
    assert!(applied.iter().all(|&n| n > 100), "{applied:?}");
}

fn kind_index(kind: &StepKind) -> usize {
    match kind {
        StepKind::Record { .. } => 0,
        StepKind::SwitchTake { .. } => 1,
        StepKind::DeleteClips { .. } => 2,
        StepKind::Tempo { .. } => 3,
        StepKind::Meter { .. } => 4,
        StepKind::Recovered | StepKind::DiscardRecovered => unreachable!("no edit makes it"),
        other => unreachable!("random_edit does not make {other:?}"),
    }
}

#[test]
fn rows_have_the_docs_around_each_step_and_numbers_of_their_own() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    let mut docs = vec![doc.clone()];
    for centi in [10_000, 11_000, 12_000, 13_000] {
        set_tempo(&mut h, &mut doc, centi);
        docs.push(doc.clone());
    }
    let mut numbers = Vec::new();
    // However many steps are taken back, the rows are the same.
    for undone in 0..=4 {
        let rows: Vec<StepRow> = (0..4).map(|i| h.row(&doc, i).unwrap()).collect();
        assert!(h.row(&doc, 4).is_none());
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(Some(&row.kind), h.steps().nth(i));
            assert_eq!(
                held(row.before),
                held(&docs[i]),
                "before step {i}, {undone} undone"
            );
            assert_eq!(
                held(row.after),
                held(&docs[i + 1]),
                "after step {i}, {undone} undone"
            );
        }
        let now: Vec<u64> = rows.iter().map(|r| r.number).collect();
        assert!(now.windows(2).all(|w| w[0] < w[1]));
        numbers.push(now);
        h.undo(&mut doc);
    }
    assert!(numbers.windows(2).all(|w| w[0] == w[1]));

    // A new step after two undone ones: the rows before it stay, it has a new number.
    h.redo(&mut doc);
    h.redo(&mut doc);
    set_tempo(&mut h, &mut doc, 9_000);
    let now: Vec<u64> = (0..3).map(|i| h.row(&doc, i).unwrap().number).collect();
    assert_eq!(now[..2], numbers[0][..2]);
    assert!(!numbers[0].contains(&now[2]));
    assert!(h.row(&doc, 3).is_none());

    // A step that is changed afterwards is another row.
    let mut other = doc.clone();
    other.name = "べつの名前".to_owned();
    assert!(h.amend(&mut doc, other));
    let amended = h.row(&doc, 2).unwrap();
    assert_ne!(amended.number, now[2]);
    assert_eq!(amended.after.name, "べつの名前");
}

#[test]
fn a_jump_lands_where_single_steps_do() {
    for seed in 1..=40u64 {
        let mut rng = Rng(seed.wrapping_mul(0xD6E8_FEB8_6659_FD93) | 1);
        // The same steps in two histories: one jumps, the other walks.
        let (mut h, mut walked) = (History::new(), History::new());
        let (mut doc, mut walked_doc) = (Doc::new_default(), Doc::new_default());
        let mut book = IdBook::new(&doc);
        for round in 0..300 {
            let ctx = format!("seed {seed}, round {round}");
            if rng.below(3) > 0 {
                if let Some(e) = random_edit(&mut rng, &doc) {
                    let before = doc.clone();
                    walked.apply(&mut walked_doc, e.clone());
                    h.apply(&mut doc, e);
                    book.saw(&doc, Some(&before))
                        .unwrap_or_else(|e| panic!("{ctx}: {e}"));
                }
            } else {
                let total = h.undo_count() + h.redo_count();
                // Now and then past the newest step, which is the newest step.
                let to = rng.below(total as u64 + 2) as usize;
                let was = h.undo_count();
                let steps = h.jump(&mut doc, to);
                let to = to.min(total);
                assert_eq!(steps, was.abs_diff(to), "{ctx}");
                while walked.undo_count() > to {
                    walked.undo(&mut walked_doc);
                }
                while walked.undo_count() < to {
                    walked.redo(&mut walked_doc);
                }
                if rng.below(4) == 0 {
                    h.mark_saved();
                    walked.mark_saved();
                }
            }
            book.saw(&doc, None)
                .unwrap_or_else(|e| panic!("{ctx}: {e}"));
            assert_eq!(held(&doc), held(&walked_doc), "{ctx}");
            assert_eq!(doc.next_id, walked_doc.next_id, "{ctx}");
            assert_eq!(h.undo_count(), walked.undo_count(), "{ctx}");
            assert_eq!(h.redo_count(), walked.redo_count(), "{ctx}");
            assert_eq!(h.version(), walked.version(), "{ctx}");
            assert_eq!(
                h.has_unsaved_changes(),
                walked.has_unsaved_changes(),
                "{ctx}"
            );
            for index in 0..h.undo_count() + h.redo_count() {
                let (a, b) = (h.row(&doc, index), walked.row(&walked_doc, index));
                let (a, b) = (a.unwrap(), b.unwrap());
                assert_eq!((a.kind, a.number), (b.kind, b.number), "{ctx}, row {index}");
            }
        }
    }
}

#[test]
fn a_jump_of_no_steps_changes_nothing() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    assert_eq!(h.jump(&mut doc, 0), 0);
    assert_eq!(h.jump(&mut doc, 3), 0);
    set_tempo(&mut h, &mut doc, 9_000);
    set_tempo(&mut h, &mut doc, 10_000);
    let version = h.version();
    assert_eq!(h.jump(&mut doc, 2), 0);
    assert_eq!(h.version(), version);
    assert_eq!(h.jump(&mut doc, 0), 2);
    assert_eq!(doc.timeline.tempo, Doc::new_default().timeline.tempo);
    assert_eq!(h.jump(&mut doc, 9), 2);
    assert_eq!(doc.timeline.tempo, tempo(10_000));
}

fn record(h: &mut History, doc: &mut Doc, origin: u64, key: u8) -> crate::ClipId {
    let (e, added) = edit::add_recording(doc, T1, vec![take(origin, 2 * Q, key)], 0).unwrap();
    h.apply(doc, e);
    added.clip
}

#[test]
fn merged_steps_are_one_step_both_ways() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    set_tempo(&mut h, &mut doc, 10_000);
    let before = doc.clone();
    let before_number = h.version();
    let mark = h.mark();
    assert_eq!((mark.undo_count(), mark.redo_count()), (1, 0));

    let clip = record(&mut h, &mut doc, 0, 60);
    let e = edit::rename_track(&doc, T1, "ベース").unwrap();
    h.apply(&mut doc, e);
    set_tempo(&mut h, &mut doc, 12_000);
    let after = doc.clone();
    let end = h.version();
    let take_id = doc.clip(clip).unwrap().1.keys()[0].take.id;

    assert_eq!(h.merge(&mark, end), 3);
    assert_eq!((h.undo_count(), h.redo_count()), (2, 0));
    assert_eq!(doc, after, "the song stays as it is");
    assert_ne!(h.version(), end, "another step, another number");
    let row = h.row(&doc, 1).unwrap();
    assert!(
        matches!(row.kind, StepKind::Tempo { .. }),
        "named as the last"
    );
    assert_eq!(row.number, h.version());
    assert_eq!(held(row.before), held(&before));
    assert_eq!(held(row.after), held(&after));

    for _ in 0..3 {
        assert!(matches!(h.undo(&mut doc), Some(StepKind::Tempo { .. })));
        assert_eq!(held(&doc), held(&before));
        assert_eq!(h.version(), before_number);
        assert_eq!((h.undo_count(), h.redo_count()), (1, 1));
        assert!(doc.clip(clip).is_none());
        assert!(matches!(h.redo(&mut doc), Some(StepKind::Tempo { .. })));
        assert_eq!(held(&doc), held(&after));
        assert_eq!((h.undo_count(), h.redo_count()), (2, 0));
        // The take made inside the merged step is the same take, with its id.
        assert_eq!(doc.clip(clip).unwrap().1.keys()[0].take.id, take_id);
    }
    // The step before the merged one is as it was.
    h.undo(&mut doc);
    assert!(matches!(h.undo(&mut doc), Some(StepKind::Tempo { .. })));
    assert_eq!(h.undo(&mut doc), None);
}

#[test]
fn a_merge_keeps_the_saved_mark_where_the_song_is_the_same() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    let mark = h.mark();
    set_tempo(&mut h, &mut doc, 10_000);
    set_tempo(&mut h, &mut doc, 11_000);
    h.mark_saved();
    assert_eq!(h.merge(&mark, h.version()), 2);
    assert!(!h.has_unsaved_changes(), "the same song as was saved");
    h.undo(&mut doc);
    assert!(h.has_unsaved_changes());

    // Saved in the middle: no `Doc` the history can reach is the saved one any more.
    let mark = h.mark();
    set_tempo(&mut h, &mut doc, 12_000);
    h.mark_saved();
    set_tempo(&mut h, &mut doc, 13_000);
    assert_eq!(h.merge(&mark, h.version()), 2);
    assert!(h.has_unsaved_changes());
    h.undo(&mut doc);
    assert!(h.has_unsaved_changes());
}

#[test]
fn nothing_is_merged_when_the_steps_are_not_the_newest() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    let mark = h.mark();
    assert_eq!(h.merge(&mark, h.version()), 0, "no step since the mark");
    set_tempo(&mut h, &mut doc, 10_000);
    let one = h.version();
    assert_eq!(h.merge(&mark, one), 1, "one step is one step already");
    assert_eq!(h.version(), one);
    set_tempo(&mut h, &mut doc, 11_000);
    let end = h.version();

    // A step made by someone else after the last of them.
    set_tempo(&mut h, &mut doc, 12_000);
    assert_eq!(h.merge(&mark, end), 0);
    assert_eq!(h.undo_count(), 3);
    h.undo(&mut doc);
    assert_eq!(h.version(), end);
    assert_eq!(h.merge(&mark, end), 0, "a step could be redone");

    // One of them undone.
    h.undo(&mut doc);
    assert_eq!(h.merge(&mark, end), 0);
    assert_eq!(h.merge(&mark, h.version()), 0);
    assert_eq!((h.undo_count(), h.redo_count()), (1, 2));

    // Undone below the mark, and another way from there.
    let mut h = History::new();
    let mut doc = Doc::new_default();
    set_tempo(&mut h, &mut doc, 10_000);
    let mark = h.mark();
    set_tempo(&mut h, &mut doc, 11_000);
    h.undo(&mut doc);
    h.undo(&mut doc);
    set_tempo(&mut h, &mut doc, 9_000);
    set_tempo(&mut h, &mut doc, 9_500);
    assert_eq!(h.merge(&mark, h.version()), 0);
    assert_eq!(h.undo_count(), 2);
}

#[test]
fn a_rewind_leaves_the_history_as_it_was_at_the_mark() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    set_tempo(&mut h, &mut doc, 10_000);
    set_tempo(&mut h, &mut doc, 11_000);
    set_tempo(&mut h, &mut doc, 12_000);
    h.mark_saved();
    h.undo(&mut doc);
    h.undo(&mut doc);
    let before = doc.clone();
    let number = h.version();
    let rows: Vec<u64> = (0..3).map(|i| h.row(&doc, i).unwrap().number).collect();
    let mark = h.mark();
    assert_eq!((mark.undo_count(), mark.redo_count()), (1, 2));

    // No step since: nothing to do.
    assert!(h.rewind(&mut doc, mark.clone(), number));
    assert_eq!((h.undo_count(), h.redo_count()), (1, 2));

    let clip = record(&mut h, &mut doc, 0, 60);
    let e = edit::rename_track(&doc, T1, "ベース").unwrap();
    h.apply(&mut doc, e);
    assert_eq!((h.undo_count(), h.redo_count()), (3, 0));
    let given = doc.next_id;
    let end = h.version();

    assert!(h.rewind(&mut doc, mark, end));
    assert_eq!(held(&doc), held(&before));
    assert_eq!(h.version(), number);
    assert_eq!((h.undo_count(), h.redo_count()), (1, 2));
    assert!(doc.clip(clip).is_none());
    assert_eq!(doc.next_id, given, "the ids given out stay given out");
    let now: Vec<u64> = (0..3).map(|i| h.row(&doc, i).unwrap().number).collect();
    assert_eq!(now, rows);

    // What could be redone can be redone, up to the song that was saved.
    assert!(matches!(h.redo(&mut doc), Some(StepKind::Tempo { .. })));
    assert_eq!(doc.timeline.tempo, tempo(11_000));
    assert!(h.has_unsaved_changes());
    assert!(matches!(h.redo(&mut doc), Some(StepKind::Tempo { .. })));
    assert_eq!(doc.timeline.tempo, tempo(12_000));
    assert!(!h.has_unsaved_changes());
    assert_eq!(h.redo(&mut doc), None);

    // A new clip gets an id the rewound steps did not have.
    let later = record(&mut h, &mut doc, 0, 62);
    assert!(later.0 >= given);
}

#[test]
fn no_rewind_over_what_someone_else_did() {
    let mut h = History::new();
    let mut doc = Doc::new_default();
    set_tempo(&mut h, &mut doc, 10_000);
    let mark = h.mark();
    set_tempo(&mut h, &mut doc, 11_000);
    let end = h.version();

    // A step after the last of them.
    set_tempo(&mut h, &mut doc, 12_000);
    let now = doc.clone();
    assert!(!h.rewind(&mut doc, mark.clone(), end));
    assert_eq!(doc, now);
    assert_eq!((h.undo_count(), h.redo_count()), (3, 0));

    // One of them undone: the song is not where the last of them left it.
    h.undo(&mut doc);
    h.undo(&mut doc);
    let now = doc.clone();
    assert!(!h.rewind(&mut doc, mark.clone(), end));
    assert_eq!(doc, now);
    assert_eq!((h.undo_count(), h.redo_count()), (1, 2));

    // Back at the last of them, but a step could be redone after it.
    h.redo(&mut doc);
    assert_eq!(h.version(), end);
    assert!(!h.rewind(&mut doc, mark, end));
    assert_eq!((h.undo_count(), h.redo_count()), (2, 1));
}
