//! Writing the song out as an audio file: the rules of the dialog 「書き出し」 and of the run
//! in the status line (spec 3, 34-B; docs/architecture.md 22-11, 22-14, 22-20).
//!
//! [`Exporting`] is as pure as the rest of [`crate::state`]: intents and [`ExportNews`] come
//! in, [`ExportAsk`]s go out. The work itself (a second engine made from the copy of the
//! `Doc` in an [`ExportJob`], on a thread of its own, into a file of the work place) is
//! whoever takes the asks: it gets its plugins through [`crate::plugins::export`] and writes
//! through [`sink`].
//!
//! One job is one rendering of a range. It starts when the dialog opens, so that the check
//! has numbers to show, and 「書き出す」 keeps it and has it brought into the chosen form
//! ([`ExportAsk::Finish`]). Changing the range, the tail or the speed drops the job and
//! starts another; the format, the sample rate and the bits belong to the last step and
//! leave it running.

pub(crate) mod analysis;
#[cfg(test)]
pub(crate) mod fixture;
pub(crate) mod job;
#[cfg(test)]
mod kit_tests;
pub(crate) mod loudness;
pub(crate) mod render;
pub mod sink;
pub(crate) mod told;
pub(crate) mod wave;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use meloet_model::{Doc, TickRange, TrackId};
use meloet_protocol::{
    Command, ExportChange, ExportCheck, ExportDialogView, ExportFailure, ExportFormat,
    ExportLoudness, ExportRange, ExportRunView, ExportSolo, ExportStoppedPlugin, ExportTail,
    ExportView, PluginSlotRef, RecordFormat, TickSpan, TrackRef,
};

use crate::ai::{AiAnalysis, AiBandLayout};
use crate::plugins::shown_path;
use crate::settings::SAMPLE_RATE_CHOICES;

/// What the rules ask of the export's worker and of the shell.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportAsk {
    /// Start writing the job into the work place. Answer with [`ExportNews::Counted`] a few
    /// times a second, the last one with `done`, or with [`ExportNews::Failed`]. Audio that
    /// is still being made (`Core::made_counts().waiting`) is waited for first.
    Render(Box<ExportJob>),
    /// The job is not wanted any more, running or done: stop it, take its files out of the
    /// work place and end its plugin processes. Nothing is answered.
    Drop { job: u64 },
    /// 「書き出す」: once the job is rendered, bring it into the form of `target` and move
    /// it there. Answer with [`ExportNews::Finished`] or [`ExportNews::Failed`].
    Finish { job: u64, target: ExportTarget },
    /// The PC's window for saving a file, starting in `folder` with `name` filled in; with
    /// `folder_only` the one for choosing a folder. Answer with [`ExportNews::PlacePicked`].
    PickPlace {
        folder: Option<PathBuf>,
        name: String,
        folder_only: bool,
    },
}

/// One rendering, at the rate the audio runs at, as 32 bit floats.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportJob {
    pub id: u64,
    /// The song as it was when the job was asked for. What changes afterwards is not in it.
    pub doc: Arc<Doc>,
    pub span: TickRange,
    /// A rendering for each track, soloed, in the order of `doc.tracks`.
    pub per_track: bool,
    pub tail: ExportTail,
    /// No faster than it plays.
    pub realtime: bool,
    /// The plugins that have stopped: the export passes them by, as the playing does.
    pub skip: Vec<PluginSlotRef>,
    /// The rate the playing engine runs at.
    pub sample_rate: u32,
    /// For the AI link: the mix, then each track of [`Measuring::alone`], are rendered for
    /// their levels only, answered with [`ExportNews::Measured`]; no file is kept.
    pub measure: Option<Measuring>,
}

/// What a job of the AI link measures besides the mix (docs/architecture.md 24-12).
#[derive(Debug, Clone, PartialEq)]
pub struct Measuring {
    /// Rendered alone after the mix, in this order.
    pub alone: Vec<TrackId>,
    /// Those sound even when the song mutes them; the mix keeps the song's mutes.
    pub ignore_mute: bool,
    pub bands: Option<AiBandLayout>,
    /// Places in `alone` of the tracks whose every pair is compared band by band.
    pub compare: Vec<usize>,
    /// Ticks inside the span where sections after the first begin, rising.
    pub cuts: Vec<u64>,
}

/// Where a rendered job goes, and as what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportTarget {
    /// The file; for a job with `per_track` the folder its files go into, each named as
    /// its track.
    pub path: PathBuf,
    pub format: ExportFormat,
    pub sample_rate: u32,
    pub bits: RecordFormat,
    /// F1: with `Match` the job brings the file to the loudness of [`loudness::match_gain`],
    /// when the loudness it measured can be matched.
    pub loudness: ExportLoudness,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportNews {
    /// The answer to [`ExportAsk::PickPlace`]; `None` when the window was put away.
    PlacePicked(Option<PathBuf>),
    /// How far the rendering of `job` is.
    Counted {
        job: u64,
        /// 0..=100 of the frames to render, the tail not counted.
        percent: u8,
        /// The largest sample so far, in dBFS; `None`: silence.
        peak_db: Option<f32>,
        /// The places over 0 dB so far, of every file (docs/history/design-notes.ja.md B-95-2).
        over: u32,
        /// How long the audio is, the tail counted once `done`.
        length_s: u32,
        /// How long the rest takes, when there is something to guess from.
        remaining_s: Option<u32>,
        /// F1, of a job that renders one file and does not measure: the integrated loudness
        /// of the range so far, and the true peak of the file so far, the tail counted. `None`
        /// for the others, and for silence.
        lufs: Option<f32>,
        true_peak_db: Option<f32>,
        done: bool,
    },
    /// A plugin of the job says it cannot run faster than it plays: the job goes on at that
    /// speed.
    MustBeRealtime {
        job: u64,
    },
    /// The answer to [`ExportAsk::Finish`]: the files are where they were asked to be.
    Finished {
        job: u64,
        files: Vec<PathBuf>,
        /// E9: the waveform of the one file, as [`ExportRunView::Done::wave`]; empty for
        /// 「トラックごと」.
        wave: Vec<u8>,
    },
    Failed {
        job: u64,
        reason: ExportFailure,
    },
    /// The answer to a job with `measure`: the levels of each rendering, in its order.
    Measured {
        job: u64,
        passes: Vec<crate::ai::PassLevels>,
        /// With bands or sections asked for.
        analysis: Option<Box<AiAnalysis>>,
    },
}

/// What the rules need to know of the rest of meloet.
#[derive(Debug, Clone)]
pub struct Scene<'a> {
    pub doc: &'a Doc,
    /// `Play::serial`.
    pub serial: u32,
    /// The song's file, once it has one.
    pub song_path: Option<&'a Path>,
    /// The rate of the open stream, or else the one asked for.
    pub sample_rate: u32,
    /// Counting in or recording.
    pub busy: bool,
    pub stopped_plugins: Vec<ExportStoppedPlugin>,
    /// The soloed tracks, in the order of the tracks (spec 3, 42-C). Not in what is written.
    pub soloed: Vec<TrackRef>,
}

impl Scene<'_> {
    /// 「曲ぜんぶ」: from the start to where the last clip ends.
    fn song_span(&self) -> TickSpan {
        let clips = self.doc.tracks.iter().flat_map(|t| t.clips.iter());
        TickSpan {
            start: 0,
            end: clips.map(|c| c.end).max().unwrap_or(0),
        }
    }

    fn loop_span(&self) -> Option<TickSpan> {
        self.doc.loop_range.map(|r| TickSpan {
            start: r.start(),
            end: r.end(),
        })
    }

    fn tracks(&self) -> u32 {
        self.doc.tracks.len() as u32
    }
}

/// Folders the default of 「保存する所」 is made from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportDirs {
    /// Shown as "~".
    pub home: Option<PathBuf>,
    /// Where a song that has no file yet is written out to: the PC's folder for music.
    pub usual: Option<PathBuf>,
}

/// What a job was asked for; another one means another job.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    serial: u32,
    span: TickSpan,
    per_track: bool,
    tail: ExportTail,
    realtime: bool,
}

#[derive(Debug)]
struct Job {
    id: u64,
    asked: Asked,
    /// The last [`ExportNews::Counted`].
    percent: u8,
    peak_db: Option<f32>,
    over: u32,
    length_s: u32,
    remaining_s: Option<u32>,
    lufs: Option<f32>,
    true_peak_db: Option<f32>,
    counted: bool,
    done: bool,
    must_be_realtime: bool,
    failed: Option<ExportFailure>,
}

#[derive(Debug)]
struct Dialog {
    /// `None`: there is nothing to render (an empty song).
    job: Option<Job>,
    picking: bool,
}

#[derive(Debug)]
enum Run {
    Going { job: Job, target: ExportTarget },
    Ended(ExportRunView, Option<PathBuf>),
}

#[derive(Debug)]
pub struct Exporting {
    dirs: ExportDirs,
    // The choices stay for as long as meloet runs.
    range: ExportRange,
    format: ExportFormat,
    /// `None`: the rate the audio runs at.
    sample_rate: Option<u32>,
    bits: RecordFormat,
    tail: ExportTail,
    realtime: bool,
    advanced_open: bool,
    /// As chosen; while 「公開先に合わせる」 cannot be, the export is as with `AsIs`.
    loudness: ExportLoudness,
    /// Chosen with 「えらぶ…」: the folder, and the file's name without its ending.
    folder: Option<PathBuf>,
    name: Option<String>,
    dialog: Option<Dialog>,
    run: Option<Run>,
    last_job: u64,
}

impl Default for Exporting {
    fn default() -> Exporting {
        Exporting::new(ExportDirs::default())
    }
}

impl Exporting {
    pub fn new(dirs: ExportDirs) -> Exporting {
        Exporting {
            dirs,
            range: ExportRange::Song,
            format: ExportFormat::Wav,
            sample_rate: None,
            bits: RecordFormat::default(),
            tail: ExportTail::UntilSilent,
            realtime: false,
            advanced_open: false,
            loudness: ExportLoudness::AsIs,
            folder: None,
            name: None,
            dialog: None,
            run: None,
            last_job: 0,
        }
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn running(&self) -> bool {
        matches!(self.run, Some(Run::Going { .. }))
    }

    /// Before a command is done: a run that has ended is shown until the person does the
    /// next thing, and with it the dialog 「書き出しました」 (E9), which the screen closes
    /// with `Command::CloseExport`. `Command::ShowExported` and `Command::PlayExported` are
    /// about that run and leave it.
    pub fn saw_command(&mut self, command: &Command) {
        let stays = matches!(
            command,
            Command::SetViewport { .. }
                | Command::Audition { .. }
                | Command::AuditionOff
                | Command::ShowExported
                | Command::PlayExported
        );
        if !stays && matches!(self.run, Some(Run::Ended(..))) {
            self.run = None;
        }
    }

    /// The export's commands; any other does nothing.
    pub fn command(&mut self, command: &Command, scene: &Scene, asks: &mut Vec<ExportAsk>) {
        match command {
            Command::OpenExport => {
                if self.dialog.is_none() && !self.running() && !scene.busy {
                    if self.range == ExportRange::Loop && scene.loop_span().is_none() {
                        self.range = ExportRange::Song;
                    }
                    self.dialog = Some(Dialog {
                        job: None,
                        picking: false,
                    });
                }
            }
            Command::CloseExport => self.close(asks),
            Command::SetExport { change } => self.set(*change, scene),
            Command::ChooseExportPlace => {
                let per_track = self.range == ExportRange::Tracks;
                let (folder, name) = (self.folder_for(scene), self.file_name(scene));
                if let Some(dialog) = self.dialog.as_mut().filter(|d| !d.picking) {
                    dialog.picking = true;
                    asks.push(ExportAsk::PickPlace {
                        folder,
                        name,
                        folder_only: per_track,
                    });
                }
            }
            Command::StartExport => self.start(scene, asks),
            Command::CancelExport => {
                if let Some(Run::Going { job, .. }) = &self.run {
                    asks.push(ExportAsk::Drop { job: job.id });
                    self.run = Some(Run::Ended(ExportRunView::Cancelled, None));
                }
            }
            _ => {}
        }
    }

    /// What 「フォルダをひらく」 shows in the PC's file manager.
    pub fn exported(&self) -> Option<&Path> {
        match &self.run {
            Some(Run::Ended(ExportRunView::Done { .. }, shown)) => shown.as_deref(),
            _ => None,
        }
    }

    /// What 「PC のプレーヤーで聞く」 opens: the one file of a run that is done, not those
    /// of 「トラックごと」 (they have no waveform).
    pub fn played(&self) -> Option<&Path> {
        match &self.run {
            Some(Run::Ended(ExportRunView::Done { wave, .. }, shown)) if !wave.is_empty() => {
                shown.as_deref()
            }
            _ => None,
        }
    }

    fn close(&mut self, asks: &mut Vec<ExportAsk>) {
        if let Some(dialog) = self.dialog.take()
            && let Some(job) = dialog.job
        {
            asks.push(ExportAsk::Drop { job: job.id });
        }
    }

    fn set(&mut self, change: ExportChange, scene: &Scene) {
        if self.dialog.is_none() {
            return;
        }
        match change {
            ExportChange::Range { range } => {
                let usable = match range {
                    ExportRange::Song => true,
                    ExportRange::Loop => scene.loop_span().is_some(),
                    ExportRange::Tracks => scene.tracks() > 0,
                };
                if usable {
                    self.range = range;
                }
            }
            ExportChange::Format { format } => {
                self.format = format;
                if !format.bits_choices().contains(&self.bits) {
                    self.bits = RecordFormat::default();
                }
            }
            ExportChange::AdvancedOpen { open } => self.advanced_open = open,
            ExportChange::SampleRate { hz } => {
                if self.sample_rate_choices(scene).contains(&hz) {
                    self.sample_rate = Some(hz);
                }
            }
            ExportChange::Bits { bits } => {
                if self.format.bits_choices().contains(&bits) {
                    self.bits = bits;
                }
            }
            ExportChange::Tail { tail } => self.tail = tail,
            ExportChange::Realtime { on } => self.realtime = on,
            ExportChange::Loudness { loudness } => {
                let job = self.dialog.as_ref().and_then(|d| d.job.as_ref());
                if loudness == ExportLoudness::AsIs || self.can_match(job) {
                    self.loudness = loudness;
                }
            }
        }
    }

    fn start(&mut self, scene: &Scene, asks: &mut Vec<ExportAsk>) {
        if !self.can_export(scene) {
            return;
        }
        let job = self.dialog.as_ref().and_then(|d| d.job.as_ref());
        let target = ExportTarget {
            path: self.place(scene),
            format: self.format,
            sample_rate: self.sample_rate(scene),
            bits: self.bits,
            loudness: self.loudness_of(job),
        };
        let Some(job) = self.dialog.take().and_then(|d| d.job) else {
            return;
        };
        asks.push(ExportAsk::Finish {
            job: job.id,
            target: target.clone(),
        });
        self.run = Some(Run::Going { job, target });
    }

    /// After anything that happened: the job of the open dialog is the one for what the
    /// dialog says now, and a dialog whose song is gone closes.
    pub fn settle(&mut self, scene: &Scene, asks: &mut Vec<ExportAsk>) {
        let wanted = self.asked(scene);
        let had = (self.dialog.as_ref()).and_then(|d| Some(&d.job.as_ref()?.asked));
        if had.is_some_and(|a| a.serial != scene.serial) {
            self.close(asks);
            return;
        }
        if had == wanted.as_ref() {
            return;
        }
        let Some(dialog) = &mut self.dialog else {
            return;
        };
        if let Some(old) = dialog.job.take() {
            asks.push(ExportAsk::Drop { job: old.id });
        }
        let Some(asked) = wanted else {
            return;
        };
        let Some(span) = TickRange::new(asked.span.start, asked.span.end) else {
            return;
        };
        self.last_job += 1;
        asks.push(ExportAsk::Render(Box::new(ExportJob {
            id: self.last_job,
            doc: Arc::new(scene.doc.clone()),
            span,
            per_track: asked.per_track,
            tail: asked.tail,
            realtime: asked.realtime,
            skip: scene.stopped_plugins.iter().map(|p| p.slot).collect(),
            sample_rate: scene.sample_rate,
            measure: None,
        })));
        dialog.job = Some(Job {
            id: self.last_job,
            asked,
            percent: 0,
            peak_db: None,
            over: 0,
            length_s: 0,
            remaining_s: None,
            lufs: None,
            true_peak_db: None,
            counted: false,
            done: false,
            must_be_realtime: false,
            failed: None,
        });
    }

    pub fn news(&mut self, news: ExportNews) {
        match news {
            ExportNews::PlacePicked(path) => {
                let Some(dialog) = self.dialog.as_mut().filter(|d| d.picking) else {
                    return;
                };
                dialog.picking = false;
                let Some(path) = path else {
                    return;
                };
                if self.range == ExportRange::Tracks {
                    self.folder = Some(path);
                } else if let Some(name) = path.file_stem() {
                    self.folder = path.parent().map(Path::to_path_buf);
                    self.name = Some(name.to_string_lossy().into_owned());
                }
            }
            ExportNews::Counted {
                job,
                percent,
                peak_db,
                over,
                length_s,
                remaining_s,
                lufs,
                true_peak_db,
                done,
            } => {
                if let Some(job) = self.job_mut(job) {
                    job.percent = percent.min(100);
                    job.peak_db = peak_db.filter(|db| !db.is_nan());
                    job.over = over;
                    job.length_s = length_s;
                    job.remaining_s = remaining_s;
                    job.lufs = lufs.filter(|l| l.is_finite());
                    job.true_peak_db = true_peak_db.filter(|db| db.is_finite());
                    job.counted = true;
                    job.done = done;
                }
            }
            ExportNews::MustBeRealtime { job } => {
                if let Some(job) = self.job_mut(job) {
                    job.must_be_realtime = true;
                }
            }
            ExportNews::Finished { job, files, wave } => {
                let Some(Run::Going { job: going, target }) = &self.run else {
                    return;
                };
                if going.id != job {
                    return;
                }
                let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
                let (lufs, true_peak_db) = match target.loudness {
                    _ if going.asked.per_track || files.len() != 1 => (None, None),
                    ExportLoudness::Match => {
                        match loudness::match_gain(going.lufs, going.true_peak_db) {
                            Some(m) => (Some(m.lufs), going.true_peak_db.map(|p| p + m.gain_db)),
                            None => (going.lufs, going.true_peak_db),
                        }
                    }
                    ExportLoudness::AsIs => (going.lufs, going.true_peak_db),
                };
                let view = ExportRunView::Done {
                    file: files.first().and_then(|p| name(p)).unwrap_or_default(),
                    files: files.len() as u32,
                    place: shown_path(&target.path, self.dirs.home.as_deref()),
                    length_s: going.length_s,
                    sample_rate: target.sample_rate,
                    bits: target.bits,
                    wave,
                    lufs,
                    true_peak_db,
                };
                let shown = files.into_iter().next();
                self.run = Some(Run::Ended(view, shown));
            }
            ExportNews::Measured { .. } => {}
            ExportNews::Failed { job, reason } => {
                if matches!(&self.run, Some(Run::Going { job: going, .. }) if going.id == job) {
                    self.run = Some(Run::Ended(ExportRunView::Failed { reason }, None));
                } else if let Some(job) = self.job_mut(job) {
                    job.failed = Some(reason);
                }
            }
        }
    }

    fn job_mut(&mut self, id: u64) -> Option<&mut Job> {
        let of_dialog = self.dialog.as_mut().and_then(|d| d.job.as_mut());
        let of_run = match &mut self.run {
            Some(Run::Going { job, .. }) => Some(job),
            _ => None,
        };
        of_dialog.or(of_run).filter(|j| j.id == id)
    }

    fn span(&self, scene: &Scene) -> TickSpan {
        match self.range {
            ExportRange::Loop => scene.loop_span().unwrap_or_else(|| scene.song_span()),
            ExportRange::Song | ExportRange::Tracks => scene.song_span(),
        }
    }

    /// What the job of the dialog is to render; `None`: nothing.
    fn asked(&self, scene: &Scene) -> Option<Asked> {
        let span = self.span(scene);
        let per_track = self.range == ExportRange::Tracks;
        (span.end > span.start && !(per_track && scene.tracks() == 0)).then_some(Asked {
            serial: scene.serial,
            span,
            per_track,
            tail: self.tail,
            realtime: self.realtime,
        })
    }

    fn sample_rate(&self, scene: &Scene) -> u32 {
        self.sample_rate.unwrap_or(scene.sample_rate)
    }

    fn sample_rate_choices(&self, scene: &Scene) -> Vec<u32> {
        let mut choices = SAMPLE_RATE_CHOICES.to_vec();
        if !choices.contains(&scene.sample_rate) {
            choices.push(scene.sample_rate);
            choices.sort_unstable();
        }
        choices
    }

    /// A song with a file is written out into its folder.
    fn folder_for(&self, scene: &Scene) -> Option<PathBuf> {
        let of_song = || {
            scene
                .song_path
                .and_then(Path::parent)
                .map(Path::to_path_buf)
        };
        (self.folder.clone())
            .or_else(of_song)
            .or_else(|| self.dirs.usual.clone())
            .or_else(|| self.dirs.home.clone())
    }

    fn file_name(&self, scene: &Scene) -> String {
        let name = self.name.as_deref().unwrap_or(&scene.doc.name);
        format!("{name}.{}", self.format.extension())
    }

    /// The file, or for 「トラックごと」 the folder.
    fn place(&self, scene: &Scene) -> PathBuf {
        let folder = self.folder_for(scene).unwrap_or_default();
        match self.range {
            ExportRange::Tracks => folder,
            ExportRange::Song | ExportRange::Loop => folder.join(self.file_name(scene)),
        }
    }

    fn check(&self, job: Option<&Job>) -> ExportCheck {
        let Some(job) = job else {
            return ExportCheck::Waiting;
        };
        let over = job.over;
        // 32 bits are floats: a sample over 0 dB is written as it is. Matched, the true peak
        // stays under 0 dB.
        let matched = self.loudness_of(Some(job)) == ExportLoudness::Match;
        let clips = over > 0 && self.bits != RecordFormat::Bits32 && !matched;
        match &job.failed {
            Some(reason) => ExportCheck::Failed {
                reason: reason.clone(),
            },
            None if !job.counted => ExportCheck::Waiting,
            None if job.done => ExportCheck::Done {
                peak_db: job.peak_db,
                clips,
                over,
                lufs: job.lufs,
                true_peak_db: job.true_peak_db,
            },
            None => ExportCheck::Counting {
                percent: job.percent.min(99),
                peak_db: job.peak_db,
                clips,
                over,
                lufs: job.lufs,
                true_peak_db: job.true_peak_db,
            },
        }
    }

    /// 「公開先に合わせる」 can be chosen: not for 「トラックごと」, nor for a range whose
    /// loudness is known to be unmeasurable. Until the job is done that is not known.
    fn can_match(&self, job: Option<&Job>) -> bool {
        self.range != ExportRange::Tracks
            && job.is_some_and(|j| {
                j.failed.is_none()
                    && (!j.done || loudness::match_gain(j.lufs, j.true_peak_db).is_some())
            })
    }

    /// What 「音の大きさ」 is for the export now.
    fn loudness_of(&self, job: Option<&Job>) -> ExportLoudness {
        match self.can_match(job) {
            true => self.loudness,
            false => ExportLoudness::AsIs,
        }
    }

    fn can_export(&self, scene: &Scene) -> bool {
        let job = self.dialog.as_ref().and_then(|d| d.job.as_ref());
        job.is_some_and(|j| j.counted && j.failed.is_none())
            && self.folder_for(scene).is_some()
            && !scene.busy
    }

    pub fn view(&self, scene: &Scene) -> ExportView {
        let dialog = self.dialog.as_ref().map(|dialog| {
            let job = dialog.job.as_ref();
            let forced = job.is_some_and(|j| j.must_be_realtime);
            let mut place = shown_path(&self.place(scene), self.dirs.home.as_deref());
            if self.range == ExportRange::Tracks && !place.ends_with('/') {
                place.push('/');
            }
            ExportDialogView {
                range: self.range,
                loop_range: scene.loop_span(),
                tracks: scene.tracks(),
                span: self.span(scene),
                format: self.format,
                formats: ExportFormat::ALL.to_vec(),
                place,
                check: self.check(job),
                solo: (scene.soloed.split_first()).map(|(first, others)| ExportSolo {
                    first: first.clone(),
                    others: others.len() as u32,
                }),
                stopped_plugins: scene.stopped_plugins.clone(),
                advanced_open: self.advanced_open,
                sample_rate: self.sample_rate(scene),
                sample_rate_choices: self.sample_rate_choices(scene),
                bits: self.bits,
                bits_choices: self.format.bits_choices().to_vec(),
                tail: self.tail,
                realtime: self.realtime || forced,
                realtime_forced: forced,
                can_export: self.can_export(scene),
                loudness: self.loudness_of(job),
                can_match: self.can_match(job),
                loudness_result: (job.filter(|j| j.done && self.can_match(Some(j))))
                    .and_then(|j| loudness::match_gain(j.lufs, j.true_peak_db)),
            }
        });
        let run = self.run.as_ref().map(|run| match run {
            Run::Going { job, target } => ExportRunView::Running {
                percent: job.percent.min(99),
                file: match target.path.file_name() {
                    Some(name) if !job.asked.per_track => name.to_string_lossy().into_owned(),
                    _ => String::new(),
                },
                remaining_s: job.remaining_s,
                realtime: job.asked.realtime || job.must_be_realtime,
                realtime_forced: job.must_be_realtime,
                length_s: job.length_s,
            },
            Run::Ended(view, _) => view.clone(),
        });
        ExportView { dialog, run }
    }
}

#[cfg(test)]
mod tests;
