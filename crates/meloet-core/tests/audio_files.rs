//! Audio files through the whole core (docs/architecture.md 22-4, 22-18 A2): brought in
//! from the PC's window, kept in the work place, moved into the song's folder by a save,
//! copied by a save under another name, found again when the song is opened, missing when
//! the file is gone, looked for, and taken out. No device: nothing sounds here.
//!
//! The mp3, ogg and flac files are in `tests/fixtures` (`make.sh` there says how they were
//! made); the wav is written here.

mod plughost;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meloet_core::{Core, CoreOptions, ShellRequest};
use meloet_protocol::{
    AppState, AudioClipState, ClipView, Command, MissingRef, Peaks, SelectMode, SelectedParams,
    SongAnswer, SongFile, StatusMessage,
};

use plughost::{temp_dir, wait_for};

const PATIENCE: Duration = Duration::from_secs(60);
const AUDIO: &str = "音";

/// What the PC's windows answer next.
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

fn state_when(core: &Core, what: &str, done: impl Fn(&AppState) -> bool) -> AppState {
    wait_for(PATIENCE, || Some(core.snapshot().state).filter(|s| done(s))).unwrap_or_else(|| {
        let state = core.snapshot().state;
        panic!("{what}: {:#?} {:#?}", state.status, state.song.tracks)
    })
}

fn clips(state: &AppState) -> Vec<ClipView> {
    (state.song.tracks.iter())
        .flat_map(|t| t.clips.iter().cloned())
        .collect()
}

fn states(state: &AppState) -> Vec<(String, AudioClipState)> {
    (clips(state).iter())
        .filter_map(|c| c.audio.as_ref().map(|a| (a.file.clone(), a.state)))
        .collect()
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// One second at 44100 Hz, 16 bits, mono: a 441 Hz sine.
fn write_wav(path: &Path) {
    let frames = 44_100u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + 2 * frames).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&44_100u32.to_le_bytes());
    out.extend_from_slice(&88_200u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(2 * frames).to_le_bytes());
    for i in 0..frames {
        let s = (std::f32::consts::TAU * 441.0 * i as f32 / 44_100.0).sin() * 0.5;
        out.extend_from_slice(&((s * 32_767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, out).unwrap();
}

/// 「音のファイルを入れる…」 with these files chosen; waits for their clips.
fn import(core: &Core, pick: &Pick, files: Vec<PathBuf>, count: usize) -> AppState {
    let before = clips(&core.snapshot().state).len();

    pick.lock().unwrap().audio = files;
    core.send(Command::ImportAudio { track: None });
    state_when(core, "the files are in the song", |s| {
        clips(s).len() == before + count
    })
}

fn save(core: &Core) -> AppState {
    core.send(Command::SaveSong);
    state_when(core, "the song is saved", |s| {
        s.song.file == SongFile::Saved
    })
}

fn open(core: &Core, pick: &Pick, song: &Path) -> AppState {
    pick.lock().unwrap().song = Some(song.to_owned());
    let serial = core.snapshot().state.song.serial;
    core.send(Command::OpenSong);
    state_when(core, "the song is open", |s| s.song.serial != serial)
}

#[test]
fn audio_goes_from_the_work_place_into_the_song_folder_and_comes_back() {
    let root = temp_dir("audio-files");
    let work = root.join("state/work").join(AUDIO);
    let from = root.join("from");
    std::fs::create_dir_all(&from).unwrap();
    let wav = from.join("ドラム.wav");
    write_wav(&wav);
    // A file nothing names, left in the work place by a run before: gone at the start.
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("古い.wav"), b"old").unwrap();

    let (core, pick) = start(&root);
    wait_for(PATIENCE, || names(&work).is_empty().then_some(()))
        .expect("the work place is emptied");

    // Brought in: copied into the work place as they are, one step, in a row on a new
    // vocal track from the play line.
    let state = import(
        &core,
        &pick,
        vec![wav.clone(), fixture("click.mp3"), from.join("ない.wav")],
        2,
    );
    assert_eq!(names(&work), ["click.mp3", "ドラム.wav"]);
    assert_eq!(
        std::fs::read(work.join("ドラム.wav")).unwrap(),
        std::fs::read(&wav).unwrap()
    );
    assert!(matches!(
        &state.status.message,
        Some(StatusMessage::AudioImported { file, count: 2, skipped: 1, tick: 0, .. }) if file == "click.mp3"
    ));
    assert_eq!(state.history.current, 1);
    let all = clips(&state);
    assert_eq!(
        (all[0].name.as_str(), all[1].name.as_str()),
        ("click", "ドラム")
    );
    assert_eq!(all[1].start, all[0].end);
    let (mp3, drum) = (all[0].audio.clone().unwrap(), all[1].audio.clone().unwrap());
    assert_eq!(
        (mp3.frames, mp3.sample_rate, mp3.bits, mp3.channels),
        (44_100, 44_100, None, 2)
    );
    assert_eq!(
        (drum.frames, drum.sample_rate, drum.bits, drum.channels),
        (44_100, 44_100, Some(16), 1)
    );
    // Their peaks are made: 173 of the finest level for a second at 44.1 kHz.
    for audio in [mp3.audio, drum.audio] {
        let peaks = wait_for(PATIENCE, || {
            let bytes = core.peaks(audio, 0, 0, 1_000)?;
            Some(Peaks::decode(&bytes).unwrap()).filter(|p| !p.growing)
        })
        .expect("the peaks are made");
        assert_eq!((peaks.total, peaks.count()), (173, 173));
    }
    assert_eq!(names(&root.join("cache/peaks")).len(), 2);

    // One more file, and its step undone: only the history names it from now on.
    import(&core, &pick, vec![fixture("click.flac")], 1);
    assert_eq!(names(&work), ["click.flac", "click.mp3", "ドラム.wav"]);
    core.send(Command::Undo);
    state_when(&core, "the import is undone", |s| clips(s).len() == 2);

    // The first save: what the song names moves into the song's folder, the rest stays.
    let state = save(&core);
    let song_dir = root.join("songs/はじめての曲");
    assert_eq!(names(&song_dir), ["はじめての曲.meloet", AUDIO]);
    assert_eq!(names(&song_dir.join(AUDIO)), ["click.mp3", "ドラム.wav"]);
    assert_eq!(names(&work), ["click.flac"]);
    assert!(
        states(&state)
            .iter()
            .all(|(_, state)| *state == AudioClipState::Ready)
    );
    // Done again after the save: the clip is back and its file is still found.
    core.send(Command::Redo);
    let state = state_when(&core, "the clip is back", |s| clips(s).len() == 3);
    assert!(
        states(&state)
            .iter()
            .all(|(_, state)| *state == AudioClipState::Ready)
    );

    // A save under another name: what the folder has is copied, what is new is moved; the
    // folder before is as it was.
    pick.lock().unwrap().save_as = Some(root.join("elsewhere/別の曲"));
    core.send(Command::SaveSongAs);
    let state = state_when(&core, "the song is saved under another name", |s| {
        s.song.name == "別の曲" && s.song.file == SongFile::Saved
    });
    let copy_dir = root.join("elsewhere/別の曲");
    assert_eq!(names(&copy_dir), ["別の曲.meloet", AUDIO]);
    assert_eq!(
        names(&copy_dir.join(AUDIO)),
        ["click.flac", "click.mp3", "ドラム.wav"]
    );
    assert_eq!(names(&song_dir.join(AUDIO)), ["click.mp3", "ドラム.wav"]);
    assert!(names(&work).is_empty());
    assert!(
        states(&state)
            .iter()
            .all(|(_, state)| *state == AudioClipState::Ready)
    );

    // A new song without saving: what was brought in for the old one is nobody's.
    import(&core, &pick, vec![fixture("click.ogg")], 1);
    assert_eq!(names(&work), ["click.ogg"]);
    core.send(Command::NewSong);
    let asked = state_when(&core, "the song asks before it goes", |s| {
        s.workspace.song_dialog.is_some()
    });
    core.send(Command::AnswerSongDialog {
        dialog: asked.workspace.song_dialog.unwrap().id,
        answer: SongAnswer::DontSave,
    });
    state_when(&core, "the new song is there", |s| s.song.tracks.len() == 1);
    wait_for(PATIENCE, || names(&work).is_empty().then_some(()))
        .expect("the work place is emptied");
    assert_eq!(names(&copy_dir.join(AUDIO)).len(), 3);
    core.stop();
    drop(core);

    // Another run: the song is opened from its folder, and every file is found there.
    let (core, pick) = start(&root);
    let song = song_dir.join("はじめての曲.meloet");
    let state = open(&core, &pick, &song);
    assert_eq!(
        states(&state),
        [
            ("click.mp3".to_owned(), AudioClipState::Ready),
            ("ドラム.wav".to_owned(), AudioClipState::Ready)
        ]
    );
    assert!(!matches!(
        state.status.message,
        Some(StatusMessage::AudioMissing { .. })
    ));

    // A file is gone from the folder: said when the song is opened, and its clip is silent.
    std::fs::remove_file(song_dir.join(AUDIO).join("click.mp3")).unwrap();
    open(&core, &pick, &copy_dir.join("別の曲.meloet"));
    open(&core, &pick, &song);
    let state = state_when(&core, "the missing file is said", |s| {
        s.status.message == Some(StatusMessage::AudioMissing { count: 1 })
    });
    assert_eq!(
        states(&state),
        [
            ("click.mp3".to_owned(), AudioClipState::Missing),
            ("ドラム.wav".to_owned(), AudioClipState::Ready)
        ]
    );
    let missing = clips(&state)[0].clone();
    let audio = missing.audio.as_ref().unwrap().audio;
    core.send(Command::SelectClips {
        clips: vec![missing.id],
        mode: SelectMode::Replace,
    });
    let state = state_when(&core, "the panel shows the clip", |s| {
        s.params.selected.is_some()
    });
    let Some(SelectedParams::Clip(selected)) = state.params.selected else {
        panic!("not the clip");
    };
    let notice = selected.missing.expect("the notice of the missing file");
    assert_eq!(
        (notice.audio, notice.file.as_str(), notice.was_at.as_deref()),
        (audio, "click.mp3", Some("はじめての曲/音/click.mp3"))
    );

    // 「さがす…」 with a file that is not that audio, then with the one that is.
    pick.lock().unwrap().audio = vec![wav.clone()];
    core.send(Command::FindAudio { audio });
    state_when(
        &core,
        "the wrong file is said",
        |s| matches!(&s.status.message, Some(StatusMessage::AudioUnreadable { file }) if file == "ドラム.wav"),
    );
    pick.lock().unwrap().audio = vec![copy_dir.join(AUDIO).join("click.mp3")];
    core.send(Command::FindAudio { audio });
    let state = state_when(&core, "the file is found", |s| {
        states(s)
            .iter()
            .all(|(_, state)| *state == AudioClipState::Ready)
    });
    // Not a step, and the song is as its file has it; the file waits in the work place
    // for the next save.
    assert_eq!((state.history.total, state.song.file), (0, SongFile::Saved));
    assert_eq!(names(&work), ["click.mp3"]);
    assert_eq!(names(&song_dir.join(AUDIO)), ["ドラム.wav"]);

    // 「曲から外す」 for the other file once it is gone: its clip goes, as one step.
    std::fs::remove_file(song_dir.join(AUDIO).join("ドラム.wav")).unwrap();
    open(&core, &pick, &copy_dir.join("別の曲.meloet"));
    // (Opening another song emptied the work place: the found file was never saved.)
    open(&core, &pick, &song);
    let state = state_when(&core, "both are missing", |s| {
        s.status.message == Some(StatusMessage::AudioMissing { count: 2 })
    });
    let drum = clips(&state)[1].audio.as_ref().unwrap().audio;
    core.send(Command::RemoveMissing {
        missing: MissingRef::Audio { audio: drum },
    });
    let state = state_when(&core, "the clip is taken out", |s| clips(s).len() == 1);
    assert_eq!(
        (state.history.total, state.song.file),
        (1, SongFile::Changed)
    );
    core.stop();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn audio_in_the_work_place_is_there_again_after_meloet_ended_without_a_word() {
    let root = temp_dir("audio-recover");
    let work = root.join("state/work").join(AUDIO);
    let (core, pick) = start(&root);
    import(&core, &pick, vec![fixture("click.ogg")], 1);
    // The recovery file is written 2 seconds after the change.
    wait_for(PATIENCE, || {
        root.join("state/recovery.json").is_file().then_some(())
    })
    .expect("the recovery file is written");
    // Ended without closing: neither the recovery file nor the work place is emptied.
    core.stop();
    drop(core);
    assert_eq!(names(&work), ["click.ogg"]);

    let (core, _pick) = start(&root);
    let state = state_when(&core, "the song is brought back", |s| !clips(s).is_empty());
    assert_eq!(names(&work), ["click.ogg"]);
    assert_eq!(
        states(&state),
        [("click.ogg".to_owned(), AudioClipState::Ready)]
    );
    core.stop();
    let _ = std::fs::remove_dir_all(&root);
}
