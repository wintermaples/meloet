//! The audio files of a song: bringing them in, the places they are kept in, the song's
//! folder (docs/architecture.md 22-4, 22-8, 22-18 A2).
//!
//! - A song's audio is in the folder `音/` beside the song's file, or else in the work
//!   place (`<state>/work/音/`): what was recorded or brought in since the last save.
//! - A file that is not a plain wav at the rate the engine runs at is decoded and
//!   resampled once into the cache (`<cache>/audio/<uid>-<rate>.wav`, 32-bit float), and
//!   played from there.
//! - [`AudioStore`] knows the places. [`keeper::Keeper`] is the thread that copies,
//!   decodes and makes peaks.

pub mod decode;
pub mod keeper;
pub mod resample;
pub mod wav;

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use meloet_protocol::AUDIO_EXTENSIONS;
use sha2::{Digest, Sha256};

use crate::pcm::{PcmFile, PcmStore};
use crate::vocalrec::work_audio_dir;

/// The folder of a song's audio, beside the song's file.
pub const SONG_AUDIO: &str = "音";

/// Where the read-ahead thread finds the song's audio at the engine's rate (22-4, 22-8):
/// the song's `音/`, the work place, and the cache of decoded and resampled files. One for
/// the life of the core, shared with that thread and with the thread that reads audio
/// files in.
pub struct AudioStore {
    places: Mutex<Places>,
    /// Goes up whenever a file may be found that was not before.
    changed: AtomicU64,
}

#[derive(Default)]
struct Places {
    /// `$MELOET_STATE_DIR`… as the recovery file has it; `None` in tests without one.
    state_dir: Option<PathBuf>,
    /// `$XDG_CACHE_HOME/meloet`; `None`: nothing is kept, and only plain wav files at the
    /// engine's rate sound.
    cache_dir: Option<PathBuf>,
    /// The folder of the song file; `None` while the song was never saved.
    song_dir: Option<PathBuf>,
    /// Audio folders the song had before it was saved under another name, and the one a
    /// save is moving files into: the history may still name files that are only there.
    also: Vec<PathBuf>,
    /// `AudioSource::uid` → `AudioSource::file`, for the song as it is.
    sources: Vec<([u8; 16], String)>,
}

/// A name of the audio table is followed only as a plain file name.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
}

impl Places {
    fn work(&self) -> Option<PathBuf> {
        self.state_dir.as_deref().map(work_audio_dir)
    }

    /// The audio folders of the song, the one of its file first.
    fn folders(&self) -> impl Iterator<Item = PathBuf> + '_ {
        (self.song_dir.iter().map(|d| d.join(SONG_AUDIO))).chain(self.also.iter().cloned())
    }

    fn find(&self, name: &str) -> Option<PathBuf> {
        if !plain_name(name) {
            return None;
        }
        (self.folders().chain(self.work()))
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether a file can be played now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Playable {
    /// As it is, or from the cache.
    Ready,
    /// The file is there and has to be decoded into the cache first.
    NeedsCache(PathBuf),
    Missing,
}

impl AudioStore {
    pub fn new(state_dir: Option<PathBuf>) -> AudioStore {
        AudioStore {
            places: Mutex::new(Places {
                state_dir,
                ..Places::default()
            }),
            changed: AtomicU64::new(0),
        }
    }

    /// With the folder of the caches (`CoreOptions::cache_dir`).
    pub fn with_cache(self, cache_dir: Option<PathBuf>) -> AudioStore {
        self.lock().cache_dir = cache_dir;
        self
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Places> {
        self.places.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn touch(&self) {
        self.changed.fetch_add(1, Relaxed);
    }

    /// Goes up whenever a file that [`PcmStore::open`] did not give may be there now (it
    /// was found, made ready, or the song's folder changed). The read-ahead thread asks
    /// again for what it is missing when this is another number.
    pub fn generation(&self) -> u64 {
        self.changed.load(Relaxed)
    }

    /// Another song: it was opened (`Some`), or it is new (`None`).
    pub fn set_song_dir(&self, dir: Option<PathBuf>) {
        let mut places = self.lock();
        places.song_dir = dir;
        places.also.clear();
        drop(places);
        self.touch();
    }

    /// The same song has its file in `dir` now (a save, a save under another name). The
    /// folder it had stays a place to look in, for what only the history names.
    pub fn song_moved(&self, dir: PathBuf) {
        let mut places = self.lock();
        let new_audio = dir.join(SONG_AUDIO);
        if let Some(old) = places.song_dir.replace(dir).map(|d| d.join(SONG_AUDIO))
            && old != new_audio
            && !places.also.contains(&old)
        {
            places.also.push(old);
        }
        places.also.retain(|d| *d != new_audio);
        drop(places);
        self.touch();
    }

    /// The song's audio table changed.
    pub fn set_sources(&self, sources: Vec<([u8; 16], String)>) {
        self.lock().sources = sources;
        self.touch();
    }

    /// The work place; `None` without a state folder.
    pub fn work_dir(&self) -> Option<PathBuf> {
        self.lock().work()
    }

    /// Where the file of that name is: the song's audio folder, or else the work place.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        self.lock().find(name)
    }

    /// Where the audio with this `uid` is kept once it is brought to `sample_rate`.
    pub fn cache_path(&self, uid: &[u8; 16], sample_rate: u32) -> Option<PathBuf> {
        let dir = self.lock().cache_dir.clone()?;
        Some(
            dir.join("audio")
                .join(format!("{}-{sample_rate}.wav", hex(uid))),
        )
    }

    /// Where the peaks of the audio with this `uid` are kept.
    pub fn peaks_path(&self, uid: &[u8; 16]) -> Option<PathBuf> {
        let dir = self.lock().cache_dir.clone()?;
        Some(crate::peaks::peaks_path(&dir.join("peaks"), uid))
    }

    fn cached(&self, uid: &[u8; 16], sample_rate: u32) -> Option<wav::WavPcm> {
        let wav = wav::WavPcm::open(&self.cache_path(uid, sample_rate)?).ok()?;
        (wav.sample_rate() == sample_rate).then_some(wav)
    }

    /// Whether the file `name` with this `uid` can be played at `sample_rate`.
    pub fn playable(&self, uid: &[u8; 16], name: &str, sample_rate: u32) -> Playable {
        let Some(path) = self.find(name) else {
            return Playable::Missing;
        };
        let direct = wav::WavPcm::open(&path).is_ok_and(|w| w.sample_rate() == sample_rate);
        if direct || self.cached(uid, sample_rate).is_some() {
            Playable::Ready
        } else {
            Playable::NeedsCache(path)
        }
    }

    /// A file was put where the song looks for it, or into the cache.
    pub fn file_arrived(&self) {
        self.touch();
    }

    /// Copies the file at `from` into the work place under its own name, as it is. A file
    /// of that name that is there already (in the song's folder or the work place) is
    /// used when it holds the same bytes; else the name gets " 2", " 3", …. So do names in
    /// `taken`: names of the song's table whose files are not found. Returns the name and
    /// the SHA-256 of the file.
    pub fn bring_in(&self, from: &Path, taken: &[String]) -> io::Result<(String, [u8; 32])> {
        let work = (self.work_dir()).ok_or_else(|| io::Error::other("no work place"))?;
        fs::create_dir_all(&work)?;
        let original = (from.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| plain_name(n))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let temp = work.join(format!(
            ".in-{}-{}.part",
            std::process::id(),
            SERIAL.fetch_add(1, Relaxed)
        ));
        let sha = copy_synced(from, &temp).inspect_err(|_| {
            let _ = fs::remove_file(&temp);
        })?;
        let (stem, ending) = match original.rfind('.').filter(|&dot| dot > 0) {
            Some(dot) => (&original[..dot], &original[dot..]),
            None => (original.as_str(), ""),
        };
        for n in 1u32.. {
            let name = match n {
                1 => original.clone(),
                _ => format!("{stem} {n}{ending}"),
            };
            match self.find(&name) {
                Some(there) => {
                    if sha256_of(&there).is_ok_and(|s| s == sha) {
                        let _ = fs::remove_file(&temp);
                        return Ok((name, sha));
                    }
                }
                None if taken.contains(&name) => {}
                None => {
                    fs::rename(&temp, work.join(&name))?;
                    sync_dir(&work)?;
                    self.touch();
                    return Ok((name, sha));
                }
            }
        }
        unreachable!("u32 names run out")
    }

    /// Before the song's file is written into `song_dir` (22-4): every file of `names`
    /// (the audio the saved song names) that is not in `song_dir/音` yet is put there.
    /// From the work place it is moved (a rename; across file systems a copy that is
    /// synced and renamed, then the original is removed). From an audio folder the song
    /// had before (a save under another name) it is copied, and that folder is left as
    /// it is. A file that is found nowhere is passed over: its clips stay silent.
    ///
    /// At any moment a file of `names` is whole in one of the places the song looks in.
    pub fn settle(&self, song_dir: &Path, names: &[String]) -> io::Result<()> {
        self.settle_until(song_dir, names, usize::MAX)
    }

    /// As [`AudioStore::settle`], giving up with an error before file number `stop` (from
    /// 0) is put in place: what a save that is cut short leaves behind.
    fn settle_until(&self, song_dir: &Path, names: &[String], stop: usize) -> io::Result<()> {
        let dest = song_dir.join(SONG_AUDIO);
        let (folders, work) = {
            let mut places = self.lock();
            let folders: Vec<PathBuf> = places.folders().filter(|d| *d != dest).collect();
            // From now on the song looks in the folder files are moved into.
            let current = places.song_dir.as_ref().map(|d| d.join(SONG_AUDIO));
            if current.as_ref() != Some(&dest) && !places.also.contains(&dest) {
                places.also.push(dest.clone());
            }
            (folders, places.work())
        };
        let mut seen = HashSet::new();
        let mut put = 0usize;
        for name in names {
            if !plain_name(name) || !seen.insert(name.as_str()) {
                continue;
            }
            let to = dest.join(name);
            if to.is_file() {
                continue;
            }
            let kept = folders.iter().map(|d| d.join(name)).find(|p| p.is_file());
            let fresh = work.as_ref().map(|d| d.join(name)).filter(|p| p.is_file());
            let Some((from, moving)) = kept.map(|p| (p, false)).or(fresh.map(|p| (p, true))) else {
                continue;
            };
            if put == stop {
                return Err(io::Error::other("stopped"));
            }
            fs::create_dir_all(&dest)?;
            if moving {
                move_file(&from, &to, false)?;
            } else {
                copy_into(&from, &to)?;
            }
            put += 1;
        }
        if put > 0 {
            sync_dir(&dest)?;
            self.touch();
        }
        Ok(())
    }

    /// Empties the work place of audio (22-4): when the song is let go of, and at the
    /// start when nothing names what is in there. Never the song's folder.
    pub fn clear_work(&self) {
        let Some(work) = self.work_dir() else {
            return;
        };
        let Ok(entries) = fs::read_dir(&work) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_file())
                && let Err(e) = fs::remove_file(entry.path())
            {
                eprintln!("meloet: {} was not removed: {e}", entry.path().display());
            }
        }
    }
}

impl PcmStore for AudioStore {
    fn open(&self, uid: &[u8; 16], sample_rate: u32) -> Option<Box<dyn PcmFile>> {
        let places = self.lock();
        let name = &places.sources.iter().find(|(u, _)| u == uid)?.1;
        let path = places.find(name)?;
        drop(places);
        match wav::WavPcm::open(&path) {
            Ok(wav) if wav.sample_rate() == sample_rate => Some(Box::new(wav)),
            _ => Some(Box::new(self.cached(uid, sample_rate)?)),
        }
    }
}

fn sha256_of(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut block = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut block)?;
        if n == 0 {
            return Ok(hash.finalize().into());
        }
        hash.update(&block[..n]);
    }
}

/// Copies `from` to `to` and syncs it. Returns the SHA-256 of what was copied.
fn copy_synced(from: &Path, to: &Path) -> io::Result<[u8; 32]> {
    let mut source = File::open(from)?;
    let mut out = File::create(to)?;
    let mut hash = Sha256::new();
    let mut block = vec![0u8; 1 << 16];
    loop {
        let n = source.read(&mut block)?;
        if n == 0 {
            break;
        }
        hash.update(&block[..n]);
        out.write_all(&block[..n])?;
    }
    out.sync_all()?;
    Ok(hash.finalize().into())
}

/// `from` appears at `to` whole or not at all: copied under a temporary name beside `to`,
/// synced, then renamed.
fn copy_into(from: &Path, to: &Path) -> io::Result<()> {
    let mut temp = to.as_os_str().to_owned();
    temp.push(".part");
    let temp = PathBuf::from(temp);
    let copied = copy_synced(from, &temp).and_then(|_| fs::rename(&temp, to));
    if copied.is_err() {
        let _ = fs::remove_file(&temp);
    }
    copied
}

/// Moves a file. Across file systems (`copy`, or the rename says so): a synced copy that
/// is renamed into place, and only then the original is removed.
fn move_file(from: &Path, to: &Path, copy: bool) -> io::Result<()> {
    if !copy {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {}
            Err(e) => return Err(e),
        }
    }
    copy_into(from, to)?;
    if let Some(dir) = to.parent() {
        sync_dir(dir)?;
    }
    fs::remove_file(from)
}

/// The renames in `dir` reach the disk. Windows has no way to open a folder for this.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_: &Path) -> io::Result<()> {
    Ok(())
}

/// Whether a file held over the window may be audio that reads, going by its name alone
/// (spec 3 「読めるファイル」). What it holds is found out when it is brought in.
pub fn looks_like_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

#[cfg(test)]
mod tests;
