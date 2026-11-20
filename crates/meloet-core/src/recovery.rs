//! The recovery file ("なおすためのファイル", spec 3): the song with its unsaved changes, written
//! when the PC ends and a little after the song changes, so that the next start can bring it
//! back. It wraps the song file's shape ([`crate::songfile`]) without changing it:
//!
//! ```json
//! {"format": "meloet-recovery", "version": 2, "writtenMs": 1794750060000, "cause": "pcEnding",
//!  "path": "/home/…/はじめての曲/はじめての曲.meloet", "song": { …a whole song file… }}
//! ```
//!
//! `path` is the song's file, `null` for a song never saved. The song comes last, so that the
//! head still reads when the song does not (a newer meloet, a broken rule).

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use meloet_model::Song;
use meloet_protocol::RecoveryCause;
use serde::{Deserialize, Serialize};

use crate::songfile;

pub const FORMAT: &str = "meloet-recovery";
/// The shape this meloet writes, and the only one it reads. 2: the song inside is a song
/// file of version 6.
pub const VERSION: u32 = 2;
pub const FILE_NAME: &str = "recovery.json";

/// `$MELOET_STATE_DIR`, `$XDG_STATE_HOME/meloet`, or `~/.local/state/meloet`.
pub fn dir() -> Option<PathBuf> {
    std::env::var_os("MELOET_STATE_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            dirs::state_dir()
                .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))
                .map(|d| d.join("meloet"))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Cause {
    PcEnding,
    Sudden,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Head {
    format: String,
    version: u32,
    written_ms: u64,
    cause: Cause,
    path: Option<String>,
}

#[derive(Deserialize)]
struct Whole {
    #[serde(flatten)]
    head: Head,
    song: serde_json::Value,
}

/// A recovery file that read.
#[derive(Debug, Clone, PartialEq)]
pub struct Recovered {
    pub song: Song,
    /// The song's file; `None` for a song never saved.
    pub path: Option<PathBuf>,
    pub cause: RecoveryCause,
    pub written_ms: u64,
}

/// A recovery file that did not read. `path`: the song's file, when the head read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    pub path: Option<PathBuf>,
}

pub fn encode(song: &Song, path: Option<&Path>, cause: RecoveryCause, written_ms: u64) -> Vec<u8> {
    let head = Head {
        format: FORMAT.into(),
        version: VERSION,
        written_ms,
        cause: match cause {
            RecoveryCause::PcEnding => Cause::PcEnding,
            RecoveryCause::Sudden => Cause::Sudden,
        },
        // A path that is not UTF-8 cannot be written back as it is: the song counts as never
        // saved, and its first save makes a new folder.
        path: path.and_then(Path::to_str).map(str::to_owned),
    };
    let mut bytes = serde_json::to_vec(&head).expect("the head always serializes");
    let song = songfile::encode(song);
    let song = song.strip_suffix(b"\n").unwrap_or(&song);
    bytes.pop();
    bytes.extend_from_slice(b",\"song\":");
    bytes.extend_from_slice(song);
    bytes.extend_from_slice(b"}\n");
    bytes
}

pub fn decode(bytes: &[u8]) -> Result<Recovered, Unread> {
    let head_path = |head: &Head| head.path.as_ref().map(PathBuf::from);
    let Ok(whole) = serde_json::from_slice::<Whole>(bytes) else {
        // Unknown fields are skipped, the song too: only the head has to be right.
        let path = serde_json::from_slice::<Head>(bytes)
            .ok()
            .filter(|h| h.format == FORMAT)
            .and_then(|h| head_path(&h));
        return Err(Unread { path });
    };
    let head = whole.head;
    let path = head_path(&head);
    if head.format != FORMAT || head.version != VERSION {
        return Err(Unread {
            path: path.filter(|_| head.format == FORMAT),
        });
    }
    let song = serde_json::to_vec(&whole.song)
        .ok()
        .and_then(|b| songfile::decode(&b).ok());
    let Some(song) = song else {
        return Err(Unread { path });
    };
    Ok(Recovered {
        song,
        path,
        cause: match head.cause {
            Cause::PcEnding => RecoveryCause::PcEnding,
            Cause::Sudden => RecoveryCause::Sudden,
        },
        written_ms: head.written_ms,
    })
}

/// What the start finds in the recovery folder.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    Read(Box<Recovered>),
    /// The file did not read. It was renamed to `kept`, so that the next start does not find
    /// it again.
    Unreadable {
        kept: PathBuf,
        unread: Unread,
    },
}

/// Reads the recovery file in `dir`, if there is one. A file that does not read is kept under
/// a name with the time in it.
pub fn find(dir: &Path) -> Option<Found> {
    let file = dir.join(FILE_NAME);
    let bytes = match fs::read(&file) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => {
            eprintln!("meloet: cannot read {}: {e}", file.display());
            Vec::new()
        }
    };
    match decode(&bytes) {
        Ok(recovered) => Some(Found::Read(Box::new(recovered))),
        Err(unread) => {
            let kept = keep_unreadable(dir, &file, now_ms()).unwrap_or_else(|e| {
                eprintln!("meloet: cannot rename {}: {e}", file.display());
                file.clone()
            });
            Some(Found::Unreadable { kept, unread })
        }
    }
}

/// `recovery-unreadable-20261115-134100.json` (UTC), with " 2", " 3", … when taken.
fn keep_unreadable(dir: &Path, file: &Path, ms: u64) -> io::Result<PathBuf> {
    let stamp = utc_stamp(ms / 1000);
    for n in 1u32.. {
        let name = if n == 1 {
            format!("recovery-unreadable-{stamp}.json")
        } else {
            format!("recovery-unreadable-{stamp} {n}.json")
        };
        let kept = dir.join(name);
        if kept.exists() {
            continue;
        }
        fs::rename(file, &kept)?;
        sync_dir(dir)?;
        return Ok(kept);
    }
    unreachable!("u32 names run out")
}

/// `YYYYMMDD-HHMMSS` in UTC.
fn utc_stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Howard Hinnant's days-from-civil, backwards.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Writes `bytes` as the recovery file in `dir`. Like a song's save: a whole temporary file,
/// synced, renamed into place, and the folder synced, so that the file there is always whole.
pub fn write(dir: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let fresh = dir.join(format!(".{FILE_NAME}.new"));
    let result = (|| {
        let mut file = File::create(&fresh)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&fresh, dir.join(FILE_NAME))?;
        sync_dir(dir)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&fresh);
    }
    result
}

pub fn remove(dir: &Path) -> io::Result<()> {
    match fs::remove_file(dir.join(FILE_NAME)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => sync_dir(dir).or(Ok(())),
    }
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_: &Path) -> io::Result<()> {
    Ok(())
}

enum Job {
    Write {
        song: Box<Song>,
        path: Option<PathBuf>,
        cause: RecoveryCause,
        /// Told once the file is written (or failed to be).
        done: Vec<Sender<()>>,
    },
    Remove,
}

/// The thread that writes and removes the recovery file, in the order asked, so that the core
/// thread never waits for the disk. Of jobs that pile up, only the last is done.
pub struct Writer {
    tx: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    pub fn start(dir: PathBuf) -> Writer {
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("meloet-recovery".into())
            .spawn(move || writer_loop(&dir, &rx))
            .expect("failed to start the recovery thread");
        Writer {
            tx: Some(tx),
            thread: Some(thread),
        }
    }

    /// `done` is told once this song, or a later job, is done.
    pub fn write(
        &self,
        song: Box<Song>,
        path: Option<PathBuf>,
        cause: RecoveryCause,
        done: Vec<Sender<()>>,
    ) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Job::Write {
                song,
                path,
                cause,
                done,
            });
        }
    }

    pub fn remove(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Job::Remove);
        }
    }
}

impl Drop for Writer {
    /// Waits for the jobs asked so far.
    fn drop(&mut self) {
        drop(self.tx.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn writer_loop(dir: &Path, rx: &Receiver<Job>) {
    while let Ok(first) = rx.recv() {
        let mut last = first;
        let mut done = Vec::new();
        while let Ok(next) = rx.try_recv() {
            if let Job::Write { done: d, .. } = std::mem::replace(&mut last, next) {
                done.extend(d);
            }
        }
        match last {
            Job::Write {
                song,
                path,
                cause,
                done: d,
            } => {
                done.extend(d);
                let bytes = encode(&song, path.as_deref(), cause, now_ms());
                if let Err(e) = write(dir, &bytes) {
                    eprintln!("meloet: the recovery file was not written: {e}");
                }
            }
            Job::Remove => {
                if let Err(e) = remove(dir) {
                    eprintln!("meloet: the recovery file was not removed: {e}");
                }
            }
        }
        for d in done {
            let _ = d.send(());
        }
    }
}

#[cfg(test)]
mod tests;
