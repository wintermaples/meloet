use std::sync::Arc;

use super::*;
use crate::automation as auto;
use crate::edit::{add_recording, add_track, add_vocal_track, rename_track};
use crate::effects::EffectKind;
use crate::history::History;
use crate::id_book::{IdBook, held};
use crate::mixer::{self, Bus, Gain, Pan, StripId};
use crate::song::Note;
use crate::time::Tempo;
use crate::{BuiltinInstrument, Language, NewTake, TICKS_PER_QUARTER, timeline};

const T1: TrackId = TrackId(1);
const Q: u64 = TICKS_PER_QUARTER;

fn audio(frames: u64) -> NewAudio {
    NewAudio {
        uid: [7; 16],
        file: "ピアノ（音） 12.wav".to_owned(),
        frames,
        sample_rate: 48_000,
        channels: 2,
        bits: Some(24),
    }
}

/// Piano track 1 with a clip, its fader, pan and sends moved, a reverb in a slot, lanes for
/// the fader, a send and the reverb, and a bass track below it.
fn song() -> Doc {
    let strip = StripId::Track(T1);
    let mut doc = Doc::new_default();
    let take = NewTake {
        origin: 4 * Q,
        length: 4 * Q,
        notes: vec![Note {
            start: 0,
            length: Q,
            key: 60,
            velocity: 100,
            from: None,
        }],
        controls: Vec::new(),
    };
    doc = add_recording(&doc, T1, vec![take], 0).unwrap().0.doc;
    doc = mixer::set_gain(&doc, strip, Gain::from_db(-6.0))
        .unwrap()
        .doc;
    doc = mixer::set_pan(&doc, strip, Pan::new(-30)).unwrap().doc;
    doc = mixer::set_send(&doc, T1, Bus::B, Gain::from_db(-10.0))
        .unwrap()
        .doc;
    doc = mixer::add_effect(&doc, strip, 0, EffectKind::Reverb)
        .unwrap()
        .doc;
    let reverb = doc.insert(strip, 0).unwrap().id;
    for target in [
        AutoTarget::Volume,
        AutoTarget::Send(Bus::B),
        AutoTarget::Insert {
            insert: reverb,
            param: 0,
        },
    ] {
        let (e, lane) = auto::add_lane(&doc, T1, target, false).unwrap();
        let (e, _, _) = auto::add_point(&e.doc, lane, 0, Some(0.25), 0.5).unwrap();
        doc = e.doc;
    }
    let bass = InstrumentRef::Builtin(BuiltinInstrument::SynthLead);
    doc = add_track(&doc, None, bass, crate::Language::Ja)
        .unwrap()
        .0
        .doc;
    doc
}

#[test]
fn the_audio_goes_on_a_new_vocal_track_right_below_with_the_strip_and_the_track_is_muted() {
    let doc = song();
    let b = bounce_track(&doc, T1, audio(48_000 * 3), 4 * Q, Language::Ja).unwrap();
    let new = &b.edit.doc;
    assert_eq!(b.edit.step, StepKind::Bounce { track: b.track });
    let order: Vec<TrackId> = new.tracks.iter().map(|t| t.id).collect();
    assert_eq!(order, [T1, b.track, doc.tracks[1].id]);
    assert!(Arc::ptr_eq(&doc.tracks[1], &new.tracks[2]));

    let (from, to) = (&new.tracks[0], &new.tracks[1]);
    assert!(from.muted, "the track is muted");
    assert_eq!(
        Track {
            muted: false,
            ..Track::clone(from)
        },
        *doc.tracks[0],
        "and keeps everything else"
    );
    assert_eq!(to.name, "ピアノ（音）");
    assert!(to.kind.is_vocal());
    assert!(!to.muted);
    assert_eq!(to.color, TrackColor::for_index(1));
    assert_eq!(
        (to.channel.gain, to.channel.pan, to.sends),
        (from.channel.gain, from.channel.pan, from.sends)
    );
    assert!(to.channel.inserts.iter().all(Option::is_none), "no inserts");
    // The lanes of the fader and the send, under new ids; not the reverb's.
    let targets: Vec<AutoTarget> = to.automation.iter().map(|l| l.target).collect();
    assert_eq!(targets, [AutoTarget::Volume, AutoTarget::Send(Bus::B)]);
    for (lane, was) in to.automation.iter().zip(&from.automation) {
        assert_ne!(lane.id, was.id);
        assert_eq!(lane.points, was.points);
    }

    // One clip from the start, as long as three seconds at 96 a minute, not following.
    let [clip] = &to.clips[..] else {
        panic!("{:?}", to.clips)
    };
    assert_eq!(clip.id, b.clip);
    assert_eq!(clip.name, "ピアノ（音）");
    assert_eq!((clip.start, clip.end), (4 * Q, 4 * Q + 48 * Q / 10));
    let shape = clip.takes.shape().unwrap();
    assert!(!shape.follow);
    assert_eq!((shape.fade_in, shape.fade_out), (0, 0));
    let take = &clip.takes.vocal()[0];
    assert_eq!((take.offset, take.take.first_frame), (0, 0));
    assert_eq!(take.take.frames, 48_000 * 3);
    let source = &take.take.source;
    assert_eq!(new.audio.last(), Some(source));
    assert_eq!(new.audio.len(), doc.audio.len() + 1);
    assert_eq!(
        (&source.file[..], source.uid, source.sha256, source.bits),
        ("ピアノ（音） 12.wav", [7; 16], None, Some(24))
    );
    assert_eq!(source.channels, 2);

    let mut book = IdBook::new(&doc);
    book.saw(new, Some(&doc)).unwrap();
    new.check_lanes().unwrap();
    assert!(source.id.0 < new.next_id && b.track.0 < new.next_id);
}

#[test]
fn undo_takes_the_whole_step_back_and_redo_brings_it() {
    let doc = song();
    let mut history = History::default();
    let mut now = doc.clone();
    let b = bounce_track(&now, T1, audio(48_000), 0, Language::Ja).unwrap();
    history.apply(&mut now, b.edit.clone());
    assert_eq!(now, b.edit.doc);
    assert!(history.undo(&mut now).is_some());
    assert_eq!(held(&now), held(&doc));
    assert!(history.redo(&mut now).is_some());
    assert_eq!(held(&now), held(&b.edit.doc));
}

#[test]
fn a_mute_lane_that_moves_rests_in_the_same_step_so_the_mute_is_heard() {
    let (e, lane) = auto::add_lane(&song(), T1, AutoTarget::Mute, true).unwrap();
    let (e, _, _) = auto::add_point(&e.doc, lane, 0, Some(0.0), 0.0).unwrap();
    let doc = e.doc;
    let mut history = History::default();
    let mut now = doc.clone();
    let b = bounce_track(&now, T1, audio(48_000), 0, Language::Ja).unwrap();
    history.apply(&mut now, b.edit);
    let from = now.track(T1).unwrap();
    assert!(from.muted);
    let rested = from.lane(AutoTarget::Mute).unwrap();
    assert!(rested.resting, "the hand's mute is what is heard");
    assert_eq!(rested.points, doc.track(T1).unwrap().automation[3].points);
    assert!(history.undo(&mut now).is_some());
    assert_eq!(held(&now), held(&doc));
    assert!(
        now.track(T1)
            .unwrap()
            .lane(AutoTarget::Mute)
            .unwrap()
            .moves()
    );
}

#[test]
fn a_name_that_is_taken_gets_a_number_and_the_clip_follows_the_songs_tempo_map() {
    let mut doc = song();
    let (e, vocal) = add_vocal_track(&doc, None, crate::Language::Ja).unwrap();
    doc = rename_track(&e.doc, vocal, "ピアノ（音）").unwrap().doc;
    // From the second bar the song is twice as fast.
    let fast = Tempo::from_centi(19_200).unwrap();
    doc = timeline::add_tempo_point(&doc, 4 * Q, fast).unwrap().doc;
    let b = bounce_track(&doc, T1, audio(48_000 * 3), 0, Language::Ja).unwrap();
    let to = b.edit.doc.track(b.track).unwrap();
    assert_eq!(to.name, "ピアノ（音） 2");
    let clip = &to.clips[0];
    // 2.5 s at 96 (4 quarters), then 0.5 s at 192 (1.6 quarters).
    assert_eq!((clip.start, clip.end), (0, 4 * Q + 16 * Q / 10));
}

#[test]
fn in_english_the_name_ends_in_audio_with_the_same_numbers() {
    let mut doc = song();
    let b = bounce_track(&doc, T1, audio(48_000), 0, Language::En).unwrap();
    let to = b.edit.doc.track(b.track).unwrap();
    assert_eq!(to.name, "ピアノ (audio)");
    assert_eq!(to.clips[0].name, "ピアノ (audio)");
    let (e, vocal) = add_vocal_track(&doc, None, Language::En).unwrap();
    doc = rename_track(&e.doc, vocal, "ピアノ (audio)").unwrap().doc;
    let b = bounce_track(&doc, T1, audio(48_000), 0, Language::En).unwrap();
    assert_eq!(b.edit.doc.track(b.track).unwrap().name, "ピアノ (audio) 2");
}

#[test]
fn a_vocal_track_bad_audio_or_no_track_change_nothing() {
    let doc = song();
    let (e, vocal) = add_vocal_track(&doc, None, crate::Language::Ja).unwrap();
    assert_eq!(
        bounce_track(&e.doc, vocal, audio(10), 0, Language::Ja),
        Err(EditError::WrongKind)
    );
    assert_eq!(
        bounce_track(&doc, TrackId(99), audio(10), 0, Language::Ja),
        Err(EditError::NoSuchTrack(TrackId(99)))
    );
    for bad in [
        audio(0),
        NewAudio {
            channels: 3,
            ..audio(10)
        },
        NewAudio {
            file: String::new(),
            ..audio(10)
        },
    ] {
        assert_eq!(
            bounce_track(&doc, T1, bad, 0, Language::Ja),
            Err(EditError::BadTake(0))
        );
    }
    assert_eq!(
        bounce_track(&doc, T1, audio(10), MAX_TICK, Language::Ja),
        Err(EditError::OffTheSong)
    );
}
