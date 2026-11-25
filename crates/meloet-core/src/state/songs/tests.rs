use std::fs;
use std::path::{Path, PathBuf};

use meloet_model::{Song, TakeId};
use meloet_protocol::{
    Command, SongAnswer, SongDialogKind, SongDialogView, SongFile, StatusMessage, TransportState,
};

use crate::play::tests::{
    cmd, event, new_song_without_pack, off, on, ready, record_once, report, started, status,
};
use crate::play::{EngineOp, Heard, RecordReport};
use crate::service::{read_song, save_song};
use crate::settings::temp_dir;
use crate::songfile;
use crate::state::{Effect, Input, Model};

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

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
                Effect::SaveSong { song, to } => {
                    let store = crate::audiofiles::AudioStore::new(None);
                    save_song(Some(&self.songs), &song, to, &store, &mut |_| {})
                }
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
    report(
        m,
        RecordReport::LapChanged {
            lap: 1,
            frame: 0,
            file_frame: None,
        },
    );
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
}

#[test]
fn a_saved_song_opens_as_it_was() {
    let w = &mut World::new("flow-roundtrip");
    let m = &mut ready();
    busy_song(m);
    let before = song(m);
    assert_eq!(before.doc.tracks[0].clips.len(), 2);
    assert_eq!(before.doc.tracks[0].clips[0].takes.len(), 2);
    assert_eq!(file(m), SongFile::Changed);

    w.cmd(m, Command::SaveSong);
    let path = w.songs.join("はじめての曲/はじめての曲.meloet");
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(file(m), SongFile::Saved);

    w.cmd(m, Command::NewSong);
    assert_eq!(dialog(m), None, "nothing unsaved: no question");
    assert_eq!(song(m), new_song_without_pack());
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
        Effect::Engine(EngineOp::Timeline { timeline }) if timeline.tempo.centi() == 12_050
    )));
}

#[test]
fn a_deleted_take_is_not_saved() {
    let w = &mut World::new("flow-delete-take");
    let m = &mut ready();
    busy_song(m);
    let clip = &song(m).doc.tracks[0].clips[0];
    assert_eq!(clip.takes.len(), 2);
    let gone = clip.active_take().take.id;
    let kept = clip.keys()[1 - clip.active].take.id;
    cmd(m, Command::DeleteTake { clip: clip.id.0 });
    w.cmd(m, Command::SaveSong);
    let path = m.song_path().unwrap().to_owned();
    let text = std::fs::read_to_string(&path).unwrap();
    let saved = song(m);

    w.cmd(m, Command::NewSong);
    w.pick_file = Some(path);
    w.cmd(m, Command::OpenSong);
    assert_eq!(song(m), saved);
    let ids: Vec<_> = song(m).doc.tracks[0].clips[0]
        .keys()
        .iter()
        .map(|t| t.take.id)
        .collect();
    assert_eq!(ids, [kept]);
    let has = |id: TakeId| text.contains(&format!("\"id\":{}", id.0));
    assert!(has(kept) && !has(gone));
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
    assert!(matches!(status(m), Some(StatusMessage::Redone { .. })));
    // The next thing done puts the notice away.
    cmd(m, Command::ClearNoteSelection);
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
        songfile::read(&first).unwrap().doc.timeline.tempo.centi(),
        10_000,
        "the first file stays as it was"
    );

    // Mod S from now on: the new file.
    cmd(m, Command::SetTempo { bpm: 120.0 });
    w.cmd(m, Command::SaveSong);
    assert_eq!(
        songfile::read(&path).unwrap().doc.timeline.tempo.centi(),
        12_000
    );
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
    song.doc =
        meloet_model::edit::set_tempo(&song.doc, meloet_model::Tempo::from_centi(14_000).unwrap())
            .unwrap()
            .doc;
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
    assert_eq!(m.view().timeline.tempo_bpm, 96.0, "the copy before");
    assert_eq!(file(m), SongFile::Changed, "the file still needs writing");
    cmd(m, Command::SetTempo { bpm: 97.0 });
    cmd(m, Command::Undo);
    assert_eq!(file(m), SongFile::Changed);

    w.cmd(m, Command::SaveSong);
    assert_eq!(file(m), SongFile::Saved);
    assert_eq!(
        songfile::read(&path).unwrap().doc.timeline.tempo.centi(),
        9_600
    );
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
    v["version"] = (songfile::VERSION + 1).into();
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
    assert_eq!(m.view().timeline.tempo_bpm, 100.0);

    // The open window put away: the song stays.
    w.cmd(m, Command::OpenSong);
    w.answer(m, SongAnswer::DontSave);
    assert_eq!(w.windows, ["file"]);
    assert_eq!(m.view().timeline.tempo_bpm, 100.0);
}

#[test]
fn a_new_song_asks_first() {
    let w = &mut World::new("flow-new");
    let m = &mut ready();
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.cmd(m, Command::NewSong);
    assert_eq!(kind(m), Some(SongDialogKind::Unsaved));
    w.answer(m, SongAnswer::Save);
    assert_eq!(song(m), new_song_without_pack());
    assert_eq!(file(m), SongFile::NeverSaved);
    assert_eq!(m.song_path(), None);
    let saved = songfile::read(&w.songs.join("はじめての曲/はじめての曲.meloet")).unwrap();
    assert_eq!(saved.doc.timeline.tempo.centi(), 10_000);
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

/// Mod S on a changed song; the save is written by the caller with what is returned.
fn start_saving(m: &mut Model) -> Effect {
    cmd(m, Command::SetTempo { bpm: 100.0 });
    let effects = cmd(m, Command::SaveSong);
    let save = effects
        .into_iter()
        .find(|e| matches!(e, Effect::SaveSong { .. }));
    save.expect("a save")
}

fn written(w: &World, save: Effect) -> Input {
    let Effect::SaveSong { song, to } = save else {
        panic!("{save:?}");
    };
    let store = crate::audiofiles::AudioStore::new(None);
    save_song(Some(&w.songs), &song, to, &store, &mut |_| {})
}

#[test]
fn closing_while_a_save_is_written_closes_once_it_has_answered() {
    let w = &mut World::new("flow-close-saving");
    let m = &mut ready();
    let save = start_saving(m);
    w.input(m, Input::CloseRequested);
    w.cmd(m, Command::Quit);
    assert!(!w.exited);
    assert_eq!(dialog(m), None);
    let answer = written(w, save);
    w.input(m, answer);
    assert!(w.exited);

    // A change made while the save went on is asked about.
    let w = &mut World::new("flow-close-saving-changed");
    let m = &mut ready();
    let save = start_saving(m);
    w.input(m, Input::CloseRequested);
    cmd(m, Command::SetTempo { bpm: 110.0 });
    let answer = written(w, save);
    w.input(m, answer);
    assert!(!w.exited);
    assert_eq!(kind(m), Some(SongDialogKind::Unsaved));

    // A save that fails says so first, and the close is not kept past it.
    let w = &mut World::new("flow-close-saving-failed");
    let m = &mut ready();
    let _ = start_saving(m);
    w.input(m, Input::CloseRequested);
    w.input(m, Input::SongNotSaved);
    assert!(!w.exited);
    assert_eq!(kind(m), Some(SongDialogKind::CannotSave));
    w.answer(m, SongAnswer::Cancel);
    w.cmd(m, Command::SaveSong);
    assert_eq!(file(m), SongFile::Saved);
    assert!(!w.exited);
}

#[test]
fn a_keyboard_fader_step_still_waiting_goes_with_the_song_it_was_for() {
    let w = &mut World::new("flow-fader");
    let m = &mut ready();
    // 0 dB is at 80 % of the fader: passed, so it is heard.
    for (t_us, value) in [(0, 100), (10, 103), (20, 90)] {
        m.set_now(t_us, None);
        m.handle(Input::KeyboardFader { value });
    }
    assert_eq!(m.play().mix_previews().len(), 1);
    w.cmd(m, Command::NewSong);
    m.set_now(20 + 500_000, None);
    m.handle(Input::Tick);
    assert_eq!(m.view().song.undo_count, 0);
    assert_eq!(m.view().song.tracks[0].gain_db, Some(0.0));
    assert_eq!(file(m), SongFile::NeverSaved);
}

fn serial(m: &Model) -> u32 {
    m.view().song.serial
}

#[test]
fn the_serial_goes_up_with_each_song_put_in_and_not_with_undo() {
    let w = &mut World::new("flow-serial");
    let m = &mut ready();
    let first = serial(m);
    busy_song(m);
    cmd(m, Command::Undo);
    cmd(m, Command::Redo);
    w.cmd(m, Command::SaveSong);
    assert_eq!(serial(m), first, "steps, undo, redo and saving keep it");
    let path = m.song_path().unwrap().to_path_buf();

    w.cmd(m, Command::NewSong);
    assert_eq!(serial(m), first + 1);

    w.pick_file = Some(path);
    w.cmd(m, Command::OpenSong);
    assert_eq!(serial(m), first + 2);
    cmd(m, Command::SetTempo { bpm: 97.0 });
    cmd(m, Command::Undo);
    assert_eq!(serial(m), first + 2);

    let path = write_two_versions(&w.songs);
    fs::write(&path, b"{").unwrap();
    w.pick_file = Some(path);
    w.cmd(m, Command::OpenSong);
    assert_eq!(dialog(m).map(|d| d.previous), Some(true));
    assert_eq!(serial(m), first + 2, "a broken file puts nothing in");
    w.answer(m, SongAnswer::OpenPrevious);
    assert_eq!(serial(m), first + 3);
}

#[test]
fn a_save_that_puts_audio_in_place_says_how_far_it_is_until_it_is_over() {
    let m = &mut ready();
    busy_song(m);
    let effects = cmd(m, Command::SaveSong);
    assert!(effects.iter().any(|e| matches!(e, Effect::SaveSong { .. })));
    let saving = |done, done_bytes| Input::SavingAudio {
        done,
        total: 12,
        done_bytes,
        total_bytes: 240_000_000,
    };
    m.handle(saving(3, 80_000_000));
    assert_eq!(
        status(m),
        Some(StatusMessage::SavingAudio {
            done: 3,
            total: 12,
            done_bytes: 80_000_000,
            total_bytes: 240_000_000,
        })
    );
    let said = |m: &Model| matches!(status(m), Some(StatusMessage::SavingAudio { .. }));
    m.handle(Input::SongSaved {
        path: PathBuf::from("/songs/曲/曲.meloet"),
        name: "曲".into(),
    });
    assert!(!said(m));
    // A word that comes when no save goes on is not said.
    m.handle(saving(12, 240_000_000));
    assert!(!said(m));
}

#[test]
fn a_handed_file_waits_for_the_screen_and_opens_with_no_window() {
    let w = &mut World::new("flow-given");
    let path = write_two_versions(&w.songs);
    let m = &mut ready();
    w.input(m, Input::OpenGiven(path.clone()));
    assert_eq!(song(m), Song::new_default(), "not before the screen is up");
    assert_eq!(dialog(m), None);

    w.input(m, Input::ScreenUp);
    assert_eq!(m.view().song.name, "曲");
    assert_eq!(m.view().timeline.tempo_bpm, 140.0);
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(file(m), SongFile::Saved);
    assert!(
        w.windows.is_empty(),
        "the PC's open window is not asked for"
    );

    // The page loaded again: nothing is opened again.
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.input(m, Input::ScreenUp);
    assert_eq!(m.view().timeline.tempo_bpm, 100.0);
}

#[test]
fn a_handed_file_asks_first_about_unsaved_changes() {
    let w = &mut World::new("flow-given-ask");
    let path = write_two_versions(&w.songs);
    let m = &mut ready();
    w.input(m, Input::ScreenUp);
    cmd(m, Command::SetTempo { bpm: 100.0 });

    w.input(m, Input::OpenGiven(path.clone()));
    let d = dialog(m).unwrap();
    assert_eq!(
        (d.kind, d.title.as_str()),
        (SongDialogKind::Unsaved, "はじめての曲")
    );
    w.answer(m, SongAnswer::Cancel);
    assert_eq!(m.view().timeline.tempo_bpm, 100.0);
    assert_eq!(m.song_path(), None);

    w.input(m, Input::OpenGiven(path.clone()));
    w.answer(m, SongAnswer::Save);
    let saved = songfile::read(&w.songs.join("はじめての曲/はじめての曲.meloet")).unwrap();
    assert_eq!(saved.doc.timeline.tempo.centi(), 10_000, "saved first");
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(m.view().timeline.tempo_bpm, 140.0);
    assert!(w.windows.is_empty());
}

#[test]
fn a_handed_file_that_does_not_read_says_so_as_opening_does() {
    let w = &mut World::new("flow-given-broken");
    let m = &mut ready();
    w.input(m, Input::ScreenUp);
    let missing = w.songs.join("ない.meloet");
    w.input(m, Input::OpenGiven(missing));
    let d = dialog(m).unwrap();
    assert_eq!(
        (d.kind, d.title.as_str(), d.previous),
        (SongDialogKind::Broken, "ない.meloet", false)
    );

    let path = write_two_versions(&w.songs);
    fs::write(&path, b"{\"format\": \"meloet-song\", \"version\": 1, \"so").unwrap();
    w.input(m, Input::OpenGiven(path));
    let d = dialog(m).unwrap();
    assert_eq!((d.kind, d.previous), (SongDialogKind::Broken, true));
    assert_eq!(song(m), Song::new_default());
}

#[test]
fn the_song_open_now_is_left_as_it_is() {
    let w = &mut World::new("flow-given-same");
    let path = write_two_versions(&w.songs);
    let m = &mut ready();
    w.input(m, Input::ScreenUp);
    w.input(m, Input::OpenGiven(path.clone()));
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.input(m, Input::OpenGiven(path));
    assert_eq!(dialog(m), None);
    assert_eq!(m.view().timeline.tempo_bpm, 100.0);
    assert_eq!(file(m), SongFile::Changed);
}

#[test]
fn a_handed_file_takes_the_place_of_a_dialog_up_now() {
    let w = &mut World::new("flow-given-dialog");
    let path = write_two_versions(&w.songs);
    let m = &mut ready();
    w.input(m, Input::ScreenUp);
    cmd(m, Command::SetTempo { bpm: 100.0 });
    w.cmd(m, Command::NewSong);
    let first = dialog(m).unwrap();
    w.input(m, Input::OpenGiven(path.clone()));
    let d = dialog(m).unwrap();
    assert_eq!(d.kind, SongDialogKind::Unsaved);
    assert_ne!(d.id, first.id);
    w.answer(m, SongAnswer::DontSave);
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(m.view().timeline.tempo_bpm, 140.0);
}

#[test]
fn a_handed_file_is_let_go_while_recording() {
    let w = &mut World::new("flow-given-rec");
    let path = write_two_versions(&w.songs);
    let m = &mut ready();
    w.input(m, Input::ScreenUp);
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    w.input(m, Input::OpenGiven(path));
    assert_eq!(dialog(m), None);
    assert_eq!(m.song_path(), None);
    assert!(m.view().transport.recording.is_some());
}

fn recent_ids(m: &Model) -> Vec<u32> {
    m.view().recent.iter().map(|r| r.id).collect()
}

fn kept_recent(effects: &[Effect]) -> Option<Vec<PathBuf>> {
    effects.iter().find_map(|e| match e {
        Effect::SaveRecentSongs(songs) => Some(songs.clone()),
        _ => None,
    })
}

#[test]
fn the_recent_rows_are_the_newest_ten_by_name_and_folder() {
    let w = &mut World::new("flow-recent-rows");
    let list: Vec<PathBuf> = (0..12)
        .map(|n| w.songs.join(format!("曲{n}/曲{n}.meloet")))
        .collect();
    let m = &mut ready().with_song_places(None, list);
    let rows = m.view().recent;
    assert_eq!(rows.len(), meloet_protocol::RECENT_SHOWN);
    assert_eq!(rows[0].name, "曲0");
    let folder = crate::plugins::shown_path(&w.songs.join("曲0"), m.plugins.home());
    assert_eq!(rows[0].place, format!("{folder}/"));
    let mut ids = recent_ids(m);
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), rows.len(), "every row its own id");
}

#[test]
fn a_recent_row_opens_its_song_asking_first_and_an_unknown_id_does_nothing() {
    let w = &mut World::new("flow-recent-open");
    let path = write_two_versions(&w.songs);
    let other = w.songs.join("別/別.meloet");
    let m = &mut ready().with_song_places(None, vec![other.clone(), path.clone()]);
    w.input(m, Input::ScreenUp);
    let ids = recent_ids(m);
    cmd(m, Command::SetTempo { bpm: 100.0 });

    w.cmd(m, Command::OpenRecent { id: ids[1] });
    let d = dialog(m).unwrap();
    assert_eq!(
        (d.kind, d.title.as_str()),
        (SongDialogKind::Unsaved, "はじめての曲")
    );
    w.answer(m, SongAnswer::DontSave);
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(m.view().timeline.tempo_bpm, 140.0);
    assert!(
        w.windows.is_empty(),
        "the PC's open window is not asked for"
    );

    // Opened: it comes first, and keeps its id.
    assert_eq!(m.recent_songs(), [path.clone(), other.clone()]);
    assert_eq!(recent_ids(m), [ids[1], ids[0]]);

    // The song open now, and an id no row has: nothing.
    assert!(w.cmd(m, Command::OpenRecent { id: ids[1] }).is_empty());
    assert!(w.cmd(m, Command::OpenRecent { id: 9_999 }).is_empty());
    assert_eq!(dialog(m), None);
}

#[test]
fn a_recent_song_that_is_gone_says_it_does_not_read_and_leaves_the_list() {
    let w = &mut World::new("flow-recent-gone");
    let path = write_two_versions(&w.songs);
    let gone = w.songs.join("ない/ない.meloet");
    let m = &mut ready().with_song_places(None, vec![gone, path.clone()]);
    w.input(m, Input::ScreenUp);
    let ids = recent_ids(m);

    let rest = w.cmd(m, Command::OpenRecent { id: ids[0] });
    let d = dialog(m).unwrap();
    assert_eq!(
        (d.kind, d.title.as_str(), d.previous),
        (SongDialogKind::Broken, "ない.meloet", false)
    );
    assert_eq!(kept_recent(&rest), Some(vec![path.clone()]));
    assert_eq!(recent_ids(m), [ids[1]]);
    assert_eq!(song(m), Song::new_default());

    // A file that is there but does not read stays in the list.
    w.answer(m, SongAnswer::Cancel);
    fs::write(&path, b"{\"format\": \"meloet-song\", \"version\": 1, \"so").unwrap();
    let rest = w.cmd(m, Command::OpenRecent { id: ids[1] });
    assert_eq!(kind(m), Some(SongDialogKind::Broken));
    assert_eq!(kept_recent(&rest), None);
    assert_eq!(recent_ids(m), [ids[1]]);

    // The id of the row that left does nothing.
    w.answer(m, SongAnswer::Cancel);
    w.cmd(m, Command::OpenRecent { id: ids[0] });
    assert_eq!(dialog(m), None);
}

#[test]
fn clearing_the_recent_list_empties_it_and_ids_are_not_given_again() {
    let w = &mut World::new("flow-recent-clear");
    let path = write_two_versions(&w.songs);
    let m = &mut ready().with_song_places(None, vec![path.clone()]);
    w.input(m, Input::ScreenUp);
    let old = recent_ids(m)[0];
    let rest = w.cmd(m, Command::ClearRecent);
    assert_eq!(kept_recent(&rest), Some(Vec::new()));
    assert!(m.view().recent.is_empty());
    assert!(m.recent_songs().is_empty());
    let rest = w.cmd(m, Command::ClearRecent);
    assert_eq!(kept_recent(&rest), None, "not written again");
    w.cmd(m, Command::OpenRecent { id: old });
    assert_eq!(m.song_path(), None);

    // Opened again: the song gets a new id.
    w.input(m, Input::OpenGiven(path.clone()));
    assert_eq!(m.recent_songs(), [path]);
    assert_ne!(recent_ids(m), [old]);
}

#[test]
fn a_recent_row_does_nothing_while_recording() {
    let w = &mut World::new("flow-recent-rec");
    let path = write_two_versions(&w.songs);
    let m = &mut ready().with_song_places(None, vec![path]);
    w.input(m, Input::ScreenUp);
    let id = recent_ids(m)[0];
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    w.cmd(m, Command::OpenRecent { id });
    assert_eq!(dialog(m), None);
    assert_eq!(m.song_path(), None);
}
