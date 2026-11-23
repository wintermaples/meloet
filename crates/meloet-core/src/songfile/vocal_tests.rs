//! Vocal tracks, the audio table and vocal clips in the song file (docs/architecture.md
//! 22-10, 22-15).

use serde_json::{Value, json};

use super::*;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

/// The new song with a vocal track (id 40) below the piano: two clips that share an audio
/// file, the first with two takes, one of them recorded inside a tempo ramp.
pub(super) fn vocal_song() -> Song {
    let mut song = Song::new_default();
    let tempo = song.doc.timeline.tempo;
    let source = |id: u32, file: &str, sha256: Option<[u8; 32]>| {
        Arc::new(AudioSource {
            id: AudioId(id),
            uid: std::array::from_fn(|i| (id as u8).wrapping_mul(17).wrapping_add(i as u8)),
            file: file.to_owned(),
            frames: 960_000,
            sample_rate: 48_000,
            channels: 1,
            // A file brought in may be of a kind with no bits (mp3).
            bits: sha256.is_none().then_some(24),
            sha256,
        })
    };
    let recorded = source(50, "歌 テイク 1.wav", None);
    let brought = source(51, "ドラム.wav", Some([0xab; 32]));
    let take = |id: u32, source: &Arc<AudioSource>, first_frame: u64, sync| {
        Arc::new(AudioTake {
            id: TakeId(id),
            source: source.clone(),
            first_frame,
            frames: 240_000,
            sync,
        })
    };
    let plain = Some(SourceTempo {
        tempo,
        head_ramp: false,
        points: Vec::new(),
    });
    let ramped = Some(SourceTempo {
        tempo,
        head_ramp: true,
        points: vec![TempoPoint {
            tick: 2 * BAR,
            tempo: Tempo::from_centi(12_000).unwrap(),
            ramp: false,
        }],
    });
    let (t52, t53, t54) = (
        take(52, &recorded, 1_234, plain),
        take(53, &recorded, 480_000, ramped),
        take(54, &brought, 0, None),
    );
    let clip = |id: u32, start: u64, takes: Vec<(Arc<AudioTake>, i64)>, active, shape| {
        Arc::new(Clip {
            id: ClipId(id),
            name: format!("歌 テイク {id}"),
            start,
            end: start + 4 * BAR,
            takes: ClipTakes::Vocal {
                takes: (takes.into_iter())
                    .map(|(take, offset)| ClipTake { take, offset })
                    .collect(),
                shape,
            },
            active,
            comp: Vec::new(),
        })
    };
    let shaped = AudioShape {
        fade_in: 240,
        fade_out: Q,
        gain: ClipGain::from_db(-3.0).unwrap(),
        follow: false,
        semitones: -2,
        cents: 15,
        keep_formant: false,
        tune: None,
    };
    let keys = Track::clone(&song.doc.tracks[0]);
    song.doc.tracks.push(Arc::new(Track {
        id: TrackId(40),
        name: "歌".to_owned(),
        color: TrackColor::for_index(1),
        kind: TrackKind::Vocal {
            input: VocalInput::Stereo12,
        },
        instrument: InstrumentRef::SINE,
        sends: [Gain::SILENT; 2],
        clips: vec![
            clip(
                41,
                4 * BAR,
                vec![(t52.clone(), 0), (t53, -(Q as i64))],
                1,
                AudioShape::RECORDED,
            ),
            // The same take in a second clip, as after a cut.
            clip(42, 8 * BAR, vec![(t52, -(4 * BAR as i64))], 0, shaped),
            clip(43, 16 * BAR, vec![(t54, 0)], 0, AudioShape::RECORDED),
        ],
        ..keys
    }));
    song.doc.audio = Arc::new(vec![recorded, brought]);
    song.doc.next_id = 60;
    song
}

fn vocal_json() -> Value {
    serde_json::from_slice(&encode(&vocal_song())).unwrap()
}

fn reads(v: &Value) -> bool {
    match decode(&serde_json::to_vec(v).unwrap()) {
        Ok(_) => true,
        Err(ReadError::Broken) => false,
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn a_vocal_track_comes_back_as_it_was_and_shares_its_takes_and_files() {
    let song = vocal_song();
    let back = decode(&encode(&song)).unwrap();
    assert_eq!(back, song);
    let clips = &back.doc.tracks[1].clips;
    let take = |clip: usize, index: usize| &clips[clip].takes.vocal()[index].take;
    assert!(Arc::ptr_eq(take(0, 0), take(1, 0)));
    assert!(Arc::ptr_eq(&take(0, 0).source, &take(0, 1).source));
    assert!(Arc::ptr_eq(&take(0, 0).source, &back.doc.audio[0]));
    // A song with no vocal track is written as before, but for the empty audio table.
    let plain: Value = serde_json::from_slice(&encode(&Song::new_default())).unwrap();
    assert_eq!(plain.get("audioTakes"), None);
    assert_eq!(plain["song"]["audio"], json!([]));
    assert_eq!(plain["song"]["tracks"][0].get("kind"), None);
    assert_eq!(plain["song"]["tracks"][0].get("input"), None);
}

#[test]
fn the_shape_of_a_vocal_track_on_disk() {
    let v = vocal_json();
    assert_eq!(v["version"], 7);
    let track = &v["song"]["tracks"][1];
    assert_eq!(track["kind"], "vocal");
    assert_eq!(track["input"], "stereo12");
    assert_eq!(
        v["song"]["audio"][1],
        json!({"id": 51, "uid": "63646566676869 6a6b6c6d6e6f707172".replace(' ', ""),
               "file": "ドラム.wav", "frames": 960_000, "rate": 48_000, "channels": 1,
               "sha256": "ab".repeat(32)})
    );
    assert_eq!(v["song"]["audio"][0].get("sha256"), None);
    // The bits of a recorded file; a file of a kind with none leaves them out.
    assert_eq!(v["song"]["audio"][0]["bits"], 24);
    assert_eq!(v["song"]["audio"][1].get("bits"), None);
    assert_eq!(
        v["audioTakes"],
        json!([
            {"id": 52, "audio": 50, "firstFrame": 1_234, "frames": 240_000,
             "sync": {"tempo": 9_600, "headRamp": false, "points": []}},
            {"id": 53, "audio": 50, "firstFrame": 480_000, "frames": 240_000,
             "sync": {"tempo": 9_600, "headRamp": true,
                      "points": [{"tick": 2 * BAR, "tempo": 12_000, "ramp": false}]}},
            {"id": 54, "audio": 51, "firstFrame": 0, "frames": 240_000},
        ])
    );
    assert_eq!(
        track["clips"][1],
        json!({"id": 42, "name": "歌 テイク 42", "start": 8 * BAR, "end": 12 * BAR,
               "active": 0, "takes": [{"take": 52, "offset": -(4 * BAR as i64)}],
               "shape": {"fadeIn": 240, "fadeOut": Q, "gainDb": -3.0, "follow": false,
                         "semitones": -2, "cents": 15, "keepFormant": false}})
    );
    // The keys takes are no audio takes.
    assert_eq!(v["takes"], json!([]));
}

#[test]
fn vocal_tracks_and_audio_that_cannot_be_do_not_read() {
    assert!(reads(&vocal_json()));
    let broken = |change: &dyn Fn(&mut Value)| {
        let mut v = vocal_json();
        change(&mut v);
        !reads(&v)
    };
    fn track(v: &mut Value) -> &mut Value {
        &mut v["song"]["tracks"][1]
    }
    assert!(broken(&|v| v["song"]["audio"][0]["bits"] = json!(12)));
    // The kind and the input.
    assert!(broken(&|v| track(v)["kind"] = json!("drums")));
    assert!(broken(&|v| track(v)["input"] = json!("mono3")));
    assert!(broken(&|v| v["song"]["tracks"][0]["input"] = json!("mono1")));
    // A clip holds what its track holds.
    assert!(broken(&|v| {
        track(v).as_object_mut().unwrap().remove("kind");
        track(v).as_object_mut().unwrap().remove("input");
    }));
    assert!(broken(&|v| {
        track(v)["clips"][0]
            .as_object_mut()
            .unwrap()
            .remove("shape");
    }));
    // The audio table.
    for (field, bad) in [
        ("file", json!("../歌.wav")),
        ("file", json!("音/歌.wav")),
        ("file", json!("")),
        ("file", json!("..")),
        ("uid", json!("abc")),
        ("uid", json!("zz".repeat(16))),
        ("rate", json!(0)),
        ("channels", json!(3)),
        ("channels", json!(0)),
        ("id", json!(51)),
        ("id", json!(41)),
        ("id", json!(60)),
        ("frames", json!(100)),
    ] {
        assert!(
            broken(&|v| v["song"]["audio"][0][field] = bad.clone()),
            "{field} {bad}"
        );
    }
    assert!(broken(&|v| v["song"]["audio"][1]["sha256"] = json!("ab")));
    // The takes.
    for (field, bad) in [
        ("audio", json!(99)),
        ("frames", json!(0)),
        ("firstFrame", json!(960_000)),
        ("firstFrame", json!(u64::MAX)),
        ("id", json!(53)),
        ("id", json!(50)),
        ("id", json!(41)),
    ] {
        assert!(
            broken(&|v| v["audioTakes"][0][field] = bad.clone()),
            "{field} {bad}"
        );
    }
    assert!(broken(&|v| v["audioTakes"][1]["sync"]["tempo"] = json!(1)));
    assert!(broken(&|v| v["audioTakes"][1]["sync"]["points"] =
        json!([{"tick": 0, "tempo": 9_600, "ramp": false}])));
    // The shape.
    for (field, bad) in [
        ("gainDb", json!(12.5)),
        ("gainDb", json!(-61.0)),
        ("semitones", json!(13)),
        ("cents", json!(-51)),
        ("fadeIn", json!(4 * BAR)),
        ("fadeOut", json!(u64::MAX)),
    ] {
        assert!(
            broken(&|v| track(v)["clips"][1]["shape"][field] = bad.clone()),
            "{field} {bad}"
        );
    }
    // A keys clip does not play an audio take, nor a vocal clip a keys take.
    assert!(broken(
        &|v| track(v)["clips"][0]["takes"][0]["take"] = json!(99)
    ));
}

#[test]
fn a_vocal_track_without_its_input_records_from_the_first_one() {
    let mut v = vocal_json();
    v["song"]["tracks"][1]
        .as_object_mut()
        .unwrap()
        .remove("input");
    let song = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(
        song.doc.tracks[1].kind,
        TrackKind::Vocal {
            input: VocalInput::Mono1
        }
    );
    // What is not saved: the ● and the monitor are off in a song just opened.
    assert!(song.session.armed_vocals.is_empty());
    assert!(song.session.monitored.is_empty());
    assert_eq!(song.session.last_keys, None);
    assert_eq!(song.keys_target(), Some(TrackId(1)));
}

#[test]
fn the_stretching_and_the_pitch_of_a_clip_come_back() {
    let mut song = vocal_song();
    // Clip 42 shows take 52, as clip 41 has it too.
    let clip = ClipId(42);
    let edits: [&dyn Fn(&Doc) -> meloet_model::Edit; 5] = [
        &|doc| meloet_model::set_clip_follow(doc, clip, true).unwrap(),
        &|doc| meloet_model::set_clip_semitones(doc, clip, 2).unwrap(),
        &|doc| meloet_model::set_clip_cents(doc, clip, -30).unwrap(),
        &|doc| meloet_model::set_clip_keep_formant(doc, clip, true).unwrap(),
        &|doc| {
            let tempo = Tempo::from_centi(12_050).unwrap();
            meloet_model::set_source_tempo(doc, clip, tempo).unwrap().0
        },
    ];
    for edit in edits {
        song.doc = edit(&song.doc).doc;
    }

    let bytes = encode(&song);
    let back = decode(&bytes).unwrap();
    assert_eq!(back, song);
    let clips = &back.doc.tracks[1].clips;
    let shape = clips[1].takes.shape().unwrap();
    assert!(shape.follow && shape.keep_formant);
    assert_eq!((shape.semitones, shape.cents), (2, -30));
    // The new take is the clip's alone, with the one tempo; the other clip keeps the old.
    let new = &clips[1].takes.vocal()[0].take;
    assert_eq!(new.id, TakeId(60));
    assert_eq!(
        new.sync,
        Some(SourceTempo {
            tempo: Tempo::from_centi(12_050).unwrap(),
            head_ramp: false,
            points: Vec::new(),
        })
    );
    assert_eq!((new.first_frame, new.frames), (1_234, 240_000));
    let old = &clips[0].takes.vocal()[0].take;
    assert_eq!(old.id, TakeId(52));
    assert_eq!(old.sync.as_ref().unwrap().tempo, song.doc.timeline.tempo);
    assert!(Arc::ptr_eq(&new.source, &old.source));
    assert_eq!(back.doc.next_id, 61);

    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["version"], 7);
    assert_eq!(
        v["song"]["tracks"][1]["clips"][1]["shape"],
        json!({"fadeIn": 240, "fadeOut": Q, "gainDb": -3.0, "follow": true,
               "semitones": 2, "cents": -30, "keepFormant": true})
    );
    assert_eq!(
        v["song"]["tracks"][1]["clips"][1]["takes"],
        json!([{"take": 60, "offset": -(4 * BAR as i64)}])
    );
    let takes = v["audioTakes"].as_array().unwrap();
    assert_eq!(takes.len(), 4);
    assert!(takes.contains(
        &json!({"id": 60, "audio": 50, "firstFrame": 1_234, "frames": 240_000,
                "sync": {"tempo": 12_050, "headRamp": false, "points": []}})
    ));
}
