//! Vocal clips and the song's audio files, as far as they change the song: cutting clips
//! (S), the values of a clip, files brought in, a file taken out. And what the song's
//! audio files are right now (found, being read, missing), which the song itself does not
//! say (docs/architecture.md 22-10, 22-18 A2).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::arrange::{self, ClipsSplit};
use meloet_model::history::StepKind;
use meloet_model::{
    AudioId, AudioShape, AudioSource, ClipGain, ClipId, EditError, FadeEnd, ImportPlan,
    ImportedAudio, ImportedClip, MAX_TICK, TrackId,
};
use meloet_protocol::{
    AudioClipState, AudioJob, AudioJobKind, AudioWorkView, ClipField, SOURCE_BPM_MAX,
    SOURCE_BPM_MIN, SelectedClips, StatusMessage, StretchChange,
};

use super::{ClipPick, Play};
use crate::audiofiles::keeper::{AudioNews, Growing, SourceState};
use crate::maker::MakerJob;
use crate::peaks::PeaksPlace;
use crate::state::Effect;

/// The volume of `Command::ShiftClipGains` and `Command::SetClipGains`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipGains {
    By(f64),
    To(f64),
}

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
            AudioNews::Brought { .. }
            | AudioNews::Found { .. }
            | AudioNews::Measured { .. }
            | AudioNews::Reversed { .. }
            | AudioNews::Adopted { .. } => false,
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
            // The rules hold 「音のクリップにする」 (`state/bounce.rs`).
            bounce: None,
        }
    }

    /// 「さがす…」 took a file for `audio`: "歌 テイク 1.wav を見つけました。2 つのクリップが
    /// また鳴ります。" (spec 6), with the clips whose shown take plays it.
    pub fn audio_found(&self, audio: u32) -> Option<StatusMessage> {
        let doc = self.doc();
        let file = doc.audio(AudioId(audio))?.file.clone();
        let clips = (doc.tracks.iter().flat_map(|t| &t.clips))
            .filter(|c| (c.active_audio()).is_some_and(|a| a.take.source.id.0 == audio))
            .count() as u32;
        Some(StatusMessage::AudioFound { file, clips })
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
        self.split_at(&clips, at, effects)
    }

    /// Cuts the clips at `at` as one step, the right halves selected afterwards. Returns the
    /// notice to show.
    pub(super) fn split_at(
        &mut self,
        clips: &[ClipId],
        at: u64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let doc = self.doc();
        let ClipsSplit { edit, left, right } = arrange::split_clips(doc, clips, at).ok()?;
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
            // round12 stub: M7
            StretchChange::TuneStrength { .. } | StretchChange::TuneScale => return,
        };
        if let Ok(e) = edited {
            self.apply(e, effects);
        }
    }

    /// 「まとめて変える」 (spec 3, 40-A): two or more clips are selected and every one is a
    /// vocal clip. A value is `None` when the clips do not all have the same one.
    pub fn selected_vocal_clips(&self) -> Option<SelectedClips> {
        if self.clips_sel.len() < 2 {
            return None;
        }
        let mut clips = Vec::new();
        let mut tracks = Vec::new();
        let mut shapes: Vec<AudioShape> = Vec::new();
        for track in &self.doc().tracks {
            for clip in track
                .clips
                .iter()
                .filter(|c| self.clips_sel.contains(&c.id))
            {
                // A keys clip among them (B-95-8).
                shapes.push(*clip.takes.shape()?);
                clips.push(clip.id.0);
                if tracks.last() != Some(&track.id.0) {
                    tracks.push(track.id.0);
                }
            }
        }
        if clips.len() < 2 {
            return None;
        }
        fn same<T: PartialEq>(shapes: &[AudioShape], of: impl Fn(&AudioShape) -> T) -> Option<T> {
            let first = of(&shapes[0]);
            shapes[1..].iter().all(|s| of(s) == first).then_some(first)
        }
        Some(SelectedClips {
            clips,
            tracks,
            gain_db: same(&shapes, |s| s.gain).map(|g| g.db() as f32),
            follow: same(&shapes, |s| s.follow),
            semitones: same(&shapes, |s| s.semitones),
            cents: same(&shapes, |s| s.cents),
            keep_formant: same(&shapes, |s| s.keep_formant),
        })
    }

    /// `Command::ShiftClipGains` (39-B) and `Command::SetClipGains` (40-A): the volume of
    /// every vocal clip of `clips`, moved by `by_db` or set to `db`, as one step. Each stops
    /// at the ends of its range.
    pub fn set_clip_gains(&mut self, clips: &[u32], gain: ClipGains, effects: &mut Vec<Effect>) {
        let usable = match gain {
            ClipGains::By(db) => db != 0.0 && !db.is_nan(),
            ClipGains::To(db) => !db.is_nan(),
        };
        if self.busy() || !usable {
            return;
        }
        let change = |s: &AudioShape| {
            let db = match gain {
                ClipGains::By(by) => s.gain.db() + by,
                ClipGains::To(to) => to,
            };
            AudioShape {
                gain: ClipGain::from_db(db).unwrap_or(s.gain),
                ..*s
            }
        };
        self.set_clips_shape(
            clips,
            change,
            |count| StepKind::SetClipsGain { count },
            effects,
        );
    }

    /// `Command::SetClipsStretch`: one row of 「まとめて変える」 for every vocal clip of
    /// `clips`, as one step. 「この音のテンポ」 is for one clip only: nothing.
    pub fn set_clips_stretch(
        &mut self,
        clips: &[u32],
        change: StretchChange,
        effects: &mut Vec<Effect>,
    ) {
        if self.busy() {
            return;
        }
        let whole = |v: f64| (!v.is_nan()).then(|| v.round().clamp(-1e6, 1e6) as i32);
        let semis = i32::from(AudioShape::SEMITONES_MAX);
        let cents = i32::from(AudioShape::CENTS_MAX);
        match change {
            StretchChange::Follow { on } => self.set_clips_shape(
                clips,
                |s| AudioShape { follow: on, ..*s },
                |count| StepKind::SetClipsFollow { count },
                effects,
            ),
            StretchChange::KeepFormant { on } => self.set_clips_shape(
                clips,
                |s| AudioShape {
                    keep_formant: on,
                    ..*s
                },
                |count| StepKind::SetClipsPitch { count },
                effects,
            ),
            StretchChange::Semitones { semitones } => {
                let Some(n) = whole(semitones) else {
                    return;
                };
                self.set_clips_shape(
                    clips,
                    |s| AudioShape {
                        semitones: n.clamp(-semis, semis) as i8,
                        ..*s
                    },
                    |count| StepKind::SetClipsPitch { count },
                    effects,
                );
            }
            StretchChange::Cents { cents: c } => {
                let Some(n) = whole(c) else {
                    return;
                };
                self.set_clips_shape(
                    clips,
                    |s| AudioShape {
                        cents: n.clamp(-cents, cents) as i8,
                        ..*s
                    },
                    |count| StepKind::SetClipsPitch { count },
                    effects,
                );
            }
            StretchChange::SourceBpm { .. }
            | StretchChange::TuneStrength { .. }
            | StretchChange::TuneScale => {}
        }
    }

    fn set_clips_shape(
        &mut self,
        clips: &[u32],
        change: impl Fn(&AudioShape) -> AudioShape,
        step: impl FnOnce(usize) -> StepKind,
        effects: &mut Vec<Effect>,
    ) {
        let ids: Vec<ClipId> = clips.iter().map(|&c| ClipId(c)).collect();
        if let Ok(e) = meloet_model::set_clips_shape(self.doc(), &ids, change, step) {
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
        let mut same = false;
        match meloet_model::set_source_tempo(doc, id, guess.tempo) {
            Ok((e, _)) => self.apply(e, effects),
            Err(EditError::Unchanged) => same = true,
            Err(_) => return None,
        }
        Some(StatusMessage::SourceTempoFound {
            name,
            bpm: guess.tempo.bpm(),
            bars: guess.bars,
            same,
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
