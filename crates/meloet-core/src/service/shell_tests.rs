//! The shell's side of the core: where the PC's windows start, and the shell's own listening,
//! which is not the screen's greeting.

use std::sync::mpsc::channel;
use std::time::Duration;

use meloet_model::Song;
use meloet_protocol::{SongAnswer, SongDialogKind};

use super::plug_tests::{options, plughost, state_when};
use super::*;

#[test]
fn the_shells_own_listening_is_not_the_screens_greeting() {
    let root = plughost::temp_dir("shell-listen");
    let given = root.join("曲.meloet");
    let mut song = Song::new_default();
    song.doc.name = "曲".into();
    songfile::save(&given, &song).unwrap();
    let mut waiting = options(&root);
    if let Some(plugins) = waiting.plugins.as_mut() {
        plugins.wait_for_screen = true;
    }
    let core = Core::start(waiting);
    core.open_given(given);
    core.listen(Box::new(|_| true));
    std::thread::sleep(Duration::from_millis(500));
    let held = core.peek(|w| w.plugins.as_ref().unwrap().held.as_ref().map(Vec::len));
    assert_eq!(held, Some(1), "the scan waits for the page");
    assert_ne!(
        core.snapshot().state.song.name,
        "曲",
        "so does the song handed over"
    );

    core.subscribe(Box::new(|_| true), Box::new(|_| true));
    state_when(&core, "the song handed over", |s| s.song.name == "曲");
    state_when(&core, "the scan after the screen", |s| {
        s.library.plugins.len() == 2
    });
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_pcs_windows_start_among_the_song_folders_or_in_the_songs_folder() {
    let songs = crate::settings::temp_dir("shell-windows");
    let core = Core::start(CoreOptions {
        songs_dir: Some(songs.clone()),
        ..CoreOptions::offline()
    });
    let (shell, asked) = channel();
    core.set_shell(Box::new(move |request| {
        let _ = shell.send(request);
    }));
    let started_in = || match asked.recv_timeout(Duration::from_secs(5)) {
        Ok(
            ShellRequest::PickSongFile { folder }
            | ShellRequest::PickFolder { folder }
            | ShellRequest::PickSaveAs { folder, .. },
        ) => folder,
        other => panic!("{other:?}"),
    };
    core.send(Command::SaveSong);
    let name = core.snapshot().state.song.name;
    let song_folder = songs.join(&name);
    assert!(song_folder.join(format!("{name}.meloet")).is_file());

    core.send(Command::SaveSongAs);
    assert_eq!(
        started_in(),
        Some(songs.clone()),
        "beside the song's folder"
    );
    core.save_as_picked(None);
    core.send(Command::OpenSong);
    assert_eq!(started_in(), Some(song_folder.clone()));
    core.song_file_picked(None);

    std::fs::remove_dir_all(&song_folder).unwrap();
    core.send(Command::OpenSong);
    assert_eq!(
        started_in(),
        Some(songs.clone()),
        "the song's folder is gone"
    );
    core.song_file_picked(None);
    core.send(Command::SaveSongAs);
    assert_eq!(started_in(), Some(songs.clone()));
    core.save_as_picked(None);
    core.send(Command::SaveSong);
    let dialog = core.snapshot().state.workspace.song_dialog.unwrap();
    assert_eq!(dialog.kind, SongDialogKind::CannotSave);
    core.send(Command::AnswerSongDialog {
        dialog: dialog.id,
        answer: SongAnswer::ChooseLocation,
    });
    assert_eq!(started_in(), Some(songs.clone()));
    core.folder_picked(None);
    core.stop();
}
