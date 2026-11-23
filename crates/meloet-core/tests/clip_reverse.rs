//! 「ノーマライズ」 and 「逆にする」 of a vocal clip through the whole core (round 12, A9): the
//! thread that reads audio files measures and writes, the step is made when it answers, the
//! file the clip played is never changed, and a save keeps the backwards take. No device:
//! nothing sounds here.

mod plughost;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meloet_core::{Core, CoreOptions, ShellRequest};
use meloet_protocol::{AppState, ClipView, Command, SongAnswer, SongFile, StepName};

use plughost::{temp_dir, wait_for};

const PATIENCE: Duration = Duration::from_secs(60);
const RATE: u32 = 48_000;
/// Two seconds: a bar at 120 a minute.
const FRAMES: usize = 2 * RATE as usize;

#[derive(Default)]
struct Windows {
    audio: Vec<PathBuf>,
    song: Option<PathBuf>,
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

fn the_clip(state: &AppState) -> Option<ClipView> {
    (state.song.tracks.iter())
        .flat_map(|t| t.clips.iter().cloned())
        .next()
}

/// A wav of 24-bit samples, mono: a quiet sine with one loud sample.
fn write_wav(path: &Path, sample: impl Fn(usize) -> f32) {
    let data = FRAMES * 3;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 3).to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data as u32).to_le_bytes());
    for i in 0..FRAMES {
        let v = (sample(i) * 8_388_607.0).round() as i32;
        out.extend_from_slice(&v.to_le_bytes()[..3]);
    }
    std::fs::write(path, out).unwrap();
}

/// The samples of a 24-bit mono wav, as numbers.
fn samples(path: &Path) -> Vec<i32> {
    let bytes = std::fs::read(path).unwrap();
    (bytes[44..].as_chunks::<3>().0.iter())
        .map(|b| i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8)
        .collect()
}

#[test]
fn normalize_and_reverse_a_clip_and_keep_it_through_a_save() {
    let root = temp_dir("clip-reverse");
    let work = root.join("state/work/音");
    let from = root.join("from");
    std::fs::create_dir_all(&from).unwrap();
    let wav = from.join("声.wav");
    write_wav(&wav, |i| {
        if i == 30_000 {
            -0.4
        } else {
            0.1 * (i as f32 / 37.0).sin() * (i as f32 / FRAMES as f32)
        }
    });

    let (core, pick) = start(&root);
    core.send(Command::SetTempo { bpm: 120.0 });
    pick.lock().unwrap().audio = vec![wav.clone()];
    core.send(Command::ImportAudio { track: None });
    let state = state_when(&core, "the file is in", |s| the_clip(s).is_some());
    let first = the_clip(&state).unwrap();
    assert_eq!((first.start, first.end), (0, 7_680));
    let kept = work.join("声.wav");
    let original = std::fs::read(&kept).unwrap();
    let steps = state.history.current;

    // −0.4 is the loudest: −7.96 dBFS, so the volume is +6.96 dB.
    core.send(Command::NormalizeClips {
        clips: vec![first.id],
    });
    let state = state_when(&core, "the clip is normalized", |s| {
        s.history.current == steps + 1
    });
    let gain = f64::from(the_clip(&state).unwrap().audio.unwrap().gain_db);
    let loudest = samples(&kept)
        .iter()
        .map(|s| s.unsigned_abs())
        .max()
        .unwrap();
    let peak_db = 20.0 * (f64::from(loudest) / 8_388_608.0).log10();
    assert!((peak_db + gain + 1.0).abs() <= 0.1, "{peak_db} {gain}");
    assert_eq!(
        core.history_rows(steps + 1, steps + 1)[0].name,
        StepName::NormalizeClips
    );

    // Backwards: a new file and a new take in the same place; the volume stays.
    core.send(Command::ReverseClips {
        clips: vec![first.id],
    });
    let state = state_when(&core, "the clip is reversed", |s| {
        s.history.current == steps + 2
    });
    let once = the_clip(&state).unwrap();
    assert_ne!(once.take_id, first.take_id);
    assert_eq!(
        (once.id, once.start, once.end),
        (first.id, first.start, first.end)
    );
    let audio = once.audio.clone().unwrap();
    assert_eq!(f64::from(audio.gain_db), gain);
    assert_ne!(audio.file, "声.wav");
    let backwards = samples(&work.join(&audio.file));
    let mut want = samples(&kept);
    want.reverse();
    assert_eq!(backwards, want);
    assert_eq!(std::fs::read(&kept).unwrap(), original);

    // Twice: the samples of the file it all came from, in another file.
    core.send(Command::ReverseClips {
        clips: vec![first.id],
    });
    let state = state_when(&core, "the clip is reversed again", |s| {
        s.history.current == steps + 3
    });
    let twice = the_clip(&state).unwrap().audio.unwrap();
    assert_ne!(twice.file, audio.file);
    assert_eq!(samples(&work.join(&twice.file)), samples(&kept));
    assert_eq!(std::fs::read(&kept).unwrap(), original);

    // Undo goes back a step at a time; the first backwards take is the one shown again.
    core.send(Command::Undo);
    let state = state_when(&core, "one step back", |s| s.history.current == steps + 2);
    assert_eq!(the_clip(&state).unwrap().take_id, once.take_id);

    // Saved with the backwards take: its file goes into the song's folder, and the song
    // opened again shows it.
    core.send(Command::SaveSong);
    state_when(&core, "the song is saved", |s| {
        s.song.file == SongFile::Saved
    });
    let song_dir = root.join("songs/はじめての曲");
    let in_folder = song_dir.join("音").join(&audio.file);
    assert_eq!(samples(&in_folder), want);
    core.send(Command::NewSong);
    let asked = wait_for(PATIENCE, || {
        let state = core.snapshot().state;
        if let Some(dialog) = state.workspace.song_dialog {
            return Some(Some(dialog.id));
        }
        (state.song.tracks.len() == 1).then_some(None)
    })
    .expect("a new song");
    if let Some(dialog) = asked {
        core.send(Command::AnswerSongDialog {
            dialog,
            answer: SongAnswer::DontSave,
        });
    }
    state_when(&core, "the new song is there", |s| the_clip(s).is_none());
    pick.lock().unwrap().song = Some(song_dir.join("はじめての曲.meloet"));
    core.send(Command::OpenSong);
    let state = state_when(&core, "the song is open again", |s| the_clip(s).is_some());
    let opened = the_clip(&state).unwrap();
    let opened_audio = opened.audio.unwrap();
    assert_eq!(opened_audio.file, audio.file);
    assert_eq!(
        (opened_audio.first_frame, opened_audio.frames),
        (0, FRAMES as u64)
    );
    assert_eq!(f64::from(opened_audio.gain_db), gain);
    assert_eq!(
        (opened.start, opened.end, opened.take_offset),
        (0, 7_680, 0)
    );
    core.stop();
}
