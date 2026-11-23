use std::collections::HashSet;
use std::sync::Arc;

use super::*;
use crate::automation::{self as auto, LaneId};
use crate::effects::EffectKind;
use crate::history::History;
use crate::id_book::{IdBook, held};
use crate::mixer::{Bus, Gain, Insert, Pan, StripId};
use crate::{
    AudioShape, NewAudio, NewAudioTake, PluginRef, TICKS_PER_QUARTER, VocalRecording,
    add_vocal_recordings,
};

const T1: TrackId = TrackId(1);
const Q: u64 = TICKS_PER_QUARTER;

fn ok(e: Result<Edit, EditError>) -> Doc {
    e.unwrap().doc
}

fn plugin(key: &str, state: &[u8]) -> Arc<PluginRef> {
    Arc::new(PluginRef::new(key, "Plug", "Maker", "/p").with_state(Arc::from(state)))
}

/// Piano track 1 with everything a track can have: a plugin as its instrument, two clips (one
/// with two takes), its fader, pan and sends moved, a reverb and a plugin effect (passed by)
/// in its slots, lanes for the fader, a parameter of each insert and of the instrument, and its
/// mute on. Returns the doc and the ids of the reverb and the plugin effect.
fn full_track() -> (Doc, InsertId, InsertId) {
    let strip = StripId::Track(T1);
    let mut doc = Doc::new_default();
    doc = ok(set_instrument(
        &doc,
        T1,
        InstrumentRef::Plugin(plugin("vst3:synth", b"S0")),
    ));
    let take = |origin, length| NewTake {
        origin,
        length,
        notes: vec![Note {
            start: 0,
            length: 1,
            key: 60,
            velocity: 100,
            from: None,
        }],
        controls: Vec::new(),
    };
    doc = add_recording(&doc, T1, vec![take(0, 2 * Q), take(0, 2 * Q)], 1)
        .unwrap()
        .0
        .doc;
    doc = add_recording(&doc, T1, vec![take(8 * Q, Q)], 0)
        .unwrap()
        .0
        .doc;
    doc = ok(mixer::set_gain(&doc, strip, Gain::from_db(-6.0)));
    doc = ok(mixer::set_pan(&doc, strip, Pan::new(-30)));
    doc = ok(mixer::set_send(&doc, T1, Bus::B, Gain::from_db(-10.0)));
    doc = ok(mixer::add_effect(&doc, strip, 0, EffectKind::Reverb));
    doc = ok(mixer::set_param(&doc, strip, 0, EffectKind::Reverb, 0, 2.5));
    doc = ok(mixer::add_plugin_effect(
        &doc,
        strip,
        2,
        plugin("lv2:comp", b"C0"),
    ));
    doc = ok(mixer::toggle_bypass(&doc, strip, 2));
    let id_at = |doc: &Doc, slot: usize| doc.insert(strip, slot).unwrap().id;
    let (reverb, comp) = (id_at(&doc, 0), id_at(&doc, 2));
    for target in [
        AutoTarget::Volume,
        AutoTarget::Insert {
            insert: reverb,
            param: 0,
        },
        AutoTarget::Insert {
            insert: comp,
            param: 7,
        },
        AutoTarget::Instrument { param: 3 },
    ] {
        let (e, lane) = auto::add_lane(&doc, T1, target, false).unwrap();
        doc = e.doc;
        let (e, lane, _) = auto::add_point(&doc, lane, 0, Some(0.25), 0.5).unwrap();
        doc = auto::add_point(&e.doc, lane, 4 * Q, Some(0.75), 0.5)
            .unwrap()
            .0
            .doc;
    }
    doc = ok(toggle_mute(&doc, T1));
    (doc, reverb, comp)
}

/// Every insert id of the song, of tracks, returns and the master.
fn insert_ids(doc: &Doc) -> Vec<InsertId> {
    let channels = (doc.tracks.iter().map(|t| &t.channel))
        .chain(doc.returns.iter().map(|r| &r.channel))
        .chain([&*doc.master]);
    channels
        .flat_map(|c| c.inserts.iter().flatten().map(|i| i.id))
        .collect()
}

/// Every id the song holds is below `next_id` and given once (takes may be shared).
fn ids_once(doc: &Doc) {
    let mut seen = HashSet::new();
    let lanes = doc
        .tracks
        .iter()
        .flat_map(|t| &t.automation)
        .map(|l| l.id.0);
    let clips = doc.tracks.iter().flat_map(|t| &t.clips).map(|c| c.id.0);
    for id in insert_ids(doc)
        .into_iter()
        .map(|i| i.0)
        .chain(lanes)
        .chain(clips)
    {
        assert!(id < doc.next_id, "{id} is not below {}", doc.next_id);
        assert!(seen.insert(id), "{id} is given twice");
    }
    doc.check_lanes().unwrap();
}

fn same_insert(a: &Insert, b: &Insert) -> bool {
    let mut b = b.clone();
    b.id = a.id;
    *a == b
}

#[test]
fn the_copy_has_everything_of_the_track_with_new_ids_right_below_it() {
    let (doc, reverb, comp) = full_track();
    let mut doc = doc;
    doc.tracks.push(Arc::new(Track {
        id: TrackId(99),
        name: "ベース".to_owned(),
        ..Track::clone(&doc.tracks[0])
    }));
    doc.tracks[1] = Arc::new(Track {
        clips: Vec::new(),
        automation: Vec::new(),
        channel: Channel::default(),
        ..Track::clone(&doc.tracks[1])
    });
    let d = duplicate_track(&doc, T1).unwrap();
    let new = &d.edit.doc;
    let copy = d.track;
    assert_eq!(copy, TrackId(100), "past every track id");
    assert_eq!(
        (d.from, d.edit.step),
        (T1, StepKind::DuplicateTrack { track: copy })
    );
    let order: Vec<TrackId> = new.tracks.iter().map(|t| t.id).collect();
    assert_eq!(order, [T1, copy, TrackId(99)]);
    assert!(
        Arc::ptr_eq(&doc.tracks[0], &new.tracks[0]),
        "the track stays as it is"
    );
    assert!(Arc::ptr_eq(&doc.tracks[1], &new.tracks[2]));

    let (a, b) = (&new.tracks[0], &new.tracks[1]);
    assert_eq!(b.name, "ピアノ 2");
    assert_eq!(b.color, a.color, "the color of the track, not of the place");
    assert_eq!(
        (&b.kind, &b.instrument, b.sends, b.muted),
        (&a.kind, &a.instrument, a.sends, a.muted)
    );
    assert!(b.muted);
    assert_eq!(
        (b.channel.gain, b.channel.pan),
        (a.channel.gain, a.channel.pan)
    );

    // The inserts: the same effects with their values and marks, under new ids.
    assert_eq!(d.inserts.len(), 2);
    for (from, to) in (a.channel.inserts.iter()).zip(&b.channel.inserts) {
        match (from, to) {
            (Some(from), Some(to)) => {
                assert_ne!(from.id, to.id);
                assert!(same_insert(from, to), "{from:?} {to:?}");
                assert!(d.inserts.contains(&(from.id, to.id)));
            }
            (None, None) => {}
            other => panic!("{other:?}"),
        }
    }
    let copied = |id: InsertId| d.inserts.iter().find(|(f, _)| *f == id).unwrap().1;
    assert!(b.channel.inserts[2].as_ref().unwrap().bypassed);

    // The clips: new ids, the same takes.
    assert_eq!(b.clips.len(), 2);
    for (from, to) in a.clips.iter().zip(&b.clips) {
        assert_ne!(from.id, to.id);
        assert_eq!(
            (&from.name, from.start, from.end, from.active),
            (&to.name, to.start, to.end, to.active)
        );
        assert_eq!(from.takes, to.takes);
        for (x, y) in from.keys().iter().zip(to.keys()) {
            assert!(Arc::ptr_eq(&x.take, &y.take), "the takes are shared");
        }
    }
    assert_eq!(b.clips[0].takes.len(), 2);

    // The lanes: new ids, the same points, aimed at the copy's inserts.
    assert_eq!(b.automation.len(), 4);
    for (from, to) in a.automation.iter().zip(&b.automation) {
        assert_ne!(from.id, to.id);
        assert_eq!(
            (&from.points, from.step, from.resting),
            (&to.points, to.step, to.resting)
        );
        assert_eq!(to.target, d.target(from.target));
    }
    let targets: Vec<AutoTarget> = b.automation.iter().map(|l| l.target).collect();
    assert_eq!(
        targets,
        [
            AutoTarget::Volume,
            AutoTarget::Insert {
                insert: copied(reverb),
                param: 0
            },
            AutoTarget::Insert {
                insert: copied(comp),
                param: 7
            },
            AutoTarget::Instrument { param: 3 },
        ]
    );

    // Every id is new: 1 track, 2 clips, 2 inserts and 4 lanes.
    assert_eq!(new.next_id, copy.0 + 1 + 2 + 2 + 4);
    ids_once(new);
    let mut book = IdBook::new(&doc);
    book.saw(new, Some(&doc)).unwrap();
}

#[test]
fn the_track_and_its_copy_change_apart() {
    let (doc, reverb, _) = full_track();
    let d = duplicate_track(&doc, T1).unwrap();
    let (doc, copy) = (d.edit.doc.clone(), d.track);
    let lane_of = |doc: &Doc, track: TrackId, target: AutoTarget| {
        doc.track(track).unwrap().lane(target).unwrap().clone()
    };

    // The track changes: its reverb, its fader lane, its first clip.
    let strip = StripId::Track(T1);
    let mut changed = ok(mixer::set_param(&doc, strip, 0, EffectKind::Reverb, 0, 4.0));
    let lane = lane_of(&changed, T1, AutoTarget::Volume).id;
    changed = auto::add_point(&changed, lane, 2 * Q, Some(1.0), 0.5)
        .unwrap()
        .0
        .doc;
    let first = changed.track(T1).unwrap().clips[0].id;
    changed = ok(delete_clips(&changed, &[first], false));
    assert!(Arc::ptr_eq(
        doc.track(copy).unwrap(),
        changed.track(copy).unwrap()
    ));
    assert_ne!(
        lane_of(&changed, T1, AutoTarget::Volume).points,
        lane_of(&changed, copy, AutoTarget::Volume).points
    );

    // The copy changes: its reverb (by its own strip), its fader lane, its name.
    let target = d.target(AutoTarget::Insert {
        insert: reverb,
        param: 0,
    });
    let mut changed = ok(mixer::set_param(
        &doc,
        StripId::Track(copy),
        0,
        EffectKind::Reverb,
        0,
        1.0,
    ));
    let lane = lane_of(&changed, copy, target).id;
    changed = ok(auto::delete_points(&changed, lane, &[0]).map(|(e, _)| e));
    changed = ok(rename_track(&changed, copy, "写し"));
    assert!(Arc::ptr_eq(
        doc.track(T1).unwrap(),
        changed.track(T1).unwrap()
    ));
    ids_once(&changed);
}

#[test]
fn undo_takes_the_copy_away_and_redo_brings_it_back() {
    let (mut doc, _, _) = full_track();
    let before = doc.clone();
    let mut h = History::new();
    let d = duplicate_track(&doc, T1).unwrap();
    let after = d.edit.doc.clone();
    h.apply(&mut doc, d.edit);
    assert_eq!(held(&doc), held(&after));
    h.undo(&mut doc);
    assert_eq!(held(&doc), held(&before));
    assert!(doc.next_id >= after.next_id, "the ids are not given again");
    h.redo(&mut doc);
    assert_eq!(held(&doc), held(&after));

    // A copy of the copy, after it and after the track again: never an id twice.
    let mut book = IdBook::new(&doc);
    for track in [d.track, T1] {
        let before = doc.clone();
        let e = duplicate_track(&doc, track).unwrap();
        h.apply(&mut doc, e.edit);
        book.saw(&doc, Some(&before)).unwrap();
        ids_once(&doc);
    }
    let names: Vec<&str> = doc.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["ピアノ", "ピアノ 4", "ピアノ 2", "ピアノ 3"]);
}

#[test]
fn a_vocal_track_is_copied_with_its_input_and_its_audio() {
    let (e, track) = add_vocal_track(&Doc::new_default(), None, crate::Language::Ja).unwrap();
    let doc = set_vocal_input(&e.doc, track, VocalInput::Stereo12)
        .unwrap()
        .doc;
    let recording = VocalRecording {
        track,
        audio: NewAudio {
            uid: [7; 16],
            file: "歌 9.wav".into(),
            frames: 96_000,
            sample_rate: 48_000,
            channels: 2,
            bits: Some(24),
        },
        takes: vec![NewAudioTake {
            origin: 0,
            length: 4 * Q,
            first_frame: 0,
            frames: 96_000,
        }],
    };
    let doc = add_vocal_recordings(&doc, vec![recording], None)
        .unwrap()
        .0
        .doc;
    let d = duplicate_track(&doc, track).unwrap();
    let new = &d.edit.doc;
    let (a, b) = (new.track(track).unwrap(), new.track(d.track).unwrap());
    assert_eq!(b.name, "歌 2");
    assert_eq!(
        b.kind,
        TrackKind::Vocal {
            input: VocalInput::Stereo12
        }
    );
    assert_eq!(b.clips.len(), 1);
    assert_ne!(a.clips[0].id, b.clips[0].id);
    assert_eq!(b.clips[0].takes.shape(), Some(&AudioShape::RECORDED));
    assert!(Arc::ptr_eq(
        &a.clips[0].takes.vocal()[0].take,
        &b.clips[0].takes.vocal()[0].take
    ));
    assert_eq!(new.audio, doc.audio, "the audio table stays as it is");
    ids_once(new);
}

#[test]
fn the_copy_is_named_by_the_rule_of_new_tracks() {
    let named = |name: &str, others: &[&str]| {
        let mut doc = rename_track(&Doc::new_default(), T1, name).unwrap().doc;
        for (n, other) in others.iter().enumerate() {
            doc.tracks.push(Arc::new(Track {
                id: TrackId(10 + n as u32),
                name: (*other).to_owned(),
                ..Track::clone(&doc.tracks[0])
            }));
        }
        let d = duplicate_track(&doc, T1).unwrap();
        d.edit.doc.track(d.track).unwrap().name.clone()
    };
    assert_eq!(named("リード", &[]), "リード 2");
    assert_eq!(named("リード", &["リード 2", "リード 3"]), "リード 4");
    assert_eq!(named("リード 2", &["リード"]), "リード 3");
    assert_eq!(
        named("リード 2", &[]),
        "リード 3",
        "the number is never left out"
    );
    assert_eq!(named("リード 12", &["リード 2"]), "リード 3");
    // Not a number the rule puts there.
    assert_eq!(named("リード 1", &[]), "リード 1 2");
    assert_eq!(named("リード 02", &[]), "リード 02 2");
    assert_eq!(named("リード +3", &[]), "リード +3 2");
    assert_eq!(named("808", &[]), "808 2");
}

#[test]
fn the_folded_lanes_of_the_track_are_folded_on_the_copy() {
    let (doc, reverb, comp) = full_track();
    let d = duplicate_track(&doc, T1).unwrap();
    let on_reverb = AutoTarget::Insert {
        insert: reverb,
        param: 0,
    };
    let folded = [
        (T1, AutoTarget::Volume),
        (TrackId(7), AutoTarget::Pan),
        (T1, on_reverb),
    ];
    assert_eq!(
        d.folded(&folded),
        [
            (d.track, AutoTarget::Volume),
            (d.track, d.target(on_reverb))
        ]
    );
    assert_ne!(d.target(on_reverb), on_reverb);
    let on_comp = AutoTarget::Insert {
        insert: comp,
        param: 7,
    };
    assert!(
        d.edit
            .doc
            .track(d.track)
            .unwrap()
            .lane(d.target(on_comp))
            .is_some()
    );
    let lane: LaneId = d.edit.doc.track(d.track).unwrap().automation[0].id;
    assert!(lane.0 > d.track.0);
}

#[test]
fn a_track_that_is_not_there_or_no_id_left() {
    let doc = Doc::new_default();
    assert_eq!(
        duplicate_track(&doc, TrackId(9)),
        Err(EditError::NoSuchTrack(TrackId(9)))
    );
    let full = Doc {
        next_id: u32::MAX,
        ..doc
    };
    assert_eq!(duplicate_track(&full, T1), Err(EditError::OutOfIds));
}

/// Round 12 (E2, E3, A8, M7): what a track holds of waves 3 and 4 names no id, and the copy
/// has it as it is.
#[test]
fn the_arpeggio_the_knobs_a_comp_and_a_tune_are_copied() {
    let (e, vocal) = add_vocal_track(&Doc::new_default(), None, crate::Language::Ja).unwrap();
    let recording = VocalRecording {
        track: vocal,
        audio: NewAudio {
            uid: [7; 16],
            file: "歌 9.wav".into(),
            frames: 192_000,
            sample_rate: 48_000,
            channels: 1,
            bits: Some(24),
        },
        takes: [0, 96_000]
            .map(|first_frame| NewAudioTake {
                origin: 0,
                length: 4 * Q,
                first_frame,
                frames: 96_000,
            })
            .to_vec(),
    };
    let mut doc = add_vocal_recordings(&e.doc, vec![recording], None)
        .unwrap()
        .0
        .doc;
    let piano = Arc::make_mut(&mut doc.tracks[0]);
    piano.arp = crate::Arp {
        on: true,
        octaves: 3,
        ..crate::Arp::default()
    };
    piano.knobs = (crate::SoundKnobs::default())
        .with(crate::SoundKnob::Release, 90.0)
        .unwrap();
    let singer = Arc::make_mut(&mut doc.tracks[1]);
    let clip = Arc::make_mut(&mut singer.clips[0]);
    assert_eq!(clip.takes.len(), 2);
    clip.comp = vec![
        crate::CompPiece { from: 0, take: 0 },
        crate::CompPiece { from: Q, take: 1 },
    ];
    let crate::ClipTakes::Vocal { shape, .. } = &mut clip.takes else {
        panic!("a vocal clip");
    };
    shape.tune = Some(crate::Tune {
        root: 2,
        mode: crate::ScaleMode::Dorian,
        strength: 40,
    });
    for track in [T1, vocal] {
        let d = duplicate_track(&doc, track).unwrap();
        let new = &d.edit.doc;
        let (a, b) = (new.track(track).unwrap(), new.track(d.track).unwrap());
        assert_eq!((b.arp, b.knobs), (a.arp, a.knobs));
        for (x, y) in a.clips.iter().zip(&b.clips) {
            assert_eq!(y.comp, x.comp);
            assert_eq!(y.takes.shape(), x.takes.shape());
        }
        ids_once(new);
    }
}
