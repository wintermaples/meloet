use super::*;
use crate::TrackId;
use crate::arrange::ClipEdge;
use crate::edit::{NewTake, add_recording, switch_take};
use crate::notes::{WriteAt, move_notes, resize_notes, write_note};
use crate::roll::Grid;
use crate::song::Take;
use crate::tempo_map::{MeterChange, TempoMap, Timeline};
use crate::time::{Meter, TICKS_PER_QUARTER, Tempo};

const T1: TrackId = TrackId(1);
const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

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
    (c.zero(ct) as u64, ct.take.clone())
}

/// `(start, length, from)` in song positions.
fn notes_of(doc: &Doc, clip: ClipId) -> Vec<(u64, u64, Option<u32>)> {
    song_notes(doc.clip(clip).unwrap().1)
        .iter()
        .map(|n| (n.start, n.length, n.from.map(OriginIndex::get)))
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
    let map = |meter| TempoMap::new(&Timeline::new(Tempo::from_centi(9_600).unwrap(), meter));
    let m = map(Meter::FOUR_FOUR);
    let q = |tick, s| quantized_tick(tick, s, &m);
    let s = Q / 4;
    // 1/16 is s = Q / 4: s + 10 goes to s, s + 50 too, s + s / 2 (half way) to 2 s.
    assert_eq!(q(s + 10, settings(16, false, 100, 0)), s);
    assert_eq!(q(s + 50, settings(16, false, 100, 0)), s);
    assert_eq!(q(s + s / 2, settings(16, false, 100, 0)), 2 * s);
    // Strength 0 leaves it; 50 goes half way, rounding half away from 0.
    assert_eq!(q(s + 50, settings(16, false, 0, 0)), s + 50);
    assert_eq!(q(s + 50, settings(16, false, 50, 0)), s + 25);
    assert_eq!(q(s + 11, settings(16, false, 50, 0)), s + 5);
    assert_eq!(q(s - 11, settings(16, false, 50, 0)), s - 5);
    assert_eq!(q(s - 19, settings(16, false, 75, 0)), s - 5);
    // Swing: odd steps later by g × s / 100 / 3; 100 % is the triplet swing.
    assert_eq!(q(s + 10, settings(16, false, 100, 100)), s + s / 3);
    assert_eq!(q(2 * s + 10, settings(16, false, 100, 100)), 2 * s);
    assert_eq!(q(s + 10, settings(16, false, 100, 50)), s + s / 6);
    assert_eq!(
        q(Q + Q / 2 + 10, settings(8, false, 100, 30)),
        Q + Q / 2 + Q / 20
    );
    // Triplet: 2/3 of the step. 1/8 triplet is Q / 3.
    assert_eq!(q(Q / 3 + 10, settings(8, true, 100, 0)), Q / 3);
    assert_eq!(q(2 * Q / 3 - 70, settings(8, true, 100, 0)), 2 * Q / 3);
    // One bar, as the meter makes it.
    assert_eq!(q(2 * Q + 40, settings(0, false, 100, 0)), BAR);
    assert_eq!(q(2 * Q - 60, settings(0, false, 100, 0)), 0);
    let three = map(Meter::new(3, 4).unwrap());
    assert_eq!(
        quantized_tick(BAR + 80, settings(0, false, 100, 0), &three),
        3 * Q
    );
    assert_eq!(
        quantized_tick(BAR + 80, settings(0, true, 100, 0), &three),
        BAR
    );
    // A half note.
    assert_eq!(q(2 * Q + 40, settings(2, false, 100, 0)), 2 * Q);
}

#[test]
fn the_grid_starts_again_at_a_meter_change() {
    // 4/4 for two bars, 3/4 for two (to 2 BAR + 6 Q), then 7/8.
    let mut timeline = Timeline::new(Tempo::from_centi(9_600).unwrap(), Meter::FOUR_FOUR);
    for (bar, num, den) in [(2, 3, 4), (4, 7, 8)] {
        timeline.meter_changes.push(MeterChange {
            bar,
            meter: Meter::new(num, den).unwrap(),
        });
    }
    let map = TempoMap::new(&timeline);
    let q = |tick, s| quantized_tick(tick, s, &map);
    let (three, seven) = (2 * BAR, 2 * BAR + 6 * Q);
    let bar = settings(0, false, 100, 0);
    // Before the change the bar is 4 Q, after it 3 Q from 2 BAR, then 7 Q / 2 from 2 BAR + 6 Q.
    assert_eq!(q(2 * Q + 40, bar), BAR);
    assert_eq!(q(three + 3 * Q / 2 - 20, bar), three);
    assert_eq!(q(three + 3 * Q / 2 + 20, bar), three + 3 * Q);
    assert_eq!(q(three + 3 * Q + 3 * Q / 2 + 80, bar), seven);
    assert_eq!(q(seven + 7 * Q / 4 + 60, bar), seven + 7 * Q / 2);
    assert_eq!(
        q(seven + 7 * Q / 2 + 7 * Q / 4 - 40, bar),
        seven + 7 * Q / 2
    );
    // A half note (2 Q) does not fit the 3/4 bars: the steps count from the change, and the
    // one past the next change is the head of its bar.
    let half = settings(2, false, 100, 0);
    assert_eq!(q(three + Q + 20, half), three + 2 * Q);
    assert_eq!(q(three + 3 * Q + 60, half), three + 4 * Q);
    assert_eq!(q(three + 6 * Q - 3 * Q / 8, half), seven);
    assert_eq!(q(seven + 100, half), seven);
    // 1/16 lands on the 7/8 bar lines.
    let sixteenth = settings(16, false, 100, 0);
    assert_eq!(
        q(seven + 7 * Q / 2 + Q / 8 - 10, sixteenth),
        seven + 7 * Q / 2
    );
    assert_eq!(
        q(seven + 7 * Q / 2 + Q / 8 + 10, sixteenth),
        seven + 7 * Q / 2 + Q / 4
    );
    // The swing counts its odd steps from the change too.
    let swung = settings(8, false, 100, 100);
    assert_eq!(q(seven + Q / 2 + 10, swung), seven + Q / 2 + Q / 6);
    assert_eq!(q(seven + Q - 10, swung), seven + Q);
}

#[test]
fn quantize_to_the_bar_after_a_meter_change() {
    // The clip is [BAR, 3 BAR); from its head on the bars are 3/4.
    let (doc, clip) = one_take(vec![note(2 * Q - 160, 100, 60), note(5 * Q - 100, 100, 62)]);
    let doc = crate::timeline::add_meter_change(&doc, 1, Meter::new(3, 4).unwrap())
        .unwrap()
        .doc;
    let (edited, count) = quantize(&doc, clip, &[], settings(0, false, 100, 0)).unwrap();
    assert_eq!(count, 2);
    let starts: Vec<u64> = (notes_of(&edited.edit.doc, clip).iter())
        .map(|n| n.0)
        .collect();
    assert_eq!(starts, [BAR + 3 * Q, BAR + 6 * Q]);
}

#[test]
fn quantize_makes_a_new_take_that_points_to_the_recorded_one() {
    let (doc, clip) = one_take(vec![
        note(10, 100, 60),
        note(Q / 2 + 10, 100, 62),
        note(Q + 20, 30, 64),
    ]);
    let (_, recorded) = shown(&doc, clip);
    let (e, count) = quantize(&doc, clip, &[], settings(4, false, 100, 0)).unwrap();
    assert_eq!(count, 3);
    let doc2 = &e.edit.doc;
    assert_eq!(
        notes_of(doc2, clip),
        [
            (BAR, 100, Some(0)),
            (BAR + Q, 100, Some(1)),
            (BAR + Q, 30, Some(2))
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
            (BAR + Q, 100, Some(1)),
            (BAR + Q + 20, 30, Some(2))
        ]
    );
    assert_eq!(e.selected, [1]);
    // Notes on the grid already: nothing to do.
    let (doc, clip) = one_take(vec![note(0, 100, 60), note(Q, 100, 62)]);
    assert_eq!(
        quantize(&doc, clip, &[], settings(4, false, 100, 0)).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn start_and_end() {
    let (doc, clip) = one_take(vec![note(10, Q / 2 - 40, 60), note(Q - 10, 20, 62)]);
    let s = QuantizeSettings::new(
        Grid::new(8, false).unwrap(),
        100,
        0,
        QuantizeAlign::StartAndEnd,
    )
    .unwrap();
    let (e, _) = quantize(&doc, clip, &[], s).unwrap();
    // 10 to Q / 2 - 30 → 0 to Q / 2; Q - 10 to Q + 10 → Q to Q, shorter than a tick: one step.
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(BAR, Q / 2, Some(0)), (BAR + Q, Q / 2, Some(1))]
    );
    let half = QuantizeSettings::new(
        Grid::new(8, false).unwrap(),
        50,
        0,
        QuantizeAlign::StartAndEnd,
    )
    .unwrap();
    let (e, _) = quantize(&doc, clip, &[0], half).unwrap();
    assert_eq!(
        notes_of(&e.edit.doc, clip)[0],
        (BAR + 5, Q / 2 - 20, Some(0))
    );
}

#[test]
fn quantizing_again_starts_from_where_the_notes_were_played() {
    let (doc, clip) = one_take(vec![
        note(Q / 4 + 10, 100, 60),
        note(3 * Q / 2 - 20, 100, 62),
    ]);
    let (e, _) = quantize(&doc, clip, &[], settings(4, false, 100, 0)).unwrap();
    let doc = e.edit.doc;
    assert_eq!(
        notes_of(&doc, clip),
        [(BAR, 100, Some(0)), (BAR + Q, 100, Some(1))]
    );
    // Moved by hand: the note keeps where it came from.
    let doc = move_notes(&doc, clip, &[1], 1000, 0, false)
        .unwrap()
        .edit
        .doc;
    assert_eq!(notes_of(&doc, clip)[1], (BAR + Q + 1000, 100, Some(1)));
    // A written note has no origin note: it is quantized from where it is.
    let doc = write_note(&doc, WriteAt::Clip(clip), note(BAR + 2 * Q + 40, 50, 70))
        .unwrap()
        .edit
        .doc;
    let (e, count) = quantize(&doc, clip, &[], settings(16, false, 50, 0)).unwrap();
    assert_eq!(count, 3);
    // Q / 4 + 10 → Q / 4 half way: Q / 4 + 5; 3 Q / 2 - 20 → 3 Q / 2: 3 Q / 2 - 10; the written 2 Q + 40 → 2 Q: 2 Q + 20.
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [
            (BAR + Q / 4 + 5, 100, Some(0)),
            (BAR + 3 * Q / 2 - 10, 100, Some(1)),
            (BAR + 2 * Q + 20, 50, None)
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
fn start_only_keeps_a_length_changed_by_hand() {
    let (doc, clip) = one_take(vec![note(Q / 4 + 10, 100, 60), note(Q - 10, 200, 62)]);
    let doc = quantize(&doc, clip, &[], settings(4, false, 100, 0))
        .unwrap()
        .0
        .edit
        .doc;
    let doc = resize_notes(&doc, clip, &[0], ClipEdge::End, 300)
        .unwrap()
        .edit
        .doc;
    assert_eq!(notes_of(&doc, clip)[0], (BAR, 400, Some(0)));
    // Again, start only: the start from where it was played, the length as it is now.
    let (e, _) = quantize(&doc, clip, &[], settings(16, false, 100, 0)).unwrap();
    let again = e.edit.doc;
    assert_eq!(
        notes_of(&again, clip),
        [(BAR + Q / 4, 400, Some(0)), (BAR + Q, 200, Some(1))]
    );
    // Start and end: both ends from where they were played.
    let both = QuantizeSettings::new(
        Grid::new(16, false).unwrap(),
        100,
        0,
        QuantizeAlign::StartAndEnd,
    )
    .unwrap();
    let (e, _) = quantize(&doc, clip, &[0], both).unwrap();
    // Q / 4 + 10 to Q / 4 + 110 → Q / 4 to Q / 4, shorter than a tick: one step.
    assert_eq!(
        notes_of(&e.edit.doc, clip)[0],
        (BAR + Q / 4, Q / 4, Some(0))
    );
    // Back: the recorded start and length.
    let back = unquantize(&again, clip, &[]).unwrap().edit.doc;
    assert_eq!(
        notes_of(&back, clip),
        [(BAR + Q / 4 + 10, 100, None), (BAR + Q - 10, 200, None)]
    );
    assert!(shown(&back, clip).1.origin.is_none());
}

#[test]
fn start_only_fits_the_length_of_now_into_the_clip() {
    // A note near the clip's end, made longer by hand up to the end.
    let (doc, clip) = one_take(vec![note(2 * BAR - Q + 30, 100, 60)]);
    let doc = quantize(&doc, clip, &[], settings(4, false, 100, 0))
        .unwrap()
        .0
        .edit
        .doc;
    assert_eq!(notes_of(&doc, clip), [(3 * BAR - Q, 100, Some(0))]);
    let doc = resize_notes(&doc, clip, &[0], ClipEdge::End, (Q - 100 - 10) as i64)
        .unwrap()
        .edit
        .doc;
    // Half way to the grid is 15 past the beat, where the note of Q - 10 would pass the end.
    let (e, _) = quantize(&doc, clip, &[], settings(4, false, 50, 0)).unwrap();
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(3 * BAR - Q + 10, Q - 10, Some(0))]
    );
}

#[test]
fn back_to_where_they_were_drops_the_origin() {
    let (doc, clip) = one_take(vec![
        note(Q / 4 + 10, 100, 60),
        note(3 * Q / 2 - 20, 100, 62),
    ]);
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
    assert_eq!(
        notes_of(&half, clip)[1],
        (BAR + 3 * Q / 2 - 20, 100, Some(1))
    );
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
    // The clip [BAR, 3 BAR + Q - 10): a take at BAR, then one at 2 BAR + Q - 10, shown.
    let (doc, clip) = one_take(vec![note(0, 10, 40)]);
    let (e, added) = add_recording(
        &doc,
        T1,
        vec![NewTake {
            origin: 2 * BAR + Q - 10,
            length: BAR,
            notes: vec![note(30, 100, 60), note(Q + 20, 100, 62)],
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
        [(2 * BAR + Q + 20, 100), (2 * BAR + 2 * Q + 10, 100)]
    );
    assert_eq!(doc2.next_id, doc.next_id + 2);
    // Back to the recorded place: it reads from the copy.
    let back = unquantize(&doc2, clip, &[]).unwrap().edit.doc;
    assert_eq!(
        notes_of(&back, clip),
        [
            (2 * BAR + Q + 20, 100, None),
            (2 * BAR + 2 * Q + 10, 100, None)
        ]
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
    let (doc, clip) = one_take(vec![note(2 * Q + 40, 100, 60), note(25 * Q / 4, 300, 62)]);
    let (e, _) = quantize(&doc, clip, &[], settings(0, false, 100, 0)).unwrap();
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(2 * BAR, 100, Some(0)), (3 * BAR - 300, 300, Some(1))]
    );
}
