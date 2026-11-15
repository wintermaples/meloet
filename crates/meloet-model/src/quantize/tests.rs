use super::*;
use crate::TrackId;
use crate::edit::{NewTake, add_recording, switch_take};
use crate::notes::{WriteAt, move_notes, write_note};
use crate::roll::Grid;
use crate::song::Take;

const T1: TrackId = TrackId(1);
const BAR: u64 = 1920;

fn settings(division: u16, triplet: bool, strength: u8, swing: u8) -> QuantizeSettings {
    QuantizeSettings::new(
        Grid::new(division, triplet).unwrap(),
        strength,
        swing,
        QuantizeAlign::Start,
    )
    .unwrap()
}

fn note(start: u64, length: u64, key: u8) -> Note {
    Note {
        start,
        length,
        key,
        velocity: 100,
        from: None,
    }
}

/// A clip [BAR, 3 BAR) of one recorded take at BAR.
fn one_take(notes: Vec<Note>) -> (Doc, ClipId) {
    let take = NewTake {
        origin: BAR,
        length: 2 * BAR,
        notes,
        controls: Vec::new(),
    };
    let (e, added) = add_recording(&Doc::new_default(), T1, vec![take], 0).unwrap();
    (e.doc, added.clip)
}

fn shown(doc: &Doc, clip: ClipId) -> (u64, Arc<Take>) {
    let (_, c) = doc.clip(clip).unwrap();
    let ct = c.active_take();
    (c.start + ct.offset, ct.take.clone())
}

/// `(start, length, from)` in song positions.
fn notes_of(doc: &Doc, clip: ClipId) -> Vec<(u64, u64, Option<u32>)> {
    song_notes(doc.clip(clip).unwrap().1)
        .iter()
        .map(|n| (n.start, n.length, n.from))
        .collect()
}

/// The origin take's notes in song positions: where the screen draws "そろえる前の位置".
fn origin_of(doc: &Doc, clip: ClipId) -> Option<Vec<(u64, u64)>> {
    let (zero, take) = shown(doc, clip);
    let o = take.origin.as_ref()?;
    Some(
        o.take
            .notes
            .iter()
            .map(|n| (zero + n.start, n.length))
            .collect(),
    )
}

#[test]
fn the_grid_strength_and_swing() {
    let m = Meter::FOUR_FOUR;
    let q = |tick, s| quantized_tick(tick, s, m);
    // 1/16 is 120 ticks: 130 goes to 120, 170 to 120, 180 (half way) to 240.
    assert_eq!(q(130, settings(16, false, 100, 0)), 120);
    assert_eq!(q(170, settings(16, false, 100, 0)), 120);
    assert_eq!(q(180, settings(16, false, 100, 0)), 240);
    // Strength 0 leaves it; 50 goes half way, rounding half away from 0.
    assert_eq!(q(170, settings(16, false, 0, 0)), 170);
    assert_eq!(q(170, settings(16, false, 50, 0)), 145);
    assert_eq!(q(131, settings(16, false, 50, 0)), 125);
    assert_eq!(q(109, settings(16, false, 50, 0)), 115);
    assert_eq!(q(101, settings(16, false, 75, 0)), 115);
    // Swing: odd steps later by g × s / 100 / 3; 100 % is the triplet swing.
    assert_eq!(q(130, settings(16, false, 100, 100)), 160);
    assert_eq!(q(250, settings(16, false, 100, 100)), 240);
    assert_eq!(q(130, settings(16, false, 100, 50)), 140);
    assert_eq!(q(480 + 250, settings(8, false, 100, 30)), 480 + 240 + 24);
    // Triplet: 2/3 of the step. 1/8 triplet is 160.
    assert_eq!(q(170, settings(8, true, 100, 0)), 160);
    assert_eq!(q(250, settings(8, true, 100, 0)), 320);
    // One bar, as the meter makes it.
    assert_eq!(q(1000, settings(0, false, 100, 0)), 1920);
    assert_eq!(q(900, settings(0, false, 100, 0)), 0);
    let three = Meter::new(3, 4).unwrap();
    assert_eq!(
        quantized_tick(2000, settings(0, false, 100, 0), three),
        1440
    );
    assert_eq!(quantized_tick(2000, settings(0, true, 100, 0), three), 1920);
    // A half note.
    assert_eq!(q(1000, settings(2, false, 100, 0)), 960);
}

#[test]
fn quantize_makes_a_new_take_that_points_to_the_recorded_one() {
    let (doc, clip) = one_take(vec![
        note(10, 100, 60),
        note(250, 100, 62),
        note(500, 30, 64),
    ]);
    let (_, recorded) = shown(&doc, clip);
    let (e, count) = quantize(&doc, clip, &[], settings(4, false, 100, 0)).unwrap();
    assert_eq!(count, 3);
    let doc2 = &e.edit.doc;
    assert_eq!(
        notes_of(doc2, clip),
        [
            (BAR, 100, Some(0)),
            (BAR + 480, 100, Some(1)),
            (BAR + 480, 30, Some(2))
        ]
    );
    let (_, take) = shown(doc2, clip);
    let o = take.origin.as_ref().unwrap();
    assert!(Arc::ptr_eq(&o.take, &recorded));
    assert_eq!(o.settings, settings(4, false, 100, 0));
    assert_eq!(doc2.clip(clip).unwrap().1.takes.len(), 1);
    assert_eq!(e.edit.step, StepKind::Quantize { clip, count: 3 });
    assert!(e.selected.is_empty(), "nothing was picked");
    // Only some notes: the others keep their place but still point to the recorded notes.
    let (e, count) = quantize(&doc, clip, &[1], settings(4, false, 100, 0)).unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [
            (BAR + 10, 100, Some(0)),
            (BAR + 480, 100, Some(1)),
            (BAR + 500, 30, Some(2))
        ]
    );
    assert_eq!(e.selected, [1]);
    // Notes on the grid already: nothing to do.
    let (doc, clip) = one_take(vec![note(0, 100, 60), note(480, 100, 62)]);
    assert_eq!(
        quantize(&doc, clip, &[], settings(4, false, 100, 0)).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn start_and_end() {
    let (doc, clip) = one_take(vec![note(10, 200, 60), note(470, 20, 62)]);
    let s = QuantizeSettings::new(
        Grid::new(8, false).unwrap(),
        100,
        0,
        QuantizeAlign::StartAndEnd,
    )
    .unwrap();
    let (e, _) = quantize(&doc, clip, &[], s).unwrap();
    // 10–210 → 0–240; 470–490 → 480–480, shorter than a tick: one step.
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(BAR, 240, Some(0)), (BAR + 480, 240, Some(1))]
    );
    let half = QuantizeSettings::new(
        Grid::new(8, false).unwrap(),
        50,
        0,
        QuantizeAlign::StartAndEnd,
    )
    .unwrap();
    let (e, _) = quantize(&doc, clip, &[0], half).unwrap();
    assert_eq!(notes_of(&e.edit.doc, clip)[0], (BAR + 5, 220, Some(0)));
}

#[test]
fn quantizing_again_starts_from_where_the_notes_were_played() {
    let (doc, clip) = one_take(vec![note(130, 100, 60), note(700, 100, 62)]);
    let (e, _) = quantize(&doc, clip, &[], settings(4, false, 100, 0)).unwrap();
    let doc = e.edit.doc;
    assert_eq!(
        notes_of(&doc, clip),
        [(BAR, 100, Some(0)), (BAR + 480, 100, Some(1))]
    );
    // Moved by hand: the note keeps where it came from.
    let doc = move_notes(&doc, clip, &[1], 1000, 0, false)
        .unwrap()
        .edit
        .doc;
    assert_eq!(notes_of(&doc, clip)[1], (BAR + 1480, 100, Some(1)));
    // A written note has no origin note: it is quantized from where it is.
    let doc = write_note(&doc, WriteAt::Clip(clip), note(BAR + 1000, 50, 70))
        .unwrap()
        .edit
        .doc;
    let (e, count) = quantize(&doc, clip, &[], settings(16, false, 50, 0)).unwrap();
    assert_eq!(count, 3);
    // 130 → 120 half way: 125; 700 → 720: 710; the written 1000 → 960: 980.
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [
            (BAR + 125, 100, Some(0)),
            (BAR + 710, 100, Some(1)),
            (BAR + 980, 50, None)
        ]
    );
    // Still the recorded take, not the one quantized first.
    let (_, first) = shown(&doc, clip);
    let (_, again) = shown(&e.edit.doc, clip);
    assert!(Arc::ptr_eq(
        &first.origin.as_ref().unwrap().take,
        &again.origin.as_ref().unwrap().take
    ));
    assert!(again.origin.as_ref().unwrap().take.origin.is_none());
    assert_eq!(
        again.origin.as_ref().unwrap().settings,
        settings(16, false, 50, 0)
    );
}

#[test]
fn back_to_where_they_were_drops_the_origin() {
    let (doc, clip) = one_take(vec![note(130, 100, 60), note(700, 100, 62)]);
    let recorded = notes_of(&doc, clip);
    let doc = quantize(&doc, clip, &[], settings(4, false, 100, 0))
        .unwrap()
        .0
        .edit
        .doc;
    let doc = move_notes(&doc, clip, &[0], 0, 5, false).unwrap().edit.doc;
    // One of two: the take stays quantized.
    let e = unquantize(&doc, clip, &[1]).unwrap();
    assert_eq!(e.edit.step, StepKind::Unquantize { clip, count: 1 });
    assert_eq!(e.selected, [1]);
    let half = e.edit.doc;
    assert_eq!(notes_of(&half, clip)[1], (BAR + 700, 100, Some(1)));
    assert!(shown(&half, clip).1.origin.is_some());
    // Both: no origin, no origin notes; the key moved by hand stays.
    let e = unquantize(&half, clip, &[]).unwrap();
    let (_, take) = shown(&e.edit.doc, clip);
    assert!(take.origin.is_none());
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        recorded
            .iter()
            .map(|&(s, l, _)| (s, l, None))
            .collect::<Vec<_>>()
    );
    assert_eq!(take.notes[0].key, 65);
    assert!(e.selected.is_empty());
    // Nothing quantized: nothing to do.
    assert_eq!(
        unquantize(&e.edit.doc, clip, &[]).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn the_origin_starts_where_the_take_starts_when_the_take_grows_earlier() {
    // The clip [BAR, 3 BAR + 470): a take at BAR, then one at 2 BAR + 470, shown.
    let (doc, clip) = one_take(vec![note(0, 10, 40)]);
    let (e, added) = add_recording(
        &doc,
        T1,
        vec![NewTake {
            origin: 2 * BAR + 470,
            length: BAR,
            notes: vec![note(30, 100, 60), note(500, 100, 62)],
            controls: Vec::new(),
        }],
        0,
    )
    .unwrap();
    assert_eq!(added.clip, clip);
    let doc = e.doc;
    // A bar grid pulls the first note back to 2 BAR, before the take's tick 0.
    let (e, _) = quantize(&doc, clip, &[], settings(0, false, 100, 0)).unwrap();
    let doc2 = e.edit.doc;
    assert_eq!(
        notes_of(&doc2, clip),
        [(2 * BAR, 100, Some(0)), (3 * BAR, 100, Some(1))]
    );
    let (zero, take) = shown(&doc2, clip);
    assert_eq!(zero, 2 * BAR);
    let o = take.origin.as_ref().unwrap();
    assert_ne!(
        o.take.id,
        shown(&doc, clip).1.id,
        "a copy from the new tick 0"
    );
    assert!(o.take.origin.is_none());
    assert_eq!(
        origin_of(&doc2, clip).unwrap(),
        [(2 * BAR + 500, 100), (2 * BAR + 970, 100)]
    );
    assert_eq!(doc2.next_id, doc.next_id + 2);
    // Back to the recorded place: it reads from the copy.
    let back = unquantize(&doc2, clip, &[]).unwrap().edit.doc;
    assert_eq!(
        notes_of(&back, clip),
        [(2 * BAR + 500, 100, None), (2 * BAR + 970, 100, None)]
    );
    // The other take is untouched.
    let c = back.clip(clip).unwrap().1;
    assert_eq!(c.takes.len(), 2);
    assert!(switch_take(&back, clip, 0).is_ok());
}

#[test]
fn quantized_notes_stay_in_the_clip() {
    // The clip is [BAR, 3 BAR); a 1 bar grid would put the second note at 3 BAR: it goes only
    // to where it ends there.
    let (doc, clip) = one_take(vec![note(1000, 100, 60), note(3000, 300, 62)]);
    let (e, _) = quantize(&doc, clip, &[], settings(0, false, 100, 0)).unwrap();
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(2 * BAR, 100, Some(0)), (3 * BAR - 300, 300, Some(1))]
    );
}
