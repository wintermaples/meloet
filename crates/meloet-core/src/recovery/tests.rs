use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_model::{Clip, ClipId, ClipTake, Doc, Note, Take, TakeId};

use super::*;
use crate::settings::temp_dir;

/// A song named `name` with one clip of one take of `notes` notes.
pub(crate) fn song_with_notes(name: &str, notes: u64) -> Song {
    let mut song = Song::new_default();
    let length = (notes * 120).max(1920);
    let take = Arc::new(Take {
        origin: None,
        id: TakeId(1),
        length,
        notes: (0..notes)
            .map(|i| Note {
                from: None,
                start: i * 120,
                length: 100,
                key: 36 + (i % 60) as u8,
                velocity: 1 + (i % 127) as u8,
            })
            .collect(),
        controls: Vec::new().into(),
    });
    let mut track = (*song.doc.tracks[0]).clone();
    track.clips = vec![Arc::new(Clip {
        id: ClipId(2),
        name: "ピアノ".into(),
        start: 0,
        end: length,
        takes: vec![ClipTake { take, offset: 0 }],
        active: 0,
    })];
    song.doc = Doc {
        name: name.into(),
        tracks: vec![Arc::new(track)],
        next_id: 3,
        ..song.doc
    };
    song
}

#[test]
fn a_song_comes_back_with_its_path_cause_and_time() {
    let song = song_with_notes("はじめての曲", 30);
    let path = Path::new("/home/x/ミュージック/はじめての曲/はじめての曲.meloet");
    let bytes = encode(
        &song,
        Some(path),
        RecoveryCause::PcEnding,
        1_794_750_060_000,
    );
    let back = decode(&bytes).unwrap();
    assert_eq!(back.song, song);
    assert_eq!(back.path.as_deref(), Some(path));
    assert_eq!(back.cause, RecoveryCause::PcEnding);
    assert_eq!(back.written_ms, 1_794_750_060_000);

    let never = decode(&encode(&song, None, RecoveryCause::Sudden, 1)).unwrap();
    assert_eq!((never.path, never.cause), (None, RecoveryCause::Sudden));
}

#[test]
fn the_shape_on_disk_wraps_a_song_file() {
    let song = song_with_notes("x", 2);
    let bytes = encode(&song, None, RecoveryCause::Sudden, 5);
    assert_eq!(bytes.last(), Some(&b'\n'));
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["format"], "meloet-recovery");
    assert_eq!(v["version"], 1);
    assert_eq!(v["writtenMs"], 5);
    assert_eq!(v["cause"], "sudden");
    assert_eq!(v["path"], serde_json::Value::Null);
    let file: serde_json::Value = serde_json::from_slice(&songfile::encode(&song)).unwrap();
    assert_eq!(v["song"], file);
}

#[test]
fn a_file_that_does_not_read_still_tells_the_song_file_when_its_head_reads() {
    let song = song_with_notes("x", 2);
    let path = Path::new("/m/x/x.meloet");
    let good = encode(&song, Some(path), RecoveryCause::Sudden, 1);
    let mut v: serde_json::Value = serde_json::from_slice(&good).unwrap();

    v["song"]["song"]["tempo"] = 1.into();
    let rule_broken = serde_json::to_vec(&v).unwrap();
    assert_eq!(
        decode(&rule_broken).unwrap_err().path.as_deref(),
        Some(path)
    );

    v["version"] = 2.into();
    let newer = serde_json::to_vec(&v).unwrap();
    assert_eq!(decode(&newer).unwrap_err().path.as_deref(), Some(path));

    assert_eq!(decode(&good[..good.len() / 2]).unwrap_err().path, None);
    assert_eq!(decode(b"").unwrap_err().path, None);
    let other = br#"{"format": "meloet-song", "version": 1, "path": "/a"}"#;
    assert_eq!(decode(other).unwrap_err().path, None);
}

#[test]
fn an_unreadable_file_is_kept_under_a_name_with_the_time() {
    let dir = temp_dir("recovery-keep");
    assert_eq!(find(&dir), None);
    fs::write(dir.join(FILE_NAME), b"{\"format\": \"meloet-rec").unwrap();
    let Some(Found::Unreadable { kept, unread }) = find(&dir) else {
        panic!("expected an unreadable file");
    };
    assert_eq!(unread.path, None);
    assert!(!dir.join(FILE_NAME).exists());
    let name = kept.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("recovery-unreadable-"), "{name}");
    assert_eq!(fs::read(&kept).unwrap(), b"{\"format\": \"meloet-rec");
    assert_eq!(find(&dir), None, "not found again");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_time_in_a_kept_name_is_utc() {
    assert_eq!(utc_stamp(0), "19700101-000000");
    // 2026-11-15 13:41:00 UTC (22:41 in Japan).
    assert_eq!(utc_stamp(1_794_750_060), "20261115-134100");
    assert_eq!(utc_stamp(951_782_400), "20000229-000000");
    let dir = temp_dir("recovery-keep-twice");
    for _ in 0..2 {
        fs::write(dir.join(FILE_NAME), b"x").unwrap();
        keep_unreadable(&dir, &dir.join(FILE_NAME), 1_794_750_060_000).unwrap();
    }
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "recovery-unreadable-20261115-134100 2.json",
            "recovery-unreadable-20261115-134100.json"
        ]
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_writer_does_the_jobs_in_order_and_tells_when_done() {
    let dir = temp_dir("recovery-writer");
    let writer = Writer::start(dir.clone());
    let (tx, rx) = mpsc::channel();
    writer.write(
        Box::new(song_with_notes("1", 3)),
        None,
        RecoveryCause::Sudden,
        Vec::new(),
    );
    writer.write(
        Box::new(song_with_notes("2", 3)),
        None,
        RecoveryCause::PcEnding,
        vec![tx.clone()],
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let back = decode(&fs::read(dir.join(FILE_NAME)).unwrap()).unwrap();
    assert_eq!(
        (back.song.doc.name.as_str(), back.cause),
        ("2", RecoveryCause::PcEnding)
    );

    writer.write(
        Box::new(song_with_notes("3", 3)),
        None,
        RecoveryCause::Sudden,
        Vec::new(),
    );
    writer.remove();
    drop(writer);
    assert!(!dir.join(FILE_NAME).exists(), "the remove came last");
    assert!(
        fs::read_dir(&dir).unwrap().next().is_none(),
        "no temporary file left"
    );
    let _ = fs::remove_dir_all(dir);
}

/// Prints how long a song of 10 000 notes takes to be written (encoded, synced, renamed).
#[test]
fn time_to_write_ten_thousand_notes() {
    let dir = temp_dir("recovery-time");
    let song = song_with_notes("1万", 10_000);
    let mut times = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        write(
            &dir,
            &encode(&song, None, RecoveryCause::PcEnding, now_ms()),
        )
        .unwrap();
        times.push(started.elapsed());
    }
    let size = fs::metadata(dir.join(FILE_NAME)).unwrap().len();
    eprintln!("10 000 notes: {size} bytes, written in {times:?}");
    assert_eq!(
        decode(&fs::read(dir.join(FILE_NAME)).unwrap())
            .unwrap()
            .song,
        song
    );
    let _ = fs::remove_dir_all(dir);
}

const CRASH_CHILD: &str = "MELOET_RECOVERY_CRASH_CHILD";
const CRASH_READY: &str = "MELOET-RECOVERY-CRASH-CHILD-READY";
const CRASH_NOTES: u64 = 4_000;
const CRASH_SONGS: u32 = 40;

/// The child of `a_write_killed_at_any_moment_leaves_a_whole_file`: writes songs "0", "1", …,
/// round and round, as the recovery file until it is killed. Does nothing in a plain test run.
#[test]
fn crash_child() {
    let Some(dir) = std::env::var_os(CRASH_CHILD).map(PathBuf::from) else {
        return;
    };
    let songs: Vec<Vec<u8>> = (0..CRASH_SONGS)
        .map(|n| {
            encode(
                &song_with_notes(&n.to_string(), CRASH_NOTES),
                None,
                RecoveryCause::Sudden,
                u64::from(n),
            )
        })
        .collect();
    for (n, bytes) in songs.iter().enumerate().cycle() {
        write(&dir, bytes).unwrap();
        if n == 1 {
            println!("\n{CRASH_READY}");
        }
    }
}

/// Kills a process in the middle of writing the recovery file, at many moments: each time the
/// file reads, and is one of the songs written.
#[test]
fn a_write_killed_at_any_moment_leaves_a_whole_file() {
    const ROUNDS: u32 = 60;
    let exe = std::env::current_exe().unwrap();
    let base = temp_dir("recovery-crash");
    let mut seed = u64::from(std::process::id()) | 1;
    let mut landed = std::collections::HashSet::new();
    let mut mid_write = 0;
    for round in 0..ROUNDS {
        let dir = base.join(round.to_string());
        fs::create_dir_all(&dir).unwrap();
        let mut child = Command::new(&exe)
            .args(["--exact", "recovery::tests::crash_child", "--nocapture"])
            .args(["--test-threads", "1"])
            .env(CRASH_CHILD, &dir)
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

        let back = decode(&fs::read(dir.join(FILE_NAME)).unwrap())
            .unwrap_or_else(|e| panic!("round {round}: {e:?}"));
        let n: u32 = back.song.doc.name.parse().unwrap();
        assert_eq!(
            back.song,
            song_with_notes(&n.to_string(), CRASH_NOTES),
            "round {round}"
        );
        assert_eq!(back.written_ms, u64::from(n), "round {round}");
        landed.insert(n);
        if dir.join(format!(".{FILE_NAME}.new")).exists() {
            mid_write += 1;
        }
    }
    eprintln!("{ROUNDS} kills, {mid_write} with a temporary file left, songs {landed:?}");
    assert!(landed.len() > 1, "{landed:?}");
    assert!(
        mid_write >= ROUNDS / 10,
        "only {mid_write} kills inside a write"
    );
    let _ = fs::remove_dir_all(base);
}
