//! Vocal clips and audio files, as the rules see them (spec 3, 26-A, 27-A, 29-A, 30-C;
//! docs/architecture.md 22-18 A2): the intents S, a clip's values, bringing files in,
//! 「さがす…」 and 「曲から外す」, and what the thread that reads audio files in is asked.

use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::{AudioSource, ImportPlan, TrackId};
use meloet_protocol::{
    AudioClipState, Command, DropMode, MissingAudioView, ParamsView, SelectedClip, SelectedParams,
    StatusMessage,
};

use super::{Effect, Model};
use crate::audiofiles::SONG_AUDIO;
use crate::audiofiles::decode::Unread;
use crate::audiofiles::keeper::{AudioNews, Brought, Finding};

/// Where files that are being brought in go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Target {
    /// `TrackView::id`; `None`: a new vocal track.
    track: Option<u32>,
    tick: u64,
    mode: DropMode,
}

#[derive(Debug)]
struct Bringing {
    ticket: u64,
    /// `Play::serial` of the song they go into.
    serial: u32,
    target: Target,
}

/// What the PC's window for audio files was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Picking {
    /// 「音のファイルを入れる…」.
    Import { track: Option<u32> },
    /// 「さがす…」 for this file of the audio table.
    Find { audio: u32 },
}

/// What the audio thread was last asked to watch.
#[derive(Debug)]
struct Watched {
    /// `Play::serial` of the song.
    serial: u32,
    /// Its audio table, as the `Doc` had it.
    table: Arc<Vec<Arc<AudioSource>>>,
    /// The rate of the open stream.
    rate: Option<u32>,
}

#[derive(Debug, Default)]
pub(super) struct AudioSide {
    watched: Option<Watched>,
    last_ticket: u64,
    bringing: Vec<Bringing>,
    /// Files that are in the work place and wait for a recording to end.
    brought: Vec<(Bringing, Vec<Brought>)>,
    picking: Option<Picking>,
    /// The song with this serial was put in place and its missing files were not said yet.
    missing_unsaid: Option<u32>,
    /// A clip was selected after the last insert or instrument: the parameters panel shows
    /// 「選んだクリップ」.
    clip_panel: bool,
}

fn sorted_by_name(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort_by_cached_key(|p| p.file_name().map(|n| n.to_os_string()));
    paths
}

impl Model {
    /// `dir`: `CoreOptions::cache_dir`. The peaks are kept in `<dir>/peaks`.
    pub fn with_cache_dir(mut self, dir: Option<PathBuf>) -> Model {
        self.play.set_peaks_dir(dir.map(|d| d.join("peaks")));
        self
    }

    /// Before a command is done: which of the two the parameters panel shows.
    pub(super) fn audio_saw_command(&mut self, command: &Command) {
        match command {
            Command::SelectClip { .. }
            | Command::SelectClips { .. }
            | Command::SelectAllClips
            | Command::SplitClips
            | Command::SplitClipAt { .. }
            | Command::JoinClips { .. }
            | Command::CreateClip { .. }
            | Command::DropAudio { .. }
            | Command::ImportAudio { .. } => self.audio.clip_panel = true,
            Command::SelectInsert { .. } | Command::SelectInstrument { .. } => {
                self.audio.clip_panel = false;
            }
            _ => {}
        }
    }

    /// S.
    pub(super) fn split_clips(&mut self, effects: &mut Vec<Effect>) {
        if let Some(notice) = self.play.split_clips(effects) {
            self.notice = Some(notice);
        }
    }

    /// 「音のファイルを入れる…」.
    pub(super) fn import_audio(&mut self, track: Option<u32>, effects: &mut Vec<Effect>) {
        if self.play.busy() || self.audio.picking.is_some() {
            return;
        }
        self.audio.picking = Some(Picking::Import { track });
        effects.push(Effect::PickAudio { many: true });
    }

    /// 「さがす…」.
    pub(super) fn find_audio(&mut self, audio: u32, effects: &mut Vec<Effect>) {
        let missing = self.play.missing_audio().iter().any(|s| s.id.0 == audio);
        if missing && self.audio.picking.is_none() {
            self.audio.picking = Some(Picking::Find { audio });
            effects.push(Effect::PickAudio { many: false });
        }
    }

    /// The answer of the PC's window; empty when it was put away.
    pub(super) fn audio_picked(&mut self, paths: Vec<PathBuf>, effects: &mut Vec<Effect>) {
        match self.audio.picking.take() {
            Some(Picking::Import { track }) if !paths.is_empty() => {
                let doc = &self.play.song().doc;
                let vocal = |id: u32| doc.track(TrackId(id)).is_some_and(|t| t.kind.is_vocal());
                let selected = self.play.song().session.play_target.map(|t| t.0);
                let target = Target {
                    track: track.or(selected).filter(|&t| vocal(t)),
                    tick: self.play.play_line_tick(),
                    mode: DropMode::InRow,
                };
                self.bring(paths, target, effects);
            }
            Some(Picking::Find { audio }) => {
                let source = self
                    .play
                    .missing_audio()
                    .into_iter()
                    .find(|s| s.id.0 == audio);
                if let (Some(source), Some(path)) = (source, paths.into_iter().next()) {
                    effects.push(Effect::FindAudio { source, path });
                }
            }
            _ => {}
        }
    }

    /// The screen said where the audio files that were let go of over the window go.
    pub(super) fn drop_audio(
        &mut self,
        drop: u32,
        track: Option<u32>,
        tick: u64,
        mode: DropMode,
        effects: &mut Vec<Effect>,
    ) {
        let Some(pending) = self.pending_drop.take_if(|p| p.id == drop) else {
            return;
        };
        if self.play.busy() {
            return;
        }
        self.bring(pending.paths, Target { track, tick, mode }, effects);
    }

    /// The AI link brings one file in, as 「音のファイルを入れる…」 does once it is picked.
    /// Returns the ticket of the bringing.
    pub(super) fn ai_import(
        &mut self,
        path: PathBuf,
        track: Option<u32>,
        tick: u64,
        effects: &mut Vec<Effect>,
    ) -> u64 {
        let target = Target {
            track,
            tick,
            mode: DropMode::InRow,
        };
        self.bring(vec![path], target, effects);
        self.audio.last_ticket
    }

    fn bring(&mut self, paths: Vec<PathBuf>, target: Target, effects: &mut Vec<Effect>) {
        self.audio.last_ticket += 1;
        let ticket = self.audio.last_ticket;
        self.audio.bringing.push(Bringing {
            ticket,
            serial: self.play.serial(),
            target,
        });
        let doc = &self.play.song().doc;
        effects.push(Effect::BringAudio {
            ticket,
            paths: sorted_by_name(paths),
            taken: doc.audio.iter().map(|s| s.file.clone()).collect(),
        });
    }

    pub(super) fn audio_news(&mut self, news: AudioNews) {
        match news {
            AudioNews::Brought { ticket, files } => {
                let Some(at) = self.audio.bringing.iter().position(|b| b.ticket == ticket) else {
                    return;
                };
                let bringing = self.audio.bringing.remove(at);
                self.audio.brought.push((bringing, files));
            }
            AudioNews::Found { audio, file, found } => {
                self.notice = match found {
                    Finding::Taken => {
                        // Looked at again: the clips sound.
                        self.audio.watched = None;
                        self.play.audio_found(audio)
                    }
                    Finding::Mismatch => Some(StatusMessage::AudioFoundMismatch),
                    Finding::Unreadable => Some(StatusMessage::AudioUnreadable { file }),
                };
            }
            AudioNews::States { serial, .. } => {
                if serial != self.play.serial() {
                    return;
                }
                self.play.audio_news(news);
                if self
                    .audio
                    .missing_unsaid
                    .take_if(|s| *s == serial)
                    .is_some()
                {
                    let count = self.play.missing_audio().len() as u32;
                    if count > 0 && self.notice.is_none() {
                        self.notice = Some(StatusMessage::AudioMissing { count });
                    }
                }
            }
            news => {
                self.play.audio_news(news);
            }
        }
    }

    /// Files that are in the work place go into the song, as one step.
    fn put_in(&mut self, bringing: &Bringing, files: Vec<Brought>, effects: &mut Vec<Effect>) {
        let mut unreadable = Vec::new();
        let mut read = Vec::new();
        for file in files {
            match file {
                Brought::In(file) => read.push(file),
                Brought::Unreadable(name, why) => unreadable.push((name, why)),
            }
        }
        if read.is_empty() {
            if let Some((file, why)) = unreadable.into_iter().next() {
                self.notice = Some(match why {
                    Unread::NotAudio => StatusMessage::AudioUnreadable { file },
                    Unread::Broken => StatusMessage::AudioBroken { file },
                    Unread::TooManyChannels => StatusMessage::AudioTooManyChannels { file },
                });
            }
            return;
        }
        let Target { track, tick, mode } = bringing.target;
        let plan = match (mode, read.len()) {
            (DropMode::Separate, 2..) => ImportPlan::Separate,
            _ => ImportPlan::InRow(track.map(TrackId)),
        };
        let skipped = unreadable.len() as u32;
        if let Some(notice) = self.play.import_audio(read, plan, tick, skipped, effects) {
            self.notice = Some(notice);
            self.audio.clip_panel = true;
        }
    }

    /// After every input: asks the audio thread about the song's files when the song, its
    /// audio table or the rate of the stream is another, and puts in what was brought.
    pub(super) fn audio_follow(&mut self, effects: &mut Vec<Effect>) {
        let serial = self.play.serial();
        if !self.play.busy() && !self.audio.brought.is_empty() {
            for (bringing, files) in std::mem::take(&mut self.audio.brought) {
                // Files for a song that is gone stay out.
                let before = self.play.undo_count();
                if bringing.serial == serial {
                    self.put_in(&bringing, files, effects);
                }
                let put_in = self.play.undo_count() != before;
                self.ai_brought(bringing.ticket, put_in);
            }
        }
        let rate = self.open.as_ref().map(|open| open.sample_rate);
        self.play.set_audio_rate(rate);
        let table = self.play.song().doc.audio.clone();
        let before = self.audio.watched.as_ref();
        if before
            .is_some_and(|w| w.serial == serial && Arc::ptr_eq(&w.table, &table) && w.rate == rate)
        {
            return;
        }
        // A song without audio after one without: the audio thread has nothing to hear.
        let nothing = table.is_empty() && before.is_none_or(|w| w.table.is_empty());
        if before.is_none_or(|w| w.serial != serial) {
            self.play.forget_audio_states();
            self.audio.missing_unsaid = Some(serial);
            self.audio.bringing.retain(|b| b.serial == serial);
        }
        if !nothing {
            effects.push(Effect::WatchAudio {
                serial,
                sources: table.to_vec(),
                sample_rate: rate,
            });
        }
        self.audio.watched = Some(Watched {
            serial,
            table,
            rate,
        });
    }

    /// 「曲から外す」 for an audio file that is not found.
    pub(super) fn remove_missing_audio(&mut self, audio: u32, effects: &mut Vec<Effect>) {
        if self.play.missing_audio().iter().any(|s| s.id.0 == audio) {
            self.play.remove_audio(audio, effects);
        }
    }

    /// The section `params`: 「選んだクリップ」 when exactly one clip is selected, it is a
    /// vocal clip, and no insert or instrument was selected after it; 「まとめて変える」 the
    /// same for two or more that are all vocal clips.
    pub(super) fn params_view(&self) -> ParamsView {
        let view = self.play.params_view();
        if let Some(clips) = self.play.selected_vocal_clips() {
            if !self.audio.clip_panel && view.selected.is_some() {
                return view;
            }
            return ParamsView {
                selected: None,
                clips: Some(clips),
            };
        }
        let Some((track, clip, source)) = self.play.selected_vocal_clip() else {
            return view;
        };
        if !self.audio.clip_panel && view.selected.is_some() {
            return view;
        }
        let missing = (self.play.audio_state_of(&source) == AudioClipState::Missing).then(|| {
            MissingAudioView {
                audio: source.id.0,
                file: source.file.clone(),
                was_at: self
                    .song_folder_name()
                    .map(|folder| format!("{folder}/{SONG_AUDIO}/{}", source.file)),
            }
        });
        ParamsView {
            selected: Some(SelectedParams::Clip(SelectedClip {
                track: track.0,
                clip: clip.0,
                missing,
            })),
            clips: None,
        }
    }

    /// The name of the song's folder; `None` for a song that was never saved.
    fn song_folder_name(&self) -> Option<String> {
        let folder = self.files.path.as_deref()?.parent()?.file_name()?;
        Some(folder.to_string_lossy().into_owned())
    }

    /// The folder of the song's file, for the places the audio is looked for in.
    pub fn song_dir(&self) -> Option<PathBuf> {
        Some(self.files.path.as_deref()?.parent()?.to_owned())
    }

    /// `Play::serial`: another number is another song.
    pub fn song_serial(&self) -> u32 {
        self.play.serial()
    }
}

#[cfg(test)]
mod tests;
