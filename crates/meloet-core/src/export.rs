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
#[cfg(test)]
mod info_tests;
pub(crate) mod job;
#[cfg(test)]
mod kit_tests;
pub(crate) mod loudness;
pub(crate) mod midi;
#[cfg(test)]
mod midi_tests;
#[cfg(test)]
mod mp3_gate_tests;
#[cfg(test)]
mod mp3_tests;
mod numbers;
pub(crate) mod render;
pub mod sink;
pub mod tags;
pub(crate) mod told;
pub(crate) mod wave;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use meloet_model::{Doc, TickRange, TrackId};
use meloet_protocol::{
    Command, EXPORT_MP3_DEFAULT_KBPS, EXPORT_MP3_KBPS, EXPORT_MP3_SAMPLE_RATES, ExportChange,
    ExportCheck, ExportDialogView, ExportFailure, ExportFormat, ExportLoudness, ExportMidiTracks,
    ExportRange, ExportRunView, ExportSolo, ExportStoppedPlugin, ExportTail, ExportView,
    PluginSlotRef, RecordFormat, StatusMessage, TickSpan, TrackRef,
};
use midi::MidiOut;

use crate::ai::{AiAnalysis, AiBandLayout};
use crate::plugins::shown_path;
use crate::settings::SAMPLE_RATE_CHOICES;
use tags::{ExportPicture, PictureProblem, SongInfo};

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
    /// F2 「絵」 「えらぶ…」: the PC's open window for a png or a jpg, starting in `folder`.
    /// Answer with [`ExportNews::PicturePicked`], the file read with [`tags::read_picture`].
    PickPicture { folder: Option<PathBuf> },
    /// 「書き出す」 under MIDI: write the files at once with [`midi::write`], and answer
    /// with what it gives back.
    WriteMidi { job: u64, files: Vec<MidiOut> },
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
    /// Only the tracks of `alone` are rendered, not the mix; while this one is, its
    /// instrument plugin keeps the sound of each of its output sets alone, and their levels
    /// are answered too (docs/architecture.md 20-15).
    pub capture: Option<TrackId>,
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
    /// F2: the bitrate of an mp3; not looked at for the other formats.
    pub mp3_kbps: u16,
    /// F2 「曲の情報」 written into every file; for 「トラックごと」 titled
    /// [`SongInfo::for_track`].
    pub info: SongInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportNews {
    /// The answer to [`ExportAsk::PickPlace`]; `None` when the window was put away.
    PlacePicked(Option<PathBuf>),
    /// The answer to [`ExportAsk::PickPicture`]: the file chosen and how it read; `None`
    /// when the window was put away.
    PicturePicked(Option<(PathBuf, Result<ExportPicture, PictureProblem>)>),
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
        /// With [`Measuring::capture`]: the levels of each output set, or why there are
        /// none. When the capture could not begin, nothing is rendered and `passes` is empty.
        captured: Option<Result<Vec<crate::ai::AiOutputLevel>, crate::ai::AiRefusal>>,
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
    /// The selected track (`Session::play_target`): 「選んでいるトラック」 of MIDI.
    pub selected: Option<TrackId>,
    /// `Play::plugin_kits`: written on channel 10 by a MIDI export.
    pub plugin_kits: Vec<TrackId>,
}

impl Scene<'_> {
    /// 「曲ぜんぶ」: from the start to where the last clip ends.
    fn song_span(&self) -> TickSpan {
        TickSpan {
            start: 0,
            end: meloet_model::sections::last_clip_end(self.doc),
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
    /// `Play::serial` of the song it was opened for.
    serial: u32,
    /// `None`: there is nothing to render (an empty song, or MIDI, which renders nothing).
    job: Option<Job>,
    picking: bool,
    /// The file that would be written for the name typed, which is there already and is not
    /// the one the window asked about: the window is shown again with it.
    again: Option<PathBuf>,
    /// F2: the window for 「絵」 is shown.
    picking_picture: bool,
}

#[derive(Debug)]
enum Run {
    Going {
        job: Job,
        target: ExportTarget,
    },
    /// MIDI files being written ([`ExportAsk::WriteMidi`]), and what the run says once
    /// they are.
    Writing {
        id: u64,
        done: ExportRunView,
    },
    Ended(ExportRunView, Option<PathBuf>),
}

#[derive(Debug)]
pub struct Exporting {
    dirs: ExportDirs,
    // The choices stay for as long as meloet runs.
    range: ExportRange,
    format: ExportFormat,
    /// F2 「作者」, a meloet setting.
    artist: String,
    /// F2 「ビットレート」 of mp3.
    mp3_kbps: u16,
    /// F2 「題名」 as typed; `None`: the song's name. With the picture, it belongs to the
    /// song of `info_serial` and goes when another song is opened.
    title: Option<String>,
    picture: Option<ExportPicture>,
    /// Where the window for 「絵」 starts: the folder of the last picture chosen.
    picture_folder: Option<PathBuf>,
    info_serial: Option<u32>,
    /// `None`: the rate the audio runs at (for mp3, the one of
    /// `EXPORT_MP3_SAMPLE_RATES` nearest to it, [`mp3_rate`]).
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
    /// 「入れるトラック」 of MIDI, and the text of its number field as last sent.
    midi_tracks: ExportMidiTracks,
    midi_numbers: String,
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
            artist: String::new(),
            mp3_kbps: EXPORT_MP3_DEFAULT_KBPS,
            title: None,
            picture: None,
            picture_folder: None,
            info_serial: None,
            sample_rate: None,
            bits: RecordFormat::default(),
            tail: ExportTail::UntilSilent,
            realtime: false,
            advanced_open: false,
            loudness: ExportLoudness::AsIs,
            folder: None,
            name: None,
            midi_tracks: ExportMidiTracks::All,
            midi_numbers: String::new(),
            dialog: None,
            run: None,
            last_job: 0,
        }
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn running(&self) -> bool {
        matches!(self.run, Some(Run::Going { .. } | Run::Writing { .. }))
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

    /// F2: 「作者」 as the settings keep it.
    pub fn with_artist(mut self, artist: String) -> Exporting {
        self.artist = artist;
        self
    }

    pub fn artist(&self) -> &str {
        &self.artist
    }

    /// The export's commands; any other does nothing.
    pub fn command(&mut self, command: &Command, scene: &Scene, asks: &mut Vec<ExportAsk>) {
        match command {
            Command::OpenExport => {
                if self.dialog.is_none() && !self.running() && !scene.busy {
                    if self.range == ExportRange::Loop && scene.loop_span().is_none() {
                        self.range = ExportRange::Song;
                    }
                    if self.midi_tracks == ExportMidiTracks::Selected
                        && !self.midi_taken(scene).selected_usable
                    {
                        self.midi_tracks = ExportMidiTracks::All;
                    }
                    if self.info_serial != Some(scene.serial) {
                        self.info_serial = Some(scene.serial);
                        self.title = None;
                        self.picture = None;
                    }
                    self.dialog = Some(Dialog {
                        serial: scene.serial,
                        job: None,
                        picking: false,
                        again: None,
                        picking_picture: false,
                    });
                }
            }
            Command::CloseExport => self.close(asks),
            Command::SetExport { change } => self.set(change.clone(), scene, asks),
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

    fn set(&mut self, change: ExportChange, scene: &Scene, asks: &mut Vec<ExportAsk>) {
        if self.dialog.is_none() {
            return;
        }
        match change {
            ExportChange::Mp3Kbps { kbps } => {
                if EXPORT_MP3_KBPS.contains(&kbps) {
                    self.mp3_kbps = kbps;
                }
            }
            ExportChange::Title { title } => self.title = Some(title),
            ExportChange::Artist { artist } => self.artist = artist,
            ExportChange::ChoosePicture => {
                let folder = self.picture_folder.clone().or_else(|| {
                    let of_song = scene.song_path.and_then(Path::parent);
                    of_song.map(Path::to_path_buf).or(self.dirs.home.clone())
                });
                let usable = self.picture_usable();
                if let Some(dialog) = self.dialog.as_mut().filter(|d| !d.picking_picture)
                    && usable
                {
                    dialog.picking_picture = true;
                    asks.push(ExportAsk::PickPicture { folder });
                }
            }
            ExportChange::RemovePicture => self.picture = None,
            ExportChange::Range { range } => {
                let usable = match range {
                    ExportRange::Song => true,
                    ExportRange::Loop => scene.loop_span().is_some(),
                    ExportRange::Tracks => self.tracks(scene) > 0,
                };
                if usable {
                    self.range = range;
                }
            }
            ExportChange::Format { format } => {
                self.format = format;
                // mp3 and MIDI have no bits: the ones chosen stay for the other formats.
                let bits = format.bits_choices();
                if !bits.is_empty() && !bits.contains(&self.bits) {
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
            ExportChange::MidiTracks { tracks } => {
                if tracks != ExportMidiTracks::Selected || self.midi_taken(scene).selected_usable {
                    self.midi_tracks = tracks;
                }
            }
            ExportChange::MidiNumbers { text } => self.midi_numbers = text,
        }
    }

    fn start(&mut self, scene: &Scene, asks: &mut Vec<ExportAsk>) {
        if !self.can_export(scene) {
            return;
        }
        if self.format == ExportFormat::Midi {
            self.start_midi(scene, asks);
            return;
        }
        let job = self.dialog.as_ref().and_then(|d| d.job.as_ref());
        let target = ExportTarget {
            path: self.place(scene),
            format: self.format,
            sample_rate: self.sample_rate(scene),
            bits: self.bits,
            loudness: self.loudness_of(job),
            mp3_kbps: self.mp3_kbps,
            info: SongInfo {
                title: self.title(scene).to_owned(),
                artist: self.artist.clone(),
                picture: self.picture.clone().filter(|_| self.picture_usable()),
            },
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

    /// MIDI is not rendered: the files are made here, written at once, and the run is
    /// `Done` with no dialog 「書き出しました」.
    fn start_midi(&mut self, scene: &Scene, asks: &mut Vec<ExportAsk>) {
        self.close(asks);
        let taken = self.midi_taken(scene).places;
        let span = self.span(scene);
        let path = self.place(scene);
        let per_track = self.range == ExportRange::Tracks;
        let Ok(files) = midi::files(
            scene.doc,
            &taken,
            &scene.plugin_kits,
            span,
            &path,
            per_track,
        ) else {
            let reason = ExportFailure::CannotWrite;
            self.run = Some(Run::Ended(ExportRunView::Failed { reason }, None));
            return;
        };
        let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
        let done = ExportRunView::Done {
            file: (files.first())
                .and_then(|f| name(&f.path))
                .unwrap_or_default(),
            files: files.len() as u32,
            place: shown_path(&path, self.dirs.home.as_deref()),
            length_s: midi::length_s(scene.doc, span),
            sample_rate: self.sample_rate(scene),
            bits: self.bits,
            wave: Vec::new(),
            lufs: None,
            true_peak_db: None,
            mp3_kbps: None,
            midi_tracks: Some(taken.len() as u32),
        };
        self.last_job += 1;
        let id = self.last_job;
        asks.push(ExportAsk::WriteMidi { job: id, files });
        self.run = Some(Run::Writing { id, done });
    }

    /// After anything that happened: the job of the open dialog is the one for what the
    /// dialog says now, and a dialog whose song is gone closes.
    pub fn settle(&mut self, scene: &Scene, asks: &mut Vec<ExportAsk>) {
        if self
            .dialog
            .as_ref()
            .is_some_and(|d| d.serial != scene.serial)
        {
            self.close(asks);
            return;
        }
        if let Some(again) = self.dialog.as_mut().and_then(|d| d.again.take()) {
            asks.push(ExportAsk::PickPlace {
                folder: again.parent().map(Path::to_path_buf),
                name: (again.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                folder_only: false,
            });
        }
        let wanted = self.asked(scene);
        let had = (self.dialog.as_ref()).and_then(|d| Some(&d.job.as_ref()?.asked));
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

    /// What happened to the export. A picture chosen that cannot be used is told in the
    /// status line: the message is returned.
    pub fn news(&mut self, news: ExportNews) -> Option<StatusMessage> {
        match news {
            ExportNews::PicturePicked(picked) => self.picture_picked(picked),
            news => {
                self.job_news(news);
                None
            }
        }
    }

    fn picture_picked(
        &mut self,
        picked: Option<(PathBuf, Result<ExportPicture, PictureProblem>)>,
    ) -> Option<StatusMessage> {
        let dialog = self.dialog.as_mut().filter(|d| d.picking_picture)?;
        dialog.picking_picture = false;
        let (path, read) = picked?;
        let file =
            (path.file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.picture_folder = path.parent().map(Path::to_path_buf);
        match read {
            Ok(picture) => {
                self.picture = Some(picture);
                None
            }
            Err(PictureProblem::TooLarge) => Some(StatusMessage::ExportPictureTooLarge { file }),
            Err(PictureProblem::Unreadable) => {
                Some(StatusMessage::ExportPictureUnreadable { file })
            }
        }
    }

    fn job_news(&mut self, news: ExportNews) {
        match news {
            ExportNews::PicturePicked(_) => {}
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
                } else if let Some(typed) = path.file_name() {
                    let name = typed_name(&typed.to_string_lossy(), self.format).to_owned();
                    let real = path.with_file_name(format!("{name}.{}", self.format.extension()));
                    // The window asked only about writing over the name typed.
                    if real != path && real.exists() {
                        dialog.picking = true;
                        dialog.again = Some(real);
                        return;
                    }
                    self.folder = path.parent().map(Path::to_path_buf);
                    self.name = Some(name);
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
            ExportNews::Finished { job, files, .. } if matches!(&self.run, Some(Run::Writing { id, .. }) if *id == job) => {
                if let Some(Run::Writing { done, .. }) = self.run.take() {
                    self.run = Some(Run::Ended(done, files.into_iter().next()));
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
                    mp3_kbps: (target.format == ExportFormat::Mp3).then_some(target.mp3_kbps),
                    file: files.first().and_then(|p| name(p)).unwrap_or_default(),
                    files: files.len() as u32,
                    place: shown_path(&target.path, self.dirs.home.as_deref()),
                    length_s: going.length_s,
                    sample_rate: target.sample_rate,
                    bits: target.bits,
                    wave,
                    lufs,
                    true_peak_db,
                    midi_tracks: None,
                };
                let shown = files.into_iter().next();
                self.run = Some(Run::Ended(view, shown));
            }
            ExportNews::Measured { .. } => {}
            ExportNews::Failed { job, reason } => {
                let ours = match &self.run {
                    Some(Run::Going { job: going, .. }) => going.id == job,
                    Some(Run::Writing { id, .. }) => *id == job,
                    _ => false,
                };
                if ours {
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

    /// What the job of the dialog is to render; `None`: nothing, as under MIDI, where
    /// nothing is measured.
    fn asked(&self, scene: &Scene) -> Option<Asked> {
        let span = self.span(scene);
        let per_track = self.range == ExportRange::Tracks;
        let midi = self.format == ExportFormat::Midi;
        (!midi && span.end > span.start && !(per_track && scene.tracks() == 0)).then_some(Asked {
            serial: scene.serial,
            span,
            per_track,
            tail: self.tail,
            realtime: self.realtime,
        })
    }

    fn sample_rate(&self, scene: &Scene) -> u32 {
        let rate = self.sample_rate.unwrap_or(scene.sample_rate);
        match self.format {
            ExportFormat::Mp3 => mp3_rate(rate),
            ExportFormat::Wav | ExportFormat::Flac | ExportFormat::Midi => rate,
        }
    }

    fn sample_rate_choices(&self, scene: &Scene) -> Vec<u32> {
        if self.format == ExportFormat::Mp3 {
            return EXPORT_MP3_SAMPLE_RATES.to_vec();
        }
        let mut choices = SAMPLE_RATE_CHOICES.to_vec();
        if !choices.contains(&scene.sample_rate) {
            choices.push(scene.sample_rate);
            choices.sort_unstable();
        }
        choices
    }

    /// F2 「題名」: as typed, or else the song's name.
    fn title<'a>(&'a self, scene: &'a Scene) -> &'a str {
        self.title.as_deref().unwrap_or(&scene.doc.name)
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
        // stays under 0 dB. The bits kept for wav do not count for a format without bits.
        let matched = self.loudness_of(Some(job)) == ExportLoudness::Match;
        let floats = self.bits == RecordFormat::Bits32
            && self.format.bits_choices().contains(&RecordFormat::Bits32);
        let clips = over > 0 && !floats && !matched;
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

    /// F2: a wav and a MIDI file hold no picture.
    fn picture_usable(&self) -> bool {
        matches!(self.format, ExportFormat::Flac | ExportFormat::Mp3)
    }

    /// 「入れるトラック」 as it takes the song now.
    fn midi_taken(&self, scene: &Scene) -> midi::Taken {
        midi::taken(self.midi_tracks, &self.midi_numbers, scene)
    }

    /// How many files 「トラックごと」 writes: under MIDI the tracks 「入れるトラック」
    /// takes.
    fn tracks(&self, scene: &Scene) -> u32 {
        match self.format {
            ExportFormat::Midi => self.midi_taken(scene).places.len() as u32,
            _ => scene.tracks(),
        }
    }

    fn can_export(&self, scene: &Scene) -> bool {
        if self.format == ExportFormat::Midi {
            return self.midi_taken(scene).problem.is_none()
                && self.folder_for(scene).is_some()
                && !scene.busy;
        }
        let job = self.dialog.as_ref().and_then(|d| d.job.as_ref());
        job.is_some_and(|j| j.counted && j.failed.is_none())
            && self.folder_for(scene).is_some()
            && !scene.busy
    }

    pub fn view(&self, scene: &Scene) -> ExportView {
        let dialog = self.dialog.as_ref().map(|dialog| {
            let job = dialog.job.as_ref();
            let taken = self.midi_taken(scene);
            let midi = self.format == ExportFormat::Midi;
            let forced = job.is_some_and(|j| j.must_be_realtime);
            let mut place = shown_path(&self.place(scene), self.dirs.home.as_deref());
            if self.range == ExportRange::Tracks && !place.ends_with('/') {
                place.push('/');
            }
            ExportDialogView {
                range: self.range,
                loop_range: scene.loop_span(),
                tracks: self.tracks(scene),
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
                mp3_kbps: self.mp3_kbps,
                mp3_kbps_choices: EXPORT_MP3_KBPS.to_vec(),
                title: self.title(scene).to_owned(),
                artist: self.artist.clone(),
                picture: self.picture.as_ref().map(|p| p.name.clone()),
                picture_usable: self.picture_usable(),
                tail: self.tail,
                realtime: self.realtime || forced,
                realtime_forced: forced,
                can_export: self.can_export(scene),
                loudness: self.loudness_of(job),
                can_match: self.can_match(job),
                loudness_result: (job.filter(|j| j.done && self.can_match(Some(j))))
                    .and_then(|j| loudness::match_gain(j.lufs, j.true_peak_db)),
                midi_tracks: self.midi_tracks,
                midi_numbers: self.midi_numbers.clone(),
                midi_selected_usable: taken.selected_usable,
                midi_taken: midi::listed(scene.doc, &taken.places),
                midi_vocal: midi::listed(scene.doc, &taken.vocal),
                midi_problem: taken.problem.filter(|_| midi),
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
            Run::Writing { done, .. } => match done {
                ExportRunView::Done {
                    file,
                    files,
                    length_s,
                    ..
                } => ExportRunView::Running {
                    percent: 0,
                    file: if *files == 1 {
                        file.clone()
                    } else {
                        String::new()
                    },
                    remaining_s: None,
                    realtime: false,
                    realtime_forced: false,
                    length_s: *length_s,
                },
                other => other.clone(),
            },
            Run::Ended(view, _) => view.clone(),
        });
        ExportView { dialog, run }
    }
}

/// F2: an mp3 is written at 44100 or 48000 Hz; another rate gives the one of its family
/// (88200 → 44100, 96000 → 48000, 32000 → 48000).
fn mp3_rate(rate: u32) -> u32 {
    match rate {
        _ if EXPORT_MP3_SAMPLE_RATES.contains(&rate) => rate,
        _ if rate.is_multiple_of(11_025) => 44_100,
        _ => 48_000,
    }
}

/// The name typed in the save window, without the ending of `format` (in any case); other
/// text after a dot is part of the name.
fn typed_name(typed: &str, format: ExportFormat) -> &str {
    match typed.rsplit_once('.') {
        Some((name, ending))
            if !name.is_empty() && ending.eq_ignore_ascii_case(format.extension()) =>
        {
            name
        }
        _ => typed,
    }
}

#[cfg(test)]
mod tests;
