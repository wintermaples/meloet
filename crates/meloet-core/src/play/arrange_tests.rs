use meloet_protocol::{ClipEdge, ClipSpot, Command, SelectMode};

use super::tests::{cmd, ready, record_once, report, started, status};
use super::*;
use crate::state::{Effect, Model};

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn clips(m: &Model) -> Vec<ClipView> {
    m.view().song.tracks[0].clips.clone()
}

fn sequences(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::Engine(EngineOp::Sequence { track: 0, .. })))
        .count()
}

/// One clip over the first bar, recorded with a note at 0.
fn one_clip() -> (Model, ClipView) {
    let mut m = ready();
    record_once(&mut m, &[(60, 0.0, 100.0)], BAR as f64);
    let c = clips(&m)[0].clone();
    (m, c)
}

#[test]
fn a_move_is_one_step_and_selects_the_clip() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    cmd(m, Command::SelectClip { clip: None });
    let steps = m.view().song.undo_count;
    let effects = cmd(
        m,
        Command::MoveClips {
            clips: vec![c.id],
            ticks: 2 * BAR as i64,
            tracks: 0,
            copy: false,
        },
    );
    assert_eq!(sequences(&effects), 1);
    let moved = clips(m)[0].clone();
    assert_eq!((moved.id, moved.start), (c.id, c.start + 2 * BAR));
    assert_eq!(moved.take_id, c.take_id);
    assert_eq!(m.view().song.undo_count, steps + 1);
    assert_eq!(m.view().arrange.selected_clip, Some(c.id));
    assert_eq!(m.view().arrange.piano_roll_clip, Some(c.id));
    // Nowhere to go: no step.
    cmd(
        m,
        Command::MoveClips {
            clips: vec![c.id],
            ticks: 0,
            tracks: 0,
            copy: false,
        },
    );
    cmd(
        m,
        Command::MoveClips {
            clips: vec![c.id],
            ticks: 0,
            tracks: 6,
            copy: false,
        },
    );
    assert_eq!(m.view().song.undo_count, steps + 1);
    cmd(m, Command::Undo);
    assert_eq!(clips(m)[0].start, c.start);
}

#[test]
fn a_copy_and_a_duplicate_are_selected_and_shown() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    cmd(
        m,
        Command::MoveClips {
            clips: vec![c.id],
            ticks: 4 * BAR as i64,
            tracks: 0,
            copy: true,
        },
    );
    let all = clips(m);
    assert_eq!(all.len(), 2);
    let copy = all[1].clone();
    assert_ne!(copy.id, c.id);
    assert_eq!(copy.take_id, c.take_id, "the take is shared");
    assert_eq!(m.view().arrange.selected_clip, Some(copy.id));
    assert_eq!(m.view().arrange.piano_roll_clip, Some(copy.id));
    cmd(m, Command::DuplicateClips { clips: vec![c.id] });
    let all = clips(m);
    assert_eq!(all.len(), 3);
    assert_eq!(all[1].start, c.end);
    assert_eq!(m.view().arrange.selected_clip, Some(all[1].id));
    // The first duplicate is right after it now: no room.
    let steps = m.view().song.undo_count;
    cmd(m, Command::DuplicateClips { clips: vec![c.id] });
    assert_eq!(m.view().song.undo_count, steps);
    // Undo takes the copy away, and the piano roll with it.
    cmd(m, Command::Undo);
    assert_eq!(clips(m).len(), 2);
    assert_eq!(m.view().arrange.piano_roll_clip, None);
}

#[test]
fn a_trim_keeps_the_notes_in_place_and_tells_the_screen_the_offset() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    let effects = cmd(
        m,
        Command::TrimClip {
            clip: c.id,
            edge: ClipEdge::Start,
            ticks: Q as i64,
        },
    );
    assert_eq!(sequences(&effects), 1);
    let t = clips(m)[0].clone();
    assert_eq!((t.start, t.end), (c.start + Q, c.end));
    assert_eq!(t.take_offset, c.take_offset - Q as i64);
    assert_eq!(t.take_id, c.take_id, "the take does not change");
    assert_eq!(
        t.start as i64 + t.take_offset,
        c.start as i64 + c.take_offset
    );
    // The note at the head no longer sounds.
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Engine(EngineOp::Sequence { track: 0, events }) if events.is_empty()
    )));
    cmd(
        m,
        Command::TrimClip {
            clip: c.id,
            edge: ClipEdge::Start,
            ticks: -(Q as i64),
        },
    );
    assert_eq!(clips(m)[0], c);
}

#[test]
fn nothing_changes_clips_while_recording() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    cmd(m, Command::Locate { tick: 4 * BAR });
    cmd(m, Command::Record);
    started(m);
    let before = m.view().song;
    for command in [
        Command::MoveClips {
            clips: vec![c.id],
            ticks: BAR as i64,
            tracks: 0,
            copy: false,
        },
        Command::MoveClips {
            clips: vec![c.id],
            ticks: 8 * BAR as i64,
            tracks: 0,
            copy: true,
        },
        Command::DuplicateClips { clips: vec![c.id] },
        Command::TrimClip {
            clip: c.id,
            edge: ClipEdge::End,
            ticks: -10,
        },
        Command::CopyClips { clips: vec![c.id] },
        Command::PasteClips {
            to: Some(ClipSpot {
                track: 1,
                tick: 12 * BAR,
            }),
        },
        Command::CutClips { clips: vec![c.id] },
        Command::SelectAllClips,
        Command::DeleteSelected,
    ] {
        cmd(m, command);
    }
    assert_eq!(m.view().song, before);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 100,
            tick: 4.0 * BAR as f64,
            lap: 0,
        },
    );
    // Playing: taken, and the engine hears the track again.
    cmd(m, Command::PlayStop);
    let effects = cmd(
        m,
        Command::TrimClip {
            clip: c.id,
            edge: ClipEdge::End,
            ticks: -10,
        },
    );
    assert_eq!(sequences(&effects), 1);
    assert_eq!(clips(m)[0].end, c.end - 10);
}

/// Track 1: a over bar 1, b over bar 5. Track 2: c over bar 3. Nothing selected.
fn three_clips() -> (Play, ClipId, ClipId, ClipId) {
    use meloet_model::edit::{NewTake, add_recording};
    use meloet_model::song::Track;
    use meloet_model::{InstrumentRef, TrackColor};
    let mut s = Song::new_default();
    s.doc.tracks.push(Arc::new(Track {
        kind: meloet_model::TrackKind::Keys,
        id: TrackId(2),
        name: "ベース".into(),
        color: TrackColor::for_index(1),
        instrument: InstrumentRef::SINE,
        channel: Default::default(),
        sends: [meloet_model::Gain::SILENT; 2],
        muted: false,
        clips: Vec::new(),
        automation: Vec::new(),
        arp: Default::default(),
        knobs: Default::default(),
    }));
    let mut ids = Vec::new();
    for (track, origin, key) in [(1, 0, 60), (1, 4 * BAR, 62), (2, 2 * BAR, 40)] {
        let take = NewTake {
            origin,
            length: BAR,
            notes: vec![Note {
                from: None,
                start: 0,
                length: 100,
                key,
                velocity: 90,
            }],
            controls: Vec::new(),
        };
        let (e, added) = add_recording(&s.doc, TrackId(track), vec![take], 0).unwrap();
        s.doc = e.doc;
        ids.push(added.clip);
    }
    let mut p = Play::new(false, 0);
    p.replace_song(s, false, &mut Vec::new());
    (p, ids[0], ids[1], ids[2])
}

fn starts(p: &Play) -> Vec<Vec<(u32, u64)>> {
    p.song()
        .doc
        .tracks
        .iter()
        .map(|t| t.clips.iter().map(|c| (c.id.0, c.start)).collect())
        .collect()
}

#[test]
fn clicks_shift_clicks_boxes_and_mod_a_select_clips() {
    let (mut p, a, b, c) = three_clips();
    let id = |c: ClipId| c.0;
    p.select_clip(Some(a.0));
    let v = p.arrange_view();
    assert_eq!(
        (v.selected_clips, v.selected_clip),
        (vec![id(a)], Some(id(a)))
    );
    assert_eq!(p.song.session.play_target, Some(TrackId(1)));
    // Shift + click: added, clicked, shown, and its track selected.
    p.select_clips(vec![c.0], SelectMode::Add);
    let v = p.arrange_view();
    assert_eq!(v.selected_clips, [id(a), id(c)]);
    assert_eq!(
        (v.selected_clip, v.piano_roll_clip),
        (Some(id(c)), Some(id(c)))
    );
    assert_eq!(p.song.session.play_target, Some(TrackId(2)));
    // A box: neither the piano roll nor the track changes; nothing is clicked.
    p.select_clips(vec![b.0, a.0, 999], SelectMode::Replace);
    let v = p.arrange_view();
    assert_eq!(v.selected_clips, [id(a), id(b)]);
    assert_eq!((v.selected_clip, v.piano_roll_clip), (None, Some(id(c))));
    assert_eq!(p.song.session.play_target, Some(TrackId(2)));
    // A box around one clip: that one is clicked, still not shown.
    p.select_clips(vec![b.0], SelectMode::Replace);
    let v = p.arrange_view();
    assert_eq!(
        (v.selected_clip, v.piano_roll_clip),
        (Some(id(b)), Some(id(c)))
    );
    p.select_all_clips();
    let v = p.arrange_view();
    assert_eq!(
        v.selected_clips,
        [id(a), id(b), id(c)],
        "tracks, then starts"
    );
    assert_eq!(v.selected_clip, None);
    // Toggle off one: the others stay.
    p.select_clips(vec![b.0], SelectMode::Toggle);
    assert_eq!(p.arrange_view().selected_clips, [id(a), id(c)]);
    // The header of a track.
    p.select_track(1, false);
    assert_eq!(p.song.session.play_target, Some(TrackId(1)));
    p.select_track(9, false);
    assert_eq!(p.song.session.play_target, Some(TrackId(1)));
    assert_eq!(p.arrange_view().selected_clips, [id(a), id(c)]);
    p.select_clip(None);
    assert!(p.arrange_view().selected_clips.is_empty());
    assert_eq!(
        p.summary(),
        Some(StatusMessage::Summary {
            loop_bars: None,
            nothing_selected: true,
            notes: None,
        })
    );
}

#[test]
fn clips_move_together_and_undo_brings_the_selection_back() {
    let (mut p, a, b, c) = three_clips();
    let e = &mut Vec::new();
    p.select_clips(vec![b.0], SelectMode::Replace);
    let steps = p.undo_count();
    // a and c a bar later.
    assert_eq!(p.move_clips(&[a.0, c.0], BAR as i64, 0, false, e), None);
    assert_eq!(p.undo_count(), steps + 1);
    assert_eq!(
        starts(&p),
        [vec![(a.0, BAR), (b.0, 4 * BAR)], vec![(c.0, 3 * BAR)]]
    );
    let v = p.arrange_view();
    assert_eq!((v.selected_clips, v.selected_clip), (vec![a.0, c.0], None));
    // Into b: nothing, and it is said.
    assert_eq!(
        p.move_clips(&[a.0], 3 * BAR as i64, 0, false, e),
        Some(StatusMessage::ClipsNotMoved)
    );
    assert_eq!(
        p.move_clips(&[a.0], 3 * BAR as i64, 0, true, e),
        Some(StatusMessage::ClipsNotDuplicated)
    );
    // Off the tracks: nothing, and nothing is said.
    assert_eq!(p.move_clips(&[c.0], 0, 1, false, e), None);
    assert_eq!(p.undo_count(), steps + 1);
    // c up to track 1, between a and b; the clicked clip stays clicked.
    p.select_clip(Some(c.0));
    assert_eq!(p.move_clips(&[c.0], 0, -1, false, e), None);
    assert_eq!(starts(&p)[0], [(a.0, BAR), (c.0, 3 * BAR), (b.0, 4 * BAR)]);
    let v = p.arrange_view();
    assert_eq!((v.selected_clip, v.piano_roll_clip), (Some(c.0), Some(c.0)));
    // Undo: the clips, and what was selected before each step.
    p.undo(e);
    let v = p.arrange_view();
    assert_eq!((v.selected_clips, v.selected_clip), (vec![c.0], Some(c.0)));
    p.undo(e);
    let v = p.arrange_view();
    assert_eq!((v.selected_clips, v.selected_clip), (vec![b.0], Some(b.0)));
    assert_eq!(
        starts(&p),
        [vec![(a.0, 0), (b.0, 4 * BAR)], vec![(c.0, 2 * BAR)]]
    );
    p.redo(e);
    assert_eq!(p.arrange_view().selected_clips, [a.0, c.0]);
}

#[test]
fn duplicate_copies_the_group_and_selects_the_copies() {
    let (mut p, a, b, c) = three_clips();
    let e = &mut Vec::new();
    p.select_clip(Some(a.0));
    p.select_clips(vec![c.0], SelectMode::Add);
    p.select_clips(vec![b.0], SelectMode::Add);
    // b and c: bars 3 to 6, so the copies come 3 bars later.
    assert_eq!(p.duplicate_clips(&[b.0, c.0], e), None);
    let s = starts(&p);
    assert_eq!(s[0][2].1, 7 * BAR);
    assert_eq!(s[1][1].1, 5 * BAR);
    let copies = [s[0][2].0, s[1][1].0];
    let v = p.arrange_view();
    assert_eq!(v.selected_clips, copies);
    // b was clicked last: its copy is clicked and shown.
    assert_eq!(
        (v.selected_clip, v.piano_roll_clip),
        (Some(copies[0]), Some(copies[0]))
    );
    // a and c: bars 1 to 4, so c's copy would be over bar 6, where the copy of c is.
    let steps = p.undo_count();
    assert_eq!(
        p.duplicate_clips(&[a.0, c.0], e),
        Some(StatusMessage::ClipsNotDuplicated)
    );
    assert_eq!(p.undo_count(), steps);
}

#[test]
fn delete_cut_copy_and_paste_clips() {
    let (mut p, a, b, c) = three_clips();
    let e = &mut Vec::new();
    p.select_all_clips();
    let steps = p.undo_count();
    p.delete_selected(e);
    assert_eq!(starts(&p), [vec![], vec![]]);
    assert_eq!(p.undo_count(), steps + 1);
    p.undo(e);
    assert_eq!(p.arrange_view().selected_clips, [a.0, b.0, c.0]);
    // Copy a and c; paste on track 1 at bar 9: c's copy a row lower and 2 bars later.
    p.copy_clips(&[c.0, a.0, 999]);
    assert_eq!(p.arrange_view().clipboard_clips, 2);
    let to = |track, tick| Some(ClipSpot { track, tick });
    assert_eq!(p.paste_clips(to(1, 8 * BAR), e), None);
    assert_eq!(p.undo_count(), steps + 1);
    let s = starts(&p);
    let pasted = [s[0][2].0, s[1][1].0];
    assert_eq!((s[0][2].1, s[1][1].1), (8 * BAR, 10 * BAR));
    assert_eq!(p.arrange_view().selected_clips, pasted);
    // The same place again overlaps the copies.
    assert_eq!(
        p.paste_clips(to(1, 8 * BAR), e),
        Some(StatusMessage::ClipsNotPasted)
    );
    // Off the tracks.
    assert_eq!(p.paste_clips(to(2, 16 * BAR), e), None);
    assert_eq!(p.undo_count(), steps + 1);
    // Mod V: the selected track, the head of the bar the play line is in.
    p.select_track(1, false);
    p.locate(12 * BAR + 700, e);
    assert_eq!(p.paste_clips(None, e), None);
    let s = starts(&p);
    assert_eq!((s[0][3].1, s[1][2].1), (12 * BAR, 14 * BAR));
    // Cut b: one step, kept, and back with undo.
    let steps = p.undo_count();
    p.select_all_clips();
    p.cut_clips(&[b.0], e);
    assert_eq!(p.undo_count(), steps + 1);
    assert!(p.song().doc.clip(b).is_none());
    assert_eq!(p.arrange_view().clipboard_clips, 1);
    assert!(!p.arrange_view().selected_clips.contains(&b.0));
    p.paste_clips(to(2, 0), e);
    assert_eq!(starts(&p)[1][0].1, 0);
    // The copies share the takes, and the song comes back the same from its file.
    let (_, orig) = p.song().doc.clip(a).unwrap();
    let (_, copy) = p.song().doc.clip(ClipId(pasted[0])).unwrap();
    assert!(Arc::ptr_eq(&orig.keys()[0].take, &copy.keys()[0].take));
    let back = crate::songfile::decode(&crate::songfile::encode(p.song())).unwrap();
    assert_eq!(&back, p.song());
    // Another song lets go of what was kept.
    p.replace_song(Song::new_default(), false, e);
    assert_eq!(p.arrange_view().clipboard_clips, 0);
}

#[test]
fn the_status_line_says_when_clips_overlap() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    cmd(m, Command::DuplicateClips { clips: vec![c.id] });
    cmd(m, Command::SelectClip { clip: Some(c.id) });
    cmd(
        m,
        Command::MoveClips {
            clips: vec![c.id],
            ticks: 100,
            tracks: 0,
            copy: false,
        },
    );
    assert_eq!(status(m), Some(StatusMessage::ClipsNotMoved));
    cmd(m, Command::DuplicateClips { clips: vec![c.id] });
    assert_eq!(status(m), Some(StatusMessage::ClipsNotDuplicated));
    cmd(m, Command::CopyClips { clips: vec![c.id] });
    cmd(m, Command::PasteClips { to: None });
    assert_eq!(status(m), Some(StatusMessage::ClipsNotPasted));
    cmd(m, Command::SelectAllClips);
    assert_eq!(m.view().arrange.selected_clips.len(), 2);
}
