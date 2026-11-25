use super::*;
use crate::TICKS_PER_QUARTER;
use crate::edit::{add_vocal_track, switch_take};
use crate::notes::{song_controls, song_notes};
use crate::song::{CC_MODULATION, CC_SUSTAIN};

const T1: TrackId = TrackId(1);
const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn note(start: u64, length: u64, key: u8) -> Note {
    Note {
        from: None,
        start,
        length,
        key,
        velocity: 100,
    }
}

fn new_take(origin: u64, length: u64, notes: Vec<Note>) -> NewTake {
    NewTake {
        origin,
        length,
        notes,
        controls: Vec::new(),
    }
}

/// A clip of `T1` from `start`, `bars` long, recorded with one note at its head.
fn with_clip(doc: &Doc, start: u64, bars: u64, key: u8) -> Doc {
    let take = new_take(start, bars * BAR, vec![note(0, Q, key)]);
    add_recording(doc, T1, vec![take], 0).unwrap().0.doc
}

fn clip_at(doc: &Doc, index: usize) -> &Clip {
    &doc.track(T1).unwrap().clips[index]
}

fn keys(clip: &Clip) -> Vec<(u64, u8)> {
    song_notes(clip).iter().map(|n| (n.start, n.key)).collect()
}

#[test]
fn notes_go_into_the_shown_take_as_an_edit() {
    let doc = with_clip(&Doc::new_default(), 0, 4, 36);
    let before = clip_at(&doc, 0).clone();
    let (edit, added) =
        overdub_recording(&doc, T1, new_take(0, 4 * BAR, vec![note(Q, Q / 2, 42)])).unwrap();
    assert_eq!(
        edit.step,
        StepKind::Record {
            track: T1,
            clip: before.id,
            takes: 0
        }
    );
    let c = clip_at(&edit.doc, 0);
    assert_eq!(
        (c.id, c.start, c.end, c.takes.len()),
        (before.id, 0, 4 * BAR, 1)
    );
    assert_eq!(
        added,
        RecordingAdded {
            clip: c.id,
            active: 0
        }
    );
    assert_ne!(c.active_take_id(), before.active_take_id(), "a new id");
    assert_eq!(keys(c), vec![(0, 36), (Q, 42)]);
    assert_eq!(keys(&before), vec![(0, 36)], "the old take is as it was");
}

#[test]
fn the_shown_take_is_the_one_that_grows_and_the_others_stay() {
    let doc = with_clip(&Doc::new_default(), 0, 2, 36);
    let again = new_take(0, 2 * BAR, vec![note(0, Q, 38)]);
    let doc = add_recording(&doc, T1, vec![again], 0).unwrap().0.doc;
    let id = clip_at(&doc, 0).id;
    let doc = switch_take(&doc, id, 0).unwrap().doc;
    let second = clip_at(&doc, 0).keys()[1].take.clone();
    let (edit, added) =
        overdub_recording(&doc, T1, new_take(0, 2 * BAR, vec![note(Q, Q, 42)])).unwrap();
    let c = clip_at(&edit.doc, 0);
    assert_eq!((c.takes.len(), c.active, added.active), (2, 0, 0));
    assert_eq!(keys(c), vec![(0, 36), (Q, 42)]);
    assert!(Arc::ptr_eq(&c.keys()[1].take, &second));
}

#[test]
fn notes_past_the_end_grow_the_clip_by_bars_up_to_the_next_clip() {
    let doc = with_clip(&Doc::new_default(), 0, 2, 36);
    let doc = with_clip(&doc, 4 * BAR, 2, 38);
    // One note in the clip, one from bar 3 into bar 4: two bars more.
    let played = new_take(
        0,
        4 * BAR,
        vec![note(Q, Q, 41), note(2 * BAR + Q, 4 * Q, 42)],
    );
    let (edit, _) = overdub_recording(&doc, T1, played).unwrap();
    let c = clip_at(&edit.doc, 0);
    assert_eq!((c.start, c.end), (0, 4 * BAR));
    assert_eq!(keys(c), vec![(0, 36), (Q, 41), (2 * BAR + Q, 42)]);
    // Into the next clip: the clip stops there, and the note's tail is trimmed off.
    let played = new_take(0, 5 * BAR, vec![note(Q, Q, 43), note(3 * BAR, 2 * BAR, 44)]);
    let (edit, _) = overdub_recording(&doc, T1, played).unwrap();
    let c = clip_at(&edit.doc, 0);
    assert_eq!((c.start, c.end), (0, 4 * BAR));
    assert_eq!(edit.doc.track(T1).unwrap().clips.len(), 2, "not merged");
    assert_eq!(song_notes(c).last().unwrap().length, 2 * BAR);
}

#[test]
fn notes_before_the_start_grow_it_back_to_the_bar() {
    let doc = with_clip(&Doc::new_default(), 2 * BAR, 2, 36);
    let played = new_take(BAR, 3 * BAR, vec![note(3 * Q, Q, 42), note(5 * Q, Q, 43)]);
    let (edit, _) = overdub_recording(&doc, T1, played).unwrap();
    let c = clip_at(&edit.doc, 0);
    assert_eq!((c.start, c.end), (BAR, 4 * BAR));
    assert_eq!(
        keys(c),
        vec![(BAR + 3 * Q, 42), (2 * BAR, 36), (2 * BAR + Q, 43)]
    );
}

#[test]
fn with_no_clip_there_a_new_clip_fills_only_the_empty_place() {
    let doc = with_clip(&Doc::new_default(), 0, 2, 36);
    // Recorded from bar 1, played only in bars 3 and 4.
    let played = new_take(0, 4 * BAR, vec![note(2 * BAR, Q, 42), note(3 * BAR, Q, 43)]);
    let (edit, added) = overdub_recording(&doc, T1, played).unwrap();
    let clips = &edit.doc.track(T1).unwrap().clips;
    assert_eq!(clips.len(), 2);
    assert!(
        Arc::ptr_eq(&clips[0], &doc.track(T1).unwrap().clips[0]),
        "left as it was"
    );
    assert_eq!((clips[1].start, clips[1].end), (2 * BAR, 4 * BAR));
    assert_eq!(added.clip, clips[1].id);
    assert_eq!(keys(&clips[1]), vec![(2 * BAR, 42), (3 * BAR, 43)]);
    assert_eq!(
        edit.step,
        StepKind::Record {
            track: T1,
            clip: clips[1].id,
            takes: 1
        }
    );
}

/// What was held where the empty place starts (a pedal down from the start of the recording,
/// a modulation moved later) is there at the head of the new clip; a controller with a point
/// right there, or back at rest, gets none.
#[test]
fn with_no_clip_there_the_new_clip_starts_with_what_was_held() {
    let doc = with_clip(&Doc::new_default(), 0, 2, 36);
    let cc = |tick, number, value| ControlPoint {
        tick,
        value: ControlValue::Cc { number, value },
    };
    let bend = |tick, v| ControlPoint {
        tick,
        value: ControlValue::PitchBend(v),
    };
    let mut played = new_take(0, 4 * BAR, vec![note(2 * BAR + Q, Q, 42)]);
    played.controls = vec![
        cc(0, CC_SUSTAIN, 127),
        bend(Q, 500),
        cc(BAR, CC_MODULATION, 30),
        bend(BAR + Q, 0),
        cc(2 * BAR, CC_MODULATION, 40),
        cc(3 * BAR, CC_SUSTAIN, 0),
    ];
    let (edit, _) = overdub_recording(&doc, T1, played).unwrap();
    let clips = &edit.doc.track(T1).unwrap().clips;
    assert!(Arc::ptr_eq(&clips[0], &doc.track(T1).unwrap().clips[0]));
    assert_eq!((clips[1].start, clips[1].end), (2 * BAR, 4 * BAR));
    assert_eq!(
        song_controls(&clips[1]),
        [
            cc(2 * BAR, CC_SUSTAIN, 127),
            cc(2 * BAR, CC_MODULATION, 40),
            cc(3 * BAR, CC_SUSTAIN, 0)
        ]
    );
}

#[test]
fn notes_over_two_clips_make_them_one() {
    let doc = with_clip(&Doc::new_default(), 0, 1, 36);
    let again = new_take(0, BAR, vec![note(Q, Q, 37)]);
    let doc = add_recording(&doc, T1, vec![again], 0).unwrap().0.doc;
    let doc = with_clip(&doc, 2 * BAR, 1, 38);
    let (first, second) = (clip_at(&doc, 0).clone(), clip_at(&doc, 1).clone());
    let played = new_take(
        0,
        3 * BAR,
        vec![note(Q / 2, Q, 42), note(2 * BAR + Q, Q, 43)],
    );
    let (edit, added) = overdub_recording(&doc, T1, played).unwrap();
    let clips = &edit.doc.track(T1).unwrap().clips;
    assert_eq!(clips.len(), 1);
    let c = &clips[0];
    assert_eq!((c.id, c.name.as_str()), (first.id, first.name.as_str()));
    assert_eq!((c.start, c.end), (0, 3 * BAR));
    // The first clip's other take stays; the two shown takes became one.
    assert_eq!((c.takes.len(), c.active, added.active), (2, 1, 1));
    assert!(Arc::ptr_eq(&c.keys()[0].take, &first.keys()[0].take));
    assert_eq!(
        keys(c),
        vec![(Q / 2, 42), (Q, 37), (2 * BAR, 38), (2 * BAR + Q, 43)]
    );
    assert!(edit.doc.clip(second.id).is_none());
}

#[test]
fn a_point_there_already_is_not_written_twice() {
    let doc = with_clip(&Doc::new_default(), 0, 1, 36);
    let pedal = |tick, value| ControlPoint {
        tick,
        value: ControlValue::Cc {
            number: CC_SUSTAIN,
            value,
        },
    };
    let mut played = new_take(0, BAR, vec![note(0, Q, 40)]);
    played.controls = vec![pedal(0, 127), pedal(Q, 0)];
    let doc = overdub_recording(&doc, T1, played.clone()).unwrap().0.doc;
    played.notes = vec![note(Q, Q, 41)];
    let doc = overdub_recording(&doc, T1, played).unwrap().0.doc;
    assert_eq!(
        song_controls(clip_at(&doc, 0)),
        vec![pedal(0, 127), pedal(Q, 0)]
    );
}

#[test]
fn vocal_tracks_and_empty_passes_are_turned_away() {
    let (edit, vocal) = add_vocal_track(&Doc::new_default(), None, crate::Language::Ja).unwrap();
    let played = new_take(0, BAR, vec![note(0, Q, 40)]);
    assert_eq!(
        overdub_recording(&edit.doc, vocal, played).unwrap_err(),
        EditError::NotKeys
    );
    let empty = new_take(0, BAR, Vec::new());
    assert_eq!(
        overdub_recording(&Doc::new_default(), T1, empty).unwrap_err(),
        EditError::NoTakes
    );
}
