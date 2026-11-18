use std::fs;
use std::path::{Path, PathBuf};

use meloet_model::Song;
use meloet_protocol::{
    Command, SongAnswer, SongDialogKind, SongDialogView, SongFile, StatusMessage, TransportState,
};

use crate::play::tests::{cmd, event, off, on, ready, record_once, report, started, status};
use crate::play::{EngineOp, Heard, RecordReport};
use crate::service::{read_song, save_song};
use crate::settings::temp_dir;
use crate::songfile;
use crate::state::{Effect, Input, Model};

const BAR: u64 = 1920;

/// The disk, and the shell's answers.
struct World {
    songs: PathBuf,
    /// What the open window gives next; `None` puts it away.
    pick_file: Option<PathBuf>,
    pick_folder: Option<PathBuf>,
    /// What the save window gives next; `None` puts it away.
    pick_save_as: Option<PathBuf>,
    exited: bool,
    /// The PC's windows that were asked for.
    windows: Vec<&'static str>,
}

impl World {
    fn new(label: &str) -> World {
        World {
            songs: temp_dir(label),
            pick_file: None,
            pick_folder: None,
            pick_save_as: None,
            exited: false,
            windows: Vec::new(),
        }
    }

    /// Does the effects the way the core thread and the shell do, with what they answer.
    fn run(&mut self, m: &mut Model, effects: Vec<Effect>) -> Vec<Effect> {
        let mut rest = Vec::new();
        let mut queue = std::collections::VecDeque::from(effects);
        while let Some(effect) = queue.pop_front() {
            let input = match effect {
                Effect::SaveSong { song, to } => save_song(Some(&self.songs), &song, to),
                Effect::ReadSong { path, previous } => read_song(path, previous),
                Effect::PickSongFile => {
                    self.windows.push("file");
                    Input::SongFilePicked(self.pick_file.take())
                }
                Effect::PickFolder => {
                    self.windows.push("folder");
                    Input::FolderPicked(self.pick_folder.take())
                }
                Effect::PickSaveAs { name } => {
                    assert_eq!(name, m.view().song.name, "the song's name filled in");
                    self.windows.push("save as");
                    Input::SaveAsPicked(self.pick_save_as.take())
                }
                Effect::Exit => {
                    self.exited = true;
                    continue;
                }
                other => {
                    rest.push(other);
                    continue;
                }
            };
            queue.extend(m.handle(input));
        }
        rest
    }

    fn cmd(&mut self, m: &mut Model, command: Command) -> Vec<Effect> {
        let effects = cmd(m, command);
        self.run(m, effects)
    }

    fn input(&mut self, m: &mut Model, input: Input) -> Vec<Effect> {
        let effects = m.handle(input);
        self.run(m, effects)
    }

    fn answer(&mut self, m: &mut Model, answer: SongAnswer) -> Vec<Effect> {
        let dialog = dialog(m).expect("a song dialog is up").id;
        self.cmd(m, Command::AnswerSongDialog { dialog, answer })
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.songs);
    }
}

fn dialog(m: &Model) -> Option<SongDialogView> {
    m.view().workspace.song_dialog
}

fn kind(m: &Model) -> Option<SongDialogKind> {
    dialog(m).map(|d| d.kind)
}

fn file(m: &Model) -> SongFile {
    m.view().song.file
}

fn song(m: &Model) -> Song {
    m.play.song().clone()
}

/// Two recordings over a loop of bars 1–2 (two takes in one clip, then one more), a take
/// switch and a tempo: steps of every kind, and a session that is not the default one.
fn busy_song(m: &mut Model) {
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(
        m,
        Command::SetLoop {
            from_bar: 1,
            to_bar: 2,
        },
    );
    cmd(m, Command::Record);
    started(m);
    event(m, on(60), 10, 100.0, 0);
    event(m, off(60), 11, 200.0, 0);
    report(m, RecordReport::LapChanged { lap: 1 });
    event(m, on(64), 12, 300.0, 1);
    event(m, off(64), 13, 500.0, 1);
    cmd(m, Command::PlayStop);
    report(
        m,
        RecordReport::Stopped {
            t_us: 14,
            tick: 900.0,
            lap: 1,
        },
    );
    cmd(m, Command::ToggleLoop);
    cmd(m, Command::Locate { tick: 2 * BAR });
    record_once(
        m,
        &[(67, 2.0 * BAR as f64, 2.5 * BAR as f64)],
        3.0 * BAR as f64,
    );
    cmd(m, Command::SelectClip { clip: Some(1) });
    cmd(m, Command::StepTake { delta: -1 });
    cmd(m, Command::SetTempo { bpm: 120.5 });
    cmd(m, Command::ToggleArm { track: 1 });
}

#[test]
fn a_saved_song_opens_as_it_was() {
    let w = &mut World::new("flow-roundtrip");
    let m = &mut ready();
    busy_song(m);
    let before = song(m);
    assert_eq!(before.doc.tracks[0].clips.len(), 2);
    assert_eq!(before.doc.tracks[0].clips[0].takes.len(), 2);
    assert_eq!(before.session.armed, None);
    assert_eq!(file(m), SongFile::Changed);

    w.cmd(m, Command::SaveSong);
    let path = w.songs.join("はじめての曲/はじめての曲.meloet");
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(file(m), SongFile::Saved);

    w.cmd(m, Command::NewSong);
    assert_eq!(dialog(m), None, "nothing unsaved: no question");
    assert_eq!(song(m), Song::new_default());
    assert_eq!(file(m), SongFile::NeverSaved);
    assert!(matches!(status(m), Some(StatusMessage::NewSong { .. })));

    w.pick_file = Some(path.clone());
    let effects = w.cmd(m, Command::OpenSong);
    assert_eq!(song(m), before);
    assert_eq!(file(m), SongFile::Saved);
    assert_eq!(m.view().song.undo_count, 0);
    assert_eq!(m.song_path(), Some(path.as_path()));
    // The engine hears the whole song again.
    let sequences = effects
        .iter()
        .filter(|e| matches!(e, Effect::Engine(EngineOp::Sequence { .. })))
        .count();
    assert_eq!(sequences, 1);
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Engine(EngineOp::Tempo {
            bpm_centi: 12_050,
            ..
        })
    )));
}

#[test]
fn undo_and_redo_find_the_saved_place_again() {
    let w = &mut World::new("flow-undo");
    let m = &mut ready();
    busy_song(m);
    let steps = m.view().song.undo_count;
    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    w.cmd(m, Command::SaveSong);
    let saved = song(m);
    assert_eq!(file(m), SongFile::Saved);
    for _ in 0..3 {
        cmd(m, Command::Redo);
        assert_eq!(file(m), SongFile::Changed);
        cmd(m, Command::Redo);
        cmd(m, Command::Undo);
        cmd(m, Command::Undo);
        assert_eq!(file(m), SongFile::Saved);
        cmd(m, Command::Undo);
        assert_eq!(file(m), SongFile::Changed);
        cmd(m, Command::Redo);
        assert_eq!(file(m), SongFile::Saved);
        assert_eq!(song(m), saved);
    }
    assert_eq!(m.view().song.undo_count, steps - 2);
    assert_eq!(status(m), Some(StatusMessage::UndoPending { count: 2 }));
    cmd(m, Command::Redo);
    cmd(m, Command::Redo);
    assert_ne!(status(m), Some(StatusMessage::UndoPending { count: 0 }));
    assert!(!matches!(
        status(m),
        Some(StatusMessage::UndoPending { .. })
    ));
}

#[test]
fn a_taken_name_gets_a_number_and_undo_keeps_it() {
    let w = &mut World::new("flow-name");
    fs::create_dir_all(w.songs.join("はじめての曲")).unwrap();
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.cmd(m, Command::SaveSong);
    assert_eq!(m.view().song.name, "はじめての曲 2");
    assert_eq!(
        m.song_path(),
        Some(
            w.songs
                .join("はじめての曲 2/はじめての曲 2.meloet")
                .as_path()
        )
    );
    cmd(m, Command::Undo);
    assert_eq!(m.view().song.name, "はじめての曲 2");
    assert_eq!(file(m), SongFile::Changed);
    cmd(m, Command::Redo);
    assert_eq!(file(m), SongFile::Saved);
}

#[test]
fn save_as_makes_a_folder_where_picked_and_takes_its_name() {
    let w = &mut World::new("flow-save-as");
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.cmd(m, Command::SaveSong);
    let first = w.songs.join("はじめての曲/はじめての曲.meloet");
    assert_eq!(m.song_path(), Some(first.as_path()));

    let other = temp_dir("flow-save-as-other");
    w.pick_save_as = Some(other.join("夏の曲"));
    cmd(m, Command::SetTempo { bpm: 110.0 });
    w.cmd(m, Command::SaveSongAs);
    let path = other.join("夏の曲/夏の曲.meloet");
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(m.view().song.name, "夏の曲");
    assert_eq!(file(m), SongFile::Saved);
    assert_eq!(songfile::read(&path).unwrap(), song(m));
    assert_eq!(
        songfile::read(&first).unwrap().doc.tempo.centi(),
        10_000,
        "the first file stays as it was"
    );

    // Mod S from now on: the new file.
    cmd(m, Command::SetTempo { bpm: 120.0 });
    w.cmd(m, Command::SaveSong);
    assert_eq!(songfile::read(&path).unwrap().doc.tempo.centi(), 12_000);
    assert!(songfile::previous_path(&path).exists());

    // A name that is taken gets " 2", and so does the song; the extension typed is dropped.
    w.pick_save_as = Some(other.join("夏の曲.meloet"));
    w.cmd(m, Command::SaveSongAs);
    let second = other.join("夏の曲 2/夏の曲 2.meloet");
    assert_eq!(m.song_path(), Some(second.as_path()));
    assert_eq!(m.view().song.name, "夏の曲 2");
    assert_eq!(w.windows, ["save as", "save as"]);
    let _ = fs::remove_dir_all(other);
}

#[test]
fn save_as_put_away_changes_nothing_and_waits_while_recording() {
    let w = &mut World::new("flow-save-as-cancel");
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    let effects = w.cmd(m, Command::SaveSongAs);
    assert_eq!(w.windows, ["save as"]);
    assert!(effects.is_empty());
    assert_eq!(m.song_path(), None);
    assert_eq!(file(m), SongFile::Changed);
    assert_eq!(m.view().song.name, "はじめての曲");
    assert_eq!(fs::read_dir(&w.songs).unwrap().count(), 0);
    // The window is put away: Mod S works again.
    w.cmd(m, Command::SaveSong);
    assert_eq!(file(m), SongFile::Saved);

    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    w.cmd(m, Command::SaveSongAs);
    assert_eq!(w.windows, ["save as"], "not while recording");
}

#[cfg(unix)]
#[test]
fn save_as_that_cannot_be_written_keeps_the_name_for_the_folder_picked() {
    use std::os::unix::fs::PermissionsExt;
    let w = &mut World::new("flow-save-as-cannot");
    let locked = w.songs.join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::create_dir(locked.join("probe")).is_ok() {
        return; // root
    }
    let m = &mut ready();
    w.pick_save_as = Some(locked.join("冬の曲"));
    w.cmd(m, Command::SaveSongAs);
    assert_eq!(kind(m), Some(SongDialogKind::CannotSave));
    assert_eq!(m.view().song.name, "はじめての曲");
    w.pick_folder = Some(w.songs.clone());
    w.answer(m, SongAnswer::ChooseLocation);
    assert_eq!(
        m.song_path(),
        Some(w.songs.join("冬の曲/冬の曲.meloet").as_path())
    );
    assert_eq!(m.view().song.name, "冬の曲");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn mute_is_saved_and_solo_is_not() {
    let w = &mut World::new("flow-mute");
    let m = &mut ready();
    cmd(m, Command::ToggleMute { track: 1 });
    cmd(m, Command::ToggleSolo { track: 1 });
    w.cmd(m, Command::SaveSong);
    let path = w.songs.join("はじめての曲/はじめての曲.meloet");
    w.cmd(m, Command::NewSong);
    w.pick_file = Some(path);
    let effects = w.cmd(m, Command::OpenSong);
    let t = &m.view().song.tracks[0];
    assert!(t.muted);
    assert!(!t.soloed);
    assert_eq!(m.view().song.undo_count, 0);
    assert!(effects.contains(&Effect::Engine(EngineOp::SilentTracks { tracks: vec![0] })));
}

#[cfg(unix)]
#[test]
fn a_place_that_cannot_be_written_asks_for_another() {
    use std::os::unix::fs::PermissionsExt;
    let w = &mut World::new("flow-cannot");
    fs::set_permissions(&w.songs, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::create_dir(w.songs.join("probe")).is_ok() {
        return; // root
    }
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.cmd(m, Command::SaveSong);
    assert_eq!(kind(m), Some(SongDialogKind::CannotSave));
    assert_eq!(dialog(m).unwrap().title, "はじめての曲");
    assert_eq!(file(m), SongFile::Changed);

    // The folder window put away: the question again.
    w.answer(m, SongAnswer::ChooseLocation);
    assert_eq!(kind(m), Some(SongDialogKind::CannotSave));

    let other = temp_dir("flow-cannot-other");
    w.pick_folder = Some(other.clone());
    w.answer(m, SongAnswer::ChooseLocation);
    assert_eq!(w.windows, ["folder", "folder"]);
    assert_eq!(dialog(m), None);
    assert_eq!(file(m), SongFile::Saved);
    let path = other.join("はじめての曲/はじめての曲.meloet");
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(songfile::read(&path).unwrap(), song(m));
    fs::set_permissions(&w.songs, fs::Permissions::from_mode(0o755)).unwrap();
    let _ = fs::remove_dir_all(other);
}

fn write_two_versions(dir: &Path) -> PathBuf {
    let path = dir.join("曲/曲.meloet");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut song = Song::new_default();
    song.doc.name = "曲".into();
    songfile::save(&path, &song).unwrap();
    song.doc.tempo = meloet_model::Tempo::from_centi(14_000).unwrap();
    songfile::save(&path, &song).unwrap();
    path
}

#[test]
fn a_broken_file_offers_the_copy_before() {
    let w = &mut World::new("flow-broken");
    let path = write_two_versions(&w.songs);
    fs::write(&path, b"{\"format\": \"meloet-song\", \"version\": 1, \"so").unwrap();
    let m = &mut ready();
    w.pick_file = Some(path.clone());
    w.cmd(m, Command::OpenSong);
    let d = dialog(m).unwrap();
    assert_eq!(
        (d.kind, d.title.as_str(), d.previous),
        (SongDialogKind::Broken, "曲.meloet", true)
    );
    // A button this dialog does not have does nothing.
    w.answer(m, SongAnswer::Save);
    assert_eq!(dialog(m), Some(d));

    w.answer(m, SongAnswer::OpenPrevious);
    assert_eq!(dialog(m), None);
    assert_eq!(m.view().song.name, "曲");
    assert_eq!(m.view().song.tempo_bpm, 96.0, "the copy before");
    assert_eq!(file(m), SongFile::Changed, "the file still needs writing");
    cmd(m, Command::SetTempo { bpm: 97.0 });
    cmd(m, Command::Undo);
    assert_eq!(file(m), SongFile::Changed);

    w.cmd(m, Command::SaveSong);
    assert_eq!(file(m), SongFile::Saved);
    assert_eq!(songfile::read(&path).unwrap().doc.tempo.centi(), 9_600);
}

#[test]
fn a_broken_file_with_no_copy_before_only_says_so() {
    let w = &mut World::new("flow-broken-alone");
    let path = w.songs.join("x.meloet");
    fs::write(&path, b"[]").unwrap();
    let m = &mut ready();
    w.pick_file = Some(path);
    w.cmd(m, Command::OpenSong);
    let d = dialog(m).unwrap();
    assert_eq!((d.kind, d.previous), (SongDialogKind::Broken, false));
    w.answer(m, SongAnswer::OpenPrevious);
    assert_eq!(kind(m), Some(SongDialogKind::Broken), "no such button");
    cmd(m, Command::Back);
    assert_eq!(dialog(m), None);
    assert_eq!(song(m), Song::new_default());
}

#[test]
fn a_file_from_a_newer_meloet_counts_as_broken() {
    let w = &mut World::new("flow-newer");
    let path = write_two_versions(&w.songs);
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    v["version"] = 3.into();
    fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    let m = &mut ready();
    w.pick_file = Some(path);
    w.cmd(m, Command::OpenSong);
    let d = dialog(m).unwrap();
    assert_eq!((d.kind, d.previous), (SongDialogKind::Broken, true));
}

#[test]
fn opening_asks_first_and_cancel_keeps_everything() {
    let w = &mut World::new("flow-open-ask");
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.cmd(m, Command::OpenSong);
    let d = dialog(m).unwrap();
    assert_eq!(
        (d.kind, d.title.as_str()),
        (SongDialogKind::Unsaved, "はじめての曲")
    );
    assert!(w.windows.is_empty());
    // A stale answer, to a dialog that is gone.
    w.cmd(
        m,
        Command::AnswerSongDialog {
            dialog: d.id + 1,
            answer: SongAnswer::DontSave,
        },
    );
    assert_eq!(dialog(m), Some(d));
    w.answer(m, SongAnswer::Cancel);
    assert_eq!(dialog(m), None);
    assert!(w.windows.is_empty());
    assert_eq!(m.view().song.tempo_bpm, 100.0);

    // The open window put away: the song stays.
    w.cmd(m, Command::OpenSong);
    w.answer(m, SongAnswer::DontSave);
    assert_eq!(w.windows, ["file"]);
    assert_eq!(m.view().song.tempo_bpm, 100.0);
}

#[test]
fn a_new_song_asks_first() {
    let w = &mut World::new("flow-new");
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.cmd(m, Command::NewSong);
    assert_eq!(kind(m), Some(SongDialogKind::Unsaved));
    w.answer(m, SongAnswer::Save);
    assert_eq!(song(m), Song::new_default());
    assert_eq!(file(m), SongFile::NeverSaved);
    assert_eq!(m.song_path(), None);
    let saved = songfile::read(&w.songs.join("はじめての曲/はじめての曲.meloet")).unwrap();
    assert_eq!(saved.doc.tempo.centi(), 10_000);
    assert!(matches!(status(m), Some(StatusMessage::NewSong { .. })));
}

#[test]
fn closing_with_nothing_unsaved_ends_at_once() {
    let w = &mut World::new("flow-close-clean");
    let m = &mut ready();
    w.input(m, Input::CloseRequested);
    assert!(w.exited);
}

#[test]
fn quit_is_closing_the_window() {
    let w = &mut World::new("flow-quit-clean");
    let m = &mut ready();
    w.input(m, Input::Command(Command::Quit));
    assert!(w.exited);

    let w = &mut World::new("flow-quit");
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.input(m, Input::Command(Command::Quit));
    assert!(!w.exited);
    assert_eq!(kind(m), Some(SongDialogKind::Unsaved));
}

#[test]
fn closing_with_changes_asks() {
    for (answer, exits, saved) in [
        (SongAnswer::Save, true, true),
        (SongAnswer::DontSave, true, false),
        (SongAnswer::Cancel, false, false),
    ] {
        let w = &mut World::new("flow-close");
        let m = &mut ready();
        cmd(m, Command::SetTempo { bpm: 100.0 });
        w.input(m, Input::CloseRequested);
        assert!(!w.exited);
        assert_eq!(kind(m), Some(SongDialogKind::Unsaved));
        w.answer(m, answer);
        assert_eq!(w.exited, exits, "{answer:?}");
        assert_eq!(
            w.songs.join("はじめての曲/はじめての曲.meloet").exists(),
            saved,
            "{answer:?}"
        );
        if exits {
            // Ending: nothing more is taken.
            w.cmd(m, Command::SaveSong);
            w.input(m, Input::CloseRequested);
            assert_eq!(dialog(m), None);
        }
    }
}

#[cfg(unix)]
#[test]
fn closing_when_the_save_fails_does_not_end() {
    use std::os::unix::fs::PermissionsExt;
    let w = &mut World::new("flow-close-fail");
    fs::set_permissions(&w.songs, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::create_dir(w.songs.join("probe")).is_ok() {
        return;
    }
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.input(m, Input::CloseRequested);
    w.answer(m, SongAnswer::Save);
    assert_eq!(kind(m), Some(SongDialogKind::CannotSave));
    assert!(!w.exited);
    let other = temp_dir("flow-close-fail-other");
    w.pick_folder = Some(other.clone());
    w.answer(m, SongAnswer::ChooseLocation);
    assert!(w.exited, "saved elsewhere, then ended");
    assert!(other.join("はじめての曲/はじめての曲.meloet").exists());
    fs::set_permissions(&w.songs, fs::Permissions::from_mode(0o755)).unwrap();
    let _ = fs::remove_dir_all(other);
}

#[test]
fn closing_while_recording_stops_and_keeps_the_take_first() {
    let w = &mut World::new("flow-close-rec");
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    event(m, on(60), 10, 100.0, 0);
    // Song commands wait while recording.
    w.cmd(m, Command::SaveSong);
    w.cmd(m, Command::NewSong);
    assert!(!w.songs.join("はじめての曲").exists());
    assert_eq!(dialog(m), None);

    m.set_now(
        50,
        Some(Heard {
            tick: 400.0,
            state: TransportState::Recording,
            lap: 0,
        }),
    );
    let effects = w.input(m, Input::CloseRequested);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Engine(EngineOp::Stop { .. })))
    );
    assert_eq!(dialog(m), None, "not before the take is in");
    report(
        m,
        RecordReport::Stopped {
            t_us: 50,
            tick: 400.0,
            lap: 0,
        },
    );
    assert_eq!(m.view().song.tracks[0].clips.len(), 1);
    assert_eq!(kind(m), Some(SongDialogKind::Unsaved));
    w.answer(m, SongAnswer::Save);
    assert!(w.exited);
    let saved = songfile::read(&w.songs.join("はじめての曲/はじめての曲.meloet")).unwrap();
    assert_eq!(saved.doc.tracks[0].clips.len(), 1);
}
