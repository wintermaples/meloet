use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::*;
use crate::settings::temp_dir;

/// A song with two clips, a clip of three takes (one shared with the other clip), CC and bend
/// at the same tick, a loop and no armed track.
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
            ],
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
    track.gain_db = -3.5;
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
            play_target: TrackId(1),
            armed: None,
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
    assert_eq!(v["version"], 2);
    assert_eq!(v["song"]["tempo"], 12_345);
    assert_eq!(v["song"]["meter"], serde_json::json!([6, 8]));
    assert_eq!(v["song"]["nextId"], 8);
    let track = &v["song"]["tracks"][0];
    assert_eq!(
        track["instrument"],
        serde_json::json!({"kind": "placeholder"})
    );
    assert_eq!(track["plugins"], serde_json::json!([]));
    assert_eq!(track["automation"], serde_json::json!([]));
    assert_eq!(v["song"]["audio"], serde_json::json!([]));
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
        serde_json::json!({"playTarget": 1, "armed": null,
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
fn newer_and_broken_files_do_not_read() {
    let good: serde_json::Value = serde_json::from_slice(&encode(&rich_song("x", 3))).unwrap();
    let with = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        f(&mut v);
        decode(&serde_json::to_vec(&v).unwrap())
    };
    assert!(matches!(
        with(&|v| v["version"] = 3.into()),
        Err(ReadError::TooNew { version: 3 })
    ));
    assert!(matches!(
        decode(b"{\"format\": \"meloet-song\", \"vers"),
        Err(ReadError::Broken)
    ));
    assert!(matches!(decode(b""), Err(ReadError::Broken)));
    let broken: [&dyn Fn(&mut serde_json::Value); 9] = [
        &|v| v["format"] = "other".into(),
        &|v| v["song"]["tempo"] = 1.into(),
        &|v| v["song"]["meter"] = serde_json::json!([4, 3]),
        &|v| v["song"]["nextId"] = 3.into(),
        &|v| v["song"]["tracks"] = serde_json::json!([]),
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
        controls: Vec::new(),
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
        ],
        controls: Vec::new(),
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
    // Written again, it is version 2 and reads the same.
    let again = encode(&song);
    assert_eq!(decode(&again).unwrap(), song);
    let v: serde_json::Value = serde_json::from_slice(&again).unwrap();
    assert_eq!(v["version"], 2);
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
