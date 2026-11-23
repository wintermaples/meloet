//! The thread that reads audio files in (`meloet-audio`): it copies files into the work
//! place, finds out what they are, decodes and resamples them into the cache, and makes
//! their peaks. One file at a time; an ask that comes while a long file is decoded is
//! answered between two of its packets.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meloet_model::{AudioSource, ImportedAudio, NewAudio, PlayedFrames};
use sha2::{Digest, Sha256};

use super::decode::{self, Decoder, Unread};
use super::resample::Resample;
use super::wav::CacheWav;
use super::{AudioStore, Playable};
use crate::peaks::{PeakBuilder, SharedPeaks, read_peaks_head};

/// A percent is told this often at most.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// Peaks that are being made. Two are the same when they are the same peaks.
#[derive(Debug, Clone)]
pub struct Growing(pub SharedPeaks);

impl PartialEq for Growing {
    fn eq(&self, other: &Growing) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// A file of the song, as the keeper found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    /// It can be played.
    Ready,
    /// It is being decoded into the cache; a `Progress` and a `Done` follow.
    Loading,
    /// Neither the song's folder nor the work place has it.
    Missing,
}

/// A file that was asked to be brought in.
#[derive(Debug, Clone, PartialEq)]
pub enum Brought {
    /// It is in the work place.
    In(ImportedAudio),
    /// It does not read, and why; the name it has on the disk.
    Unreadable(String, Unread),
}

/// What [`Keeper::find`] made of the file it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finding {
    /// It is where the song looks for it now.
    Taken,
    /// It reads, at another rate or with other channels than the song's audio.
    Mismatch,
    /// It does not read, or could not be copied.
    Unreadable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioNews {
    /// The answer to [`Keeper::bring`], in the order of the paths.
    Brought { ticket: u64, files: Vec<Brought> },
    /// The answer to [`Keeper::watch`]: every file of the song, once.
    States {
        serial: u32,
        states: Vec<([u8; 16], SourceState)>,
    },
    /// The peaks of the file are being made: the waveform can be drawn as far as they are.
    Peaks { uid: [u8; 16], peaks: Growing },
    /// 0..=99, of a file that is `Loading`.
    Progress { uid: [u8; 16], percent: u8 },
    /// The file is ready (`ok`), or it did not decode. Its peaks are in their file.
    Done { uid: [u8; 16], ok: bool },
    /// The answer to [`Keeper::find`]. `file`: the name of the file that was given.
    Found {
        audio: u32,
        file: String,
        found: Finding,
    },
    /// The answer to [`Keeper::measure`]: the loudest sample of each, in the order asked;
    /// `None` where the file could not be read.
    Measured {
        ticket: u64,
        peaks: Vec<Option<f32>>,
    },
    /// The answer to [`Keeper::reverse`]: the file made for each, in the order asked; `None`
    /// where none was made.
    Reversed {
        ticket: u64,
        files: Vec<Option<NewAudio>>,
    },
    /// The answer to [`Keeper::adopt`]; `None` when the file could not be taken.
    Adopted {
        ticket: u64,
        audio: Option<NewAudio>,
    },
}

enum Ask {
    Bring {
        ticket: u64,
        paths: Vec<PathBuf>,
        taken: Vec<String>,
    },
    Watch {
        serial: u32,
        sources: Vec<Arc<AudioSource>>,
        sample_rate: Option<u32>,
    },
    Find {
        source: Arc<AudioSource>,
        path: PathBuf,
    },
    Measure {
        ticket: u64,
        played: Vec<PlayedFrames>,
    },
    Reverse {
        ticket: u64,
        played: Vec<(PlayedFrames, String)>,
        taken: Vec<String>,
    },
    Adopt {
        ticket: u64,
        temp: PathBuf,
        wanted: String,
        taken: Vec<String>,
    },
}

/// The thread's handle. Dropping it ends the thread after the ask it is at.
pub struct Keeper {
    tx: Sender<Ask>,
}

impl Keeper {
    /// `tell` is called on the keeper's thread.
    pub fn spawn(
        store: Arc<AudioStore>,
        tell: impl Fn(AudioNews) + Send + 'static,
    ) -> std::io::Result<Keeper> {
        Keeper::spawn_paused(store, Duration::ZERO, tell)
    }

    /// As [`Keeper::spawn`], with a wait of `pause` at every packet of a file being made
    /// ready: a test's long file then takes long in any build.
    pub(super) fn spawn_paused(
        store: Arc<AudioStore>,
        pause: Duration,
        tell: impl Fn(AudioNews) + Send + 'static,
    ) -> std::io::Result<Keeper> {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("meloet-audio".into())
            .spawn(move || {
                Work {
                    store,
                    tell: Box::new(tell),
                    queue: VecDeque::new(),
                    job: None,
                    sample_rate: None,
                    peaks_made: HashSet::new(),
                    pause,
                }
                .run(&rx)
            })?;
        Ok(Keeper { tx })
    }

    /// Copies the files into the work place and finds out what they are. `taken`: names
    /// of the song's audio table that a new file must not get.
    pub fn bring(&self, ticket: u64, paths: Vec<PathBuf>, taken: Vec<String>) {
        let _ = self.tx.send(Ask::Bring {
            ticket,
            paths,
            taken,
        });
    }

    /// The song's audio table, and the rate the engine runs at (`None` without a stream):
    /// says which files are there, and makes ready what is not (the cache, the peaks).
    /// Work for files that are no longer in the table is given up.
    pub fn watch(&self, serial: u32, sources: Vec<Arc<AudioSource>>, sample_rate: Option<u32>) {
        let _ = self.tx.send(Ask::Watch {
            serial,
            sources,
            sample_rate,
        });
    }

    /// 「さがす…」: the file at `path` is taken for the missing `source`.
    pub fn find(&self, source: Arc<AudioSource>, path: PathBuf) {
        let _ = self.tx.send(Ask::Find { source, path });
    }

    /// 「ノーマライズ」: the loudest sample of the frames each clip plays.
    pub fn measure(&self, ticket: u64, played: Vec<PlayedFrames>) {
        let _ = self.tx.send(Ask::Measure { ticket, played });
    }

    /// 「逆にする」: a file of the frames each clip plays, backwards, named as wanted (see
    /// `clips::reverse`). `taken`: names of the song's audio table a new file must not get.
    pub fn reverse(&self, ticket: u64, played: Vec<(PlayedFrames, String)>, taken: Vec<String>) {
        let _ = self.tx.send(Ask::Reverse {
            ticket,
            played,
            taken,
        });
    }

    /// 「音のクリップにする」: the file at `temp`, which the export wrote into the work place,
    /// is named as wanted (see `bounced::adopt`) and gets its peaks. `taken`: names of the
    /// song's audio table a new file must not get.
    pub fn adopt(&self, ticket: u64, temp: PathBuf, wanted: String, taken: Vec<String>) {
        let _ = self.tx.send(Ask::Adopt {
            ticket,
            temp,
            wanted,
            taken,
        });
    }
}

/// What a file still needs.
#[derive(Debug, Clone, PartialEq)]
struct Need {
    source: Arc<AudioSource>,
    path: PathBuf,
    /// The rate to bring it to; `None`: only its peaks are made.
    cache_rate: Option<u32>,
    peaks: bool,
}

struct Job {
    need: Need,
    decoder: Decoder,
    peaks: Option<SharedPeaks>,
    cache: Option<(Resample, CacheWav)>,
    frames: u64,
    percent: u8,
    told: Instant,
}

struct Work {
    store: Arc<AudioStore>,
    tell: Box<dyn Fn(AudioNews) + Send>,
    queue: VecDeque<Need>,
    job: Option<Job>,
    sample_rate: Option<u32>,
    /// Files whose peaks were made in this run: they are not made again when their file
    /// says another length than the song does.
    peaks_made: HashSet<[u8; 16]>,
    pause: Duration,
}

/// The key of the caches of a file brought in: the same bytes get the same key, in every
/// song.
fn uid_of(sha: &[u8; 32]) -> [u8; 16] {
    let mut hash = Sha256::new();
    hash.update(b"meloet audio file");
    hash.update(sha);
    let mut uid = [0u8; 16];
    uid.copy_from_slice(&hash.finalize()[..16]);
    uid
}

fn name_of(path: &Path) -> String {
    (path.file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

impl Work {
    fn run(mut self, rx: &Receiver<Ask>) {
        loop {
            if self.job.is_none() {
                self.start_next();
            }
            let ask = match &self.job {
                Some(_) => match rx.try_recv() {
                    Ok(ask) => Some(ask),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => break,
                },
                None => match rx.recv() {
                    Ok(ask) => Some(ask),
                    Err(_) => break,
                },
            };
            match ask {
                Some(ask) => self.ask(ask),
                None => self.step(),
            }
        }
        if let Some(job) = self.job.take() {
            give_up(job);
        }
    }

    fn ask(&mut self, ask: Ask) {
        match ask {
            Ask::Bring {
                ticket,
                paths,
                mut taken,
            } => {
                let files = (paths.iter())
                    .map(|path| match self.bring(path, &taken) {
                        Ok(file) => {
                            taken.push(file.audio.file.clone());
                            Brought::In(file)
                        }
                        Err((why, unread)) => {
                            eprintln!("meloet: {} was not brought in: {why}", path.display());
                            Brought::Unreadable(name_of(path), unread)
                        }
                    })
                    .collect();
                (self.tell)(AudioNews::Brought { ticket, files });
            }
            Ask::Watch {
                serial,
                sources,
                sample_rate,
            } => self.watch(serial, &sources, sample_rate),
            Ask::Find { source, path } => {
                let found = self.found(&source, &path);
                (self.tell)(AudioNews::Found {
                    audio: source.id.0,
                    file: name_of(&path),
                    found,
                });
            }
            Ask::Measure { ticket, played } => {
                let peaks = (played.iter())
                    .map(|p| {
                        (super::clips::peak(&self.store, p))
                            .inspect_err(|e| eprintln!("meloet: a clip was not measured: {e}"))
                            .ok()
                    })
                    .collect();
                (self.tell)(AudioNews::Measured { ticket, peaks });
            }
            Ask::Reverse {
                ticket,
                played,
                taken,
            } => {
                let files = (played.iter())
                    .map(|(p, wanted)| {
                        match super::clips::reverse(&self.store, p, wanted, &taken) {
                            Ok(audio) => {
                                self.peaks_made.insert(audio.uid);
                                Some(audio)
                            }
                            Err(e) => {
                                eprintln!("meloet: a clip was not reversed: {e}");
                                None
                            }
                        }
                    })
                    .collect();
                (self.tell)(AudioNews::Reversed { ticket, files });
            }
            Ask::Adopt {
                ticket,
                temp,
                wanted,
                taken,
            } => {
                let audio = match super::bounced::adopt(&self.store, &temp, &wanted, &taken) {
                    Ok(audio) => {
                        self.peaks_made.insert(audio.uid);
                        Some(audio)
                    }
                    Err(e) => {
                        eprintln!("meloet: the audio of the track was not taken in: {e}");
                        None
                    }
                };
                (self.tell)(AudioNews::Adopted { ticket, audio });
            }
        }
    }

    fn bring(&self, path: &Path, taken: &[String]) -> Result<ImportedAudio, (String, Unread)> {
        let facts = decode::facts(path).map_err(|e| (e.detail, e.why))?;
        let (file, sha256) =
            (self.store.bring_in(path, taken)).map_err(|e| (e.to_string(), Unread::NotAudio))?;
        let name = (path.file_stem())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.clone());
        Ok(ImportedAudio {
            audio: NewAudio {
                uid: uid_of(&sha256),
                file,
                frames: facts.frames,
                sample_rate: facts.sample_rate,
                channels: facts.channels,
                bits: facts.bits,
            },
            sha256,
            name,
        })
    }

    /// Takes the file at `path` for `source` when it has as many channels at the same
    /// rate: a file that another program wrote again is still the take, so the SHA-256 is
    /// not asked to match.
    fn found(&self, source: &AudioSource, path: &Path) -> Finding {
        if self.store.find(&source.file).is_some() {
            return Finding::Taken;
        }
        let Some(work) = self.store.work_dir() else {
            return Finding::Unreadable;
        };
        match decode::facts(path) {
            Ok(f) if f.sample_rate == source.sample_rate && f.channels == source.channels => {}
            Ok(_)
            | Err(decode::ReadError {
                why: Unread::TooManyChannels,
                ..
            }) => return Finding::Mismatch,
            Err(_) => return Finding::Unreadable,
        }
        let to = work.join(&source.file);
        let copied =
            std::fs::create_dir_all(&work).and_then(|()| super::copy_into(path, &to, &mut |_| {}));
        match copied {
            Ok(()) => {
                self.store.file_arrived();
                Finding::Taken
            }
            Err(e) => {
                eprintln!("meloet: {} was not copied: {e}", path.display());
                Finding::Unreadable
            }
        }
    }

    fn watch(&mut self, serial: u32, sources: &[Arc<AudioSource>], sample_rate: Option<u32>) {
        self.sample_rate = sample_rate;
        let mut seen = HashSet::new();
        let mut needs = Vec::new();
        let mut states = Vec::new();
        for source in sources {
            if !seen.insert(source.uid) {
                continue;
            }
            let playable = match sample_rate {
                Some(rate) => self.store.playable(&source.uid, &source.file, rate),
                None => match self.store.find(&source.file) {
                    Some(_) => Playable::Ready,
                    None => Playable::Missing,
                },
            };
            if playable == Playable::Missing {
                states.push((source.uid, SourceState::Missing));
                continue;
            }
            let peaks = !self.peaks_made.contains(&source.uid)
                && !(self.store.peaks_path(&source.uid))
                    .is_some_and(|p| read_peaks_head(&p).is_ok_and(|h| h.frames == source.frames));
            let (state, cache_rate) = match (&playable, self.store.cache_path(&source.uid, 0)) {
                (Playable::NeedsCache(_), Some(_)) => (SourceState::Loading, sample_rate),
                // Without a cache folder the file cannot be made ready: it stays silent.
                (Playable::NeedsCache(_), None) => (SourceState::Missing, None),
                _ => (SourceState::Ready, None),
            };
            states.push((source.uid, state));
            let path = match playable {
                Playable::NeedsCache(path) => Some(path),
                _ => self.store.find(&source.file),
            };
            if let (true, Some(path)) = (cache_rate.is_some() || peaks, path) {
                needs.push(Need {
                    source: source.clone(),
                    path,
                    cache_rate,
                    peaks,
                });
            }
        }
        // Work that is still wanted goes on; the rest is given up.
        if let Some(job) = self.job.take_if(|job| !needs.contains(&job.need)) {
            give_up(job);
        }
        let running = self.job.as_ref().map(|job| job.need.clone());
        self.queue = (needs.into_iter())
            .filter(|need| Some(need) != running.as_ref())
            .collect();
        (self.tell)(AudioNews::States { serial, states });
        if let Some(job) = &self.job {
            if let Some(peaks) = &job.peaks {
                (self.tell)(AudioNews::Peaks {
                    uid: job.need.source.uid,
                    peaks: Growing(peaks.clone()),
                });
            }
            if job.cache.is_some() {
                (self.tell)(AudioNews::Progress {
                    uid: job.need.source.uid,
                    percent: job.percent,
                });
            }
        }
    }

    fn start_next(&mut self) {
        while let Some(need) = self.queue.pop_front() {
            let uid = need.source.uid;
            match self.start(need) {
                Ok(job) => {
                    if let Some(peaks) = &job.peaks {
                        (self.tell)(AudioNews::Peaks {
                            uid,
                            peaks: Growing(peaks.clone()),
                        });
                    }
                    self.job = Some(job);
                    return;
                }
                Err(why) => {
                    eprintln!("meloet: an audio file was not made ready: {why}");
                    (self.tell)(AudioNews::Done { uid, ok: false });
                }
            }
        }
    }

    fn start(&self, need: Need) -> Result<Job, String> {
        let decoder = Decoder::open(&need.path)?;
        let source = &need.source;
        let cache = match need.cache_rate {
            Some(rate) => {
                let path = (self.store.cache_path(&source.uid, rate)).ok_or("no cache folder")?;
                let wav =
                    CacheWav::create(&path, rate, source.channels).map_err(|e| e.to_string())?;
                let resample = Resample::new(decoder.sample_rate(), rate, source.channels)
                    .map_err(|e| e.to_string())?;
                Some((resample, wav))
            }
            None => None,
        };
        Ok(Job {
            peaks: (need.peaks).then(|| Arc::new(Mutex::new(PeakBuilder::new(source.channels)))),
            need,
            decoder,
            cache,
            frames: 0,
            percent: 0,
            told: Instant::now(),
        })
    }

    /// One packet of the file that is being made ready.
    fn step(&mut self) {
        let Some(job) = &mut self.job else {
            return;
        };
        if !self.pause.is_zero() {
            std::thread::sleep(self.pause);
        }
        let uid = job.need.source.uid;
        let channels = usize::from(job.need.source.channels);
        let stepped = (|| -> Result<bool, String> {
            let Some((decoded, block)) = job.decoder.next_block()? else {
                return Ok(false);
            };
            if usize::from(decoded) != channels {
                return Err("the file is not what the song says it is".into());
            }
            if let Some(peaks) = &job.peaks {
                peaks.lock().unwrap_or_else(|e| e.into_inner()).push(block);
            }
            if let Some((resample, wav)) = &mut job.cache {
                (resample.push(block, &mut |out| wav.write(out))).map_err(|e| e.to_string())?;
            }
            job.frames += (block.len() / channels) as u64;
            Ok(true)
        })();
        match stepped {
            Ok(true) => {
                let total = job.need.source.frames.max(1);
                let percent = (job.frames.saturating_mul(100) / total).min(99) as u8;
                if job.cache.is_some()
                    && percent != job.percent
                    && job.told.elapsed() >= PROGRESS_EVERY
                {
                    job.percent = percent;
                    job.told = Instant::now();
                    (self.tell)(AudioNews::Progress { uid, percent });
                }
            }
            Ok(false) => {
                let job = self.job.take().expect("checked above");
                let ok = match self.finish(job) {
                    Ok(()) => true,
                    Err(why) => {
                        eprintln!("meloet: an audio file was not made ready: {why}");
                        false
                    }
                };
                (self.tell)(AudioNews::Done { uid, ok });
            }
            Err(why) => {
                eprintln!("meloet: an audio file was not made ready: {why}");
                if let Some(job) = self.job.take() {
                    give_up(job);
                }
                (self.tell)(AudioNews::Done { uid, ok: false });
            }
        }
    }

    fn finish(&mut self, job: Job) -> Result<(), String> {
        let uid = job.need.source.uid;
        if let Some((resample, mut wav)) = job.cache {
            if let Err(e) = resample.finish(&mut |out| wav.write(out)) {
                wav.abandon();
                return Err(e.to_string());
            }
            wav.finish().map_err(|e| e.to_string())?;
            self.store.file_arrived();
        }
        if let Some(peaks) = job.peaks {
            self.peaks_made.insert(uid);
            if let Some(path) = self.store.peaks_path(&uid) {
                let peaks = peaks.lock().unwrap_or_else(|e| e.into_inner());
                peaks.write(&path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

fn give_up(job: Job) {
    if let Some((_, wav)) = job.cache {
        wav.abandon();
    }
}
