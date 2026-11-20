use super::*;
use crate::edit::{NewTake, add_recording};
use crate::notes::{WriteAt, move_notes, write_note};
use crate::playback::{PlayKind, track_events};
use crate::song::{CC_SUSTAIN, ControlPoint, ControlValue, Note};
use crate::{InstrumentRef, TICKS_PER_QUARTER, TrackColor};

const T1: TrackId = TrackId(1);
const T2: TrackId = TrackId(2);
const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

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
        kind: crate::TrackKind::Keys,
        id: T2,
        name: "ベース".into(),
        color: TrackColor::for_index(1),
        instrument: InstrumentRef::SINE,
        channel: Default::default(),
        sends: [crate::Gain::SILENT; 2],
        muted: false,
        clips: Vec::new(),
        automation: Vec::new(),
    }));
    doc
}

/// Track 1: a clip over bar 2 with notes at its start and in its middle, and one over bar 5.
fn song() -> (Doc, ClipId, ClipId) {
    let (doc, a) = record(
        &two_tracks(),
        T1,
        take(BAR, BAR, vec![note(0, Q / 2, 60), note(2 * Q, Q / 2, 64)]),
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
    let e = move_clips(&doc, &[a], BAR as i64, 0, false).unwrap();
    assert_eq!(e.clips[0], a);
    assert_eq!(
        e.edit.step,
        StepKind::MoveClips {
            count: 1,
            copy: false
        }
    );
    assert_eq!(
        spans(&e.edit.doc, T1),
        [(a, 2 * BAR, 3 * BAR), (b, 4 * BAR, 5 * BAR)]
    );
    let (_, before) = doc.clip(a).unwrap();
    let (_, after) = e.edit.doc.clip(a).unwrap();
    assert!(Arc::ptr_eq(&before.keys()[0].take, &after.keys()[0].take));
    assert_eq!(
        ons(&e.edit.doc, T1),
        [(2 * BAR, 60), (2 * BAR + 2 * Q, 64), (4 * BAR, 67)]
    );
    assert_eq!(e.edit.doc.next_id, doc.next_id);
}

#[test]
fn a_move_that_would_overlap_does_nothing() {
    let (doc, a, _) = song();
    // Bars 4–5 would overlap the clip over bar 5 by one tick and more.
    assert_eq!(
        move_clips(&doc, &[a], 2 * BAR as i64 + 1, 0, false),
        Err(EditError::Overlap)
    );
    // Touching is not overlapping.
    let e = move_clips(&doc, &[a], 2 * BAR as i64, 0, false).unwrap();
    assert_eq!(spans(&e.edit.doc, T1)[0].1, 3 * BAR);
    // Past the other clip, the order of start is kept.
    let e = move_clips(&doc, &[a], 5 * BAR as i64, 0, false).unwrap();
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
    let e = move_clips(&doc, &[a], -10 * BAR as i64, 0, false).unwrap();
    assert_eq!(spans(&e.edit.doc, T1)[0].1, 0);
    let doc = e.edit.doc;
    assert_eq!(
        move_clips(&doc, &[a], -(BAR as i64), 0, false),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        move_clips(&doc, &[a], 0, 0, false),
        Err(EditError::Unchanged)
    );
}

#[test]
fn a_move_to_another_track() {
    let (doc, a, b) = song();
    let e = move_clips(&doc, &[a], 0, 1, false).unwrap();
    assert_eq!(
        e.edit.step,
        StepKind::MoveClips {
            count: 1,
            copy: false
        }
    );
    assert_eq!(spans(&e.edit.doc, T1), [(b, 4 * BAR, 5 * BAR)]);
    assert_eq!(spans(&e.edit.doc, T2), [(a, BAR, 2 * BAR)]);
    assert_eq!(ons(&e.edit.doc, T2), [(BAR, 60), (BAR + 2 * Q, 64)]);
    // Back onto track 1 where it would overlap the other clip.
    let doc = e.edit.doc;
    assert_eq!(
        move_clips(&doc, &[a], 3 * BAR as i64 + 5, -1, false),
        Err(EditError::Overlap)
    );
    assert_eq!(
        move_clips(&doc, &[a], 0, 8, false),
        Err(EditError::OffTheSong)
    );
}

#[test]
fn a_copy_shares_the_takes_and_gets_a_new_id() {
    let (doc, a, b) = song();
    let e = move_clips(&doc, &[a], 5 * BAR as i64, 0, true).unwrap();
    assert_ne!(e.clips[0], a);
    assert_eq!(e.clips[0], ClipId(doc.next_id));
    assert_eq!(e.edit.doc.next_id, doc.next_id + 1);
    assert_eq!(
        e.edit.step,
        StepKind::MoveClips {
            count: 1,
            copy: true
        }
    );
    assert_eq!(
        spans(&e.edit.doc, T1),
        [
            (a, BAR, 2 * BAR),
            (b, 4 * BAR, 5 * BAR),
            (e.clips[0], 6 * BAR, 7 * BAR)
        ]
    );
    let (_, orig) = e.edit.doc.clip(a).unwrap();
    let orig = orig.clone();
    let (_, copy) = e.edit.doc.clip(e.clips[0]).unwrap();
    assert!(Arc::ptr_eq(&orig.keys()[0].take, &copy.keys()[0].take));
    assert_eq!(
        (copy.name.as_str(), copy.active),
        (orig.name.as_str(), orig.active)
    );
    // A copy in place overlaps the clip it came from.
    assert_eq!(move_clips(&doc, &[a], 0, 0, true), Err(EditError::Overlap));
    // Editing the notes of one makes a new take for that one only.
    let doc = e.edit.doc;
    let edited = move_notes(&doc, e.clips[0], &[0], 10, 0, false).unwrap();
    let (_, orig2) = edited.edit.doc.clip(a).unwrap();
    let (_, copy2) = edited.edit.doc.clip(e.clips[0]).unwrap();
    assert!(Arc::ptr_eq(&orig.keys()[0].take, &orig2.keys()[0].take));
    assert!(!Arc::ptr_eq(&orig2.keys()[0].take, &copy2.keys()[0].take));
}

#[test]
fn duplicate_puts_a_copy_right_after() {
    let (doc, a, b) = song();
    let e = duplicate_clips(&doc, &[a]).unwrap();
    assert_eq!(e.edit.step, StepKind::DuplicateClips { count: 1 });
    assert_eq!(
        spans(&e.edit.doc, T1),
        [
            (a, BAR, 2 * BAR),
            (e.clips[0], 2 * BAR, 3 * BAR),
            (b, 4 * BAR, 5 * BAR)
        ]
    );
    let doc = e.edit.doc;
    let e2 = duplicate_clips(&doc, &[e.clips[0]]).unwrap();
    // The third one would start at bar 4, where the other clip is: no room after that.
    let doc = e2.edit.doc;
    assert_eq!(spans(&doc, T1)[2], (e2.clips[0], 3 * BAR, 4 * BAR));
    assert_eq!(
        duplicate_clips(&doc, &[e2.clips[0]]),
        Err(EditError::Overlap)
    );
}

/// Track 1: clips over bars 2 and 5 (as `song`). Track 2: a clip over bar 3.
fn song_two_tracks() -> (Doc, ClipId, ClipId, ClipId) {
    let (doc, a, b) = song();
    let (doc, c) = record(&doc, T2, take(2 * BAR, BAR, vec![note(0, 100, 40)]));
    (doc, a, b, c)
}

#[test]
fn a_group_moves_together_keeping_rows_and_distances() {
    let (doc, a, b, c) = song_two_tracks();
    // One row down, c would leave the tracks.
    assert_eq!(
        move_clips(&doc, &[a, c], 0, 1, false),
        Err(EditError::OffTheSong)
    );
    let e = move_clips(&doc, &[c, a], BAR as i64, 0, false).unwrap();
    assert_eq!(e.clips, [c, a]);
    assert_eq!(
        spans(&e.edit.doc, T1),
        [(a, 2 * BAR, 3 * BAR), (b, 4 * BAR, 5 * BAR)]
    );
    assert_eq!(spans(&e.edit.doc, T2), [(c, 3 * BAR, 4 * BAR)]);
    assert_eq!(e.edit.doc.next_id, doc.next_id);
    // Clips of the group do not stand in each other's way.
    let e = move_clips(&doc, &[a, b], 3 * BAR as i64, 0, false).unwrap();
    assert_eq!(
        spans(&e.edit.doc, T1),
        [(a, 4 * BAR, 5 * BAR), (b, 7 * BAR, 8 * BAR)]
    );
    // a alone 3 bars later overlaps b, which is not in the group.
    assert_eq!(
        move_clips(&doc, &[a], 3 * BAR as i64, 0, false),
        Err(EditError::Overlap)
    );
    // The group stops at the song's start as a whole.
    let e = move_clips(&doc, &[a, c], -10 * BAR as i64, 0, false).unwrap();
    assert_eq!(spans(&e.edit.doc, T1)[0], (a, 0, BAR));
    assert_eq!(spans(&e.edit.doc, T2), [(c, BAR, 2 * BAR)]);
    // Clips not there are left out, and one given twice counts once.
    let e = move_clips(&doc, &[ClipId(999), a, a], BAR as i64, 0, false).unwrap();
    assert_eq!(e.clips, [a]);
    assert_eq!(
        move_clips(&doc, &[ClipId(999)], BAR as i64, 0, false),
        Err(EditError::NoSuchClip(ClipId(999)))
    );
}

#[test]
fn a_group_copied_overlaps_its_own_originals() {
    let (doc, a, b, c) = song_two_tracks();
    let e = move_clips(&doc, &[a, c], BAR as i64, 0, true).unwrap();
    let [ca, cc] = e.clips[..] else { panic!() };
    assert_eq!((ca, cc), (ClipId(doc.next_id), ClipId(doc.next_id + 1)));
    assert_eq!(
        spans(&e.edit.doc, T1),
        [
            (a, BAR, 2 * BAR),
            (ca, 2 * BAR, 3 * BAR),
            (b, 4 * BAR, 5 * BAR)
        ]
    );
    assert_eq!(
        spans(&e.edit.doc, T2),
        [(c, 2 * BAR, 3 * BAR), (cc, 3 * BAR, 4 * BAR)]
    );
    // Half a bar later, each copy overlaps its own original.
    assert_eq!(
        move_clips(&doc, &[a, c], 2 * Q as i64, 0, true),
        Err(EditError::Overlap)
    );
}

#[test]
fn a_group_duplicated_goes_right_after_its_latest_end() {
    let (doc, a, b, c) = song_two_tracks();
    // a ends at bar 3, c at bar 4: the copies come 2 bars (bars 2 to 4) later.
    let e = duplicate_clips(&doc, &[a, c]).unwrap();
    assert_eq!(e.edit.step, StepKind::DuplicateClips { count: 2 });
    let [ca, cc] = e.clips[..] else { panic!() };
    assert_eq!(
        spans(&e.edit.doc, T1),
        [
            (a, BAR, 2 * BAR),
            (ca, 3 * BAR, 4 * BAR),
            (b, 4 * BAR, 5 * BAR)
        ]
    );
    assert_eq!(
        spans(&e.edit.doc, T2),
        [(c, 2 * BAR, 3 * BAR), (cc, 4 * BAR, 5 * BAR)]
    );
    // b and a: bars 2 to 5, so the copies are 4 bars later, in the order given.
    let e = duplicate_clips(&doc, &[b, a]).unwrap();
    assert_eq!(
        spans(&e.edit.doc, T1)[2..],
        [
            (e.clips[1], 5 * BAR, 6 * BAR),
            (e.clips[0], 8 * BAR, 9 * BAR)
        ]
    );
}

#[test]
fn copied_clips_paste_with_their_rows_and_distances() {
    let (doc, a, _, c) = song_two_tracks();
    let kept = copy_clips(&doc, &[c, a]).unwrap();
    // The earliest first.
    assert_eq!(
        kept.iter()
            .map(|k| (k.clip.id, k.row, k.after))
            .collect::<Vec<_>>(),
        [(a, 0, 0), (c, 1, BAR)]
    );
    // On track 1 at bar 7: a's copy there, c's a row lower and a bar later.
    let e = paste_clips(&doc, &kept, T1, 6 * BAR).unwrap();
    assert_eq!(e.edit.step, StepKind::PasteClips { count: 2 });
    let [ca, cc] = e.clips[..] else { panic!() };
    assert_eq!(spans(&e.edit.doc, T1)[2], (ca, 6 * BAR, 7 * BAR));
    assert_eq!(spans(&e.edit.doc, T2)[1], (cc, 7 * BAR, 8 * BAR));
    let (_, orig) = doc.clip(a).unwrap();
    let (_, copy) = e.edit.doc.clip(ca).unwrap();
    assert!(Arc::ptr_eq(&orig.keys()[0].take, &copy.keys()[0].take));
    // On track 2, c's copy would be below the last track.
    assert_eq!(
        paste_clips(&doc, &kept, T2, 6 * BAR),
        Err(EditError::OffTheSong)
    );
    assert_eq!(
        paste_clips(&doc, &kept, T1, BAR + 10),
        Err(EditError::Overlap)
    );
    // What was kept stays as it was after the clips are gone.
    let gone = crate::edit::delete_clips(&doc, &[a, c], false).unwrap().doc;
    let e = paste_clips(&gone, &kept, T1, 0).unwrap();
    assert_eq!(spans(&e.edit.doc, T2), [(e.clips[1], BAR, 2 * BAR)]);
    assert_eq!(paste_clips(&doc, &[], T1, 0), Err(EditError::Unchanged));
}

/// A clip kept, then undone away, then another recording: the new takes get ids of their own,
/// so the pasted take shares its id with no other take.
#[test]
fn pasted_takes_never_share_an_id_with_another_take() {
    let (mut doc, _, _) = song();
    let mut h = crate::History::new();
    let (e, added) =
        add_recording(&doc, T2, vec![take(0, BAR, vec![note(0, 100, 50)])], 0).unwrap();
    h.apply(&mut doc, e);
    let kept = copy_clips(&doc, &[added.clip]).unwrap();
    let kept_take = kept[0].clip.keys()[0].take.clone();
    h.undo(&mut doc);
    let (e, other) =
        add_recording(&doc, T2, vec![take(0, BAR, vec![note(10, 100, 51)])], 0).unwrap();
    h.apply(&mut doc, e);
    assert_ne!(other.clip, added.clip);
    let (_, oc) = doc.clip(other.clip).unwrap();
    assert_ne!(oc.keys()[0].take.id, kept_take.id);
    let pasted = paste_clips(&doc, &kept, T1, 8 * BAR).unwrap();
    let (_, p) = pasted.edit.doc.clip(pasted.clips[0]).unwrap();
    assert!(Arc::ptr_eq(&p.keys()[0].take, &kept_take));
    assert!(pasted.clips[0].0 > oc.keys()[0].take.id.0);
    assert!(pasted.clips[0].0 < pasted.edit.doc.next_id);
}

#[test]
fn trimming_the_start_keeps_the_notes_where_they_are() {
    let (doc, a, _) = song();
    let before = positions(&doc, a);
    let e = trim_clip(&doc, a, ClipEdge::Start, Q as i64).unwrap();
    assert_eq!(
        e.step,
        StepKind::TrimClip {
            clip: a,
            edge: ClipEdge::Start
        }
    );
    let (_, c) = e.doc.clip(a).unwrap();
    assert_eq!(
        (c.start, c.end, c.keys()[0].offset),
        (BAR + Q, 2 * BAR, -(Q as i64))
    );
    assert_eq!(positions(&e.doc, a), before);
    // The note at the start is trimmed off: it does not sound, but it is still in the take.
    assert_eq!(ons(&e.doc, T1)[0], (BAR + 2 * Q, 64));
    assert_eq!(c.keys()[0].take.notes.len(), 2);
    // Pulled out again, it comes back.
    let back = trim_clip(&e.doc, a, ClipEdge::Start, -(Q as i64)).unwrap();
    assert_eq!(back.doc, doc);
    // Pulled out further than the take: room with nothing in it.
    let wide = trim_clip(&e.doc, a, ClipEdge::Start, -(2 * Q as i64 + 40)).unwrap();
    let (_, c) = wide.doc.clip(a).unwrap();
    assert_eq!((c.start, c.keys()[0].offset), (BAR - Q - 40, Q as i64 + 40));
    assert_eq!(positions(&wide.doc, a), before);
}

#[test]
fn trimming_the_end_keeps_the_notes_and_takes() {
    let (doc, a, _) = song();
    let e = trim_clip(&doc, a, ClipEdge::End, -(2 * Q as i64 + 40)).unwrap();
    let (_, c) = e.doc.clip(a).unwrap();
    assert_eq!(
        (c.start, c.end, c.keys()[0].offset),
        (BAR, BAR + 2 * Q - 40, 0)
    );
    assert_eq!(ons(&e.doc, T1), [(BAR, 60), (4 * BAR, 67)]);
    assert_eq!(positions(&e.doc, a), positions(&doc, a));
    // Past the end of the take.
    let long = trim_clip(&doc, a, ClipEdge::End, 900).unwrap();
    let (_, c) = long.doc.clip(a).unwrap();
    assert_eq!(c.end, 2 * BAR + 900);
    assert_eq!(c.keys()[0].take.length, BAR);
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
    assert_eq!(c.keys()[0].offset, 2 * BAR as i64);
    // The first one's start stops at the song's start.
    let e = trim_clip(&doc, a, ClipEdge::Start, -10 * BAR as i64).unwrap();
    let (_, c) = e.doc.clip(a).unwrap();
    assert_eq!((c.start, c.keys()[0].offset), (0, BAR as i64));
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
    let doc = trim_clip(&doc, a, ClipEdge::Start, 2 * Q as i64)
        .unwrap()
        .doc;
    let doc = move_clips(&doc, &[a], -10 * BAR as i64, 0, false)
        .unwrap()
        .edit
        .doc;
    let (_, c) = doc.clip(a).unwrap();
    // The take's tick 0 is before the song's.
    assert_eq!((c.start, c.zero(&c.keys()[0])), (0, -(2 * Q as i64)));
    assert_eq!(positions(&doc, a), [(-(2 * Q as i64), 60), (0, 64)]);
    assert_eq!(ons(&doc, T1)[0], (0, 64));
    // Edits of the notes keep the hidden one where it is.
    let e = move_notes(&doc, a, &[1], 100, 2, false).unwrap();
    assert_eq!(
        positions(&e.edit.doc, a),
        [(-(2 * Q as i64), 60), (100, 66)]
    );
    let e = write_note(&e.edit.doc, WriteAt::Clip(a), note(500, 50, 70)).unwrap();
    assert_eq!(
        positions(&e.edit.doc, a),
        [(-(2 * Q as i64), 60), (100, 66), (500, 70)]
    );
    // Pulled back out, the hidden note comes back where it was.
    let doc = move_clips(&e.edit.doc, &[a], 3 * BAR as i64, 0, false)
        .unwrap()
        .edit
        .doc;
    let doc = trim_clip(&doc, a, ClipEdge::Start, -(2 * Q as i64))
        .unwrap()
        .doc;
    assert_eq!(ons(&doc, T1)[0], (3 * BAR - 2 * Q, 60));
}

#[test]
fn recording_over_a_trimmed_clip_keeps_every_note_in_place() {
    let (doc, a, _) = song();
    let doc = trim_clip(&doc, a, ClipEdge::Start, 2 * Q as i64)
        .unwrap()
        .doc;
    let doc = trim_clip(&doc, a, ClipEdge::End, -(Q as i64)).unwrap().doc;
    let before = positions(&doc, a);
    // A take over the end of the clip: the clips merge into [BAR + 2 Q, BAR + 3 Q + 160).
    let (doc, merged) = record(&doc, T1, take(BAR + 3 * Q - 40, 200, vec![note(0, 50, 72)]));
    assert_eq!(merged, a);
    let (_, c) = doc.clip(a).unwrap();
    assert_eq!((c.start, c.end), (BAR + 2 * Q, BAR + 3 * Q + 160));
    assert_eq!(c.keys()[0].offset, -(2 * Q as i64));
    let first = &c.keys()[0];
    let at: Vec<(i64, u8)> = first
        .take
        .notes
        .iter()
        .map(|n| (c.zero(first) + n.start as i64, n.key))
        .collect();
    assert_eq!(at, before);
    assert_eq!(positions(&doc, a), [((BAR + 3 * Q - 40) as i64, 72)]);
    // A take before the start that overlaps it: the hidden head shows again.
    let (doc, _) = record(&doc, T1, take(BAR, 2 * Q + 40, vec![note(10, 50, 50)]));
    let (_, c) = doc.clip(a).unwrap();
    assert_eq!(c.start, BAR);
    assert_eq!(c.keys()[0].offset, 0);
    let doc = crate::edit::switch_take(&doc, a, 0).unwrap().doc;
    assert_eq!(positions(&doc, a), before);
    assert_eq!(ons(&doc, T1)[..2], [(BAR, 60), (BAR + 2 * Q, 64)]);
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
            notes: vec![
                note(0, 2 * Q, 60),
                note(Q, 100, 62),
                note(2 * Q + 40, 100, 64),
            ],
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
                    tick: 5 * Q / 2,
                    value: ControlValue::Cc {
                        number: CC_SUSTAIN,
                        value: 0,
                    },
                },
            ],
        },
    );
    let doc = trim_clip(&doc, a, ClipEdge::Start, 5 * Q as i64 / 4)
        .unwrap()
        .doc;
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
        (BAR + 5 * Q / 4, "cc64 127"),
        (BAR + 2 * Q + 40, "on 64"),
        (BAR + 2 * Q + 140, "off 64"),
        (BAR + 5 * Q / 2, "cc64 0"),
    ]
    .iter()
    .map(|&(t, s)| (t, s.to_owned()))
    .collect();
    assert_eq!(short, want);
}

#[test]
fn a_keys_clip_cut_in_two_ends_the_notes_over_the_cut_in_the_left_half() {
    // Bar 2: a note over the cut (at the half bar), one before it and one after it.
    let (doc, a) = record(
        &two_tracks(),
        T1,
        take(
            BAR,
            BAR,
            vec![note(0, Q, 60), note(Q, 2 * Q, 62), note(3 * Q, Q, 64)],
        ),
    );
    let (_, old) = doc.clip(a).unwrap();
    let old_take = old.active_take().take.clone();
    let cut = split_clips(&doc, &[a], BAR + 2 * Q).unwrap();
    assert_eq!(cut.edit.step, StepKind::SplitClips { count: 1 });
    let new = &cut.edit.doc;
    let (_, left) = new.clip(a).unwrap();
    let (_, right) = new.clip(cut.right[0]).unwrap();
    assert_eq!(
        spans(new, T1),
        [(a, BAR, BAR + 2 * Q), (cut.right[0], BAR + 2 * Q, 2 * BAR)]
    );
    // The left half has a take with a new id, the right half the take as it was.
    assert!(Arc::ptr_eq(&right.active_take().take, &old_take));
    let left_take = &left.active_take().take;
    assert_ne!(left_take.id, old_take.id);
    assert!(left_take.id.0 >= doc.next_id);
    assert_eq!(
        left_take
            .notes
            .iter()
            .map(|n| (n.start, n.length))
            .collect::<Vec<_>>(),
        [(0, Q), (Q, Q), (3 * Q, Q)]
    );
    assert_eq!(left.note_count(), 2);
    assert_eq!(right.note_count(), 1);
    // What sounds: every note once, the one over the cut ending at it.
    assert_eq!(ons(new, T1), [(BAR, 60), (BAR + Q, 62), (BAR + 3 * Q, 64)]);
    assert_eq!(positions(new, cut.right[0]), positions(&doc, a));

    // A cut that no note is over makes no take.
    let cut = split_clips(&doc, &[a], BAR + Q).unwrap();
    let (_, left) = cut.edit.doc.clip(a).unwrap();
    assert!(Arc::ptr_eq(&left.active_take().take, &old_take));
}

#[test]
fn clips_of_several_tracks_are_cut_as_one_step() {
    let (doc, a, _) = song();
    let (doc, c) = record(&doc, T2, take(BAR, 2 * BAR, vec![note(0, 100, 40)]));
    let cut = split_clips(&doc, &[a, c, ClipId(999)], BAR + Q).unwrap();
    assert_eq!(cut.edit.step, StepKind::SplitClips { count: 2 });
    assert_eq!(cut.left, [a, c]);
    assert_eq!(spans(&cut.edit.doc, T1).len(), 3);
    assert_eq!(spans(&cut.edit.doc, T2).len(), 2);
    let mut now = doc.clone();
    let mut history = crate::History::new();
    history.apply(&mut now, cut.edit);
    assert_eq!(history.undo_count(), 1);
    history.undo(&mut now);
    // Ids are never given twice: the count stays where the step left it.
    assert!(now.next_id > doc.next_id);
    now.next_id = doc.next_id;
    assert!(now == doc);
}
