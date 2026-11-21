//! Measures how long audio files take to come into a song, and makes songs of vocal tracks
//! for the measuring tools (docs/measurements/2026-11-21-round7-step3.md). `#[ignore]`: it
//! runs only when asked, with the files named in the environment. No device.
//!
//! - `MELOET_AMEASURE_FILES`: the files, `;` between them. Each goes on a vocal track of its
//!   own, at the head of the song.
//! - `MELOET_AMEASURE_OPEN`: a song that is opened first (and saved where it is, with what
//!   came in).
//! - `MELOET_AMEASURE_OUT`: a folder that stays. `result.json` there says how long each file
//!   took (the clip is there; it is ready to sound; its peaks are whole), the RSS of this
//!   process all the while, the save, the save under another name and the opening again.
//!   `songs/はじめての曲` is the saved song, `elsewhere/別の曲` its copy.

mod plughost;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meloet_core::{Core, CoreOptions, ShellRequest};
use meloet_protocol::{AppState, AudioClipState, AudioClipView, Command, Peaks, SongFile};

use plughost::wait_for;

const PATIENCE: Duration = Duration::from_secs(900);
const AUDIO: &str = "音";

#[derive(Default)]
struct Windows {
    audio: Vec<PathBuf>,
    song: Option<PathBuf>,
    save_as: Option<PathBuf>,
}

type Pick = Arc<Mutex<Windows>>;

fn start(root: &Path) -> (Arc<Core>, Pick) {
    let core = Arc::new(Core::start(CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
        state_dir: Some(root.join("state")),
        cache_dir: Some(root.join("cache")),
        ..CoreOptions::offline()
    }));
    let pick = Pick::default();
    let (weak, answer) = (Arc::downgrade(&core), pick.clone());
    core.set_shell(Box::new(move |request| {
        let Some(core) = weak.upgrade() else {
            return;
        };
        let mut windows = answer.lock().unwrap();
        match request {
            ShellRequest::PickAudio { .. } => core.audio_picked(std::mem::take(&mut windows.audio)),
            ShellRequest::PickSongFile { .. } => core.song_file_picked(windows.song.take()),
            ShellRequest::PickSaveAs { .. } => core.save_as_picked(windows.save_as.take()),
            _ => {}
        }
    }));
    (core, pick)
}

fn audio_clips(state: &AppState) -> Vec<AudioClipView> {
    (state.song.tracks.iter())
        .flat_map(|t| t.clips.iter().filter_map(|c| c.audio.clone()))
        .collect()
}

fn state_when(core: &Core, what: &str, done: impl Fn(&AppState) -> bool) -> AppState {
    wait_for(PATIENCE, || Some(core.snapshot().state).filter(|s| done(s)))
        .unwrap_or_else(|| panic!("{what}: {:#?}", core.snapshot().state.status))
}

fn rss_mb() -> f64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: f64 = (statm.split_whitespace().nth(1))
        .and_then(|p| p.parse().ok())
        .unwrap_or(0.0);
    pages * 4096.0 / 1_048_576.0
}

fn ms(from: Instant) -> f64 {
    (from.elapsed().as_secs_f64() * 1e4).round() / 10.0
}

fn same_bytes(a: &Path, b: &Path) -> bool {
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[test]
#[ignore = "a measuring tool: MELOET_AMEASURE_FILES and MELOET_AMEASURE_OUT"]
fn audio_files_come_in_are_saved_and_come_back() {
    let files: Vec<PathBuf> = std::env::var("MELOET_AMEASURE_FILES")
        .expect("MELOET_AMEASURE_FILES")
        .split(';')
        .filter(|f| !f.is_empty())
        .map(PathBuf::from)
        .collect();
    let root = PathBuf::from(std::env::var("MELOET_AMEASURE_OUT").expect("MELOET_AMEASURE_OUT"));
    for dir in ["config", "songs", "state", "cache", "elsewhere"] {
        let _ = std::fs::remove_dir_all(root.join(dir));
    }
    std::fs::create_dir_all(&root).unwrap();

    // The RSS of this process every 20 ms, with the time since the start.
    let began = Instant::now();
    let rss = Arc::new(Mutex::new(Vec::<(f64, f64)>::new()));
    let watching = Arc::new(AtomicBool::new(true));
    let watcher = {
        let (rss, watching) = (rss.clone(), watching.clone());
        std::thread::spawn(move || {
            while watching.load(Ordering::Relaxed) {
                rss.lock().unwrap().push((ms(began), rss_mb()));
                std::thread::sleep(Duration::from_millis(20));
            }
        })
    };
    let most_since = |from_ms: f64| {
        (rss.lock().unwrap().iter())
            .filter(|(t, _)| *t >= from_ms)
            .map(|(_, mb)| *mb)
            .fold(0.0, f64::max)
    };

    let (core, pick) = start(&root);
    let rss_at_start = rss_mb();
    let opened = std::env::var("MELOET_AMEASURE_OPEN")
        .ok()
        .map(PathBuf::from);
    if let Some(song) = &opened {
        pick.lock().unwrap().song = Some(song.clone());
        let serial = core.snapshot().state.song.serial;
        core.send(Command::OpenSong);
        state_when(&core, "the song is open", |s| s.song.serial != serial);
    }
    let before = audio_clips(&core.snapshot().state).len();
    let mut rows = Vec::new();
    for (i, file) in files.iter().enumerate() {
        let from_ms = ms(began);
        let sent = Instant::now();
        pick.lock().unwrap().audio = vec![file.clone()];
        core.send(Command::ImportAudio { track: None });
        let state = state_when(&core, "the clip is there", |s| {
            audio_clips(s).len() == before + i + 1
        });
        let clip_ms = ms(sent);
        let state = if audio_clips(&state)
            .iter()
            .all(|a| a.state == AudioClipState::Ready)
        {
            state
        } else {
            state_when(&core, "the audio is ready", |s| {
                audio_clips(s)
                    .iter()
                    .all(|a| a.state == AudioClipState::Ready)
            })
        };
        let ready_ms = ms(sent);
        let audio = audio_clips(&state)
            .into_iter()
            .max_by_key(|a| a.audio)
            .unwrap();
        wait_for(PATIENCE, || {
            let bytes = core.peaks(audio.audio, 0, 0, 16)?;
            Some(Peaks::decode(&bytes).unwrap()).filter(|p| !p.growing)
        })
        .expect("the peaks are made");
        let peaks_ms = ms(sent);
        rows.push(serde_json::json!({
            "file": file.file_name().unwrap().to_string_lossy(),
            "bytes": std::fs::metadata(file).map(|m| m.len()).unwrap_or(0),
            "as": audio.file, "frames": audio.frames, "rate": audio.sample_rate,
            "bits": audio.bits, "channels": audio.channels, "resampled": audio.resampled,
            "clipThereMs": clip_ms, "readyMs": ready_ms, "peaksMs": peaks_ms,
            "rssMostMb": most_since(from_ms), "rssAfterMb": rss_mb(),
            "tracks": state.song.tracks.len(),
        }));
    }

    let sent = Instant::now();
    core.send(Command::SaveSong);
    let state = state_when(&core, "the song is saved", |s| {
        s.song.file == SongFile::Saved
    });
    let save_ms = ms(sent);
    let song_dir = match &opened {
        Some(song) => song.parent().unwrap().to_owned(),
        None => root.join("songs").join(&state.song.name),
    };
    let saved: Vec<String> = (audio_clips(&state).into_iter().skip(before))
        .map(|a| a.file)
        .collect();
    let same_saved: Vec<bool> = (files.iter().zip(&saved))
        .map(|(from, name)| same_bytes(from, &song_dir.join(AUDIO).join(name)))
        .collect();

    let sent = Instant::now();
    pick.lock().unwrap().save_as = Some(root.join("elsewhere/別の曲"));
    core.send(Command::SaveSongAs);
    state_when(&core, "the song is saved under another name", |s| {
        s.song.name == "別の曲" && s.song.file == SongFile::Saved
    });
    let save_as_ms = ms(sent);
    let copy_dir = root.join("elsewhere/別の曲");
    let same_copy: Vec<bool> = (files.iter().zip(&saved))
        .map(|(from, name)| same_bytes(from, &copy_dir.join(AUDIO).join(name)))
        .collect();
    let same_still: Vec<bool> = (files.iter().zip(&saved))
        .map(|(from, name)| same_bytes(from, &song_dir.join(AUDIO).join(name)))
        .collect();
    // Every file of the first folder, what was there before this run too.
    let folder: Vec<(String, bool)> = std::fs::read_dir(song_dir.join(AUDIO))
        .map(|d| {
            d.flatten()
                .map(|e| {
                    let same = same_bytes(&e.path(), &copy_dir.join(AUDIO).join(e.file_name()));
                    (e.file_name().to_string_lossy().into_owned(), same)
                })
                .collect()
        })
        .unwrap_or_default();
    core.stop();
    drop(core);

    // Another run: the copy is opened, and everything is found (what was decoded is kept).
    let (core, pick) = start(&root);
    let sent = Instant::now();
    pick.lock().unwrap().song = Some(copy_dir.join("別の曲.meloet"));
    let serial = core.snapshot().state.song.serial;
    core.send(Command::OpenSong);
    let state = state_when(&core, "the song is open", |s| s.song.serial != serial);
    let open_ms = ms(sent);
    let again: Vec<(String, AudioClipState)> = (audio_clips(&state).into_iter())
        .map(|a| (a.file, a.state))
        .collect();
    state_when(&core, "every file is ready after the opening", |s| {
        audio_clips(s)
            .iter()
            .all(|a| a.state == AudioClipState::Ready)
    });
    let open_ready_ms = ms(sent);
    core.stop();
    drop(core);

    watching.store(false, Ordering::Relaxed);
    watcher.join().unwrap();
    let work_left: Vec<String> = std::fs::read_dir(root.join("state/work").join(AUDIO))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let result = serde_json::json!({
        "rssAtStartMb": rss_at_start,
        "files": rows,
        "saveMs": save_ms, "saveAsMs": save_as_ms,
        "openMs": open_ms, "openReadyMs": open_ready_ms,
        "names": saved,
        "sameBytesAfterSave": same_saved, "sameBytesInCopy": same_copy,
        "sameBytesInFirstFolderAfterCopy": same_still,
        "statesRightAfterOpening": again.iter().map(|(f, s)| format!("{f}: {s:?}")).collect::<Vec<_>>(),
        "workFolderAtTheEnd": work_left,
        "everyFileOfTheFolderInTheCopy": folder,
        "song": song_dir.join(format!("{}.meloet", state_name(&song_dir))),
        "copy": copy_dir.join("別の曲.meloet"),
        "rss": *rss.lock().unwrap(),
    });
    std::fs::write(
        root.join("result.json"),
        serde_json::to_string(&result).unwrap(),
    )
    .unwrap();
    assert!(
        same_saved
            .iter()
            .chain(&same_copy)
            .chain(&same_still)
            .all(|s| *s)
    );
}

fn state_name(song_dir: &Path) -> String {
    song_dir.file_name().unwrap().to_string_lossy().into_owned()
}
