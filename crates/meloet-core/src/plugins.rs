//! Plugins: where they are looked for, what the scan found and what stopped it
//! (docs/architecture.md 20-4).
//!
//! Here are the rules, as pure as the rest of [`crate::state`]: [`Plugins`] takes intents and
//! the scanner's news and answers with scans to run. [`scanner`] runs them, one child process
//! for each bundle. [`hosts`] keeps the processes plugins sound in.

pub mod export;
pub mod hosts;
pub(crate) mod housed;
pub mod scanner;

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use meloet_plugin::scan::{self, BundleRecord, ScanCache, StopReason};
use meloet_protocol::{
    PluginFormat, PluginPlaceView, PluginsView, ScanStopReason, ScanView, StatusMessage,
    StoppedPluginView,
};

use crate::state::Effect;

const HOST_EXE: &str = "meloet-plughost";
/// While a scan runs, what it found so far is written this often.
const SAVE_EVERY_US: u64 = 2_000_000;

/// What the core needs to scan for plugins and to run them.
#[derive(Debug, Clone)]
pub struct PluginOptions {
    /// The `meloet-plughost` binary.
    pub host_exe: PathBuf,
    /// Where the scan file lives. `None`: nothing is read or written.
    pub cache_dir: Option<PathBuf>,
    /// The places that are always looked in and cannot be removed.
    pub usual_places: Vec<PathBuf>,
    /// How long after its last `Probing` a scanning child is given up on.
    pub scan_timeout: Duration,
    /// Set for every child, on top of meloet's own environment.
    pub child_env: Vec<(OsString, OsString)>,
    /// Scanning waits until the screen first subscribes (its page is up), at most
    /// `SCAN_WAITS_FOR_SCREEN` in `service.rs`, so the window comes up before hundreds of
    /// children start and end. Under LLDB (VS Code), children started while WebKit loaded its
    /// libraries made LLDB show its own stop in the dynamic loader as a pause.
    pub wait_for_screen: bool,
}

impl PluginOptions {
    /// The host binary beside meloet's own, the usual places of Linux and the cache folder.
    /// `None` when the binary is not there: meloet then runs without plugins.
    pub fn from_env() -> Option<PluginOptions> {
        let var = |name: &str| std::env::var_os(name);
        let host_exe = std::env::current_exe().ok()?.with_file_name(HOST_EXE);
        if !host_exe.is_file() {
            eprintln!("meloet: {} is not there; no plugins", host_exe.display());
            return None;
        }
        Some(PluginOptions {
            host_exe,
            cache_dir: scan::cache_dir(var),
            usual_places: scan::default_places(var),
            scan_timeout: Duration::from_secs(scan::SCAN_TIMEOUT_S),
            child_env: Vec::new(),
            wait_for_screen: true,
        })
    }
}

/// One scan for the scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanJob {
    pub id: u64,
    pub what: ScanWhat,
    /// What was known when the scan was asked for.
    pub known: ScanCache,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanWhat {
    /// Every bundle in these places whose files changed since `known`, and with `all` the
    /// unchanged ones too. A plugin, or a bundle, that stopped a scan and has not changed is
    /// passed over either way.
    Places { places: Vec<PathBuf>, all: bool },
    /// One stopped plugin of a bundle again, or with no `id` the whole bundle.
    Retry { path: PathBuf, id: Option<String> },
}

/// What the scanner tells about the job it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanNews {
    /// The places were looked through: `total` bundles are scanned now, and the bundles of
    /// `gone` are in these places no more.
    Counted {
        job: u64,
        total: u32,
        gone: Vec<PathBuf>,
    },
    /// One bundle was scanned; this is its record from now on.
    Bundle {
        job: u64,
        record: Box<BundleRecord>,
    },
    Done {
        job: u64,
    },
}

enum Kind {
    /// The scan at the first start: shows its count, and says nothing when there was nothing.
    First,
    /// "もう一度スキャン": shows its count.
    Again,
    /// Says only that new plugins were found.
    Quiet,
    /// "もう一度ためす" on the row with this name.
    Retry { name: String },
}

struct Job {
    id: u64,
    what: ScanWhat,
    kind: Kind,
    /// Bundles done and bundles in all, once the scanner has counted.
    count: Option<(u32, u32)>,
    /// Plugins that were not in their bundle's record before.
    new: u32,
}

impl Job {
    fn counted(&self) -> bool {
        matches!(self.kind, Kind::First | Kind::Again)
    }
}

/// The places, the scan's record and the scans asked for.
pub(crate) struct Plugins {
    home: Option<PathBuf>,
    usual: Vec<PathBuf>,
    added: Vec<PathBuf>,
    cache: ScanCache,
    /// There is a scanner: scans may be asked for.
    scanning: bool,
    /// Oldest first; the first one is with the scanner.
    jobs: VecDeque<Job>,
    next_job: u64,
    /// The numbers of the last scan that showed its count.
    finished: Option<(u32, u32)>,
    stopped_open: bool,
    places_open: bool,
    saved_us: u64,
    /// Goes up when the shelf is to take in what the record holds now: at the start, when
    /// a scan ends, and while one runs as often as its record is written.
    rows_rev: u64,
}

fn format_view(format: meloet_plugin::PluginFormat) -> PluginFormat {
    match format {
        meloet_plugin::PluginFormat::Vst3 => PluginFormat::Vst3,
        meloet_plugin::PluginFormat::Lv2 => PluginFormat::Lv2,
    }
}

fn reason_view(reason: StopReason) -> ScanStopReason {
    match reason {
        StopReason::Crashed => ScanStopReason::Crashed,
        StopReason::TimedOut => ScanStopReason::TimedOut,
        StopReason::Broken => ScanStopReason::Broken,
    }
}

impl Plugins {
    /// The usual places of Linux, as `var` gives them, and the places the person added.
    /// Nothing is scanned before [`Plugins::start`].
    pub fn new(var: impl Fn(&str) -> Option<OsString>, added: Vec<PathBuf>) -> Plugins {
        let home = var("HOME").filter(|v| !v.is_empty()).map(PathBuf::from);
        let usual = scan::default_places(&var);
        let mut plugins = Plugins {
            home,
            usual,
            added: Vec::new(),
            cache: ScanCache::default(),
            scanning: false,
            jobs: VecDeque::new(),
            next_job: 1,
            finished: None,
            stopped_open: false,
            // Mock R9-Scan shows it unfolded (docs/history/design-notes.ja.md A-16).
            // Spec 3 (94-89): the list starts closed; it is seldom touched.
            places_open: false,
            saved_us: 0,
            rows_rev: 0,
        };
        for place in added {
            if !plugins.has_place(&place) {
                plugins.added.push(place);
            }
        }
        plugins
    }

    /// The places that are always looked in, in place of the ones the environment gave.
    pub fn set_usual(&mut self, usual: Vec<PathBuf>) {
        self.added.retain(|p| !usual.contains(p));
        self.usual = usual;
    }

    /// The scan at start: with no record yet everything, showing the count; else only what
    /// changed, quietly.
    pub fn start(&mut self, cache: ScanCache, first: bool, effects: &mut Vec<Effect>) {
        self.cache = cache;
        self.scanning = true;
        self.rows_rev += 1;
        let kind = if first { Kind::First } else { Kind::Quiet };
        let places = self.places().cloned().collect();
        self.push(ScanWhat::Places { places, all: false }, kind, effects);
    }

    /// The home folder, which the screen shows as "~".
    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    pub fn added_places(&self) -> &[PathBuf] {
        &self.added
    }

    pub fn cache(&self) -> &ScanCache {
        &self.cache
    }

    pub fn rows_rev(&self) -> u64 {
        self.rows_rev
    }

    fn places(&self) -> impl Iterator<Item = &PathBuf> {
        self.usual.iter().chain(&self.added)
    }

    fn has_place(&self, place: &Path) -> bool {
        self.places().any(|p| p == place)
    }

    fn place_of(&self, bundle: &Path) -> Option<usize> {
        self.places().position(|p| bundle.starts_with(p))
    }

    fn push(&mut self, what: ScanWhat, kind: Kind, effects: &mut Vec<Effect>) {
        if !self.scanning {
            return;
        }
        let id = self.next_job;
        self.next_job += 1;
        self.jobs.push_back(Job {
            id,
            what,
            kind,
            count: None,
            new: 0,
        });
        if self.jobs.len() == 1 {
            self.send_front(effects);
        }
    }

    fn send_front(&self, effects: &mut Vec<Effect>) {
        if let Some(job) = self.jobs.front() {
            effects.push(Effect::ScanPlugins(Box::new(ScanJob {
                id: job.id,
                what: job.what.clone(),
                known: self.cache.clone(),
            })));
        }
    }

    fn save(&mut self, now_us: u64, effects: &mut Vec<Effect>) {
        self.saved_us = now_us;
        self.rows_rev += 1;
        effects.push(Effect::SavePluginScan(Box::new(self.cache.clone())));
    }

    /// `RescanPlugins`. Does nothing while a scan that shows its count runs or waits.
    pub fn rescan(&mut self, effects: &mut Vec<Effect>) {
        if self.jobs.iter().any(Job::counted) {
            return;
        }
        let places = self.places().cloned().collect();
        self.push(ScanWhat::Places { places, all: true }, Kind::Again, effects);
    }

    /// `RetryStoppedPlugin`. Does nothing for a row that is not there or is tried already.
    pub fn retry(&mut self, path: &str, id: Option<String>, effects: &mut Vec<Effect>) {
        let path = PathBuf::from(path);
        if self.retrying(&path, id.as_deref()) {
            return;
        }
        let name = self.cache.get(&path).and_then(|b| {
            b.stopped
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.name.clone())
        });
        if let Some(name) = name {
            self.push(ScanWhat::Retry { path, id }, Kind::Retry { name }, effects);
        }
    }

    fn retrying(&self, path: &Path, id: Option<&str>) -> bool {
        self.jobs.iter().any(|j| {
            matches!(&j.what, ScanWhat::Retry { path: p, id: i }
                if p == path && i.as_deref() == id)
        })
    }

    /// The answer to the folder window of "場所を足す…". Returns whether the place was added.
    pub fn place_picked(&mut self, place: PathBuf, effects: &mut Vec<Effect>) -> bool {
        if !place.is_absolute() || self.has_place(&place) {
            return false;
        }
        self.added.push(place.clone());
        let what = ScanWhat::Places {
            places: vec![place],
            all: false,
        };
        self.push(what, Kind::Quiet, effects);
        true
    }

    /// `RemovePluginPlace`. Returns whether a place the person added was removed.
    pub fn remove_place(&mut self, path: &str, now_us: u64, effects: &mut Vec<Effect>) -> bool {
        let place = Path::new(path);
        let Some(at) = self.added.iter().position(|p| p == place) else {
            return false;
        };
        self.added.remove(at);
        let before = self.cache.bundles.len();
        let kept = std::mem::take(&mut self.cache.bundles)
            .into_iter()
            .filter(|b| self.place_of(&b.path).is_some())
            .collect();
        self.cache.bundles = kept;
        if self.cache.bundles.len() != before {
            self.rows_rev += 1;
            if self.scanning {
                self.save(now_us, effects);
            }
        }
        true
    }

    pub fn open_stopped(&mut self) {
        self.stopped_open = true;
    }

    /// Returns whether the dialog was up.
    pub fn close_stopped(&mut self) -> bool {
        std::mem::take(&mut self.stopped_open)
    }

    pub fn stopped_open(&self) -> bool {
        self.stopped_open
    }

    pub fn unfold_places(&mut self) {
        self.places_open = true;
    }

    pub fn toggle_places(&mut self) {
        self.places_open = !self.places_open;
    }

    /// Takes in what the scanner says. Returns the notice for the status line.
    pub fn news(
        &mut self,
        news: ScanNews,
        now_us: u64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let (ScanNews::Counted { job, .. } | ScanNews::Bundle { job, .. } | ScanNews::Done { job }) =
            &news;
        if self.jobs.front().is_none_or(|j| j.id != *job) {
            return None;
        }
        match news {
            ScanNews::Counted { total, gone, .. } => {
                for path in gone {
                    self.cache.remove(&path);
                }
                let job = self.jobs.front_mut()?;
                job.count = Some((0, total));
                if job.counted() {
                    self.finished = None;
                }
                None
            }
            ScanNews::Bundle { record, .. } => {
                // A place that was removed while its scan ran.
                self.place_of(&record.path)?;
                let before = self.cache.get(&record.path);
                let new = record
                    .plugins
                    .iter()
                    .filter(|p| before.is_none_or(|b| b.plugins.iter().all(|q| q.key != p.key)))
                    .count();
                self.cache.put(*record);
                let job = self.jobs.front_mut()?;
                job.new += new as u32;
                if let Some((done, _)) = &mut job.count {
                    *done += 1;
                }
                if now_us.saturating_sub(self.saved_us) >= SAVE_EVERY_US {
                    self.save(now_us, effects);
                }
                None
            }
            ScanNews::Done { .. } => {
                let job = self.jobs.pop_front()?;
                self.save(now_us, effects);
                self.send_front(effects);
                self.over(job)
            }
        }
    }

    fn over(&mut self, job: Job) -> Option<StatusMessage> {
        let added = self.cache.plugins().count() as u32;
        let stopped = self.cache.stopped().count() as u32;
        match (job.kind, job.what) {
            // A PC without plugins is not told so at every start.
            (Kind::First, _) if job.count.is_none_or(|(_, total)| total == 0) => None,
            (Kind::First | Kind::Again, _) => {
                self.finished = Some((added, stopped));
                Some(StatusMessage::ScanFinished { added, stopped })
            }
            (Kind::Quiet, _) => {
                (job.new > 0).then_some(StatusMessage::PluginsFound { count: job.new })
            }
            (Kind::Retry { name }, ScanWhat::Retry { path, id }) => {
                let record = self.cache.get(&path);
                let still = record.is_none_or(|b| match &id {
                    Some(_) => b.stopped.iter().any(|s| s.id == id),
                    // The whole bundle: it reads now when something of it is on the shelf,
                    // or nothing of it stopped.
                    None => b.plugins.is_empty() && !b.stopped.is_empty(),
                });
                Some(if still {
                    StatusMessage::ScanRetryStopped { name }
                } else {
                    StatusMessage::ScanRetryAdded { name }
                })
            }
            (Kind::Retry { .. }, ScanWhat::Places { .. }) => None,
        }
    }

    pub fn view(&self) -> PluginsView {
        let home = self.home.as_deref();
        let scan = match self.jobs.front() {
            Some(Job {
                count: Some((done, total)),
                kind: Kind::First | Kind::Again,
                ..
            }) => ScanView::Running {
                done: *done,
                total: *total,
            },
            _ => match self.finished {
                Some((added, stopped)) => ScanView::Finished { added, stopped },
                None => ScanView::Idle,
            },
        };
        let mut stopped: Vec<(usize, StoppedPluginView)> = self
            .cache
            .stopped()
            .map(|(bundle, entry)| {
                let view = StoppedPluginView {
                    name: entry.name.clone(),
                    format: format_view(bundle.format),
                    reason: reason_view(entry.reason),
                    path: bundle.path.display().to_string(),
                    id: entry.id.clone(),
                    shown_path: shown_path(&bundle.path, home),
                    retrying: self.retrying(&bundle.path, entry.id.as_deref()),
                };
                (self.place_of(&bundle.path).unwrap_or(usize::MAX), view)
            })
            .collect();
        stopped.sort_by_key(|(place, _)| *place);
        let place = |added: bool| {
            move |place: &PathBuf| PluginPlaceView {
                path: place.display().to_string(),
                shown_path: shown_path(place, home),
                added,
            }
        };
        PluginsView {
            scan,
            stopped: stopped.into_iter().map(|(_, view)| view).collect(),
            places: self
                .usual
                .iter()
                .map(place(false))
                .chain(self.added.iter().map(place(true)))
                .collect(),
            stopped_open: self.stopped_open,
            places_open: self.places_open,
        }
    }
}

/// `path` as the screen shows it: the home folder written as "~".
pub(crate) fn shown_path(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_folder_is_shown_as_a_tilde() {
        let home = Path::new("/home/me");
        assert_eq!(shown_path(Path::new("/home/me/.lv2"), Some(home)), "~/.lv2");
        assert_eq!(shown_path(Path::new("/home/me"), Some(home)), "~");
        assert_eq!(
            shown_path(Path::new("/home/meme/.lv2"), Some(home)),
            "/home/meme/.lv2"
        );
        assert_eq!(
            shown_path(Path::new("/usr/lib/lv2"), Some(home)),
            "/usr/lib/lv2"
        );
        assert_eq!(
            shown_path(Path::new("/home/me/.lv2"), None),
            "/home/me/.lv2"
        );
    }

    #[test]
    fn the_usual_places_are_listed_and_cannot_be_removed() {
        let vars = |name: &str| (name == "HOME").then(|| OsString::from("/home/me"));
        let view = Plugins::new(vars, Vec::new()).view();
        let shown: Vec<&str> = view.places.iter().map(|p| p.shown_path.as_str()).collect();
        assert_eq!(
            shown,
            [
                "~/.lv2",
                "/usr/lib/lv2",
                "/usr/local/lib/lv2",
                "~/.vst3",
                "/usr/lib/vst3",
                "/usr/local/lib/vst3"
            ]
        );
        assert_eq!(view.places[0].path, "/home/me/.lv2");
        assert!(view.places.iter().all(|p| !p.added));
        assert_eq!(view.scan, ScanView::Idle);
        assert!(view.stopped.is_empty() && !view.stopped_open && !view.places_open);
    }
}
