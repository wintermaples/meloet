//! The one thread that fetches the sound pack (docs/architecture.md 23-2, 23-3 and 23-9): the
//! only place meloet goes to the net. One file at a time, into `<file>.part`, going on with
//! `Range` where a file was cut; every file is checked by its size and sha256 before it is
//! unpacked beside its place and renamed into it.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use meloet_model::BuiltinInstrument;
use meloet_protocol::PackFailure;
use sha2::{Digest, Sha256};
use ureq::tls::{RootCerts, TlsConfig};

use super::catalog::{self, COMMON_ID, Catalog, Part, Trust};

/// Room kept free beside what the files take, so that the PC is not left with a full disk.
pub const ROOM_BYTES: u64 = 64 * 1024 * 1024;
/// The online catalog is looked at no more often than this.
pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// Progress is told no more often than this (the screen gets at most 4 a second).
const TELL_EVERY: Duration = Duration::from_millis(250);
/// `remaining_s` is told once the fetch has run this long.
const GUESS_AFTER: Duration = Duration::from_secs(3);
/// A cut connection is taken up again this many times before the fetch says so.
const CUT_RETRIES: u32 = 2;
/// 403 and 410 (the signed url of the release ran out) are followed again from the catalog's
/// url this many times.
const GONE_RETRIES: u32 = 2;
const CATALOG_LIMIT: u64 = 1024 * 1024;
const CHECKED_FILE: &str = "sounds-checked";
const CATALOG_FILE: &str = "catalog.json";
const WORK_PREFIX: &str = ".work-";

/// The bytes of the pack as they are on the disk, for `PackFetchView`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Numbers {
    pub done: u64,
    pub total: u64,
}

/// The pack on this PC against a catalog.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outlook {
    pub numbers: Numbers,
    /// Something of the catalog is not here at all (asked for at the start: 43-B).
    pub fresh: bool,
    /// What the parts that are here in an older version take (a new pack, 「12 MB」).
    pub update_bytes: u64,
    /// Something is to be fetched.
    pub missing: bool,
    /// Some part was fetched before (the check at the start is only for those: 23-3).
    pub fetched_any: bool,
}

/// Where the fetched parts are: `<root>/<id>/<version>/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    pub root: PathBuf,
    /// The ids another place has (the pack of a development build): not fetched.
    pub covered: Vec<String>,
}

impl Disk {
    pub fn dir(&self, part: &Part) -> PathBuf {
        self.root.join(&part.id).join(&part.version)
    }

    fn marker(id: &str) -> &'static str {
        if id == COMMON_ID {
            "credits.json"
        } else {
            "instrument.json"
        }
    }

    pub fn part_file(&self, part: &Part) -> PathBuf {
        self.root.join(format!("{}.part", part.file))
    }

    fn here(&self, part: &Part) -> bool {
        self.dir(part).join(Disk::marker(&part.id)).is_file()
    }

    pub fn has(&self, part: &Part) -> bool {
        self.covered.contains(&part.id) || self.here(part)
    }

    /// A version of `id` that is here, the catalog's first.
    fn version_dir(&self, id: &str, preferred: Option<&str>) -> Option<PathBuf> {
        let base = self.root.join(id);
        let marker = Disk::marker(id);
        if let Some(v) = preferred
            && base.join(v).join(marker).is_file()
        {
            return Some(base.join(v));
        }
        let mut others: Vec<PathBuf> = fs::read_dir(&base)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| e.path())
            .filter(|p| p.join(marker).is_file())
            .collect();
        others.sort();
        others.pop()
    }

    /// The `instrument.json` of every instrument fetched before, the catalog's version where it
    /// is here, else another one (until the new one is fetched).
    pub fn instruments(&self, catalog: &Catalog) -> Vec<(BuiltinInstrument, PathBuf)> {
        BuiltinInstrument::all()
            .filter(|b| crate::instruments::sampled(*b))
            .filter(|b| !self.covered.iter().any(|c| c == b.name()))
            .filter_map(|b| {
                let version = catalog.part_of(b).map(|p| p.version.as_str());
                Some((
                    b,
                    self.version_dir(b.name(), version)?.join("instrument.json"),
                ))
            })
            .collect()
    }

    fn part_len(&self, part: &Part) -> u64 {
        fs::metadata(self.part_file(part)).map_or(0, |m| m.len().min(part.bytes))
    }

    pub fn outlook(&self, catalog: &Catalog) -> Outlook {
        let missing: Vec<&Part> = catalog.parts.iter().filter(|p| !self.has(p)).collect();
        let update = |p: &Part| self.version_dir(&p.id, None).is_some();
        let update_bytes = missing.iter().filter(|p| update(p)).map(|p| p.bytes).sum();
        let fresh = missing.iter().any(|p| !update(p));
        let left: u64 = missing.iter().map(|p| p.bytes - self.part_len(p)).sum();
        let total: u64 = if fresh {
            (catalog.parts.iter())
                .filter(|p| !self.covered.contains(&p.id))
                .map(|p| p.bytes)
                .sum()
        } else {
            missing.iter().map(|p| p.bytes).sum()
        };
        Outlook {
            numbers: Numbers {
                done: total.saturating_sub(left),
                total,
            },
            fresh,
            update_bytes,
            missing: !missing.is_empty(),
            fetched_any: (catalog.parts.iter()).any(|p| self.version_dir(&p.id, None).is_some()),
        }
    }

    /// What a fetch takes, in its order: the common part, then docs/history/design-notes.ja.md B-95-1 (the urgent
    /// instruments of Y2 the pack has, then the order of the shelf).
    pub fn to_fetch<'a>(&self, catalog: &'a Catalog) -> Vec<&'a Part> {
        use BuiltinInstrument as B;
        const URGENT: [B; 6] = [
            B::GrandPiano,
            B::StudioKit,
            B::FingerBass,
            B::Strings,
            B::ElectricPiano,
            B::Organ,
        ];
        let rank = |p: &Part| match p.instrument() {
            None => 0,
            Some(b) => match URGENT.iter().position(|u| *u == b) {
                Some(i) => 1 + i,
                None => 1 + URGENT.len() + B::all().position(|a| a == b).unwrap_or(0),
            },
        };
        let mut parts: Vec<&Part> = catalog.parts.iter().filter(|p| !self.has(p)).collect();
        parts.sort_by_key(|p| rank(p));
        parts
    }

    /// Takes away what an earlier run left: half unpacked folders, `.part` files of no part of
    /// the catalog, and versions the catalog has a newer one of that is here.
    fn tidy(&self, catalog: &Catalog) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        let parts: Vec<String> = catalog
            .parts
            .iter()
            .map(|p| format!("{}.part", p.file))
            .collect();
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(WORK_PREFIX) {
                let _ = fs::remove_dir_all(entry.path());
            } else if name.ends_with(".part") && !parts.contains(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
        for part in catalog.parts.iter().filter(|p| self.here(p)) {
            let Ok(versions) = fs::read_dir(self.root.join(&part.id)) else {
                continue;
            };
            for v in versions.filter_map(|e| e.ok()) {
                if v.file_name() != part.version.as_str() {
                    let _ = fs::remove_dir_all(v.path());
                }
            }
        }
    }
}

/// `$XDG_DATA_HOME/meloet/sounds` when that is an absolute path, else
/// `~/.local/share/meloet/sounds` (the base directory specification ignores a relative one).
pub fn data_root(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let set = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let data = set("XDG_DATA_HOME").filter(|p| p.is_absolute());
    let data = data.or_else(|| {
        Some(
            set("HOME")
                .filter(|h| h.is_absolute())?
                .join(".local/share"),
        )
    });
    Some(data?.join("meloet").join("sounds"))
}

/// The place shown to the person: the home folder as `~`.
pub fn shown(root: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| root.strip_prefix(h).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => root.display().to_string(),
    }
}

/// What the fetching thread is started with.
#[derive(Debug, Clone)]
pub struct Options {
    pub disk: Disk,
    /// Where the time of the last look at the online catalog is kept. `None`: never looked.
    pub state_dir: Option<PathBuf>,
    pub trust: Trust,
    /// The catalog on the net.
    pub online: String,
    /// Off only for the local server of the tests.
    pub https_only: bool,
    pub connect_timeout: Duration,
    /// A connection that brings nothing for this long counts as cut.
    pub read_timeout: Duration,
    /// The wait before a cut connection is taken up again.
    pub retry_pause: Duration,
    #[cfg(test)]
    pub hooks: Hooks,
}

impl Options {
    pub fn new(disk: Disk, state_dir: Option<PathBuf>) -> Options {
        Options {
            disk,
            state_dir,
            trust: Trust::default(),
            online: catalog::ONLINE.to_owned(),
            https_only: true,
            connect_timeout: Duration::from_secs(15),
            read_timeout: Duration::from_secs(30),
            retry_pause: Duration::from_secs(1),
            #[cfg(test)]
            hooks: Hooks::default(),
        }
    }
}

/// What the tests put in place of the disk.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct Hooks {
    /// The free bytes `statvfs` would say.
    pub free: Option<u64>,
    /// Writing more than this many bytes in all fails as a full disk does.
    pub write_limit: Option<u64>,
    /// The time now, Unix ms.
    pub now_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackNews {
    /// `instrument`: the part coming now (`None`: the common one).
    Progress {
        round: u64,
        instrument: Option<BuiltinInstrument>,
        numbers: Numbers,
        remaining_s: Option<u32>,
    },
    /// A part is in its place. `json`: the instrument's `instrument.json`.
    Installed {
        round: u64,
        instrument: Option<BuiltinInstrument>,
        json: PathBuf,
    },
    /// Every part of the catalog is here.
    Done { round: u64 },
    Failed {
        round: u64,
        failure: PackFailure,
        numbers: Numbers,
    },
    /// The look at the start found parts newer than the ones here: `bytes` is what they take.
    /// `pack_bytes` and `instruments`: of the catalog in use from now on.
    NewPack {
        bytes: u64,
        pack_bytes: u64,
        instruments: Vec<BuiltinInstrument>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Fetch {
        round: u64,
    },
    /// Look at the online catalog (23-3).
    Check,
}

pub type Tell = Box<dyn Fn(PackNews) + Send>;

/// The handle of the thread. Dropping it lets the thread end after what it is doing; a file
/// being fetched is left as its `.part`.
pub struct Fetcher {
    jobs: Sender<Job>,
    stopped: Arc<AtomicU64>,
}

impl Fetcher {
    pub fn spawn(options: Options, catalog: Catalog, tell: Tell) -> io::Result<Fetcher> {
        let (jobs, rx) = mpsc::channel();
        let stopped = Arc::new(AtomicU64::new(0));
        let worker = Worker {
            agent: agent(&options, RootCerts::PlatformVerifier),
            web_pki: false,
            options,
            catalog,
            tell,
            stopped: stopped.clone(),
            #[cfg(test)]
            written: 0,
        };
        std::thread::Builder::new()
            .name("meloet-pack".into())
            .spawn(move || worker.run(rx))?;
        Ok(Fetcher { jobs, stopped })
    }

    pub fn ask(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    /// Stops the fetch of `round` and every one before it.
    pub fn stop(&self, round: u64) {
        self.stopped.fetch_max(round, Relaxed);
    }
}

impl Drop for Fetcher {
    fn drop(&mut self) {
        self.stopped.store(u64::MAX, Relaxed);
    }
}

fn agent(options: &Options, roots: RootCerts) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(options.https_only)
        .max_redirects(10)
        .user_agent(format!("meloet/{}", env!("CARGO_PKG_VERSION")))
        .http_status_as_error(false)
        .timeout_connect(Some(options.connect_timeout))
        .timeout_recv_response(Some(options.connect_timeout))
        .timeout_per_read(Some(options.read_timeout))
        .tls_config(TlsConfig::builder().root_certs(roots).build())
        .build()
        .new_agent()
}

/// Why a part did not come.
enum Miss {
    Stopped,
    Failed(PackFailure),
}

/// What one try at a file came to.
enum Try {
    Whole,
    Stopped,
    /// No answer at all.
    Offline,
    /// The answer stopped before the end.
    Cut,
    /// 403 or 410: the url the release sent meloet to is no longer good.
    Gone,
    /// Some other answer (404, 500).
    Refused,
    /// The `.part` does not fit what came: fetch the file from its start.
    Again,
    Disk(io::Error),
}

struct Worker {
    options: Options,
    catalog: Catalog,
    agent: ureq::Agent,
    /// The PC's certificates did not load: Mozilla's are used (23-2).
    web_pki: bool,
    tell: Tell,
    stopped: Arc<AtomicU64>,
    #[cfg(test)]
    written: u64,
}

/// The bytes of one fetch, for the progress.
struct Meter {
    numbers: Numbers,
    began: Instant,
    /// Bytes that came over the net in this fetch.
    came: u64,
    told: Option<Instant>,
    instrument: Option<BuiltinInstrument>,
    /// The largest file still to come: it is on the disk twice while it is unpacked.
    largest: u64,
}

impl Meter {
    /// The room the rest needs: what is still to come, the largest file once more for its
    /// unpacking, and [`ROOM_BYTES`].
    fn needed(&self) -> u64 {
        let left = self.numbers.total.saturating_sub(self.numbers.done);
        left + self.largest + ROOM_BYTES
    }

    fn remaining_s(&self) -> Option<u32> {
        let elapsed = self.began.elapsed();
        if elapsed < GUESS_AFTER || self.came == 0 {
            return None;
        }
        let per_s = self.came as f64 / elapsed.as_secs_f64();
        let left = self.numbers.total.saturating_sub(self.numbers.done) as f64;
        Some((left / per_s).ceil().min(f64::from(u32::MAX)) as u32)
    }
}

fn full(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::StorageFull | ErrorKind::QuotaExceeded)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Worker {
    fn run(mut self, rx: Receiver<Job>) {
        let _ = fs::create_dir_all(&self.options.disk.root);
        self.options.disk.tidy(&self.catalog);
        while let Ok(job) = rx.recv() {
            if self.stopped.load(Relaxed) == u64::MAX {
                return;
            }
            match job {
                Job::Fetch { round } => self.fetch(round),
                Job::Check => self.check(),
            }
        }
    }

    fn stopping(&self, round: u64) -> bool {
        self.stopped.load(Relaxed) >= round
    }

    fn free_bytes(&self) -> io::Result<u64> {
        #[cfg(test)]
        if let Some(free) = self.options.hooks.free {
            return Ok(free);
        }
        let s = rustix::fs::statvfs(&self.options.disk.root)?;
        Ok(s.f_bavail.saturating_mul(s.f_frsize))
    }

    fn disk_full(&self, meter: &Meter) -> PackFailure {
        PackFailure::DiskFull {
            needed_bytes: meter.needed(),
            free_bytes: self.free_bytes().unwrap_or(0),
        }
    }

    fn fetch(&mut self, round: u64) {
        if self.stopping(round) {
            return;
        }
        let disk = self.options.disk.clone();
        let outlook = disk.outlook(&self.catalog);
        let mut meter = Meter {
            numbers: outlook.numbers,
            began: Instant::now(),
            came: 0,
            told: None,
            instrument: None,
            largest: 0,
        };
        let parts: Vec<Part> = disk.to_fetch(&self.catalog).into_iter().cloned().collect();
        meter.largest = parts.iter().map(|p| p.bytes).max().unwrap_or(0);
        match self.free_bytes() {
            Ok(free) if free < meter.needed() => {
                let failure = PackFailure::DiskFull {
                    needed_bytes: meter.needed(),
                    free_bytes: free,
                };
                return self.failed(round, failure, &meter);
            }
            _ => {}
        }
        for (i, part) in parts.iter().enumerate() {
            meter.largest = parts[i..].iter().map(|p| p.bytes).max().unwrap_or(0);
            meter.instrument = part.instrument();
            meter.told = None;
            self.progress(round, &mut meter);
            match self.part(round, part, &mut meter) {
                Ok(json) => (self.tell)(PackNews::Installed {
                    round,
                    instrument: part.instrument(),
                    json,
                }),
                Err(Miss::Stopped) => return,
                Err(Miss::Failed(failure)) => return self.failed(round, failure, &meter),
            }
        }
        (self.tell)(PackNews::Done { round });
    }

    fn failed(&self, round: u64, failure: PackFailure, meter: &Meter) {
        (self.tell)(PackNews::Failed {
            round,
            failure,
            numbers: meter.numbers,
        });
    }

    fn progress(&self, round: u64, meter: &mut Meter) {
        if meter.told.is_some_and(|t| t.elapsed() < TELL_EVERY) {
            return;
        }
        meter.told = Some(Instant::now());
        (self.tell)(PackNews::Progress {
            round,
            instrument: meter.instrument,
            numbers: meter.numbers,
            remaining_s: meter.remaining_s(),
        });
    }

    /// Fetches one part, checks it and puts it in its place.
    fn part(&mut self, round: u64, part: &Part, meter: &mut Meter) -> Result<PathBuf, Miss> {
        let path = self.options.disk.part_file(part);
        let (mut cut, mut gone, mut corrupt) = (0, 0, 0);
        loop {
            let failure = match self.download(round, part, &path, meter) {
                Try::Whole => match self.place(part, &path) {
                    Ok(json) => return Ok(json),
                    Err(e) if full(&e) => Some(self.disk_full(meter)),
                    Err(e) => {
                        eprintln!("meloet: the sound pack's {} did not check: {e}", part.file);
                        let _ = fs::remove_file(&path);
                        meter.numbers.done = meter.numbers.done.saturating_sub(part.bytes);
                        corrupt += 1;
                        (corrupt > 1).then_some(PackFailure::Corrupt)
                    }
                },
                Try::Stopped => return Err(Miss::Stopped),
                Try::Offline => Some(PackFailure::Offline),
                // The net is there; the place answers no. Said as a cut, so that 「もう一度」
                // fits and nobody is told the net is not there.
                Try::Refused => Some(PackFailure::CutOff),
                Try::Disk(e) if full(&e) => Some(self.disk_full(meter)),
                Try::Disk(e) => {
                    eprintln!(
                        "meloet: the sound pack's {} was not written: {e}",
                        part.file
                    );
                    Some(PackFailure::CutOff)
                }
                Try::Again => None,
                Try::Gone => {
                    gone += 1;
                    (gone > GONE_RETRIES).then_some(PackFailure::CutOff)
                }
                Try::Cut => {
                    cut += 1;
                    if cut > CUT_RETRIES {
                        Some(PackFailure::CutOff)
                    } else {
                        let until = Instant::now() + self.options.retry_pause;
                        while Instant::now() < until && !self.stopping(round) {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        None
                    }
                }
            };
            if let Some(failure) = failure {
                return Err(Miss::Failed(failure));
            }
            if self.stopping(round) {
                return Err(Miss::Stopped);
            }
        }
    }

    fn call(
        &mut self,
        url: &str,
        from: u64,
    ) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
        loop {
            let mut request = self.agent.get(url);
            if from > 0 {
                request = request.header("Range", format!("bytes={from}-"));
            }
            match request.call() {
                Err(ureq::Error::Rustls(e))
                    if !self.web_pki && e.to_string().contains("No CA certificates") =>
                {
                    self.web_pki = true;
                    self.agent = agent(&self.options, RootCerts::WebPki);
                }
                answer => return answer,
            }
        }
    }

    /// One try at the bytes of `part` into `path`, from what is there already.
    fn download(&mut self, round: u64, part: &Part, path: &Path, meter: &mut Meter) -> Try {
        let mut have = fs::metadata(path).map_or(0, |m| m.len());
        if have > part.bytes {
            let _ = fs::remove_file(path);
            have = 0;
        }
        if have == part.bytes {
            return Try::Whole;
        }
        let response = match self.call(&part.url, have) {
            Ok(r) => r,
            Err(_) => return Try::Offline,
        };
        let from = match response.status().as_u16() {
            206 => {
                let range = response.headers().get("content-range");
                let starts = range
                    .and_then(|r| r.to_str().ok())
                    .and_then(|r| r.strip_prefix("bytes "))
                    .and_then(|r| r.split('-').next())
                    .and_then(|r| r.parse::<u64>().ok());
                if starts != Some(have) {
                    let _ = fs::remove_file(path);
                    meter.numbers.done = meter.numbers.done.saturating_sub(have);
                    return Try::Again;
                }
                have
            }
            200 => {
                meter.numbers.done = meter.numbers.done.saturating_sub(have);
                0
            }
            403 | 410 => return Try::Gone,
            416 => {
                let _ = fs::remove_file(path);
                meter.numbers.done = meter.numbers.done.saturating_sub(have);
                return Try::Again;
            }
            _ => return Try::Refused,
        };
        let file = (OpenOptions::new().create(true).write(true))
            .truncate(false)
            .open(path);
        let mut file = match file.and_then(|mut f| {
            f.set_len(from)?;
            f.seek(SeekFrom::Start(from))?;
            Ok(f)
        }) {
            Ok(f) => f,
            Err(e) => return Try::Disk(e),
        };
        let mut body = response.into_body().into_reader();
        let mut buffer = vec![0u8; 1 << 16];
        let mut at = from;
        loop {
            if self.stopping(round) {
                let _ = file.sync_all();
                return Try::Stopped;
            }
            let n = match body.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => {
                    let _ = file.sync_all();
                    return Try::Cut;
                }
            };
            if let Err(e) = self.write(&mut file, &buffer[..n]) {
                return Try::Disk(e);
            }
            meter.numbers.done += (n as u64).min(part.bytes.saturating_sub(at));
            at += n as u64;
            meter.came += n as u64;
            if at > part.bytes {
                break;
            }
            self.progress(round, meter);
        }
        if let Err(e) = file.sync_all() {
            return Try::Disk(e);
        }
        if at < part.bytes {
            Try::Cut
        } else {
            Try::Whole
        }
    }

    fn write(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        #[cfg(test)]
        if let Some(limit) = self.options.hooks.write_limit {
            self.written += bytes.len() as u64;
            if self.written > limit {
                return Err(io::Error::from(ErrorKind::StorageFull));
            }
        }
        file.write_all(bytes)
    }

    /// Checks the whole file and moves its content into its place. `InvalidData`: the file
    /// is not the one of the catalog.
    fn place(&self, part: &Part, path: &Path) -> io::Result<PathBuf> {
        let mut file = File::open(path)?;
        let mut hash = Sha256::new();
        let mut buffer = vec![0u8; 1 << 16];
        let mut len = 0u64;
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
            len += n as u64;
        }
        let sum: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if len != part.bytes || sum != part.sha256 {
            return Err(io::Error::new(ErrorKind::InvalidData, "size or sha256"));
        }
        let disk = &self.options.disk;
        let work = disk.root.join(format!("{WORK_PREFIX}{}", part.file));
        let _ = fs::remove_dir_all(&work);
        let unpacked = super::untar::unpack(path, &work).and_then(|()| {
            if work.join(Disk::marker(&part.id)).is_file() {
                Ok(())
            } else {
                Err(io::Error::new(ErrorKind::InvalidData, "no marker file"))
            }
        });
        if let Err(e) = unpacked {
            let _ = fs::remove_dir_all(&work);
            return Err(e);
        }
        let dir = disk.dir(part);
        fs::create_dir_all(dir.parent().unwrap_or(&disk.root))?;
        let _ = fs::remove_dir_all(&dir);
        fs::rename(&work, &dir)?;
        let _ = fs::remove_file(path);
        Ok(dir.join(Disk::marker(&part.id)))
    }

    fn now_ms(&self) -> u64 {
        #[cfg(test)]
        if let Some(now) = self.options.hooks.now_ms {
            return now;
        }
        now_ms()
    }

    /// Looks at the online catalog once in 24 hours, for a person who fetched something
    /// before (23-3). Says nothing when the net is not there or the catalog is not.
    fn check(&mut self) {
        let disk = self.options.disk.clone();
        if !disk.outlook(&self.catalog).fetched_any {
            return;
        }
        let Some(state) = self.options.state_dir.clone() else {
            return;
        };
        let stamp = state.join(CHECKED_FILE);
        let last = fs::read_to_string(&stamp)
            .ok()
            .and_then(|t| t.trim().parse::<u64>().ok());
        let now = self.now_ms();
        if last.is_some_and(|l| now.saturating_sub(l) < CHECK_EVERY.as_millis() as u64) {
            return;
        }
        let _ = fs::create_dir_all(&state)
            .and_then(|()| write_atomic(&stamp, now.to_string().as_bytes()));
        let online = self.options.online.clone();
        let Ok(mut response) = self.call(&online, 0) else {
            return;
        };
        if response.status().as_u16() != 200 {
            return;
        }
        let Ok(text) = (response.body_mut().with_config())
            .limit(CATALOG_LIMIT)
            .read_to_string()
        else {
            return;
        };
        match catalog::check(&text, &self.options.trust) {
            Ok(found) if found.newer_than(&self.catalog) => {
                if let Err(e) = write_atomic(&disk.root.join(CATALOG_FILE), text.as_bytes()) {
                    eprintln!("meloet: the new catalog of the sound pack was not kept: {e}");
                }
                self.catalog = found;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("meloet: the online catalog of the sound pack is not used: {e}");
                return;
            }
        }
        let outlook = disk.outlook(&self.catalog);
        if outlook.update_bytes > 0 {
            (self.tell)(PackNews::NewPack {
                bytes: outlook.update_bytes,
                pack_bytes: self.catalog.bytes(),
                instruments: self.catalog.instruments().collect(),
            });
        }
    }
}

/// The catalog to start from: the one kept from the net when it is newer than the one built
/// in and still checks.
pub fn start_catalog(root: &Path, built_in: &str, trust: &Trust) -> Option<Catalog> {
    let built_in = catalog::check(built_in, trust)
        .inspect_err(|e| eprintln!("meloet: the catalog of the sound pack does not check: {e}"))
        .ok();
    let kept = fs::read_to_string(root.join(CATALOG_FILE))
        .ok()
        .and_then(|t| catalog::check(&t, trust).ok());
    match (built_in, kept) {
        (Some(b), Some(k)) if k.newer_than(&b) => Some(k),
        (Some(b), _) => Some(b),
        (None, k) => k,
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)
}
