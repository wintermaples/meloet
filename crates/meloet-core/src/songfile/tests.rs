use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use meloet_model::effects;

use super::*;
use crate::settings::temp_dir;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;

/// A song with two clips, a clip of three takes (one shared with the other clip), CC and bend
/// at the same tick and a loop.
fn rich_song(name: &str, notes_per_take: u64) -> Song {
    let take = |id: u32, base: u8| {
        Arc::new(Take {
            origin: None,
            id: TakeId(id),
            length: 16 * Q,
            notes: (0..notes_per_take)
                .map(|i| Note {
                    from: None,
                    start: i * 7,
                    length: 30 + i % 5,
                    key: base + (i % 24) as u8,
                    velocity: 1 + (i % 127) as u8,
                })
                .collect(),
            controls: vec![
                ControlPoint {
                    tick: 0,
                    value: ControlValue::Cc {
                        number: 64,
                        value: 127,
                    },
                },
                ControlPoint {
                    tick: Q / 2,
                    value: ControlValue::PitchBend(-8192),
                },
                ControlPoint {
                    tick: Q / 2,
                    value: ControlValue::Cc {
                        number: 1,
                        value: 12,
                    },
                },
            ]
            .into(),
        })
    };
    let (t2, t5, t6, t7) = (take(2, 40), take(5, 48), take(6, 50), take(7, 52));
    let clip = |id: u32, start: u64, end: u64, takes: Vec<(Arc<Take>, i64)>, active: usize| {
        Arc::new(Clip {
            id: ClipId(id),
            name: format!("クリップ {id}"),
            start,
            end,
            takes: meloet_model::ClipTakes::Keys(
                takes
                    .into_iter()
                    .map(|(take, offset)| ClipTake { take, offset })
                    .collect(),
            ),
            active,
            comp: Vec::new(),
        })
    };
    let mut doc = Doc::new_default();
    doc.name = name.into();
    doc.timeline = Arc::new(Timeline {
        head_ramp: true,
        tempo_points: vec![
            TempoPoint {
                tick: 6 * Q,
                tempo: Tempo::from_centi(10_000).unwrap(),
                ramp: true,
            },
            TempoPoint {
                tick: 12 * Q + Q / 2,
                tempo: Tempo::from_centi(12_000).unwrap(),
                ramp: false,
            },
        ],
        meter_changes: vec![MeterChange {
            bar: 3,
            meter: Meter::FOUR_FOUR,
        }],
        ..Timeline::new(
            Tempo::from_centi(12_345).unwrap(),
            Meter::new(6, 8).unwrap(),
        )
    });
    // 4 is the one id below 8 that no clip and no take has; the inserts have 8 and up.
    doc.markers = Arc::new(vec![Marker {
        id: MarkerId(4),
        tick: 6 * Q,
        name: "サビ".into(),
    }]);
    doc.loop_range = TickRange::new(16 * Q, 32 * Q);
    doc.next_id = 8;
    let mut track = (*doc.tracks[0]).clone();
    track.channel.gain = Gain::from_db(-3.5);
    track.channel.pan = Pan::new(-20);
    track.sends = [Gain::from_db(-15.0), Gain::from_db(-18.0)];
    let mut compressor = Insert::new(NO_INSERT_ID, EffectKind::Compressor);
    compressor.set(effects::compressor::THRESHOLD, -24.5);
    let mut delay = Insert::new(NO_INSERT_ID, EffectKind::Delay);
    delay.set(effects::delay::INTERVAL_STEP, 1.0);
    delay.set(effects::delay::SYNC, 0.0);
    delay.set(effects::delay::PING_PONG, 1.0);
    delay.set(effects::delay::FEEDBACK, 62.5);
    track.channel.inserts = [None, Some(compressor), None, Some(delay)];
    let a = Arc::make_mut(&mut doc.returns[0]);
    a.muted = true;
    a.channel.gain = Gain::from_db(-6.0);
    a.channel.pan = Pan::new(30);
    let b = Arc::make_mut(&mut doc.returns[1]);
    b.channel.inserts = [
        None,
        None,
        Some(Insert::new(NO_INSERT_ID, EffectKind::Chorus)),
        None,
    ];
    let master = Arc::make_mut(&mut doc.master);
    master.gain = Gain::SILENT;
    master.pan = Pan::new(-30);
    (master.inserts[0].as_mut().unwrap()).set(effects::limiter::CEILING, -1.0);
    master.inserts[1] = Some(Insert::new(NO_INSERT_ID, EffectKind::Eq4));
    track.clips = vec![
        clip(1, 0, 16 * Q, vec![(t2.clone(), 0)], 0),
        clip(
            3,
            16 * Q,
            32 * Q,
            vec![
                (t5, 0),
                (t6, Q as i64),
                (t7, -(Q as i64)),
                (t2, 2 * Q as i64),
            ],
            1,
        ),
    ];
    doc.tracks = vec![Arc::new(track)];
    number_inserts(&mut doc);
    Song {
        doc,
        session: Session {
            armed_vocals: std::collections::BTreeSet::new(),
            last_keys: None,
            monitored: std::collections::BTreeSet::new(),
            play_target: Some(TrackId(1)),
            loop_on: true,
            roll: RollSettings {
                grid: Grid::new(8, true).unwrap(),
                scale: MusicalScale::new(2, ScaleMode::Minor, true).unwrap(),
                quantize: QuantizeSettings::new(
                    Grid::new(32, true).unwrap(),
                    75,
                    10,
                    QuantizeAlign::StartAndEnd,
                )
                .unwrap(),
            },
            soloed: BTreeSet::new(),
            folded_lanes: Vec::new(),
            master_lane_folded: false,
        },
    }
}

#[test]
fn a_song_comes_back_as_it_was_and_shares_its_takes() {
    let song = rich_song("はじめての曲", 50);
    let back = decode(&encode(&song)).unwrap();
    assert_eq!(back, song);
    let clips = &back.doc.tracks[0].clips;
    assert!(Arc::ptr_eq(
        &clips[0].keys()[0].take,
        &clips[1].keys()[3].take
    ));
}

#[test]
fn the_shape_on_disk() {
    let v: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 1))).unwrap();
    assert_eq!(v["format"], "meloet-song");
    assert_eq!(v["version"], 7);
    assert_eq!(v["song"]["tempo"], 12_345);
    assert_eq!(v["song"]["meter"], serde_json::json!([6, 8]));
    assert_eq!(
        v["song"]["loop"],
        serde_json::json!({"start": 16 * Q, "end": 32 * Q})
    );
    assert!(v["song"].get("tempoChanges").is_none());
    assert_eq!(v["song"]["nextId"], 14, "8 and the six inserts");
    let track = &v["song"]["tracks"][0];
    assert!(track.get("muted").is_none(), "not muted: not written");
    assert_eq!(
        track["instrument"],
        serde_json::json!({"kind": "builtin", "name": "grandPiano"})
    );
    assert_eq!(track["plugins"], serde_json::json!([]));
    assert_eq!(track["automation"], serde_json::json!([]));
    assert_eq!(v["song"]["audio"], serde_json::json!([]));
    assert_eq!(track["gainDb"], -3.5);
    assert_eq!(track["pan"], -20);
    assert_eq!(track["sends"], serde_json::json!({"a": -15.0, "b": -18.0}));
    assert_eq!(
        track["inserts"],
        serde_json::json!([
            null,
            {"id": 8, "effect": "compressor",
             "params": {"threshold": -24.5, "ratio": 4.0, "attack": 10.0, "release": 100.0,
                        "makeup": 0.0}},
            null,
            {"id": 9, "effect": "delay",
             "params": {"intervalStep": "eighthDotted", "intervalMs": 250.0, "sync": false,
                        "feedback": 62.5, "highCut": 5000.0, "lowCut": 200.0, "pingPong": true,
                        "mix": 100.0}}
        ])
    );
    let mixer = &v["song"]["mixer"];
    let a = &mixer["returns"]["a"];
    assert_eq!(a["gainDb"], -6.0);
    assert_eq!(a["pan"], 30);
    assert_eq!(a["muted"], true);
    assert_eq!(a["inserts"][0]["effect"], "reverb");
    assert_eq!(
        a["inserts"][0]["params"]["decay"], 2.4,
        "an f32 is written as it is shown"
    );
    let b = &mixer["returns"]["b"];
    assert!(b.get("muted").is_none(), "not muted: not written");
    assert_eq!(b["inserts"][0], serde_json::Value::Null);
    assert_eq!(b["inserts"][2]["effect"], "chorus");
    assert_eq!(
        mixer["master"]["gainDb"],
        serde_json::Value::Null,
        "−∞ is null"
    );
    assert_eq!(mixer["master"]["pan"], -30);
    assert_eq!(mixer["master"]["inserts"][0]["params"]["ceiling"], -1.0);
    assert_eq!(mixer["master"]["inserts"].as_array().unwrap().len(), 4);
    let text = String::from_utf8(encode(&rich_song("x", 1))).unwrap();
    assert!(text.contains(r#""decay":2.4,"#) && text.contains(r#""rate":0.8,"#));
    assert_eq!(
        track["clips"][1]["takes"][1],
        serde_json::json!({"take": 6, "offset": Q})
    );
    assert_eq!(
        track["clips"][1]["takes"][2],
        serde_json::json!({"take": 7, "offset": -(Q as i64)})
    );
    let take = &v["takes"][0];
    assert_eq!(take["notes"][0], serde_json::json!([0, 30, 40, 1]));
    assert_eq!(
        take["controls"],
        serde_json::json!([[0, 64, 127], [Q / 2, "bend", -8192], [Q / 2, 1, 12]])
    );
    assert_eq!(
        v["session"],
        serde_json::json!({"playTarget": 1, "loopOn": true,
                           "grid": {"division": 8, "triplet": true},
                           "scale": {"root": 2, "mode": "minor", "lock": true,
                                     "highlight": true},
                           "quantize": {"division": 32, "triplet": true, "strength": 75,
                                        "swing": 10, "align": "startAndEnd"}})
    );
}

#[test]
fn piano_roll_settings_missing_or_not_understood_are_the_defaults() {
    let song = rich_song("x", 3);
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&song)).unwrap();
    // A file without the piano roll's settings.
    let session = v["session"].as_object_mut().unwrap();
    session.remove("grid");
    session.remove("scale");
    session.remove("quantize");
    let back = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(back.session.roll, RollSettings::default());
    assert_eq!(back.doc, song.doc);
    assert_eq!(back.doc.loop_range, song.doc.loop_range);

    v["session"]["grid"] = serde_json::json!({"division": 7, "triplet": false});
    v["session"]["scale"] = serde_json::json!({"root": 4, "mode": "phrygian", "lock": true});
    v["session"]["quantize"] = serde_json::json!("later");
    let back = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(back.session.roll, RollSettings::default());

    v["session"]["scale"] = serde_json::json!({"root": 4, "mode": "major", "lock": true,
                                              "someday": 1});
    let back = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(
        back.session.roll.scale,
        MusicalScale::new(4, ScaleMode::Major, true).unwrap()
    );
}

#[test]
fn unknown_fields_are_skipped_and_later_places_may_be_missing() {
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 3))).unwrap();
    v["someday"] = serde_json::json!({"a": 1});
    v["song"]["tracks"][0]["mixer"] = serde_json::json!(true);
    let track = v["song"]["tracks"][0].as_object_mut().unwrap();
    track.remove("plugins");
    track.remove("automation");
    v["song"].as_object_mut().unwrap().remove("audio");
    let back = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(back, rich_song("x", 3));
}

#[test]
fn mute_is_saved_and_solo_is_not() {
    let mut song = rich_song("x", 3);
    Arc::make_mut(&mut song.doc.tracks[0]).muted = true;
    song.session.soloed.insert(TrackId(1));
    let bytes = encode(&song);
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["song"]["tracks"][0]["muted"], true);
    assert!(v["session"].get("soloed").is_none());
    let back = decode(&bytes).unwrap();
    assert!(back.doc.tracks[0].muted);
    assert!(back.session.soloed.is_empty());
    assert_eq!(back.doc, song.doc);
}

fn with_mute_sets(song: &Song) -> Song {
    let soloed = BTreeSet::from([TrackId(1)]);
    let doc = &song.doc;
    let doc = mute_sets::save_mute_set(doc, &soloed, "爽快セット", false)
        .unwrap()
        .doc;
    let doc = meloet_model::edit::toggle_mute(&doc, TrackId(1))
        .unwrap()
        .doc;
    let doc = mixer::toggle_return_mute(&doc, Bus::B).doc;
    let doc = mute_sets::save_mute_set(&doc, &BTreeSet::new(), "クール", false)
        .unwrap()
        .doc;
    Song {
        doc,
        session: song.session.clone(),
    }
}

#[test]
fn mute_sets_come_back_and_a_song_without_them_is_written_as_before() {
    let plain = rich_song("x", 3);
    let bytes = encode(&plain);
    assert!(
        !String::from_utf8(bytes.clone())
            .unwrap()
            .contains("muteSets")
    );
    assert_eq!(decode(&bytes).unwrap(), plain);

    let song = with_mute_sets(&plain);
    let v: serde_json::Value = serde_json::from_slice(&encode(&song)).unwrap();
    assert_eq!(
        v["song"]["muteSets"],
        serde_json::json!([
            {"name": "爽快セット", "tracks": [1], "muted": [], "mutedReturns": ["a"],
             "soloed": [1]},
            {"name": "クール", "tracks": [1], "muted": [1], "mutedReturns": ["a", "b"]}
        ])
    );
    let back = decode(&encode(&song)).unwrap();
    assert_eq!(back, song);

    // Every set deleted: the same bytes as a song that never had one.
    let mut none = back.doc;
    for name in ["爽快セット", "クール"] {
        none = mute_sets::delete_mute_set(&none, name).unwrap().doc;
    }
    let none = Song {
        doc: meloet_model::edit::toggle_mute(&none, TrackId(1))
            .unwrap()
            .doc,
        session: plain.session.clone(),
    };
    let none = Song {
        doc: mixer::toggle_return_mute(&none.doc, Bus::B).doc,
        ..none
    };
    assert_eq!(encode(&none), bytes);

    // A file without the field reads as before.
    let mut v = v;
    v["song"].as_object_mut().unwrap().remove("muteSets");
    let read = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(read.doc.mute_sets.is_empty());
    assert_eq!(read.doc.tracks, song.doc.tracks);
}

#[test]
fn mute_sets_that_cannot_be_are_left_out_and_cost_nothing_else() {
    let song = with_mute_sets(&rich_song("x", 3));
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&song)).unwrap();
    let next_id = v["song"]["nextId"].as_u64().unwrap();
    v["song"]["muteSets"] = serde_json::json!([
        // Ids out of order and twice, a muted track the set does not name, an id that this
        // song never gave out, a return that is not known.
        {"name": " A ", "tracks": [5, 1, 5, next_id + 3], "muted": [1, 9, next_id + 3],
         "mutedReturns": ["b", "c"], "soloed": [5], "later": true},
        {"name": "A", "tracks": [1], "muted": []},
        {"name": "", "tracks": [1]},
        {"name": "B", "tracks": "all"},
        "C",
        {"name": "D", "tracks": [1], "muted": [1]}
    ]);
    let back = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(
        back.doc.mute_sets[..],
        [
            MuteSet {
                name: "A".into(),
                tracks: vec![TrackId(1), TrackId(5)],
                muted: vec![TrackId(1)],
                muted_returns: [false, true],
                soloed: vec![TrackId(5)],
            },
            MuteSet {
                name: "D".into(),
                tracks: vec![TrackId(1)],
                muted: vec![TrackId(1)],
                muted_returns: [false, false],
                soloed: Vec::new(),
            },
        ]
    );
    assert_eq!(back.doc.tracks, song.doc.tracks);
}

#[test]
fn version_7_is_written_and_version_6_still_reads() {
    let song = rich_song("x", 3);
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&song)).unwrap();
    assert_eq!(v["version"], 7);
    v["version"] = 6.into();
    let back = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(back.doc.tracks, song.doc.tracks);
    assert_eq!(back.session.roll, song.session.roll);
}

#[test]
fn a_scale_without_highlight_is_highlighted() {
    let mut song = rich_song("x", 3);
    song.session.roll.scale = MusicalScale::new(9, ScaleMode::Blues, true).unwrap();
    song.session.roll.scale.highlight = false;
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&song)).unwrap();
    assert_eq!(
        v["session"]["scale"],
        serde_json::json!({"root": 9, "mode": "blues", "lock": true, "highlight": false})
    );
    assert_eq!(
        decode(&serde_json::to_vec(&v).unwrap())
            .unwrap()
            .session
            .roll
            .scale,
        song.session.roll.scale
    );
    // A song of version 6 has no `highlight`.
    v["version"] = 6.into();
    v["session"]["scale"] = serde_json::json!({"root": 2, "mode": "minor", "lock": false});
    let scale = decode(&serde_json::to_vec(&v).unwrap())
        .unwrap()
        .session
        .roll
        .scale;
    assert_eq!(
        (scale.root(), scale.mode, scale.lock, scale.highlight),
        (2, ScaleMode::Minor, false, true)
    );
    // Every kind round trips.
    for mode in ScaleMode::ALL {
        song.session.roll.scale = MusicalScale::new(4, mode, false).unwrap();
        assert_eq!(
            decode(&encode(&song)).unwrap().session.roll.scale.mode,
            mode
        );
    }
}

#[test]
fn newer_older_and_broken_files_do_not_read() {
    let good: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 3))).unwrap();
    let with = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        f(&mut v);
        decode(&serde_json::to_vec(&v).unwrap())
    };
    assert!(matches!(
        with(&|v| v["version"] = 8.into()),
        Err(ReadError::TooNew { version: 8 })
    ));
    // Before version 6 a tick and a tempo meant something else.
    for version in 1..=5u32 {
        assert!(matches!(
            with(&|v| v["version"] = version.into()),
            Err(ReadError::TooOld { version: v }) if v == version
        ));
    }
    assert!(matches!(
        decode(b"{\"format\": \"meloet-song\", \"vers"),
        Err(ReadError::Broken)
    ));
    assert!(matches!(decode(b""), Err(ReadError::Broken)));
    let broken: [&dyn Fn(&mut serde_json::Value); 8] = [
        &|v| v["format"] = "other".into(),
        &|v| v["song"]["tempo"] = 1.into(),
        &|v| v["song"]["meter"] = serde_json::json!([4, 3]),
        &|v| v["song"]["nextId"] = 3.into(),
        &|v| v["song"]["tracks"][0]["clips"][1]["active"] = 9.into(),
        &|v| v["song"]["tracks"][0]["clips"][1]["takes"][0]["take"] = 99.into(),
        &|v| v["song"]["tracks"][0]["clips"][1]["start"] = 100.into(),
        &|v| v["takes"][0]["notes"][0] = serde_json::json!([0, 0, 40, 1]),
    ];
    for (i, f) in broken.iter().enumerate() {
        assert!(matches!(with(*f), Err(ReadError::Broken)), "case {i}");
    }
}

#[test]
fn a_save_keeps_the_one_before_and_only_a_good_one() {
    let dir = temp_dir("songfile-prev");
    let path = dir.join("曲.meloet");
    save(&path, &rich_song("1", 3)).unwrap();
    assert!(!previous_path(&path).exists());
    save(&path, &rich_song("2", 3)).unwrap();
    assert_eq!(read(&path).unwrap().doc.name, "2");
    assert_eq!(read(&previous_path(&path)).unwrap().doc.name, "1");
    // A broken file is written over but does not push out the good copy.
    fs::write(&path, b"not a song").unwrap();
    save(&path, &rich_song("3", 3)).unwrap();
    assert_eq!(read(&path).unwrap().doc.name, "3");
    assert_eq!(read(&previous_path(&path)).unwrap().doc.name, "1");
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names.len(), 2, "no file left over: {names:?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_first_save_takes_the_next_free_folder_and_name() {
    let base = temp_dir("songfile-new");
    let song = Song::new_default();
    let (p1, n1) = save_new(&base, &song).unwrap();
    assert_eq!(n1, "はじめての曲");
    assert_eq!(p1, base.join("はじめての曲/はじめての曲.meloet"));
    fs::create_dir(base.join("はじめての曲 2")).unwrap();
    let (p3, n3) = save_new(&base, &song).unwrap();
    assert_eq!(n3, "はじめての曲 3");
    assert_eq!(p3, base.join("はじめての曲 3/はじめての曲 3.meloet"));
    assert_eq!(read(&p3).unwrap().doc.name, "はじめての曲 3");
    let mut odd = Song::new_default();
    odd.doc.name = "a/b:c".into();
    assert_eq!(save_new(&base, &odd).unwrap().1, "a_b_c");
    let _ = fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_written_fails_and_leaves_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let base = temp_dir("songfile-ro");
    fs::set_permissions(&base, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(base.join("probe"), b"").is_ok() {
        // Running as root: permissions do not stop anything.
        return;
    }
    assert!(save_new(&base, &Song::new_default()).is_err());
    let inner = base.join("inner");
    fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir(&inner).unwrap();
    let path = inner.join("x.meloet");
    save(&path, &rich_song("1", 3)).unwrap();
    fs::set_permissions(&inner, fs::Permissions::from_mode(0o555)).unwrap();
    assert!(save(&path, &rich_song("2", 3)).is_err());
    assert_eq!(read(&path).unwrap().doc.name, "1");
    fs::set_permissions(&inner, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(fs::read_dir(&inner).unwrap().count(), 1);
    let _ = fs::remove_dir_all(base);
}

const CRASH_CHILD: &str = "MELOET_SAVE_CRASH_CHILD";
const CRASH_NOTES: u64 = 4_000;
const CRASH_READY: &str = "MELOET-CRASH-CHILD-READY";
/// Songs made before the saves start, so that the child spends its time saving.
const CRASH_SONGS: u32 = 40;

/// The child of `a_save_killed_at_any_moment_leaves_a_whole_song`: saves songs "0", "1", …
/// over one file until it is killed. Does nothing in a plain test run.
#[test]
fn crash_child() {
    let Some(path) = std::env::var_os(CRASH_CHILD).map(PathBuf::from) else {
        return;
    };
    let songs: Vec<Song> = (0..CRASH_SONGS)
        .map(|n| rich_song(&n.to_string(), CRASH_NOTES))
        .collect();
    let started = Instant::now();
    for (n, song) in songs.iter().enumerate() {
        save(&path, song).unwrap();
        if n == 1 {
            println!("\n{CRASH_READY}");
        }
    }
    eprintln!("{:?} for {CRASH_SONGS} saves", started.elapsed());
    panic!("never killed");
}

/// Kills a process in the middle of saving, at many moments: each time `<name>.meloet` and
/// `<name>.meloet.prev` read, and are whole songs that were saved, the copy older or the same.
#[test]
fn a_save_killed_at_any_moment_leaves_a_whole_song() {
    const ROUNDS: u32 = 120;
    let exe = std::env::current_exe().unwrap();
    let dir = temp_dir("songfile-crash");
    let mut seed = u64::from(std::process::id()) | 1;
    let mut landed = HashSet::new();
    // Rounds killed between writing a temporary file and renaming it into place.
    let mut mid_save = 0;
    for round in 0..ROUNDS {
        let path = dir.join(format!("{round}/曲.meloet"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut child = Command::new(&exe)
            .args(["--exact", "songfile::tests::crash_child", "--nocapture"])
            .args(["--test-threads", "1"])
            .env(CRASH_CHILD, &path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut out = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while line.trim() != CRASH_READY {
            line.clear();
            assert!(out.read_line(&mut line).unwrap() > 0, "the child ended");
        }
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        std::thread::sleep(Duration::from_micros(seed % 60_000));
        child.kill().unwrap();
        child.wait().unwrap();

        let song = read(&path).unwrap_or_else(|e| panic!("round {round}: {e:?}"));
        let n: u32 = song.doc.name.parse().unwrap();
        assert_eq!(
            song,
            rich_song(&n.to_string(), CRASH_NOTES),
            "round {round}"
        );
        let prev = read(&previous_path(&path)).unwrap_or_else(|e| panic!("round {round}: {e:?}"));
        let m: u32 = prev.doc.name.parse().unwrap();
        assert!(m <= n, "round {round}: .prev {m} is newer than {n}");
        assert_eq!(
            prev,
            rich_song(&m.to_string(), CRASH_NOTES),
            "round {round}"
        );
        landed.insert(n);
        let folder = path.parent().unwrap();
        if fs::read_dir(folder)
            .unwrap()
            .any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".new"))
        {
            mid_save += 1;
        }
        let _ = fs::remove_dir_all(folder);
    }
    eprintln!("{ROUNDS} kills, {mid_save} with a temporary file left, songs {landed:?}");
    assert!(landed.len() > 1, "{landed:?}");
    assert!(
        mid_save >= ROUNDS / 10,
        "only {mid_save} kills inside a save"
    );
    let _ = fs::remove_dir_all(dir);
}

/// `rich_song` with the shown take of clip 3 quantized: its origin (id 8) is in no clip, its
/// first note came from origin note 1, its second was written by hand.
fn quantized_song() -> Song {
    let mut song = rich_song("q", 3);
    let origin = Arc::new(Take {
        id: TakeId(8),
        length: 4 * Q,
        notes: (0..3)
            .map(|i| Note {
                start: 10 + i * 100,
                length: 50,
                key: 60,
                velocity: 90,
                from: None,
            })
            .collect(),
        controls: Vec::new().into(),
        origin: None,
    });
    let quantized = Arc::new(Take {
        id: TakeId(9),
        length: 4 * Q,
        notes: vec![
            Note {
                start: 0,
                length: 50,
                key: 60,
                velocity: 90,
                from: OriginIndex::new(1),
            },
            Note {
                start: Q,
                length: 50,
                key: 62,
                velocity: 90,
                from: None,
            },
        ]
        .into(),
        controls: Vec::new().into(),
        origin: Some(TakeOrigin {
            take: origin,
            settings: QuantizeSettings::new(
                Grid::new(0, true).unwrap(),
                60,
                40,
                QuantizeAlign::StartAndEnd,
            )
            .unwrap(),
        }),
    });
    let mut track = (*song.doc.tracks[0]).clone();
    let mut clip = (*track.clips[1]).clone();
    let active = clip.active;
    clip.takes.keys_mut().unwrap()[active].take = quantized;
    track.clips[1] = Arc::new(clip);
    song.doc.tracks[0] = Arc::new(track);
    song.doc.next_id = 10;
    renumber_inserts(&mut song.doc);
    song
}

#[test]
fn a_quantized_take_comes_back_with_its_origin() {
    let song = quantized_song();
    let bytes = encode(&song);
    let back = decode(&bytes).unwrap();
    assert_eq!(back, song);
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let take = |id: u32| {
        v["takes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        take(9)["origin"],
        serde_json::json!({"take": 8, "division": 0, "triplet": true, "strength": 60,
                           "swing": 40, "align": "startAndEnd"})
    );
    assert_eq!(
        take(9)["notes"],
        serde_json::json!([[0, 50, 60, 90, 1], [Q, 50, 62, 90]])
    );
    assert!(take(8).get("origin").is_none());
    assert_eq!(take(8)["notes"][0], serde_json::json!([10, 50, 60, 90]));
}

#[test]
fn origins_that_cannot_be_do_not_read() {
    let good: serde_json::Value = serde_json::from_slice(&encode(&quantized_song())).unwrap();
    let at = |v: &serde_json::Value, id: u32| {
        v["takes"]
            .as_array()
            .unwrap()
            .iter()
            .position(|t| t["id"] == id)
            .unwrap()
    };
    let (o, q) = (at(&good, 8), at(&good, 9));
    let with = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        f(&mut v);
        decode(&serde_json::to_vec(&v).unwrap())
    };
    assert!(with(&|_| {}).is_ok());
    let broken: [&dyn Fn(&mut serde_json::Value); 6] = [
        // An origin note past the end of the origin.
        &|v| v["takes"][q]["notes"][0][4] = 3.into(),
        // An origin note in a take with no origin.
        &|v| v["takes"][o]["notes"][0] = serde_json::json!([10, 50, 60, 90, 0]),
        // No such origin take.
        &|v| v["takes"][q]["origin"]["take"] = 77.into(),
        // An origin of an origin.
        &|v| {
            v["takes"][o]["origin"] = v["takes"][q]["origin"].clone();
            v["takes"][o]["origin"]["take"] = 2.into();
        },
        &|v| v["takes"][q]["origin"]["division"] = 3.into(),
        &|v| v["takes"][q]["origin"]["strength"] = 101.into(),
    ];
    for (i, f) in broken.iter().enumerate() {
        assert!(matches!(with(*f), Err(ReadError::Broken)), "case {i}");
    }
}

#[test]
fn every_builtin_instrument_comes_back() {
    let mut song = rich_song("x", 1);
    let first = song.doc.tracks[0].clone();
    song.doc.tracks = BuiltinInstrument::all()
        .enumerate()
        .map(|(i, b)| {
            Arc::new(Track {
                id: TrackId(i as u32 + 1),
                instrument: InstrumentRef::Builtin(b),
                clips: if i == 0 {
                    first.clips.clone()
                } else {
                    Vec::new()
                },
                ..Track::clone(&first)
            })
        })
        .collect();
    number_inserts(&mut song.doc);
    let bytes = encode(&song);
    assert_eq!(decode(&bytes).unwrap(), song);
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        v["song"]["tracks"][19]["instrument"],
        serde_json::json!({"kind": "builtin", "name": "studioKit"})
    );
    assert_eq!(
        v["song"]["tracks"][21]["instrument"],
        serde_json::json!({"kind": "builtin", "name": "brushKit"})
    );
}

/// The 「オルガン」 of a song from before 84 opens as the pipe organ, whose name it keeps.
#[test]
fn the_organ_of_an_older_song_is_the_pipe_organ() {
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 3))).unwrap();
    v["song"]["tracks"][0]["instrument"] = serde_json::json!({"kind": "builtin", "name": "organ"});
    let song = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(
        song.doc.tracks[0].instrument,
        InstrumentRef::Builtin(BuiltinInstrument::Organ)
    );
    let again: serde_json::Value = serde_json::from_slice(&encode(&song)).unwrap();
    assert_eq!(
        again["song"]["tracks"][0]["instrument"],
        serde_json::json!({"kind": "builtin", "name": "organ"})
    );
}

#[test]
fn an_instrument_that_is_not_known_reads_as_the_sine() {
    let good: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 3))).unwrap();
    for instrument in [
        serde_json::json!({"kind": "builtin", "name": "theremin"}),
        serde_json::json!({"kind": "builtin"}),
        serde_json::json!({"kind": "plugin", "name": "grandPiano"}),
        serde_json::json!({"kind": "placeholder"}),
    ] {
        let mut v = good.clone();
        v["song"]["tracks"][0]["instrument"] = instrument.clone();
        let song = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
        assert_eq!(
            song.doc.tracks[0].instrument,
            InstrumentRef::SINE,
            "{instrument}"
        );
        assert_eq!(
            song.doc.tracks[0].clips,
            rich_song("x", 3).doc.tracks[0].clips
        );
    }
}

#[test]
fn a_song_with_no_tracks_comes_back() {
    let mut song = rich_song("x", 1);
    song.doc.tracks.clear();
    song.session.play_target = None;
    let bytes = encode(&song);
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["session"]["playTarget"], serde_json::Value::Null);
    assert_eq!(decode(&bytes).unwrap(), song);
}

#[test]
fn a_selected_track_that_is_gone_is_the_first_track() {
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 1))).unwrap();
    v["session"]["playTarget"] = 9.into();
    let song = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(song.session.play_target, Some(TrackId(1)));
    v["session"].as_object_mut().unwrap().remove("playTarget");
    let song = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(song.session.play_target, Some(TrackId(1)));
}

#[test]
fn offsets_below_0_read_and_offsets_out_of_range_do_not() {
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 1))).unwrap();
    fn offset(v: &mut serde_json::Value) -> &mut serde_json::Value {
        &mut v["song"]["tracks"][0]["clips"][1]["takes"][2]["offset"]
    }
    *offset(&mut v) = serde_json::json!(-20_000);
    let song = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(song.doc.tracks[0].clips[1].keys()[2].offset, -20_000);
    for bad in [
        serde_json::json!(i64::MIN),
        serde_json::json!(u64::MAX),
        serde_json::json!(i64::MAX),
    ] {
        *offset(&mut v) = bad;
        assert!(matches!(
            decode(&serde_json::to_vec(&v).unwrap()),
            Err(ReadError::Broken)
        ));
    }
}

/// What `rich_song` is on disk, to be changed by hand.
fn rich_json() -> serde_json::Value {
    serde_json::from_slice(&encode(&rich_song("x", 3))).unwrap()
}

fn read_json(v: &serde_json::Value) -> Song {
    decode(&serde_json::to_vec(v).unwrap()).unwrap()
}

#[test]
fn a_file_without_the_mixer_reads() {
    let mut v = rich_json();
    v["song"].as_object_mut().unwrap().remove("mixer");
    let track = v["song"]["tracks"][0].as_object_mut().unwrap();
    for field in ["pan", "sends", "inserts"] {
        track.remove(field);
    }
    let song = read_json(&v);
    let t = &song.doc.tracks[0];
    assert_eq!(t.channel.gain, Gain::from_db(-3.5), "the fader was there");
    assert_eq!(t.channel.pan, Pan::CENTER);
    assert_eq!(t.channel.inserts, Inserts::default());
    assert_eq!(t.sends, [Gain::SILENT; 2]);
    assert_eq!(
        mixer_without_ids(&song.doc),
        mixer_without_ids(&Doc::new_default())
    );
    assert_eq!(t.clips, rich_song("x", 3).doc.tracks[0].clips);
    // A track with no fader at all is still not a song.
    v["song"]["tracks"][0]
        .as_object_mut()
        .unwrap()
        .remove("gainDb");
    assert!(matches!(
        decode(&serde_json::to_vec(&v).unwrap()),
        Err(ReadError::Broken)
    ));
}

#[test]
fn effects_and_parameters_that_are_not_known_do_not_cost_the_song() {
    let mut v = rich_json();
    let inserts = &mut v["song"]["tracks"][0]["inserts"];
    // An effect of a later meloet, and things that are no effect at all: empty slots.
    inserts[0] = serde_json::json!({"effect": "phaser", "params": {"rate": 1.0}});
    inserts[2] = serde_json::json!(["what"]);
    // A parameter of a later meloet is skipped; one of another shape, or a step with a name
    // not known, has its default; a missing one too.
    inserts[1]["params"]["knee"] = 6.0.into();
    inserts[1]["params"]["ratio"] = "steep".into();
    inserts[1]["params"]
        .as_object_mut()
        .unwrap()
        .remove("attack");
    inserts[3]["params"]["intervalStep"] = "whole".into();
    inserts[3]["params"]["sync"] = 1.into();
    // More slots than this meloet has.
    inserts
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"effect": "eq4"}));
    let song = read_json(&v);
    let want = rich_song("x", 3);
    let got = &song.doc.tracks[0].channel.inserts;
    assert!(got[0].is_none() && got[2].is_none());
    let compressor = got[1].as_ref().unwrap();
    assert_eq!(compressor.kind(), Some(EffectKind::Compressor));
    assert_eq!(
        compressor.value(effects::compressor::THRESHOLD),
        Some(-24.5)
    );
    assert_eq!(compressor.value(effects::compressor::RATIO), Some(4.0));
    assert_eq!(compressor.value(effects::compressor::ATTACK), Some(10.0));
    let delay = got[3].as_ref().unwrap();
    assert_eq!(delay.value(effects::delay::INTERVAL_STEP), Some(2.0), "1/8");
    assert_eq!(
        delay.value(effects::delay::SYNC),
        Some(1.0),
        "the default: on"
    );
    assert_eq!(delay.value(effects::delay::PING_PONG), Some(1.0));
    assert_eq!(delay.value(effects::delay::FEEDBACK), Some(62.5));
    assert_eq!(song.doc.tracks[0].clips, want.doc.tracks[0].clips);
    assert_eq!(song.doc.returns, want.doc.returns);
    assert_eq!(song.doc.master, want.doc.master);

    // An effect with no parameters written has its defaults.
    let mut v = rich_json();
    v["song"]["mixer"]["returns"]["b"]["inserts"] = serde_json::json!([{"effect": "reverb"}]);
    let b = &read_json(&v).doc.returns[1].channel.inserts;
    let id = b[0].as_ref().unwrap().id;
    assert_eq!(b[0], Some(Insert::new(id, EffectKind::Reverb)));
    assert!(
        b[1..].iter().all(Option::is_none),
        "fewer slots: the rest is empty"
    );

    // A master with no pan (a file from before it had one), or one that does not read: the
    // middle, and the rest of the mixer stays.
    for pan in [
        None,
        Some(serde_json::json!("left")),
        Some(serde_json::json!(1.5)),
    ] {
        let mut v = rich_json();
        let master = v["song"]["mixer"]["master"].as_object_mut().unwrap();
        match pan {
            Some(pan) => master.insert("pan".into(), pan),
            None => master.remove("pan"),
        };
        let song = read_json(&v);
        assert_eq!(song.doc.master.pan, Pan::CENTER);
        assert_eq!(song.doc.master.gain, want.doc.master.gain);
        assert_eq!(song.doc.master.inserts, want.doc.master.inserts);
        assert_eq!(song.doc.returns, want.doc.returns);
    }
    let mut v = rich_json();
    v["song"]["mixer"]["master"]["pan"] = serde_json::json!(900);
    assert_eq!(read_json(&v).doc.master.pan, Pan::new(100));

    // A mixer that is not understood: the one of a new song, and the tracks stay.
    let mut v = rich_json();
    v["song"]["mixer"] = serde_json::json!({"returns": 3});
    let song = read_json(&v);
    assert_eq!(
        mixer_without_ids(&song.doc),
        mixer_without_ids(&Doc::new_default())
    );
    assert_eq!(song.doc.tracks, want.doc.tracks);
}

/// Round 12 M6: the three effects are written by their names (the song file stays at 7) and
/// come back with their values.
#[test]
fn gate_de_esser_and_saturation_come_back() {
    let mut song = rich_song("x", 3);
    let mut gate = Insert::new(NO_INSERT_ID, EffectKind::Gate);
    gate.set(effects::gate::THRESHOLD, -30.5);
    let mut de_esser = Insert::new(NO_INSERT_ID, EffectKind::DeEsser);
    de_esser.set(effects::de_esser::FREQUENCY, 7000.0);
    let mut saturation = Insert::new(NO_INSERT_ID, EffectKind::Saturation);
    saturation.set(effects::saturation::DRIVE, 18.0);
    let track = Arc::make_mut(&mut song.doc.tracks[0]);
    track.channel.inserts = [Some(gate), Some(de_esser), Some(saturation), None];
    number_inserts(&mut song.doc);
    let bytes = encode(&song);
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let inserts = &v["song"]["tracks"][0]["inserts"];
    assert_eq!(
        [0, 1, 2].map(|i| inserts[i]["effect"].clone()),
        ["gate", "deEsser", "saturation"]
    );
    assert_eq!(v["version"], 7);
    assert_eq!(decode(&bytes).unwrap(), song);
}

#[test]
fn mixer_values_out_of_range_are_brought_into_range() {
    let mut v = rich_json();
    let track = &mut v["song"]["tracks"][0];
    track["gainDb"] = 40.0.into();
    track["pan"] = 500.into();
    track["sends"] = serde_json::json!({"a": -200.0, "b": 6.004});
    track["inserts"][1]["params"]["threshold"] = 12.0.into();
    track["inserts"][1]["params"]["ratio"] = 1e30.into();
    track["inserts"][3]["params"]["feedback"] = (-3.0).into();
    let mixer = &mut v["song"]["mixer"];
    mixer["returns"]["a"]["pan"] = (-101).into();
    mixer["returns"]["a"]["gainDb"] = (-60.5).into();
    mixer["master"]["gainDb"] = 6.5.into();
    let song = read_json(&v);
    let t = &song.doc.tracks[0];
    assert_eq!(t.channel.gain.db(), Some(6.0));
    assert_eq!(t.channel.pan.get(), 100);
    assert_eq!(t.sends, [Gain::SILENT, Gain::from_db(6.0)]);
    let compressor = t.channel.inserts[1].as_ref().unwrap();
    assert_eq!(compressor.value(effects::compressor::THRESHOLD), Some(0.0));
    assert_eq!(compressor.value(effects::compressor::RATIO), Some(20.0));
    let delay = t.channel.inserts[3].as_ref().unwrap();
    assert_eq!(delay.value(effects::delay::FEEDBACK), Some(0.0));
    assert_eq!(song.doc.returns[0].channel.pan.get(), -100);
    assert_eq!(song.doc.returns[0].channel.gain, Gain::SILENT);
    assert_eq!(song.doc.master.gain.db(), Some(6.0));
    // A pan or sends of another shape: the middle, nothing sent.
    let mut v = rich_json();
    v["song"]["tracks"][0]["pan"] = "left".into();
    v["song"]["tracks"][0]["sends"] = "all".into();
    let song = read_json(&v);
    assert_eq!(song.doc.tracks[0].channel.pan, Pan::CENTER);
    assert_eq!(song.doc.tracks[0].sends, [Gain::SILENT; 2]);
    // Written again, everything is in range and reads the same.
    assert_eq!(decode(&encode(&song)).unwrap(), song);
}

// ---- plugins, SF2 presets, inserts that are passed by ----

use meloet_model::{InstrumentGroup, PluginChange, PluginPlace};

const SURGE: &str = "vst3:535247455375726758545379000000AA";
const MVERB: &str = "lv2:http://distrho.sf.net/plugins/MVerb";

fn surge(state: &[u8]) -> Arc<PluginRef> {
    let plugin = PluginRef::new(
        SURGE,
        "Surge XT",
        "Surge Synth Team",
        "/usr/lib/vst3/Surge XT.vst3",
    );
    Arc::new(plugin.with_state(state.into()))
}

fn clean_guitar() -> Arc<Sf2Ref> {
    Arc::new(Sf2Ref {
        file: "FluidR3_GM.sf2".into(),
        sha256: "74594e8f4250680adf590507a306655a299935343583256f3b722c48a1bc1cb0".into(),
        bank: 0,
        program: 27,
        name: "Clean Guitar".into(),
        kept: Kept::NONE,
    })
}

/// `rich_song` with a track of Surge XT (a state of every byte value), a track of an SF2
/// preset and one of an SF2 drum kit; MVerb passed by in the first track's last slot, a
/// compressor passed by on return A, and a plugin with no state on the master.
fn added_song() -> Song {
    let mut song = rich_song("x", 3);
    let state: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    let kit = Sf2Ref {
        bank: 128,
        program: 40,
        name: "Brush".into(),
        ..(*clean_guitar()).clone()
    };
    let doc = &mut song.doc;
    for (id, name, instrument) in [
        (20, "Surge XT", InstrumentRef::Plugin(surge(&state))),
        (21, "ギター", InstrumentRef::Sf2(clean_guitar())),
        (22, "ドラム", InstrumentRef::Sf2(Arc::new(kit))),
    ] {
        doc.tracks.push(Arc::new(Track {
            kind: meloet_model::TrackKind::Keys,
            id: TrackId(id),
            name: name.into(),
            color: TrackColor::for_index(1),
            sends: mixer::new_track_sends(&instrument),
            instrument,
            channel: Channel::default(),
            muted: false,
            clips: Vec::new(),
            automation: Vec::new(),
            arp: Default::default(),
            knobs: Default::default(),
        }));
    }
    let mverb = PluginRef::new(MVERB, "MVerb", "DISTRHO", "").with_state([9u8, 8, 7].into());
    let mut passed = Insert::plugin(NO_INSERT_ID, Arc::new(mverb));
    passed.bypassed = true;
    let mut first = Track::clone(&doc.tracks[0]);
    first.channel.inserts[3] = Some(passed);
    doc.tracks[0] = Arc::new(first);
    let mut a = Return::clone(&doc.returns[0]);
    let mut compressor = Insert::new(NO_INSERT_ID, EffectKind::Compressor);
    compressor.bypassed = true;
    a.channel.inserts[1] = Some(compressor);
    doc.returns[0] = Arc::new(a);
    let mut master = Channel::clone(&doc.master);
    master.inserts[2] = Some(Insert::plugin(NO_INSERT_ID, surge(&[])));
    doc.master = Arc::new(master);
    number_inserts(doc);
    song
}

#[test]
fn base64_is_the_usual_one() {
    for (bytes, text) in [
        (&b""[..], ""),
        (b"f", "Zg=="),
        (b"fo", "Zm8="),
        (b"foo", "Zm9v"),
        (b"foob", "Zm9vYg=="),
        (b"fooba", "Zm9vYmE="),
        (b"foobar", "Zm9vYmFy"),
        (&[0xfb, 0xff, 0xfe], "+//+"),
    ] {
        assert_eq!(base64::encode(bytes), text);
        assert_eq!(base64::decode(text).as_deref(), Some(bytes));
    }
    let every: Vec<u8> = (0..=255u8).cycle().take(70_001).collect();
    assert_eq!(base64::decode(&base64::encode(&every)), Some(every));
    for bad in [
        "Z", "Zg", "Zg=", "Zg==Zg==", "Z===", "====", "Zm9v\n", "Zm 9", "Zh==", "Zm9=",
    ] {
        assert_eq!(base64::decode(bad), None, "{bad:?}");
    }
}

#[test]
fn plugins_sf2_presets_and_bypassed_inserts_come_back() {
    let song = added_song();
    let bytes = encode(&song);
    let back = decode(&bytes).unwrap();
    assert_eq!(back, song);
    assert_eq!(encode(&back), bytes, "and are written the same again");
    let t = &back.doc.tracks;
    let InstrumentRef::Plugin(plugin) = &t[1].instrument else {
        panic!("{:?}", t[1].instrument);
    };
    assert_eq!(plugin.state.len(), 1000);
    assert_eq!(plugin.state[255], 255);
    assert_eq!(t[2].instrument, InstrumentRef::Sf2(clean_guitar()));
    assert_eq!(t[3].instrument.group(), InstrumentGroup::Drums);
    let passed = t[0].channel.inserts[3].as_ref().unwrap();
    assert!(passed.bypassed);
    assert_eq!(*passed.plugin_ref().unwrap().state, [9, 8, 7]);
    assert!(
        back.doc.returns[0].channel.inserts[1]
            .as_ref()
            .unwrap()
            .bypassed
    );
    assert!(
        !back.doc.returns[0].channel.inserts[0]
            .as_ref()
            .unwrap()
            .bypassed
    );
    assert!(!back.doc.master.inserts[2].as_ref().unwrap().bypassed);
}

#[test]
fn the_shape_of_plugins_and_presets_on_disk() {
    let v: serde_json::Value = serde_json::from_slice(&encode(&added_song())).unwrap();
    assert_eq!(v["version"], 7);
    let tracks = &v["song"]["tracks"];
    let state: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    assert_eq!(
        tracks[1]["instrument"],
        serde_json::json!({"kind": "plugin", "key": SURGE, "name": "Surge XT",
            "vendor": "Surge Synth Team", "path": "/usr/lib/vst3/Surge XT.vst3",
            "state": base64::encode(&state)})
    );
    assert_eq!(
        tracks[2]["instrument"],
        serde_json::json!({"kind": "sf2", "file": "FluidR3_GM.sf2",
            "sha256": "74594e8f4250680adf590507a306655a299935343583256f3b722c48a1bc1cb0",
            "bank": 0, "program": 27, "name": "Clean Guitar"})
    );
    assert_eq!(tracks[3]["instrument"]["bank"], 128);
    assert_eq!(
        tracks[0]["inserts"][3],
        serde_json::json!({"id": 9,
                           "plugin": {"key": MVERB, "name": "MVerb", "vendor": "DISTRHO",
                                      "state": "CQgH"},
                           "bypassed": true}),
        "no path: not written"
    );
    let a = &v["song"]["mixer"]["returns"]["a"]["inserts"];
    assert_eq!(a[1]["effect"], "compressor");
    assert_eq!(a[1]["bypassed"], true);
    assert!(a[0].get("bypassed").is_none(), "not passed by: not written");
    assert_eq!(
        v["song"]["mixer"]["master"]["inserts"][2],
        serde_json::json!({"id": 15, "plugin": {"key": SURGE, "name": "Surge XT",
            "vendor": "Surge Synth Team", "path": "/usr/lib/vst3/Surge XT.vst3"}}),
        "no state: not written"
    );
    assert_eq!(
        tracks[0]["instrument"],
        serde_json::json!({"kind": "builtin", "name": "grandPiano"})
    );
}

#[test]
fn fields_and_formats_that_are_not_known_are_kept_and_written_back() {
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&added_song())).unwrap();
    // A plugin of a format this meloet does not read, with fields of a later meloet.
    let clap = serde_json::json!({"kind": "plugin", "key": "clap:org.surge-synth-team.surge-xt",
        "name": "Surge XT", "vendor": "Surge Synth Team", "state": "AAEC",
        "preset": {"name": "Init Saw", "bank": 3}, "latency": 64});
    v["song"]["tracks"][1]["instrument"] = clap.clone();
    let preset = serde_json::json!({"kind": "sf2", "file": "Old Keys.sf2", "sha256": "",
        "bank": 8, "program": 4, "name": "Detuned EP", "tuning": [0.5, null]});
    v["song"]["tracks"][2]["instrument"] = preset.clone();
    let slot = serde_json::json!({"plugin": {"key": "ladspa:1433", "name": "", "vendor": "",
                                             "ports": {"2": 0.25}},
                                  "bypassed": true});
    v["song"]["tracks"][0]["inserts"][3] = slot.clone();

    let song = read_json(&v);
    let InstrumentRef::Plugin(plugin) = &song.doc.tracks[1].instrument else {
        panic!("{:?}", song.doc.tracks[1].instrument);
    };
    assert_eq!(&*plugin.key, "clap:org.surge-synth-team.surge-xt");
    assert_eq!(*plugin.state, [0, 1, 2]);
    assert!(plugin.kept.text().unwrap().contains("Init Saw"));
    let InstrumentRef::Sf2(sf2) = &song.doc.tracks[2].instrument else {
        panic!("{:?}", song.doc.tracks[2].instrument);
    };
    assert_eq!((&*sf2.file, sf2.bank, sf2.program), ("Old Keys.sf2", 8, 4));
    let insert = song.doc.tracks[0].channel.inserts[3].as_ref().unwrap();
    assert!(insert.bypassed);
    assert_eq!(&*insert.plugin_ref().unwrap().key, "ladspa:1433");

    // Saved, every field is there again; and the song reads the same from that file.
    let saved = encode(&song);
    let again: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(again["song"]["tracks"][1]["instrument"], clap);
    assert_eq!(again["song"]["tracks"][2]["instrument"], preset);
    // The insert was given an id when the song was read.
    let mut saved_slot = again["song"]["tracks"][0]["inserts"][3].clone();
    assert!(saved_slot.as_object_mut().unwrap().remove("id").is_some());
    assert_eq!(saved_slot, slot);
    assert_eq!(decode(&saved).unwrap(), song);

    // A step that gives the plugin another state keeps what was kept.
    let place = PluginPlace::Instrument(song.doc.tracks[1].id);
    let key = "clap:org.surge-synth-team.surge-xt";
    let e = mixer::set_plugin_state(&song.doc, place, key, [5u8].into(), PluginChange::Window);
    let changed = Song {
        doc: e.unwrap().doc,
        ..song
    };
    let v: serde_json::Value = serde_json::from_slice(&encode(&changed)).unwrap();
    let written = &v["song"]["tracks"][1]["instrument"];
    assert_eq!(written["state"], "BQ==");
    assert_eq!(written["preset"], clap["preset"]);
    assert_eq!(written["latency"], 64);
}

#[test]
fn plugins_and_presets_that_cannot_be_read_do_not_cost_the_song() {
    let good = rich_json();
    let clips = &rich_song("x", 3).doc.tracks[0].clips;
    // No key, no file, a bank or a program that cannot be: the sine, as any unknown sound.
    for instrument in [
        serde_json::json!({"kind": "plugin"}),
        serde_json::json!({"kind": "plugin", "key": ""}),
        serde_json::json!({"kind": "plugin", "key": 7, "name": "x"}),
        serde_json::json!({"kind": "sf2", "bank": 0, "program": 0}),
        serde_json::json!({"kind": "sf2", "file": "", "bank": 0, "program": 0}),
        serde_json::json!({"kind": "sf2", "file": "a.sf2", "bank": 0, "program": 128}),
        serde_json::json!({"kind": "sf2", "file": "a.sf2", "bank": -1, "program": 0}),
        serde_json::json!({"kind": "sf2", "file": "a.sf2", "bank": 70000, "program": 0}),
        serde_json::json!({"kind": "sf2", "file": "a.sf2", "bank": "0", "program": 0}),
        serde_json::json!({"kind": "sf2", "file": "a.sf2", "program": 0}),
        serde_json::json!({"kind": 4}),
        serde_json::json!({}),
    ] {
        let mut v = good.clone();
        v["song"]["tracks"][0]["instrument"] = instrument.clone();
        let song = read_json(&v);
        assert_eq!(
            song.doc.tracks[0].instrument,
            InstrumentRef::SINE,
            "{instrument}"
        );
        assert_eq!(&song.doc.tracks[0].clips, clips);
    }
    // Fields of another shape than expected: read as empty, the plugin itself is kept.
    let mut v = good.clone();
    v["song"]["tracks"][0]["instrument"] = serde_json::json!({"kind": "plugin", "key": "lv2:urn:x",
        "name": 3, "vendor": null, "path": [], "state": "not base64!"});
    let song = read_json(&v);
    assert_eq!(
        song.doc.tracks[0].instrument,
        InstrumentRef::Plugin(Arc::new(PluginRef::new("lv2:urn:x", "", "", "")))
    );
    assert_eq!(&song.doc.tracks[0].clips, clips);
    // Slots: a plugin with no key is an empty slot, and what `bypassed` is, if not a switch,
    // does not cost the effect.
    let mut v = good.clone();
    v["song"]["tracks"][0]["inserts"] = serde_json::json!([
        {"plugin": {"name": "nameless"}},
        {"plugin": "lv2:urn:x"},
        {"effect": "reverb", "params": {}, "bypassed": "yes"},
        {"plugin": {"key": "lv2:urn:x"}, "bypassed": 1}
    ]);
    let song = read_json(&v);
    let inserts = &song.doc.tracks[0].channel.inserts;
    assert!(inserts[0].is_none() && inserts[1].is_none());
    let reverb = inserts[2].as_ref().unwrap();
    assert_eq!(reverb.kind(), Some(EffectKind::Reverb));
    assert!(!reverb.bypassed);
    let plugin = inserts[3].as_ref().unwrap();
    assert_eq!(&*plugin.plugin_ref().unwrap().key, "lv2:urn:x");
    assert!(!plugin.bypassed);
    // The instrument is still an object or the file is no song.
    let mut v = good.clone();
    v["song"]["tracks"][0]["instrument"] = "grandPiano".into();
    assert!(matches!(
        decode(&serde_json::to_vec(&v).unwrap()),
        Err(ReadError::Broken)
    ));
}

#[test]
fn a_step_on_a_plugin_and_its_undo_are_saved_as_the_song_is() {
    let song = added_song();
    let mut doc = song.doc.clone();
    let mut history = meloet_model::History::new();
    let place = PluginPlace::Insert(meloet_model::StripId::Master, 2);
    let e = mixer::set_plugin_state(
        &doc,
        place,
        SURGE,
        [1u8, 2, 3].into(),
        PluginChange::Param(4),
    );
    history.apply(&mut doc, e.unwrap());
    let stepped = Song {
        doc: doc.clone(),
        session: song.session.clone(),
    };
    assert_eq!(decode(&encode(&stepped)).unwrap(), stepped);
    assert!(history.undo(&mut doc).is_some());
    let undone = Song {
        doc,
        session: song.session.clone(),
    };
    assert_eq!(encode(&undone), encode(&song));
}

// ---- tempo points, meter changes, markers, the loop's range in the song ----

#[test]
fn the_shape_of_the_timeline_on_disk() {
    let song = rich_song("x", 1);
    let v: serde_json::Value = serde_json::from_slice(&encode(&song)).unwrap();
    assert_eq!(v["version"], 7);
    let s = &v["song"];
    assert_eq!(s["tempo"], 12_345);
    assert_eq!(s["meter"], serde_json::json!([6, 8]));
    assert_eq!(s["headRamp"], true);
    assert_eq!(
        s["tempoPoints"],
        serde_json::json!([{"tick": 6 * Q, "tempo": 10_000, "ramp": true},
                           {"tick": 12 * Q + Q / 2, "tempo": 12_000, "ramp": false}])
    );
    assert_eq!(
        s["meterChanges"],
        serde_json::json!([{"bar": 3, "meter": [4, 4]}])
    );
    assert_eq!(
        s["markers"],
        serde_json::json!([{"id": 4, "tick": 6 * Q, "name": "サビ"}])
    );
    assert_eq!(
        s["loop"],
        serde_json::json!({"start": 16 * Q, "end": 32 * Q})
    );
    assert!(v["session"].get("loop").is_none());
    assert_eq!(v["session"]["loopOn"], true);
    assert_eq!(decode(&encode(&song)).unwrap(), song);

    // A song with none of it.
    let mut plain = Song::new_default();
    plain.session.loop_on = false;
    let v: serde_json::Value = serde_json::from_slice(&encode(&plain)).unwrap();
    assert_eq!(v["song"]["headRamp"], false);
    for field in ["tempoPoints", "meterChanges", "markers"] {
        assert_eq!(v["song"][field], serde_json::json!([]), "{field}");
    }
    assert!(v["song"]["loop"].is_null());
    assert_eq!(decode(&encode(&plain)).unwrap(), plain);
    // The fields missing altogether read as a song with none of it.
    let mut v = v;
    for field in ["headRamp", "tempoPoints", "meterChanges", "markers", "loop"] {
        v["song"].as_object_mut().unwrap().remove(field);
    }
    assert_eq!(read_json(&v), plain);
}

#[test]
fn timelines_and_markers_that_cannot_be_do_not_read() {
    let good = rich_json();
    let with = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        f(&mut v["song"]);
        decode(&serde_json::to_vec(&v).unwrap())
    };
    let point =
        |tick: u64, tempo: u32| serde_json::json!({"tick": tick, "tempo": tempo, "ramp": false});
    let change = |bar: u32, num: u8, den: u8| serde_json::json!({"bar": bar, "meter": [num, den]});
    let marker = |id: u32, tick: u64| serde_json::json!({"id": id, "tick": tick, "name": "x"});
    type Change = Box<dyn Fn(&mut serde_json::Value)>;
    let broken: Vec<Change> = vec![
        // The head is not a point; points in order, one at a tick; a tempo in range.
        Box::new(move |s| s["tempoPoints"][0] = point(0, 10_000)),
        Box::new(move |s| s["tempoPoints"][0] = point(12 * Q + Q / 2, 10_000)),
        Box::new(move |s| s["tempoPoints"][0] = point(12 * Q + Q / 2 + 1000, 10_000)),
        Box::new(move |s| s["tempoPoints"][1] = point(12 * Q + Q / 2, 2_999)),
        Box::new(move |s| s["tempoPoints"][1] = point(12 * Q + Q / 2, 30_001)),
        Box::new(move |s| s["tempoPoints"][1] = point(MAX_TICK + 1, 10_000)),
        Box::new(move |s| s["tempoPoints"][1] = serde_json::json!({"tick": 12 * Q + Q / 2})),
        // Bar 0 is not a change; changes in order, one at a bar; a meter that is one.
        Box::new(move |s| s["meterChanges"][0] = change(0, 3, 4)),
        Box::new(move |s| {
            s["meterChanges"] = serde_json::json!([change(3, 3, 4), change(3, 4, 4)])
        }),
        Box::new(move |s| {
            s["meterChanges"] = serde_json::json!([change(5, 3, 4), change(3, 4, 4)])
        }),
        Box::new(move |s| s["meterChanges"][0] = change(3, 13, 4)),
        Box::new(move |s| s["meterChanges"][0] = change(3, 4, 3)),
        // A marker's id is its own: below `nextId`, and no clip's, take's or other marker's.
        Box::new(move |s| s["markers"][0] = marker(8, 0)),
        Box::new(move |s| s["markers"][0] = marker(1, 0)),
        Box::new(move |s| s["markers"][0] = marker(2, 0)),
        Box::new(move |s| s["markers"] = serde_json::json!([marker(4, 0), marker(4, Q)])),
        Box::new(move |s| s["markers"][0] = marker(4, MAX_TICK + 1)),
        // The loop is never empty.
        Box::new(|s| s["loop"] = serde_json::json!({"start": Q, "end": Q})),
        Box::new(|s| s["loop"] = serde_json::json!({"start": 2 * Q, "end": Q})),
    ];
    for (i, f) in broken.iter().enumerate() {
        assert!(
            matches!(with(f.as_ref()), Err(ReadError::Broken)),
            "case {i}"
        );
    }
    // What is not against a rule reads: a point between beats, markers in any order and at
    // one tick, a marker with no name.
    let song = with(&|s| {
        s["tempoPoints"][1] = point(12 * Q + Q / 2 + 1, 30_000);
        s["nextId"] = 20.into();
        s["markers"] = serde_json::json!([marker(15, 2 * Q), marker(4, 2 * Q), marker(14, 0)]);
        s["markers"][2]["name"] = "".into();
    })
    .unwrap();
    assert_eq!(song.doc.timeline.tempo_points[1].tick, 12 * Q + Q / 2 + 1);
    assert_eq!(
        (song.doc.markers.iter())
            .map(|m| (m.id.0, m.tick))
            .collect::<Vec<_>>(),
        [(14, 0), (4, 2 * Q), (15, 2 * Q)]
    );
}

/// Gives the inserts the ids a file that has none is read with: from 8, the `next_id` of
/// the songs of these tests before their inserts, in the order of the strips.
fn number_inserts(doc: &mut Doc) {
    doc.next_id = 8;
    renumber_inserts(doc);
}

/// The returns and the master, leaving out which ids their inserts have.
fn mixer_without_ids(doc: &Doc) -> ([Arc<Return>; 2], Arc<Channel>) {
    let mut doc = Doc {
        tracks: Vec::new(),
        ..doc.clone()
    };
    for insert in inserts_mut(&mut doc) {
        insert.id = NO_INSERT_ID;
    }
    (doc.returns, doc.master)
}

#[test]
fn inserts_keep_their_ids_and_get_one_when_the_file_has_none() {
    const PLACES: [&str; 6] = [
        "/tracks/0/inserts/1",
        "/tracks/0/inserts/3",
        "/mixer/returns/a/inserts/0",
        "/mixer/returns/b/inserts/2",
        "/mixer/master/inserts/0",
        "/mixer/master/inserts/1",
    ];
    let good: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 1))).unwrap();
    let read = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        f(&mut v["song"]);
        decode(&serde_json::to_vec(&v).unwrap())
    };
    let ids =
        |song: &Song| -> Vec<u32> { inserts_mut(&mut song.doc.clone()).map(|i| i.id.0).collect() };
    let set = |s: &mut serde_json::Value, place: &str, id: u32| {
        s.pointer_mut(place).unwrap()["id"] = id.into();
    };
    let forget = |s: &mut serde_json::Value, place: &str| {
        let insert = s.pointer_mut(place).unwrap().as_object_mut().unwrap();
        assert!(insert.remove("id").is_some(), "{place} had an id");
    };

    let as_written = read(&|_| {}).unwrap();
    assert_eq!(ids(&as_written), [8, 9, 10, 11, 12, 13]);
    assert_eq!(as_written.doc.next_id, 14);

    // A file from before inserts had ids: given from `nextId`, in the order of the strips.
    let none = read(&|s| {
        s["nextId"] = 8.into();
        PLACES.iter().for_each(|place| forget(s, place));
    })
    .unwrap();
    assert_eq!(none, as_written);
    // Only the ones without an id get one; the others keep theirs.
    let some = read(&|s| {
        forget(s, PLACES[1]);
        forget(s, PLACES[4]);
    })
    .unwrap();
    assert_eq!(ids(&some), [8, 14, 10, 11, 15, 13]);
    assert_eq!(some.doc.next_id, 16);

    let broken: [&dyn Fn(&mut serde_json::Value); 4] = [
        // Two inserts with one id.
        &|s| set(s, PLACES[3], 8),
        // Not below `nextId`.
        &|s| set(s, PLACES[0], 14),
        // A clip's id, a marker's id.
        &|s| set(s, PLACES[2], 1),
        &|s| set(s, PLACES[5], 4),
    ];
    for (i, f) in broken.iter().enumerate() {
        assert!(matches!(read(*f), Err(ReadError::Broken)), "case {i}");
    }
}
