use super::*;
use crate::TrackId;
use crate::edit::{NewTake, add_recording};
use crate::song::Note;
use crate::time::TICKS_PER_QUARTER;

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn cc(tick: u64, number: u8, value: u8) -> ControlPoint {
    ControlPoint {
        tick,
        value: ControlValue::Cc { number, value },
    }
}

/// A clip [BAR, 3 BAR) of one take at BAR + Q (offset Q in the clip, so the take can grow
/// earlier): a note and two points.
fn one_take() -> (Doc, ClipId) {
    let (e, _) = add_recording(
        &Doc::new_default(),
        TrackId(1),
        vec![NewTake {
            origin: BAR,
            length: 2 * BAR,
            notes: vec![Note {
                start: 0,
                length: 10,
                key: 40,
                velocity: 90,
                from: None,
            }],
            controls: Vec::new(),
        }],
        0,
    )
    .unwrap();
    let (e, added) = add_recording(
        &e.doc,
        TrackId(1),
        vec![NewTake {
            origin: BAR + Q,
            length: BAR,
            notes: vec![Note {
                start: 0,
                length: 100,
                key: 60,
                velocity: 90,
                from: None,
            }],
            controls: vec![cc(0, 64, 127), cc(Q / 2, 1, 30)],
        }],
        0,
    )
    .unwrap();
    (e.doc, added.clip)
}

fn points(doc: &Doc, clip: ClipId) -> Vec<ControlPoint> {
    song_controls(doc.clip(clip).unwrap().1)
}

#[test]
fn values_stop_at_their_range() {
    let v = |kind, value| control_value(kind, value);
    assert_eq!(
        v(ControlKind::Cc(1), 200),
        ControlValue::Cc {
            number: 1,
            value: 127
        }
    );
    assert_eq!(
        v(ControlKind::Cc(1), -3),
        ControlValue::Cc {
            number: 1,
            value: 0
        }
    );
    for number in 64..=69 {
        assert_eq!(
            v(ControlKind::Cc(number), 64),
            ControlValue::Cc { number, value: 127 }
        );
        assert_eq!(
            v(ControlKind::Cc(number), 63),
            ControlValue::Cc { number, value: 0 }
        );
    }
    assert_eq!(
        v(ControlKind::Cc(70), 63),
        ControlValue::Cc {
            number: 70,
            value: 63
        }
    );
    assert_eq!(
        v(ControlKind::PitchBend, 9000),
        ControlValue::PitchBend(8191)
    );
    assert_eq!(
        v(ControlKind::PitchBend, -9000),
        ControlValue::PitchBend(-8192)
    );
}

#[test]
fn adding_a_point_makes_a_new_take_and_selects_the_point() {
    let (doc, clip) = one_take();
    let old = doc.clip(clip).unwrap().1.clone();
    // At the tick of a point there already: after it.
    let e = add_control(&doc, clip, BAR + Q, ControlKind::Cc(64), 90).unwrap();
    assert!(e.of_controls);
    assert_eq!(e.selected, [1]);
    assert_eq!(e.edit.step, StepKind::AddControl { clip });
    let c = e.edit.doc.clip(clip).unwrap().1;
    assert_eq!(c.takes.len(), old.takes.len());
    assert_ne!(c.active_take().take.id, old.active_take().take.id);
    assert_eq!(
        points(&e.edit.doc, clip),
        [
            cc(BAR + Q, 64, 127),
            cc(BAR + Q, 64, 127),
            cc(BAR + 3 * Q / 2, 1, 30)
        ]
    );
    // Before the take: the take grows, the notes stay where they are in the song.
    let e = add_control(&doc, clip, BAR, ControlKind::PitchBend, -100).unwrap();
    assert_eq!(e.selected, [0]);
    let doc2 = e.edit.doc;
    let c = doc2.clip(clip).unwrap().1;
    assert_eq!(c.active_take().offset, 0);
    assert_eq!(crate::notes::song_notes(c)[0].start, BAR + Q);
    assert_eq!(
        points(&doc2, clip)[0],
        ControlPoint {
            tick: BAR,
            value: ControlValue::PitchBend(-100)
        }
    );
    // Outside the clip, or a CC that cannot be.
    assert_eq!(
        add_control(&doc, clip, 3 * BAR, ControlKind::Cc(1), 3).unwrap_err(),
        EditError::OutOfClip
    );
    assert_eq!(
        add_control(&doc, clip, BAR - 1, ControlKind::Cc(1), 3).unwrap_err(),
        EditError::OutOfClip
    );
    assert_eq!(
        add_control(&doc, clip, BAR, ControlKind::Cc(128), 3).unwrap_err(),
        EditError::BadControl
    );
}

#[test]
fn moving_points_stops_at_the_clip_and_the_range() {
    let (doc, clip) = one_take();
    let e = move_controls(&doc, clip, &[0, 1], -(5 * BAR as i64 + 400), 50).unwrap();
    assert_eq!(e.edit.step, StepKind::MoveControls { clip, count: 2 });
    assert_eq!(e.selected, [0, 1]);
    // Together, until the first reaches the start of the clip. The sustain stays on.
    assert_eq!(
        points(&e.edit.doc, clip),
        [cc(BAR, 64, 127), cc(BAR + Q / 2, 1, 80)]
    );
    let e = move_controls(&doc, clip, &[1], 5 * BAR as i64 + 400, -100).unwrap();
    assert_eq!(points(&e.edit.doc, clip)[1], cc(3 * BAR - 1, 1, 0));
    // Past the other point: the order follows the ticks, and the selection follows the point.
    let e = move_controls(&doc, clip, &[0], (Q / 2 + 60) as i64, -64).unwrap();
    assert_eq!(e.selected, [1]);
    assert_eq!(
        points(&e.edit.doc, clip),
        [cc(BAR + 3 * Q / 2, 1, 30), cc(BAR + 3 * Q / 2 + 60, 64, 0)]
    );
    assert_eq!(
        move_controls(&doc, clip, &[0], 0, 0).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        move_controls(&doc, clip, &[5], 1, 0).unwrap_err(),
        EditError::NoSuchNote(5)
    );
}

#[test]
fn deleting_points() {
    let (doc, clip) = one_take();
    let e = delete_controls(&doc, clip, &[0]).unwrap();
    assert_eq!(e.edit.step, StepKind::DeleteControls { clip, count: 1 });
    assert!(e.selected.is_empty());
    assert_eq!(points(&e.edit.doc, clip), [cc(BAR + 3 * Q / 2, 1, 30)]);
    assert_eq!(
        delete_controls(&doc, clip, &[]).unwrap_err(),
        EditError::Unchanged
    );
}
