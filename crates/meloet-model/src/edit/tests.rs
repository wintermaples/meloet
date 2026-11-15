use std::sync::Arc;

use super::*;
use crate::{InstrumentRef, TrackColor};

const T1: TrackId = TrackId(1);
const T2: TrackId = TrackId(2);

fn note(start: u64, length: u64, key: u8) -> Note {
    Note {
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
        id: T2,
        name: "ベース".to_owned(),
        color: TrackColor::for_index(1),
        instrument: InstrumentRef::Placeholder,
        gain_db: 0.0,
        clips: Vec::new(),
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
    let (edit, added) = add_recording(&doc, T1, vec![take(1920, 960)], 0).unwrap();
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
    assert_eq!(clip.id, ClipId(1));
    assert_eq!(clip.name, "ピアノ");
    assert_eq!((clip.start, clip.end), (1920, 2880));
    assert_eq!(clip.takes[0].offset, 0);
    assert_eq!(clip.takes[0].take.id, TakeId(2));
    assert_eq!(clip.takes[0].take.notes, [note(0, 1, 60)]);
    assert_eq!(clip.active, 0);
    assert_eq!(
        added,
        RecordingAdded {
            clip: ClipId(1),
            active: 0
        }
    );
    assert_eq!(doc.next_id, 3);
}

#[test]
fn recording_away_from_clips_makes_another_clip_in_order() {
    let (doc, a) = record(&Doc::new_default(), T1, vec![take(3840, 960)], 0);
    let (doc, b) = record(&doc, T1, vec![take(0, 960)], 0);
    assert_ne!(a.clip, b.clip);
    assert_eq!(clips(&doc, T1), [(0, 960, 1), (3840, 4800, 1)]);
}

#[test]
fn touching_ends_do_not_merge() {
    let (doc, _) = record(&Doc::new_default(), T1, vec![take(0, 1920)], 0);
    let (doc, _) = record(&doc, T1, vec![take(1920, 1920)], 0);
    let (doc, _) = record(&doc, T1, vec![take(0, 1920)], 0);
    // The third overlaps only the first.
    assert_eq!(clips(&doc, T1), [(0, 1920, 2), (1920, 3840, 1)]);
}

#[test]
fn overlapping_one_clip_adds_a_take_to_it() {
    let (doc, first) = record(&Doc::new_default(), T1, vec![take(0, 1920)], 0);
    let doc = rename(&doc, first.clip, "Aメロ");
    let old_take = doc.track(T1).unwrap().clips[0].takes[0].take.clone();
    let (doc, added) = record(&doc, T1, vec![take(960, 1920)], 0);
    let clip = &doc.track(T1).unwrap().clips[0];
    assert_eq!(doc.track(T1).unwrap().clips.len(), 1);
    assert_eq!(clip.id, first.clip);
    assert_eq!(clip.name, "Aメロ");
    assert_eq!((clip.start, clip.end), (0, 2880));
    assert!(Arc::ptr_eq(&clip.takes[0].take, &old_take));
    assert_eq!(
        clip.takes.iter().map(|t| t.offset).collect::<Vec<_>>(),
        [0, 960]
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
    let zeros: Vec<u64> = clip.takes.iter().map(|t| clip.start + t.offset).collect();
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
    let offsets: Vec<u64> = clip.takes.iter().map(|t| t.offset).collect();
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
    let passes = vec![take(1920, 1920), take(1920, 1920), take(1920, 960)];
    let (doc, added) = record(&Doc::new_default(), T1, passes, 1);
    assert_eq!(clips(&doc, T1), [(1920, 3840, 3)]);
    assert_eq!(added.active, 1);
    let ids: Vec<u32> = doc.track(T1).unwrap().clips[0]
        .takes
        .iter()
        .map(|t| t.take.id.0)
        .collect();
    assert_eq!(ids, [2, 3, 4]);

    // Looping over the same bars again adds after the takes already there.
    let (edit, added) =
        add_recording(&doc, T1, vec![take(1920, 1920), take(1920, 1920)], 1).unwrap();
    assert_eq!(clips(&edit.doc, T1), [(1920, 3840, 5)]);
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
    let (doc, _) = record(&two_tracks(), T2, vec![take(0, 960)], 0);
    let (doc, _) = record(&doc, T1, vec![take(0, 960)], 0);
    let (new, _) = record(&doc, T1, vec![take(4000, 960)], 0);
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
        |t| t.notes = vec![note(900, 100, 60)],
        |t| t.notes[0].velocity = 0,
        |t| t.notes[0].key = 128,
        |t| t.notes = vec![note(10, 1, 60), note(5, 1, 60)],
        |t| {
            t.controls = vec![ControlPoint {
                tick: 960,
                value: ControlValue::PitchBend(0),
            }]
        },
    ];
    for (i, change) in bad.iter().enumerate() {
        let mut t = take(0, 960);
        change(&mut t);
        assert_eq!(
            add_recording(&doc, T1, vec![take(0, 960), t], 0),
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
        vec![take(0, 960), take(0, 960), take(0, 960)],
        2,
    );
    let (doc, _) = record(&doc, T2, vec![take(0, 960)], 0);
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
    for (a, b) in clip.takes.iter().zip(&old.takes) {
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
    let (doc, a) = record(&two_tracks(), T1, vec![take(0, 960)], 0);
    let (doc, b) = record(&doc, T1, vec![take(2000, 960)], 0);
    let (doc, c) = record(&doc, T2, vec![take(0, 960)], 0);
    let edit = delete_clips(&doc, &[a.clip, c.clip]).unwrap();
    assert_eq!(edit.step, StepKind::DeleteClips { count: 2 });
    assert_eq!(clips(&edit.doc, T1), [(2000, 2960, 1)]);
    assert!(clips(&edit.doc, T2).is_empty());
    assert!(Arc::ptr_eq(
        &doc.track(T1).unwrap().clips[1],
        &edit.doc.track(T1).unwrap().clips[0]
    ));

    let only_t1 = delete_clips(&doc, &[b.clip]).unwrap();
    assert!(Arc::ptr_eq(&doc.tracks[1], &only_t1.doc.tracks[1]));

    assert_eq!(
        delete_clips(&doc, &[a.clip, ClipId(999)]),
        Err(EditError::NoSuchClip(ClipId(999)))
    );
    assert_eq!(delete_clips(&doc, &[]), Err(EditError::Unchanged));
}

#[test]
fn tempo_and_meter() {
    let doc = Doc::new_default();
    let t = Tempo::from_centi(12_050).unwrap();
    let edit = set_tempo(&doc, t).unwrap();
    assert_eq!(edit.doc.tempo, t);
    assert_eq!(
        edit.step,
        StepKind::Tempo {
            from: doc.tempo,
            to: t
        }
    );
    assert!(Arc::ptr_eq(&doc.tracks[0], &edit.doc.tracks[0]));
    assert_eq!(set_tempo(&doc, doc.tempo), Err(EditError::Unchanged));

    let m = Meter::new(6, 8).unwrap();
    let edit = set_meter(&doc, m).unwrap();
    assert_eq!(edit.doc.meter, m);
    assert_eq!(
        edit.step,
        StepKind::Meter {
            from: Meter::FOUR_FOUR,
            to: m
        }
    );
    assert_eq!(set_meter(&doc, Meter::FOUR_FOUR), Err(EditError::Unchanged));
}
