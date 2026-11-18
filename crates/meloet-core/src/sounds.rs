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
use std::time::Instant;

use meloet_model::BuiltinInstrument;
use meloet_sound::sampler::{LoadError, SampleBank, load_instrument, read_pack};

use crate::instruments::Samples;

/// The pack `scripts/fetch-sounds.sh` fetches for this build.
const PACK_MANIFEST: &str = include_str!("../../../tools/sounds/pack.json");

/// The folders a pack may be in, the first to look in first: `$MELOET_SOUNDS_DIR` and nothing
/// else when it is set (development and tests), `sounds/` beside the program, and the folder
/// `scripts/fetch-sounds.sh` unpacks into.
fn candidates(
    from_env: Option<PathBuf>,
    program: Option<PathBuf>,
    cache: Option<PathBuf>,
    pack_version: &str,
) -> Vec<PathBuf> {
    if let Some(dir) = from_env {
        return vec![dir];
    }
    let beside = program.and_then(|p| Some(p.parent()?.join("sounds")));
    let fetched = cache
        .filter(|_| !pack_version.is_empty())
        .map(|c| c.join("meloet/sounds").join(pack_version));
    beside.into_iter().chain(fetched).collect()
}

fn pack_version() -> String {
    serde_json::from_str::<serde_json::Value>(PACK_MANIFEST)
        .ok()
        .and_then(|manifest| Some(manifest.get("version")?.as_str()?.to_owned()))
        .unwrap_or_default()
}

/// The first folder with a pack in it.
pub fn find_dir() -> Option<PathBuf> {
    let from_env = std::env::var_os("MELOET_SOUNDS_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    candidates(
        from_env,
        std::env::current_exe().ok(),
        dirs::cache_dir(),
        &pack_version(),
    )
    .into_iter()
    .find(|dir| dir.join("pack.json").is_file())
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
    reader: Option<JoinHandle<()>>,
}

impl Library {
    /// `dir`: the pack's folder. Without one, or with one that does not read, no instrument
    /// has samples.
    pub fn open(dir: Option<&Path>, done: Done) -> Library {
        let files = match dir.map(read_pack) {
            Some(Ok(pack)) => pack
                .instruments
                .into_iter()
                .filter_map(|(name, json)| Some((BuiltinInstrument::from_name(&name)?, json)))
                .filter(|(instrument, _)| crate::instruments::sampled(*instrument))
                .collect(),
            Some(Err(e)) => {
                eprintln!("meloet: no sound pack: {e}");
                Vec::new()
            }
            None => Vec::new(),
        };
        let queue = Arc::new(Queue::default());
        let reader = (!files.is_empty()).then(|| {
            let queue = queue.clone();
            std::thread::Builder::new()
                .name("meloet-sounds".into())
                .spawn(move || read_loop(&queue, &done))
                .expect("failed to start the thread that reads the sounds")
        });
        Library {
            files,
            banks: Vec::new(),
            queue,
            reader,
        }
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
    pub fn load(&self, instrument: BuiltinInstrument) {
        if let Some(json) = self.file(instrument) {
            lock(&self.queue.waiting).push_back((instrument, json.clone()));
            self.queue.wake.notify_one();
        }
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
        let began = Instant::now();
        let read = load_instrument(&json, decode_threads());
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
    fn the_pack_is_looked_for_in_the_order_of_the_design() {
        let program = Some(PathBuf::from("/opt/meloet/bin/meloet"));
        let cache = Some(PathBuf::from("/home/a/.cache"));
        assert_eq!(
            candidates(None, program.clone(), cache.clone(), "2026.11.0"),
            [
                PathBuf::from("/opt/meloet/bin/sounds"),
                PathBuf::from("/home/a/.cache/meloet/sounds/2026.11.0"),
            ]
        );
        // A build with no pack version looks beside the program only.
        assert_eq!(
            candidates(None, program.clone(), cache.clone(), ""),
            [PathBuf::from("/opt/meloet/bin/sounds")]
        );
        // The variable wins, and nothing else is looked at: a test never meets the pack of
        // the person at this PC.
        assert_eq!(
            candidates(Some("/tmp/pack".into()), program, cache, "2026.11.0"),
            [PathBuf::from("/tmp/pack")]
        );
        // The manifest of the repository reads; its version may still be empty.
        let _ = pack_version();
        assert!(serde_json::from_str::<serde_json::Value>(PACK_MANIFEST).is_ok());
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
        let mut library = Library::open(Some(&dir), done);
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
        let none = Library::open(None, Box::new(|_, _| {}));
        assert!(none.installed().is_empty() && none.reader.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
