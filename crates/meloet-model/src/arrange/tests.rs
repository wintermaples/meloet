use super::*;
use crate::edit::{NewTake, add_recording};
use crate::notes::{WriteAt, move_notes, write_note};
use crate::playback::{PlayKind, track_events};
use crate::song::{CC_SUSTAIN, ControlPoint, ControlValue, Note};
use crate::{InstrumentRef, TrackColor};

const T1: TrackId = TrackId(1);
const T2: TrackId = TrackId(2);
const BAR: u64 = 1920;

fn note(start: u64, length: u64, key: u8) -> Note {
    Note {
        from: None,
        start,
        length,
        key,
        velocity: 80,
    }
}

fn take(origin: u64, length: u64, notes: Vec<Note>) -> NewTake {
    NewTake {
        origin,
        length,
        notes,
        controls: Vec::new(),
    }
}

fn record(doc: &Doc, track: TrackId, t: NewTake) -> (Doc, ClipId) {
    let (e, added) = add_recording(doc, track, vec![t], 0).unwrap();
    (e.doc, added.clip)
}

/// Two tracks, no clips.
fn two_tracks() -> Doc {
    let mut doc = Doc::new_default();
    doc.tracks.push(Arc::new(Track {
        id: T2,
        name: "ベース".into(),
        color: TrackColor::for_index(1),
        instrument: InstrumentRef::Placeholder,
        gain_db: 0.0,
        muted: false,
        clips: Vec::new(),
    }));
    doc
}

/// Track 1: a clip over bar 2 with notes at its start and in its middle, and one over bar 5.
fn song() -> (Doc, ClipId, ClipId) {
    let (doc, a) = record(
        &two_tracks(),
        T1,
        take(BAR, BAR, vec![note(0, 240, 60), note(960, 240, 64)]),
    );
    let (doc, b) = record(&doc, T1, take(4 * BAR, BAR, vec![note(0, 100, 67)]));
    (doc, a, b)
}

fn spans(doc: &Doc, track: TrackId) -> Vec<(ClipId, u64, u64)> {
    doc.track(track)
        .unwrap()
        .clips
        .iter()
        .map(|c| (c.id, c.start, c.end))
        .collect()
}

/// Song positions of the notes of the clip's shown take, hidden ones too.
fn positions(doc: &Doc, clip: ClipId) -> Vec<(i64, u8)> {
    let (_, c) = doc.clip(clip).unwrap();
    let ct = c.active_take();
    ct.take
        .notes
        .iter()
        .map(|n| (c.zero(ct) + n.start as i64, n.key))
        .collect()
}

fn ons(doc: &Doc, track: TrackId) -> Vec<(u64, u8)> {
    track_events(doc, track)
        .into_iter()
        .filter_map(|e| match e.kind {
            PlayKind::NoteOn { key, .. } => Some((e.tick, key)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_move_keeps_the_id_and_the_takes() {
    let (doc, a, b) = song();
    let e = move_clip(&doc, a, BAR as i64, T1, false).unwrap();
    assert_eq!(e.clip, a);
    assert_eq!(e.edit.step, StepKind::MoveClip { clip: a, track: T1 });
    assert_eq!(
        spans(&e.edit.doc, T1),
        [(a, 2 * BAR, 3 * BAR), (b, 4 * BAR, 5 * BAR)]
    );
    let (_, before) = doc.clip(a).unwrap();
    let (_, after) = e.edit.doc.clip(a).unwrap();
    assert!(Arc::ptr_eq(&before.takes[0].take, &after.takes[0].take));
    assert_eq!(
        ons(&e.edit.doc, T1),
        [(2 * BAR, 60), (2 * BAR + 960, 64), (4 * BAR, 67)]
    );
    assert_eq!(e.edit.doc.next_id, doc.next_id);
}

#[test]
fn a_move_that_would_overlap_does_nothing() {
    let (doc, a, _) = song();
    // Bars 4–5 would overlap the clip over bar 5 by one tick and more.
    assert_eq!(
        move_clip(&doc, a, 2 * BAR as i64 + 1, T1, false),
        Err(EditError::Overlap)
    );
    // Touching is not overlapping.
    let e = move_clip(&doc, a, 2 * BAR as i64, T1, false).unwrap();
    assert_eq!(spans(&e.edit.doc, T1)[0].1, 3 * BAR);
    // Past the other clip, the order of start is kept.
    let e = move_clip(&doc, a, 5 * BAR as i64, T1, false).unwrap();
    assert_eq!(
        spans(&e.edit.doc, T1)
            .iter()
            .map(|s| s.1)
            .collect::<Vec<_>>(),
        [4 * BAR, 6 * BAR]
    );
}

#[test]
fn a_move_stops_at_the_song_start() {
    let (doc, a, _) = song();
    let e = move_clip(&doc, a, -10 * BAR as i64, T1, false).unwrap();
    assert_eq!(spans(&e.edit.doc, T1)[0].1, 0);
    let doc = e.edit.doc;
    assert_eq!(
        move_clip(&doc, a, -(BAR as i64), T1, false),
        Err(EditError::Unchanged)
    );
    assert_eq!(move_clip(&doc, a, 0, T1, false), Err(EditError::Unchanged));
}

#[test]
fn a_move_to_another_track() {
    let (doc, a, b) = song();
    let e = move_clip(&doc, a, 0, T2, false).unwrap();
    assert_eq!(e.edit.step, StepKind::MoveClip { clip: a, track: T2 });
    assert_eq!(spans(&e.edit.doc, T1), [(b, 4 * BAR, 5 * BAR)]);
    assert_eq!(spans(&e.edit.doc, T2), [(a, BAR, 2 * BAR)]);
    assert_eq!(ons(&e.edit.doc, T2), [(BAR, 60), (BAR + 960, 64)]);
    // Back onto track 1 where it would overlap the other clip.
    let doc = e.edit.doc;
    assert_eq!(
        move_clip(&doc, a, 3 * BAR as i64 + 5, T1, false),
        Err(EditError::Overlap)
    );
    assert_eq!(
        move_clip(&doc, a, 0, TrackId(9), false),
        Err(EditError::NoSuchTrack(TrackId(9)))
    );
}

#[test]
fn a_copy_shares_the_takes_and_gets_a_new_id() {
    let (doc, a, b) = song();
    let e = move_clip(&doc, a, 5 * BAR as i64, T1, true).unwrap();
    assert_ne!(e.clip, a);
    assert_eq!(e.clip, ClipId(doc.next_id));
    assert_eq!(e.edit.doc.next_id, doc.next_id + 1);
    assert_eq!(e.edit.step, StepKind::CopyClip { clip: a, track: T1 });
    assert_eq!(
        spans(&e.edit.doc, T1),
        [
            (a, BAR, 2 * BAR),
            (b, 4 * BAR, 5 * BAR),
            (e.clip, 6 * BAR, 7 * BAR)
        ]
    );
    let (_, orig) = e.edit.doc.clip(a).unwrap();
    let orig = orig.clone();
    let (_, copy) = e.edit.doc.clip(e.clip).unwrap();
    assert!(Arc::ptr_eq(&orig.takes[0].take, &copy.takes[0].take));
    assert_eq!(
        (copy.name.as_str(), copy.active),
        (orig.name.as_str(), orig.active)
    );
    // A copy in place overlaps the clip it came from.
    assert_eq!(move_clip(&doc, a, 0, T1, true), Err(EditError::Overlap));
    // Editing the notes of one makes a new take for that one only.
    let doc = e.edit.doc;
    let edited = move_notes(&doc, e.clip, &[0], 10, 0, false).unwrap();
    let (_, orig2) = edited.edit.doc.clip(a).unwrap();
    let (_, copy2) = edited.edit.doc.clip(e.clip).unwrap();
    assert!(Arc::ptr_eq(&orig.takes[0].take, &orig2.takes[0].take));
    assert!(!Arc::ptr_eq(&orig2.takes[0].take, &copy2.takes[0].take));
}

#[test]
fn duplicate_puts_a_copy_right_after() {
    let (doc, a, b) = song();
    let e = duplicate_clip(&doc, a).unwrap();
    assert_eq!(e.edit.step, StepKind::DuplicateClip { clip: a });
    assert_eq!(
        spans(&e.edit.doc, T1),
        [
            (a, BAR, 2 * BAR),
            (e.clip, 2 * BAR, 3 * BAR),
            (b, 4 * BAR, 5 * BAR)
        ]
    );
    let doc = e.edit.doc;
    let e2 = duplicate_clip(&doc, e.clip).unwrap();
    // The third one would start at bar 4, where the other clip is: no room after that.
    let doc = e2.edit.doc;
    assert_eq!(spans(&doc, T1)[2], (e2.clip, 3 * BAR, 4 * BAR));
    assert_eq!(duplicate_clip(&doc, e2.clip), Err(EditError::Overlap));
}

#[test]
fn trimming_the_start_keeps_the_notes_where_they_are() {
    let (doc, a, _) = song();
    let before = positions(&doc, a);
    let e = trim_clip(&doc, a, ClipEdge::Start, 480).unwrap();
    assert_eq!(
        e.step,
        StepKind::TrimClip {
            clip: a,
            edge: ClipEdge::Start
        }
    );
    let (_, c) = e.doc.clip(a).unwrap();
    assert_eq!(
        (c.start, c.end, c.takes[0].offset),
        (BAR + 480, 2 * BAR, -480)
    );
    assert_eq!(positions(&e.doc, a), before);
    // The note at the start is trimmed off: it does not sound, but it is still in the take.
    assert_eq!(ons(&e.doc, T1)[0], (BAR + 960, 64));
    assert_eq!(c.takes[0].take.notes.len(), 2);
    // Pulled out again, it comes back.
    let back = trim_clip(&e.doc, a, ClipEdge::Start, -480).unwrap();
    assert_eq!(back.doc, doc);
    // Pulled out further than the take: room with nothing in it.
    let wide = trim_clip(&e.doc, a, ClipEdge::Start, -1000).unwrap();
    let (_, c) = wide.doc.clip(a).unwrap();
    assert_eq!((c.start, c.takes[0].offset), (BAR - 520, 520));
    assert_eq!(positions(&wide.doc, a), before);
}

#[test]
fn trimming_the_end_keeps_the_notes_and_takes() {
    let (doc, a, _) = song();
    let e = trim_clip(&doc, a, ClipEdge::End, -1000).unwrap();
    let (_, c) = e.doc.clip(a).unwrap();
    assert_eq!((c.start, c.end, c.takes[0].offset), (BAR, BAR + 920, 0));
    assert_eq!(ons(&e.doc, T1), [(BAR, 60), (4 * BAR, 67)]);
    assert_eq!(positions(&e.doc, a), positions(&doc, a));
    // Past the end of the take.
    let long = trim_clip(&doc, a, ClipEdge::End, 900).unwrap();
    let (_, c) = long.doc.clip(a).unwrap();
    assert_eq!(c.end, 2 * BAR + 900);
    assert_eq!(c.takes[0].take.length, BAR);
}

#[test]
fn trims_stop_short_of_the_neighbours_and_keep_a_tick() {
    let (doc, a, b) = song();
    // The end of the first stops at the start of the second.
    let e = trim_clip(&doc, a, ClipEdge::End, 10 * BAR as i64).unwrap();
    assert_eq!(spans(&e.doc, T1)[0], (a, BAR, 4 * BAR));
    assert_eq!(
        trim_clip(&e.doc, b, ClipEdge::Start, -1),
        Err(EditError::Unchanged)
    );
    // The start of the second stops at the end of the first.
    let e = trim_clip(&doc, b, ClipEdge::Start, -10 * BAR as i64).unwrap();
    assert_eq!(spans(&e.doc, T1)[1], (b, 2 * BAR, 5 * BAR));
    let (_, c) = e.doc.clip(b).unwrap();
    assert_eq!(c.takes[0].offset, 2 * BAR as i64);
    // The first one's start stops at the song's start.
    let e = trim_clip(&doc, a, ClipEdge::Start, -10 * BAR as i64).unwrap();
    let (_, c) = e.doc.clip(a).unwrap();
    assert_eq!((c.start, c.takes[0].offset), (0, BAR as i64));
    // 1 tick at the least.
    let e = trim_clip(&doc, a, ClipEdge::Start, 10 * BAR as i64).unwrap();
    assert_eq!(spans(&e.doc, T1)[0], (a, 2 * BAR - 1, 2 * BAR));
    let e = trim_clip(&doc, a, ClipEdge::End, -10 * BAR as i64).unwrap();
    assert_eq!(spans(&e.doc, T1)[0], (a, BAR, BAR + 1));
    assert_eq!(
        trim_clip(&doc, a, ClipEdge::End, 0),
        Err(EditError::Unchanged)
    );
}

#[test]
fn a_trimmed_clip_moved_to_the_song_start_keeps_its_hidden_head() {
    let (doc, a, _) = song();
    let doc = trim_clip(&doc, a, ClipEdge::Start, 960).unwrap().doc;
    let doc = move_clip(&doc, a, -10 * BAR as i64, T1, false)
        .unwrap()
        .edit
        .doc;
    let (_, c) = doc.clip(a).unwrap();
    // The take's tick 0 is before the song's.
    assert_eq!((c.start, c.zero(&c.takes[0])), (0, -960));
    assert_eq!(positions(&doc, a), [(-960, 60), (0, 64)]);
    assert_eq!(ons(&doc, T1)[0], (0, 64));
    // Edits of the notes keep the hidden one where it is.
    let e = move_notes(&doc, a, &[1], 100, 2, false).unwrap();
    assert_eq!(positions(&e.edit.doc, a), [(-960, 60), (100, 66)]);
    let e = write_note(&e.edit.doc, WriteAt::Clip(a), note(500, 50, 70)).unwrap();
    assert_eq!(
        positions(&e.edit.doc, a),
        [(-960, 60), (100, 66), (500, 70)]
    );
    // Pulled back out, the hidden note comes back where it was.
    let doc = move_clip(&e.edit.doc, a, 3 * BAR as i64, T1, false)
        .unwrap()
        .edit
        .doc;
    let doc = trim_clip(&doc, a, ClipEdge::Start, -960).unwrap().doc;
    assert_eq!(ons(&doc, T1)[0], (3 * BAR - 960, 60));
}

#[test]
fn recording_over_a_trimmed_clip_keeps_every_note_in_place() {
    let (doc, a, _) = song();
    let doc = trim_clip(&doc, a, ClipEdge::Start, 960).unwrap().doc;
    let doc = trim_clip(&doc, a, ClipEdge::End, -480).unwrap().doc;
    let before = positions(&doc, a);
    // A take over the end of the clip: the clips merge into [BAR + 960, BAR + 1600).
    let (doc, merged) = record(&doc, T1, take(BAR + 1400, 200, vec![note(0, 50, 72)]));
    assert_eq!(merged, a);
    let (_, c) = doc.clip(a).unwrap();
    assert_eq!((c.start, c.end), (BAR + 960, BAR + 1600));
    assert_eq!(c.takes[0].offset, -960);
    let first = &c.takes[0];
    let at: Vec<(i64, u8)> = first
        .take
        .notes
        .iter()
        .map(|n| (c.zero(first) + n.start as i64, n.key))
        .collect();
    assert_eq!(at, before);
    assert_eq!(positions(&doc, a), [(BAR as i64 + 1400, 72)]);
    // A take before the start that overlaps it: the hidden head shows again.
    let (doc, _) = record(&doc, T1, take(BAR, 1000, vec![note(10, 50, 50)]));
    let (_, c) = doc.clip(a).unwrap();
    assert_eq!(c.start, BAR);
    assert_eq!(c.takes[0].offset, 0);
    let doc = crate::edit::switch_take(&doc, a, 0).unwrap().doc;
    assert_eq!(positions(&doc, a), before);
    assert_eq!(ons(&doc, T1)[..2], [(BAR, 60), (BAR + 960, 64)]);
}

#[test]
fn a_trimmed_head_plays_only_inside_the_clip() {
    let doc = two_tracks();
    let (doc, a) = record(
        &doc,
        T1,
        NewTake {
            origin: BAR,
            length: BAR,
            notes: vec![note(0, 960, 60), note(480, 100, 62), note(1000, 100, 64)],
            controls: vec![
                ControlPoint {
                    tick: 100,
                    value: ControlValue::Cc {
                        number: CC_SUSTAIN,
                        value: 127,
                    },
                },
                ControlPoint {
                    tick: 200,
                    value: ControlValue::PitchBend(500),
                },
                ControlPoint {
                    tick: 300,
                    value: ControlValue::PitchBend(0),
                },
                ControlPoint {
                    tick: 1200,
                    value: ControlValue::Cc {
                        number: CC_SUSTAIN,
                        value: 0,
                    },
                },
            ],
        },
    );
    let doc = trim_clip(&doc, a, ClipEdge::Start, 600).unwrap().doc;
    let short: Vec<(u64, String)> = track_events(&doc, T1)
        .into_iter()
        .map(|e| {
            let what = match e.kind {
                PlayKind::NoteOn { key, .. } => format!("on {key}"),
                PlayKind::NoteOff { key } => format!("off {key}"),
                PlayKind::Control(ControlValue::Cc { number, value }) => {
                    format!("cc{number} {value}")
                }
                PlayKind::Control(ControlValue::PitchBend(v)) => format!("bend {v}"),
            };
            (e.tick, what)
        })
        .collect();
    // The note held over the start (from 0) and the one before it do not sound; the pedal
    // still down at the start is put there; the bend already back at 0 is not.
    let want: Vec<(u64, String)> = [
        (BAR + 600, "cc64 127"),
        (BAR + 1000, "on 64"),
        (BAR + 1100, "off 64"),
        (BAR + 1200, "cc64 0"),
    ]
    .iter()
    .map(|&(t, s)| (t, s.to_owned()))
    .collect();
    assert_eq!(short, want);
}
