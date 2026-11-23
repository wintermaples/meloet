//! The export's asks on the core thread (docs/architecture.md 22-11, 22-20): what a job
//! needs of the core is gathered here, and the job goes off on its own thread
//! (`crate::export::job`). The playing engine is not touched.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use meloet_model::{Doc, InstrumentRef};

use super::{Input, Msg, ShellRequest, Worker};
use crate::export::job::{self, JobHandle, LiveState, Setup};
use crate::export::{ExportAsk, ExportJob, ExportNews, ExportTarget};
use crate::instruments::Samples;
use meloet_protocol::ExportFailure;

/// How long a job waits for the samples of the song's instruments to be read before it
/// starts without them (their tracks are silent then, as they are while playing).
const SAMPLES_WAIT: Duration = Duration::from_secs(30);
/// The frames of a block when no stream ever said its own.
const USUAL_BLOCK: usize = 256;
/// The ids of the AI link's jobs start here, far from the rules' own.
pub(super) const AI_JOBS: u64 = 1 << 48;

/// Who hears a job: the rules, or the AI link's caller waiting on its own thread.
type Teller = Option<mpsc::Sender<ExportNews>>;

/// A job that is asked for and not started.
struct Waiting {
    job: Box<ExportJob>,
    since: Instant,
    target: Option<ExportTarget>,
    tell: Teller,
}

pub(super) struct Exports {
    /// Where every job gets a folder.
    root: PathBuf,
    jobs: Vec<JobHandle>,
    waiting: Vec<Waiting>,
}

impl Exports {
    pub fn new(state_dir: Option<&Path>) -> Exports {
        let root = match state_dir {
            Some(dir) => dir.join("export"),
            None => std::env::temp_dir().join("meloet-export"),
        };
        remove_left(&root);
        Exports {
            root,
            jobs: Vec::new(),
            waiting: Vec::new(),
        }
    }

    fn work(&self, job: u64) -> PathBuf {
        self.root.join(format!("{}-{job}", std::process::id()))
    }

    /// Gives every job up and waits until its files are gone and its plugin processes
    /// have ended.
    pub fn end(&mut self) {
        self.waiting.clear();
        for job in &self.jobs {
            job.give_up();
        }
        for job in self.jobs.drain(..) {
            job.join();
        }
    }
}

impl Drop for Exports {
    fn drop(&mut self) {
        self.end();
    }
}

/// The folders of jobs whose meloet is gone (it ended in the middle).
fn remove_left(root: &Path) {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let pid = name.to_string_lossy().split('-').next().map(str::to_owned);
        let Some(pid) = pid.and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        if pid != std::process::id() && !Path::new(&format!("/proc/{pid}")).exists() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

impl Worker {
    /// [`Effect::Export`](crate::state::Effect::Export). Returns what the rules are to
    /// hear at once.
    pub(super) fn export_ask(&mut self, ask: ExportAsk) -> Option<Input> {
        match ask {
            ExportAsk::Render(job) => self.export_render(job, None),
            ExportAsk::Drop { job } => {
                self.exports.waiting.retain(|w| w.job.id != job);
                if let Some(handle) = self.exports.jobs.iter().find(|j| j.id == job) {
                    handle.give_up();
                }
                None
            }
            ExportAsk::Finish { job, target } => {
                if let Some(handle) = self.exports.jobs.iter().find(|j| j.id == job) {
                    handle.finish(target);
                } else if let Some(w) = self.exports.waiting.iter_mut().find(|w| w.job.id == job) {
                    w.target = Some(target);
                }
                None
            }
            ExportAsk::PickPlace {
                folder,
                name,
                folder_only,
            } => {
                let folder = folder.filter(|f| f.is_dir());
                let asked = self.ask_shell(ShellRequest::PickExportPlace {
                    folder,
                    name,
                    folder_only,
                });
                (!asked).then_some(Input::Export(ExportNews::PlacePicked(None)))
            }
            ExportAsk::PickPicture { folder } => {
                let folder = folder.filter(|f| f.is_dir());
                let asked = self.ask_shell(ShellRequest::PickExportPicture { folder });
                (!asked).then_some(Input::Export(ExportNews::PicturePicked(None)))
            }
        }
    }

    /// A job waits for the samples of the song, then starts. With `tell` its news go there,
    /// not to the rules.
    pub(super) fn export_render(&mut self, job: Box<ExportJob>, tell: Teller) -> Option<Input> {
        self.exports.waiting.push(Waiting {
            job,
            since: Instant::now(),
            target: None,
            tell,
        });
        self.export_tick()
    }

    /// Jobs of the AI link that are asked for or running.
    pub(super) fn ai_jobs(&self) -> usize {
        let waiting = self.exports.waiting.iter().filter(|w| w.tell.is_some());
        let running = self.exports.jobs.iter().filter(|j| j.id >= AI_JOBS);
        waiting.count() + running.filter(|j| !j.is_over()).count()
    }

    /// Starts the jobs that can start, and lets go of the ones that have ended. Returns
    /// what the rules are to hear of a job that could not start.
    pub(super) fn export_tick(&mut self) -> Option<Input> {
        self.exports.jobs.retain(|job| !job.is_over());
        let mut failed = None;
        let waiting = std::mem::take(&mut self.exports.waiting);
        for w in waiting {
            let (samples, read) = self.export_samples(&w.job.doc);
            if !read && w.since.elapsed() < SAMPLES_WAIT {
                self.exports.waiting.push(w);
                continue;
            }
            let id = w.job.id;
            match self.start_export(*w.job, samples, w.tell.clone()) {
                Ok(handle) => {
                    if let Some(target) = w.target {
                        handle.finish(target);
                    }
                    self.exports.jobs.push(handle);
                }
                Err(e) => {
                    eprintln!("meloet: the export did not start: {e}");
                    let news = ExportNews::Failed {
                        job: id,
                        reason: ExportFailure::CannotWrite,
                    };
                    match w.tell {
                        Some(tell) => {
                            let _ = tell.send(news);
                        }
                        None => failed = Some(Input::Export(news)),
                    }
                }
            }
        }
        failed
    }

    /// The samples of the instruments of `doc` as they are in memory now, and whether
    /// none of them is still to be read.
    fn export_samples(&self, doc: &Doc) -> (Vec<(InstrumentRef, Samples)>, bool) {
        let mut all: Vec<(InstrumentRef, Samples)> = Vec::new();
        let mut read = true;
        for track in &doc.tracks {
            if all.iter().any(|(i, _)| *i == track.instrument) {
                continue;
            }
            let samples = match &track.instrument {
                InstrumentRef::Builtin(b) => self.audio.sounds.samples(*b),
                InstrumentRef::Sf2(preset) => self.audio.sf2.samples(preset),
                InstrumentRef::Plugin(_) => continue,
            };
            read &= !matches!(samples, Samples::NotRead);
            all.push((track.instrument.clone(), samples));
        }
        (all, read)
    }

    fn start_export(
        &mut self,
        job: ExportJob,
        samples: Vec<(InstrumentRef, Samples)>,
        to: Teller,
    ) -> std::io::Result<JobHandle> {
        let audio = &self.audio;
        let said = match audio.stream.is_some() {
            true => audio.handle.meters().last_frames() as usize,
            false => 0,
        };
        let block = [said, audio.wish_frames as usize, USUAL_BLOCK]
            .into_iter()
            .find(|frames| *frames > 0)
            .unwrap_or(USUAL_BLOCK);
        // What the plugins that play say of themselves now, as a save asks them.
        let mut states = None;
        if let Some(side) = &self.plugins {
            let (tx, rx) = mpsc::channel::<LiveState>();
            for (place, ticket) in self.model.plugin_tickets() {
                if let Some((id, _)) = side.ids.get(&ticket) {
                    let tx = tx.clone();
                    let done = move |state| {
                        let _ = tx.send((place, state));
                    };
                    side.hosts.state(*id, Box::new(done));
                }
            }
            states = Some(rx);
        }
        let tx = self.links.tx.clone();
        let tell = move |news: ExportNews| match &to {
            Some(to) => {
                let _ = to.send(news);
            }
            None => {
                let _ = tx.send(Msg::Input(Input::Export(news), None));
            }
        };
        let setup = Setup {
            work: self.exports.work(job.id),
            block,
            store: self.audio_store.clone(),
            samples,
            known: self.model.known_plugins(),
            hosts: self.plugins.as_ref().map(|side| side.options.clone()),
            states,
            maker_waiting: audio.maker.as_ref().map(|m| m.waiting_count()),
            job,
        };
        job::spawn(setup, tell)
    }
}
