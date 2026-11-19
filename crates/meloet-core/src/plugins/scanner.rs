//! Runs the scans (docs/architecture.md 20-4): one thread takes the jobs in order and writes
//! the scan file; while a job runs, a few more threads each scan one bundle at a time with a
//! child process (`meloet-plughost scan`).
//!
//! A child that dies or goes quiet after a `Probing` stopped at that plugin: the plugin is
//! recorded as stopped and the bundle is scanned on by a new child that passes it over. A
//! child that goes quiet is killed; none is left behind.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_plugin::scan::{
    BundleRecord, BundleStamp, PluginInfo, ScanCache, StopReason, StoppedEntry, bundle_format,
    stopped_name,
};
use meloet_plugin::wire::{ScanOutput, WireError, read_frame};
use meloet_plugin::{PluginFormat, launch};

use super::{PluginOptions, ScanJob, ScanNews, ScanWhat};

/// How often a thread that waits for a child looks whether meloet is ending.
const STOP_TICK: Duration = Duration::from_millis(100);
/// Folders inside a place are looked into this deep for bundles (VST3 allows a folder for
/// each maker).
const PLACE_DEPTH: u32 = 3;

/// How many bundles are scanned at once: half of the processors, so that the other half stays
/// with the screen and whatever else the PC does, and four at most: the big bundles are read
/// from the disk and more children gain little. The children run at the lowest priority of
/// the ordinary processes (`launch::low_priority`).
fn workers() -> usize {
    let cpus = std::thread::available_parallelism().map_or(1, usize::from);
    (cpus / 2).clamp(1, 4)
}

type News = Arc<dyn Fn(ScanNews) + Send + Sync>;

enum Ask {
    Job(Box<ScanJob>),
    Save(Box<ScanCache>),
}

struct Context {
    exe: PathBuf,
    env: Vec<(OsString, OsString)>,
    timeout: Duration,
    cache_dir: Option<PathBuf>,
    stop: Arc<AtomicBool>,
    news: News,
}

pub struct Scanner {
    tx: Option<Sender<Ask>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Scanner {
    /// `news` is called on the scanner's threads.
    pub fn start(
        options: &PluginOptions,
        news: impl Fn(ScanNews) + Send + Sync + 'static,
    ) -> Scanner {
        let stop = Arc::new(AtomicBool::new(false));
        let context = Context {
            exe: options.host_exe.clone(),
            env: options.child_env.clone(),
            timeout: options.scan_timeout,
            cache_dir: options.cache_dir.clone(),
            stop: stop.clone(),
            news: Arc::new(news),
        };
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("meloet-scan".into())
            .spawn(move || run(&context, &rx))
            .expect("failed to start the scan thread");
        Scanner {
            tx: Some(tx),
            stop,
            thread: Some(thread),
        }
    }

    /// Runs after the jobs asked for before it. Its news end with `ScanNews::Done`.
    pub fn scan(&self, job: ScanJob) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Ask::Job(Box::new(job)));
        }
    }

    /// Writes the scan file, after the jobs asked for before it.
    pub fn save(&self, cache: ScanCache) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Ask::Save(Box::new(cache)));
        }
    }
}

impl Drop for Scanner {
    /// Gives up the scans (their children are killed), writes what was still to be written
    /// and waits for the threads.
    fn drop(&mut self) {
        self.stop.store(true, Relaxed);
        drop(self.tx.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(context: &Context, rx: &Receiver<Ask>) {
    while let Ok(ask) = rx.recv() {
        match ask {
            Ask::Job(job) => {
                if !context.stop.load(Relaxed) {
                    run_job(context, *job);
                }
            }
            Ask::Save(cache) => {
                if let Some(dir) = &context.cache_dir
                    && let Err(e) = cache.save(dir)
                {
                    eprintln!("meloet: the plugin scan was not saved: {e}");
                }
            }
        }
    }
}

/// One bundle to scan.
struct Todo {
    path: PathBuf,
    format: PluginFormat,
    stamp: BundleStamp,
    /// Passed over from the first child on.
    skip: Vec<String>,
    /// In the record whatever this scan finds.
    plugins: Vec<PluginInfo>,
    stopped: Vec<StoppedEntry>,
    /// The record before this scan, for the names of what stops.
    before: Option<BundleRecord>,
}

impl Todo {
    fn fresh(
        path: PathBuf,
        format: PluginFormat,
        stamp: BundleStamp,
        before: Option<BundleRecord>,
    ) -> Todo {
        Todo {
            path,
            format,
            stamp,
            skip: Vec::new(),
            plugins: Vec::new(),
            stopped: Vec::new(),
            before,
        }
    }
}

fn run_job(context: &Context, job: ScanJob) {
    let id = job.id;
    let todo = match job.what {
        ScanWhat::Places { places, all } => {
            let found = bundles_in(&places);
            let gone = job
                .known
                .bundles
                .iter()
                .filter(|b| places.iter().any(|p| b.path.starts_with(p)))
                .filter(|b| !found.iter().any(|(path, _)| *path == b.path))
                .map(|b| b.path.clone())
                .collect();
            let todo: Vec<Todo> = found
                .into_iter()
                .filter_map(|(path, format)| places_todo(path, format, &job.known, all))
                .collect();
            (context.news)(ScanNews::Counted {
                job: id,
                total: todo.len() as u32,
                gone,
            });
            todo
        }
        ScanWhat::Retry { path, id: plugin } => {
            retry_todo(path, plugin, &job.known).into_iter().collect()
        }
    };
    let (began, bundles) = (Instant::now(), todo.len());
    let queue = Mutex::new(VecDeque::from(todo));
    std::thread::scope(|scope| {
        for n in 0..workers() {
            let queue = &queue;
            // Each thread lives until its last child is gone: a child is killed when the
            // thread that started it ends (`launch::die_with_parent`).
            let spawned = std::thread::Builder::new()
                .name(format!("meloet-scan-{n}"))
                .spawn_scoped(scope, move || {
                    loop {
                        let next = queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front();
                        let Some(todo) = next else { break };
                        if let Some(record) = scan_bundle(context, todo) {
                            (context.news)(ScanNews::Bundle {
                                job: id,
                                record: Box::new(record),
                            });
                        }
                    }
                });
            if let Err(e) = spawned {
                eprintln!("meloet: no thread to scan with: {e}");
            }
        }
    });
    if !context.stop.load(Relaxed) {
        (context.news)(ScanNews::Done { job: id });
        if bundles > 0 {
            eprintln!(
                "meloet: scanned {bundles} plugin bundle(s) in {:.1} s",
                began.elapsed().as_secs_f64()
            );
        }
    }
}

/// Every bundle in the places, each once, in the order of their paths.
fn bundles_in(places: &[PathBuf]) -> Vec<(PathBuf, PluginFormat)> {
    fn look(dir: &Path, depth: u32, found: &mut Vec<(PathBuf, PluginFormat)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(format) = bundle_format(&path) {
                found.push((path, format));
            } else if depth > 0 && entry.file_type().is_ok_and(|t| t.is_dir()) {
                look(&path, depth - 1, found);
            }
        }
    }
    let mut found = Vec::new();
    for place in places {
        look(place, PLACE_DEPTH, &mut found);
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.dedup_by(|a, b| a.0 == b.0);
    found
}

/// What a scan of places does with one bundle; `None` leaves its record as it is.
fn places_todo(path: PathBuf, format: PluginFormat, known: &ScanCache, all: bool) -> Option<Todo> {
    let stamp = BundleStamp::read(&path).ok()?;
    let before = known.get(&path).cloned();
    match &before {
        Some(b) if b.stamp == stamp => {
            // Unchanged. What stopped in it is passed over (spec 3, Y13); with a bundle
            // that stopped as a whole, that is all of it.
            if !all || b.stopped.iter().any(|s| s.id.is_none()) {
                return None;
            }
            let stopped = b.stopped.clone();
            Some(Todo {
                skip: stopped.iter().filter_map(|s| s.id.clone()).collect(),
                stopped,
                ..Todo::fresh(path, format, stamp, before)
            })
        }
        _ => Some(Todo::fresh(path, format, stamp, before)),
    }
}

/// "もう一度ためす": only the one plugin when the bundle is as it was, else the whole bundle.
fn retry_todo(path: PathBuf, plugin: Option<String>, known: &ScanCache) -> Option<Todo> {
    let format = bundle_format(&path)?;
    let stamp = BundleStamp::read(&path).ok()?;
    let before = known.get(&path).cloned();
    match (&before, &plugin) {
        (Some(b), Some(id)) if b.stamp == stamp => {
            let stopped: Vec<StoppedEntry> = b
                .stopped
                .iter()
                .filter(|s| s.id.as_ref() != Some(id))
                .cloned()
                .collect();
            let skip = b
                .plugins
                .iter()
                .map(|p| p.key.id().to_owned())
                .chain(stopped.iter().filter_map(|s| s.id.clone()))
                .collect();
            Some(Todo {
                skip,
                plugins: b.plugins.clone(),
                stopped,
                ..Todo::fresh(path, format, stamp, before)
            })
        }
        _ => Some(Todo::fresh(path, format, stamp, before)),
    }
}

/// How one child ended.
enum End {
    Done,
    /// The bundle cannot be read at all.
    Broken,
    /// It died or went quiet; `at` is the plugin it was reading.
    Stopped {
        at: Option<(String, Option<String>)>,
        reason: StopReason,
    },
}

/// Scans the bundle with as many children as it takes. `None` when meloet is ending or no
/// child could be started: nothing is known about the bundle then.
fn scan_bundle(context: &Context, todo: Todo) -> Option<BundleRecord> {
    let Todo {
        path,
        format,
        stamp,
        mut skip,
        mut plugins,
        mut stopped,
        before,
    } = todo;
    let entry = |id: Option<&str>, name: Option<&str>, reason| StoppedEntry {
        id: id.map(str::to_owned),
        name: stopped_name(&path, id, name, before.as_ref()),
        reason,
    };
    loop {
        let mut found = Vec::new();
        let mut broken = Vec::new();
        let end = run_child(context, format, &path, &skip, &mut found, &mut broken)?;
        for plugin in found {
            skip.push(plugin.key.id().to_owned());
            if plugins.iter().all(|p| p.key != plugin.key) {
                plugins.push(plugin);
            }
        }
        for (id, name) in broken {
            stopped.push(entry(Some(&id), name.as_deref(), StopReason::Broken));
            skip.push(id);
        }
        match end {
            End::Done => break,
            End::Broken => {
                stopped.push(entry(None, None, StopReason::Broken));
                break;
            }
            // A child that reads again what it was told to pass over would go round forever.
            End::Stopped {
                at: Some((id, name)),
                reason,
            } if !skip.contains(&id) => {
                stopped.push(entry(Some(&id), name.as_deref(), reason));
                skip.push(id);
            }
            End::Stopped { reason, .. } => {
                stopped.push(entry(None, None, reason));
                break;
            }
        }
    }
    Some(BundleRecord {
        path,
        format,
        stamp,
        plugins,
        stopped,
    })
}

/// One child over the bundle. `found` and `broken` (id and name) get what it read.
fn run_child(
    context: &Context,
    format: PluginFormat,
    bundle: &Path,
    skip: &[String],
    found: &mut Vec<PluginInfo>,
    broken: &mut Vec<(String, Option<String>)>,
) -> Option<End> {
    let (mut ours, theirs) = UnixStream::pair().ok()?;
    let mut command =
        launch::scan_command(&context.exe, format, bundle, skip, theirs.as_fd(), None).ok()?;
    // Plugins built on Qt make a Qt application when they are read. Without this they pick
    // a platform that needs a display (X), and abort where there is none. A scan shows no
    // window. The processes plugins sound in are not given this: they do show windows.
    command.env("QT_QPA_PLATFORM", "offscreen");
    command.envs(context.env.iter().map(|(name, value)| (name, value)));
    let mut child = command
        .spawn()
        .inspect_err(|e| eprintln!("meloet: cannot start {}: {e}", context.exe.display()))
        .ok()?;
    // The command holds a copy of the socket: with it open here, the child's end would
    // never read as closed.
    drop((command, theirs));
    // The time limit runs from the last `Probing`; before the first one, from the start.
    let mut last = Instant::now();
    let mut at: Option<(String, Option<String>)> = None;
    let end = loop {
        if context.stop.load(Relaxed) {
            break None;
        }
        let left = (last + context.timeout).saturating_duration_since(Instant::now());
        if left.is_zero() {
            break Some(End::Stopped {
                at,
                reason: StopReason::TimedOut,
            });
        }
        if !launch::wait_readable(ours.as_fd(), left.min(STOP_TICK)) {
            continue;
        }
        let _ = ours.set_read_timeout(Some(left));
        match read_frame::<ScanOutput>(&mut ours) {
            Ok((ScanOutput::Probing { id, name }, _)) => {
                last = Instant::now();
                at = Some((id, name));
            }
            Ok((ScanOutput::Found { plugin }, _)) => {
                at = None;
                found.push(plugin);
            }
            Ok((ScanOutput::PluginBroken { id, .. }, _)) => {
                let name = at
                    .take()
                    .filter(|(probed, _)| *probed == id)
                    .and_then(|a| a.1);
                broken.push((id, name));
            }
            Ok((ScanOutput::Broken { .. }, _)) => break Some(End::Broken),
            Ok((ScanOutput::Done, _)) => break Some(End::Done),
            Err(WireError::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                break Some(End::Stopped {
                    at,
                    reason: StopReason::TimedOut,
                });
            }
            Err(_) => {
                break Some(End::Stopped {
                    at,
                    reason: StopReason::Crashed,
                });
            }
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::temp_dir;

    fn bundle(root: &Path, path: &str) -> PathBuf {
        let bundle = root.join(path);
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
        bundle
    }

    fn info(id: &str) -> PluginInfo {
        PluginInfo {
            key: meloet_plugin::PluginKey::lv2(id).unwrap(),
            name: id.into(),
            vendor: String::new(),
            version: String::new(),
            kind: meloet_plugin::PluginKind::Effect(meloet_plugin::EffectCategory::Other),
            editor: meloet_plugin::EditorKind::None,
            audio_inputs: 2,
            audio_outputs: 2,
        }
    }

    fn stopped(id: Option<&str>) -> StoppedEntry {
        StoppedEntry {
            id: id.map(str::to_owned),
            name: "n".into(),
            reason: StopReason::Crashed,
        }
    }

    fn known(path: &Path, plugins: &[&str], stops: &[Option<&str>]) -> ScanCache {
        ScanCache {
            bundles: vec![BundleRecord {
                path: path.into(),
                format: PluginFormat::Lv2,
                stamp: BundleStamp::read(path).unwrap(),
                plugins: plugins.iter().map(|id| info(id)).collect(),
                stopped: stops.iter().map(|id| stopped(*id)).collect(),
            }],
        }
    }

    #[test]
    fn bundles_are_found_in_a_place_and_in_its_folders() {
        let root = temp_dir("places");
        let a = bundle(&root, "lv2/a.lv2");
        let b = bundle(&root, "vst3/maker/b.vst3");
        bundle(&root, "vst3/c.clap");
        bundle(&root, "vst3/1/2/3/4/too-deep.vst3");
        std::fs::write(root.join("lv2/readme.txt"), "x").unwrap();
        let places = [root.join("lv2"), root.join("vst3"), root.join("missing")];
        let found = bundles_in(&places);
        assert_eq!(
            found,
            [(a.clone(), PluginFormat::Lv2), (b, PluginFormat::Vst3)]
        );
        // A place inside another gives each bundle once.
        let found = bundles_in(&[root.join("lv2"), root.join("lv2")]);
        assert_eq!(found, [(a, PluginFormat::Lv2)]);
    }

    #[test]
    fn a_scan_passes_over_what_has_not_changed_and_what_stopped() {
        let root = temp_dir("todo");
        let path = bundle(&root, "a.lv2");
        let lv2 = PluginFormat::Lv2;
        let todo = |known: &ScanCache, all| places_todo(path.clone(), lv2, known, all);

        // Not known: all of it.
        let fresh = todo(&ScanCache::default(), false).unwrap();
        assert!(fresh.skip.is_empty() && fresh.stopped.is_empty() && fresh.before.is_none());

        // Unchanged: left alone, and with "もう一度スキャン" read again but for what stopped.
        let cache = known(&path, &["urn:a"], &[Some("urn:b")]);
        assert!(todo(&cache, false).is_none());
        let again = todo(&cache, true).unwrap();
        assert_eq!(again.skip, ["urn:b"]);
        assert_eq!(again.stopped, [stopped(Some("urn:b"))]);
        assert!(again.plugins.is_empty());

        // A bundle that stopped as a whole is not read at all.
        let cache = known(&path, &[], &[None]);
        assert!(todo(&cache, false).is_none());
        assert!(todo(&cache, true).is_none());

        // Changed: all of it, whatever stopped before; the old record gives the names.
        std::fs::write(path.join("plugin.so"), [0u8; 17]).unwrap();
        let changed = todo(&cache, false).unwrap();
        assert!(changed.skip.is_empty() && changed.stopped.is_empty());
        assert!(changed.before.is_some());
    }

    #[test]
    fn trying_again_reads_the_one_plugin_or_the_whole_bundle() {
        let root = temp_dir("retry");
        let path = bundle(&root, "a.lv2");
        let cache = known(&path, &["urn:a"], &[Some("urn:b"), Some("urn:c")]);
        let one = retry_todo(path.clone(), Some("urn:b".into()), &cache).unwrap();
        assert_eq!(one.skip, ["urn:a", "urn:c"]);
        assert_eq!(one.plugins.len(), 1);
        assert_eq!(one.stopped, [stopped(Some("urn:c"))]);

        let whole = retry_todo(path.clone(), None, &cache).unwrap();
        assert!(whole.skip.is_empty() && whole.plugins.is_empty() && whole.stopped.is_empty());

        // The bundle changed since: the whole of it.
        std::fs::write(path.join("plugin.so"), [0u8; 17]).unwrap();
        let changed = retry_todo(path.clone(), Some("urn:b".into()), &cache).unwrap();
        assert!(changed.skip.is_empty() && changed.plugins.is_empty());

        assert!(retry_todo(root.join("gone.lv2"), None, &cache).is_none());
        assert!(retry_todo(root.join("a.clap"), None, &cache).is_none());
    }
}
