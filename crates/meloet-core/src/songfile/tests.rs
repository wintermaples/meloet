use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use meloet_model::effects;

use super::*;
use crate::settings::temp_dir;

/// A song with two clips, a clip of three takes (one shared with the other clip), CC and bend
/// at the same tick and a loop.
fn rich_song(name: &str, notes_per_take: u64) -> Song {
    let take = |id: u32, base: u8| {
        Arc::new(Take {
            origin: None,
            id: TakeId(id),
            length: 1920 * 4,
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
                    tick: 240,
                    value: ControlValue::PitchBend(-8192),
                },
                ControlPoint {
                    tick: 240,
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
            takes: takes
                .into_iter()
                .map(|(take, offset)| ClipTake { take, offset })
                .collect(),
            active,
        })
    };
    let mut doc = Doc::new_default();
    doc.name = name.into();
    doc.tempo = Tempo::from_centi(12_345).unwrap();
    doc.meter = Meter::new(6, 8).unwrap();
    doc.next_id = 8;
    let mut track = (*doc.tracks[0]).clone();
    track.channel.gain = Gain::from_db(-3.5);
    track.channel.pan = Pan::new(-20);
    track.sends = [Gain::from_db(-15.0), Gain::from_db(-18.0)];
    let mut compressor = Insert::new(EffectKind::Compressor);
    compressor.set(effects::compressor::THRESHOLD, -24.5);
    let mut delay = Insert::new(EffectKind::Delay);
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
    b.channel.inserts = [None, None, Some(Insert::new(EffectKind::Chorus)), None];
    let master = Arc::make_mut(&mut doc.master);
    master.gain = Gain::SILENT;
    master.pan = Pan::new(-30);
    (master.inserts[0].as_mut().unwrap()).set(effects::limiter::CEILING, -1.0);
    master.inserts[1] = Some(Insert::new(EffectKind::Eq4));
    track.clips = vec![
        clip(1, 0, 1920 * 4, vec![(t2.clone(), 0)], 0),
        clip(
            3,
            1920 * 4,
            1920 * 8,
            vec![(t5, 0), (t6, 480), (t7, -480), (t2, 960)],
            1,
        ),
    ];
    doc.tracks = vec![Arc::new(track)];
    Song {
        doc,
        session: Session {
            play_target: Some(TrackId(1)),
            loop_range: TickRange::new(1920 * 4, 1920 * 8),
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
        &clips[0].takes[0].take,
        &clips[1].takes[3].take
    ));
}

#[test]
fn the_shape_on_disk() {
    let v: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 1))).unwrap();
    assert_eq!(v["format"], "meloet-song");
    assert_eq!(v["version"], 3);
    assert_eq!(v["song"]["tempo"], 12_345);
    assert_eq!(v["song"]["meter"], serde_json::json!([6, 8]));
    assert_eq!(v["song"]["nextId"], 8);
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
            {"effect": "compressor",
             "params": {"threshold": -24.5, "ratio": 4.0, "attack": 10.0, "release": 100.0,
                        "makeup": 0.0}},
            null,
            {"effect": "delay",
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
        serde_json::json!({"take": 6, "offset": 480})
    );
    assert_eq!(
        track["clips"][1]["takes"][2],
        serde_json::json!({"take": 7, "offset": -480})
    );
    let take = &v["takes"][0];
    assert_eq!(take["notes"][0], serde_json::json!([0, 30, 40, 1]));
    assert_eq!(
        take["controls"],
        serde_json::json!([[0, 64, 127], [240, "bend", -8192], [240, 1, 12]])
    );
    assert_eq!(
        v["session"],
        serde_json::json!({"playTarget": 1,
                           "loop": {"start": 7680, "end": 15360}, "loopOn": true,
                           "grid": {"division": 8, "triplet": true},
                           "scale": {"root": 2, "mode": "minor", "lock": true},
                           "quantize": {"division": 32, "triplet": true, "strength": 75,
                                        "swing": 10, "align": "startAndEnd"}})
    );
}

#[test]
fn piano_roll_settings_missing_or_not_understood_are_the_defaults() {
    let song = rich_song("x", 3);
    let mut v: serde_json::Value = serde_json::from_slice(&encode(&song)).unwrap();
    // As round 2 wrote it.
    let session = v["session"].as_object_mut().unwrap();
    session.remove("grid");
    session.remove("scale");
    session.remove("quantize");
    let back = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(back.session.roll, RollSettings::default());
    assert_eq!(back.doc, song.doc);
    assert_eq!(back.session.loop_range, song.session.loop_range);

    v["session"]["grid"] = serde_json::json!({"division": 7, "triplet": false});
    v["session"]["scale"] = serde_json::json!({"root": 4, "mode": "dorian", "lock": true});
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

#[test]
fn newer_and_broken_files_do_not_read() {
    let good: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 3))).unwrap();
    let with = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        f(&mut v);
        decode(&serde_json::to_vec(&v).unwrap())
    };
    assert!(matches!(
        with(&|v| v["version"] = 4.into()),
        Err(ReadError::TooNew { version: 4 })
    ));
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
        length: 1920,
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
        length: 1920,
        notes: vec![
            Note {
                start: 0,
                length: 50,
                key: 60,
                velocity: 90,
                from: OriginIndex::new(1),
            },
            Note {
                start: 480,
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
    clip.takes[active].take = quantized;
    track.clips[1] = Arc::new(clip);
    song.doc.tracks[0] = Arc::new(track);
    song.doc.next_id = 10;
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
        serde_json::json!([[0, 50, 60, 90, 1], [480, 50, 62, 90]])
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
fn a_file_of_version_1_reads() {
    let v1 = r#"{"format": "meloet-song", "version": 1, "app": "meloet 0.1.0",
 "song": {"name": "はじめての曲", "tempo": 9600, "meter": [4, 4], "tempoChanges": [], "nextId": 8,
   "tracks": [{"id": 1, "name": "ピアノ", "color": 1, "instrument": {"kind": "placeholder"}, "gainDb": 0.0,
     "clips": [{"id": 3, "name": "ピアノ", "start": 7680, "end": 15360, "active": 1,
                "takes": [{"take": 5, "offset": 0}, {"take": 6, "offset": 480}]}],
     "plugins": [], "automation": []}],
   "audio": []},
 "takes": [{"id": 5, "length": 7680, "notes": [[0, 30, 60, 100]],
            "controls": [[0, 64, 127], [240, "bend", -8192], [240, 1, 12]]},
           {"id": 6, "length": 960, "notes": [[0, 30, 62, 100], [480, 30, 64, 90]]}],
 "session": {"playTarget": 1, "armed": 1, "loop": {"start": 0, "end": 3840}, "loopOn": false}}
"#;
    let song = decode(v1.as_bytes()).unwrap();
    assert!(!song.doc.tracks[0].muted);
    assert_eq!(
        song.doc.tracks[0].instrument,
        InstrumentRef::SINE,
        "the placeholder sound is the sine"
    );
    assert_eq!(song.session.play_target, Some(TrackId(1)));
    // Nothing is sent, so it sounds as it did; the returns and the master are a new song's.
    assert_eq!(song.doc.tracks[0].sends, [Gain::SILENT; 2]);
    assert_eq!(song.doc.tracks[0].channel, Channel::default());
    assert_eq!(song.doc.returns, mixer::new_song_returns());
    assert_eq!(song.doc.master, mixer::new_song_master());
    // Version 2 as written before mute came: the same.
    let v2 = v1.replace(r#""version": 1"#, r#""version": 2"#);
    assert_eq!(decode(v2.as_bytes()).unwrap(), song);
    let clip = &song.doc.tracks[0].clips[0];
    let shown = &clip.active_take().take;
    assert_eq!(shown.id, TakeId(6));
    assert!(shown.origin.is_none());
    assert_eq!(
        shown
            .notes
            .iter()
            .map(|n| (n.start, n.key, n.from))
            .collect::<Vec<_>>(),
        [(0, 62, None), (480, 64, None)]
    );
    assert_eq!(clip.takes[0].take.controls.len(), 3);
    // Written again, it is version 3 and reads the same.
    let again = encode(&song);
    assert_eq!(decode(&again).unwrap(), song);
    let v: serde_json::Value = serde_json::from_slice(&again).unwrap();
    assert_eq!(v["version"], 3);
    assert_eq!(
        v["song"]["tracks"][0]["instrument"],
        serde_json::json!({"kind": "builtin", "name": "sine"})
    );
    assert!(v["session"].get("armed").is_none());
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
    assert_eq!(song.doc.tracks[0].clips[1].takes[2].offset, -20_000);
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
fn a_file_of_version_3_from_before_the_mixer_reads() {
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
    assert_eq!(song.doc.returns, mixer::new_song_returns());
    assert_eq!(song.doc.master, mixer::new_song_master());
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
    assert_eq!(compressor.kind(), EffectKind::Compressor);
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
    assert_eq!(b[0], Some(Insert::new(EffectKind::Reverb)));
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
    assert_eq!(song.doc.returns, mixer::new_song_returns());
    assert_eq!(song.doc.master, mixer::new_song_master());
    assert_eq!(song.doc.tracks, want.doc.tracks);
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
