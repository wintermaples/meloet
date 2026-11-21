//! Vocal clips and the song's audio files, as far as they change the song: cutting clips
//! (S), the values of a clip, files brought in, a file taken out. And what the song's
//! audio files are right now (found, being read, missing), which the song itself does not
//! say (docs/architecture.md 22-10, 22-18 A2).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::arrange::{self, ClipsSplit};
use meloet_model::{
    AudioId, AudioSource, ClipGain, ClipId, EditError, FadeEnd, ImportPlan, ImportedAudio,
    ImportedClip, MAX_TICK, TrackId,
};
use meloet_protocol::{
    AudioClipState, AudioJob, AudioJobKind, AudioWorkView, ClipField, SOURCE_BPM_MAX,
    SOURCE_BPM_MIN, StatusMessage, StretchChange,
};

use super::{ClipPick, Play};
use crate::audiofiles::keeper::{AudioNews, Growing, SourceState};
use crate::maker::MakerJob;
use crate::peaks::PeaksPlace;
use crate::state::Effect;

/// What the thread that reads audio files in said of the files of the song.
#[derive(Debug, Default)]
pub(super) struct AudioBook {
    /// By `AudioSource::uid`. A file that is not in here counts as ready: nothing was said
    /// of it yet.
    states: HashMap<[u8; 16], SourceState>,
    /// The files that are `Loading`, in the order they were started, with how far they are.
    loading: Vec<([u8; 16], u8)>,
    growing: HashMap<[u8; 16], Growing>,
    /// The clips whose audio is being made or waits to be (22-19 pass 2), as the maker said.
    making: Vec<MakerJob>,
    /// The rate the engine runs at; `None` without a stream.
    rate: Option<u32>,
    /// `<cache>/peaks`.
    peaks_dir: Option<PathBuf>,
}

impl Play {
    pub fn set_peaks_dir(&mut self, dir: Option<PathBuf>) {
        self.audio.peaks_dir = dir;
    }

    /// The rate the engine runs at now.
    pub fn set_audio_rate(&mut self, rate: Option<u32>) {
        self.audio.rate = rate;
    }

    /// Another song: what was said of the files of the one before is forgotten.
    pub fn forget_audio_states(&mut self) {
        self.audio.states.clear();
        self.audio.loading.clear();
        self.audio.growing.clear();
        self.audio.making.clear();
    }

    /// What the thread that makes audio for clips is at.
    pub fn set_audio_making(&mut self, jobs: Vec<MakerJob>) {
        self.audio.making = jobs;
    }

    /// Returns whether the clips look different now.
    pub fn audio_news(&mut self, news: AudioNews) -> bool {
        let book = &mut self.audio;
        match news {
            AudioNews::States { states, .. } => {
                let new: HashMap<_, _> = states.into_iter().collect();
                book.loading
                    .retain(|(uid, _)| new.get(uid) == Some(&SourceState::Loading));
                for (uid, state) in &new {
                    let listed = book.loading.iter().any(|(u, _)| u == uid);
                    if *state == SourceState::Loading && !listed {
                        book.loading.push((*uid, 0));
                    }
                }
                book.growing.retain(|uid, _| new.contains_key(uid));
                let changed = book.states != new;
                book.states = new;
                changed
            }
            AudioNews::Peaks { uid, peaks } => {
                book.growing.insert(uid, peaks);
                true
            }
            AudioNews::Progress { uid, percent } => {
                match book.loading.iter_mut().find(|(u, _)| *u == uid) {
                    Some(entry) if entry.1 != percent => {
                        entry.1 = percent;
                        true
                    }
                    _ => false,
                }
            }
            AudioNews::Done { uid, ok } => {
                book.growing.remove(&uid);
                book.loading.retain(|(u, _)| *u != uid);
                // A file that did not decode is as good as not there.
                let state = match (ok, book.states.get(&uid)) {
                    (false, Some(SourceState::Loading)) => SourceState::Missing,
                    (_, Some(SourceState::Missing)) => SourceState::Missing,
                    _ => SourceState::Ready,
                };
                book.states.insert(uid, state);
                true
            }
            AudioNews::Brought { .. } | AudioNews::Found { .. } => false,
        }
    }

    pub(super) fn audio_state(&self, source: &AudioSource) -> AudioClipState {
        match self.audio.states.get(&source.uid) {
            Some(SourceState::Missing) => AudioClipState::Missing,
            Some(SourceState::Loading) => AudioClipState::Loading,
            Some(SourceState::Ready) | None => AudioClipState::Ready,
        }
    }

    /// The file's rate is not the one the audio runs at.
    pub(super) fn audio_resampled(&self, source: &AudioSource) -> bool {
        self.audio
            .rate
            .is_some_and(|rate| rate != source.sample_rate)
    }

    /// The files of the song as it is now that are not found, in the order of the table.
    pub fn missing_audio(&self) -> Vec<Arc<AudioSource>> {
        (self.doc().audio.iter())
            .filter(|s| self.audio_state(s) == AudioClipState::Missing)
            .cloned()
            .collect()
    }

    /// The section `audioWork`.
    pub fn audio_work_view(&self) -> AudioWorkView {
        let doc = self.doc();
        let loading = (self.audio.loading.iter()).flat_map(|&(uid, percent)| {
            (doc.audio.iter().filter(move |s| s.uid == uid)).map(move |s| AudioJob {
                kind: AudioJobKind::Loading,
                audio: s.id.0,
                clip: None,
                percent,
            })
        });
        // A clip that is gone since the maker said so is not shown.
        let making = (self.audio.making.iter())
            .filter(|job| {
                (doc.clip(ClipId(job.clip)))
                    .and_then(|(_, c)| c.active_audio())
                    .is_some_and(|shown| shown.take.source.id.0 == job.audio)
            })
            .map(|job| AudioJob {
                kind: AudioJobKind::Rendering,
                audio: job.audio,
                clip: Some(job.clip),
                percent: job.percent,
            });
        AudioWorkView {
            jobs: loading.chain(making).collect(),
        }
    }

    /// Where the peaks of the file are: being made, or in their file.
    pub fn peaks_place(&self, audio: u32) -> Option<PeaksPlace> {
        let source = self.doc().audio(AudioId(audio))?;
        if let Some(growing) = self.audio.growing.get(&source.uid) {
            return Some(PeaksPlace::Growing(growing.0.clone()));
        }
        let dir = self.audio.peaks_dir.as_deref()?;
        Some(PeaksPlace::File(crate::peaks::peaks_path(dir, &source.uid)))
    }

    /// The one selected clip when it is a vocal clip, with its track and the file of its
    /// shown take.
    pub fn selected_vocal_clip(&self) -> Option<(TrackId, ClipId, Arc<AudioSource>)> {
        let [id] = self.clips_sel[..] else {
            return None;
        };
        let (track, clip) = self.doc().clip(id)?;
        let take = clip.active_audio()?;
        Some((track.id, id, take.take.source.clone()))
    }

    /// S. Returns the notice to show.
    pub fn split_clips(&mut self, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let at = self.play_line();
        let doc = self.doc();
        let clips: Vec<ClipId> = if self.clips_sel.is_empty() {
            let track = doc.track(self.song.session.play_target?)?;
            (track.clips.iter())
                .find(|c| c.start < at && at < c.end)
                .map(|c| vec![c.id])?
        } else {
            self.clips_sel.clone()
        };
        let ClipsSplit { edit, left, right } = arrange::split_clips(doc, &clips, at).ok()?;
        let name = doc.clip(left[0]).map(|(_, c)| c.name.clone())?;
        let count = left.len() as u32;
        let mut picked = right.clone();
        picked.sort_unstable();
        let pick = ClipPick {
            clicked: match right[..] {
                [one] => Some(one),
                _ => None,
            },
            clips: picked,
            shown: None,
        };
        self.apply_selecting(edit, None, Some(pick), effects);
        Some(StatusMessage::ClipSplit {
            name,
            tick: at,
            count,
        })
    }

    /// `Command::SetClipField`. Returns the notice to show.
    pub fn set_clip_field(
        &mut self,
        clip: u32,
        field: ClipField,
        value: f64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() || value.is_nan() {
            return None;
        }
        let id = ClipId(clip);
        // A value outside its range is put into it, without a word.
        let ticks = value.round().clamp(0.0, MAX_TICK as f64) as u64;
        let doc = self.doc();
        let edited = match field {
            ClipField::Start => {
                let (_, c) = doc.clip(id)?;
                let last = MAX_TICK - (c.end - c.start);
                arrange::set_clip_start(doc, id, ticks.min(last)).map(|e| e.edit)
            }
            ClipField::Length => arrange::set_clip_length(doc, id, ticks),
            ClipField::GainDb => meloet_model::set_clip_gain(doc, id, ClipGain::from_db(value)?),
            ClipField::FadeIn => meloet_model::set_clip_fade(doc, id, FadeEnd::In, ticks),
            ClipField::FadeOut => meloet_model::set_clip_fade(doc, id, FadeEnd::Out, ticks),
        };
        match edited {
            Ok(e) => {
                self.apply(e, effects);
                None
            }
            Err(EditError::Overlap) => Some(StatusMessage::ClipsNotMoved),
            Err(_) => None,
        }
    }

    /// `Command::SetClipStretch`: one row of 「のび縮み ・ 音の高さ」, one step.
    pub fn set_clip_stretch(
        &mut self,
        clip: u32,
        change: StretchChange,
        effects: &mut Vec<Effect>,
    ) {
        if self.busy() {
            return;
        }
        let id = ClipId(clip);
        let doc = self.doc();
        // The nearest whole number; the model puts it into its range.
        let whole = |v: f64| (!v.is_nan()).then(|| v.round().clamp(-1e6, 1e6) as i32);
        let edited = match change {
            StretchChange::Follow { on } => meloet_model::set_clip_follow(doc, id, on),
            StretchChange::KeepFormant { on } => meloet_model::set_clip_keep_formant(doc, id, on),
            StretchChange::Semitones { semitones } => match whole(semitones) {
                Some(n) => meloet_model::set_clip_semitones(doc, id, n),
                None => return,
            },
            StretchChange::Cents { cents } => match whole(cents) {
                Some(n) => meloet_model::set_clip_cents(doc, id, n),
                None => return,
            },
            StretchChange::SourceBpm { bpm } => {
                let tempo = (!bpm.is_nan())
                    .then(|| bpm.clamp(SOURCE_BPM_MIN, SOURCE_BPM_MAX))
                    .and_then(meloet_model::Tempo::from_bpm);
                match tempo {
                    Some(tempo) => meloet_model::set_source_tempo(doc, id, tempo).map(|(e, _)| e),
                    None => return,
                }
            }
        };
        if let Ok(e) = edited {
            self.apply(e, effects);
        }
    }

    /// `Command::FindSourceTempo`. Returns the notice to show.
    pub fn find_source_tempo(
        &mut self,
        clip: u32,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let id = ClipId(clip);
        let doc = self.doc();
        let guess = meloet_model::guess_source_tempo(doc, id).ok()?;
        let name = doc.clip(id)?.1.name.clone();
        let Some(guess) = guess else {
            return Some(StatusMessage::SourceTempoNotFound { name });
        };
        match meloet_model::set_source_tempo(doc, id, guess.tempo) {
            Ok((e, _)) => self.apply(e, effects),
            Err(EditError::Unchanged) => {}
            Err(_) => return None,
        }
        Some(StatusMessage::SourceTempoFound {
            name,
            bpm: guess.tempo.bpm(),
            bars: guess.bars,
        })
    }

    /// Where the play line is: where files brought in from the menu go.
    pub fn play_line_tick(&self) -> u64 {
        self.play_line()
    }

    pub fn audio_state_of(&self, source: &AudioSource) -> AudioClipState {
        self.audio_state(source)
    }

    /// Audio files that were brought in go into the song, as one step; the clips made are
    /// selected. `skipped`: files that came with them and did not read. Returns the notice
    /// to show; `None` when nothing went in.
    pub fn import_audio(
        &mut self,
        files: Vec<ImportedAudio>,
        plan: ImportPlan,
        tick: u64,
        skipped: u32,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        // The track the plan names may be gone, or a keys track: a new vocal track then.
        let plan = match plan {
            ImportPlan::InRow(Some(track))
                if !(self.doc().track(track)).is_some_and(|t| t.kind.is_vocal()) =>
            {
                ImportPlan::InRow(None)
            }
            plan => plan,
        };
        let (edit, made): (_, Vec<ImportedClip>) =
            meloet_model::import_audio(self.doc(), files, plan, tick).ok()?;
        if edit.doc.tracks.len() > super::MAX_TRACKS {
            return None;
        }
        let mut clips: Vec<ClipId> = made.iter().map(|m| m.clip).collect();
        clips.sort_unstable();
        let clicked = match made[..] {
            [one] => Some(one.clip),
            _ => None,
        };
        let pick = ClipPick {
            clips,
            clicked,
            shown: None,
        };
        self.apply_selecting(edit, None, Some(pick), effects);
        let first = made[0];
        Some(StatusMessage::AudioImported {
            file: self.doc().audio(first.audio)?.file.clone(),
            track: self.track_ref(first.track)?,
            tick: first.start,
            count: made.len() as u32,
            skipped,
        })
    }

    /// 「曲から外す」 for an audio file. Returns whether the song changed.
    pub fn remove_audio(&mut self, audio: u32, effects: &mut Vec<Effect>) -> bool {
        if self.busy() {
            return false;
        }
        match meloet_model::remove_audio(self.doc(), AudioId(audio)) {
            Ok(e) => {
                self.apply(e, effects);
                true
            }
            Err(_) => false,
        }
    }
}
