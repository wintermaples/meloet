use std::sync::Arc;

use super::*;
use crate::edit::{self, NewTake};
use crate::id_book::{IdBook, held};
use crate::song::Note;

const T1: TrackId = TrackId(1);

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
    let (e, a) = edit::add_recording(&doc, T1, vec![take(0, 960, 60)], 0).unwrap();
    h.apply(&mut doc, e);
    let clip_a = doc.tracks[0].clips[0].clone();
    let (e, _) = edit::add_recording(&doc, T1, vec![take(5000, 960, 62)], 0).unwrap();
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
    let (e, a) = edit::add_recording(&doc, T1, vec![take(0, 960, 60)], 0).unwrap();
    h.apply(&mut doc, e);
    let (e, b) = edit::add_recording(&doc, T1, vec![take(5000, 960, 62)], 0).unwrap();
    h.apply(&mut doc, e);
    let given = doc.next_id;
    let b_take = doc.clip(b.clip).unwrap().1.takes[0].take.id;
    h.undo(&mut doc);
    assert_eq!(doc.next_id, given, "undo keeps the ids given out");
    assert!(doc.clip(b.clip).is_none());
    let (e, c) = edit::add_recording(&doc, T1, vec![take(5000, 960, 64)], 0).unwrap();
    h.apply(&mut doc, e);
    assert!(c.clip.0 >= given);
    assert!(doc.clip(c.clip).unwrap().1.takes[0].take.id.0 >= given);
    assert_ne!(doc.clip(c.clip).unwrap().1.takes[0].take.id, b_take);
    // All the way back, then a new step: still past every id given out.
    h.undo(&mut doc);
    h.undo(&mut doc);
    assert!(doc.clip(a.clip).is_none());
    let past = doc.next_id;
    assert!(past > c.clip.0);
    let (e, d) = edit::add_recording(&doc, T1, vec![take(0, 960, 65)], 0).unwrap();
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
            let origin = rng.below(20) * 480;
            let takes = (0..n)
                .map(|_| take(origin, 480 + rng.below(4) * 480, rng.below(128) as u8))
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
