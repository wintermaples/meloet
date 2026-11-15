//! New song, open, save, and asking before the song is let go (closing, a new song, opening
//! another). The files themselves are read and written by the core thread
//! ([`crate::songfile`]); the windows to pick them are the shell's.

use std::path::PathBuf;

use meloet_model::Song;
use meloet_protocol::{SongAnswer, SongDialogKind, SongDialogView, SongFile};

use super::{Effect, Input, Model, SaveTo};

/// What comes once the song is saved, or let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
    Stay,
    Close,
    NewSong,
    Open,
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
    exiting: bool,
}

impl Files {
    /// meloet ends: nothing is asked or written any more.
    pub(super) fn exiting(&self) -> bool {
        self.exiting
    }
}

impl Model {
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

    pub(super) fn save_song(&mut self, effects: &mut Vec<Effect>) {
        if self.song_free() {
            self.save(Then::Stay, None, effects);
        }
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
            (Input::SongSaved { path, name }, Some(Waiting::Save(then))) => {
                self.files.waiting = None;
                self.files.path = Some(path);
                self.play.saved_as(name);
                self.recovery_done(effects);
                self.then(then, effects);
            }
            (Input::SongNotSaved, Some(Waiting::Save(then))) => {
                self.files.waiting = None;
                self.put_up(SongDialogKind::CannotSave, then, None);
            }
            (
                Input::SongRead {
                    path,
                    song,
                    from_previous,
                },
                _,
            ) if reading => {
                self.files.waiting = None;
                self.files.path = Some(path);
                self.take_song(*song, from_previous, effects);
            }
            (Input::SongUnreadable { path, previous }, _) if reading => {
                self.files.waiting = None;
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

    fn close_requested(&mut self, effects: &mut Vec<Effect>) {
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
        self.files.waiting = Some(Waiting::Save(then));
        effects.push(Effect::SaveSong {
            song: Box::new(self.play.song().clone()),
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
                effects.push(Effect::Exit);
            }
            Then::NewSong => {
                self.files.path = None;
                self.take_song(Song::new_default(), false, effects);
                self.new_song_unsaid = true;
            }
            Then::Open => {
                self.files.waiting = Some(Waiting::PickFile);
                effects.push(Effect::PickSongFile);
            }
        }
    }

    /// Another song takes the place of this one, which is let go.
    fn take_song(&mut self, song: Song, from_previous: bool, effects: &mut Vec<Effect>) {
        self.replace_song(song, from_previous, effects);
        self.recovery_done(effects);
    }

    /// `unsaved`: the song does not match its file.
    pub(super) fn replace_song(&mut self, song: Song, unsaved: bool, effects: &mut Vec<Effect>) {
        self.play.replace_song(song, unsaved, effects);
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

#[cfg(test)]
mod tests;
