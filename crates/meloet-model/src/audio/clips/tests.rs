use std::sync::Arc;

use super::*;
use crate::arrange::{ClipEdge, set_clip_length, set_clip_start, split_clips, trim_clip};
use crate::audio::{SourceTempo, TrackKind, VocalInput, vocal_track_name};
use crate::history::History;
use crate::song::Song;
use crate::{TICKS_PER_QUARTER, Tempo};

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
const VOCAL: TrackId = TrackId(2);
const CLIP: ClipId = ClipId(10);

/// The default song at 120 a minute with a vocal track (id 2) that has one clip (id 10)
/// over bar 2, of two takes of the file 11.
fn doc() -> Doc {
    let mut song = Song::new_default();
    let mut timeline = (*song.doc.timeline).clone();
    timeline.tempo = Tempo::from_centi(12_000).unwrap();
    song.doc.timeline = Arc::new(timeline);
    let source = Arc::new(AudioSource {
        id: AudioId(11),
        uid: [7; 16],
        file: "歌 11.wav".to_owned(),
        frames: 480_000,
        sample_rate: 48_000,
        channels: 1,
        bits: Some(24),
        sha256: None,
    });
    let take = |id, first_frame| ClipTake {
        take: Arc::new(AudioTake {
            id: TakeId(id),
            source: source.clone(),
            first_frame,
            frames: 96_000,
            sync: Some(SourceTempo {
                tempo: song.doc.timeline.tempo,
                head_ramp: false,
                points: Vec::new(),
            }),
        }),
        offset: 0,
    };
    let clip = Clip {
        id: CLIP,
        name: "歌".to_owned(),
        start: BAR,
        end: 2 * BAR,
        takes: ClipTakes::Vocal {
            takes: vec![take(12, 0), take(13, 96_000)],
            shape: AudioShape {
                fade_in: Q,
                fade_out: 2 * Q,
                ..AudioShape::RECORDED
            },
        },
        active: 1,
        comp: Vec::new(),
    };
    let keys = Track::clone(&song.doc.tracks[0]);
    song.doc.tracks.push(Arc::new(Track {
        id: VOCAL,
        name: vocal_track_name(crate::Language::Ja).to_owned(),
        kind: TrackKind::Vocal {
            input: VocalInput::Mono1,
        },
        clips: vec![Arc::new(clip)],
        ..keys
    }));
    song.doc.audio = Arc::new(vec![source]);
    song.doc.next_id = 20;
    song.doc
}

fn shape(doc: &Doc, clip: ClipId) -> AudioShape {
    *doc.clip(clip).unwrap().1.takes.shape().unwrap()
}

fn file(name: &str, frames: u64, rate: u32) -> ImportedAudio {
    ImportedAudio {
        audio: NewAudio {
            uid: [name.len() as u8; 16],
            file: name.to_owned(),
            frames,
            sample_rate: rate,
            channels: 2,
            bits: None,
        },
        sha256: [3; 32],
        name: name.trim_end_matches(".mp3").to_owned(),
    }
}

#[test]
fn a_vocal_clip_cut_in_two_plays_the_same_takes_with_the_fades_outside() {
    let doc = doc();
    let cut = split_clips(&doc, &[CLIP], BAR + Q).unwrap();
    assert_eq!(cut.edit.step, StepKind::SplitClips { count: 1 });
    assert_eq!(cut.left, [CLIP]);
    let (_, old) = doc.clip(CLIP).unwrap();
    let (_, left) = cut.edit.doc.clip(CLIP).unwrap();
    let (_, right) = cut.edit.doc.clip(cut.right[0]).unwrap();
    assert_eq!((left.start, left.end), (BAR, BAR + Q));
    assert_eq!((right.start, right.end), (BAR + Q, 2 * BAR));
    assert_eq!((left.active, right.active), (1, 1));
    for (i, ct) in old.takes.vocal().iter().enumerate() {
        assert!(Arc::ptr_eq(&ct.take, &left.takes.vocal()[i].take));
        assert!(Arc::ptr_eq(&ct.take, &right.takes.vocal()[i].take));
        // Each take's tick 0 stays where it was in the song.
        assert_eq!(left.zero(&left.takes.vocal()[i]), old.zero(ct));
        assert_eq!(right.zero(&right.takes.vocal()[i]), old.zero(ct));
    }
    // The fade-in fits the left half (a quarter note long); the fade-out stays whole.
    assert_eq!(
        (
            shape(&cut.edit.doc, CLIP).fade_in,
            shape(&cut.edit.doc, CLIP).fade_out
        ),
        (Q, 0)
    );
    let r = shape(&cut.edit.doc, cut.right[0]);
    assert_eq!((r.fade_in, r.fade_out), (0, 2 * Q));
    // No take was made, the table is as it was, and one undo puts the clip back.
    assert!(Arc::ptr_eq(&doc.audio, &cut.edit.doc.audio));
    let mut now = doc.clone();
    let mut history = History::new();
    history.apply(&mut now, cut.edit);
    assert_eq!(now.track(VOCAL).unwrap().clips.len(), 2);
    history.undo(&mut now);
    // Ids are never given twice: the count stays where the step left it.
    assert!(now.next_id > doc.next_id);
    now.next_id = doc.next_id;
    assert!(now == doc);
}

#[test]
fn a_cut_on_an_edge_or_outside_cuts_nothing() {
    let doc = doc();
    for at in [0, BAR, 2 * BAR, 3 * BAR] {
        assert_eq!(
            split_clips(&doc, &[CLIP], at).unwrap_err(),
            EditError::Unchanged
        );
    }
    assert_eq!(
        split_clips(&doc, &[ClipId(99)], BAR + 1).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn a_shorter_clip_has_shorter_fades() {
    let doc = doc();
    // 3 quarter notes of fades in a clip pulled in to 2: the fade-out gives way.
    let e = trim_clip(&doc, CLIP, ClipEdge::End, -(2 * Q as i64)).unwrap();
    let s = shape(&e.doc, CLIP);
    assert_eq!((s.fade_in, s.fade_out), (Q, Q));
    let e = set_clip_length(&doc, CLIP, Q / 2).unwrap();
    let s = shape(&e.doc, CLIP);
    assert_eq!((s.fade_in, s.fade_out), (Q / 2, 0));
    assert_eq!(e.doc.clip(CLIP).unwrap().1.end, BAR + Q / 2);
    assert_eq!(
        set_clip_length(&doc, CLIP, 0)
            .unwrap()
            .doc
            .clip(CLIP)
            .unwrap()
            .1
            .end,
        BAR + 1
    );
    assert_eq!(
        set_clip_length(&doc, CLIP, BAR).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn a_typed_start_moves_the_clip_and_stops_at_another() {
    let doc = doc();
    let cut = split_clips(&doc, &[CLIP], BAR + Q).unwrap();
    let (doc, right) = (cut.edit.doc, cut.right[0]);
    let moved = set_clip_start(&doc, right, 4 * BAR).unwrap();
    let (_, c) = moved.edit.doc.clip(right).unwrap();
    assert_eq!((c.start, c.end), (4 * BAR, 4 * BAR + 3 * Q));
    assert_eq!(
        set_clip_start(&doc, right, BAR).unwrap_err(),
        EditError::Overlap
    );
    assert_eq!(
        set_clip_length(&doc, CLIP, 2 * Q).unwrap_err(),
        EditError::Overlap
    );
    assert_eq!(
        set_clip_start(&doc, right, BAR + Q).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn the_volume_and_the_fades_are_steps_of_their_own() {
    let doc = doc();
    let e = set_clip_gain(&doc, CLIP, ClipGain::from_db(-3.0).unwrap()).unwrap();
    assert_eq!(e.step, StepKind::SetClipGain { clip: CLIP });
    assert_eq!(shape(&e.doc, CLIP).gain.db(), -3.0);
    assert_eq!(
        set_clip_gain(&doc, CLIP, ClipGain::UNITY).unwrap_err(),
        EditError::Unchanged
    );
    // A fade is no longer than the clip less the other fade.
    let e = set_clip_fade(&doc, CLIP, FadeEnd::In, 10 * BAR).unwrap();
    assert_eq!(e.step, StepKind::SetClipFade { clip: CLIP });
    assert_eq!(shape(&e.doc, CLIP).fade_in, 2 * Q);
    let e = set_clip_fade(&doc, CLIP, FadeEnd::Out, 0).unwrap();
    assert_eq!(shape(&e.doc, CLIP).fade_out, 0);
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
    assert_eq!(
        set_clip_gain(&keys.0.doc, keys.1.clip, ClipGain::UNITY).unwrap_err(),
        EditError::WrongKind
    );
}

#[test]
fn several_clips_change_as_one_step_and_the_others_are_passed_by() {
    let doc = doc();
    // A second clip on the vocal track, at +10 dB, and a keys clip.
    let second = ClipId(30);
    let mut d = doc.clone();
    let vocal = Track::clone(&d.tracks[1]);
    let mut c = Clip::clone(&vocal.clips[0]);
    c.id = second;
    c.start = 3 * BAR;
    c.end = 4 * BAR;
    if let ClipTakes::Vocal { shape, .. } = &mut c.takes {
        shape.gain = ClipGain::from_db(10.0).unwrap();
    }
    let mut clips = vocal.clips.clone();
    clips.push(Arc::new(c));
    d.tracks[1] = Arc::new(Track { clips, ..vocal });
    let (keys, made) = crate::edit::add_recording(
        &d,
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
    let d = keys.doc;
    let all = [CLIP, second, made.clip, ClipId(999)];
    let up = |s: &AudioShape| AudioShape {
        gain: ClipGain::from_db(s.gain.db() + 3.0).unwrap(),
        ..*s
    };
    let e = set_clips_shape(&d, &all, up, |count| StepKind::SetClipsGain { count }).unwrap();
    assert_eq!(e.step, StepKind::SetClipsGain { count: 2 });
    assert_eq!(shape(&e.doc, CLIP).gain.db(), 3.0);
    // Stopped at the end of the range.
    assert_eq!(shape(&e.doc, second).gain.db(), 12.0);
    assert_eq!(e.doc.clip(made.clip), d.clip(made.clip));
    // One that is at the end already is left out of the count.
    let e = set_clips_shape(&e.doc, &all, up, |count| StepKind::SetClipsGain { count }).unwrap();
    assert_eq!(e.step, StepKind::SetClipsGain { count: 1 });
    assert_eq!(shape(&e.doc, CLIP).gain.db(), 6.0);
    let same = |s: &AudioShape| *s;
    assert_eq!(
        set_clips_shape(&d, &all, same, |count| StepKind::SetClipsGain { count }).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        set_clips_shape(&d, &[made.clip], up, |count| StepKind::SetClipsGain {
            count
        })
        .unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn files_in_a_row_go_one_after_the_other_and_around_what_is_there() {
    let doc = doc();
    // At 120 a minute a second is half a bar. From the song's head: the first ends where
    // the clip of bar 2 starts, the second would overlap it, the third comes after it.
    let files = vec![
        file("a.mp3", 2 * 44_100, 44_100),
        file("b.mp3", 96_000, 48_000),
        file("c.mp3", 48_000, 48_000),
    ];
    let (e, made) = import_audio(
        &doc,
        files,
        ImportPlan::InRow(Some(VOCAL)),
        0,
        crate::Language::Ja,
    )
    .unwrap();
    assert_eq!(e.step, StepKind::ImportAudio { count: 3 });
    assert_eq!(e.doc.tracks.len(), 3);
    let spare = e.doc.tracks[2].id;
    assert!(e.doc.tracks[2].kind.is_vocal());
    assert_eq!(e.doc.tracks[2].name, "歌 2");
    assert_eq!(
        made.iter().map(|m| (m.track, m.start)).collect::<Vec<_>>(),
        [(VOCAL, 0), (spare, BAR), (VOCAL, 2 * BAR)]
    );
    let (_, a) = e.doc.clip(made[0].clip).unwrap();
    assert_eq!((a.start, a.end, a.name.as_str()), (0, BAR, "a"));
    let take = &a.active_audio().unwrap().take;
    assert_eq!(
        (take.first_frame, take.frames, take.sync.clone()),
        (0, 88_200, None)
    );
    assert_eq!(take.source.sha256, Some([3; 32]));
    assert!(!shape(&e.doc, made[0].clip).follow);
    assert_eq!(e.doc.audio.len(), 4);
    // No clip of a track overlaps another, and every id is given once.
    for t in &e.doc.tracks {
        assert!(t.clips.windows(2).all(|w| w[0].end <= w[1].start));
    }
    let mut ids: Vec<u32> = (e.doc.audio.iter().map(|a| a.id.0))
        .chain(e.doc.tracks.iter().map(|t| t.id.0))
        .chain(
            e.doc
                .tracks
                .iter()
                .flat_map(|t| t.clips.iter().map(|c| c.id.0)),
        )
        .collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count);
    assert!(ids.iter().all(|&id| id < e.doc.next_id));
}

#[test]
fn files_apart_get_a_track_each_from_the_same_tick() {
    let doc = doc();
    let files = vec![file("a.mp3", 48_000, 48_000), file("b.mp3", 48_000, 48_000)];
    let (e, made) = import_audio(
        &doc,
        files,
        ImportPlan::Separate,
        3 * BAR,
        crate::Language::Ja,
    )
    .unwrap();
    assert_eq!(e.doc.tracks.len(), 4);
    assert!(made.iter().all(|m| m.start == 3 * BAR));
    assert_ne!(made[0].track, made[1].track);
    assert!(made.iter().all(|m| m.track != VOCAL));
    // One file with no track named: a new track; on a keys track: nothing.
    let one = vec![file("a.mp3", 48_000, 48_000)];
    let (e, made) = import_audio(
        &doc,
        one.clone(),
        ImportPlan::InRow(None),
        0,
        crate::Language::Ja,
    )
    .unwrap();
    assert_eq!((e.doc.tracks.len(), made[0].track), (3, e.doc.tracks[2].id));
    assert_eq!(
        import_audio(
            &doc,
            one,
            ImportPlan::InRow(Some(TrackId(1))),
            0,
            crate::Language::Ja
        )
        .unwrap_err(),
        EditError::WrongKind
    );
    assert_eq!(
        import_audio(
            &doc,
            Vec::new(),
            ImportPlan::Separate,
            0,
            crate::Language::Ja
        )
        .unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn a_file_taken_out_takes_its_clips_and_its_takes() {
    let doc = doc();
    let (e, made) = import_audio(
        &doc,
        vec![file("a.mp3", 48_000, 48_000)],
        ImportPlan::InRow(Some(VOCAL)),
        0,
        crate::Language::Ja,
    )
    .unwrap();
    let with = e.doc;
    let e = remove_audio(&with, made[0].audio).unwrap();
    assert_eq!(
        e.step,
        StepKind::RemoveAudio {
            audio: made[0].audio,
            clips: 1
        }
    );
    assert!(e.doc.clip(made[0].clip).is_none());
    assert!(e.doc.clip(CLIP).is_some());
    assert_eq!(e.doc.audio.len(), 1);
    // The recorded file: both takes of the clip play it, so the clip goes.
    let e = remove_audio(&with, AudioId(11)).unwrap();
    assert!(e.doc.clip(CLIP).is_none());
    assert_eq!(
        remove_audio(&e.doc, AudioId(11)).unwrap_err(),
        EditError::Unchanged
    );
}
