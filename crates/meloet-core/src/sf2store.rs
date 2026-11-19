//! The SF2 files the person added: the folder they are copied into, what each holds, and the
//! thread that copies them and reads their presets (docs/architecture.md 20-7).
//!
//! A file is known by its name ("FluidR3_GM.sf2"); that is also what a song names it by. A
//! file added under a name that is there already takes that one's place.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use meloet_model::{GmFamily, SF2_DRUM_BANK, Sf2Ref};
use meloet_sound::sampler::SampleBank;
use meloet_sound::sf2::{Sf2Error, Sf2File};
use sha2::{Digest, Sha256};

use crate::instruments::Samples;

/// `$MELOET_SF2_DIR` (for tests), else `meloet/sf2` in the user's data folder
/// (`$XDG_DATA_HOME`, usually `~/.local/share`).
pub fn dir() -> Option<PathBuf> {
    std::env::var_os("MELOET_SF2_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::data_dir().map(|d| d.join("meloet").join("sf2")))
}

/// One preset of a file, as the shelf and the tracks need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sf2Preset {
    pub bank: u16,
    pub program: u8,
    pub name: String,
    /// A drum kit: the keys it has a sound on, lowest first. Empty for the others.
    pub keys: Vec<u8>,
}

impl Sf2Preset {
    pub fn family(&self) -> GmFamily {
        GmFamily::of(self.bank, self.program)
    }
}

/// An added file and its presets, in the order of the shelf: by family, inside a family by
/// program, then by bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sf2Info {
    pub file: String,
    /// Of the file as it was copied in, in lower-case hex; empty when not known (a file put
    /// into the folder by hand). A record only: a song is tied to a file by its name.
    pub sha256: String,
    pub presets: Vec<Sf2Preset>,
}

impl Sf2Info {
    pub fn preset(&self, bank: u16, program: u8) -> Option<&Sf2Preset> {
        (self.presets.iter()).find(|p| p.bank == bank && p.program == program)
    }
}

/// A preset of an added file: what its samples are read and kept by.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sf2Key {
    pub file: Arc<str>,
    pub bank: u16,
    pub program: u8,
}

impl Sf2Key {
    pub fn of(preset: &Sf2Ref) -> Sf2Key {
        Sf2Key {
            file: preset.file.clone(),
            bank: preset.bank,
            program: preset.program,
        }
    }
}

/// What the file at `path` holds. Presets General MIDI has no place for (a program over
/// 127, a bank over 128) are left out.
pub fn describe(path: &Path) -> Result<Vec<Sf2Preset>, Sf2Error> {
    let file = Sf2File::open(path)?;
    let mut presets: Vec<Sf2Preset> = (file.presets().into_iter())
        .filter(|p| p.program <= 127 && p.bank <= SF2_DRUM_BANK)
        .map(|p| {
            let keys = if p.bank == SF2_DRUM_BANK {
                file.keys(p.bank, p.program).unwrap_or_default()
            } else {
                Vec::new()
            };
            Sf2Preset {
                bank: p.bank,
                program: p.program as u8,
                name: p.name,
                keys,
            }
        })
        .collect();
    presets.sort_by_key(|p| (p.family(), p.program, p.bank));
    Ok(presets)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn is_sf2_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sf2"))
}

/// Whether a file held over the window can be added, going by its name alone.
pub fn looks_like_sf2(path: &Path) -> bool {
    is_sf2_name(&file_name(path))
}

/// Where the hash of a copied file is noted, beside it: "<hex> <length of the file>".
fn hash_note(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".sha256");
    PathBuf::from(name)
}

/// The hash noted when the file was copied in. None when there is no note, or the file is
/// not as long as the one the note is of.
fn noted_hash(path: &Path) -> Option<String> {
    let note = std::fs::read_to_string(hash_note(path)).ok()?;
    let (hash, len) = note.trim_end().split_once(' ')?;
    let fits = hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let same = len.parse::<u64>().ok()? == std::fs::metadata(path).ok()?.len();
    (fits && same).then(|| hash.to_string())
}

/// Every SF2 file in `dir` that reads, in the order of their names.
pub fn list(dir: &Path) -> Vec<Sf2Info> {
    let mut files: Vec<Sf2Info> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let file = file_name(&path);
            if !is_sf2_name(&file) || !path.is_file() {
                return None;
            }
            match describe(&path) {
                Ok(presets) => Some(Sf2Info {
                    sha256: noted_hash(&path).unwrap_or_default(),
                    file,
                    presets,
                }),
                Err(e) => {
                    eprintln!("meloet: {} does not read: {e}", path.display());
                    None
                }
            }
        })
        .collect();
    files.sort_by(|a, b| a.file.cmp(&b.file));
    files
}

/// Copies `source` into `dir` under its own name and tells what it holds. The file is read
/// first: one that does not read is not copied. Whenever this stops, `dir` holds the file of
/// that name as it was before, or the new one whole: the copy is written beside it, synced
/// and renamed over it.
pub fn add(dir: &Path, source: &Path) -> Result<Sf2Info, Sf2Error> {
    let io = |e: std::io::Error| Sf2Error::Io(e.to_string());
    let file = file_name(source);
    let presets = describe(source)?;
    std::fs::create_dir_all(dir).map_err(io)?;
    let target = dir.join(&file);
    let temporary = dir.join(format!(".{file}.tmp"));
    let copied = (|| {
        let mut from = std::fs::File::open(source)?;
        let mut to = std::fs::File::create(&temporary)?;
        let (mut hash, mut len) = (Sha256::new(), 0u64);
        let mut buffer = vec![0u8; 1 << 16];
        loop {
            let n = match from.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            hash.update(&buffer[..n]);
            to.write_all(&buffer[..n])?;
            len += n as u64;
        }
        to.sync_all()?;
        drop(to);
        let sha256: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        // The note of a file this one takes the place of must not be read as this one's.
        let note = hash_note(&target);
        let _ = std::fs::remove_file(&note);
        std::fs::rename(&temporary, &target)?;
        // Without the note the hash is only not known at the next start.
        let _ = std::fs::write(&note, format!("{sha256} {len}\n"));
        #[cfg(unix)]
        std::fs::File::open(dir)?.sync_all()?;
        Ok(sha256)
    })();
    match copied {
        Ok(sha256) => Ok(Sf2Info {
            file,
            sha256,
            presets,
        }),
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            Err(io(e))
        }
    }
}

/// What the thread answers with.
pub(crate) enum Sf2Done {
    /// `Err`: the name of the file that was not added.
    Added(Result<Sf2Info, String>),
    Loaded(Sf2Key, Result<Arc<SampleBank>, Sf2Error>),
}

enum Job {
    Add(PathBuf),
    Load(Sf2Key, u32),
}

pub(crate) type Done = Box<dyn Fn(Sf2Done) + Send>;

/// The added files as the core thread knows them, the samples of the presets in use, and the
/// thread that reads both. The thread starts with its first job.
pub(crate) struct Sf2Store {
    dir: Option<PathBuf>,
    files: Vec<Sf2Info>,
    banks: Vec<(Sf2Key, Arc<SampleBank>)>,
    done: Option<Done>,
    jobs: Option<Sender<Job>>,
    reader: Option<JoinHandle<()>>,
}

impl Sf2Store {
    /// Reads what `dir` holds. With `None` nothing can be added.
    pub fn open(dir: Option<PathBuf>, done: Done) -> Sf2Store {
        Sf2Store {
            files: dir.as_deref().map(list).unwrap_or_default(),
            dir,
            banks: Vec::new(),
            done: Some(done),
            jobs: None,
            reader: None,
        }
    }

    pub fn files(&self) -> &[Sf2Info] {
        &self.files
    }

    fn job(&mut self, job: Job) {
        let Some(dir) = self.dir.clone() else {
            return;
        };
        if self.jobs.is_none()
            && let Some(done) = self.done.take()
        {
            let (tx, rx) = mpsc::channel();
            self.jobs = Some(tx);
            self.reader = std::thread::Builder::new()
                .name("meloet-sf2".into())
                .spawn(move || {
                    for job in rx {
                        done(run(&dir, job));
                    }
                })
                .ok();
        }
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// Copies the file in, off this thread. Answered with [`Sf2Done::Added`]. Returns
    /// false when there is no folder to copy it into: no answer comes then.
    pub fn add(&mut self, source: PathBuf) -> bool {
        self.job(Job::Add(source));
        self.dir.is_some()
    }

    /// The file was added: it is listed from now on, in place of one of the same name, and
    /// the samples read from that one are let go.
    pub fn added(&mut self, info: Sf2Info) {
        self.banks.retain(|(key, _)| *key.file != *info.file);
        self.files.retain(|f| f.file != info.file);
        self.files.push(info);
        self.files.sort_by(|a, b| a.file.cmp(&b.file));
    }

    /// Reads the samples of the preset, off this thread. Answered with [`Sf2Done::Loaded`].
    pub fn load(&mut self, key: Sf2Key, sample_rate: u32) {
        self.job(Job::Load(key, sample_rate));
    }

    pub fn keep(&mut self, key: Sf2Key, bank: Arc<SampleBank>) {
        if !self.banks.iter().any(|(k, _)| *k == key) {
            self.banks.push((key, bank));
        }
    }

    pub fn let_go(&mut self, key: &Sf2Key) {
        self.banks.retain(|(k, _)| k != key);
    }

    /// The preset does not read: it is listed no more.
    pub fn unreadable(&mut self, key: &Sf2Key) {
        self.let_go(key);
        for f in self.files.iter_mut().filter(|f| *f.file == *key.file) {
            (f.presets).retain(|p| (p.bank, p.program) != (key.bank, key.program));
        }
    }

    pub fn samples(&self, preset: &Sf2Ref) -> Samples {
        let key = Sf2Key::of(preset);
        let listed = (self.files.iter())
            .any(|f| *f.file == *key.file && f.preset(key.bank, key.program).is_some());
        match self.banks.iter().find(|(k, _)| *k == key) {
            Some((_, bank)) => Samples::Ready(bank.clone()),
            None if listed => Samples::NotRead,
            None => Samples::NotInstalled,
        }
    }
}

impl Drop for Sf2Store {
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn run(dir: &Path, job: Job) -> Sf2Done {
    match job {
        Job::Add(source) => Sf2Done::Added(add(dir, &source).map_err(|e| {
            eprintln!("meloet: {} was not added: {e}", source.display());
            file_name(&source)
        })),
        Job::Load(key, sample_rate) => {
            let read = Sf2File::open(&dir.join(&*key.file))
                .and_then(|file| file.load(key.bank, u16::from(key.program), sample_rate as f32));
            Sf2Done::Loaded(key, read.map(Arc::new))
        }
    }
}
