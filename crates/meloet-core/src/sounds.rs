//! The sound pack on this PC: where it is, which instruments it has, and their samples in
//! memory (docs/architecture.md 19-1 and 19-7).
//!
//! Samples are read on a thread of their own, one instrument after the other, so that neither
//! the core thread nor the audio thread waits for the disk. The core thread alone decides
//! what is kept ([`crate::state::Effect::LoadSound`] and `DropSound`). With `MELOET_MEASURE_DIR`
//! set, every read is timed on stderr.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use meloet_model::BuiltinInstrument;
use meloet_sound::sampler::{LoadError, SampleBank, load_instrument, read_pack};

use crate::instruments::Samples;

pub mod catalog;
pub mod fetch;
#[cfg(test)]
mod fetch_tests;
mod untar;

/// A pack in one folder (what `tools/sounds/build.py` writes) that is used in place of the
/// fetched one: `$MELOET_SOUNDS_DIR` (development and tests), else `sounds/` beside the
/// program when it has a `pack.json` (`target/*/sounds` of a development build).
pub fn pinned_dir() -> Option<PathBuf> {
    pinned_from(pinned_env(), std::env::current_exe().ok())
}

#[cfg(feature = "dev-hooks")]
fn pinned_env() -> Option<PathBuf> {
    std::env::var_os("MELOET_SOUNDS_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(not(feature = "dev-hooks"))]
fn pinned_env() -> Option<PathBuf> {
    None
}

fn pinned_from(from_env: Option<PathBuf>, program: Option<PathBuf>) -> Option<PathBuf> {
    from_env.or_else(|| {
        let beside = program?.parent()?.join("sounds");
        beside.join("pack.json").is_file().then_some(beside)
    })
}

/// Whether `$MELOET_SOUNDS_DIR` names the pack: then nothing is fetched, so that a test never
/// meets the pack of the person at this PC.
pub fn pinned_by_env() -> bool {
    pinned_env().is_some()
}

/// The `instrument.json` of each instrument with samples that the pack in `dir` has. A folder
/// that does not read has none.
pub fn read_pinned(dir: &Path) -> Vec<(BuiltinInstrument, PathBuf)> {
    match read_pack(dir) {
        Ok(pack) => (pack.instruments.into_iter())
            .filter_map(|(name, json)| Some((BuiltinInstrument::from_name(&name)?, json)))
            .filter(|(instrument, _)| crate::instruments::sampled(*instrument))
            .collect(),
        Err(e) => {
            eprintln!("meloet: no sound pack: {e}");
            Vec::new()
        }
    }
}

/// How the pack is fetched (docs/architecture.md 23-9).
#[derive(Debug, Clone)]
pub struct PackOptions {
    /// `$XDG_DATA_HOME/meloet/sounds`.
    pub root: PathBuf,
    /// The catalog to start from when none newer was kept.
    pub built_in: String,
    pub trust: catalog::Trust,
    pub online: String,
    pub https_only: bool,
}

impl PackOptions {
    /// `None` without a home folder, or when `$MELOET_SOUNDS_DIR` names the pack.
    /// `MELOET_PACK_ORIGIN` (a test server, `http://127.0.0.1:8000/`) takes the place of the
    /// release and of the online catalog (`<origin>catalog.json`); `MELOET_PACK_CATALOG` names
    /// a catalog file that takes the place of the built-in one.
    pub fn from_env() -> Option<PackOptions> {
        if pinned_by_env() {
            return None;
        }
        let options = PackOptions {
            root: fetch::data_root(|name| std::env::var_os(name))?,
            built_in: catalog::BUILT_IN.to_owned(),
            trust: catalog::Trust::default(),
            online: catalog::ONLINE.to_owned(),
            https_only: true,
        };
        #[cfg(feature = "dev-hooks")]
        let options = options.with_test_places();
        Some(options)
    }

    #[cfg(feature = "dev-hooks")]
    fn with_test_places(mut self) -> PackOptions {
        let options = &mut self;
        let set = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        if let Some(origin) = set("MELOET_PACK_ORIGIN") {
            options.online = format!("{origin}catalog.json");
            options.https_only = !origin.starts_with("http://");
            options.trust.urls_under = origin;
        }
        if let Some(file) = set("MELOET_PACK_CATALOG") {
            match std::fs::read_to_string(&file) {
                Ok(text) => options.built_in = text,
                Err(e) => eprintln!("meloet: MELOET_PACK_CATALOG {file}: {e}"),
            }
        }
        self
    }
}

/// Called on the reading thread with what it read.
pub(crate) type Done = Box<dyn Fn(BuiltinInstrument, Result<Arc<SampleBank>, LoadError>) + Send>;

#[derive(Default)]
struct Queue {
    waiting: Mutex<VecDeque<(BuiltinInstrument, PathBuf)>>,
    wake: Condvar,
    stop: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The instruments of the pack and the samples that are in memory.
pub(crate) struct Library {
    /// The `instrument.json` of each instrument the pack has and this meloet knows.
    files: Vec<(BuiltinInstrument, PathBuf)>,
    banks: Vec<(BuiltinInstrument, Arc<SampleBank>)>,
    queue: Arc<Queue>,
    /// Until the reading thread is started, at the first read.
    done: Option<Done>,
    reader: Option<JoinHandle<()>>,
    /// A thread gives the memory of freed samples back to the PC.
    giving_back: Arc<AtomicBool>,
}

/// Samples freed (of every instrument and SF2 preset, wherever the last holder dropped them)
/// before their memory is given back to the PC.
const GIVE_BACK_AFTER: usize = 4 << 20;

impl Library {
    /// `files`: the `instrument.json` of each instrument with samples on this PC.
    pub fn open(files: Vec<(BuiltinInstrument, PathBuf)>, done: Done) -> Library {
        Library {
            files,
            banks: Vec::new(),
            queue: Arc::new(Queue::default()),
            done: Some(done),
            reader: None,
            giving_back: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Gives the memory of freed samples back to the PC once enough was freed, on a thread of
    /// its own (docs/architecture.md 23-12). Called often.
    pub fn give_back_freed(&self) {
        if meloet_sound::memory::freed_bytes() < GIVE_BACK_AFTER
            || self.giving_back.swap(true, Relaxed)
        {
            return;
        }
        let busy = self.giving_back.clone();
        let spawned = std::thread::Builder::new()
            .name("meloet-give-back".into())
            .spawn(move || {
                #[cfg(feature = "dev-hooks")]
                let (began, mib) = (
                    std::time::Instant::now(),
                    meloet_sound::memory::freed_bytes() >> 20,
                );
                meloet_sound::memory::give_back();
                #[cfg(feature = "dev-hooks")]
                if crate::measure::dir_from_env().is_some() {
                    let ms = began.elapsed().as_secs_f64() * 1000.0;
                    eprintln!("meloet: gave back the memory of {mib} MiB of samples in {ms:.1} ms");
                }
                busy.store(false, Relaxed);
            });
        if spawned.is_err() {
            self.giving_back.store(false, Relaxed);
        }
    }

    /// An instrument came, or a newer version of it: it is read from `json` from now on.
    pub fn add(&mut self, instrument: BuiltinInstrument, json: PathBuf) {
        self.files.retain(|(i, _)| *i != instrument);
        self.files.push((instrument, json));
    }

    /// The instruments whose samples the pack has.
    pub fn installed(&self) -> Vec<BuiltinInstrument> {
        self.files
            .iter()
            .map(|(instrument, _)| *instrument)
            .collect()
    }

    fn file(&self, instrument: BuiltinInstrument) -> Option<&PathBuf> {
        let found = self.files.iter().find(|(i, _)| *i == instrument);
        found.map(|(_, json)| json)
    }

    /// Asks the reading thread for the samples; the answer comes through `done`.
    pub fn load(&mut self, instrument: BuiltinInstrument) {
        let Some(json) = self.file(instrument).cloned() else {
            return;
        };
        if let Some(done) = self.done.take() {
            let queue = self.queue.clone();
            self.reader = Some(
                std::thread::Builder::new()
                    .name("meloet-sounds".into())
                    .spawn(move || read_loop(&queue, &done))
                    .expect("failed to start the thread that reads the sounds"),
            );
        }
        lock(&self.queue.waiting).push_back((instrument, json));
        self.queue.wake.notify_one();
    }

    /// Lets go of the samples, and of the wish to read them. Instruments that sound with them
    /// keep them until they are dropped themselves.
    pub fn let_go(&mut self, instrument: BuiltinInstrument) {
        lock(&self.queue.waiting).retain(|(i, _)| *i != instrument);
        self.banks.retain(|(i, _)| *i != instrument);
    }

    /// What the reading thread read. The first one read stays.
    pub fn keep(&mut self, instrument: BuiltinInstrument, bank: Arc<SampleBank>) {
        if !self.banks.iter().any(|(i, _)| *i == instrument) {
            self.banks.push((instrument, bank));
        }
    }

    /// The samples do not read: from now on the pack counts as not having the instrument.
    pub fn unreadable(&mut self, instrument: BuiltinInstrument) {
        self.let_go(instrument);
        self.files.retain(|(i, _)| *i != instrument);
    }

    pub fn samples(&self, instrument: BuiltinInstrument) -> Samples {
        match self.banks.iter().find(|(i, _)| *i == instrument) {
            Some((_, bank)) => Samples::Ready(bank.clone()),
            None if self.file(instrument).is_some() => Samples::NotRead,
            None => Samples::NotInstalled,
        }
    }

    /// Each instrument in memory: its sample bytes and how many hold them (this one too).
    #[cfg(feature = "dev-hooks")]
    pub fn held(&self) -> Vec<(BuiltinInstrument, usize, usize)> {
        (self.banks.iter())
            .map(|(i, bank)| (*i, bank.sample_bytes(), Arc::strong_count(bank)))
            .collect()
    }

    #[cfg(test)]
    pub fn in_memory(&self) -> Vec<BuiltinInstrument> {
        self.banks
            .iter()
            .map(|(instrument, _)| *instrument)
            .collect()
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        self.queue.stop.store(true, Relaxed);
        lock(&self.queue.waiting).clear();
        self.queue.wake.notify_all();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

/// Decoding threads for one instrument: enough to be quick, few enough to leave the PC to the
/// person who is playing.
fn decode_threads() -> usize {
    let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
    (cores / 2).clamp(2, 4)
}

fn read_loop(queue: &Queue, done: &Done) {
    loop {
        let (instrument, json) = {
            let mut waiting = lock(&queue.waiting);
            loop {
                if queue.stop.load(Relaxed) {
                    return;
                }
                if let Some(next) = waiting.pop_front() {
                    break next;
                }
                waiting = queue.wake.wait(waiting).unwrap_or_else(|e| e.into_inner());
            }
        };
        #[cfg(feature = "dev-hooks")]
        let began = std::time::Instant::now();
        let read = load_instrument(&json, decode_threads());
        #[cfg(feature = "dev-hooks")]
        if crate::measure::dir_from_env().is_some() {
            let ms = began.elapsed().as_millis();
            let mib = read.as_ref().map_or(0, |bank| bank.sample_bytes() >> 20);
            eprintln!(
                "meloet: read the sound of {} in {ms} ms, {mib} MiB",
                instrument.name()
            );
        }
        done(instrument, read);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A pack in a new temporary folder: every instrument named gets one short tone, looped,
    /// for every key.
    pub(crate) fn tiny_pack(label: &str, instruments: &[BuiltinInstrument]) -> PathBuf {
        let dir = crate::settings::temp_dir(label);
        let mut listed = Vec::new();
        for instrument in instruments {
            let name = instrument.name();
            std::fs::create_dir_all(dir.join(name)).unwrap();
            std::fs::write(dir.join(name).join("tone.wav"), tone_wav()).unwrap();
            std::fs::write(
                dir.join(name).join("instrument.json"),
                format!(
                    "{{\"format\": \"meloet-instrument\", \"version\": 1, \"kind\": \"{name}\",\n \
                     \"regions\": [{{\"sample\": \"tone.wav\", \"keyLo\": 0, \"keyHi\": 127, \
                     \"root\": 69, \"loopStart\": 1000, \"loopEnd\": 4000}}]}}\n"
                ),
            )
            .unwrap();
            listed.push(format!("\"{name}\": \"{name}/instrument.json\""));
        }
        std::fs::write(
            dir.join("pack.json"),
            format!(
                "{{\"format\": \"meloet-sounds\", \"version\": 1, \"pack\": \"test\",\n \
                 \"instruments\": {{{}}}}}\n",
                listed.join(", ")
            ),
        )
        .unwrap();
        dir
    }

    /// 0.1 s of 440 Hz, mono, 16 bit, 48 kHz.
    fn tone_wav() -> Vec<u8> {
        let frames = 4800u32;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + frames * 2).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(48_000u32.to_le_bytes());
        bytes.extend(96_000u32.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend((frames * 2).to_le_bytes());
        for i in 0..frames {
            let phase = std::f64::consts::TAU * 440.0 * f64::from(i) / 48_000.0;
            bytes.extend(((phase.sin() * 0.4 * 32767.0) as i16).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn the_pinned_pack_is_the_variable_or_the_one_beside_the_program() {
        let dir = tiny_pack("pinned", &[BuiltinInstrument::GrandPiano]);
        let base = crate::settings::temp_dir("pinned-program");
        let program = Some(base.join("meloet"));
        assert_eq!(pinned_from(None, program.clone()), None);
        let beside = base.join("sounds");
        std::fs::rename(&dir, &beside).unwrap();
        assert_eq!(pinned_from(None, program.clone()), Some(beside.clone()));
        assert_eq!(
            pinned_from(Some("/elsewhere".into()), program),
            Some(PathBuf::from("/elsewhere")),
            "the variable wins"
        );
        let files = read_pinned(&beside);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, BuiltinInstrument::GrandPiano);
        assert!(read_pinned(&beside.join("none")).is_empty());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn the_library_reads_keeps_and_lets_go() {
        use BuiltinInstrument as B;
        let dir = tiny_pack("library", &[B::GrandPiano, B::Strings, B::SynthLead]);
        // A broken instrument, and a name this meloet does not know.
        std::fs::create_dir_all(dir.join("organ")).unwrap();
        std::fs::write(dir.join("organ/instrument.json"), "{").unwrap();
        let pack = std::fs::read_to_string(dir.join("pack.json")).unwrap();
        let pack = pack.replace(
            "\"instruments\": {",
            "\"instruments\": {\"organ\": \"organ/instrument.json\", \"kazoo\": \"kazoo/i.json\", ",
        );
        std::fs::write(dir.join("pack.json"), pack).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        let done: Done = Box::new(move |instrument, read| {
            let _ = tx.send((instrument, read));
        });
        let mut library = Library::open(read_pinned(&dir), done);
        let mut installed = library.installed();
        installed.sort_by_key(|i| i.name());
        assert_eq!(
            installed,
            [B::GrandPiano, B::Organ, B::Strings],
            "a computed instrument needs no samples, whatever the pack says"
        );
        assert!(matches!(library.samples(B::GrandPiano), Samples::NotRead));
        assert!(matches!(
            library.samples(B::FingerBass),
            Samples::NotInstalled
        ));

        library.load(B::GrandPiano);
        library.load(B::Organ);
        let wait = std::time::Duration::from_secs(10);
        let (instrument, read) = rx.recv_timeout(wait).unwrap();
        assert_eq!(instrument, B::GrandPiano);
        let bank = read.unwrap();
        assert_eq!(bank.kind(), "grandPiano");
        library.keep(B::GrandPiano, bank.clone());
        assert!(
            matches!(library.samples(B::GrandPiano), Samples::Ready(b) if Arc::ptr_eq(&b, &bank))
        );
        assert_eq!(library.in_memory(), [B::GrandPiano]);

        let (instrument, read) = rx.recv_timeout(wait).unwrap();
        assert_eq!(instrument, B::Organ);
        assert!(read.is_err());
        library.unreadable(B::Organ);
        assert!(matches!(library.samples(B::Organ), Samples::NotInstalled));
        assert!(!library.installed().contains(&B::Organ));

        assert_eq!(Arc::strong_count(&bank), 2);
        library.let_go(B::GrandPiano);
        assert_eq!(Arc::strong_count(&bank), 1, "the library let go of it");
        assert!(matches!(library.samples(B::GrandPiano), Samples::NotRead));

        drop(library);
        assert!(rx.recv().is_err(), "the reading thread has ended");
        // No pack: nothing is installed and no thread runs.
        let none = Library::open(Vec::new(), Box::new(|_, _| {}));
        assert!(none.installed().is_empty() && none.reader.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
