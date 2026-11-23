//! 「新しい版をたしかめる」 (round 12, D1): after the screen's greeting, at most once a day, one
//! read of the newest release. Only the `User-Agent` goes out; a failure says nothing. Runs on
//! a thread of its own, never on the core thread.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ureq::tls::{RootCerts, TlsConfig};

/// Where the newest release is read.
pub const LATEST_URL: &str = "https://api.github.com/repos/wintermaples/meloet/releases/latest";
/// The page of a release is this followed by its tag.
const RELEASE_PAGE: &str = "https://github.com/wintermaples/meloet/releases/tag/v";
/// The release is read no more often than this.
pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// The whole read, from the name lookup to the end of the answer.
pub const TIMEOUT: Duration = Duration::from_secs(5);
const ANSWER_LIMIT: u64 = 1024 * 1024;
/// Beside settings.json.
const STAMP_FILE: &str = "update-check.json";

/// This meloet's version ("0.3.0").
pub fn this_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The page of the release of `version` ("0.4.0").
pub fn release_page(version: &str) -> String {
    format!("{RELEASE_PAGE}{version}")
}

/// Three numbers, as the tags have them (`v0.4.0`, or `0.4.0`). Anything else is not a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64, u64, u64);

impl Version {
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        let mut parts = text.split('.');
        let mut number = || -> Option<u64> {
            let p = parts.next()?;
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse().ok()
        };
        let version = Version(number()?, number()?, number()?);
        parts.next().is_none().then_some(version)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// `found` ("v0.4.0") as shown ("0.4.0") when it is newer than `current`.
pub fn newer(found: &str, current: &str) -> Option<String> {
    let found = Version::parse(found)?;
    let current = Version::parse(current)?;
    (found > current).then(|| found.to_string())
}

#[derive(Debug, Clone)]
pub struct Options {
    pub url: String,
    /// Where the time of the last read and what it found are kept. `None`: nothing is kept,
    /// and every start reads once.
    pub stamp_dir: Option<PathBuf>,
    /// Off only for a test server.
    pub https_only: bool,
    pub timeout: Duration,
    /// The version the found one is compared with.
    pub current: String,
}

impl Options {
    pub fn new(url: String, stamp_dir: Option<PathBuf>) -> Options {
        Options {
            https_only: !url.starts_with("http://"),
            url,
            stamp_dir,
            timeout: TIMEOUT,
            current: this_version().to_owned(),
        }
    }

    /// The release on the net, remembered beside the settings in `config_dir`.
    #[cfg(not(feature = "dev-hooks"))]
    pub fn from_env(config_dir: Option<PathBuf>) -> Option<Options> {
        Some(Options::new(LATEST_URL.to_owned(), config_dir))
    }

    /// With the development hooks meloet reads only `MELOET_UPDATE_URL` (a test server), and
    /// keeps nothing, so that what a test server said is never shown by the release build.
    #[cfg(feature = "dev-hooks")]
    pub fn from_env(_config_dir: Option<PathBuf>) -> Option<Options> {
        let url = std::env::var("MELOET_UPDATE_URL")
            .ok()
            .filter(|v| !v.is_empty())?;
        Some(Options::new(url, None))
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stamp {
    /// Unix ms of the last read (also of one that failed).
    checked_ms: u64,
    /// The version the last read found ("0.4.0"), newer or not.
    #[serde(default)]
    latest: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Starts the read on a thread of its own; `tell` gets a version newer than this one.
pub fn spawn(options: Options, tell: Box<dyn FnOnce(String) + Send>) {
    let started = std::thread::Builder::new()
        .name("meloet-updates".into())
        .spawn(move || {
            if let Some(version) = check(&options, now_ms()) {
                tell(version);
            }
        });
    if let Err(e) = started {
        eprintln!("meloet: no thread to look for a new version: {e}");
    }
}

/// A version newer than `options.current`, read at most once in [`CHECK_EVERY`]; in between,
/// what the last read found.
pub fn check(options: &Options, now: u64) -> Option<String> {
    let stamp_path = options.stamp_dir.as_ref().map(|d| d.join(STAMP_FILE));
    let kept: Option<Stamp> = stamp_path
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok());
    let recent = kept.as_ref().is_some_and(|s| {
        s.checked_ms <= now && now - s.checked_ms < CHECK_EVERY.as_millis() as u64
    });
    let latest = if recent {
        kept.and_then(|s| s.latest)
    } else {
        let before = kept.and_then(|s| s.latest);
        // Kept before the read, so that a read that fails or hangs also counts for the day.
        keep(&stamp_path, now, before.clone());
        match read(options) {
            Some(found) => {
                keep(&stamp_path, now, Some(found.clone()));
                Some(found)
            }
            None => before,
        }
    };
    newer(&latest?, &options.current)
}

fn keep(path: &Option<PathBuf>, now: u64, latest: Option<String>) {
    let Some(path) = path else {
        return;
    };
    let stamp = Stamp {
        checked_ms: now,
        latest,
    };
    let Ok(text) = serde_json::to_string(&stamp) else {
        return;
    };
    let written = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| write_atomic(path, text.as_bytes()));
    if let Err(e) = written {
        eprintln!("meloet: the time of the look for a new version was not kept: {e}");
    }
}

fn agent(options: &Options, roots: RootCerts) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(options.https_only)
        .max_redirects(5)
        .user_agent(format!("meloet/{}", this_version()))
        .accept("")
        .http_status_as_error(false)
        .timeout_global(Some(options.timeout))
        .tls_config(TlsConfig::builder().root_certs(roots).build())
        .build()
        .new_agent()
}

/// The version of the newest release ("0.4.0"), or `None` for anything that went wrong.
fn read(options: &Options) -> Option<String> {
    let mut answer = match agent(options, RootCerts::PlatformVerifier)
        .get(&options.url)
        .call()
    {
        Err(ureq::Error::Rustls(e)) if e.to_string().contains("No CA certificates") => {
            agent(options, RootCerts::WebPki).get(&options.url).call()
        }
        answer => answer,
    }
    .ok()?;
    if answer.status().as_u16() != 200 {
        return None;
    }
    let text = (answer.body_mut().with_config())
        .limit(ANSWER_LIMIT)
        .read_to_string()
        .ok()?;
    let release: Release = serde_json::from_str(&text).ok()?;
    Version::parse(&release.tag_name).map(|v| v.to_string())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests;
