use std::sync::Arc;

use super::*;
use crate::arrange::split_clips;
use crate::audio::clips::{ImportPlan, ImportedAudio, import_audio};
use crate::history::History;
use crate::song::Song;
use crate::tempo_map::TempoPoint;
use crate::{FileClock, NewAudio, TrackId};

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

/// The default song at 120 a minute with a file of four seconds (two bars) brought in at
/// bar 2. Returns the clip's id.
fn doc() -> (Doc, ClipId) {
    let mut song = Song::new_default();
    let mut timeline = (*song.doc.timeline).clone();
    timeline.tempo = tempo(12_000);
    song.doc.timeline = Arc::new(timeline);
    let file = ImportedAudio {
        audio: NewAudio {
            uid: [5; 16],
            file: "ドラム.wav".to_owned(),
            frames: 4 * 44_100,
            sample_rate: 44_100,
            channels: 2,
            bits: Some(16),
        },
        sha256: [3; 32],
        name: "ドラム".to_owned(),
    };
    let (e, made) = import_audio(
        &song.doc,
        vec![file],
        ImportPlan::Separate,
        BAR,
        crate::Language::Ja,
    )
    .unwrap();
    (e.doc, made[0].clip)
}

fn shape(doc: &Doc, clip: ClipId) -> AudioShape {
    *doc.clip(clip).unwrap().1.takes.shape().unwrap()
}

fn shown(doc: &Doc, clip: ClipId) -> Arc<AudioTake> {
    let (_, c) = doc.clip(clip).unwrap();
    c.active_audio().unwrap().take.clone()
}

#[test]
fn the_switches_and_the_pitch_are_steps_and_stay_in_their_ranges() {
    let (doc, clip) = doc();
    assert_eq!(
        shape(&doc, clip),
        AudioShape {
            follow: false,
            ..AudioShape::RECORDED
        }
    );

    let e = set_clip_follow(&doc, clip, true).unwrap();
    assert_eq!(e.step, StepKind::SetClipFollow { clip });
    assert!(shape(&e.doc, clip).follow);
    assert_eq!(
        set_clip_follow(&doc, clip, false).unwrap_err(),
        EditError::Unchanged
    );

    let e = set_clip_semitones(&doc, clip, 2).unwrap();
    assert_eq!(e.step, StepKind::SetClipPitch { clip });
    assert_eq!(shape(&e.doc, clip).semitones, 2);
    let semitones = |n| shape(&set_clip_semitones(&doc, clip, n).unwrap().doc, clip).semitones;
    assert_eq!(
        (semitones(40), semitones(-13), semitones(-12)),
        (12, -12, -12)
    );
    assert_eq!(
        set_clip_semitones(&doc, clip, 0).unwrap_err(),
        EditError::Unchanged
    );

    let e = set_clip_cents(&doc, clip, -30).unwrap();
    assert_eq!(e.step, StepKind::SetClipPitch { clip });
    assert_eq!(shape(&e.doc, clip).cents, -30);
    let cents = |n| shape(&set_clip_cents(&doc, clip, n).unwrap().doc, clip).cents;
    assert_eq!((cents(51), cents(i32::MIN), cents(50)), (50, -50, 50));

    let e = set_clip_keep_formant(&doc, clip, false).unwrap();
    assert_eq!(e.step, StepKind::SetClipPitch { clip });
    assert!(!shape(&e.doc, clip).keep_formant);
    assert!(shape(&e.doc, clip).is_valid());
    assert_eq!(
        set_clip_keep_formant(&doc, clip, true).unwrap_err(),
        EditError::Unchanged
    );

    // Nothing else of the clip moves, and the take is the same one.
    let (_, before) = doc.clip(clip).unwrap();
    let (_, after) = e.doc.clip(clip).unwrap();
    assert_eq!((after.start, after.end), (before.start, before.end));
    assert!(Arc::ptr_eq(&shown(&doc, clip), &shown(&e.doc, clip)));

    // Not for a clip of notes, nor for no clip.
    let keys = crate::edit::add_recording(
        &doc,
        TrackId(1),
        vec![crate::NewTake {
            origin: 0,
            length: BAR,
            notes: Vec::new(),
            controls: Vec::new(),
        }],
        0,
    )
    .unwrap();
    let (keys_doc, keys_clip) = (keys.0.doc, keys.1.clip);
    assert_eq!(
        set_clip_follow(&keys_doc, keys_clip, true).unwrap_err(),
        EditError::WrongKind
    );
    assert_eq!(
        set_clip_semitones(&keys_doc, keys_clip, 1).unwrap_err(),
        EditError::WrongKind
    );
    assert_eq!(
        set_source_tempo(&keys_doc, keys_clip, tempo(9_600)).unwrap_err(),
        EditError::WrongKind
    );
    assert_eq!(
        guess_source_tempo(&keys_doc, keys_clip).unwrap_err(),
        EditError::WrongKind
    );
    assert_eq!(
        set_clip_cents(&doc, ClipId(999), 1).unwrap_err(),
        EditError::NoSuchClip(ClipId(999))
    );
}

#[test]
fn tuning_takes_the_scale_when_the_strength_leaves_nothing_and_keeps_it_after() {
    use crate::roll::ScaleMode;
    let (doc, clip) = doc();
    let tune = |doc: &Doc| shape(doc, clip).tune;
    assert_eq!(tune(&doc), None);
    // At nothing there is nothing to change: not the strength, not the scale.
    assert_eq!(
        set_clip_tune_strength(&doc, clip, 0, 9, ScaleMode::Minor).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        set_clip_tune_scale(&doc, clip, 9, ScaleMode::Minor).unwrap_err(),
        EditError::Unchanged
    );

    let e = set_clip_tune_strength(&doc, clip, 60, 9, ScaleMode::Minor).unwrap();
    assert_eq!(e.step, StepKind::SetClipTune { clip });
    let sixty = Tune {
        root: 9,
        mode: ScaleMode::Minor,
        strength: 60,
    };
    assert_eq!(tune(&e.doc), Some(sixty));
    assert!(shape(&e.doc, clip).is_valid());
    // Nothing else of the shape moves, and the take is the same one.
    assert_eq!(
        AudioShape {
            tune: None,
            ..shape(&e.doc, clip)
        },
        shape(&doc, clip)
    );
    assert!(Arc::ptr_eq(&shown(&doc, clip), &shown(&e.doc, clip)));
    let tuned = e.doc;

    // Another strength keeps the scale the clip took, whatever the song's is now.
    let e = set_clip_tune_strength(&tuned, clip, 100, 0, ScaleMode::Blues).unwrap();
    assert_eq!(
        tune(&e.doc),
        Some(Tune {
            strength: 100,
            ..sixty
        })
    );
    assert_eq!(
        set_clip_tune_strength(&tuned, clip, 60, 0, ScaleMode::Blues).unwrap_err(),
        EditError::Unchanged
    );
    // Out of the range it stops at the ends.
    let strength = |n| {
        tune(
            &set_clip_tune_strength(&tuned, clip, n, 0, ScaleMode::Major)
                .unwrap()
                .doc,
        )
        .map(|t| t.strength)
    };
    assert_eq!(
        (strength(101), strength(i32::MAX), strength(1)),
        (Some(100), Some(100), Some(1))
    );
    assert_eq!((strength(0), strength(-5)), (None, None));

    // 「いまのスケールにする」 takes the scale and keeps the strength.
    let e = set_clip_tune_scale(&tuned, clip, 0, ScaleMode::Blues).unwrap();
    assert_eq!(e.step, StepKind::SetClipTune { clip });
    assert_eq!(
        tune(&e.doc),
        Some(Tune {
            root: 0,
            mode: ScaleMode::Blues,
            strength: 60
        })
    );
    assert_eq!(
        set_clip_tune_scale(&tuned, clip, 9, ScaleMode::Minor).unwrap_err(),
        EditError::Unchanged
    );
    // A root that is no note changes nothing.
    assert_eq!(
        set_clip_tune_scale(&tuned, clip, 12, ScaleMode::Major).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        set_clip_tune_strength(&doc, clip, 50, 12, ScaleMode::Major).unwrap_err(),
        EditError::Unchanged
    );

    // Back to nothing and up again: the scale of that moment, not the old one.
    let off = set_clip_tune_strength(&tuned, clip, 0, 0, ScaleMode::Major)
        .unwrap()
        .doc;
    assert_eq!(shape(&off, clip), shape(&doc, clip));
    let again = set_clip_tune_strength(&off, clip, 30, 2, ScaleMode::Dorian)
        .unwrap()
        .doc;
    assert_eq!(
        tune(&again),
        Some(Tune {
            root: 2,
            mode: ScaleMode::Dorian,
            strength: 30
        })
    );

    // One step each, and undo brings the clip back.
    let mut history = History::new();
    let mut now = doc.clone();
    let e = set_clip_tune_strength(&now, clip, 40, 0, ScaleMode::Major).unwrap();
    history.apply(&mut now, e);
    assert!(tune(&now).is_some());
    assert!(history.undo(&mut now).is_some());
    assert_eq!(tune(&now), None);
    assert!(history.redo(&mut now).is_some());
    assert_eq!(tune(&now).map(|t| t.strength), Some(40));

    // Not for a clip of notes, nor for no clip.
    assert_eq!(
        set_clip_tune_strength(&doc, ClipId(999), 50, 0, ScaleMode::Major).unwrap_err(),
        EditError::NoSuchClip(ClipId(999))
    );
}

#[test]
fn the_tempo_of_the_audio_makes_a_new_take_in_the_same_place() {
    let (doc, clip) = doc();
    let old = shown(&doc, clip);
    assert_eq!(old.sync, None);

    let (e, id) = set_source_tempo(&doc, clip, tempo(9_600)).unwrap();
    assert_eq!(e.step, StepKind::SetSourceTempo { clip });
    let new = shown(&e.doc, clip);
    assert_eq!(new.id, id);
    assert_ne!(new.id, old.id);
    assert_eq!(id.0, doc.next_id);
    assert_eq!(e.doc.next_id, doc.next_id + 1);
    assert_eq!(new.sync, Some(SourceTempo::steady(tempo(9_600))));
    assert!(Arc::ptr_eq(&new.source, &old.source));
    assert_eq!((new.first_frame, new.frames), (old.first_frame, old.frames));
    // The old take is as it was.
    assert_eq!(old.sync, None);
    let (_, before) = doc.clip(clip).unwrap();
    let (_, after) = e.doc.clip(clip).unwrap();
    assert_eq!(after.takes.len(), before.takes.len());
    assert_eq!(after.active, before.active);
    assert_eq!(after.active_zero(), before.active_zero());
    assert_eq!((after.start, after.end), (before.start, before.end));
    assert_eq!(shape(&e.doc, clip), shape(&doc, clip));

    // The same tempo again is no step; another one is, with one more id.
    assert_eq!(
        set_source_tempo(&e.doc, clip, tempo(9_600)).unwrap_err(),
        EditError::Unchanged
    );
    let (again, next) = set_source_tempo(&e.doc, clip, tempo(9_601)).unwrap();
    assert_eq!(next.0, id.0 + 1);
    assert_eq!(
        shown(&again.doc, clip).sync.clone().unwrap().tempo,
        tempo(9_601)
    );

    // Undo brings the old take back, redo the new one.
    let mut now = doc.clone();
    let mut history = History::new();
    history.apply(&mut now, e);
    assert_eq!(shown(&now, clip).id, id);
    assert_eq!(
        history.undo(&mut now),
        Some(StepKind::SetSourceTempo { clip })
    );
    assert!(Arc::ptr_eq(&shown(&now, clip), &old));
    history.redo(&mut now);
    assert_eq!(shown(&now, clip).id, id);
}

#[test]
fn a_recorded_tempo_with_points_becomes_one_tempo() {
    let (doc, clip) = doc();
    // As if recorded through a ramp.
    let (ti, ci) = clip_index(&doc, clip).unwrap();
    let mut track = Track::clone(&doc.tracks[ti]);
    let c = Clip::clone(&track.clips[ci]);
    let ClipTakes::Vocal { takes, shape } = &c.takes else {
        unreachable!()
    };
    let ramped = SourceTempo {
        tempo: tempo(12_000),
        head_ramp: true,
        points: vec![TempoPoint {
            tick: BAR,
            tempo: tempo(9_000),
            ramp: false,
        }],
    };
    let take = AudioTake {
        sync: Some(ramped),
        ..AudioTake::clone(&takes[0].take)
    };
    track.clips[ci] = Arc::new(Clip {
        takes: ClipTakes::Vocal {
            takes: vec![ClipTake {
                take: Arc::new(take),
                offset: 0,
            }],
            shape: *shape,
        },
        ..c.clone()
    });
    let doc = with_track(&doc, ti, track);
    // The head's tempo typed again is still a change: the points go.
    let (e, _) = set_source_tempo(&doc, clip, tempo(12_000)).unwrap();
    assert_eq!(
        shown(&e.doc, clip).sync,
        Some(SourceTempo::steady(tempo(12_000)))
    );
}

#[test]
fn of_two_clips_cut_from_one_only_the_changed_one_gets_the_new_take() {
    let (doc, clip) = doc();
    let cut = split_clips(&doc, &[clip], BAR + 2 * Q).unwrap();
    let right = cut.right[0];
    let doc = cut.edit.doc;
    assert!(Arc::ptr_eq(&shown(&doc, clip), &shown(&doc, right)));

    let (e, id) = set_source_tempo(&doc, right, tempo(10_000)).unwrap();
    assert_eq!(shown(&e.doc, right).id, id);
    assert!(Arc::ptr_eq(&shown(&e.doc, clip), &shown(&doc, clip)));
    assert_eq!(shown(&e.doc, clip).sync, None);
    // The right half still starts half a bar into the take.
    let (_, r) = e.doc.clip(right).unwrap();
    assert_eq!(r.active_zero(), BAR as i64);
    assert_eq!(r.start, BAR + 2 * Q);
}

#[test]
fn following_changes_where_the_file_is() {
    let (doc, clip) = doc();
    let map = TempoMap::new(&doc.timeline);
    let end = (BAR + 2 * BAR) as f64;
    let clock = |doc: &Doc| FileClock::of_clip(doc.clip(clip).unwrap().1).unwrap();
    let frame = |doc: &Doc| clock(doc).frame_at(&map, end);
    // As brought in: four seconds of the file over two bars at 120.
    assert_eq!(frame(&doc), 4.0 * 44_100.0);
    // Asked to follow with no tempo of its own: still the file's speed.
    let follow = set_clip_follow(&doc, clip, true).unwrap().doc;
    assert!(!clock(&follow).follows());
    assert_eq!(frame(&follow), 4.0 * 44_100.0);
    // Played at 96, it says: two bars of it are five seconds of the file.
    let (e, _) = set_source_tempo(&follow, clip, tempo(9_600)).unwrap();
    assert!(clock(&e.doc).follows());
    assert_eq!(frame(&e.doc), 5.0 * 44_100.0);
    assert_eq!(clock(&e.doc).ratio_at(&map, BAR as f64), Some(1.25));
    // With a tempo and not following: the file's speed again.
    let off = set_clip_follow(&e.doc, clip, false).unwrap().doc;
    assert_eq!(frame(&off), 4.0 * 44_100.0);
}

#[test]
fn guesses_are_whole_bars_between_60_and_180() {
    let four = Meter::FOUR_FOUR;
    let guess = |bars, centi| TempoGuess {
        bars,
        tempo: tempo(centi),
    };
    // Four seconds: 1 bar at 60, 2 at 120 (4 at 240 is out).
    assert_eq!(
        source_tempo_guesses(4 * 48_000, 48_000, four),
        [guess(1, 6_000), guess(2, 12_000)]
    );
    // Ten seconds: 4 bars at 96 (2 at 48 and 8 at 192 are out).
    assert_eq!(
        source_tempo_guesses(441_000, 44_100, four),
        [guess(4, 9_600)]
    );
    // The ends of the range are in it.
    assert_eq!(
        source_tempo_guesses(48_000 * 4 / 3, 48_000, four),
        [guess(1, 18_000)]
    );
    assert_eq!(
        source_tempo_guesses(32 * 48_000, 48_000, four),
        [guess(8, 6_000)]
    );
    // Too short and too long for any, and nothing at all.
    assert_eq!(source_tempo_guesses(48_000, 48_000, four), []);
    assert_eq!(source_tempo_guesses(33 * 48_000, 48_000, four), []);
    assert_eq!(source_tempo_guesses(0, 48_000, four), []);
    assert_eq!(source_tempo_guesses(48_000, 0, four), []);
    // A bar of 6/8 is three quarter notes: three seconds are 1 bar at 60 and 2 at 120.
    assert_eq!(
        source_tempo_guesses(3 * 48_000, 48_000, Meter::new(6, 8).unwrap()),
        [guess(1, 6_000), guess(2, 12_000)]
    );
    // To the hundredth: 4 bars in 7 seconds are 137.142857….
    assert_eq!(
        source_tempo_guesses(7 * 48_000, 48_000, four),
        [guess(2, 6_857), guess(4, 13_714)]
    );
}

#[test]
fn the_guess_nearest_to_the_songs_tempo_is_picked() {
    let guesses = source_tempo_guesses(4 * 48_000, 48_000, Meter::FOUR_FOUR);
    let bars = |bpm| pick_guess(&guesses, bpm).unwrap().bars;
    assert_eq!((bars(120.0), bars(70.0), bars(100.0)), (2, 1, 2));
    // As a ratio 84.85 is the middle of 60 and 120.
    assert_eq!((bars(84.0), bars(86.0)), (1, 2));
    assert_eq!(pick_guess(&[], 120.0), None);

    // The clip's own: four seconds at bar 2 of a song at 120.
    let (doc, clip) = doc();
    assert_eq!(
        guess_source_tempo(&doc, clip),
        Ok(Some(TempoGuess {
            bars: 2,
            tempo: tempo(12_000)
        }))
    );
    assert_eq!(
        guess_source_tempo(&doc, ClipId(999)),
        Err(EditError::NoSuchClip(ClipId(999)))
    );
}
