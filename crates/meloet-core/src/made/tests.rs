use meloet_model::song::{ClipId, Doc};
use meloet_model::{
    NewAudio, NewAudioTake, Tempo, TrackId, VocalRecording, add_vocal_recordings, arrange, edit,
    set_clip_cents, set_clip_follow, set_clip_keep_formant, set_clip_semitones, set_source_tempo,
    timeline,
};

use super::*;
use crate::pcm::feed_clip;

const BAR: u64 = 7_680;
const RATE: u32 = 48_000;

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

/// 96, a ramp from bar 3 to 120 at bar 5, then 90 from bar 7; and a vocal track.
fn ramped() -> (Doc, TrackId) {
    let doc = Doc::new_default();
    let doc = timeline::set_tempo(&doc, tempo(9_600)).map_or(doc, |e| e.doc);
    let doc = timeline::add_tempo_point(&doc, 2 * BAR, tempo(9_600))
        .unwrap()
        .doc;
    let doc = timeline::add_tempo_point(&doc, 4 * BAR, tempo(12_000))
        .unwrap()
        .doc;
    let doc = timeline::add_tempo_point(&doc, 6 * BAR, tempo(9_000))
        .unwrap()
        .doc;
    let doc = timeline::set_tempo_ramp(&doc, 2 * BAR, true).unwrap().doc;
    let (e, track) = edit::add_vocal_track(&doc, None).unwrap();
    (e.doc, track)
}

/// A recording of `length` ticks from `origin`, as the song's map is now.
fn record(doc: &Doc, track: TrackId, origin: u64, length: u64) -> (Doc, ClipId) {
    let recording = VocalRecording {
        track,
        audio: NewAudio {
            uid: [origin as u8 | 1; 16],
            file: format!("歌 {origin}.wav"),
            frames: 4_000_000,
            sample_rate: RATE,
            channels: 1,
            bits: Some(24),
        },
        takes: vec![NewAudioTake {
            origin,
            length,
            first_frame: 1_000,
            frames: 3_000_000,
        }],
    };
    let (e, added) = add_vocal_recordings(doc, vec![recording], None).unwrap();
    (e.doc, added[0].clip)
}

fn plan_of(doc: &Doc, clip: ClipId, rate: u32) -> Option<MadePlan> {
    let map = Arc::new(TempoMap::new(&doc.timeline));
    let (_, clip) = doc.clip(clip).unwrap();
    let feed = feed_clip(&map, rate, clip).unwrap();
    plan(&doc.timeline, &map, rate, clip, &feed)
}

#[test]
fn recorded_audio_plays_as_it_is_while_the_map_under_it_stays() {
    let (doc, track) = ramped();
    // In the steady part, from the middle of the ramp (where the head's tempo is rounded),
    // over the ramp's end, and over everything.
    for (origin, length) in [
        (0, BAR),
        (3 * BAR + 777, BAR / 2),
        (3 * BAR + 1, 2 * BAR),
        (BAR / 3, 8 * BAR),
    ] {
        let (doc, clip) = record(&doc, track, origin, length);
        assert!(plan_of(&doc, clip, RATE).is_none(), "{origin}");
        assert!(plan_of(&doc, clip, 44_100).is_none(), "{origin}");
        // Cut in two: both halves show the same take.
        let cut = arrange::split_clips(&doc, &[clip], origin + length / 2).unwrap();
        for half in [cut.left[0], cut.right[0]] {
            assert!(plan_of(&cut.edit.doc, half, RATE).is_none(), "{origin}");
        }
    }

    let (doc, clip) = record(&doc, track, 3 * BAR + 777, BAR / 2);
    // Another tempo past the clip's end is nothing to it.
    let later = timeline::set_tempo_at(&doc, 6 * BAR, tempo(6_000))
        .unwrap()
        .doc;
    assert!(plan_of(&later, clip, RATE).is_none());
    // The ramp it was sung in ends somewhere else: the clip is stretched.
    let bent = timeline::set_tempo_at(&doc, 4 * BAR, tempo(13_000))
        .unwrap()
        .doc;
    let made = plan_of(&bent, clip, RATE).unwrap();
    assert!(matches!(made.place, Place::Follow { .. }));
    assert_eq!((made.semitones, made.keep_formant), (0.0, false));
    // Not following, it plays at the file's speed whatever the map is.
    let free = set_clip_follow(&bent, clip, false).unwrap().doc;
    assert!(plan_of(&free, clip, RATE).is_none());
    // Moved to where the tempo is another one.
    let moved = arrange::set_clip_start(&doc, clip, 0).unwrap().edit.doc;
    assert!(plan_of(&moved, clip, RATE).is_some());
}

#[test]
fn a_file_with_a_tempo_is_stretched_to_the_songs() {
    let (doc, track) = ramped();
    let (doc, clip) = record(&doc, track, 0, BAR);
    // It was at 96 as the song: nothing to make. At 120 it is.
    let same = set_source_tempo(&doc, clip, tempo(9_600)).map_or(doc.clone(), |(e, _)| e.doc);
    assert!(plan_of(&same, clip, RATE).is_none());
    let (e, _) = set_source_tempo(&doc, clip, tempo(12_000)).unwrap();
    let made = plan_of(&e.doc, clip, RATE).unwrap();
    // A bar of the song (2.5 s at 96) is a bar of the take (2 s of the file at 120).
    assert_eq!(made.frames, 120_000);
    assert_eq!(made.file_frame_at(0), 1_000.0);
    assert!((made.file_frame_at(120_000) - 97_000.0).abs() < 1e-6);
    assert!((made.file_frame_at(60_000) - 49_000.0).abs() < 1e-6);
    assert_eq!((made.start, made.end), (0, BAR));
    let (_, c) = e.doc.clip(clip).unwrap();
    assert_eq!(made.audio, c.active_audio().unwrap().take.source.id.0);
    assert_eq!(made.clip, clip.0);
}

#[test]
fn the_pitch_alone_is_made_at_the_files_speed() {
    let (doc, track) = ramped();
    let (doc, clip) = record(&doc, track, BAR, BAR);
    let up = set_clip_semitones(&doc, clip, 2).unwrap().doc;
    let made = plan_of(&up, clip, RATE).unwrap();
    let (_, c) = up.clip(clip).unwrap();
    let map = TempoMap::new(&up.timeline);
    let feed = feed_clip(&map, RATE, c).unwrap();
    assert!(matches!(made.place, Place::Straight { file_frame } if file_frame == feed.file_frame));
    assert_eq!(made.file_frame_at(77), (feed.file_frame + 77) as f64);
    assert_eq!((made.semitones, made.keep_formant), (2.0, true));
    assert_eq!(
        (made.frames, made.file_end),
        (feed.end - feed.start, feed.file_end)
    );
    assert_eq!(made_feed(&feed, &made).made, Some(made.key));
    assert_eq!(made_feed(&feed, &made).file_frame, 0);
    // The cents too, and each value is other audio.
    let fine = set_clip_cents(&up, clip, -30).unwrap().doc;
    let finer = plan_of(&fine, clip, RATE).unwrap();
    assert!((finer.semitones - 1.7).abs() < 1e-6);
    assert_ne!(finer.key, made.key);
    let plain = set_clip_keep_formant(&up, clip, false).unwrap().doc;
    assert_ne!(plan_of(&plain, clip, RATE).unwrap().key, made.key);
    assert_ne!(plan_of(&up, clip, 44_100).unwrap().key, made.key);
    // Back at 0 nothing is made; 「声の感じを保つ」 alone makes nothing either.
    let back = set_clip_semitones(&up, clip, 0).unwrap().doc;
    assert!(plan_of(&back, clip, RATE).is_none());
    let off = set_clip_keep_formant(&back, clip, false).unwrap().doc;
    assert!(plan_of(&off, clip, RATE).is_none());
}

#[test]
fn a_tuned_clip_is_made_and_strength_nothing_is_the_old_way() {
    use meloet_model::{ScaleMode, set_clip_gain, set_clip_tune_scale, set_clip_tune_strength};
    let (doc, track) = ramped();
    let (doc, clip) = record(&doc, track, BAR, BAR);
    // Recorded to the song's own map and not pitched: the file plays as it is.
    assert!(plan_of(&doc, clip, RATE).is_none());
    let tuned = |percent, root, mode| {
        set_clip_tune_strength(&doc, clip, percent, root, mode)
            .unwrap()
            .doc
    };
    let c_major = tuned(60, 0, ScaleMode::Major);
    let made = plan_of(&c_major, clip, RATE).unwrap();
    let mut notes = [false; 12];
    for n in [0, 2, 4, 5, 7, 9, 11] {
        notes[n] = true;
    }
    assert_eq!(
        made.tune,
        Some(TunePlan {
            scale: notes,
            strength: 60
        })
    );
    // At the file's speed, and the voice keeps its character while its pitch moves.
    assert!(matches!(made.place, Place::Straight { .. }));
    assert_eq!((made.semitones, made.keep_formant), (0.0, true));
    let tuning = made.tuning().unwrap();
    assert_eq!((tuning.strength, tuning.transpose), (0.6, 0.0));
    assert_eq!(tuning.scale, notes);

    // The strength and the notes are in the key; the name of the scale is not.
    assert_eq!(plan_of(&c_major, clip, RATE).unwrap().key, made.key);
    let stronger = plan_of(&tuned(61, 0, ScaleMode::Major), clip, RATE).unwrap();
    assert_ne!(stronger.key, made.key);
    let d_major = plan_of(&tuned(60, 2, ScaleMode::Major), clip, RATE).unwrap();
    assert_ne!(d_major.key, made.key);
    assert!(d_major.tune.unwrap().scale[1] && !d_major.tune.unwrap().scale[0]);
    let a_minor = plan_of(&tuned(60, 9, ScaleMode::Minor), clip, RATE).unwrap();
    assert_eq!(a_minor.key, made.key);
    // B major goes round the octave.
    let b_major = plan_of(&tuned(60, 11, ScaleMode::Major), clip, RATE).unwrap();
    assert_eq!(
        b_major.tune.unwrap().scale,
        [
            false, true, false, true, true, false, true, false, true, false, true, true
        ]
    );
    let off = set_clip_keep_formant(&c_major, clip, false).unwrap().doc;
    let plain = plan_of(&off, clip, RATE).unwrap();
    assert!(!plain.keep_formant);
    assert_ne!(plain.key, made.key);

    // What does not change the audio does not make it again: the volume, the song's scale
    // (it is not in the clip), a change of the song somewhere else.
    let louder = set_clip_gain(
        &c_major,
        clip,
        meloet_model::ClipGain::from_db(-6.0).unwrap(),
    )
    .unwrap()
    .doc;
    assert_eq!(plan_of(&louder, clip, RATE).unwrap().key, made.key);
    let later = timeline::set_tempo_at(&c_major, 6 * BAR, tempo(6_000))
        .unwrap()
        .doc;
    assert_eq!(plan_of(&later, clip, RATE).unwrap().key, made.key);
    let rescaled = set_clip_tune_scale(&c_major, clip, 9, ScaleMode::Minor)
        .unwrap()
        .doc;
    assert_eq!(plan_of(&rescaled, clip, RATE).unwrap().key, made.key);

    // With the pitch: one run, the tuning told what the transpose is.
    let up = set_clip_semitones(&c_major, clip, 2).unwrap().doc;
    let up = set_clip_cents(&up, clip, -30).unwrap().doc;
    let both = plan_of(&up, clip, RATE).unwrap();
    assert!((both.tuning().unwrap().transpose - 1.7).abs() < 1e-6);
    assert_ne!(both.key, made.key);
    // Strength 0: the plan and the key of the clip that was never tuned.
    let untuned = set_clip_tune_strength(&up, clip, 0, 0, ScaleMode::Major)
        .unwrap()
        .doc;
    let never = set_clip_semitones(&doc, clip, 2).unwrap().doc;
    let never = set_clip_cents(&never, clip, -30).unwrap().doc;
    let (a, b) = (
        plan_of(&untuned, clip, RATE).unwrap(),
        plan_of(&never, clip, RATE).unwrap(),
    );
    assert_eq!((a.key, a.tune, a.tuning()), (b.key, None, None));
    assert_ne!(a.key, both.key);
    let back = set_clip_tune_strength(&c_major, clip, 0, 0, ScaleMode::Major)
        .unwrap()
        .doc;
    assert!(plan_of(&back, clip, RATE).is_none());

    // A clip that follows another tempo: stretched and tuned in one plan.
    let sung = set_source_tempo(&c_major, clip, tempo(12_000))
        .unwrap()
        .0
        .doc;
    let follows = plan_of(&sung, clip, RATE).unwrap();
    assert!(matches!(follows.place, Place::Follow { .. }));
    assert!(follows.tune.is_some());
    let sung_untuned = set_source_tempo(&doc, clip, tempo(12_000)).unwrap().0.doc;
    assert_ne!(follows.key, plan_of(&sung_untuned, clip, RATE).unwrap().key);
}

#[test]
fn the_key_is_what_the_audio_is_made_from_and_nothing_else() {
    let (doc, track) = ramped();
    let (doc, clip) = record(&doc, track, 0, BAR);
    let doc = set_source_tempo(&doc, clip, tempo(12_000)).unwrap().0.doc;
    let key = plan_of(&doc, clip, RATE).unwrap().key;
    // The same again, and after a change somewhere else in the song.
    assert_eq!(plan_of(&doc, clip, RATE).unwrap().key, key);
    let later = timeline::set_tempo_at(&doc, 6 * BAR, tempo(6_000))
        .unwrap()
        .doc;
    assert_eq!(plan_of(&later, clip, RATE).unwrap().key, key);
    // A bar on the tempo is the same, so the audio is.
    let moved = arrange::set_clip_start(&doc, clip, BAR).unwrap().edit.doc;
    assert_eq!(plan_of(&moved, clip, RATE).unwrap().key, key);
    // Into the ramp it is not.
    let ramp = arrange::set_clip_start(&doc, clip, 2 * BAR)
        .unwrap()
        .edit
        .doc;
    let in_ramp = plan_of(&ramp, clip, RATE).unwrap();
    assert_ne!(in_ramp.key, key);
    // Another tempo under it, another tempo of its own, another length.
    let faster = timeline::set_tempo(&doc, tempo(10_000)).unwrap().doc;
    assert_ne!(plan_of(&faster, clip, RATE).unwrap().key, key);
    let sung = set_source_tempo(&doc, clip, tempo(12_001)).unwrap().0.doc;
    assert_ne!(plan_of(&sung, clip, RATE).unwrap().key, key);
    let short = arrange::set_clip_length(&doc, clip, BAR / 2).unwrap().doc;
    assert_ne!(plan_of(&short, clip, RATE).unwrap().key, key);

    // Inside the ramp the file's frames are the take's own seconds at the song's ticks.
    let map = TempoMap::new(&ramp.timeline);
    let Place::Follow { clock, .. } = &in_ramp.place else {
        panic!("follows");
    };
    for tick in [2 * BAR, 2 * BAR + 1_000, 3 * BAR - 1] {
        let frame = (map.seconds_at(tick as f64) - map.seconds_at(2.0 * BAR as f64)) * 48_000.0;
        let want = clock.frame_at(&map, tick as f64);
        let got = in_ramp.file_frame_at(frame.round() as u64);
        assert!((got - want).abs() < 2.0, "{tick}: {got} {want}");
        assert!((want - (1_000.0 + (tick - 2 * BAR) as f64 * 12.5)).abs() < 1e-6);
    }
}

#[test]
fn only_other_audio_in_the_same_places_is_laid_over() {
    let feed = |start: u64, made: Option<u8>, file_frame: i64| FeedClip {
        start,
        end: start + 100,
        uid: [1; 16],
        made: made.map(|k| [k; 32]),
        file_frame,
        file_end: 1_000,
    };
    let old = [feed(0, None, 5), feed(200, Some(1), 0)];
    assert!(!only_audio_differs(&old, &old));
    assert!(only_audio_differs(&old, &[feed(0, Some(2), 0), old[1]]));
    assert!(only_audio_differs(&old, &[old[0], feed(200, None, 9)]));
    assert!(only_audio_differs(&old, &[old[0], feed(200, Some(3), 0)]));
    // The same file from another frame, a clip that moved, a clip more: read anew.
    assert!(!only_audio_differs(&old, &[feed(0, None, 6), old[1]]));
    assert!(!only_audio_differs(&old, &[old[0], feed(201, Some(2), 0)]));
    assert!(!only_audio_differs(&old, &old[..1]));
}
