use std::sync::Arc;

use super::*;
use crate::edit::{self, NewTake};
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
            edit::delete_clips(doc, &[c.id]).ok()
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
                        h.apply(&mut doc, e);
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
            assert_eq!(doc, line[at].0, "{ctx}");
            assert_eq!(h.undo_count(), at, "{ctx}");
            assert_eq!(h.redo_count(), line.len() - 1 - at, "{ctx}");
            assert_eq!(h.has_unsaved_changes(), saved != line[at].1, "{ctx}");
        }
        // All the way back and all the way forward.
        while h.undo(&mut doc).is_some() {}
        assert_eq!(doc, line[0].0);
        while h.redo(&mut doc).is_some() {}
        assert_eq!(doc, line.last().unwrap().0);
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
    }
}
