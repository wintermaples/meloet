//! The thread that makes the audio of stretched and pitched clips (`meloet-render`;
//! docs/architecture.md 22-8, 22-19 pass 2), and keeps the cache under its limit.
//!
//! It is told which audio the song needs now ([`Maker::want`], a [`MadePlan`] for every
//! clip that does not play its file as it is). What is in the cache already is said at
//! once ([`MadeNews::Ready`]). The rest is made one at a time, the clip nearest to the play
//! line first, a little after the last change ([`SETTLE`]): somebody who is still trying
//! values does not have every one of them made. A plan that is no longer wanted is given
//! up, also in the middle. The file gets its name only when it is whole.

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use meloet_stretch::{RenderError, render, render_tuned};

use crate::audiofiles::wav::CacheWav;
use crate::audiofiles::{AudioStore, CACHE_AUDIO, CACHE_MADE, CACHE_PEAKS, hex};
use crate::made::{MadeKey, MadePlan};
use crate::pcm::{PcmFile, PcmStore};

/// How long after the last change of what is wanted the making starts.
pub const SETTLE: Duration = Duration::from_millis(300);
/// A percent is told this often at most.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// How much the cache may hold (decoded audio, made audio, peaks) before the files that
/// were used longest ago go: 4 GiB.
pub const CACHE_LIMIT: u64 = 4 << 30;
/// The cache is looked through this often at most, but for a file that was just made.
const TRIM_EVERY: Duration = Duration::from_secs(30);
/// A file that is being written (`….part`) and was not touched for this long was left
/// behind by a meloet that ended in the middle.
const PART_LEFT: Duration = Duration::from_secs(600);

/// A clip whose audio is being made, or waits to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MakerJob {
    pub clip: u32,
    pub audio: u32,
    /// 0..=99; 0 while it waits.
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MadeNews {
    /// These of the wanted keys are in the cache.
    Ready(Vec<MadeKey>),
    /// Everything that is being made or waits, whenever it is another list (a percent at
    /// most every quarter of a second).
    Work(Vec<MakerJob>),
}

enum Ask {
    Want {
        plans: Vec<MadePlan>,
        uids: Vec<[u8; 16]>,
    },
    Retry,
    Quit,
}

/// The thread's handle. Dropping it gives up what is being made and ends the thread.
pub struct Maker {
    tx: Sender<Ask>,
    near: Arc<AtomicU64>,
    limit: Arc<AtomicU64>,
    waiting: Arc<AtomicUsize>,
    thread: Option<JoinHandle<()>>,
}

impl Maker {
    /// `tell` is called on the maker's thread.
    pub fn spawn(
        store: Arc<AudioStore>,
        tell: impl Fn(MadeNews) + Send + 'static,
    ) -> io::Result<Maker> {
        let (tx, rx) = mpsc::channel();
        let near = Arc::new(AtomicU64::new(0));
        let limit = Arc::new(AtomicU64::new(CACHE_LIMIT));
        let waiting = Arc::new(AtomicUsize::new(0));
        let work = Work {
            store,
            tell: Box::new(tell),
            rx,
            later: VecDeque::new(),
            plans: Vec::new(),
            uids: Vec::new(),
            ready: HashSet::new(),
            failed: HashSet::new(),
            start_at: Instant::now(),
            told: Vec::new(),
            trimmed: None,
            near: near.clone(),
            limit: limit.clone(),
            waiting: waiting.clone(),
        };
        let thread = std::thread::Builder::new()
            .name("meloet-render".into())
            .spawn(move || work.run())?;
        Ok(Maker {
            tx,
            near,
            limit,
            waiting,
            thread: Some(thread),
        })
    }

    /// What the song needs made now, all of it: a plan that is not in here any more is
    /// given up. `uids`: the audio files of the song, whose decoded audio and peaks the
    /// cache keeps whatever its size.
    pub fn want(&self, plans: Vec<MadePlan>, uids: Vec<[u8; 16]>) {
        let _ = self.tx.send(Ask::Want { plans, uids });
    }

    /// A file that could not be opened may be there now: what failed is tried again.
    pub fn retry(&self) {
        let _ = self.tx.send(Ask::Retry);
    }

    /// Where the play line is, in ticks: the clip nearest to it is made first.
    pub fn set_near(&self, tick: u64) {
        self.near.store(tick, Relaxed);
    }

    /// How much the cache may hold, in bytes ([`CACHE_LIMIT`] unless told).
    pub fn set_cache_limit(&self, bytes: u64) {
        self.limit.store(bytes, Relaxed);
    }

    /// How many wanted files are not made yet (nor failed). An export waits for 0.
    pub fn waiting(&self) -> usize {
        self.waiting.load(Relaxed)
    }

    /// [`Maker::waiting`] for another thread to look at.
    pub fn waiting_count(&self) -> Arc<AtomicUsize> {
        self.waiting.clone()
    }
}

impl Drop for Maker {
    fn drop(&mut self) {
        let _ = self.tx.send(Ask::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Work {
    store: Arc<AudioStore>,
    tell: Box<dyn Fn(MadeNews) + Send>,
    rx: Receiver<Ask>,
    /// Asks that came while a file was being made.
    later: VecDeque<Ask>,
    plans: Vec<MadePlan>,
    uids: Vec<[u8; 16]>,
    /// The wanted keys whose files are in the cache.
    ready: HashSet<MadeKey>,
    /// The wanted keys that could not be made; tried again after [`Maker::retry`].
    failed: HashSet<MadeKey>,
    start_at: Instant,
    told: Vec<MakerJob>,
    trimmed: Option<Instant>,
    near: Arc<AtomicU64>,
    limit: Arc<AtomicU64>,
    waiting: Arc<AtomicUsize>,
}

enum Outcome {
    Made,
    GivenUp,
    Failed,
    Quit,
}

fn jobs(
    plans: &[MadePlan],
    done: impl Fn(&MadeKey) -> bool,
    now: Option<(MadeKey, u8)>,
) -> Vec<MakerJob> {
    let mut jobs: Vec<MakerJob> = (plans.iter())
        .filter(|p| !done(&p.key))
        .map(|p| MakerJob {
            clip: p.clip,
            audio: p.audio,
            percent: now.filter(|(key, _)| *key == p.key).map_or(0, |n| n.1),
        })
        .collect();
    jobs.sort_unstable();
    jobs.dedup();
    jobs
}

impl Work {
    fn run(mut self) {
        if let Some(dir) = self.store.cache_dir() {
            remove_left_parts(&dir, SystemTime::now());
        }
        loop {
            let ask = match self.later.pop_front() {
                Some(ask) => ask,
                None => match self.next() {
                    None => match self.rx.recv() {
                        Ok(ask) => ask,
                        Err(_) => return,
                    },
                    Some(plan) => {
                        let wait = self.start_at.saturating_duration_since(Instant::now());
                        match self.rx.recv_timeout(wait) {
                            Ok(ask) => ask,
                            Err(RecvTimeoutError::Disconnected) => return,
                            Err(RecvTimeoutError::Timeout) => {
                                if self.make(&plan) {
                                    return;
                                }
                                continue;
                            }
                        }
                    }
                },
            };
            match ask {
                Ask::Quit => return,
                Ask::Retry => {
                    let before = self.pending();
                    self.failed.clear();
                    self.look(before, false);
                }
                Ask::Want { plans, uids } => {
                    let before = self.pending();
                    let plans = std::mem::replace(&mut self.plans, plans);
                    let uids = std::mem::replace(&mut self.uids, uids);
                    self.mark_changed(&plans, &uids);
                    self.look(before, true);
                }
            }
        }
    }

    fn pending(&self) -> Vec<MadeKey> {
        let mut keys: Vec<MadeKey> = (self.plans.iter())
            .map(|p| p.key)
            .filter(|k| !self.ready.contains(k) && !self.failed.contains(k))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    /// The plan to make next: of those ahead of the play line the nearest, then of those
    /// behind it.
    fn next(&self) -> Option<MadePlan> {
        let near = self.near.load(Relaxed);
        (self.plans.iter())
            .filter(|p| !self.ready.contains(&p.key) && !self.failed.contains(&p.key))
            .min_by_key(|p| match near < p.end {
                true => (0, p.start.saturating_sub(near)),
                false => (1, near - p.end),
            })
            .cloned()
    }

    /// After what is wanted changed: which of it the cache has, and what is to be made.
    /// `before`: what was to be made until now.
    fn look(&mut self, before: Vec<MadeKey>, changed: bool) {
        let wanted: HashSet<MadeKey> = self.plans.iter().map(|p| p.key).collect();
        self.failed.retain(|k| wanted.contains(k));
        self.ready = (wanted.into_iter())
            .filter(|key| (self.store.made_path(key)).is_some_and(|p| p.is_file()))
            .collect();
        if !self.ready.is_empty() {
            let mut ready: Vec<MadeKey> = self.ready.iter().copied().collect();
            ready.sort_unstable();
            (self.tell)(MadeNews::Ready(ready));
        }
        let pending = self.pending();
        if pending != before {
            self.start_at = Instant::now() + SETTLE;
        }
        self.waiting.store(pending.len(), Relaxed);
        self.tell_work(None);
        if changed && self.trimmed.is_none_or(|at| at.elapsed() >= TRIM_EVERY) {
            self.trim();
        }
    }

    fn tell_work(&mut self, now: Option<(MadeKey, u8)>) {
        let done = |k: &MadeKey| self.ready.contains(k) || self.failed.contains(k);
        let jobs = jobs(&self.plans, done, now);
        if jobs != self.told {
            self.told = jobs.clone();
            (self.tell)(MadeNews::Work(jobs));
        }
    }

    /// Makes the audio of one plan. Returns whether the thread is to end.
    fn make(&mut self, plan: &MadePlan) -> bool {
        let outcome = self.render(plan).unwrap_or_else(|e| {
            eprintln!("meloet: the audio of a clip was not made: {e}");
            Outcome::Failed
        });
        match outcome {
            Outcome::Quit => return true,
            Outcome::GivenUp => {}
            Outcome::Failed => {
                self.failed.insert(plan.key);
            }
            Outcome::Made => {
                self.ready.insert(plan.key);
                (self.tell)(MadeNews::Ready(vec![plan.key]));
                self.trim();
            }
        }
        self.waiting.store(self.pending().len(), Relaxed);
        // What came meanwhile says what is wanted now; else the list is one shorter.
        if self.later.is_empty() {
            self.tell_work(None);
        }
        false
    }

    fn render(&mut self, plan: &MadePlan) -> Result<Outcome, Box<dyn std::error::Error>> {
        let Some(path) = self.store.made_path(&plan.key) else {
            return Ok(Outcome::Failed);
        };
        let cancel = AtomicBool::new(false);
        let mut quit = false;
        let mut percent = 0u8;
        let mut told_at = Instant::now();
        let Work {
            store,
            rx,
            later,
            plans,
            ready,
            failed,
            told,
            tell,
            ..
        } = self;
        let mut wrote = || {
            while let Ok(ask) = rx.try_recv() {
                match &ask {
                    Ask::Quit => {
                        quit = true;
                        cancel.store(true, Relaxed);
                    }
                    Ask::Want { plans, .. } if !plans.iter().any(|p| p.key == plan.key) => {
                        cancel.store(true, Relaxed);
                    }
                    _ => {}
                }
                later.push_back(ask);
            }
        };
        let mut progress = |now: u8| {
            percent = now.min(99);
            // While asks wait, the list of what is wanted is about to be another one.
            if told_at.elapsed() >= PROGRESS_EVERY || told.is_empty() {
                told_at = Instant::now();
                let done = |k: &MadeKey| ready.contains(k) || failed.contains(k);
                let jobs = jobs(plans, done, Some((plan.key, percent)));
                if jobs != *told {
                    *told = jobs.clone();
                    tell(MadeNews::Work(jobs));
                }
            }
        };
        let made = render_plan(store, plan, &path, &mut wrote, &mut progress, &cancel)?;
        Ok(match made {
            Rendered::Made => Outcome::Made,
            // Not there yet (it is being decoded, or missing): tried again when the store
            // has other files.
            Rendered::NoSource => Outcome::Failed,
            Rendered::Cancelled if quit => Outcome::Quit,
            Rendered::Cancelled => Outcome::GivenUp,
        })
    }

    /// What came into the table or left it is in use at this moment: the cache is looked
    /// through only now and then, and would take a file that was used in between for as
    /// old as it was before. `plans`, `uids`: the table until now.
    fn mark_changed(&self, plans: &[MadePlan], uids: &[[u8; 16]]) {
        let Some(dir) = self.store.cache_dir() else {
            return;
        };
        let now = SystemTime::now();
        let old: HashSet<MadeKey> = plans.iter().map(|p| p.key).collect();
        let new: HashSet<MadeKey> = self.plans.iter().map(|p| p.key).collect();
        for key in old.symmetric_difference(&new) {
            if let Some(path) = self.store.made_path(key) {
                set_used(&path, now);
            }
        }
        let changed: Vec<String> = (uids.iter().filter(|u| !self.uids.contains(u)))
            .chain(self.uids.iter().filter(|u| !uids.contains(u)))
            .map(|u| hex(u))
            .collect();
        if changed.is_empty() {
            return;
        }
        for folder in [CACHE_AUDIO, CACHE_PEAKS] {
            for (path, _) in files_of(&dir.join(folder)) {
                let name = path.file_name().map(|n| n.to_string_lossy());
                let used = |name: &str| changed.iter().any(|u| name.starts_with(u.as_str()));
                if !is_part(&path) && name.is_some_and(|name| used(&name)) {
                    set_used(&path, now);
                }
            }
        }
    }

    /// Takes the oldest files out of the cache while it is over its limit. What the song
    /// uses now stays, and counts as used now.
    fn trim(&mut self) {
        self.trimmed = Some(Instant::now());
        let Some(dir) = self.store.cache_dir() else {
            return;
        };
        let uids: Vec<String> = self.uids.iter().map(|u| hex(u)).collect();
        let keys: HashSet<String> = self.plans.iter().map(|p| hex(&p.key)).collect();
        let used = |folder: &str, name: &str| match folder {
            CACHE_MADE => name.strip_suffix(".wav").is_some_and(|k| keys.contains(k)),
            _ => uids.iter().any(|u| name.starts_with(u.as_str())),
        };
        trim_cache(&dir, self.limit.load(Relaxed), &used, SystemTime::now());
    }
}

enum Rendered {
    Made,
    Cancelled,
    /// The store does not have the audio the plan is made from.
    NoSource,
}

/// Makes the audio of `plan` into the file at `path`, which gets its name only when it is
/// whole. `wrote` is called after every piece that went out; with `cancel` set the making
/// ends and nothing is left.
fn render_plan(
    store: &AudioStore,
    plan: &MadePlan,
    path: &Path,
    wrote: &mut dyn FnMut(),
    progress: &mut dyn FnMut(u8),
    cancel: &AtomicBool,
) -> Result<Rendered, Box<dyn std::error::Error>> {
    let Some(mut file) = store.open(&plan.uid, plan.sample_rate) else {
        return Ok(Rendered::NoSource);
    };
    let channels = usize::from(file.channels().clamp(1, 2));
    let mut stretcher = plan.stretcher(channels)?;
    let mut out = CacheWav::create(path, plan.sample_rate, channels as u8)?;
    let end = (file.frames().min(i64::MAX as u64) as i64).min(plan.file_end);
    let (mut raw, mut interleaved) = (Vec::new(), Vec::new());
    let read = |start: i64, into: &mut [&mut [f32]]| -> io::Result<()> {
        read_frames(&mut *file, start, end, into, &mut raw)
    };
    // Called from `write` and, while a tuned clip's pitch is looked for and nothing is
    // written yet, from `progress`; never from both at once.
    let wrote = RefCell::new(wrote);
    let mut write = |frames: &[&[f32]]| -> io::Result<()> {
        interleaved.clear();
        for i in 0..frames[0].len() {
            interleaved.extend(frames.iter().map(|c| c[i]));
        }
        out.write(&interleaved)?;
        (wrote.borrow_mut())();
        Ok(())
    };
    let p = |j: u64| plan.file_frame_at(j);
    let made = match plan.tuning() {
        None => render(
            &mut stretcher,
            plan.frames,
            p,
            read,
            &mut write,
            progress,
            cancel,
        ),
        Some(tune) => render_tuned(
            &mut stretcher,
            plan.frames,
            p,
            plan.sample_rate,
            &tune,
            read,
            &mut write,
            |percent| {
                (wrote.borrow_mut())();
                progress(percent);
            },
            cancel,
        ),
    };
    match made {
        Ok(()) => {
            out.finish()?;
            Ok(Rendered::Made)
        }
        Err(RenderError::Cancelled) => {
            out.abandon();
            Ok(Rendered::Cancelled)
        }
        Err(e) => {
            out.abandon();
            Err(e.to_string().into())
        }
    }
}

/// Makes the audio of `plan` on the calling thread, for an export whose song the maker's
/// thread is not asked for (22-11). It is written beside the cache's file under a name of
/// its own and given the cache's name when it is whole, so it never meets a file the
/// thread is writing; the two files would hold the same audio. Returns whether it is in
/// the cache now: not when `stop` was set, nor when the store does not have the audio the
/// plan is made from.
pub fn make_now(store: &AudioStore, plan: &MadePlan, stop: &AtomicBool) -> io::Result<bool> {
    let Some(path) = store.made_path(&plan.key) else {
        return Ok(false);
    };
    let own = path.with_extension(format!("export-{}.wav", std::process::id()));
    let made = render_plan(store, plan, &own, &mut || {}, &mut |_| {}, stop)
        .map_err(|e| io::Error::other(e.to_string()))?;
    match made {
        Rendered::Made => {
            fs::rename(&own, &path)?;
            Ok(true)
        }
        Rendered::Cancelled | Rendered::NoSource => Ok(false),
    }
}

/// Fills `into` with the frames of `file` from `start` on; silence before frame 0 and from
/// `end` on.
fn read_frames(
    file: &mut dyn PcmFile,
    start: i64,
    end: i64,
    into: &mut [&mut [f32]],
    raw: &mut Vec<f32>,
) -> io::Result<()> {
    let frames = into[0].len();
    for channel in into.iter_mut() {
        channel.fill(0.0);
    }
    let from = start.max(0);
    let to = (start + frames as i64).min(end);
    if to <= from {
        return Ok(());
    }
    let channels = usize::from(file.channels().clamp(1, 2));
    raw.resize((to - from) as usize * channels, 0.0);
    let got = file.read(from as u64, raw)?;
    let at = (from - start) as usize;
    for (i, frame) in raw.chunks_exact(channels).take(got).enumerate() {
        for (channel, sample) in into.iter_mut().zip(frame) {
            channel[at + i] = *sample;
        }
    }
    Ok(())
}

const CACHE_FOLDERS: [&str; 3] = [CACHE_AUDIO, CACHE_MADE, CACHE_PEAKS];

fn files_of(dir: &Path) -> impl Iterator<Item = (PathBuf, fs::Metadata)> {
    (fs::read_dir(dir).into_iter().flatten().flatten())
        .filter_map(|entry| Some((entry.path(), entry.metadata().ok()?)))
        .filter(|(_, meta)| meta.is_file())
}

fn is_part(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "part")
}

/// At the start: files that a meloet which ended in the middle was still writing. One
/// that was written to a moment ago may be another meloet's, and stays.
pub fn remove_left_parts(cache: &Path, now: SystemTime) {
    for folder in CACHE_FOLDERS {
        for (path, meta) in files_of(&cache.join(folder)) {
            let age = meta
                .modified()
                .ok()
                .and_then(|at| now.duration_since(at).ok());
            if is_part(&path) && age.is_some_and(|age| age >= PART_LEFT) {
                let _ = fs::remove_file(path);
            }
        }
    }
}

/// The time a file was last used is the time it has on the disk.
fn set_used(path: &Path, now: SystemTime) {
    if let Ok(file) = fs::File::options().append(true).open(path) {
        let _ = file.set_modified(now);
    }
}

/// While the files of the cache are more than `limit` bytes together, removes the one that
/// was used longest ago. `used(folder, file name)`: the song uses the file now; it stays,
/// and its time is set to `now`. Returns how many bytes went.
pub fn trim_cache(
    cache: &Path,
    limit: u64,
    used: &dyn Fn(&str, &str) -> bool,
    now: SystemTime,
) -> u64 {
    let mut total = 0u64;
    let mut loose: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    for folder in CACHE_FOLDERS {
        for (path, meta) in files_of(&cache.join(folder)) {
            total += meta.len();
            if is_part(&path) {
                continue;
            }
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            if name.is_some_and(|name| used(folder, &name)) {
                set_used(&path, now);
            } else {
                let at = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                loose.push((at, meta.len(), path));
            }
        }
    }
    loose.sort();
    let mut gone = 0;
    for (_, len, path) in loose {
        if total <= limit {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total -= len;
            gone += len;
        }
    }
    gone
}

#[cfg(test)]
mod tests;
