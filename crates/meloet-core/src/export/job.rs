//! One job of the export on its thread (docs/architecture.md 22-11, 22-14): the song is
//! rendered into the work place as 32 bit floats at the rate the audio runs at, and what it
//! saw on the way is told for 「書き出す前のたしかめ」. With 「書き出す」 those files are
//! brought into the form asked for and moved to where the person wants them.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_model::{Doc, InstrumentRef, PluginPlace, TickRange, TrackId};
use meloet_protocol::{EXPORT_TAIL_MAX_S, ExportFailure, ExportLoudness, ExportTail};

use super::analysis::{self, Heard, Listener};
use super::loudness::{self, TruePeak};
use super::render::{ExportEngine, Plugs};
use super::sink::{self, SinkSpec};
use super::tags;
use super::told::{KnownPlugins, Told};
use super::wave::WaveBars;
use super::{ExportJob, ExportNews, ExportTarget};
use crate::ai::{AiOutputLevel, AiRefusal, PassLevels, capture_longest_s};
use crate::audiofiles::resample::Resample;
use crate::audiofiles::wav::{CacheWav, WavPcm};
use crate::audiofiles::{AudioStore, Playable};
use crate::instruments::Samples;
use crate::made::MadeKey;
use crate::maker::make_now;
use crate::pcm::PcmFile;
use crate::plugins::export::{
    ExportPlugin, ExportPluginError, ExportSeats, ExportWant, export_seats,
};
use crate::plugins::hosts::{HostError, HostOptions, capture_frames_max};

/// 「書き出す前のたしかめ」 hears how far the job is this often at most.
const TELL_EVERY: Duration = Duration::from_millis(250);
/// How often a job that waits looks whether it is still wanted.
const POLL: Duration = Duration::from_millis(50);
/// The tail ends once what comes out stayed below this (−90 dBFS) for [`QUIET_S`].
const QUIET: f32 = 3.162_277_7e-5;
const QUIET_S: f64 = 0.5;
/// How long the playing plugins are waited for to say their state, as a save waits.
const STATE_WAIT: Duration = Duration::from_secs(3);
/// How long audio files that are being read in are waited for.
const FILES_WAIT: Duration = Duration::from_secs(300);
/// Frames brought into the file's form in one go.
const CHUNK: usize = 4_096;
/// Samples over 0 dB this far apart or more are two places (docs/history/design-notes.ja.md B-95-2).
const OVER_GAP_S: f64 = 0.1;

/// What a plugin that plays says its state is now.
pub type LiveState = (PluginPlace, Result<Vec<u8>, HostError>);

/// Everything a job needs of the core, taken on the core thread when the job is asked for.
pub struct Setup {
    pub job: ExportJob,
    /// The frames of a block of the playing engine's stream.
    pub block: usize,
    pub store: Arc<AudioStore>,
    /// The samples of the song's instruments.
    pub samples: Vec<(InstrumentRef, Samples)>,
    pub known: KnownPlugins,
    /// `None`: this meloet runs no plugins.
    pub hosts: Option<HostOptions>,
    /// The states of the plugins that play, as they answer.
    pub states: Option<Receiver<LiveState>>,
    /// How many files the maker's thread still has to make for the song that plays.
    pub maker_waiting: Option<Arc<AtomicUsize>>,
    /// A folder for this job alone. It is removed when the job ends.
    pub work: PathBuf,
}

/// The core's end of a job. Dropping it gives the job up.
pub struct JobHandle {
    pub id: u64,
    target: Sender<ExportTarget>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl JobHandle {
    /// 「書き出す」: once it is rendered, the job goes to `target`.
    pub fn finish(&self, target: ExportTarget) {
        let _ = self.target.send(target);
    }

    /// The job is not wanted any more: its thread takes its files away and ends.
    pub fn give_up(&self) {
        self.stop.store(true, Relaxed);
    }

    pub fn is_over(&self) -> bool {
        self.thread.as_ref().is_none_or(|t| t.is_finished())
    }

    /// Waits for the thread to end. A plugin process that is starting is waited for.
    pub fn join(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for JobHandle {
    fn drop(&mut self) {
        self.give_up();
    }
}

/// Starts the thread of a job. `tell` is called on it.
pub fn spawn(setup: Setup, tell: impl Fn(ExportNews) + Send + 'static) -> io::Result<JobHandle> {
    let id = setup.job.id;
    let (target, targets) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread = std::thread::Builder::new()
        .name("meloet-export".into())
        .spawn({
            let stop = stop.clone();
            move || {
                // The threads this one starts (the export's `Hosts`) are at the same priority.
                meloet_plugin::launch::low_priority();
                run(setup, &targets, &stop, &tell)
            }
        })?;
    Ok(JobHandle {
        id,
        target,
        stop,
        thread: Some(thread),
    })
}

enum Ended {
    /// Given up; nothing is said.
    Dropped,
    Failed(ExportFailure),
    /// The output sets of [`crate::export::Measuring::capture`] cannot be kept: the job
    /// answers that, with nothing measured.
    Refused(AiRefusal),
}

impl From<io::Error> for Ended {
    fn from(e: io::Error) -> Ended {
        eprintln!("meloet: the export could not write: {e}");
        Ended::Failed(ExportFailure::CannotWrite)
    }
}

fn run(
    setup: Setup,
    targets: &Receiver<ExportTarget>,
    stop: &AtomicBool,
    tell: &dyn Fn(ExportNews),
) {
    let id = setup.job.id;
    let work = setup.work.clone();
    let mut made = Vec::new();
    match go(setup, targets, stop, tell, &mut made) {
        Err(Ended::Failed(reason)) => {
            tell(ExportNews::Failed { job: id, reason });
            // What is already where the person wanted it is not left there half done.
            for file in made {
                let _ = fs::remove_file(file);
            }
        }
        Err(Ended::Refused(refusal)) => tell(ExportNews::Measured {
            job: id,
            passes: Vec::new(),
            analysis: None,
            captured: Some(Err(refusal)),
        }),
        Ok(()) | Err(Ended::Dropped) => {}
    }
    let _ = fs::remove_dir_all(work);
}

/// One file of the job in the work place.
struct Rendered {
    path: PathBuf,
    /// The track it is, for 「トラックごと」.
    name: Option<String>,
    levels: PassLevels,
    /// What a job that measures heard.
    heard: Option<Heard>,
    /// Of a job that renders one file and does not measure.
    true_peak_db: Option<f32>,
}

fn go(
    setup: Setup,
    targets: &Receiver<ExportTarget>,
    stop: &AtomicBool,
    tell: &dyn Fn(ExportNews),
    moved: &mut Vec<PathBuf>,
) -> Result<(), Ended> {
    let job = &setup.job;
    fs::create_dir_all(&setup.work)?;
    if job.measure.is_some() {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&setup.work, fs::Permissions::from_mode(0o700))?;
    }
    let mut target = None;
    let mut captured = None;
    let mut rendered = render(&setup, targets, stop, tell, &mut target, &mut captured)?;
    if let Some(measure) = &job.measure {
        let heard: Vec<Heard> = (rendered.iter_mut())
            .map(|r| r.heard.take().unwrap_or_default())
            .collect();
        let analysis = (measure.bands.is_some() || !measure.cuts.is_empty()).then(|| {
            let bands = (measure.bands)
                .map_or_else(Vec::new, |layout| analysis::bands(layout, job.sample_rate));
            Box::new(analysis::gather(
                bands,
                job.sample_rate,
                &heard,
                &measure.compare,
                job.span,
                &measure.cuts,
            ))
        });
        tell(ExportNews::Measured {
            job: job.id,
            passes: rendered.iter().map(|r| r.levels).collect(),
            analysis,
            captured,
        });
        return Ok(());
    }
    let target = match target {
        Some(target) => target,
        None => loop {
            if stop.load(Relaxed) {
                return Err(Ended::Dropped);
            }
            match targets.recv_timeout(POLL) {
                Ok(target) => break target,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(Ended::Dropped),
            }
        },
    };
    let gain = match (target.loudness, &rendered[..]) {
        (ExportLoudness::Match, [one]) if !job.per_track => {
            loudness::match_gain(one.levels.lufs, one.true_peak_db).map(|m| m.gain_db)
        }
        _ => None,
    };
    // Every file is brought into its form in the work place first: moving takes a moment,
    // so a job given up on the way leaves nothing where the person looks.
    let mut ready = Vec::new();
    let mut taken = Vec::new();
    let mut wave = Vec::new();
    for (i, file) in rendered.iter().enumerate() {
        let to = match &file.name {
            Some(name) => {
                let name = free_name(name, target.format.extension(), &taken);
                taken.push(name.clone());
                target.path.join(name)
            }
            None => target.path.clone(),
        };
        let out = setup.work.join(format!("out-{i}"));
        let bars = convert(&file.path, &out, job.sample_rate, &target, gain, stop)?;
        let info = match &file.name {
            Some(track) => target.info.for_track(track),
            None => target.info.clone(),
        };
        tags::write(&out, target.format, &info)?;
        if !job.per_track && rendered.len() == 1 {
            wave = bars;
        }
        ready.push((out, to));
    }
    for (out, to) in ready {
        if stop.load(Relaxed) {
            for file in moved.drain(..) {
                let _ = fs::remove_file(file);
            }
            return Err(Ended::Dropped);
        }
        move_to(&out, &to)?;
        moved.push(to);
    }
    tell(ExportNews::Finished {
        job: job.id,
        files: std::mem::take(moved),
        wave,
    });
    Ok(())
}

/// `<name>.<ending>`, with what a file name cannot hold taken out, and a number after it
/// when a track before has the same name.
pub(crate) fn free_name(track: &str, ending: &str, taken: &[String]) -> String {
    let clean: String = track
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0') && !c.is_control())
        .collect();
    let clean = cut_name(clean.trim().trim_matches('.'));
    let clean = if clean.is_empty() { "_" } else { clean };
    let mut name = format!("{clean}.{ending}");
    let mut n = 1;
    while taken.contains(&name) {
        n += 1;
        name = format!("{clean} {n}.{ending}");
    }
    name
}

/// The bytes of a file name that a name made from a track's name may take: a file name holds
/// 255, and a number, an id and an ending go after it.
pub(crate) const NAME_BYTES: usize = 200;

/// `name` cut to at most [`NAME_BYTES`] bytes, between two characters, with no space or dot
/// left at its end.
pub(crate) fn cut_name(name: &str) -> &str {
    let mut end = name.len().min(NAME_BYTES);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].trim_end().trim_end_matches('.')
}

/// Renders the job into the work place. A target that comes meanwhile is kept in `target`,
/// what [`crate::export::Measuring::capture`] measured in `captured`.
fn render(
    setup: &Setup,
    targets: &Receiver<ExportTarget>,
    stop: &AtomicBool,
    tell: &dyn Fn(ExportNews),
    target: &mut Option<ExportTarget>,
    captured: &mut Option<Result<Vec<AiOutputLevel>, AiRefusal>>,
) -> Result<Vec<Rendered>, Ended> {
    let job = &setup.job;
    let doc: &Doc = &job.doc;
    let told = Told::of(doc, None, &setup.known, &job.skip);
    let mut engine = ExportEngine::new(
        job.sample_rate,
        setup.block,
        setup.store.clone(),
        setup.samples.clone(),
    );
    let mut plugs = Plugs::default();
    for op in told.ops.iter().cloned() {
        engine.tell(op, &mut plugs);
    }
    wait_files(setup, stop)?;
    wait_made(&mut engine, setup, stop)?;
    // The seats end the plugin processes when they are let go of, which is after the
    // engine that holds their plugins.
    let mut seats = load_plugins(&engine, &told, setup)?;
    if seats.realtime {
        tell(ExportNews::MustBeRealtime { job: job.id });
    }
    let ticket = |place: PluginPlace| told.plugins.iter().find(|p| p.place == place);
    for (place, plugin) in std::mem::take(&mut seats.instruments) {
        if let Some(told) = ticket(place) {
            plugs.instruments.push((told.ticket, plugin));
        }
    }
    for (place, plugin) in std::mem::take(&mut seats.effects) {
        if let Some(told) = ticket(place) {
            plugs.effects.push((told.ticket, plugin));
        }
    }
    for op in told.plugged.iter().cloned() {
        engine.tell(op, &mut plugs);
    }
    drop(plugs);
    engine.open_files();

    let each_track = (doc.tracks.iter()).map(|t| Some((t.id, t.name.clone())));
    let passes: Vec<Option<(TrackId, String)>> = match (&job.measure, job.per_track) {
        (Some(measure), _) => {
            let alone = (measure.alone.iter()).filter_map(|id| doc.track(*id));
            let alone = alone.map(|t| Some((t.id, t.name.clone())));
            match measure.capture {
                Some(_) => alone.collect(),
                None => std::iter::once(None).chain(alone).collect(),
            }
        }
        (None, true) => each_track.collect(),
        (None, false) => vec![None],
    };
    let cuts: Vec<u64> = (job.measure.iter())
        .flat_map(|m| m.cuts.iter())
        .map(|&tick| frames_to(&engine, job.span, tick))
        .collect();
    let bands = (job.measure.as_ref().and_then(|m| m.bands))
        .map(|layout| analysis::bands(layout, job.sample_rate));
    // F1: the loudness and the true peak of one file (the dialog's; the AI link's too).
    let loud = !job.per_track && job.measure.is_none();
    let mut watch = Watch {
        job: job.id,
        passes: passes.len() as u64,
        pass: 0,
        range: range_frames(&engine, job.span),
        rate: job.sample_rate,
        started: Instant::now(),
        told_at: None,
        peak: 0.0,
        over_before: 0,
        over: 0,
        longest: 0,
        loudness: (None, None),
    };
    let realtime = job.realtime || seats.realtime;
    let capture = job.measure.as_ref().and_then(|m| m.capture);
    let mut rendered = Vec::new();
    let result: Result<(), Ended> = (|| {
        for (i, pass) in passes.into_iter().enumerate() {
            if let Some((track, _)) = &pass {
                let unmuted;
                let doc = match &job.measure {
                    Some(m) if m.ignore_mute => {
                        unmuted = unmute(doc, *track);
                        &unmuted
                    }
                    _ => doc,
                };
                let solo = Told::of(doc, Some(*track), &setup.known, &job.skip);
                engine.set_silent(solo.silent());
            }
            let listen = match &job.measure {
                Some(m) => {
                    let keep = i > 0 && m.compare.contains(&(i - 1));
                    let bands = bands.as_deref().map(|b| (b, keep));
                    Some(Listener::new(job.sample_rate, bands, &cuts))
                }
                None => loud.then(|| Listener::new(job.sample_rate, None, &[])),
            };
            let true_peak = if loud {
                TruePeak::new(job.sample_rate)
            } else {
                None
            };
            watch.pass = i as u64;
            let path = setup.work.join(format!("pass-{i}.wav"));
            engine.play_from(job.span.start(), i > 0);
            let capturing = (pass.as_ref())
                .map(|(track, _)| *track)
                .filter(|track| capture == Some(*track))
                .map(PluginPlace::Instrument);
            if let Some(place) = capturing {
                start_capture(&engine, &seats, place, job)?;
            }
            let mut each = |written: u64, taker: &Taker| -> Result<(), Ended> {
                if stop.load(Relaxed) {
                    return Err(Ended::Dropped);
                }
                if let Some(place) = seats.stopped() {
                    let name = ticket(place).map(|p| p.name.to_string());
                    return Err(Ended::Failed(ExportFailure::Plugin {
                        name: name.unwrap_or_default(),
                    }));
                }
                if let Ok(wanted) = targets.try_recv() {
                    *target = Some(wanted);
                }
                watch.saw(written, taker, tell);
                Ok(())
            };
            let listening = Listening { listen, true_peak };
            let (levels, heard, true_peak_db) =
                render_pass(&mut engine, job, &path, realtime, &mut each, listening)?;
            if let Some(place) = capturing {
                let range = range_frames(&engine, job.span);
                *captured = Some(output_levels(&seats, place, job.sample_rate, range));
            }
            watch.over_before = watch.over;
            watch.longest = watch.longest.max(levels.frames);
            if loud {
                watch.loudness = (levels.lufs, true_peak_db);
            }
            if job.measure.is_some() {
                let _ = fs::remove_file(&path);
            }
            rendered.push(Rendered {
                path,
                name: pass.map(|(_, name)| name),
                levels,
                heard,
                true_peak_db,
            });
        }
        Ok(())
    })();
    if engine.missing() > 0 {
        eprintln!(
            "meloet: the export found {} frames of vocal clips not read",
            engine.missing()
        );
    }
    drop(engine);
    drop(seats);
    result?;
    watch.done(tell);
    Ok(rendered)
}

/// Asks the instrument plugin at `place` to keep the sound of each of its output sets from
/// the next block on, for as long as the pass renders: the range, and the head the master's
/// inserts delay the rest by.
fn start_capture(
    engine: &ExportEngine,
    seats: &ExportSeats,
    place: PluginPlace,
    job: &ExportJob,
) -> Result<(), Ended> {
    let head = engine.master_latency() as u64;
    let frames = range_frames(engine, job.span) + head;
    let sets = (seats.output_sets(place)).ok_or(Ended::Refused(AiRefusal::PluginNotLoaded))?;
    if frames > capture_frames_max(sets.len()) {
        let rate = f64::from(job.sample_rate.max(1));
        let longest_s = capture_longest_s(sets.len(), job.sample_rate) - head as f64 / rate;
        return Err(Ended::Refused(AiRefusal::CaptureTooLong {
            sets: sets.len() as u32,
            longest_s: longest_s.max(0.0),
        }));
    }
    seats.capture(place, frames).map_err(|e| {
        Ended::Refused(AiRefusal::Cannot(format!(
            "the plugin host could not keep the sound of the outputs ({e})"
        )))
    })
}

/// The levels of each output set the plugin at `place` said it has, over the first `range`
/// frames of what it kept; none for a set it kept nothing of. The track plays its plugin
/// early by what its effects delay it by: with such effects, what is measured is a stretch
/// that much later than the range.
fn output_levels(
    seats: &ExportSeats,
    place: PluginPlace,
    rate: u32,
    range: u64,
) -> Result<Vec<AiOutputLevel>, AiRefusal> {
    let sounds = seats.captured(place).map_err(|e| {
        AiRefusal::Cannot(format!(
            "the plugin host did not give the sound of the outputs ({e})"
        ))
    })?;
    Ok((seats.output_sets(place).unwrap_or_default().iter())
        .map(|set| AiOutputLevel {
            set: set.clone(),
            levels: (sounds.iter())
                .find(|s| s.id == set.id)
                .map(|s| set_levels(&s.samples, rate, range)),
        })
        .collect())
}

/// The levels of interleaved stereo `samples` over their first `range` frames, measured as
/// a pass of the range is.
fn set_levels(samples: &[f32], rate: u32, range: u64) -> PassLevels {
    let samples = &samples[..samples.len().min(range as usize * 2)];
    let mut places = OverPlaces::new(rate);
    places.feed(samples);
    let mut listen = Listener::new(rate, None, &[]);
    listen.feed(samples);
    let heard = listen.finish();
    let squares: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    PassLevels {
        peak: peak_of(samples),
        rms: (squares / samples.len().max(1) as f64).sqrt() as f32,
        over: places.count,
        frames: (samples.len() / 2) as u64,
        lufs: heard.lufs,
        lufs_short_max: heard.lufs_short_max,
    }
}

/// How many frames the range has: from where the engine starts to play to the first frame
/// at or past the range's end, as the engine counts them.
fn range_frames(engine: &ExportEngine, span: TickRange) -> u64 {
    frames_to(engine, span, span.end())
}

/// The frames from the range's start to `tick`, as the engine counts them.
fn frames_to(engine: &ExportEngine, span: TickRange, tick: u64) -> u64 {
    let map = engine.map();
    let from = map.seconds_at(span.start() as f64);
    map.frame_of(tick as f64, from, f64::from(engine.rate()))
}

/// The song with `track` not muted (a lane that moves its mute still does).
fn unmute(doc: &Doc, track: TrackId) -> Doc {
    let mut doc = doc.clone();
    for t in doc.tracks.iter_mut().filter(|t| t.id == track) {
        Arc::make_mut(t).muted = false;
    }
    doc
}

/// What 「書き出す前のたしかめ」 is told.
struct Watch {
    job: u64,
    passes: u64,
    pass: u64,
    range: u64,
    rate: u32,
    started: Instant,
    told_at: Option<Instant>,
    /// The largest sample of every file so far.
    peak: f32,
    /// The places over 0 dB of the files before this pass, and of every file so far.
    over_before: u32,
    over: u32,
    /// The frames of the longest file so far.
    longest: u64,
    /// F1, once the one file is rendered: its loudness and true peak.
    loudness: (Option<f32>, Option<f32>),
}

impl Watch {
    fn peak_db(&self) -> Option<f32> {
        (self.peak > 0.0).then(|| 20.0 * self.peak.log10())
    }

    fn seconds(&self, frames: u64) -> u32 {
        (frames as f64 / f64::from(self.rate.max(1))).round() as u32
    }

    /// `written`: frames of the range that are in the file of this pass.
    fn saw(&mut self, written: u64, taker: &Taker, tell: &dyn Fn(ExportNews)) {
        self.peak = self.peak.max(taker.peak);
        self.over = self.over_before.saturating_add(taker.places.count);
        if self.told_at.is_some_and(|at| at.elapsed() < TELL_EVERY) {
            return;
        }
        let (lufs, true_peak_db) = taker.loudness();
        self.told_at = Some(Instant::now());
        let all = (self.passes * self.range).max(1);
        let done = (self.pass * self.range + written.min(self.range)).min(all);
        let elapsed = self.started.elapsed().as_secs_f64();
        // Not before there is a second to guess from.
        let remaining = (elapsed >= 1.0 && done > 0)
            .then(|| (elapsed * (all - done) as f64 / done as f64).ceil() as u32);
        tell(ExportNews::Counted {
            job: self.job,
            percent: (done * 100 / all) as u8,
            peak_db: self.peak_db(),
            over: self.over,
            length_s: self.seconds(self.range),
            remaining_s: remaining,
            lufs,
            true_peak_db,
            done: false,
        });
    }

    fn done(&self, tell: &dyn Fn(ExportNews)) {
        tell(ExportNews::Counted {
            job: self.job,
            percent: 100,
            peak_db: self.peak_db(),
            over: self.over,
            length_s: self.seconds(self.longest),
            remaining_s: Some(0),
            lufs: self.loudness.0,
            true_peak_db: self.loudness.1,
            done: true,
        });
    }
}

/// Sorts what comes out of the engine into what the file gets: not the head the master's
/// inserts delay everything by, all of the range, and of what rings on after it as much as
/// the tail says.
struct Taker {
    out: CacheWav,
    head_left: u64,
    range_left: u64,
    tail: ExportTail,
    /// Frames after the range that are in the file or held.
    tail_frames: u64,
    tail_max: u64,
    /// Quiet blocks after the range, not written unless something louder follows.
    held: Vec<f32>,
    quiet_for: u64,
    rate: u32,
    peak: f32,
    places: OverPlaces,
    /// The sum of every sample squared, for the levels of the AI link.
    squares: f64,
    written: u64,
    over: bool,
    /// Hears the range for the AI link, or for F1.
    listen: Option<Listener>,
    /// F1: hears all that is written, the tail too.
    true_peak: Option<TruePeak>,
}

/// What hears a rendering besides the file.
struct Listening {
    listen: Option<Listener>,
    true_peak: Option<TruePeak>,
}

fn peak_of(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

/// Counts the places of a file that go over 0 dB (docs/history/design-notes.ja.md B-95-2): a run of frames
/// with a sample over 0 dB is one place, and frames not over for [`OVER_GAP_S`] end it.
/// A sample of exactly 0 dB is not over.
#[derive(Debug)]
struct OverPlaces {
    gap: u64,
    /// Frames not over since the last frame that was; `None` before the first.
    since: Option<u64>,
    count: u32,
}

impl OverPlaces {
    fn new(rate: u32) -> OverPlaces {
        OverPlaces {
            gap: ((OVER_GAP_S * f64::from(rate)).round() as u64).max(1),
            since: None,
            count: 0,
        }
    }

    /// Interleaved stereo, in the order of the file.
    fn feed(&mut self, samples: &[f32]) {
        if peak_of(samples) <= 1.0 {
            if let Some(since) = &mut self.since {
                *since += (samples.len() / 2) as u64;
            }
            return;
        }
        for frame in samples.as_chunks::<2>().0 {
            if frame[0].abs() > 1.0 || frame[1].abs() > 1.0 {
                if self.since.is_none_or(|since| since >= self.gap) {
                    self.count = self.count.saturating_add(1);
                }
                self.since = Some(0);
            } else if let Some(since) = &mut self.since {
                *since += 1;
            }
        }
    }
}

impl Taker {
    /// F1 so far: the loudness of the range and the true peak of the file.
    fn loudness(&self) -> (Option<f32>, Option<f32>) {
        match &self.true_peak {
            Some(true_peak) => (
                self.listen.as_ref().and_then(Listener::lufs_now),
                true_peak.db(),
            ),
            None => (None, None),
        }
    }

    fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        self.peak = self.peak.max(peak_of(samples));
        self.places.feed(samples);
        if let Some(true_peak) = &mut self.true_peak {
            true_peak.feed(samples);
        }
        self.squares += samples
            .iter()
            .map(|s| f64::from(*s) * f64::from(*s))
            .sum::<f64>();
        self.written += (samples.len() / 2) as u64;
        self.out.write(samples)
    }

    /// One block, interleaved stereo.
    fn take(&mut self, mut block: &[f32]) -> io::Result<()> {
        let skip = self.head_left.min((block.len() / 2) as u64);
        self.head_left -= skip;
        block = &block[skip as usize * 2..];
        let of_range = self.range_left.min((block.len() / 2) as u64);
        self.range_left -= of_range;
        let (range, after) = block.split_at(of_range as usize * 2);
        if let Some(listen) = &mut self.listen {
            listen.feed(range);
        }
        self.write(range)?;
        if self.range_left > 0 || self.head_left > 0 {
            return Ok(());
        }
        if self.tail == ExportTail::Off {
            self.over = true;
            return Ok(());
        }
        let room = self.tail_max.saturating_sub(self.tail_frames);
        let after = &after[..after.len().min(room as usize * 2)];
        self.tail_frames += (after.len() / 2) as u64;
        if peak_of(after) < QUIET {
            self.held.extend_from_slice(after);
            self.quiet_for += (after.len() / 2) as u64;
        } else {
            let held = std::mem::take(&mut self.held);
            self.write(&held)?;
            self.held = held;
            self.held.clear();
            self.quiet_for = 0;
            self.write(after)?;
        }
        let quiet = (QUIET_S * f64::from(self.rate)) as u64;
        self.over = self.quiet_for >= quiet || self.tail_frames >= self.tail_max;
        Ok(())
    }
}

/// Plays the range once, from where `play_from` put the engine, and writes it to `path`.
/// `each` is called after every block with the frames of the range written so far and what
/// the file holds so far.
/// Returns the frames of the file and what was in them, what `listen` heard of the range,
/// and the true peak of the file.
fn render_pass(
    engine: &mut ExportEngine,
    job: &ExportJob,
    path: &Path,
    realtime: bool,
    each: &mut dyn FnMut(u64, &Taker) -> Result<(), Ended>,
    listening: Listening,
) -> Result<(PassLevels, Option<Heard>, Option<f32>), Ended> {
    let range = range_frames(engine, job.span);
    let mut taker = Taker {
        out: CacheWav::create(path, job.sample_rate, 2)?,
        head_left: engine.master_latency() as u64,
        range_left: range,
        tail: job.tail,
        tail_frames: 0,
        tail_max: u64::from(EXPORT_TAIL_MAX_S) * u64::from(job.sample_rate),
        held: Vec::new(),
        quiet_for: 0,
        rate: job.sample_rate,
        peak: 0.0,
        places: OverPlaces::new(job.sample_rate),
        squares: 0.0,
        written: 0,
        over: false,
        listen: listening.listen,
        true_peak: listening.true_peak,
    };
    let started = Instant::now();
    let block = engine.block() as u64;
    // Frames the engine has played since the range began.
    let mut played = 0u64;
    let result = loop {
        // The block that ends the range ends on its last frame: the song stops there.
        let frames = match range.checked_sub(played) {
            Some(left) if left > 0 => block.min(left),
            _ => block,
        };
        let out = engine.render(frames as usize);
        if let Err(e) = taker.take(out) {
            break Err(e.into());
        }
        played += frames;
        if played == range {
            engine.stop();
        }
        if let Err(e) = each(taker.written.min(range), &taker) {
            break Err(e);
        }
        if taker.over {
            break Ok(());
        }
        if realtime {
            let due = Duration::from_secs_f64(played as f64 / f64::from(job.sample_rate));
            std::thread::sleep(due.saturating_sub(started.elapsed()));
        }
    };
    match result {
        Ok(()) => {
            let frames = taker.out.finish()?;
            let samples = taker.written.saturating_mul(2).max(1);
            let true_peak_db = taker.true_peak.as_ref().and_then(TruePeak::db);
            let heard = taker.listen.map(Listener::finish);
            let levels = PassLevels {
                peak: taker.peak,
                rms: (taker.squares / samples as f64).sqrt() as f32,
                over: taker.places.count,
                frames,
                lufs: heard.as_ref().and_then(|h| h.lufs),
                lufs_short_max: heard.as_ref().and_then(|h| h.lufs_short_max),
            };
            Ok((levels, heard, true_peak_db))
        }
        Err(e) => {
            taker.out.abandon();
            Err(e)
        }
    }
}

/// The audio files that are still being read in (decoded, brought to the rate the audio
/// runs at) are waited for: their clips are silent until then. One that is not found is
/// not waited for, and one that takes longer than [`FILES_WAIT`] is given up.
fn wait_files(setup: &Setup, stop: &AtomicBool) -> Result<(), Ended> {
    let job = &setup.job;
    let until = Instant::now() + FILES_WAIT;
    let coming = || {
        (job.doc.audio.iter()).any(|source| {
            let playable = (setup.store).playable(&source.uid, &source.file, job.sample_rate);
            matches!(playable, Playable::NeedsCache(_))
        })
    };
    while coming() && Instant::now() < until {
        if stop.load(Relaxed) {
            return Err(Ended::Dropped);
        }
        std::thread::sleep(POLL);
    }
    Ok(())
}

/// The audio that is made for stretched and pitched clips is in the cache before the
/// first block: what the maker's thread is at is waited for, the rest is made here.
fn wait_made(engine: &mut ExportEngine, setup: &Setup, stop: &AtomicBool) -> Result<(), Ended> {
    let plans = engine.plans();
    if plans.is_empty() {
        return Ok(());
    }
    let store = &setup.store;
    let there = |key: &MadeKey| store.made_path(key).is_some_and(|p| p.is_file());
    // What cannot be made (its file is not there): the clip plays as it does while
    // playing without it.
    let mut left_out: HashSet<MadeKey> = HashSet::new();
    loop {
        if stop.load(Relaxed) {
            return Err(Ended::Dropped);
        }
        let next = (plans.iter()).find(|p| !left_out.contains(&p.key) && !there(&p.key));
        let Some(plan) = next else {
            break;
        };
        let busy = (setup.maker_waiting.as_ref()).is_some_and(|w| w.load(Relaxed) > 0);
        if busy {
            std::thread::sleep(POLL);
            continue;
        }
        match make_now(store, plan, stop) {
            Ok(true) => {}
            Ok(false) => {
                left_out.insert(plan.key);
            }
            Err(e) => {
                eprintln!("meloet: the audio of a clip was not made for the export: {e}");
                left_out.insert(plan.key);
            }
        }
    }
    if stop.load(Relaxed) {
        return Err(Ended::Dropped);
    }
    let made: HashSet<MadeKey> = (plans.iter().map(|p| p.key)).filter(there).collect();
    engine.set_made(made);
    Ok(())
}

fn load_plugins(engine: &ExportEngine, told: &Told, setup: &Setup) -> Result<ExportSeats, Ended> {
    let failed = |name: &str| {
        Ended::Failed(ExportFailure::Plugin {
            name: name.to_owned(),
        })
    };
    let first = told.plugins.first().map_or("", |p| &*p.name);
    // What the plugins that play say of themselves now, as a save asks them; one that
    // does not answer goes with what the song holds.
    let mut live: Vec<(PluginPlace, Vec<u8>)> = Vec::new();
    if let (Some(states), false) = (&setup.states, told.plugins.is_empty()) {
        let until = Instant::now() + STATE_WAIT;
        while let Ok((place, state)) =
            states.recv_timeout(until.saturating_duration_since(Instant::now()))
        {
            if let Ok(state) = state {
                live.push((place, state));
            }
        }
    }
    let mut plugins = Vec::new();
    for p in &told.plugins {
        let key = p.key.parse().map_err(|_| failed(&p.name))?;
        let state = match live.iter().position(|(place, _)| *place == p.place) {
            Some(at) => live.swap_remove(at).1,
            None => p.state.to_vec(),
        };
        plugins.push(ExportPlugin {
            place: p.place,
            name: p.name.clone(),
            key,
            bundle: p.bundle.clone(),
            state,
            outputs: p.outputs.clone(),
        });
    }
    let want = ExportWant {
        plugins,
        sample_rate: engine.rate(),
        max_frames: engine.max_frames() as u32,
    };
    let Some(options) = &setup.hosts else {
        return match want.plugins.is_empty() {
            true => Ok(ExportSeats::none()),
            false => Err(failed(first)),
        };
    };
    export_seats(options, want, engine.plugin_block()).map_err(|e| match e {
        ExportPluginError::Load { name, .. } => failed(&name),
        ExportPluginError::NotYet => failed(first),
        ExportPluginError::Start(why) => {
            eprintln!("meloet: the export's plugin process did not start: {why}");
            failed(first)
        }
    })
}

/// Brings a rendered file into the form of `target`, as the file `to` of the work place.
/// `gain` (dB) is on every sample before anything else; without it the samples are as read.
/// Returns the waveform of what is written (E9), before the sample rate is changed.
fn convert(
    from: &Path,
    to: &Path,
    rate: u32,
    target: &ExportTarget,
    gain: Option<f32>,
    stop: &AtomicBool,
) -> Result<Vec<u8>, Ended> {
    let factor = gain.map(loudness::factor);
    let mut wav = WavPcm::open(from)?;
    let mut wave = WaveBars::new(wav.frames());
    let spec = SinkSpec {
        format: target.format,
        sample_rate: target.sample_rate,
        channels: 2,
        bits: target.bits,
        mp3_kbps: target.mp3_kbps,
    };
    let mut out = sink::create(to, spec)?;
    let mut work = || -> Result<(), Ended> {
        let mut resample = match rate == target.sample_rate {
            true => None,
            false => Some(Resample::new(rate, target.sample_rate, 2)?),
        };
        let mut buf = vec![0.0f32; CHUNK * 2];
        let mut at = 0u64;
        loop {
            if stop.load(Relaxed) {
                return Err(Ended::Dropped);
            }
            let got = wav.read(at, &mut buf)?;
            if got == 0 {
                break;
            }
            at += got as u64;
            if let Some(factor) = factor {
                buf[..got * 2].iter_mut().for_each(|s| *s *= factor);
            }
            wave.feed(&buf[..got * 2]);
            match &mut resample {
                Some(r) => r.push(&buf[..got * 2], &mut |block| out.write(block))?,
                None => out.write(&buf[..got * 2])?,
            }
        }
        if let Some(r) = resample {
            r.finish(&mut |block| out.write(block))?;
        }
        Ok(())
    };
    match work() {
        Ok(()) => {
            out.finish()?;
            Ok(wave.finish())
        }
        Err(e) => {
            out.abandon();
            Err(e)
        }
    }
}

fn sync_dir(dir: &Path) {
    if let Ok(dir) = fs::File::open(dir) {
        let _ = dir.sync_all();
    }
}

/// Moves a finished file, which is on the disk, to where the person wants it: at no time
/// is a half-written file under that name. In another file system it is copied to a name
/// beside the place first.
fn move_to(from: &Path, to: &Path) -> io::Result<()> {
    let dir = to.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    match fs::rename(from, to) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            let mut part = to.as_os_str().to_owned();
            part.push(".part");
            let part = PathBuf::from(part);
            let copied = (|| {
                fs::copy(from, &part)?;
                fs::File::open(&part)?.sync_all()?;
                fs::rename(&part, to)
            })();
            if let Err(e) = copied {
                let _ = fs::remove_file(&part);
                return Err(e);
            }
            let _ = fs::remove_file(from);
        }
        Err(e) => return Err(e),
    }
    sync_dir(dir);
    Ok(())
}

#[cfg(test)]
mod tests;
