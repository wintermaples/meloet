//! When the recovery file is written and removed, and what the start does with one (spec 3:
//! "なおすためのファイル"). The file itself is written by [`crate::recovery`] on a thread of
//! its own.

use std::path::PathBuf;

use meloet_model::{Doc, Song};
use meloet_protocol::{KeptVocals, RecoveryCause, RecoveryView, StatusMessage};

use super::{Effect, Model};

/// How long the song stays unchanged before it is written every now and then.
pub const QUIET_US: u64 = 2_000_000;

/// What the start found in the recovery folder, with the song's file read beside it.
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryStart {
    Read {
        song: Box<Song>,
        /// The song's file; `None` for a song never saved.
        path: Option<PathBuf>,
        cause: RecoveryCause,
        written_ms: u64,
        /// The song as its file has it now; `None` when never saved, or when the file does not
        /// read.
        saved: Option<Box<Song>>,
        /// Vocals that were being recorded when meloet ended: they are clips of `song` now,
        /// which the recovery file does not hold yet.
        vocals: Option<KeptVocals>,
    },
    Unreadable {
        /// Where the file that did not read was kept.
        kept: PathBuf,
        /// The song last saved, from the file the recovery file named, when that reads.
        saved: Option<(PathBuf, Box<Song>)>,
    },
}

#[derive(Debug, Default)]
pub(super) struct Recovery {
    /// `Play::changes` as last seen.
    seen: u64,
    /// When a change not written yet was seen.
    changed_at_us: Option<u64>,
    /// `Play::changes` and the cause of what the file holds now.
    written: Option<(u64, RecoveryCause)>,
    view: Option<RecoveryView>,
    /// What "戻さない" puts back.
    saved: Option<Doc>,
    /// The file that did not read.
    kept: Option<PathBuf>,
    /// The PC is ending: write once the song is free (a recording stopped and in the song).
    ending: bool,
}

impl Model {
    pub(super) fn recovery_view(&self) -> Option<RecoveryView> {
        self.recovery.view.clone()
    }

    /// The song changed and waits to be written: the core thread comes back with
    /// [`super::Input::Tick`].
    pub fn recovery_waiting(&self) -> bool {
        self.recovery.changed_at_us.is_some()
    }

    pub(super) fn recovery_start(&mut self, start: RecoveryStart, effects: &mut Vec<Effect>) {
        match start {
            RecoveryStart::Read {
                song,
                path,
                cause,
                written_ms,
                saved,
                vocals,
            } => {
                let takes = song
                    .doc
                    .tracks
                    .iter()
                    .flat_map(|t| &t.clips)
                    .map(|c| c.takes.len() as u32)
                    .sum();
                let name = song.doc.name.clone();
                let base = match (saved, &path) {
                    (Some(saved), _) => saved.doc,
                    (None, None) => self.play.new_song().doc,
                    // The file is gone or broken: what "戻さない" gives keeps the name only.
                    (None, Some(_)) => Doc {
                        name: name.clone(),
                        ..self.play.new_song().doc
                    },
                };
                self.recovery.saved = Some(base.clone());
                self.recovery.view = Some(RecoveryView::Recovered {
                    written_ms,
                    cause,
                    song: path.is_some().then_some(name),
                    takes,
                    vocals,
                });
                self.files.path = path;
                let Song { doc, session } = *song;
                self.replace_song(Song { doc: base, session }, false, effects);
                self.play.recovered(doc, effects);
                self.notice = Some(StatusMessage::Recovered);
                // The file holds this song already; with vocals kept, it is written anew.
                self.recovery.seen = self.play.changes();
                self.recovery.written = Some((self.recovery.seen, cause));
                if vocals.is_some() {
                    self.recovery.written = None;
                    self.recovery.changed_at_us = Some(self.now_us);
                }
            }
            RecoveryStart::Unreadable { kept, saved } => {
                let song = saved.map(|(path, song)| {
                    let name = song.doc.name.clone();
                    self.files.path = Some(path);
                    self.replace_song(*song, false, effects);
                    name
                });
                self.notice = Some(StatusMessage::RecoveryUnreadable {
                    saved_song: song.is_some(),
                });
                self.recovery.view = Some(RecoveryView::Unreadable { song });
                self.recovery.kept = Some(kept);
                self.recovery.seen = self.play.changes();
            }
        }
        self.new_song_unsaid = false;
    }

    /// "戻さない".
    pub(super) fn discard_recovered(&mut self, effects: &mut Vec<Effect>) {
        if !matches!(self.recovery.view, Some(RecoveryView::Recovered { .. }))
            || self.play.busy()
            || !self.song_free()
        {
            return;
        }
        let Some(saved) = self.recovery.saved.take() else {
            return;
        };
        self.play.discard_recovered(saved, effects);
        self.recovery.view = None;
        self.notice = Some(StatusMessage::RecoveryDiscarded);
    }

    /// "ファイルの場所をひらく".
    pub(super) fn show_recovery_file(&mut self, effects: &mut Vec<Effect>) {
        if let (Some(RecoveryView::Unreadable { .. }), Some(kept)) =
            (&self.recovery.view, &self.recovery.kept)
        {
            effects.push(Effect::ShowFile(kept.clone()));
        }
    }

    /// "閉じる". Returns whether a recovery hint took it; the one about a song brought back
    /// has no "閉じる" and stays.
    pub(super) fn dismiss_recovery(&mut self) -> bool {
        match self.recovery.view {
            Some(RecoveryView::Unreadable { .. }) => {
                self.recovery.view = None;
                self.recovery.kept = None;
                true
            }
            Some(RecoveryView::Recovered { .. }) => true,
            None => false,
        }
    }

    /// The song is in its file, or let go: the file is no longer needed, nor the hint.
    pub(super) fn recovery_done(&mut self, effects: &mut Vec<Effect>) {
        self.recovery.view = None;
        self.recovery.saved = None;
        self.recovery.kept = None;
        self.recovery.written = None;
        effects.push(Effect::RemoveRecovery);
    }

    /// The song has changes its file does not have, though it was just saved (they came
    /// while the save went on): they are written as any other change.
    pub(super) fn recovery_changed_again(&mut self) {
        if self.play.has_unsaved_changes() {
            self.recovery.changed_at_us = Some(self.now_us);
        }
    }

    /// The PC is ending: the song is written as soon as it is free.
    pub(super) fn pc_ending(&mut self, effects: &mut Vec<Effect>) {
        self.recovery.ending = true;
        self.play.stop_recording(effects);
    }

    /// After every input: writes what is due.
    pub(super) fn recovery_tick(&mut self, effects: &mut Vec<Effect>) {
        let changes = self.play.changes();
        if changes != self.recovery.seen {
            self.recovery.seen = changes;
            self.recovery.changed_at_us = Some(self.now_us);
        }
        if self.play.busy() {
            return;
        }
        if self.recovery.ending {
            self.recovery.ending = false;
            self.recovery.changed_at_us = None;
            if !self.write_recovery(RecoveryCause::PcEnding, effects) {
                effects.push(Effect::EndingReady);
            }
            return;
        }
        if self
            .recovery
            .changed_at_us
            .is_some_and(|at| self.now_us.saturating_sub(at) >= QUIET_US)
        {
            self.recovery.changed_at_us = None;
            if !self.files.exiting() {
                self.write_recovery(RecoveryCause::Sudden, effects);
            }
        }
    }

    /// Returns whether a write was asked for. Nothing is written without unsaved changes, nor
    /// again what the file holds already.
    fn write_recovery(&mut self, cause: RecoveryCause, effects: &mut Vec<Effect>) -> bool {
        let now = (self.play.changes(), cause);
        if !self.play.has_unsaved_changes() || self.recovery.written == Some(now) {
            return false;
        }
        self.recovery.written = Some(now);
        effects.push(Effect::WriteRecovery {
            song: Box::new(self.play.song().clone()),
            path: self.files.path.clone(),
            cause,
            ending: cause == RecoveryCause::PcEnding,
        });
        true
    }
}

#[cfg(test)]
mod tests;
