//! 「音のクリップにする」 and 「このトラックを音にする」 (round 12, A7) as the rules see them. One
//! at a time, a job of the export (with ids of its own) renders the track alone into a file
//! of the work place; the thread that reads audio files in gives the file its name and its
//! peaks; then the step is made, once the song may change, if the song and the track are
//! still there. What is not used is taken out of the work place.

use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::NewAudio;
use meloet_protocol::{
    BounceView, ExportFailure, ExportTail, RecordFormat, StatusMessage, TrackRef,
};

use super::{Effect, Model};
use crate::export::{ExportAsk, ExportJob, ExportNews};
use crate::play::MAX_TRACKS;
use crate::play::bounce::{BounceRange, bounce_doc};

/// The ids of the export's jobs that make audio of a track start here, far from the rules'
/// own and below the AI link's.
pub(crate) const BOUNCE_JOBS: u64 = 1 << 47;
/// Where the AI link's jobs start (`service/export.rs`).
const AI_JOBS: u64 = 1 << 48;

#[derive(Debug)]
enum Phase {
    /// The job renders, then writes the file into the work place.
    Rendering,
    /// The file is in the work place under a name of its own: the thread that reads audio
    /// files in is to be asked to take it.
    Written(PathBuf),
    Taking,
    /// The audio is the song's to use: the step waits until the song may change.
    Made(NewAudio),
}

#[derive(Debug)]
struct Bouncing {
    job: u64,
    /// `Play::serial` of the song.
    serial: u32,
    range: BounceRange,
    /// The track as it was when the job started.
    shown: TrackRef,
    /// The name the file is asked for.
    wanted: String,
    percent: u8,
    phase: Phase,
}

#[derive(Debug, Default)]
pub(super) struct BounceSide {
    last: u64,
    now: Option<Bouncing>,
    /// Names of files in the work place that nothing uses, to take out.
    unused: Vec<String>,
}

/// What the job is to write into the work place, as the export writes a file.
#[derive(Debug, Clone, PartialEq)]
pub struct BounceAsk {
    pub job: Box<ExportJob>,
    /// The settings' 「録音の形」.
    pub bits: RecordFormat,
}

fn name_of(path: &std::path::Path) -> Option<String> {
    Some(path.file_name()?.to_string_lossy().into_owned())
}

impl Model {
    pub(super) fn bouncing(&self) -> bool {
        self.bounce.now.is_some()
    }

    /// 「音のクリップにする」 of the keys clips among `clips` (of one track).
    pub(super) fn bounce_clips(&mut self, clips: &[u32], effects: &mut Vec<Effect>) {
        if self.play.busy() {
            return;
        }
        if let Some(range) = self.play.bounce_clips_range(clips) {
            self.start_bounce(range, effects);
        }
    }

    /// 「このトラックを音にする」.
    pub(super) fn bounce_whole_track(&mut self, track: u32, effects: &mut Vec<Effect>) {
        if self.play.busy() {
            return;
        }
        match self.play.bounce_track_range(track) {
            None => {}
            Some(None) => self.notice = Some(StatusMessage::NothingToBounce),
            Some(Some(range)) => self.start_bounce(range, effects),
        }
    }

    fn start_bounce(&mut self, range: BounceRange, effects: &mut Vec<Effect>) {
        if self.bouncing() || self.export.dialog_open() || self.export.running() {
            self.notice = Some(StatusMessage::BounceBusy);
            return;
        }
        let song = &self.play.song().doc;
        if song.tracks.len() >= MAX_TRACKS {
            self.notice = Some(StatusMessage::TooManyTracks);
            return;
        }
        let Some(at) = song.tracks.iter().position(|t| t.id == range.track) else {
            return;
        };
        let Some(doc) = bounce_doc(song, range.track) else {
            return;
        };
        let shown = TrackRef {
            number: at as u32 + 1,
            name: song.tracks[at].name.clone(),
        };
        self.bounce.last += 1;
        let job = BOUNCE_JOBS + self.bounce.last;
        let scene = self.export_scene();
        let ask = ExportJob {
            id: job,
            doc: Arc::new(doc),
            span: range.span,
            per_track: false,
            tail: ExportTail::UntilSilent,
            realtime: false,
            skip: scene.stopped_plugins.iter().map(|p| p.slot).collect(),
            sample_rate: scene.sample_rate,
            measure: None,
        };
        effects.push(Effect::Bounce(BounceAsk {
            job: Box::new(ask),
            bits: self.sound.record_format,
        }));
        self.bounce.now = Some(Bouncing {
            job,
            serial: self.play.serial(),
            range,
            shown,
            wanted: self.play.bounce_file_name(range.track),
            percent: 0,
            phase: Phase::Rendering,
        });
    }

    /// 「やめる」.
    pub(super) fn cancel_bounce(&mut self, effects: &mut Vec<Effect>) {
        let Some(bouncing) = self.bounce.now.take() else {
            return;
        };
        match bouncing.phase {
            Phase::Rendering => effects.push(Effect::Export(ExportAsk::Drop { job: bouncing.job })),
            Phase::Written(path) => self.bounce.unused.extend(name_of(&path)),
            // The answer is let go of when it comes.
            Phase::Taking => {}
            Phase::Made(audio) => self.bounce.unused.push(audio.file),
        }
        self.notice = Some(StatusMessage::BounceCancelled);
    }

    /// News of a job of the export. Returns whether it was a bounce's.
    pub(super) fn bounce_export_news(&mut self, news: &ExportNews) -> bool {
        let job = match news {
            ExportNews::Counted { job, .. }
            | ExportNews::MustBeRealtime { job }
            | ExportNews::Finished { job, .. }
            | ExportNews::Failed { job, .. }
            | ExportNews::Measured { job, .. } => *job,
            ExportNews::PlacePicked(_) | ExportNews::PicturePicked(_) => return false,
        };
        if !(BOUNCE_JOBS..AI_JOBS).contains(&job) {
            return false;
        }
        let now = self.bounce.now.as_mut().filter(|b| b.job == job);
        match (news, now) {
            (ExportNews::Counted { percent, .. }, Some(b)) => b.percent = (*percent).min(99),
            (ExportNews::Finished { files, .. }, Some(b))
                if matches!(b.phase, Phase::Rendering) =>
            {
                match files.first() {
                    Some(path) => {
                        b.percent = 99;
                        b.phase = Phase::Written(path.clone());
                    }
                    None => {
                        self.bounce.now = None;
                        self.notice = Some(StatusMessage::BounceFailed {
                            reason: ExportFailure::CannotWrite,
                        });
                    }
                }
            }
            // Of a job that was let go of meanwhile.
            (ExportNews::Finished { files, .. }, _) => {
                let names = files.iter().filter_map(|f| name_of(f));
                self.bounce.unused.extend(names);
            }
            (ExportNews::Failed { reason, .. }, Some(_)) => {
                self.bounce.now = None;
                self.notice = Some(StatusMessage::BounceFailed {
                    reason: reason.clone(),
                });
            }
            _ => {}
        }
        true
    }

    /// The worker did not start the job: the AI link's export runs.
    pub(super) fn bounce_refused(&mut self, job: u64) {
        if self.bounce.now.take_if(|b| b.job == job).is_some() {
            self.notice = Some(StatusMessage::BounceBusy);
        }
    }

    /// The answer of the thread that reads audio files in.
    pub(super) fn bounce_taken(&mut self, ticket: u64, audio: Option<NewAudio>) {
        let now = (self.bounce.now.as_mut())
            .filter(|b| b.job == ticket && matches!(b.phase, Phase::Taking));
        match (now, audio) {
            (Some(b), Some(audio)) => b.phase = Phase::Made(audio),
            (Some(_), None) => {
                self.bounce.now = None;
                self.notice = Some(StatusMessage::BounceFailed {
                    reason: ExportFailure::CannotWrite,
                });
            }
            (None, Some(audio)) => self.bounce.unused.push(audio.file),
            (None, None) => {}
        }
    }

    /// After every input: asks what is to be asked, makes the step once the song may change,
    /// and takes out of the work place what nothing uses.
    pub(super) fn bounce_follow(&mut self, effects: &mut Vec<Effect>) {
        let free = !self.play.busy() && !self.ai_batch_open();
        if let Some(b) = &mut self.bounce.now {
            match std::mem::replace(&mut b.phase, Phase::Taking) {
                Phase::Written(temp) => {
                    let doc = &self.play.song().doc;
                    effects.push(Effect::TakeBounced {
                        ticket: b.job,
                        temp,
                        wanted: b.wanted.clone(),
                        taken: doc.audio.iter().map(|s| s.file.clone()).collect(),
                    });
                }
                Phase::Made(audio) if free => {
                    let b = self.bounce.now.take().expect("matched above");
                    self.bounce_in(b, audio, effects);
                }
                phase => b.phase = phase,
            }
        }
        if !self.bounce.unused.is_empty() {
            let names = std::mem::take(&mut self.bounce.unused);
            effects.push(Effect::DropVocalFiles(names));
        }
    }

    fn bounce_in(&mut self, b: Bouncing, audio: NewAudio, effects: &mut Vec<Effect>) {
        if b.serial != self.play.serial() {
            self.bounce.unused.push(audio.file);
            self.notice = Some(StatusMessage::BounceGone);
            return;
        }
        let before = self.play.undo_count();
        let file = audio.file.clone();
        let notice = (self.play).bounce_made(b.range.track, audio, b.range.span.start(), effects);
        if self.play.undo_count() == before {
            self.bounce.unused.push(file);
        }
        if notice.is_some() {
            self.notice = notice;
        }
    }

    /// `AudioWorkView::bounce`.
    pub(super) fn bounce_view(&self) -> Option<BounceView> {
        (self.bounce.now.as_ref()).map(|b| BounceView {
            track: b.shown.clone(),
            percent: b.percent.min(99),
        })
    }
}

#[cfg(test)]
mod tests;
