//! New song, open, save, and asking before the song is let go (closing, a new song, opening
//! another). The files themselves are read and written by the core thread
//! ([`crate::songfile`]); the windows to pick them are the shell's.

use std::path::{Path, PathBuf};

use meloet_model::Song;
use meloet_protocol::{
    RECENT_SHOWN, RecentSong, SongAnswer, SongDialogKind, SongDialogView, SongFile,
};

use super::{Effect, Input, Model, SaveTo};
use crate::ai::AiOutcome;

/// What comes once the song is saved, or let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
    Stay,
    Close,
    NewSong,
    Open,
    /// The file the PC handed over ([`Files::given`]).
    OpenGiven,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Dialog {
    id: u32,
    kind: SongDialogKind,
    title: String,
    previous: bool,
    then: Then,
    /// `Broken`: the file that did not read.
    path: Option<PathBuf>,
}

/// What the core waits for from the shell or the disk. Song commands wait meanwhile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waiting {
    Save(Then),
    PickFile,
    PickFolder(Then),
    PickSaveAs,
    Read,
}

#[derive(Debug, Default)]
pub(super) struct Files {
    /// The song's file; `None` until the first save.
    pub(super) path: Option<PathBuf>,
    dialog: Option<Dialog>,
    last_dialog: u32,
    waiting: Option<Waiting>,
    /// Asked to close while recording: once the take is in the song.
    close_when_recorded: bool,
    /// Asked to close while a save is written: once it has answered.
    close_when_saved: bool,
    exiting: bool,
    /// The name typed in the save window, for the save that goes on from there (also after
    /// "保存できない" and a folder picked).
    save_as: Option<String>,
    /// The song file the PC handed over (a double click, `meloet <file>`), until it is read.
    given: Option<PathBuf>,
    /// The screen greeted once: a handed file waits for it.
    screen_up: bool,
    /// The save or the read going on was asked for by the AI link, with this ticket: it
    /// hears how it ended, and no dialog is put up for it.
    ai: Option<u64>,
    /// Song files opened or saved lately, the newest first (`crate::recent`).
    recent: Vec<PathBuf>,
    /// `RecentSong::id` of each of `recent`: a path keeps its id while it stays in the list,
    /// and an id is not given again, so an id from before the list changed names its song
    /// or nothing.
    recent_ids: Vec<u32>,
    last_recent_id: u32,
    /// Where a song's first save makes its folder.
    songs_dir: Option<PathBuf>,
}

impl Files {
    /// meloet ends: nothing is asked or written any more.
    pub(super) fn exiting(&self) -> bool {
        self.exiting
    }
}

impl Model {
    /// The songs folder, and the songs opened or saved lately as the state folder keeps them.
    pub fn with_song_places(mut self, songs_dir: Option<PathBuf>, recent: Vec<PathBuf>) -> Model {
        self.files.songs_dir = songs_dir;
        self.set_recent(recent);
        self
    }

    /// 「自分で残したもの」 of the effects as the PC keeps them (round 12, M1).
    pub fn with_user_presets(mut self, presets: crate::presets::UserPresets) -> Model {
        self.play.set_user_presets(presets);
        self
    }

    fn set_recent(&mut self, list: Vec<PathBuf>) {
        let files = &mut self.files;
        let ids = (list.iter())
            .map(|path| {
                let kept = files.recent.iter().position(|p| p == path);
                kept.map(|i| files.recent_ids[i]).unwrap_or_else(|| {
                    files.last_recent_id += 1;
                    files.last_recent_id
                })
            })
            .collect();
        files.recent = list;
        files.recent_ids = ids;
    }

    /// 「最近ひらいた曲 ▸」: the newest `RECENT_SHOWN`, by the song file's name and its folder.
    pub(super) fn recent_view(&self) -> Vec<RecentSong> {
        let home = self.plugins.home();
        (self.files.recent.iter().zip(&self.files.recent_ids))
            .take(RECENT_SHOWN)
            .map(|(path, &id)| {
                let name = path.file_stem().unwrap_or(path.as_os_str());
                let mut place = path.parent().map_or_else(String::new, |folder| {
                    crate::plugins::shown_path(folder, home)
                });
                if !place.ends_with('/') {
                    place.push('/');
                }
                RecentSong {
                    id,
                    name: name.to_string_lossy().into_owned(),
                    place,
                }
            })
            .collect()
    }

    /// A row of 「最近ひらいた曲 ▸」: as 「ひらく…」 with the file already picked. The song open
    /// now is left as it is.
    pub(super) fn open_recent(&mut self, id: u32, effects: &mut Vec<Effect>) {
        let Some(i) = self.files.recent_ids.iter().position(|&r| r == id) else {
            return;
        };
        let path = self.files.recent[i].clone();
        if self.song_free() && self.files.path.as_deref() != Some(path.as_path()) {
            self.files.given = Some(path);
            self.let_go(Then::OpenGiven, effects);
        }
    }

    /// 「一覧を消す」.
    pub(super) fn clear_recent(&mut self, effects: &mut Vec<Effect>) {
        if !self.files.recent.is_empty() {
            self.set_recent(Vec::new());
            effects.push(Effect::SaveRecentSongs(Vec::new()));
        }
    }

    /// A song file that did not read because it is gone leaves the list.
    fn forget_if_gone(&mut self, path: &Path, effects: &mut Vec<Effect>) {
        if self.files.recent.iter().any(|p| p == path) && !path.exists() {
            let list: Vec<PathBuf> = (self.files.recent.iter())
                .filter(|p| *p != path)
                .cloned()
                .collect();
            self.set_recent(list.clone());
            effects.push(Effect::SaveRecentSongs(list));
        }
    }

    pub fn recent_songs(&self) -> &[PathBuf] {
        &self.files.recent
    }

    pub fn songs_dir(&self) -> Option<&Path> {
        self.files.songs_dir.as_deref()
    }

    /// The song at `path` was opened or saved: it heads the list.
    fn note_recent(&mut self, path: &Path, effects: &mut Vec<Effect>) {
        let list = crate::recent::noted(&self.files.recent, path);
        if list != self.files.recent {
            self.set_recent(list.clone());
            effects.push(Effect::SaveRecentSongs(list));
        }
    }

    /// A save was asked for and has not answered yet.
    pub(super) fn saving(&self) -> bool {
        matches!(self.files.waiting, Some(Waiting::Save(_)))
    }

    /// The song's file, once it has one.
    pub fn song_path(&self) -> Option<&std::path::Path> {
        self.files.path.as_deref()
    }

    pub(super) fn song_file(&self) -> SongFile {
        if self.play.has_unsaved_changes() {
            SongFile::Changed
        } else if self.files.path.is_some() {
            SongFile::Saved
        } else {
            SongFile::NeverSaved
        }
    }

    pub(super) fn song_dialog_view(&self) -> Option<SongDialogView> {
        self.files.dialog.as_ref().map(|d| SongDialogView {
            id: d.id,
            kind: d.kind,
            title: d.title.clone(),
            previous: d.previous,
        })
    }

    /// The song can be saved, replaced or let go now.
    pub(super) fn song_free(&self) -> bool {
        !self.play.busy()
            && self.files.waiting.is_none()
            && self.files.dialog.is_none()
            && !self.files.exiting
    }

    pub(super) fn new_song(&mut self, effects: &mut Vec<Effect>) {
        if self.song_free() {
            self.let_go(Then::NewSong, effects);
        }
    }

    pub(super) fn open_song(&mut self, effects: &mut Vec<Effect>) {
        if self.song_free() {
            self.let_go(Then::Open, effects);
        }
    }

    /// A song file the PC handed over: opened as 「ひらく」 does with the file already picked,
    /// once the screen is up. A dialog up now gives way to it; while recording, saving or
    /// waiting for a window it is let go. The song already open is left as it is.
    pub(super) fn open_given(&mut self, path: PathBuf, effects: &mut Vec<Effect>) {
        if !self.files.screen_up {
            self.files.given = Some(path);
            return;
        }
        if self.play.busy()
            || self.files.waiting.is_some()
            || self.files.exiting
            || self.files.path.as_deref() == Some(path.as_path())
        {
            return;
        }
        self.files.dialog = None;
        self.files.given = Some(path);
        self.let_go(Then::OpenGiven, effects);
    }

    pub(super) fn songs_screen_up(&mut self, effects: &mut Vec<Effect>) {
        if self.files.screen_up {
            return;
        }
        self.files.screen_up = true;
        if let Some(path) = self.files.given.take() {
            self.open_given(path, effects);
        }
    }

    pub(super) fn save_song(&mut self, effects: &mut Vec<Effect>) {
        if self.song_free() {
            self.files.save_as = None;
            self.save(Then::Stay, None, effects);
        }
    }

    /// Mod Shift S. The song gets a new folder where the user picks, and the name typed there.
    pub(super) fn save_song_as(&mut self, effects: &mut Vec<Effect>) {
        if self.song_free() {
            self.files.waiting = Some(Waiting::PickSaveAs);
            effects.push(Effect::PickSaveAs {
                name: self.play.song().doc.name.clone(),
            });
        }
    }

    /// The AI link saves: over the song's file, or (`to`) as a new song folder of that name
    /// in that folder. The caller has checked `song_free`.
    pub(super) fn ai_save(
        &mut self,
        ticket: u64,
        to: Option<(PathBuf, String)>,
        effects: &mut Vec<Effect>,
    ) {
        self.files.ai = Some(ticket);
        let folder = to.map(|(folder, name)| {
            self.files.save_as = Some(name);
            folder
        });
        if folder.is_none() {
            self.files.save_as = None;
        }
        self.save(Then::Stay, folder, effects);
    }

    /// The AI link opens a song file. The caller has checked that nothing unsaved is lost.
    pub(super) fn ai_open(&mut self, ticket: u64, path: PathBuf, effects: &mut Vec<Effect>) {
        self.files.ai = Some(ticket);
        self.files.save_as = None;
        self.files.waiting = Some(Waiting::Read);
        effects.push(Effect::ReadSong {
            path,
            previous: false,
        });
    }

    /// The AI link starts a new song. The caller has checked that nothing unsaved is lost.
    pub(super) fn ai_new_song(&mut self, effects: &mut Vec<Effect>) {
        self.files.save_as = None;
        self.then(Then::NewSong, effects);
    }

    /// Esc. Returns whether a song dialog was there to put away.
    pub(super) fn cancel_song_dialog(&mut self) -> bool {
        self.files.dialog.take().is_some()
    }

    pub(super) fn answer_song_dialog(
        &mut self,
        id: u32,
        answer: SongAnswer,
        effects: &mut Vec<Effect>,
    ) {
        let Some(dialog) = self.files.dialog.take_if(|d| d.id == id) else {
            return;
        };
        match (dialog.kind, answer) {
            (_, SongAnswer::Cancel) => {}
            (SongDialogKind::Unsaved, SongAnswer::Save) => self.save(dialog.then, None, effects),
            (SongDialogKind::Unsaved, SongAnswer::DontSave) => self.then(dialog.then, effects),
            (SongDialogKind::CannotSave, SongAnswer::ChooseLocation) => {
                self.files.waiting = Some(Waiting::PickFolder(dialog.then));
                effects.push(Effect::PickFolder);
            }
            (SongDialogKind::Broken, SongAnswer::OpenPrevious) if dialog.previous => {
                if let Some(path) = dialog.path {
                    self.files.waiting = Some(Waiting::Read);
                    effects.push(Effect::ReadSong {
                        path,
                        previous: true,
                    });
                }
            }
            // A button this dialog does not have: it stays.
            _ => self.files.dialog = Some(dialog),
        }
    }

    pub(super) fn song_input(&mut self, input: Input, effects: &mut Vec<Effect>) {
        let waiting = self.files.waiting;
        let reading = waiting == Some(Waiting::Read);
        match (input, waiting) {
            (Input::CloseRequested, _) => self.close_requested(effects),
            (Input::SongFilePicked(path), Some(Waiting::PickFile)) => {
                self.files.waiting = None;
                if let Some(path) = path {
                    self.files.waiting = Some(Waiting::Read);
                    effects.push(Effect::ReadSong {
                        path,
                        previous: false,
                    });
                }
            }
            (Input::FolderPicked(folder), Some(Waiting::PickFolder(then))) => {
                self.files.waiting = None;
                match folder {
                    Some(folder) => self.save(then, Some(folder), effects),
                    // Back to where the folder window was asked for.
                    None => self.put_up(SongDialogKind::CannotSave, then, None),
                }
            }
            (Input::SaveAsPicked(picked), Some(Waiting::PickSaveAs)) => {
                self.files.waiting = None;
                if let Some((folder, name)) = picked.as_deref().and_then(split_save_as) {
                    self.files.save_as = Some(name);
                    self.save(Then::Stay, Some(folder), effects);
                }
            }
            (Input::SongSaved { path, name }, Some(Waiting::Save(then))) => {
                if let Some(ticket) = self.files.ai.take() {
                    self.ai_ended(ticket, AiOutcome::Done(Some(path.clone())));
                }
                self.files.waiting = None;
                self.files.save_as = None;
                self.note_recent(&path, effects);
                self.files.path = Some(path);
                self.play.saved_as(name);
                self.recovery_done(effects);
                // What was changed while the save went on is not in the file.
                self.recovery_changed_again();
                if std::mem::take(&mut self.files.close_when_saved) && then != Then::Close {
                    self.close_requested(effects);
                } else {
                    self.then(then, effects);
                }
            }
            (Input::SongNotSaved, Some(Waiting::Save(then))) => {
                self.files.waiting = None;
                self.play.not_saved();
                let close = std::mem::take(&mut self.files.close_when_saved);
                match self.files.ai.take() {
                    Some(ticket) => {
                        let why = "the song could not be written there".to_owned();
                        self.ai_ended(ticket, AiOutcome::Failed(why));
                        if close {
                            self.close_requested(effects);
                        }
                    }
                    // 「保存できない」 comes before a close asked meanwhile.
                    None => self.put_up(SongDialogKind::CannotSave, then, None),
                }
            }
            (
                Input::SongRead {
                    path,
                    song,
                    from_previous,
                },
                _,
            ) if reading => {
                if let Some(ticket) = self.files.ai.take() {
                    self.ai_ended(ticket, AiOutcome::Done(Some(path.clone())));
                }
                self.files.waiting = None;
                self.note_recent(&path, effects);
                self.files.path = Some(path);
                self.take_song(*song, from_previous, effects);
                self.say_missing();
            }
            (Input::SongUnreadable { path, previous }, _) if reading => {
                self.files.waiting = None;
                self.forget_if_gone(&path, effects);
                if let Some(ticket) = self.files.ai.take() {
                    let why = match previous {
                        true => "the file does not read; its .prev copy might",
                        false => "the file does not read",
                    };
                    self.ai_ended(ticket, AiOutcome::Failed(why.to_owned()));
                    return;
                }
                self.put_up(SongDialogKind::Broken, Then::Stay, Some(path));
                if let Some(d) = &mut self.files.dialog {
                    d.previous = previous;
                }
            }
            _ => {}
        }
    }

    /// Once a recording stopped for closing is in the song, asks about it.
    pub(super) fn close_when_recorded(&mut self, effects: &mut Vec<Effect>) {
        if self.files.close_when_recorded && !self.play.is_moving() {
            self.files.close_when_recorded = false;
            self.close_requested(effects);
        }
    }

    pub(super) fn close_requested(&mut self, effects: &mut Vec<Effect>) {
        if self.saving() && !self.files.exiting {
            self.files.close_when_saved = true;
        }
        if self.files.exiting || self.files.waiting.is_some() {
            return;
        }
        if self.play.busy() || self.play.is_recording() {
            self.files.close_when_recorded = true;
            self.play.stop_recording(effects);
            return;
        }
        // Closing comes before whatever the dialog up now was asking about.
        self.files.dialog = None;
        self.let_go(Then::Close, effects);
    }

    /// Asks first when there is something unsaved to lose.
    fn let_go(&mut self, then: Then, effects: &mut Vec<Effect>) {
        self.files.save_as = None;
        if self.play.has_unsaved_changes() {
            self.put_up(SongDialogKind::Unsaved, then, None);
        } else {
            self.then(then, effects);
        }
    }

    fn save(&mut self, then: Then, folder: Option<PathBuf>, effects: &mut Vec<Effect>) {
        let to = match (folder, &self.files.path) {
            (Some(folder), _) => SaveTo::NewFolder(Some(folder)),
            (None, Some(path)) => SaveTo::File(path.clone()),
            (None, None) => SaveTo::NewFolder(None),
        };
        let mut song = self.play.song().clone();
        if let (SaveTo::NewFolder(Some(_)), Some(name)) = (&to, &self.files.save_as) {
            song.doc.name = name.clone();
        }
        self.files.waiting = Some(Waiting::Save(then));
        self.play.saving();
        effects.push(Effect::SaveSong {
            song: Box::new(song),
            to,
        });
    }

    fn then(&mut self, then: Then, effects: &mut Vec<Effect>) {
        match then {
            Then::Stay => {}
            Then::Close => {
                self.files.exiting = true;
                // Ending the usual way: whatever was not saved was let go on purpose.
                self.recovery_done(effects);
                effects.push(Effect::ClearWorkAudio);
                effects.push(Effect::Exit);
            }
            Then::NewSong => {
                self.files.path = None;
                self.take_song(self.play.new_song(), false, effects);
                self.new_song_unsaid = true;
            }
            Then::Open => {
                self.files.waiting = Some(Waiting::PickFile);
                effects.push(Effect::PickSongFile);
            }
            Then::OpenGiven => {
                if let Some(path) = self.files.given.take() {
                    self.files.waiting = Some(Waiting::Read);
                    effects.push(Effect::ReadSong {
                        path,
                        previous: false,
                    });
                }
            }
        }
    }

    /// Another song takes the place of this one, which is let go.
    fn take_song(&mut self, song: Song, from_previous: bool, effects: &mut Vec<Effect>) {
        self.replace_song(song, from_previous, effects);
        self.recovery_done(effects);
        // What the song before left in the work place is named by nothing now
        // (docs/architecture.md 22-4).
        effects.push(Effect::ClearWorkAudio);
    }

    /// `unsaved`: the song does not match its file.
    pub(super) fn replace_song(&mut self, song: Song, unsaved: bool, effects: &mut Vec<Effect>) {
        self.play.replace_song(song, unsaved, effects);
        // A keyboard fader's level that waits to become a step goes with the song it was for.
        self.fader = Default::default();
        self.buffer_unsaid = true;
        self.new_song_unsaid = false;
    }

    fn put_up(&mut self, kind: SongDialogKind, then: Then, path: Option<PathBuf>) {
        self.files.last_dialog += 1;
        let title = match &path {
            Some(p) => p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            None => self.play.song().doc.name.clone(),
        };
        self.files.dialog = Some(Dialog {
            id: self.files.last_dialog,
            kind,
            title,
            previous: false,
            then,
            path,
        });
    }
}

/// The folder and the song's name from the path the save window gave. A name typed with the
/// song file's extension is taken without it.
fn split_save_as(path: &Path) -> Option<(PathBuf, String)> {
    let folder = path.parent().filter(|d| !d.as_os_str().is_empty())?;
    let name = path.file_name()?.to_string_lossy();
    let suffix = format!(".{}", crate::songfile::EXTENSION);
    let name = name.strip_suffix(&suffix).unwrap_or(&name).trim();
    (!name.is_empty()).then(|| (folder.to_path_buf(), name.to_owned()))
}

#[cfg(test)]
mod tests;
