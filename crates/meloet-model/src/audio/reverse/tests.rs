use std::sync::Arc;

use super::*;
use crate::arrange::{move_clips, split_clips, trim_clip};
use crate::audio::clips::{ImportPlan, ImportedAudio, import_audio, set_clip_fade};
use crate::audio::record::{NewAudioTake, VocalRecording, add_vocal_recordings};
use crate::audio::stretch::set_clip_follow;
use crate::edit::add_vocal_track;
use crate::history::History;
use crate::tempo_map::TempoPoint;
use crate::time::TICKS_PER_QUARTER;
use crate::{ClipEdge, FadeEnd};

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
/// A bar at 120 a minute, at 48 kHz.
const BAR_FRAMES: i64 = 96_000;

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

fn at_tempo(doc: &Doc, centi: u32) -> Doc {
    let mut doc = doc.clone();
    let mut timeline = (*doc.timeline).clone();
    timeline.tempo = tempo(centi);
    doc.timeline = Arc::new(timeline);
    doc
}

/// The default song at 120 a minute with a file of four seconds at 48 kHz (two bars)
/// brought in at bar 2.
fn imported() -> (Doc, ClipId) {
    let doc = at_tempo(&Doc::new_default(), 12_000);
    let file = ImportedAudio {
        audio: NewAudio {
            uid: [5; 16],
            file: "ドラム.wav".to_owned(),
            frames: 4 * 48_000,
            sample_rate: 48_000,
            channels: 2,
            bits: Some(24),
        },
        sha256: [3; 32],
        name: "ドラム".to_owned(),
    };
    let (e, made) = import_audio(&doc, vec![file], ImportPlan::Separate, BAR).unwrap();
    (e.doc, made[0].clip)
}

/// A recording of two bars from bar 2 into `doc`: the file starts half a second before the
/// take, which is three laps long; the second lap is shown.
fn recorded(doc: &Doc) -> (Doc, ClipId) {
    let (e, track) = add_vocal_track(doc, None).unwrap();
    let lap = |first_frame| NewAudioTake {
        origin: BAR,
        length: 2 * BAR,
        first_frame,
        frames: 2 * BAR_FRAMES as u64,
    };
    let recording = VocalRecording {
        track,
        audio: NewAudio {
            uid: [7; 16],
            file: "歌 9.wav".to_owned(),
            frames: 24_000 + 6 * BAR_FRAMES as u64,
            sample_rate: 48_000,
            channels: 1,
            bits: Some(24),
        },
        takes: vec![
            lap(24_000),
            lap(24_000 + 2 * BAR_FRAMES as u64),
            lap(24_000 + 4 * BAR_FRAMES as u64),
        ],
    };
    let (e, added) = add_vocal_recordings(&e.doc, vec![recording], None).unwrap();
    let mut doc = e.doc;
    // The second lap.
    let clip = added[0].clip;
    let (ti, ci) = clip_index(&doc, clip).unwrap();
    let mut track = Track::clone(&doc.tracks[ti]);
    let mut c = Clip::clone(&track.clips[ci]);
    c.active = 1;
    track.clips[ci] = Arc::new(c);
    doc.tracks[ti] = Arc::new(track);
    (doc, clip)
}

fn clip_of(doc: &Doc, clip: ClipId) -> Arc<Clip> {
    doc.clip(clip).unwrap().1.clone()
}

fn reversed(played: &PlayedFrames, uid: u8, file: &str) -> Reversed {
    Reversed {
        played: played.clone(),
        audio: NewAudio {
            uid: [uid; 16],
            file: file.to_owned(),
            frames: played.backwards_frames(),
            sample_rate: played.source.sample_rate,
            channels: played.source.channels,
            bits: played.source.bits,
        },
    }
}

#[test]
fn the_frames_a_clip_plays_follow_its_ends_and_its_take() {
    let (doc, clip) = imported();
    let played = played_frames(&doc, clip).unwrap();
    assert_eq!(
        (played.from, played.to, played.audio_end),
        (0, 2 * BAR_FRAMES, 192_000)
    );
    assert_eq!(played.sounding(), 0..192_000);
    assert_eq!(played.backwards_frames(), 192_000);
    let take = clip_of(&doc, clip).active_take_id();
    assert_eq!((played.clip, played.take), (clip, take));

    // A quarter trimmed off the head, and the end pulled a bar past the file's.
    let doc = trim_clip(&doc, clip, ClipEdge::Start, Q as i64)
        .unwrap()
        .doc;
    let doc = trim_clip(&doc, clip, ClipEdge::End, BAR as i64)
        .unwrap()
        .doc;
    let played = played_frames(&doc, clip).unwrap();
    assert_eq!((played.from, played.to), (24_000, 3 * BAR_FRAMES));
    assert_eq!(played.sounding(), 24_000..192_000);
    // What is silent at the end is part of the backwards file; its head is silence.
    assert_eq!(played.backwards_frames(), 3 * BAR_FRAMES as u64 - 24_000);

    // The head pulled out a bar before the file: silent there.
    let (doc, clip) = imported();
    let doc = trim_clip(&doc, clip, ClipEdge::Start, -(BAR as i64))
        .unwrap()
        .doc;
    let played = played_frames(&doc, clip).unwrap();
    assert_eq!((played.from, played.to), (-BAR_FRAMES, 2 * BAR_FRAMES));
    assert_eq!(played.sounding(), 0..192_000);
    assert_eq!(played.backwards_frames(), 192_000);

    // A clip that plays the file at its own speed plays more of it at a slower tempo.
    let (doc, clip) = imported();
    let slow = at_tempo(&doc, 6_000);
    let played = played_frames(&slow, clip).unwrap();
    assert_eq!((played.from, played.to), (0, 4 * BAR_FRAMES));
    assert_eq!(played.sounding(), 0..192_000);
    assert!(played_frames(&doc, ClipId(9_999)).is_none());
}

#[test]
fn a_clip_that_follows_plays_the_frames_of_its_takes_own_tempo() {
    // Recorded at 120: a bar of the take is two seconds of the file whatever the song does.
    let (doc, clip) = recorded(&at_tempo(&Doc::new_default(), 12_000));
    let lap = 24_000 + 2 * BAR_FRAMES;
    let played = played_frames(&doc, clip).unwrap();
    assert_eq!((played.from, played.to), (lap, lap + 2 * BAR_FRAMES));
    assert_eq!(played.audio_end, (lap + 2 * BAR_FRAMES) as u64);
    let fast = at_tempo(&doc, 24_000);
    assert_eq!(played_frames(&fast, clip), Some(played.clone()));
    // Not following, the same clip plays at the file's speed: half as many frames at 240.
    let free = set_clip_follow(&fast, clip, false).unwrap().doc;
    let played = played_frames(&free, clip).unwrap();
    assert_eq!((played.from, played.to), (lap, lap + BAR_FRAMES));
}

#[test]
fn the_volume_puts_the_loudest_sample_at_minus_one() {
    let (gain, limited) = normalize_gain(0.5).unwrap();
    assert!(!limited);
    assert_eq!(gain.centi(), 502);
    let (gain, limited) = normalize_gain(1.0).unwrap();
    assert_eq!((gain.centi(), limited), (-100, false));
    // A float file louder than full scale.
    let (gain, limited) = normalize_gain(100.0).unwrap();
    assert_eq!((gain.centi(), limited), (-4_100, false));
    // Too quiet to reach −1 dBFS, too loud to come down to it.
    assert_eq!(
        normalize_gain(0.01).map(|(g, l)| (g.db(), l)),
        Some((12.0, true))
    );
    assert_eq!(
        normalize_gain(1e4).map(|(g, l)| (g.db(), l)),
        Some((-60.0, true))
    );
    // Just inside the range.
    let quiet = 10f32.powf(-13.0 / 20.0);
    assert_eq!(
        normalize_gain(quiet).map(|(g, l)| (g.centi(), l)),
        Some((1_200, false))
    );
    for nothing in [0.0, -0.0, f32::NAN, f32::INFINITY] {
        assert_eq!(normalize_gain(nothing), None);
    }
}

#[test]
fn normalizing_is_one_step_for_the_clips_that_still_play_what_was_measured() {
    let (doc, left) = imported();
    let split = split_clips(&doc, &[left], 2 * BAR).unwrap();
    let doc = split.edit.doc;
    let right = split.right[0];
    let measured_left = played_frames(&doc, left).unwrap();
    let measured_right = played_frames(&doc, right).unwrap();
    let quiet = ClipGain::from_db(6.0).unwrap();
    let loud = ClipGain::from_db(-3.0).unwrap();
    let gains = [
        (measured_left.clone(), quiet),
        (measured_right.clone(), loud),
    ];

    let e = normalize_clips(&doc, &gains).unwrap();
    assert_eq!(e.step, StepKind::NormalizeClips { count: 2 });
    let gain = |doc: &Doc, clip| doc.clip(clip).unwrap().1.takes.shape().unwrap().gain;
    assert_eq!((gain(&e.doc, left), gain(&e.doc, right)), (quiet, loud));
    // The fades stay.
    let faded = set_clip_fade(&doc, left, FadeEnd::In, Q).unwrap().doc;
    let e = normalize_clips(&faded, &gains).unwrap();
    let shape = *e.doc.clip(left).unwrap().1.takes.shape().unwrap();
    assert_eq!((shape.fade_in, shape.gain), (Q, quiet));

    // The right one was trimmed since it was measured: only the left one changes.
    let trimmed = trim_clip(&doc, right, ClipEdge::Start, Q as i64)
        .unwrap()
        .doc;
    let e = normalize_clips(&trimmed, &gains).unwrap();
    assert_eq!(e.step, StepKind::NormalizeClips { count: 1 });
    assert_eq!(gain(&e.doc, right), ClipGain::UNITY);
    // Moved, to where the tempo is the same: the same frames.
    let moved = move_clips(&doc, &[right], 2 * BAR as i64, 0, false)
        .unwrap()
        .edit
        .doc;
    assert_eq!(
        normalize_clips(&moved, &gains).unwrap().step,
        StepKind::NormalizeClips { count: 2 }
    );
    // Deleted, or the volume is that already: no step.
    let e = normalize_clips(&doc, &gains).unwrap();
    assert_eq!(normalize_clips(&e.doc, &gains), Err(EditError::Unchanged));
    assert_eq!(normalize_clips(&doc, &[]), Err(EditError::Unchanged));

    let mut history = History::new();
    let mut now = doc.clone();
    history.apply(&mut now, normalize_clips(&doc, &gains).unwrap());
    assert_eq!(
        history.undo(&mut now),
        Some(StepKind::NormalizeClips { count: 2 })
    );
    // Ids are never given twice: only the next one stays where it got to.
    assert_eq!(
        Doc {
            next_id: doc.next_id,
            ..now.clone()
        },
        doc
    );
    history.redo(&mut now);
    assert_eq!(gain(&now, right), loud);
}

#[test]
fn a_reversed_clip_shows_a_new_take_of_the_new_file_in_place_of_the_old_one() {
    let (doc, clip) = recorded(&at_tempo(&Doc::new_default(), 12_000));
    let doc = set_clip_fade(&doc, clip, FadeEnd::Out, Q).unwrap().doc;
    let doc = crate::set_clip_gain(&doc, clip, ClipGain::from_db(-4.0).unwrap())
        .unwrap()
        .doc;
    let before = clip_of(&doc, clip);
    let played = played_frames(&doc, clip).unwrap();
    let made = reversed(&played, 9, "歌 40.wav");

    let ClipsReversed { edit, follow_off } =
        reverse_clips(&doc, std::slice::from_ref(&made)).unwrap();
    assert_eq!(edit.step, StepKind::ReverseClips { count: 1 });
    assert_eq!(follow_off, 0);
    let after = clip_of(&edit.doc, clip);
    assert_eq!(
        (after.start, after.end, after.active),
        (before.start, before.end, 1)
    );
    assert_eq!(after.takes.shape(), before.takes.shape());
    let (old, new) = (before.takes.vocal(), after.takes.vocal());
    assert_eq!(new.len(), 3);
    // The other laps are the same takes.
    assert!(Arc::ptr_eq(&old[0].take, &new[0].take) && Arc::ptr_eq(&old[2].take, &new[2].take));
    let take = &new[1];
    assert_ne!(take.take.id, old[1].take.id);
    assert_eq!(
        (take.offset, take.take.first_frame, take.take.frames),
        (0, 0, 192_000)
    );
    assert_eq!(take.take.sync, Some(SourceTempo::steady(tempo(12_000))));
    // The file is in the table, after the old one, which stays for the other laps.
    let source = &take.take.source;
    assert_eq!(edit.doc.audio.len(), 2);
    assert!(Arc::ptr_eq(&edit.doc.audio[1], source));
    assert_eq!(
        (
            source.uid,
            source.file.as_str(),
            source.frames,
            source.channels,
            source.sha256
        ),
        ([9; 16], "歌 40.wav", 192_000, 1, None)
    );
    assert!(edit.doc.next_id > doc.next_id);
    // Played from its first frame at the clip's start, as many frames as the old take.
    let again = played_frames(&edit.doc, clip).unwrap();
    assert_eq!(
        (again.from, again.to, again.audio_end),
        (0, 192_000, 192_000)
    );

    // The answer comes late: the clip shows another take now, or the step is in.
    let mut switched = edit.doc.clone();
    let (ti, ci) = clip_index(&doc, clip).unwrap();
    let mut track = Track::clone(&doc.tracks[ti]);
    let mut c = Clip::clone(&track.clips[ci]);
    c.active = 0;
    track.clips[ci] = Arc::new(c);
    switched.tracks[ti] = Arc::new(track);
    assert_eq!(
        reverse_clips(&switched, std::slice::from_ref(&made)).map(|_| ()),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        reverse_clips(&edit.doc, std::slice::from_ref(&made)).map(|_| ()),
        Err(EditError::Unchanged)
    );
    // A file that is not as long as the frames is not taken.
    let mut short = made.clone();
    short.audio.frames -= 1;
    assert_eq!(
        reverse_clips(&doc, &[short]).map(|_| ()),
        Err(EditError::Unchanged)
    );

    // One step, back and forth.
    let mut history = History::new();
    let mut now = doc.clone();
    history.apply(&mut now, edit.clone());
    assert_eq!(
        history.undo(&mut now),
        Some(StepKind::ReverseClips { count: 1 })
    );
    // Ids are never given twice: only the next one stays where it got to.
    assert_eq!(
        Doc {
            next_id: doc.next_id,
            ..now.clone()
        },
        doc
    );
    history.redo(&mut now);
    assert_eq!(now, edit.doc);
}

#[test]
fn reversing_the_reversed_take_reads_the_same_frames_backwards() {
    // A clip that starts a quarter into its take (the right half of a cut) and ends a bar
    // after its file.
    let (doc, clip) = imported();
    let split = split_clips(&doc, &[clip], BAR + Q).unwrap();
    let right = split.right[0];
    let doc = trim_clip(&split.edit.doc, right, ClipEdge::End, BAR as i64)
        .unwrap()
        .doc;
    let played = played_frames(&doc, right).unwrap();
    assert_eq!((played.from, played.to), (24_000, 3 * BAR_FRAMES));
    let once = reverse_clips(&doc, &[reversed(&played, 1, "ドラム 1.wav")])
        .unwrap()
        .edit
        .doc;
    let back = played_frames(&once, right).unwrap();
    // The new file holds the clip's frames backwards, the silence of its end at its head.
    assert_eq!(
        (back.from, back.to, back.audio_end),
        (0, 3 * BAR_FRAMES - 24_000, 264_000)
    );
    assert_eq!(back.backwards_frames(), played.backwards_frames());
    let twice = reverse_clips(&once, &[reversed(&back, 2, "ドラム 2.wav")])
        .unwrap()
        .edit
        .doc;
    let last = played_frames(&twice, right).unwrap();
    assert_eq!((last.from, last.to), (0, back.to));
    // The clip itself stays where it was, with its other values.
    let (a, b) = (clip_of(&doc, right), clip_of(&twice, right));
    assert_eq!(
        (a.start, a.end, a.takes.shape()),
        (b.start, b.end, b.takes.shape())
    );
}

#[test]
fn several_clips_reversed_are_one_step_and_each_keeps_its_place() {
    let (doc, clip) = imported();
    let split = split_clips(&doc, &[clip], 2 * BAR).unwrap();
    let doc = split.edit.doc;
    let right = split.right[0];
    let made = [
        reversed(&played_frames(&doc, clip).unwrap(), 1, "ドラム 1.wav"),
        reversed(&played_frames(&doc, right).unwrap(), 2, "ドラム 2.wav"),
    ];
    let e = reverse_clips(&doc, &made).unwrap().edit;
    assert_eq!(e.step, StepKind::ReverseClips { count: 2 });
    assert_eq!(e.doc.audio.len(), 3);
    let file = |clip| {
        clip_of(&e.doc, clip)
            .active_audio()
            .unwrap()
            .take
            .source
            .file
            .clone()
    };
    assert_eq!(
        (file(clip), file(right)),
        ("ドラム 1.wav".to_owned(), "ドラム 2.wav".to_owned())
    );
    // The same clip twice in one answer: the second finds the first's take and is passed by.
    let twice = [
        made[0].clone(),
        reversed(&made[0].played, 3, "ドラム 3.wav"),
    ];
    let e = reverse_clips(&doc, &twice).unwrap().edit;
    assert_eq!(
        (e.step, e.doc.audio.len()),
        (StepKind::ReverseClips { count: 1 }, 2)
    );
}

#[test]
fn a_clip_whose_audio_changes_tempo_along_it_stops_following() {
    // Recorded in a song that goes from 120 to 90 at bar 3: the take keeps that map.
    let base = at_tempo(&Doc::new_default(), 12_000);
    let changing = crate::timeline::add_tempo_point(&base, 2 * BAR, tempo(9_000))
        .unwrap()
        .doc;
    let (doc, clip) = recorded(&changing);
    let sync = |doc: &Doc| clip_of(doc, clip).active_audio().unwrap().take.sync.clone();
    assert!(sync(&doc).is_some_and(|s| !s.points.is_empty()));
    let played = played_frames(&doc, clip).unwrap();
    let ClipsReversed { edit, follow_off } =
        reverse_clips(&doc, &[reversed(&played, 1, "歌 1.wav")]).unwrap();
    assert_eq!(follow_off, 1);
    assert_eq!(sync(&edit.doc), None);
    assert!(!clip_of(&edit.doc, clip).takes.shape().unwrap().follow);

    // Not following already: the same, and nothing to tell.
    let free = set_clip_follow(&doc, clip, false).unwrap().doc;
    let played = played_frames(&free, clip).unwrap();
    let done = reverse_clips(&free, &[reversed(&played, 1, "歌 1.wav")]).unwrap();
    assert_eq!((done.follow_off, sync(&done.edit.doc)), (0, None));

    // The change is past the clip's end: one tempo all along it, which it keeps.
    let later = crate::timeline::add_tempo_point(&base, 8 * BAR, tempo(9_000))
        .unwrap()
        .doc;
    let (doc, clip) = recorded(&later);
    let played = played_frames(&doc, clip).unwrap();
    let done = reverse_clips(&doc, &[reversed(&played, 1, "歌 1.wav")]).unwrap();
    assert_eq!(done.follow_off, 0);
    let after = clip_of(&done.edit.doc, clip);
    assert_eq!(
        after.active_audio().unwrap().take.sync,
        Some(SourceTempo::steady(tempo(12_000)))
    );
    assert!(after.takes.shape().unwrap().follow);

    // A ramp under the clip is a tempo that changes.
    let mut ramped = (*base.timeline).clone();
    ramped.tempo_points = vec![
        TempoPoint {
            tick: BAR,
            tempo: tempo(12_000),
            ramp: true,
        },
        TempoPoint {
            tick: 6 * BAR,
            tempo: tempo(9_000),
            ramp: false,
        },
    ];
    let mut ramp = base.clone();
    ramp.timeline = Arc::new(ramped);
    let (doc, clip) = recorded(&ramp);
    let played = played_frames(&doc, clip).unwrap();
    assert_eq!(
        reverse_clips(&doc, &[reversed(&played, 1, "歌 1.wav")])
            .unwrap()
            .follow_off,
        1
    );

    // A file brought in has no tempo: the switch stays as it is.
    let (doc, clip) = imported();
    let on = set_clip_follow(&doc, clip, true).unwrap().doc;
    let played = played_frames(&on, clip).unwrap();
    let done = reverse_clips(&on, &[reversed(&played, 1, "ドラム 1.wav")]).unwrap();
    assert_eq!(done.follow_off, 0);
    let after = clip_of(&done.edit.doc, clip);
    assert!(
        after.takes.shape().unwrap().follow && after.active_audio().unwrap().take.sync.is_none()
    );
}
