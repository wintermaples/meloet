//! Clips named, cut at a place, joined and made on an empty place; the chord stamp (round 10).

use meloet_model::song::{Clip, ClipTake, ClipTakes, Track};
use meloet_model::{AudioId, AudioShape, AudioSource, AudioTake, TrackColor, TrackKind};
use meloet_protocol::{
    Band, ClipView, Command, HistoryRow, IndexRange, ScaleMode, StatusMessage, StepName, StepWhat,
};

use super::tests::{cmd, ready, record_once, started, status};
use super::*;
use crate::state::Model;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn clips_of(m: &Model, track: usize) -> Vec<ClipView> {
    m.view().song.tracks[track].clips.clone()
}

fn steps(m: &Model) -> u32 {
    m.view().song.undo_count
}

fn last_row(m: &Model) -> HistoryRow {
    let n = steps(m);
    m.history_rows(n, n).pop().unwrap()
}

fn open_bands(m: &Model) -> Vec<Band> {
    (m.view().workspace.layout.lower.iter())
        .map(|b| b.band)
        .collect()
}

fn create(m: &mut Model, track: u32, tick: u64) {
    cmd(m, Command::CreateClip { track, tick });
}

/// A note at `start` in the clip, through the write tool.
fn write(m: &mut Model, clip: u32, start: u64, key: u8) {
    cmd(m, Command::SelectClip { clip: Some(clip) });
    let take = m.view().piano_roll.take;
    cmd(
        m,
        Command::AddNote {
            take,
            start,
            length: Q,
            key,
            velocity: 100,
        },
    );
}

/// The new song with a second keys track (id 2) and a vocal track (id 3) with two clips
/// side by side over bars 1 and 2 (ids 31 and 32).
fn three_tracks(m: &mut Model) {
    let mut song = m.play().song().clone();
    let keys = Track::clone(&song.doc.tracks[0]);
    song.doc.tracks.push(Arc::new(Track {
        id: TrackId(2),
        name: "ベース".into(),
        color: TrackColor::for_index(1),
        clips: Vec::new(),
        ..keys.clone()
    }));
    let source = Arc::new(AudioSource {
        id: AudioId(30),
        uid: [3; 16],
        file: "歌.wav".into(),
        frames: 480_000,
        sample_rate: 48_000,
        channels: 1,
        bits: Some(24),
        sha256: None,
    });
    let take = Arc::new(AudioTake {
        id: TakeId(33),
        source: source.clone(),
        first_frame: 0,
        frames: 96_000,
        sync: None,
    });
    let clip = |id: u32, start: u64| {
        Arc::new(Clip {
            id: ClipId(id),
            name: "歌".into(),
            start,
            end: start + BAR,
            takes: ClipTakes::Vocal {
                takes: vec![ClipTake {
                    take: take.clone(),
                    offset: -(start as i64),
                }],
                shape: AudioShape::RECORDED,
            },
            active: 0,
            comp: Vec::new(),
        })
    };
    song.doc.tracks.push(Arc::new(Track {
        id: TrackId(3),
        name: "歌".into(),
        color: TrackColor::for_index(2),
        kind: TrackKind::Vocal {
            input: meloet_model::VocalInput::Mono1,
        },
        clips: vec![clip(31, 0), clip(32, BAR)],
        ..keys
    }));
    song.doc.audio = Arc::new(vec![source]);
    song.doc.next_id = 40;
    m.play_mut().replace_song(song, false, &mut Vec::new());
}

#[test]
fn a_clip_name_is_one_step_on_keys_and_vocal_clips() {
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], BAR as f64);
    let c = clips_of(m, 0)[0].clone();
    let take = c.take_id;
    cmd(
        m,
        Command::RenameClip {
            clip: c.id,
            name: "  サビ ".into(),
        },
    );
    assert_eq!(steps(m), 2);
    let now = clips_of(m, 0)[0].clone();
    assert_eq!((now.name.as_str(), now.take_id), ("サビ", take));
    let row = last_row(m);
    assert_eq!(row.name, StepName::RenameClip);
    assert_eq!(
        row.what,
        StepWhat::Renamed {
            from: c.name.clone(),
            to: "サビ".into()
        }
    );
    for same in ["", "  ", "サビ"] {
        cmd(
            m,
            Command::RenameClip {
                clip: c.id,
                name: same.into(),
            },
        );
    }
    cmd(
        m,
        Command::RenameClip {
            clip: 999,
            name: "x".into(),
        },
    );
    assert_eq!(steps(m), 2);
    cmd(m, Command::Undo);
    assert_eq!(clips_of(m, 0)[0].name, c.name);
    cmd(m, Command::Redo);
    assert_eq!(clips_of(m, 0)[0].name, "サビ");

    three_tracks(m);
    cmd(
        m,
        Command::RenameClip {
            clip: 31,
            name: "Aメロ".into(),
        },
    );
    assert_eq!(clips_of(m, 2)[0].name, "Aメロ");
    assert_eq!(last_row(m).name, StepName::RenameClip);
}

#[test]
fn a_cut_at_a_place_selects_the_right_half() {
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], BAR as f64);
    let c = clips_of(m, 0)[0].clone();
    cmd(m, Command::Locate { tick: 3 * Q });
    cmd(
        m,
        Command::SplitClipAt {
            clip: c.id,
            tick: Q,
        },
    );
    let now = clips_of(m, 0);
    assert_eq!(now.len(), 2);
    assert_eq!((now[0].id, now[0].end, now[1].start), (c.id, Q, Q));
    let v = m.view();
    assert_eq!(v.arrange.selected_clips, [now[1].id]);
    assert_eq!(v.arrange.selected_clip, Some(now[1].id));
    assert_eq!(
        status(m),
        Some(StatusMessage::ClipSplit {
            name: c.name.clone(),
            tick: Q,
            count: 1
        })
    );
    assert_eq!(last_row(m).name, StepName::SplitClips);
    // Not inside the clip (an edge is not inside): nothing.
    cmd(
        m,
        Command::SplitClipAt {
            clip: c.id,
            tick: Q,
        },
    );
    cmd(
        m,
        Command::SplitClipAt {
            clip: c.id,
            tick: 5 * BAR,
        },
    );
    cmd(
        m,
        Command::SplitClipAt {
            clip: 999,
            tick: 10,
        },
    );
    assert_eq!(steps(m), 2);
    cmd(m, Command::Undo);
    assert_eq!(clips_of(m, 0).len(), 1);
}

#[test]
fn joined_clips_become_the_first_one_selected_and_undo_brings_them_back() {
    let m = &mut ready();
    create(m, 1, 0);
    let a = clips_of(m, 0)[0].id;
    create(m, 1, 6 * BAR);
    let b = clips_of(m, 0)[1].id;
    write(m, a, Q, 60);
    write(m, b, 7 * BAR, 64);
    let before = clips_of(m, 0);
    cmd(
        m,
        Command::SelectClips {
            clips: vec![a, b],
            mode: SelectMode::Replace,
        },
    );
    let n = steps(m);
    cmd(m, Command::JoinClips { clips: vec![a, b] });
    assert_eq!(steps(m), n + 1);
    let now = clips_of(m, 0);
    assert_eq!(now.len(), 1);
    let j = &now[0];
    assert_eq!((j.id, j.start, j.end), (a, 0, 10 * BAR));
    assert_eq!((j.name.as_str(), j.takes, j.note_count), ("ピアノ", 1, 2));
    assert!(before.iter().all(|c| c.take_id != j.take_id));
    let v = m.view();
    assert_eq!(v.arrange.selected_clips, [a]);
    assert_eq!(v.arrange.selected_clip, Some(a));
    assert_eq!(v.arrange.piano_roll_clip, Some(a));
    let row = last_row(m);
    assert_eq!(
        (row.name, row.what),
        (StepName::JoinClips, StepWhat::Clips { count: 2 })
    );
    cmd(m, Command::Undo);
    assert_eq!(clips_of(m, 0), before);
    assert_eq!(m.view().arrange.selected_clips, [a, b]);
    cmd(m, Command::Redo);
    assert_eq!(clips_of(m, 0).len(), 1);
}

#[test]
fn joining_does_nothing_with_one_clip_other_tracks_vocal_clips_or_a_clip_between() {
    let m = &mut ready();
    three_tracks(m);
    create(m, 1, 0);
    let a = clips_of(m, 0)[0].id;
    create(m, 1, 6 * BAR);
    let b = clips_of(m, 0)[1].id;
    create(m, 1, 4 * BAR);
    let between = clips_of(m, 0)[1].id;
    create(m, 2, 0);
    let other = clips_of(m, 1)[0].id;
    let n = steps(m);
    for clips in [
        vec![a],
        vec![a, a],
        vec![],
        vec![a, other],
        vec![a, b],
        vec![31, 32],
        vec![a, 999],
    ] {
        cmd(m, Command::JoinClips { clips });
    }
    assert_eq!(steps(m), n);
    assert_eq!(clips_of(m, 0).len(), 3);
    cmd(
        m,
        Command::JoinClips {
            clips: vec![b, between, a],
        },
    );
    assert_eq!(steps(m), n + 1);
    assert_eq!(clips_of(m, 0).len(), 1);
}

#[test]
fn a_clip_made_on_an_empty_place_is_selected_and_opens_the_piano_roll() {
    let m = &mut ready();
    three_tracks(m);
    assert!(!open_bands(m).contains(&Band::PianoRoll));
    create(m, 2, BAR + 700);
    let c = clips_of(m, 1)[0].clone();
    assert_eq!((c.start, c.end, c.name.as_str()), (BAR, 5 * BAR, "ベース"));
    assert_eq!((c.takes, c.note_count), (1, 0));
    assert!(open_bands(m).contains(&Band::PianoRoll));
    let v = m.view();
    assert_eq!(v.arrange.selected_clips, [c.id]);
    assert_eq!(v.arrange.piano_roll_clip, Some(c.id));
    assert_eq!(v.piano_roll.take, Some(c.take_id));
    assert_eq!(m.play().song().session.play_target, Some(TrackId(2)));
    let row = last_row(m);
    assert_eq!(
        (row.name, row.what),
        (
            StepName::WriteClip,
            StepWhat::Name {
                name: "ベース".into()
            }
        )
    );
    assert_eq!(steps(m), 1);

    // The band open already stays open; up to the next clip.
    create(m, 2, 0);
    assert!(open_bands(m).contains(&Band::PianoRoll));
    let made = clips_of(m, 1)[0].clone();
    assert_eq!((made.start, made.end), (0, BAR));
    assert_eq!(steps(m), 2);

    // Inside a clip, on a vocal track, on no track: nothing.
    create(m, 2, 2 * BAR);
    create(m, 3, 5 * BAR);
    create(m, 9, 0);
    assert_eq!(steps(m), 2);

    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    assert!(clips_of(m, 1).is_empty());
    assert_eq!(m.view().arrange.piano_roll_clip, None);
}

#[test]
fn nothing_of_the_clips_changes_while_recording() {
    let m = &mut ready();
    create(m, 1, 0);
    create(m, 1, 6 * BAR);
    let [a, b] = [0, 1].map(|i| clips_of(m, 0)[i].id);
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Locate { tick: 12 * BAR });
    cmd(m, Command::Record);
    started(m);
    let n = steps(m);
    for command in [
        Command::RenameClip {
            clip: a,
            name: "x".into(),
        },
        Command::SplitClipAt { clip: a, tick: Q },
        Command::JoinClips { clips: vec![a, b] },
        Command::CreateClip {
            track: 1,
            tick: 4 * BAR,
        },
        Command::AddNotes {
            take: None,
            start: 0,
            length: Q,
            keys: vec![60],
            velocity: 100,
        },
    ] {
        cmd(m, command);
    }
    assert_eq!(steps(m), n);
    assert_eq!(clips_of(m, 0).len(), 2);
}

#[test]
fn the_chord_stamp_is_one_step_and_keeps_keys_outside_the_locked_scale() {
    let m = &mut ready();
    create(m, 1, 0);
    let c = clips_of(m, 0)[0].clone();
    cmd(
        m,
        Command::SetMusicalScale {
            root: 0,
            mode: ScaleMode::Major,
            lock: true,
            highlight: true,
        },
    );
    // 63 is outside C major; 200 is no key; 60 twice is one note.
    cmd(
        m,
        Command::AddNotes {
            take: Some(c.take_id),
            start: Q,
            length: Q,
            keys: vec![67, 63, 60, 200, 60],
            velocity: 100,
        },
    );
    assert_eq!(steps(m), 2);
    assert!(!matches!(status(m), Some(StatusMessage::OutOfScale { .. })));
    let now = clips_of(m, 0)[0].clone();
    assert_eq!((now.id, now.takes, now.note_count), (c.id, 1, 3));
    assert_ne!(now.take_id, c.take_id);
    let notes = TakeNotes::decode(&m.take_notes(now.take_id).unwrap()).unwrap();
    let keys: Vec<u8> = notes.notes.iter().map(|n| n.key).collect();
    assert_eq!(keys, [60, 63, 67]);
    assert_eq!(
        m.view().piano_roll.selected_notes,
        [IndexRange { start: 0, end: 3 }]
    );
    let row = last_row(m);
    assert_eq!(
        (row.name, row.what),
        (StepName::WriteNote, StepWhat::Notes { count: 3 })
    );

    // A take that is not shown now, and no key left: nothing.
    for (take, keys) in [(Some(c.take_id), vec![60]), (Some(now.take_id), vec![128])] {
        cmd(
            m,
            Command::AddNotes {
                take,
                start: 2 * Q,
                length: Q,
                keys,
                velocity: 100,
            },
        );
    }
    assert_eq!(steps(m), 2);

    // Past the clip it grows by bars, as one step.
    cmd(
        m,
        Command::AddNotes {
            take: Some(now.take_id),
            start: 4 * BAR + Q,
            length: Q,
            keys: vec![60, 64],
            velocity: 100,
        },
    );
    assert_eq!(steps(m), 3);
    assert_eq!(clips_of(m, 0)[0].end, 5 * BAR);
    cmd(m, Command::Undo);
    assert_eq!(clips_of(m, 0)[0].end, 4 * BAR);
    cmd(m, Command::Undo);
    assert_eq!(clips_of(m, 0)[0].take_id, c.take_id);
}

#[test]
fn the_chord_stamp_with_no_clip_shown_makes_one() {
    let m = &mut ready();
    cmd(
        m,
        Command::AddNotes {
            take: None,
            start: 2 * BAR,
            length: Q,
            keys: vec![64, 60],
            velocity: 0,
        },
    );
    assert_eq!(steps(m), 1);
    let c = clips_of(m, 0)[0].clone();
    assert_eq!((c.start, c.end, c.note_count), (2 * BAR, 6 * BAR, 2));
    let v = m.view();
    assert_eq!(v.arrange.piano_roll_clip, Some(c.id));
    assert_eq!(
        v.piano_roll.selected_notes,
        [IndexRange { start: 0, end: 2 }]
    );
    let notes = TakeNotes::decode(&m.take_notes(c.take_id).unwrap()).unwrap();
    assert!(notes.notes.iter().all(|n| n.velocity == 1));
    cmd(m, Command::Undo);
    assert!(clips_of(m, 0).is_empty());
}

fn open_roll(m: &mut Model, track: u32) {
    cmd(m, Command::OpenPianoRoll { track });
}

#[test]
fn open_in_the_piano_roll_shows_the_clip_under_the_play_line_or_none() {
    let m = &mut ready();
    three_tracks(m);
    create(m, 1, 0);
    let a = clips_of(m, 0)[0].id;
    cmd(
        m,
        Command::ToggleBand {
            band: Band::PianoRoll,
        },
    );
    assert!(!open_bands(m).contains(&Band::PianoRoll));
    cmd(m, Command::Locate { tick: 2 * BAR });
    cmd(
        m,
        Command::SelectTrack {
            track: 2,
            from_arm: false,
        },
    );
    let n = steps(m);

    open_roll(m, 1);
    assert!(open_bands(m).contains(&Band::PianoRoll));
    let v = m.view();
    assert_eq!(v.arrange.selected_clips, [a]);
    assert_eq!(v.arrange.piano_roll_clip, Some(a));
    assert_eq!(v.piano_roll.take, Some(clips_of(m, 0)[0].take_id));
    assert_eq!(m.play().song().session.play_target, Some(TrackId(1)));

    // No clip there: the clip of the other track is no longer shown, and the band stays open.
    open_roll(m, 2);
    assert!(open_bands(m).contains(&Band::PianoRoll));
    let v = m.view();
    assert!(v.arrange.selected_clips.is_empty());
    assert_eq!(v.arrange.selected_clip, None);
    assert_eq!(v.arrange.piano_roll_clip, None);
    assert_eq!(v.piano_roll.take, None);
    assert_eq!(m.play().song().session.play_target, Some(TrackId(2)));

    // The end of a clip is not in it.
    cmd(m, Command::Locate { tick: 4 * BAR });
    open_roll(m, 1);
    assert_eq!(m.view().arrange.piano_roll_clip, None);
    assert_eq!(m.play().song().session.play_target, Some(TrackId(1)));
    assert_eq!(steps(m), n, "not a step");
}

#[test]
fn open_in_the_piano_roll_does_nothing_on_a_vocal_track_or_no_track() {
    let m = &mut ready();
    three_tracks(m);
    cmd(m, Command::SelectClip { clip: None });
    let before = m.view();
    assert!(!open_bands(m).contains(&Band::PianoRoll));
    open_roll(m, 3);
    open_roll(m, 9);
    let after = m.view();
    assert!(!open_bands(m).contains(&Band::PianoRoll));
    assert_eq!(after.arrange, before.arrange);
    assert_eq!(after.song.keys_target, before.song.keys_target);
}

#[test]
fn open_in_the_piano_roll_while_recording_keeps_the_track() {
    let m = &mut ready();
    three_tracks(m);
    create(m, 2, 0);
    let b = clips_of(m, 1)[0].id;
    cmd(
        m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Locate { tick: 0 });
    cmd(m, Command::Record);
    started(m);
    open_roll(m, 2);
    assert_eq!(m.play().song().session.play_target, Some(TrackId(1)));
    assert_eq!(m.view().arrange.piano_roll_clip, Some(b));
}

#[test]
fn a_click_on_a_vocal_clip_selects_its_track_as_its_head_does() {
    let m = &mut ready();
    three_tracks(m);
    let select = |track| Command::SelectTrack {
        track,
        from_arm: false,
    };
    cmd(m, select(2));
    cmd(m, Command::SelectClip { clip: Some(31) });
    let song = m.play().song();
    assert_eq!(song.session.play_target, Some(TrackId(3)));
    assert_eq!(song.keys_target(), Some(TrackId(2)), "the keys stay");
    assert!(song.session.armed_vocals.contains(&TrackId(3)), "its ●");
    assert_eq!(status(m), Some(StatusMessage::NoInput));

    cmd(m, select(1));
    cmd(
        m,
        Command::SelectClips {
            clips: vec![32],
            mode: SelectMode::Add,
        },
    );
    let song = m.play().song();
    assert_eq!(song.session.play_target, Some(TrackId(3)));
    assert_eq!(song.keys_target(), Some(TrackId(1)));
}

#[test]
fn a_vocal_track_selected_in_place_of_a_deleted_one_gets_its_arm() {
    let m = &mut ready();
    three_tracks(m);
    cmd(
        m,
        Command::SelectTrack {
            track: 2,
            from_arm: false,
        },
    );
    cmd(m, Command::DeleteTrack { track: 2 });
    let song = m.play().song();
    assert_eq!(song.session.play_target, Some(TrackId(3)));
    assert!(song.session.armed_vocals.contains(&TrackId(3)));
}

#[test]
fn a_vocal_track_selected_when_undo_takes_the_selected_one_away_gets_its_arm() {
    let m = &mut ready();
    three_tracks(m);
    assert_eq!(m.play().doc().tracks[2].id, TrackId(3));
    cmd(
        m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    cmd(m, Command::AddVocalTrack);
    let added = m.play().song().session.play_target;
    assert_ne!(added, Some(TrackId(1)));
    cmd(m, Command::Undo);
    let song = m.play().song();
    assert_eq!(song.session.play_target, Some(TrackId(3)));
    assert!(song.session.armed_vocals.contains(&TrackId(3)));
    assert_eq!(song.keys_target(), Some(TrackId(1)), "the keys stay");
}
