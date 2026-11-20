use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::arrange::{ClipEdge, move_clips, trim_clip};
use crate::edit::{EditError, NewTake, add_recording, delete_take, switch_take};
use crate::notes::{WriteAt, clipboard, write_note};
use crate::playback::track_events;
use crate::song::{Clip, ClipId, ClipTake, ClipTakes, Note, Song, Track};
use crate::{TICKS_PER_QUARTER, TrackId};

const BAR: u64 = 4 * TICKS_PER_QUARTER;

/// The default song with a vocal track (id 2) that has one clip (id 10) of two takes.
fn song() -> Song {
    let mut song = Song::new_default();
    let source = Arc::new(AudioSource {
        id: AudioId(11),
        uid: [7; 16],
        file: "歌 テイク 1.wav".to_owned(),
        frames: 480_000,
        sample_rate: 48_000,
        channels: 1,
        bits: Some(24),
        sha256: None,
    });
    let tempo = song.doc.timeline.tempo;
    let take = |id, first_frame| ClipTake {
        take: Arc::new(AudioTake {
            id: TakeId(id),
            source: source.clone(),
            first_frame,
            frames: 120_000,
            sync: Some(SourceTempo {
                tempo,
                head_ramp: false,
                points: Vec::new(),
            }),
        }),
        offset: 0,
    };
    let clip = Clip {
        id: ClipId(10),
        name: "歌 テイク 1".to_owned(),
        start: BAR,
        end: 2 * BAR,
        takes: ClipTakes::Vocal {
            takes: vec![take(12, 0), take(13, 120_000)],
            shape: AudioShape::RECORDED,
        },
        active: 1,
    };
    let keys = Track::clone(&song.doc.tracks[0]);
    song.doc.tracks.push(Arc::new(Track {
        id: TrackId(2),
        name: VOCAL_TRACK_NAME.to_owned(),
        kind: TrackKind::Vocal {
            input: VocalInput::Mono1,
        },
        clips: vec![Arc::new(clip)],
        ..keys
    }));
    song.doc.audio = Arc::new(vec![source]);
    song.doc.next_id = 20;
    song
}

#[test]
fn a_vocal_clip_takes_no_notes() {
    let song = song();
    let doc = &song.doc;
    let note = Note {
        start: BAR,
        length: 240,
        key: 60,
        velocity: 100,
        from: None,
    };
    assert_eq!(
        write_note(doc, WriteAt::Clip(ClipId(10)), note).unwrap_err(),
        EditError::NotKeys
    );
    // An empty place of a vocal track makes no clip of notes either.
    let later = Note {
        start: 5 * BAR,
        ..note
    };
    assert_eq!(
        write_note(doc, WriteAt::Track(TrackId(2)), later).unwrap_err(),
        EditError::NotKeys
    );
    let (_, clip) = doc.clip(ClipId(10)).unwrap();
    assert_eq!(clipboard(clip, &[0]).unwrap_err(), EditError::NotKeys);
    let take = NewTake {
        origin: 0,
        length: BAR,
        notes: vec![Note { start: 0, ..note }],
        controls: Vec::new(),
    };
    assert_eq!(
        add_recording(doc, TrackId(2), vec![take], 0).unwrap_err(),
        EditError::NotKeys
    );
    assert_eq!(clip.note_count(), 0);
    assert!(track_events(doc, TrackId(2)).is_empty());
}

#[test]
fn a_vocal_clip_moves_trims_and_switches_as_a_keys_clip() {
    let song = song();
    let doc = &song.doc;
    let trimmed = trim_clip(doc, ClipId(10), ClipEdge::Start, 480)
        .unwrap()
        .doc;
    let (_, clip) = trimmed.clip(ClipId(10)).unwrap();
    assert_eq!((clip.start, clip.active_zero()), (BAR + 480, BAR as i64));
    assert_eq!(clip.takes.shape(), Some(&AudioShape::RECORDED));
    assert_eq!(clip.active_take_id(), TakeId(13));
    assert_eq!(clip.active_audio().unwrap().take.first_frame, 120_000);

    let switched = switch_take(doc, ClipId(10), 0).unwrap().doc;
    assert_eq!(
        switched.clip(ClipId(10)).unwrap().1.active_take_id(),
        TakeId(12)
    );
    let deleted = delete_take(doc, ClipId(10)).unwrap().doc;
    let (_, clip) = deleted.clip(ClipId(10)).unwrap();
    assert_eq!((clip.takes.len(), clip.active_take_id()), (1, TakeId(12)));

    let moved = move_clips(doc, &[ClipId(10)], BAR as i64, 0, false).unwrap();
    let (_, clip) = moved.edit.doc.clip(ClipId(10)).unwrap();
    assert_eq!((clip.start, clip.end), (2 * BAR, 3 * BAR));
    // Not onto the keys track above it.
    assert_eq!(
        move_clips(doc, &[ClipId(10)], 0, -1, false).unwrap_err(),
        EditError::WrongKind
    );
}

#[test]
fn the_keys_stay_on_the_keys_track_selected_last() {
    let mut song = song();
    assert_eq!(song.keys_target(), Some(TrackId(1)));
    song.session.play_target = Some(TrackId(2));
    assert_eq!(song.keys_target(), None);
    song.session.last_keys = Some(TrackId(1));
    assert_eq!(song.keys_target(), Some(TrackId(1)));
    // A keys track that is gone, or a vocal one, is no target.
    song.session.last_keys = Some(TrackId(9));
    assert_eq!(song.keys_target(), None);
    song.session.last_keys = Some(TrackId(2));
    assert_eq!(song.keys_target(), None);
    song.session.armed_vocals = BTreeSet::from([TrackId(2)]);
    assert_eq!(song.keys_target(), None);
}

#[test]
fn a_vocal_track_is_added_named_and_given_another_input() {
    use crate::edit::{add_vocal_track, set_vocal_input};
    use crate::history::StepKind;
    let doc = Song::new_default().doc;
    let (added, id) = add_vocal_track(&doc, None).unwrap();
    assert_eq!(added.step, StepKind::AddTrack { track: id });
    let track = added.doc.track(id).unwrap();
    assert_eq!(track.name, "歌");
    assert_eq!(track.kind.input(), Some(VocalInput::Mono1));
    assert_eq!(track.color.get(), 2);
    assert_eq!(track.sends, [crate::Gain::SILENT; 2]);
    assert!(track.clips.is_empty());
    let (second, id2) = add_vocal_track(&added.doc, Some(TrackId(1))).unwrap();
    assert_eq!(second.doc.tracks[1].id, id2);
    assert_eq!(second.doc.tracks[1].name, "歌 2");

    let doc = added.doc;
    let stereo = set_vocal_input(&doc, id, VocalInput::Stereo12).unwrap();
    assert_eq!(stereo.step, StepKind::SetVocalInput { track: id });
    assert_eq!(
        stereo.doc.track(id).unwrap().kind.input(),
        Some(VocalInput::Stereo12)
    );
    assert_eq!(
        set_vocal_input(&doc, id, VocalInput::Mono1).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        set_vocal_input(&doc, TrackId(1), VocalInput::Mono2).unwrap_err(),
        EditError::NoSuchTrack(TrackId(1))
    );
}
