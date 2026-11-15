use super::*;
use crate::edit::{NewTake, add_recording, switch_take};
use crate::song::ControlValue;

const T1: TrackId = TrackId(1);
const BAR: u64 = 1920;

fn note(start: u64, length: u64, key: u8) -> Note {
    Note {
        from: None,
        start,
        length,
        key,
        velocity: 100,
    }
}

/// One clip [1920, 5760) whose one take starts at 2400 and is 960 long, with a sustain at 0.
fn one_clip(notes: Vec<Note>) -> (Doc, ClipId) {
    let mut take = NewTake {
        origin: 2400,
        length: 960,
        notes,
        controls: vec![ControlPoint {
            tick: 0,
            value: ControlValue::Cc {
                number: 64,
                value: 127,
            },
        }],
    };
    take.notes.sort_by_key(|n| n.start);
    let (doc, added) = record(&Doc::new_default(), vec![take]);
    let (edit, _) = add_recording(
        &doc,
        T1,
        vec![NewTake {
            origin: BAR,
            length: 2 * BAR,
            notes: vec![note(0, 10, 40)],
            controls: Vec::new(),
        }],
        0,
    )
    .unwrap();
    let doc = switch_take(&edit.doc, added, 0).unwrap().doc;
    (doc, added)
}

fn record(doc: &Doc, takes: Vec<NewTake>) -> (Doc, ClipId) {
    let (edit, added) = add_recording(doc, T1, takes, 0).unwrap();
    (edit.doc, added.clip)
}

fn shown(doc: &Doc, clip: ClipId) -> (u64, Arc<Take>) {
    let (_, c) = doc.clip(clip).unwrap();
    let ct = c.active_take();
    (c.zero(ct) as u64, ct.take.clone())
}

fn notes_of(doc: &Doc, clip: ClipId) -> Vec<(u64, u64, u8)> {
    song_notes(doc.clip(clip).unwrap().1)
        .iter()
        .map(|n| (n.start, n.length, n.key))
        .collect()
}

#[test]
fn moving_makes_a_new_take_in_the_same_place() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60), note(480, 100, 64)]);
    let before = doc.clip(clip).unwrap().1.clone();
    let e = move_notes(&doc, clip, &[1], -480, 2, false).unwrap();
    let after = e.edit.doc.clip(clip).unwrap().1.clone();
    assert_eq!(after.takes.len(), before.takes.len());
    assert_eq!(after.active, before.active);
    assert_ne!(after.active_take().take.id, before.active_take().take.id);
    assert_eq!(e.edit.doc.next_id, doc.next_id + 1);
    // Same start: the lower key first.
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(2400, 100, 60), (2400, 100, 66)]
    );
    assert_eq!(e.selected, [1]);
    assert_eq!(
        e.edit.step,
        StepKind::MoveNotes {
            clip,
            count: 1,
            copy: false
        }
    );
    // The other takes and the old one are untouched.
    assert!(Arc::ptr_eq(&after.takes[1].take, &before.takes[1].take));
    assert_eq!(notes_of(&doc, clip), [(2400, 100, 60), (2880, 100, 64)]);
}

#[test]
fn moving_stops_at_the_clip_and_the_keys() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60), note(480, 100, 120)]);
    let e = move_notes(&doc, clip, &[0, 1], -100_000, 50, false).unwrap();
    // The earliest stops at the clip start, the highest at key 127.
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(BAR, 100, 67), (BAR + 480, 100, 127)]
    );
    let e = move_notes(&doc, clip, &[0, 1], 100_000, -100, false).unwrap();
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(3 * BAR - 580, 100, 0), (3 * BAR - 100, 100, 60)]
    );
    // The take grew both ways; the sustain stays where it was in the song.
    let (origin, take) = shown(
        &move_notes(&doc, clip, &[0], -480, 0, false)
            .unwrap()
            .edit
            .doc,
        clip,
    );
    assert_eq!(origin, 1920);
    assert_eq!(take.length, 960 + 480);
    assert_eq!(take.controls[0].tick, 480);
    let (origin, take) = shown(
        &move_notes(&doc, clip, &[1], 960, 0, false)
            .unwrap()
            .edit
            .doc,
        clip,
    );
    assert_eq!(origin, 2400);
    assert_eq!(take.length, 480 + 960 + 100);
    // Against the wall: nothing to do.
    let at_top = move_notes(&e.edit.doc, clip, &[0, 1], 100, 0, false);
    assert_eq!(at_top.unwrap_err(), EditError::Unchanged);
}

#[test]
fn copying_adds_and_selects_the_copies() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60), note(480, 100, 64)]);
    let e = move_notes(&doc, clip, &[0, 1], 960, 0, true).unwrap();
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [
            (2400, 100, 60),
            (2880, 100, 64),
            (3360, 100, 60),
            (3840, 100, 64)
        ]
    );
    assert_eq!(e.selected, [2, 3]);
    // In place: copies right after their originals, still a change.
    let e = move_notes(&doc, clip, &[0], 0, 0, true).unwrap();
    assert_eq!(e.selected, [1]);
    assert_eq!(notes_of(&e.edit.doc, clip).len(), 3);
}

#[test]
fn resizing_keeps_one_tick_and_the_clip_end() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60), note(480, 10, 64)]);
    let e = resize_notes(&doc, clip, &[0, 1], -50).unwrap();
    assert_eq!(notes_of(&e.edit.doc, clip), [(2400, 50, 60), (2880, 1, 64)]);
    let e = resize_notes(&doc, clip, &[0, 1], 100_000).unwrap();
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(2400, 3 * BAR - 2400, 60), (2880, 3 * BAR - 2880, 64)]
    );
    assert_eq!(shown(&e.edit.doc, clip).1.length, 3 * BAR - 2400);
    let one = resize_notes(&doc, clip, &[1], -10).unwrap().edit.doc;
    assert_eq!(
        resize_notes(&one, clip, &[1], -10).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn velocity_stops_at_1_and_127() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60), note(480, 10, 64)]);
    let e = change_velocity(&doc, clip, &[0, 1], 1000).unwrap();
    let (_, take) = shown(&e.edit.doc, clip);
    assert!(take.notes.iter().all(|n| n.velocity == 127));
    let e = change_velocity(&e.edit.doc, clip, &[1], -1000).unwrap();
    let (_, take) = shown(&e.edit.doc, clip);
    assert_eq!(take.notes[1].velocity, 1);
    assert_eq!(
        change_velocity(&e.edit.doc, clip, &[1], -5).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn deleting_and_bad_indexes() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60), note(480, 10, 64)]);
    let e = delete_notes(&doc, clip, &[0, 0], true).unwrap();
    assert_eq!(notes_of(&e.edit.doc, clip), [(2880, 10, 64)]);
    assert!(e.selected.is_empty());
    assert_eq!(
        e.edit.step,
        StepKind::DeleteNotes {
            clip,
            count: 1,
            cut: true
        }
    );
    let e = delete_notes(&e.edit.doc, clip, &[0], false).unwrap();
    assert!(notes_of(&e.edit.doc, clip).is_empty());
    assert_eq!(
        delete_notes(&doc, clip, &[2], false).unwrap_err(),
        EditError::NoSuchNote(2)
    );
    assert_eq!(
        delete_notes(&doc, clip, &[], false).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        delete_notes(&doc, ClipId(999), &[0], false).unwrap_err(),
        EditError::NoSuchClip(ClipId(999))
    );
}

#[test]
fn pasting_leaves_out_what_does_not_fit() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60), note(480, 100, 64)]);
    let copied = clipboard(doc.clip(clip).unwrap().1, &[1, 0]).unwrap();
    assert_eq!(copied, [note(0, 100, 60), note(480, 100, 64)]);
    let e = paste_notes(&doc, clip, &copied, BAR).unwrap();
    assert_eq!(e.selected, [0, 2]);
    assert_eq!(notes_of(&e.edit.doc, clip).len(), 4);
    assert_eq!(shown(&e.edit.doc, clip).0, BAR);
    // The second would end past the clip.
    let e = paste_notes(&doc, clip, &copied, 3 * BAR - 500).unwrap();
    assert_eq!(e.selected, [2]);
    assert_eq!(e.edit.step, StepKind::PasteNotes { clip, count: 1 });
    assert_eq!(
        paste_notes(&doc, clip, &copied, 3 * BAR - 50).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        paste_notes(&doc, clip, &copied, 0).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn writing_in_a_clip() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60)]);
    let e = write_note(&doc, WriteAt::Clip(clip), note(3 * BAR - 10, 480, 62)).unwrap();
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(2400, 100, 60), (3 * BAR - 10, 10, 62)]
    );
    assert_eq!(e.selected, [1]);
    assert_eq!(
        write_note(&doc, WriteAt::Clip(clip), note(3 * BAR, 10, 62)).unwrap_err(),
        EditError::OutOfClip
    );
    assert_eq!(
        write_note(&doc, WriteAt::Clip(clip), note(BAR, 0, 62)).unwrap_err(),
        EditError::BadNote
    );
    // On the track, inside the clip: the same clip.
    let e = write_note(&doc, WriteAt::Track(T1), note(BAR, 10, 62)).unwrap();
    assert_eq!(e.clip, clip);
    assert_eq!(
        e.edit.step,
        StepKind::WriteNote {
            clip,
            clip_made: false
        }
    );
}

#[test]
fn writing_where_there_is_no_clip_makes_one() {
    let doc = Doc::new_default();
    let e = write_note(&doc, WriteAt::Track(T1), note(BAR + 700, 480, 60)).unwrap();
    let (track, c) = e.edit.doc.clip(e.clip).unwrap();
    assert_eq!((c.start, c.end), (BAR, 5 * BAR));
    assert_eq!(c.name, track.name);
    assert_eq!(c.takes.len(), 1);
    assert_eq!(notes_of(&e.edit.doc, e.clip), [(BAR + 700, 480, 60)]);
    assert_eq!(e.selected, [0]);
    assert_eq!(e.edit.doc.next_id, doc.next_id + 2);
    assert!(matches!(
        e.edit.step,
        StepKind::WriteNote {
            clip_made: true,
            ..
        }
    ));

    // Up to the next clip, and from the end of the one before.
    let (doc, clip) = one_clip(vec![note(0, 100, 60)]);
    let e = write_note(&doc, WriteAt::Track(T1), note(100, 4000, 60)).unwrap();
    let c = e.edit.doc.clip(e.clip).unwrap().1;
    assert_eq!((c.start, c.end), (0, BAR));
    assert_eq!(notes_of(&e.edit.doc, e.clip), [(100, BAR - 100, 60)]);
    let e = write_note(&doc, WriteAt::Track(T1), note(3 * BAR + 10, 10, 60)).unwrap();
    let c = e.edit.doc.clip(e.clip).unwrap().1;
    assert_eq!((c.start, c.end), (3 * BAR, 7 * BAR));
    let order: Vec<ClipId> = e.edit.doc.tracks[0].clips.iter().map(|c| c.id).collect();
    assert_eq!(order, [clip, e.clip]);
}

#[test]
fn the_fields_of_the_note_list() {
    // The clip [1920, 5760); the take at 2400.
    let (doc, clip) = one_clip(vec![
        note(0, 100, 60),
        note(480, 100, 64),
        note(900, 50, 67),
    ]);
    let set = |field, value| set_notes_field(&doc, clip, &[0, 2], field, value).unwrap();
    let e = set(NoteField::Length, 240);
    assert_eq!(e.edit.step, StepKind::SetNotesField { clip, count: 2 });
    assert_eq!(e.selected, [0, 2]);
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(2400, 240, 60), (2880, 100, 64), (3300, 240, 67)]
    );
    // Not past the end of the clip, never under 1.
    let e = set(NoteField::Length, 100_000);
    assert_eq!(notes_of(&e.edit.doc, clip)[2], (3300, 5760 - 3300, 67));
    let e = set(NoteField::Length, -5);
    assert_eq!(notes_of(&e.edit.doc, clip)[0], (2400, 1, 60));
    let e = set(NoteField::Velocity, 300);
    let (_, take) = shown(&e.edit.doc, clip);
    assert_eq!(
        take.notes.iter().map(|n| n.velocity).collect::<Vec<_>>(),
        [127, 100, 127]
    );
    // The earliest goes to the value, the other keeps its distance.
    let e = set(NoteField::Start, 2000);
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(2000, 100, 60), (2880, 100, 64), (2900, 50, 67)]
    );
    // Only as far as the clip lets the notes go together.
    let e = set(NoteField::Start, 0);
    assert_eq!(notes_of(&e.edit.doc, clip)[0], (1920, 100, 60));
    // The lowest goes on the value, the other keeps the interval.
    let e = set(NoteField::Key, 72);
    assert_eq!(
        notes_of(&e.edit.doc, clip),
        [(2400, 100, 72), (2880, 100, 64), (3300, 50, 79)]
    );
    let e = set(NoteField::Key, 200);
    assert_eq!(notes_of(&e.edit.doc, clip)[2], (3300, 50, 127));
    assert_eq!(
        set_notes_field(&doc, clip, &[0], NoteField::Key, 60).unwrap_err(),
        EditError::Unchanged
    );
}
