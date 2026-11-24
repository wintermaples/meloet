use std::sync::Arc;

use super::*;
use crate::{InstrumentRef, Meter, TICKS_PER_QUARTER, Tempo, TrackColor};

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
        velocity: 100,
    }
}

fn take(origin: u64, length: u64) -> NewTake {
    NewTake {
        origin,
        length,
        notes: vec![note(0, 1, 60)],
        controls: Vec::new(),
    }
}

/// Two tracks, the second named "ベース".
fn two_tracks() -> Doc {
    let mut doc = Doc::new_default();
    doc.tracks.push(Arc::new(Track {
        kind: crate::TrackKind::Keys,
        id: T2,
        name: "ベース".to_owned(),
        color: TrackColor::for_index(1),
        instrument: InstrumentRef::SINE,
        channel: Default::default(),
        sends: [crate::Gain::SILENT; 2],
        muted: false,
        clips: Vec::new(),
        automation: Vec::new(),
        arp: Default::default(),
        knobs: Default::default(),
    }));
    doc
}

fn record(doc: &Doc, track: TrackId, takes: Vec<NewTake>, active: usize) -> (Doc, RecordingAdded) {
    let (edit, added) = add_recording(doc, track, takes, active).unwrap();
    (edit.doc, added)
}

fn clips(doc: &Doc, track: TrackId) -> Vec<(u64, u64, usize)> {
    doc.track(track)
        .unwrap()
        .clips
        .iter()
        .map(|c| (c.start, c.end, c.takes.len()))
        .collect()
}

fn rename(doc: &Doc, clip: ClipId, name: &str) -> Doc {
    let mut doc = doc.clone();
    for t in &mut doc.tracks {
        let t = Arc::make_mut(t);
        for c in &mut t.clips {
            if c.id == clip {
                Arc::make_mut(c).name = name.to_owned();
            }
        }
    }
    doc
}

#[test]
fn first_recording_makes_a_clip_named_after_the_track() {
    let doc = Doc::new_default();
    let (edit, added) = add_recording(&doc, T1, vec![take(BAR, 2 * Q)], 0).unwrap();
    assert_eq!(
        edit.step,
        StepKind::Record {
            track: T1,
            clip: added.clip,
            takes: 1
        }
    );
    let doc = edit.doc;
    let clip = &doc.track(T1).unwrap().clips[0];
    assert_eq!(clip.id, ClipId(4));
    assert_eq!(clip.name, "ピアノ");
    assert_eq!((clip.start, clip.end), (BAR, BAR + 2 * Q));
    assert_eq!(clip.keys()[0].offset, 0);
    assert_eq!(clip.keys()[0].take.id, TakeId(5));
    assert_eq!(clip.keys()[0].take.notes, [note(0, 1, 60)]);
    assert_eq!(clip.active, 0);
    assert_eq!(
        added,
        RecordingAdded {
            clip: ClipId(4),
            active: 0
        }
    );
    assert_eq!(doc.next_id, 6);
}

#[test]
fn recording_away_from_clips_makes_another_clip_in_order() {
    let (doc, a) = record(&Doc::new_default(), T1, vec![take(2 * BAR, 2 * Q)], 0);
    let (doc, b) = record(&doc, T1, vec![take(0, 2 * Q)], 0);
    assert_ne!(a.clip, b.clip);
    assert_eq!(
        clips(&doc, T1),
        [(0, 2 * Q, 1), (2 * BAR, 2 * BAR + 2 * Q, 1)]
    );
}

#[test]
fn touching_ends_do_not_merge() {
    let (doc, _) = record(&Doc::new_default(), T1, vec![take(0, BAR)], 0);
    let (doc, _) = record(&doc, T1, vec![take(BAR, BAR)], 0);
    let (doc, _) = record(&doc, T1, vec![take(0, BAR)], 0);
    // The third overlaps only the first.
    assert_eq!(clips(&doc, T1), [(0, BAR, 2), (BAR, 2 * BAR, 1)]);
}

#[test]
fn overlapping_one_clip_adds_a_take_to_it() {
    let (doc, first) = record(&Doc::new_default(), T1, vec![take(0, BAR)], 0);
    let doc = rename(&doc, first.clip, "Aメロ");
    let old_take = doc.track(T1).unwrap().clips[0].keys()[0].take.clone();
    let (doc, added) = record(&doc, T1, vec![take(2 * Q, BAR)], 0);
    let clip = &doc.track(T1).unwrap().clips[0];
    assert_eq!(doc.track(T1).unwrap().clips.len(), 1);
    assert_eq!(clip.id, first.clip);
    assert_eq!(clip.name, "Aメロ");
    assert_eq!((clip.start, clip.end), (0, BAR + 2 * Q));
    assert!(Arc::ptr_eq(&clip.keys()[0].take, &old_take));
    assert_eq!(
        clip.keys().iter().map(|t| t.offset).collect::<Vec<_>>(),
        [0, 2 * Q as i64]
    );
    assert_eq!(clip.active, 1);
    assert_eq!(
        added,
        RecordingAdded {
            clip: first.clip,
            active: 1
        }
    );
}

#[test]
fn recording_before_a_clip_moves_its_start_and_keeps_takes_in_place() {
    let (doc, _) = record(&Doc::new_default(), T1, vec![take(1000, 1000)], 0);
    let (doc, _) = record(&doc, T1, vec![take(500, 600)], 0);
    let clip = &doc.track(T1).unwrap().clips[0];
    assert_eq!((clip.start, clip.end), (500, 2000));
    // Song positions of each take's tick 0 do not move.
    let zeros: Vec<i64> = clip.keys().iter().map(|t| clip.zero(t)).collect();
    assert_eq!(zeros, [1000, 500]);
}

#[test]
fn spanning_two_clips_merges_them_with_the_first_ones_name() {
    let (doc, a) = record(&Doc::new_default(), T1, vec![take(0, 1000)], 0);
    let (doc, b) = record(&doc, T1, vec![take(2000, 1000)], 0);
    let (doc, b2) = record(&doc, T1, vec![take(2500, 400)], 0);
    assert_eq!(
        b2,
        RecordingAdded {
            clip: b.clip,
            active: 1
        }
    );
    let (doc, _) = record(&doc, T1, vec![take(5000, 1000)], 0);
    let doc = rename(&rename(&doc, a.clip, "前"), b.clip, "後");
    let far = doc.track(T1).unwrap().clips[2].clone();

    let (doc, added) = record(&doc, T1, vec![take(500, 2000)], 0);
    let track = doc.track(T1).unwrap();
    assert_eq!(clips(&doc, T1), [(0, 3000, 4), (5000, 6000, 1)]);
    let clip = &track.clips[0];
    assert_eq!(clip.id, a.clip);
    assert_eq!(clip.name, "前");
    // The first clip's take, the second clip's two takes, then the new one.
    let offsets: Vec<i64> = clip.keys().iter().map(|t| t.offset).collect();
    assert_eq!(offsets, [0, 2000, 2500, 500]);
    assert_eq!(
        added,
        RecordingAdded {
            clip: a.clip,
            active: 3
        }
    );
    assert!(Arc::ptr_eq(&track.clips[1], &far));
    assert!(doc.clip(b.clip).is_none());
}

#[test]
fn loop_passes_go_into_one_clip_with_the_chosen_take_shown() {
    let passes = vec![take(BAR, BAR), take(BAR, BAR), take(BAR, 2 * Q)];
    let (doc, added) = record(&Doc::new_default(), T1, passes, 1);
    assert_eq!(clips(&doc, T1), [(BAR, 2 * BAR, 3)]);
    assert_eq!(added.active, 1);
    let ids: Vec<u32> = doc.track(T1).unwrap().clips[0]
        .keys()
        .iter()
        .map(|t| t.take.id.0)
        .collect();
    assert_eq!(ids, [5, 6, 7]);

    // Looping over the same bars again adds after the takes already there.
    let (edit, added) = add_recording(&doc, T1, vec![take(BAR, BAR), take(BAR, BAR)], 1).unwrap();
    assert_eq!(clips(&edit.doc, T1), [(BAR, 2 * BAR, 5)]);
    assert_eq!(added.active, 4);
    assert_eq!(
        edit.step,
        StepKind::Record {
            track: T1,
            clip: added.clip,
            takes: 2
        }
    );
}

#[test]
fn other_tracks_and_clips_stay_shared() {
    let (doc, _) = record(&two_tracks(), T2, vec![take(0, 2 * Q)], 0);
    let (doc, _) = record(&doc, T1, vec![take(0, 2 * Q)], 0);
    let (new, _) = record(&doc, T1, vec![take(2 * BAR + 160, 2 * Q)], 0);
    assert!(Arc::ptr_eq(&doc.tracks[1], &new.tracks[1]));
    assert!(!Arc::ptr_eq(&doc.tracks[0], &new.tracks[0]));
    assert!(Arc::ptr_eq(
        &doc.tracks[0].clips[0],
        &new.tracks[0].clips[0]
    ));
}

#[test]
fn bad_recordings() {
    let doc = Doc::new_default();
    assert_eq!(
        add_recording(&doc, T2, vec![take(0, 1)], 0),
        Err(EditError::NoSuchTrack(T2))
    );
    assert_eq!(add_recording(&doc, T1, vec![], 0), Err(EditError::NoTakes));
    assert_eq!(
        add_recording(&doc, T1, vec![take(0, 1)], 1),
        Err(EditError::NoSuchTake(1))
    );
    let bad: [fn(&mut NewTake); 7] = [
        |t| t.length = 0,
        |t| t.notes = vec![note(0, 0, 60)],
        |t| t.notes = vec![note(2 * Q - 60, 100, 60)],
        |t| t.notes[0].velocity = 0,
        |t| t.notes[0].key = 128,
        |t| t.notes = vec![note(10, 1, 60), note(5, 1, 60)],
        |t| {
            t.controls = vec![ControlPoint {
                tick: 2 * Q,
                value: ControlValue::PitchBend(0),
            }]
        },
    ];
    for (i, change) in bad.iter().enumerate() {
        let mut t = take(0, 2 * Q);
        change(&mut t);
        assert_eq!(
            add_recording(&doc, T1, vec![take(0, 2 * Q), t], 0),
            Err(EditError::BadTake(1)),
            "case {i}"
        );
    }
}

#[test]
fn switching_takes() {
    let (doc, added) = record(
        &two_tracks(),
        T1,
        vec![take(0, 2 * Q), take(0, 2 * Q), take(0, 2 * Q)],
        2,
    );
    let (doc, _) = record(&doc, T2, vec![take(0, 2 * Q)], 0);
    let edit = switch_take(&doc, added.clip, 0).unwrap();
    assert_eq!(
        edit.step,
        StepKind::SwitchTake {
            clip: added.clip,
            from: 2,
            to: 0
        }
    );
    let (_, clip) = edit.doc.clip(added.clip).unwrap();
    assert_eq!(clip.active, 0);
    let (_, old) = doc.clip(added.clip).unwrap();
    for (a, b) in clip.keys().iter().zip(old.keys()) {
        assert!(Arc::ptr_eq(&a.take, &b.take));
    }
    assert!(Arc::ptr_eq(&doc.tracks[1], &edit.doc.tracks[1]));
    assert_eq!(switch_take(&doc, added.clip, 2), Err(EditError::Unchanged));
    assert_eq!(
        switch_take(&doc, added.clip, 3),
        Err(EditError::NoSuchTake(3))
    );
    assert_eq!(
        switch_take(&doc, ClipId(999), 0),
        Err(EditError::NoSuchClip(ClipId(999)))
    );
}

#[test]
fn deleting_clips() {
    let (doc, a) = record(&two_tracks(), T1, vec![take(0, 2 * Q)], 0);
    let (doc, b) = record(&doc, T1, vec![take(BAR + 80, 2 * Q)], 0);
    let (doc, c) = record(&doc, T2, vec![take(0, 2 * Q)], 0);
    let edit = delete_clips(&doc, &[a.clip, c.clip], false).unwrap();
    assert_eq!(
        edit.step,
        StepKind::DeleteClips {
            count: 2,
            cut: false
        }
    );
    assert_eq!(clips(&edit.doc, T1), [(BAR + 80, BAR + 80 + 2 * Q, 1)]);
    assert!(clips(&edit.doc, T2).is_empty());
    assert!(Arc::ptr_eq(
        &doc.track(T1).unwrap().clips[1],
        &edit.doc.track(T1).unwrap().clips[0]
    ));

    let only_t1 = delete_clips(&doc, &[b.clip], true).unwrap();
    assert_eq!(
        only_t1.step,
        StepKind::DeleteClips {
            count: 1,
            cut: true
        }
    );
    assert!(Arc::ptr_eq(&doc.tracks[1], &only_t1.doc.tracks[1]));

    assert_eq!(
        delete_clips(&doc, &[a.clip, ClipId(999)], false),
        Err(EditError::NoSuchClip(ClipId(999)))
    );
    assert_eq!(delete_clips(&doc, &[], false), Err(EditError::Unchanged));
}

#[test]
fn tempo_and_meter() {
    let doc = Doc::new_default();
    let t = Tempo::from_centi(12_050).unwrap();
    let edit = set_tempo(&doc, t).unwrap();
    assert_eq!(edit.doc.timeline.tempo, t);
    assert_eq!(
        edit.step,
        StepKind::Tempo {
            from: doc.timeline.tempo,
            to: t
        }
    );
    assert!(Arc::ptr_eq(&doc.tracks[0], &edit.doc.tracks[0]));
    assert_eq!(
        set_tempo(&doc, doc.timeline.tempo),
        Err(EditError::Unchanged)
    );

    let m = Meter::new(6, 8).unwrap();
    let edit = set_meter(&doc, m).unwrap();
    assert_eq!(edit.doc.timeline.meter, m);
    assert_eq!(
        edit.step,
        StepKind::Meter {
            from: Meter::FOUR_FOUR,
            to: m
        }
    );
    assert_eq!(set_meter(&doc, Meter::FOUR_FOUR), Err(EditError::Unchanged));
}

#[test]
fn muting_a_track() {
    let doc = two_tracks();
    let e = toggle_mute(&doc, T2).unwrap();
    assert_eq!(
        e.step,
        StepKind::Mute {
            track: T2,
            muted: true
        }
    );
    assert!(e.doc.track(T2).unwrap().muted);
    assert!(
        Arc::ptr_eq(&doc.tracks[0], &e.doc.tracks[0]),
        "the other track is shared"
    );
    let back = toggle_mute(&e.doc, T2).unwrap();
    assert_eq!(
        back.step,
        StepKind::Mute {
            track: T2,
            muted: false
        }
    );
    assert_eq!(back.doc, doc);
    assert_eq!(
        toggle_mute(&doc, TrackId(9)),
        Err(EditError::NoSuchTrack(TrackId(9)))
    );
}

#[test]
fn renaming_a_track() {
    let doc = two_tracks();
    let e = rename_track(&doc, T2, "  うら  ").unwrap();
    assert_eq!(e.step, StepKind::RenameTrack { track: T2 });
    assert_eq!(e.doc.track(T2).unwrap().name, "うら");
    assert!(Arc::ptr_eq(&doc.tracks[0], &e.doc.tracks[0]));
    for same in ["", " \u{3000}\t", "ベース", " ベース "] {
        assert_eq!(rename_track(&doc, T2, same), Err(EditError::Unchanged));
    }
    assert_eq!(
        rename_track(&doc, TrackId(9), "x"),
        Err(EditError::NoSuchTrack(TrackId(9)))
    );
}

#[test]
fn coloring_a_track() {
    let doc = two_tracks();
    let five = TrackColor::new(5).unwrap();
    let e = set_track_color(&doc, T1, five).unwrap();
    assert_eq!(e.step, StepKind::SetTrackColor { track: T1 });
    assert_eq!(e.doc.track(T1).unwrap().color, five);
    assert_eq!(e.doc.track(T2).unwrap().color.get(), 2);
    assert_eq!(set_track_color(&e.doc, T1, five), Err(EditError::Unchanged));
}

#[test]
fn deleting_a_track_takes_its_clips_and_may_leave_none() {
    let (doc, added) = record(&two_tracks(), T2, vec![take(0, 2 * Q)], 0);
    let e = delete_track(&doc, T2).unwrap();
    assert_eq!(e.step, StepKind::DeleteTrack { track: T2 });
    assert_eq!(e.doc.tracks.len(), 1);
    assert!(e.doc.clip(added.clip).is_none());
    assert!(Arc::ptr_eq(&doc.tracks[0], &e.doc.tracks[0]));
    assert_eq!(e.doc.next_id, doc.next_id, "ids are not given out again");
    let none = delete_track(&e.doc, T1).unwrap().doc;
    assert!(none.tracks.is_empty());
    assert_eq!(delete_track(&none, T1), Err(EditError::NoSuchTrack(T1)));
}

#[test]
fn moving_a_track_keeps_the_others_in_order_and_every_color() {
    let mut doc = two_tracks();
    let t3 = TrackId(3);
    doc.tracks.push(Arc::new(Track {
        id: t3,
        color: TrackColor::for_index(2),
        ..Track::clone(&doc.tracks[1])
    }));
    let order = |doc: &Doc| -> Vec<(u32, u8)> {
        doc.tracks.iter().map(|t| (t.id.0, t.color.get())).collect()
    };
    let e = move_track(&doc, T1, 2).unwrap();
    assert_eq!(e.step, StepKind::MoveTrack { track: T1 });
    assert_eq!(order(&e.doc), [(2, 2), (3, 3), (1, 1)]);
    assert_eq!(
        order(&move_track(&doc, t3, 0).unwrap().doc),
        [(3, 3), (1, 1), (2, 2)]
    );
    assert_eq!(
        order(&move_track(&doc, T1, 1).unwrap().doc),
        [(2, 2), (1, 1), (3, 3)]
    );
    assert_eq!(
        order(&move_track(&doc, T1, 99).unwrap().doc),
        [(2, 2), (3, 3), (1, 1)],
        "stops at the end"
    );
    assert_eq!(move_track(&doc, T2, 1), Err(EditError::Unchanged));
    assert_eq!(move_track(&doc, t3, 99), Err(EditError::Unchanged));
    assert_eq!(
        move_track(&doc, TrackId(9), 0),
        Err(EditError::NoSuchTrack(TrackId(9)))
    );
}

#[test]
fn which_tracks_play() {
    let doc = toggle_mute(&two_tracks(), T2).unwrap().doc;
    let plays = |soloed: &[TrackId]| {
        let soloed = soloed.iter().copied().collect();
        [T1, T2].map(|t| doc.plays(doc.track(t).unwrap(), &soloed))
    };
    assert_eq!(plays(&[]), [true, false]);
    assert_eq!(plays(&[T1]), [true, false]);
    assert_eq!(plays(&[T2]), [false, false], "mute beats solo");
    assert_eq!(plays(&[T1, T2]), [true, false]);
    assert_eq!(
        plays(&[TrackId(9)]),
        [true, false],
        "a track not in the song is no solo"
    );
}

#[test]
fn deleting_a_take_shows_the_one_before_or_the_next_and_keeps_the_others() {
    let takes = vec![take(0, 2 * Q), take(0, 2 * Q), take(0, 2 * Q)];
    let (doc, added) = record(&Doc::new_default(), T1, takes, 1);
    let ids = |doc: &Doc| {
        let c = doc.clip(added.clip).unwrap().1;
        (
            c.keys().iter().map(|t| t.take.id).collect::<Vec<_>>(),
            c.active,
        )
    };
    let (all, _) = ids(&doc);

    let e = delete_take(&doc, added.clip).unwrap();
    assert_eq!(
        e.step,
        StepKind::DeleteTake {
            clip: added.clip,
            index: 1
        }
    );
    assert_eq!(ids(&e.doc), (vec![all[0], all[2]], 0));
    assert_eq!(e.doc.next_id, doc.next_id);

    let e = delete_take(&e.doc, added.clip).unwrap();
    assert_eq!(ids(&e.doc), (vec![all[2]], 0));
    assert_eq!(delete_take(&e.doc, added.clip), Err(EditError::Unchanged));
    assert_eq!(
        delete_take(&doc, ClipId(999)),
        Err(EditError::NoSuchClip(ClipId(999)))
    );
}

#[test]
fn deleting_the_other_takes_keeps_the_shown_one_as_the_first_and_undo_brings_them_back() {
    let takes = vec![take(0, 2 * Q), take(0, 2 * Q), take(0, 2 * Q)];
    let (doc, added) = record(&Doc::new_default(), T1, takes, 1);
    let clip = |doc: &Doc| doc.clip(added.clip).unwrap().1.clone();
    let before = clip(&doc);

    let e = delete_other_takes(&doc, added.clip).unwrap();
    assert_eq!(
        e.step,
        StepKind::DeleteOtherTakes {
            clip: added.clip,
            index: 1
        }
    );
    let after = clip(&e.doc);
    assert_eq!((after.takes.len(), after.active), (1, 0));
    assert!(
        Arc::ptr_eq(&after.keys()[0].take, &before.keys()[1].take),
        "the same take, not a copy"
    );
    assert_eq!(after.keys()[0].offset, before.keys()[1].offset);
    assert_eq!(
        (after.start, after.end, &after.name),
        (before.start, before.end, &before.name)
    );
    assert_eq!(e.doc.next_id, doc.next_id);
    assert_eq!(
        delete_other_takes(&e.doc, added.clip),
        Err(EditError::Unchanged),
        "one take is left"
    );
    assert_eq!(
        delete_other_takes(&doc, ClipId(999)),
        Err(EditError::NoSuchClip(ClipId(999)))
    );

    let mut h = crate::History::new();
    let mut now = doc.clone();
    h.apply(&mut now, e);
    assert_eq!(clip(&now), after);
    assert_eq!(
        h.undo(&mut now),
        Some(StepKind::DeleteOtherTakes {
            clip: added.clip,
            index: 1
        })
    );
    assert_eq!(clip(&now), before, "every take back, the same one shown");
    assert!(h.redo(&mut now).is_some());
    assert_eq!(clip(&now), after);
}

#[test]
fn deleting_the_other_takes_of_a_clip_of_one_take_is_no_change() {
    let (doc, added) = record(&Doc::new_default(), T1, vec![take(0, 2 * Q)], 0);
    assert_eq!(
        delete_other_takes(&doc, added.clip),
        Err(EditError::Unchanged)
    );
}

#[test]
fn adding_a_track_below_another_with_a_name_a_color_and_a_fresh_id() {
    use crate::BuiltinInstrument as B;
    let bass = InstrumentRef::Builtin(B::SynthBass);
    let (doc, _) = record(&two_tracks(), T2, vec![take(0, 2 * Q)], 0);
    assert_eq!(
        doc.next_id, 6,
        "the three inserts of a new song, a clip and a take"
    );

    let (e, id) = add_track(&doc, Some(T1), bass.clone(), crate::Language::Ja).unwrap();
    assert_eq!(id, TrackId(6), "past every track, clip and take id");
    assert_eq!(e.step, StepKind::AddTrack { track: id });
    assert_eq!(e.doc.next_id, 7);
    let rows = |doc: &Doc| -> Vec<(u32, String, u8)> {
        doc.tracks
            .iter()
            .map(|t| (t.id.0, t.name.clone(), t.color.get()))
            .collect()
    };
    assert_eq!(
        rows(&e.doc),
        [
            (1, "ピアノ".to_owned(), 1),
            (6, "ベース 2".to_owned(), 2),
            (2, "ベース".to_owned(), 2)
        ],
        "right below the first track, with the color of its place"
    );
    let new = &e.doc.tracks[1];
    assert_eq!(new.instrument, bass);
    assert!(new.clips.is_empty() && !new.muted && new.channel == Channel::default());
    assert!(Arc::ptr_eq(&doc.tracks[1], &e.doc.tracks[2]));

    // The smallest free number from 2, and at the end without a track to go below.
    let (e2, id2) = add_track(
        &e.doc,
        None,
        InstrumentRef::Builtin(B::FingerBass),
        crate::Language::Ja,
    )
    .unwrap();
    assert_eq!(id2, TrackId(7));
    assert_eq!(rows(&e2.doc)[3], (7, "ベース 3".to_owned(), 4));
    let renamed = rename_track(&e2.doc, TrackId(6), "低い音").unwrap().doc;
    let (e3, _) = add_track(&renamed, None, bass.clone(), crate::Language::Ja).unwrap();
    assert_eq!(e3.doc.tracks[4].name, "ベース 2");
    assert_eq!(e3.doc.tracks[4].color.get(), 5);
    let (e4, _) = add_track(
        &e3.doc,
        None,
        InstrumentRef::Builtin(B::StudioKit),
        crate::Language::Ja,
    )
    .unwrap();
    assert_eq!(e4.doc.tracks[5].name, "ドラム");
    assert_eq!(
        e4.doc.tracks[5].color.get(),
        1,
        "the sixth is color 1 again"
    );

    // A track id above `next_id` (the tracks of a song from a file are not counted in it).
    let fresh = Doc {
        next_id: 1,
        ..Doc::new_default()
    };
    let (e, id) = add_track(&fresh, Some(T1), InstrumentRef::SINE, crate::Language::Ja).unwrap();
    assert_eq!((id, e.doc.next_id), (TrackId(2), 3));
    assert_eq!(e.doc.tracks[1].name, "シンセ");

    // A track that is gone keeps its id to itself.
    let none = delete_track(&fresh, T1).unwrap().doc;
    assert_eq!(none.next_id, 2);
    let (e, id) = add_track(&none, None, InstrumentRef::GRAND_PIANO, crate::Language::Ja).unwrap();
    assert_eq!((id, e.doc.tracks.len()), (TrackId(2), 1));
    let two = Doc {
        next_id: 1,
        ..two_tracks()
    };
    let less = delete_track(&two, T2).unwrap().doc;
    let (_, id) = add_track(&less, None, InstrumentRef::SINE, crate::Language::Ja).unwrap();
    assert_eq!(id, TrackId(3), "not the id the deleted track had");
    assert_eq!(
        add_track(
            &doc,
            Some(TrackId(9)),
            InstrumentRef::SINE,
            crate::Language::Ja
        ),
        Err(EditError::NoSuchTrack(TrackId(9)))
    );
    let full = Doc {
        next_id: u32::MAX,
        ..doc.clone()
    };
    assert_eq!(
        add_track(&full, None, InstrumentRef::SINE, crate::Language::Ja),
        Err(EditError::OutOfIds)
    );
}

#[test]
fn a_new_track_is_named_by_its_instrument() {
    use crate::BuiltinInstrument as B;
    let mut doc = Doc::new_default();
    let mut names = Vec::new();
    for b in [
        B::OrganChurch,
        B::Organ,
        B::Vibraphone,
        B::Marimba,
        B::GuitarNylon,
        B::Trumpet,
        B::ElectricPiano,
    ] {
        let (e, id) =
            add_track(&doc, None, InstrumentRef::Builtin(b), crate::Language::Ja).unwrap();
        names.push(e.doc.track(id).unwrap().name.clone());
        doc = e.doc;
    }
    assert_eq!(
        names,
        [
            "オルガン",
            "オルガン 2",
            "マレット",
            "マレット 2",
            "ギター",
            "管楽器",
            "ピアノ 2"
        ]
    );
}

#[test]
fn replacing_an_instrument_keeps_the_name_and_the_clips() {
    let (doc, added) = record(&two_tracks(), T2, vec![take(0, 2 * Q)], 0);
    let organ = InstrumentRef::Builtin(crate::BuiltinInstrument::Organ);
    let e = set_instrument(&doc, T2, organ.clone()).unwrap();
    assert_eq!(e.step, StepKind::SetInstrument { track: T2 });
    let t = &e.doc.tracks[1];
    assert_eq!((t.name.as_str(), &t.instrument), ("ベース", &organ));
    assert!(Arc::ptr_eq(
        &t.clips[0],
        &doc.clip(added.clip).unwrap().1.clone()
    ));
    assert!(Arc::ptr_eq(&doc.tracks[0], &e.doc.tracks[0]));
    assert_eq!(
        set_instrument(&doc, T2, InstrumentRef::SINE),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        set_instrument(&doc, TrackId(9), organ),
        Err(EditError::NoSuchTrack(TrackId(9)))
    );
}
