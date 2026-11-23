//! The moving part: the threads around the pure rules.
//!
//! Only the core thread changes the state. The MIDI thread, the device watcher and the
//! telemetry thread send it what they learn over one channel. The core thread owns the MIDI
//! input and the device watcher, and starts them again when they stop by themselves.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU8, AtomicU16, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_devices::{DeviceWatcher, DevicesError, DevicesEvent};
use meloet_engine::{
    AudioOutput, Chord, Engine, EngineCommand, EngineConfig, EngineHandle, EngineMeters,
    Instrument, InstrumentEvent, LatencyRecord, MidiFeed, MidiIn, OutputTarget, Recorded,
    RunningOutput, Sequence, SequenceEvent, TrackSet, TransportClock, monotonic_us, stream_ring,
};
use meloet_midi::{MidiEvent, MidiInput, MidiMessage, MidiOptions, MidiSink};
use meloet_model::TempoMap;
use meloet_model::mixer::set_plugin_state;
use meloet_model::playback::{PlayEvent, PlayKind};
use meloet_model::record::{HeldControls, RecEventKind};
use meloet_model::{
    BuiltinInstrument, ControlValue, InstrumentGroup, InstrumentRef, PluginChange, PluginPlace,
    Song,
};
use meloet_plugin::wire::Notice;
use meloet_plugin::{PluginKey, ScanCache};
use meloet_protocol::{
    Command, Event, HistoryRow, LiveNotesSnapshot, ParamTable, PluginWindowKind, Snapshot,
    Telemetry, TransportReading, TransportState,
};
use meloet_sound::phrases::{self, PhrasePlayer};
use meloet_sound::sampler::{LoadError, SampleBank};

use crate::ai::{
    AiAnswer, AiAsk, AiLevels, AiMeasure, AiRefusal, AiRender, AiRendered, AiSong, AiSwitch,
};
use crate::audiofiles::keeper::AudioNews;
use crate::audiofiles::{AudioStore, Settling};
use crate::export::{ExportAsk, ExportNews};
use crate::hub::{Hub, MessageSink, TelemetrySink};
use crate::instruments;
use crate::made::{MadeKey, MadePlan, only_audio_differs, seat_audio};
use crate::maker::{MadeNews, Maker};
#[cfg(feature = "dev-hooks")]
use crate::measure::{self, KeyToSound, MemoryLog, PluginLog};
use crate::mixbook::{self, MixBook};
use crate::pcm::{FeedClip, PcmStore};
use crate::play::{
    AUDITION_SEAT, EngineOp, Heard, MAX_TRACKS, PlugState, RecordReport, Spot, VoiceSpec,
};
use crate::plugins::hosts::{
    DEFAULT_GEOMETRY, EditorKind, HostError, HostNews, HostOptions, Hosts, LoadSpec, Loaded,
    ParamNow, PresetAsk, PresetSaid, SlotId, StartState,
};
use crate::plugins::housed::{HousedEffect, HousedInstrument, Which};
use crate::plugins::scanner::Scanner;
use crate::plugins::{PluginOptions, ScanJob, ScanNews};
use crate::readahead::ReadAhead;
use crate::recovery;
use crate::settings::{self, Settings};
use crate::sf2store::{self, Sf2Done, Sf2Store};
use crate::songfile;
use crate::sounds::fetch::{Fetcher, PackNews};
use crate::sounds::{self, Library};
use crate::state::{
    Effect, Input, Model, PackAsk, PackStart, PluginAsk, PluginSaid, RecoveryStart, SaveTo,
    retry_delay,
};

const TELEMETRY_TICK: Duration = Duration::from_micros(16_667);
/// How often at most the status line is told that the microphone is too loud.
const MIC_LOUD_EVERY: Duration = Duration::from_secs(3);
/// How long scanning waits for the screen at most (`PluginOptions::wait_for_screen`).
const SCAN_WAITS_FOR_SCREEN: Duration = Duration::from_secs(10);
/// How often the core thread reads the engine's reports while the transport moves.
const MOVING_TICK: Duration = Duration::from_millis(15);
/// How often the core thread looks at the engine's rings when no message wakes it.
const HOUSEKEEPING: Duration = Duration::from_millis(100);
/// -60 dBFS.
const SOUND_THRESHOLD: f32 = 0.001;
const LOAD_STEP: u16 = 10;
/// A key-to-sound time above this is a key that waited for a stream, not a measurement.
const LATENCY_SANE_US: u64 = 1_000_000;
/// How long a save waits for the plugins of the song to say their states. One that has not
/// answered by then is saved with the last state the song knows of it.
const STATE_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct CoreOptions {
    /// Where `settings.json` lives. `None`: nothing is read or saved.
    pub config_dir: Option<PathBuf>,
    /// Where `key-to-sound.json` goes. `None`: nothing is recorded.
    #[cfg(feature = "dev-hooks")]
    pub measure_dir: Option<PathBuf>,
    /// `None`: no keyboard input.
    pub midi: Option<MidiOptions>,
    /// Watch the outputs and inputs (PipeWire).
    pub devices: bool,
    /// Allow opening an output stream.
    pub audio: bool,
    /// Where a song's first save makes its folder. `None`: the first save fails.
    pub songs_dir: Option<PathBuf>,
    /// Where the recovery file lives. `None`: none is read or written.
    pub state_dir: Option<PathBuf>,
    /// A pack in one folder used in place of the fetched one (`sounds::pinned_dir`). `None`
    /// and no `pack`: only the computed instruments sound.
    pub sounds_dir: Option<PathBuf>,
    /// Where the sound pack is fetched to, and from where. `None`: nothing is fetched.
    pub pack: Option<sounds::PackOptions>,
    /// `None`: no plugins; nothing is scanned and no process is started.
    pub plugins: Option<PluginOptions>,
    /// Where added SF2 files are copied to. `None`: none is listed, and none can be added.
    pub sf2_dir: Option<PathBuf>,
    /// Where what can be made again is kept (`$XDG_CACHE_HOME/meloet`): the peaks of the
    /// audio files in `peaks/`, the decoded and resampled audio in `audio/`. `None`:
    /// nothing is kept; only plain wav files at the rate of the stream sound, and no
    /// waveform is drawn.
    pub cache_dir: Option<PathBuf>,
}

/// The pack as the start found it, and the thread that fetches it.
struct PackStarted {
    /// The instruments fetched before.
    found: Vec<(BuiltinInstrument, PathBuf)>,
    start: PackStart,
    fetcher: Fetcher,
}

impl PackStarted {
    /// `pinned`: what a pack in one folder has; those instruments are not fetched.
    fn look(
        options: &sounds::PackOptions,
        pinned: &[(BuiltinInstrument, PathBuf)],
        state_dir: Option<PathBuf>,
        tx: &Sender<Msg>,
    ) -> Option<PackStarted> {
        let catalog =
            sounds::fetch::start_catalog(&options.root, &options.built_in, &options.trust)?;
        let mut covered: Vec<String> = pinned.iter().map(|(b, _)| b.name().to_owned()).collect();
        if !pinned.is_empty() {
            covered.push(sounds::catalog::COMMON_ID.to_owned());
        }
        let disk = sounds::fetch::Disk {
            root: options.root.clone(),
            covered,
        };
        let start = PackStart {
            instruments: catalog.instruments().collect(),
            outlook: disk.outlook(&catalog),
            pack_bytes: catalog.bytes(),
            place: sounds::fetch::shown(&options.root, dirs::home_dir().as_deref()),
        };
        let found = disk.instruments(&catalog);
        let mut fetch = sounds::fetch::Options::new(disk, state_dir);
        fetch.trust = options.trust.clone();
        fetch.online = options.online.clone();
        fetch.https_only = options.https_only;
        let tx = Mutex::new(tx.clone());
        let tell = move |news| {
            let _ = lock(&tx).send(Msg::Pack(news));
        };
        let fetcher = Fetcher::spawn(fetch, catalog, Box::new(tell))
            .inspect_err(|e| eprintln!("meloet: no thread to fetch the sound pack: {e}"))
            .ok()?;
        Some(PackStarted {
            found,
            start,
            fetcher,
        })
    }
}

/// What the core asks of the shell. Answer the windows with [`Core::song_file_picked`] and
/// [`Core::folder_picked`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellRequest {
    /// The PC's open window for a song file, starting in `folder`.
    PickSongFile { folder: Option<PathBuf> },
    /// The PC's folder window, starting in `folder`.
    PickFolder { folder: Option<PathBuf> },
    /// The PC's save window ("名前をつけて保存"), starting in `folder` with `name` filled in.
    /// Answer with [`Core::save_as_picked`].
    PickSaveAs {
        folder: Option<PathBuf>,
        name: String,
    },
    /// End meloet: everything that had to be asked or saved is done.
    Exit,
    /// Show this file in the PC's file manager.
    ShowFile { path: PathBuf },
    /// The PC's folder window for a place to look for plugins in, starting in `folder`.
    /// Answer with [`Core::plugin_place_picked`].
    PickPluginPlace { folder: Option<PathBuf> },
    /// The PC's open window for an SF2 file, starting in `folder`. Answer with
    /// [`Core::sf2_picked`].
    PickSf2 { folder: Option<PathBuf> },
    /// The PC's open window for audio files (wav, mp3, flac, ogg), starting in `folder`.
    /// `many`: more than one may be chosen. Answer with [`Core::audio_picked`].
    PickAudio { folder: Option<PathBuf>, many: bool },
    /// Where an export goes: the PC's save window, starting in `folder` with `name` filled
    /// in (what is there is written over, so the window asks), or with `folder_only` the
    /// folder window. Answer with [`Core::export_place_picked`].
    PickExportPlace {
        folder: Option<PathBuf>,
        name: String,
        folder_only: bool,
    },
    /// Open this address (`https://` only) in the PC's browser (round 12, G1 and D1).
    OpenUrl { url: String },
    /// Open this file with the PC's own app for it (round 12, E9 「PC のプレーヤーで聞く」).
    OpenFile { path: PathBuf },
}

/// Called on the core thread; must not wait for the main thread.
pub type ShellSink = Box<dyn Fn(ShellRequest) + Send>;

impl CoreOptions {
    /// Everything on, with the directories and the MIDI options the environment gives.
    pub fn from_env() -> CoreOptions {
        CoreOptions {
            config_dir: settings::config_dir(),
            #[cfg(feature = "dev-hooks")]
            measure_dir: measure::dir_from_env(),
            midi: Some(MidiOptions::from_env()),
            devices: true,
            audio: true,
            songs_dir: songfile::songs_dir(),
            state_dir: recovery::dir(),
            sounds_dir: sounds::pinned_dir(),
            pack: sounds::PackOptions::from_env(),
            plugins: PluginOptions::from_env(),
            sf2_dir: sf2store::dir(),
            cache_dir: dirs::cache_dir().map(|d| d.join("meloet")),
        }
    }

    /// No device and no file: only the rules and the hub.
    pub fn offline() -> CoreOptions {
        CoreOptions {
            config_dir: None,
            #[cfg(feature = "dev-hooks")]
            measure_dir: None,
            midi: None,
            devices: false,
            audio: false,
            songs_dir: None,
            state_dir: None,
            sounds_dir: None,
            pack: None,
            plugins: None,
            sf2_dir: None,
            cache_dir: None,
        }
    }
}

enum Msg {
    /// The reply gets the rev after the input was handled.
    Input(Input, Option<Sender<u64>>),
    /// From the telemetry thread: frames per block of the stream of `epoch`.
    Frames {
        epoch: u64,
        actual: Option<u32>,
    },
    /// From the telemetry thread: the stream of `epoch` ended by itself.
    StreamLost {
        epoch: u64,
    },
    /// From the telemetry thread: xruns of the stream of `epoch` since it opened.
    Xruns {
        epoch: u64,
        count: u64,
    },
    /// The MIDI thread moved a keyboard's fader; the newest value is in `Shared::fader`.
    Fader,
    /// The MIDI thread ended by itself.
    MidiStopped {
        reason: String,
    },
    /// The device watcher lost PipeWire and ended.
    DevicesLost,
    LiveNotes(Sender<LiveNotesSnapshot>),
    TakeNotes(u32, Sender<Option<Vec<u8>>>),
    Lane(u32, Sender<Option<Vec<u8>>>),
    MadeCounts(Sender<MadeCounts>),
    PeaksPlace(u32, Sender<Option<crate::peaks::PeaksPlace>>),
    PcmStore(Arc<dyn PcmStore>),
    HistoryRows(u32, u32, Sender<Vec<HistoryRow>>),
    PluginParams(u32, Sender<Option<ParamTable>>),
    /// Told once the recovery file holds the song, or there was nothing to write.
    PcEnding(Sender<()>),
    /// From the thread that reads the sounds: the answer to [`Effect::LoadSound`].
    SoundRead {
        instrument: BuiltinInstrument,
        read: Result<Arc<SampleBank>, LoadError>,
    },
    /// From the thread that makes the audio of stretched clips.
    Made(MadeNews),
    /// From the thread that fetches the sound pack.
    Pack(PackNews),
    /// From the scanner's threads.
    Scan(ScanNews),
    /// The screen subscribed: its page is up.
    ScreenReady,
    /// From the thread of the plugin processes.
    Plugins(HostNews),
    /// From the thread of the plugin processes: the answer to [`Effect::LoadPlugin`] and to
    /// [`Effect::ReloadPlugin`].
    PluginLoaded {
        ticket: u64,
        loaded: Result<Loaded, HostError>,
    },
    /// From the thread that reads SF2 files.
    Sf2(Sf2Done),
    /// A look at the worker, for a test.
    #[cfg(test)]
    Peek(Box<dyn FnOnce(&mut Worker) + Send>),
    /// The AI link reads the song.
    AiSong(Sender<AiSong>),
    /// The AI link writes.
    Ai(AiAsk, Sender<AiAnswer>),
    /// The AI link measures: the job's id or why not, then its news on `news`.
    AiMeasure {
        ask: AiMeasure,
        news: Sender<ExportNews>,
        reply: Sender<Result<AiJob, AiRefusal>>,
    },
    /// The AI link renders into files: the job's id or why not, then its news on `news`.
    AiRender {
        ask: AiRender,
        news: Sender<ExportNews>,
        reply: Sender<Result<AiJob, AiRefusal>>,
    },
    /// The AI link gives a measurement or a rendering up.
    AiDrop(u64),
    AiSwitch(AiSwitch),
    Stop,
}

/// What the MIDI thread writes and the telemetry thread reads.
#[derive(Default)]
struct Held {
    keys_low: AtomicU64,
    keys_high: AtomicU64,
    last_velocity: AtomicU8,
    sustain: AtomicU8,
    modulation: AtomicU8,
    bend: AtomicI16,
}

impl Held {
    fn note(&self, event: MidiEvent) {
        let key = |note: u8| {
            let cell = if note < 64 {
                &self.keys_low
            } else {
                &self.keys_high
            };
            (cell, 1u64 << (note % 64))
        };
        match event {
            MidiEvent::NoteOn { note, velocity } => {
                let (cell, bit) = key(note.min(127));
                cell.fetch_or(bit, Relaxed);
                self.last_velocity.store(velocity, Relaxed);
            }
            MidiEvent::NoteOff { note, .. } => {
                let (cell, bit) = key(note.min(127));
                cell.fetch_and(!bit, Relaxed);
            }
            MidiEvent::Control {
                controller: 64,
                value,
            } => self.sustain.store(value, Relaxed),
            MidiEvent::Control {
                controller: 1,
                value,
            } => self.modulation.store(value, Relaxed),
            MidiEvent::Control { .. } => {}
            MidiEvent::PitchBend { value } => self.bend.store(value, Relaxed),
        }
    }

    fn release_all(&self) {
        self.keys_low.store(0, Relaxed);
        self.keys_high.store(0, Relaxed);
        self.sustain.store(0, Relaxed);
    }
}

/// What the core thread writes about the stream, for the MIDI and the telemetry thread.
#[derive(Default)]
struct StreamFacts {
    open: AtomicBool,
    /// Goes up at every open and close, so a late report about an older stream can be told.
    epoch: AtomicU64,
    /// `EngineMeters::callbacks` when the stream was opened.
    callbacks_at_open: AtomicU64,
    /// `EngineMeters::streams_lost` when the stream was opened.
    lost_at_open: AtomicU64,
    /// `EngineMeters::xruns` when the stream was opened.
    xruns_at_open: AtomicU64,
    /// The engine's track of each track of the song, in the order of the song: the meters go
    /// to the screen in that order. Only the core thread and the telemetry thread lock it.
    track_seats: Mutex<Vec<Option<usize>>>,
    /// The core thread wants one `SoundHeard`.
    wants_sound: AtomicBool,
    /// The core thread wants one `KeyWhileLoading`.
    wants_loading_key: AtomicBool,
}

struct Shared {
    held: Held,
    /// The velocity every NoteOn played takes; 0 while not fixed. The core thread writes it,
    /// the MIDI thread reads it.
    fixed_velocity: AtomicU8,
    stream: StreamFacts,
    stop: AtomicBool,
    /// The engine's clock; a new one when the engine is made anew.
    clock: Mutex<Arc<TransportClock>>,
    /// The tempo map the engine was last sent and its number, for the meter thread to read
    /// the clock with.
    tempo_map: Mutex<(Arc<TempoMap>, u32)>,
    shell: Mutex<Option<ShellSink>>,
    /// The newest value of a keyboard's fader that the core thread has not read yet, or
    /// [`NO_FADER`]. Many moves of the fader are one [`Msg::Fader`] while the core is busy.
    fader: AtomicU16,
    /// The open input's meters and the vocal tracks they are read for. The core thread
    /// writes it, the telemetry thread reads it.
    mics: Mutex<Option<input::MicFacts>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

struct Running {
    core: JoinHandle<()>,
    telemetry: JoinHandle<()>,
}

/// [`Core::made_counts`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MadeCounts {
    /// Audio that clips need and that is not made yet (`Maker::waiting`).
    pub waiting: usize,
    /// Made audio that clips of the song play now.
    pub playing: usize,
    /// Times made audio came in, or went, over what a track was sounding
    /// (`ReadStats::splices`).
    pub splices: u64,
}

pub struct Core {
    hub: Arc<Hub>,
    tx: Sender<Msg>,
    shared: Arc<Shared>,
    running: Mutex<Option<Running>>,
}

impl Core {
    /// Starts the threads. A part that cannot start (no ALSA sequencer, no PipeWire) is left
    /// out: it never shows up in the state, and the rest works.
    pub fn start(options: CoreOptions) -> Core {
        let settings = options
            .config_dir
            .as_deref()
            .map_or_else(Settings::default, settings::load);
        let (tx, rx) = mpsc::channel();
        let pinned = options
            .sounds_dir
            .as_deref()
            .map_or_else(Vec::new, sounds::read_pinned);
        let pack = options
            .pack
            .as_ref()
            .and_then(|pack| PackStarted::look(pack, &pinned, options.state_dir.clone(), &tx));
        let library = {
            let tx = tx.clone();
            let done = move |instrument, read| {
                let _ = tx.send(Msg::SoundRead { instrument, read });
            };
            let mut files = pinned;
            files.extend(pack.as_ref().map_or_else(Vec::new, |p| p.found.clone()));
            Library::open(files, Box::new(done))
        };
        let sf2 = {
            let tx = Mutex::new(tx.clone());
            let done = move |done| {
                let _ = lock(&tx).send(Msg::Sf2(done));
            };
            Sf2Store::open(options.sf2_dir.clone(), Box::new(done))
        };
        let wishes = (settings.sound.buffer, settings.sound.sample_rate);
        let mut model = Model::new(settings).with_sounds(library.installed());
        let mut fetcher = None;
        if let Some(pack) = pack {
            model = model.with_pack(pack.start);
            fetcher = Some(pack.fetcher);
        }
        let mut model = model
            .with_sf2(sf2.files().to_vec())
            .with_cache_dir(options.cache_dir.clone())
            .with_export_dirs(crate::export::ExportDirs {
                home: dirs::home_dir(),
                usual: dirs::audio_dir(),
            })
            .with_song_places(
                options.songs_dir.clone(),
                (options.state_dir.as_deref()).map_or_else(Vec::new, crate::recent::load),
            );
        let audio_store = Arc::new(
            crate::audiofiles::AudioStore::new(options.state_dir.clone())
                .with_cache(options.cache_dir.clone()),
        );
        let keeper = {
            let tx = tx.clone();
            let tell = move |news| {
                let _ = tx.send(Msg::Input(Input::Audio(news), None));
            };
            crate::audiofiles::keeper::Keeper::spawn(audio_store.clone(), tell)
                .inspect_err(|e| eprintln!("meloet: no thread for the audio files: {e}"))
                .ok()
        };
        let maker = {
            let tx = tx.clone();
            let tell = move |news| {
                let _ = tx.send(Msg::Made(news));
            };
            Maker::spawn(audio_store.clone(), tell)
                .inspect_err(|e| eprintln!("meloet: no thread to make audio for clips: {e}"))
                .ok()
        };
        // For the measurements that need a loop with no cable: the monitor of this sink is
        // the input.
        #[cfg(feature = "dev-hooks")]
        if let Some(sink) = std::env::var("MELOET_INPUT_SINK_MONITOR")
            .ok()
            .filter(|s| !s.is_empty())
        {
            model = model.with_input_stand_in(&sink);
        }
        model.set_uid_seed(recovery::now_ms() ^ u64::from(std::process::id()) << 40);
        if let Some(plugin_options) = &options.plugins {
            model = model.with_plugin_places(plugin_options.usual_places.clone());
        }
        let hub = Arc::new(Hub::new(model.view()));
        let parts = Engine::new(EngineConfig {
            sample_rate: 48_000,
            max_tracks: MAX_TRACKS,
            ..EngineConfig::default()
        });
        let shared = Arc::new(Shared {
            held: Held::default(),
            fixed_velocity: AtomicU8::new(model.fixed_velocity().unwrap_or(0)),
            stream: StreamFacts::default(),
            stop: AtomicBool::new(false),
            clock: Mutex::new(parts.handle.clock().clone()),
            tempo_map: Mutex::new(map_of(&parts.handle)),
            shell: Mutex::new(None),
            fader: AtomicU16::new(NO_FADER),
            mics: Mutex::new(None),
        });
        let meters = parts.handle.meters().clone();
        *lock(&shared.stream.track_seats) = model.track_seats();

        let links = Links {
            hub: hub.clone(),
            shared: shared.clone(),
            tx: tx.clone(),
        };
        let plugins = options.plugins.as_ref().map(|plugin_options| {
            let scan_tx = Mutex::new(tx.clone());
            let scanner = Scanner::start(plugin_options, move |news| {
                let _ = lock(&scan_tx).send(Msg::Scan(news));
            });
            let host_tx = tx.clone();
            let host_options = HostOptions {
                exe: plugin_options.host_exe.clone(),
                env: plugin_options.child_env.clone(),
                geometry: DEFAULT_GEOMETRY,
            };
            let hosts = Hosts::start(host_options.clone(), move |news| {
                let _ = host_tx.send(Msg::Plugins(news));
            });
            let dir = plugin_options.cache_dir.as_deref();
            let (gone_tx, gone_rx) = mpsc::channel();
            PluginSide {
                scanner,
                hosts,
                options: host_options,
                start: Some(Input::PluginsStart {
                    cache: Box::new(dir.map_or_else(ScanCache::default, ScanCache::load)),
                    first: !dir.is_some_and(ScanCache::is_saved),
                }),
                held: plugin_options.wait_for_screen.then(Vec::new),
                ready: HashMap::new(),
                ids: HashMap::new(),
                gone_tx,
                gone_rx,
            }
        });
        let midi = options.midi.and_then(|midi_options| {
            MidiInput::start_boxed(midi_options.clone(), links.midi_sink(parts.midi_in))
                .inspect_err(|(e, _)| eprintln!("meloet: no keyboard input: {e}"))
                .ok()
                .map(|input| Midi {
                    options: midi_options,
                    state: MidiState::Running(input),
                })
        });
        // Without PipeWire at start, it is tried again quietly on the core thread: nothing has
        // stopped, so nothing is said.
        let devices = options.devices.then(|| match links.device_watcher() {
            Ok(watcher) => DevicesState::Running { _watcher: watcher },
            Err(_) => DevicesState::Waiting {
                attempt: 1,
                at: Instant::now(),
            },
        });

        let core = {
            let worker = Worker {
                hub: hub.clone(),
                shared: shared.clone(),
                audio: Audio::new(
                    parts.engine,
                    parts.handle,
                    options.audio,
                    library,
                    sf2,
                    model.engine_seats(),
                    wishes,
                )
                .with_store(audio_store.clone())
                .with_maker(maker),
                model,
                input: {
                    #[cfg_attr(not(feature = "dev-hooks"), allow(unused_mut))]
                    let mut input = input::InputSide::new(options.state_dir.clone());
                    #[cfg(feature = "dev-hooks")]
                    {
                        input.measure_dir = options.measure_dir.clone();
                    }
                    input
                },
                audio_store,
                keeper,
                song_told: None,
                exports: export::Exports::new(options.state_dir.as_deref()),
                fetcher,
                config_dir: options.config_dir.clone(),
                songs_dir: options.songs_dir.clone(),
                state_dir: options.state_dir.clone(),
                recovery: options.state_dir.clone().map(recovery::Writer::start),
                ending: Vec::new(),
                #[cfg(feature = "dev-hooks")]
                key_to_sound: options.measure_dir.clone().map(KeyToSound::new),
                #[cfg(feature = "dev-hooks")]
                plugin_log: options.measure_dir.clone().map(PluginLog::new),
                #[cfg(feature = "dev-hooks")]
                memory_log: options.measure_dir.clone().map(MemoryLog::new),
                links,
                midi,
                devices,
                audio_retry_at: None,
                plugins,
                ai_switch: None,
                ai_jobs: 0,
                held_replies: VecDeque::new(),
            };
            std::thread::Builder::new()
                .name("meloet-core".into())
                .spawn(move || worker.run(rx))
                .expect("failed to start the core thread")
        };

        let telemetry = {
            let (hub, shared, tx) = (hub.clone(), shared.clone(), tx.clone());
            std::thread::Builder::new()
                .name("meloet-telemetry".into())
                .spawn(move || telemetry_loop(&hub, &shared, &meters, &tx))
                .expect("failed to start the telemetry thread")
        };

        Core {
            hub,
            tx,
            shared,
            running: Mutex::new(Some(Running { core, telemetry })),
        }
    }

    /// The whole state and its rev.
    pub fn snapshot(&self) -> Snapshot {
        self.hub.snapshot()
    }

    /// Applies an intent and waits until it is done. Returns the rev after it (unchanged when
    /// nothing changed).
    pub fn send(&self, command: Command) -> u64 {
        self.input(Input::Command(command))
    }

    /// What the screen the window is on can give to the inside of the window, in logical px.
    /// Only the shell knows it. Does not wait.
    pub fn set_screen_space(&self, width: f32, height: f32) {
        let input = Input::ScreenSpace { width, height };
        let _ = self.tx.send(Msg::Input(input, None));
    }

    /// Where requests for the shell go. Without one, the windows count as put away and
    /// nothing ends.
    pub fn set_shell(&self, shell: ShellSink) {
        *lock(&self.shared.shell) = Some(shell);
    }

    /// The window's close button. Does not wait: meloet ends through [`ShellRequest::Exit`],
    /// after asking about unsaved changes.
    pub fn close_requested(&self) {
        let _ = self.tx.send(Msg::Input(Input::CloseRequested, None));
    }

    /// The answer to [`ShellRequest::PickSongFile`]. Does not wait.
    pub fn song_file_picked(&self, path: Option<PathBuf>) {
        let _ = self.tx.send(Msg::Input(Input::SongFilePicked(path), None));
    }

    /// The answer to [`ShellRequest::PickFolder`]. Does not wait.
    pub fn folder_picked(&self, path: Option<PathBuf>) {
        let _ = self.tx.send(Msg::Input(Input::FolderPicked(path), None));
    }

    /// The answer to [`ShellRequest::PickPluginPlace`]. Does not wait.
    pub fn plugin_place_picked(&self, path: Option<PathBuf>) {
        let input = Input::PluginPlacePicked(path);
        let _ = self.tx.send(Msg::Input(input, None));
    }

    /// The answer to [`ShellRequest::PickSf2`]. Does not wait.
    pub fn sf2_picked(&self, path: Option<PathBuf>) {
        let _ = self.tx.send(Msg::Input(Input::Sf2Picked(path), None));
    }

    /// The answer to [`ShellRequest::PickAudio`]: the files chosen, none when the window
    /// was put away. Does not wait.
    pub fn audio_picked(&self, paths: Vec<PathBuf>) {
        let _ = self.tx.send(Msg::Input(Input::AudioPicked(paths), None));
    }

    /// Files are held over the window. Does not wait.
    pub fn files_over(&self, paths: Vec<PathBuf>) {
        let _ = self.tx.send(Msg::Input(Input::FilesOver(paths), None));
    }

    /// The files held over the window were taken out of it, or the drag was given up. Does
    /// not wait.
    pub fn files_left(&self) {
        let _ = self.tx.send(Msg::Input(Input::FilesLeft, None));
    }

    /// The files held over the window were let go of there. Does not wait.
    pub fn files_dropped(&self, paths: Vec<PathBuf>) {
        let _ = self.tx.send(Msg::Input(Input::FilesDropped(paths), None));
    }

    /// Where the pointer is while files are held over the window, in px of the page from
    /// the top left corner of the window's inside: after `files_over`, with every move, and
    /// before `files_dropped`. Does not wait.
    pub fn files_at(&self, x: i32, y: i32) {
        let _ = self.tx.send(Msg::Input(Input::FilesAt { x, y }, None));
    }

    /// A song file the PC handed over (a double click, `meloet <file>`): opened once the
    /// screen is up, asking first about unsaved changes as 「ひらく」 does. Does not wait.
    pub fn open_given(&self, path: PathBuf) {
        let _ = self.tx.send(Msg::Input(Input::OpenGiven(path), None));
    }

    /// The answer to [`ShellRequest::PickSaveAs`]: the folder and the name typed, as one path.
    /// Does not wait.
    pub fn save_as_picked(&self, path: Option<PathBuf>) {
        let _ = self.tx.send(Msg::Input(Input::SaveAsPicked(path), None));
    }

    /// The answer to [`ShellRequest::PickExportPlace`]. Does not wait.
    pub fn export_place_picked(&self, path: Option<PathBuf>) {
        let news = crate::export::ExportNews::PlacePicked(path);
        let _ = self.tx.send(Msg::Input(Input::Export(news), None));
    }

    /// From this call on, every patch reaches `messages` in rev order: a snapshot taken after
    /// `subscribe` returns misses nothing.
    pub fn subscribe(&self, messages: MessageSink, telemetry: TelemetrySink) {
        self.hub.subscribe(messages, telemetry);
        let _ = self.tx.send(Msg::ScreenReady);
    }

    /// Stops every thread, closes the stream, and writes what is left to write. Dropping the
    /// core does the same.
    pub fn stop(&self) {
        let running = self
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let Some(running) = running else {
            return;
        };
        let _ = self.tx.send(Msg::Stop);
        let _ = running.core.join();
        self.shared.stop.store(true, Relaxed);
        let _ = running.telemetry.join();
    }

    /// Every note of the recording going on, as of the last `liveNotes` message.
    pub fn live_notes(&self) -> LiveNotesSnapshot {
        let (reply, answer) = mpsc::channel();
        if self.tx.send(Msg::LiveNotes(reply)).is_err() {
            return LiveNotesSnapshot {
                seq: 0,
                recording: None,
                notes: Vec::new(),
            };
        }
        answer.recv().unwrap_or(LiveNotesSnapshot {
            seq: 0,
            recording: None,
            notes: Vec::new(),
        })
    }

    /// The PC is ending: writes the recovery file when there are unsaved changes (a recording
    /// is stopped and put in the song first). Returns whether that was done within `timeout`.
    pub fn pc_ending(&self, timeout: Duration) -> bool {
        let (reply, done) = mpsc::channel();
        if self.tx.send(Msg::PcEnding(reply)).is_err() {
            return false;
        }
        done.recv_timeout(timeout).is_ok()
    }

    /// The take binary (docs/architecture.md 3-4) of a take in the song as it is now.
    pub fn take_notes(&self, id: u32) -> Option<Vec<u8>> {
        let (reply, answer) = mpsc::channel();
        self.tx.send(Msg::TakeNotes(id, reply)).ok()?;
        answer.recv().ok().flatten()
    }

    /// The peaks binary (`meloet_protocol::Peaks`) of an audio file of the song as it is
    /// now: the peaks `first .. first + count` of `level` that there are. `None` for an
    /// `audio` that is no file's, and while its peaks are not made. The core thread only
    /// says where the peaks are; they are read on the caller's thread.
    pub fn peaks(&self, audio: u32, level: u8, first: u32, count: u32) -> Option<Vec<u8>> {
        let (reply, answer) = mpsc::channel();
        self.tx.send(Msg::PeaksPlace(audio, reply)).ok()?;
        answer.recv().ok()??.reply(audio, level, first, count)
    }

    /// For a test that brings its own audio files: the read-ahead thread opens the vocal
    /// clips through `store` from now on, not through the song's own places.
    pub fn set_pcm_store(&self, store: Arc<dyn PcmStore>) {
        let _ = self.tx.send(Msg::PcmStore(store));
    }

    /// For tests and an export that waits: what the making of audio for clips is at.
    pub fn made_counts(&self) -> MadeCounts {
        let (reply, answer) = mpsc::channel();
        let _ = self.tx.send(Msg::MadeCounts(reply));
        answer.recv().unwrap_or_default()
    }

    /// The points of a lane (`meloet_protocol::LanePoints`) of the song as it is now; `None`
    /// for a `lane_id` that is no lane's.
    pub fn lane(&self, lane_id: u32) -> Option<Vec<u8>> {
        let (reply, answer) = mpsc::channel();
        self.tx.send(Msg::Lane(lane_id, reply)).ok()?;
        answer.recv().ok().flatten()
    }

    /// The rows `from ..= to` of the history band that there are (rows count from 1).
    pub fn history_rows(&self, from: u32, to: u32) -> Vec<HistoryRow> {
        let (reply, answer) = mpsc::channel();
        if self.tx.send(Msg::HistoryRows(from, to, reply)).is_err() {
            return Vec::new();
        }
        answer.recv().unwrap_or_default()
    }

    /// The parameters of the plugin the parameters panel shows, when `table` is still
    /// `SelectedPlugin::table`: the screen has another table by now otherwise, and fetches
    /// that one.
    pub fn plugin_params(&self, table: u32) -> Option<ParamTable> {
        let (reply, answer) = mpsc::channel();
        self.tx.send(Msg::PluginParams(table, reply)).ok()?;
        answer.recv().ok().flatten()
    }

    /// Where the switch 「AI とつなぐ」 goes: at once with how it is, and again with every
    /// change. Does not wait.
    pub fn set_ai_switch(&self, switch: AiSwitch) {
        let _ = self.tx.send(Msg::AiSwitch(switch));
    }

    /// What the AI link's socket is doing. Does not wait.
    pub fn ai_port(&self, port: meloet_protocol::AiPort) {
        let _ = self.tx.send(Msg::Input(Input::AiPort(port), None));
    }

    /// The song as it is now, for the AI link. `None` once the core has ended.
    pub fn ai_song(&self) -> Option<AiSong> {
        let (reply, answer) = mpsc::channel();
        self.tx.send(Msg::AiSong(reply)).ok()?;
        answer.recv().ok()
    }

    /// A write of the AI link, as one step. Waits until it is done.
    pub fn ai(&self, ask: AiAsk) -> AiAnswer {
        let (reply, answer) = mpsc::channel();
        if self.tx.send(Msg::Ai(ask, reply)).is_err() {
            return AiAnswer::Refused(AiRefusal::Closing);
        }
        answer
            .recv()
            .unwrap_or(AiAnswer::Refused(AiRefusal::Closing))
    }

    /// Renders `ask` through the export's engine and measures it, on the caller's thread.
    /// `progress` hears 0..=100 as it goes; when `cancelled` says so, the job is given up.
    pub fn ai_measure(
        &self,
        ask: AiMeasure,
        progress: &mut dyn FnMut(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AiLevels, AiRefusal> {
        let (news_tx, news) = mpsc::channel();
        let (reply, answer) = mpsc::channel();
        let msg = Msg::AiMeasure {
            ask,
            news: news_tx,
            reply,
        };
        self.tx.send(msg).map_err(|_| AiRefusal::Closing)?;
        let job = answer.recv().map_err(|_| AiRefusal::Closing)??;
        loop {
            if cancelled() {
                let _ = self.tx.send(Msg::AiDrop(job.id));
                return Err(AiRefusal::Cannot("it was cancelled".into()));
            }
            match news.recv_timeout(Duration::from_millis(100)) {
                Ok(ExportNews::Counted { percent, .. }) => progress(percent),
                Ok(ExportNews::Measured {
                    passes, analysis, ..
                }) => {
                    let mut passes = passes.into_iter();
                    let master = passes.next().ok_or(AiRefusal::Closing)?;
                    let muted = (job.tracks.iter()).filter(|t| t.2).map(|t| t.0);
                    let muted = muted.collect();
                    let tracks = job.tracks.into_iter().zip(passes);
                    return Ok(AiLevels {
                        sample_rate: job.sample_rate,
                        master,
                        tracks: tracks.map(|((n, name, _), p)| (n, name, p)).collect(),
                        muted,
                        analysis,
                    });
                }
                Ok(ExportNews::Failed { reason, .. }) => {
                    return Err(AiRefusal::ExportFailed(format!("{reason:?}")));
                }
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(AiRefusal::Closing),
            }
        }
    }

    /// Renders `ask` into files through the export's engine, on the caller's thread. As
    /// [`Core::ai_measure`] for `progress` and `cancelled`.
    pub fn ai_render(
        &self,
        ask: AiRender,
        progress: &mut dyn FnMut(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AiRendered, AiRefusal> {
        let (news_tx, news) = mpsc::channel();
        let (reply, answer) = mpsc::channel();
        let msg = Msg::AiRender {
            ask,
            news: news_tx,
            reply,
        };
        self.tx.send(msg).map_err(|_| AiRefusal::Closing)?;
        let job = answer.recv().map_err(|_| AiRefusal::Closing)??;
        let (mut peak_db, mut seconds) = (None, 0);
        loop {
            if cancelled() {
                let _ = self.tx.send(Msg::AiDrop(job.id));
                return Err(AiRefusal::Cannot("it was cancelled".into()));
            }
            match news.recv_timeout(Duration::from_millis(100)) {
                Ok(ExportNews::Counted {
                    percent,
                    peak_db: peak,
                    length_s,
                    ..
                }) => {
                    progress(percent);
                    peak_db = peak;
                    seconds = length_s;
                }
                Ok(ExportNews::Finished { files, .. }) => {
                    return Ok(AiRendered {
                        files,
                        sample_rate: job.sample_rate,
                        seconds: f64::from(seconds),
                        peak_db,
                    });
                }
                Ok(ExportNews::Failed { reason, .. }) => {
                    return Err(AiRefusal::ExportFailed(format!("{reason:?}")));
                }
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(AiRefusal::Closing),
            }
        }
    }

    fn input(&self, input: Input) -> u64 {
        let (reply, rev) = mpsc::channel();
        if self.tx.send(Msg::Input(input, Some(reply))).is_err() {
            return self.hub.rev();
        }
        rev.recv().unwrap_or_else(|_| self.hub.rev())
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A measuring job of the AI link as it was given out: the tracks are numbered and named as
/// the song had them then, and whether it muted them.
struct AiJob {
    id: u64,
    sample_rate: u32,
    tracks: Vec<(u32, String, bool)>,
}

/// What the sinks of the MIDI thread and the device watcher need, to make new ones.
struct Links {
    hub: Arc<Hub>,
    shared: Arc<Shared>,
    tx: Sender<Msg>,
}

impl Links {
    /// Keys go straight to the engine and the screen; plug changes, buttons and the fader go
    /// to the core thread.
    fn midi_sink(&self, mut midi_in: MidiFeed) -> MidiSink {
        let (hub, shared, tx) = (self.hub.clone(), self.shared.clone(), self.tx.clone());
        let mut last_played = String::new();
        let mut hands = keys::Hands::default();
        Box::new(move |message: MidiMessage| match message {
            MidiMessage::Event {
                keyboard,
                event,
                t_us,
            } => {
                let reset = match event {
                    MidiEvent::Control {
                        controller: keys::FADER,
                        value,
                    } => {
                        if shared.fader.swap(u16::from(value), Relaxed) == NO_FADER {
                            let _ = tx.send(Msg::Fader);
                        }
                        return;
                    }
                    MidiEvent::Control {
                        controller: keys::ALL_NOTES_OFF,
                        ..
                    } => Some(false),
                    MidiEvent::Control {
                        controller: keys::RESET_ALL_CONTROLLERS,
                        ..
                    } => Some(true),
                    _ => None,
                };
                if let Some(reset) = reset {
                    for event in hands.let_go(&keyboard, reset) {
                        play_key(event, t_us, &shared, &mut midi_in, &hub, &tx);
                    }
                    return;
                }
                hands.saw(&keyboard, event);
                let event = fixed(event, shared.fixed_velocity.load(Relaxed));
                play_key(event, t_us, &shared, &mut midi_in, &hub, &tx);
                // Only a change of keyboard goes to the core thread.
                if matches!(event, MidiEvent::NoteOn { .. }) && keyboard != last_played {
                    last_played = keyboard;
                    let id = last_played.clone();
                    let _ = tx.send(Msg::Input(Input::KeyboardPlayed { id }, None));
                }
            }
            MidiMessage::Added(keyboard) => {
                let _ = tx.send(Msg::Input(Input::KeyboardAdded(keyboard), None));
            }
            MidiMessage::Button { button, .. } => {
                let _ = tx.send(Msg::Input(Input::KeyboardButton(button), None));
            }
            MidiMessage::Removed { id } => {
                hands.forget(&id);
                if id == last_played {
                    last_played.clear();
                }
                let _ = tx.send(Msg::Input(Input::KeyboardRemoved { id }, None));
            }
            MidiMessage::Ready => {
                let _ = tx.send(Msg::Input(Input::MidiBack, None));
            }
            MidiMessage::Stopped { reason } => {
                // The next thread reports every keyboard again, the played one too.
                last_played.clear();
                hands.clear();
                let _ = tx.send(Msg::MidiStopped { reason });
            }
        })
    }

    fn device_watcher(&self) -> Result<DeviceWatcher, DevicesError> {
        let tx = self.tx.clone();
        DeviceWatcher::start(move |event| {
            let _ = tx.send(match event {
                DevicesEvent::Picture(picture) => Msg::Input(Input::Devices(picture), None),
                DevicesEvent::Lost => Msg::DevicesLost,
            });
        })
    }
}

/// The keyboard input, started again after it stops by itself.
struct Midi {
    options: MidiOptions,
    state: MidiState,
}

enum MidiState {
    Running(MidiInput),
    /// `attempt` counts the tries of this series, starting at 1.
    Waiting {
        sink: MidiSink,
        attempt: u32,
        at: Instant,
    },
    /// The sink went with a thread that could not give it back.
    Gone,
}

impl Midi {
    fn stopped(&mut self, reason: &str) {
        eprintln!("meloet: the keyboard input stopped: {reason}");
        let state = std::mem::replace(&mut self.state, MidiState::Gone);
        self.state = match state {
            MidiState::Running(input) => match input.into_sink() {
                Some(sink) => MidiState::Waiting {
                    sink,
                    attempt: 1,
                    at: Instant::now() + retry_delay(1),
                },
                None => {
                    eprintln!("meloet: the keyboard input cannot start again");
                    MidiState::Gone
                }
            },
            other => other,
        };
    }

    fn tick(&mut self, now: Instant) {
        if !matches!(&self.state, MidiState::Waiting { at, .. } if *at <= now) {
            return;
        }
        let MidiState::Waiting { sink, attempt, .. } =
            std::mem::replace(&mut self.state, MidiState::Gone)
        else {
            return;
        };
        self.state = match MidiInput::start_boxed(self.options.clone(), sink) {
            Ok(input) => MidiState::Running(input),
            Err((e, sink)) => {
                if attempt == 1 {
                    eprintln!("meloet: the keyboard input does not start again yet: {e}");
                }
                match sink {
                    Some(sink) => MidiState::Waiting {
                        sink,
                        attempt: attempt + 1,
                        at: now + retry_delay(attempt + 1),
                    },
                    None => MidiState::Gone,
                }
            }
        };
    }

    /// The engine was made anew: the keys must go into its ring from now on.
    fn feed(&mut self, sink: MidiSink) {
        let state = std::mem::replace(&mut self.state, MidiState::Gone);
        self.state = match state {
            MidiState::Running(input) => {
                drop(input.into_sink());
                match MidiInput::start_boxed(self.options.clone(), sink) {
                    Ok(input) => MidiState::Running(input),
                    Err((_, Some(sink))) => MidiState::Waiting {
                        sink,
                        attempt: 1,
                        at: Instant::now() + retry_delay(1),
                    },
                    Err((_, None)) => MidiState::Gone,
                }
            }
            MidiState::Waiting { attempt, at, .. } => MidiState::Waiting { sink, attempt, at },
            MidiState::Gone => MidiState::Waiting {
                sink,
                attempt: 1,
                at: Instant::now(),
            },
        };
    }
}

/// The device watcher, connected again after PipeWire comes back.
enum DevicesState {
    /// Held for its drop, which stops the thread.
    Running {
        _watcher: DeviceWatcher,
    },
    Waiting {
        attempt: u32,
        at: Instant,
    },
}

/// A key, pedal or bend to the instrument of the selected track (so to a recording too), to
/// the meters and to the screen.
fn play_key(
    event: MidiEvent,
    t_us: u64,
    shared: &Shared,
    midi_in: &mut MidiFeed,
    hub: &Hub,
    tx: &Sender<Msg>,
) {
    // Keys played while no stream runs must not sound when one opens.
    if shared.stream.open.load(Relaxed) {
        let _ = midi_in.push(MidiIn {
            event: instrument_event(event),
            t_us,
        });
        let stream = &shared.stream;
        if matches!(event, MidiEvent::NoteOn { .. })
            && stream.wants_loading_key.load(Relaxed)
            && stream.wants_loading_key.swap(false, Relaxed)
        {
            let _ = tx.send(Msg::Input(Input::KeyWhileLoading, None));
        }
    }
    shared.held.note(event);
    hub.event(protocol_event(event, t_us));
}

/// [`Shared::fader`] when no value waits.
const NO_FADER: u16 = u16::MAX;

/// A NoteOn at the fixed velocity (B-66), when there is one (`fixed` 1..=127; 0: not fixed).
/// Done first on the MIDI thread, so the sound, the recording, the meter and the screen all get
/// the same value.
fn fixed(event: MidiEvent, fixed: u8) -> MidiEvent {
    match event {
        MidiEvent::NoteOn { note, .. } if fixed > 0 => MidiEvent::NoteOn {
            note,
            velocity: fixed,
        },
        other => other,
    }
}

fn instrument_event(event: MidiEvent) -> InstrumentEvent {
    match event {
        MidiEvent::NoteOn { note, velocity } => InstrumentEvent::NoteOn { note, velocity },
        MidiEvent::NoteOff { note, .. } => InstrumentEvent::NoteOff { note },
        MidiEvent::Control { controller, value } => InstrumentEvent::Control { controller, value },
        MidiEvent::PitchBend { value } => InstrumentEvent::PitchBend { value },
    }
}

fn protocol_event(event: MidiEvent, t_us: u64) -> Event {
    match event {
        MidiEvent::NoteOn { note, velocity } => Event::Note {
            on: true,
            note,
            velocity,
            t_us,
        },
        MidiEvent::NoteOff { note, velocity } => Event::Note {
            on: false,
            note,
            velocity,
            t_us,
        },
        MidiEvent::Control { controller, value } => Event::Control {
            controller,
            value,
            t_us,
        },
        MidiEvent::PitchBend { value } => Event::Bend { value, t_us },
    }
}

/// The engine and its stream. One engine for the whole run: a new output gets the same one.
/// Only if a stream never gives it back is a new one made.
struct Audio {
    handle: EngineHandle,
    /// Here while no stream runs.
    engine: Option<Engine>,
    stream: Option<RunningOutput>,
    allowed: bool,
    /// The ring into an engine made anew, for the MIDI thread.
    rebuilt: Option<MidiFeed>,
    /// What the engine has on its tracks, to put it on an engine made anew.
    seats: SeatBook,
    /// What the engine has in its mixer, for the same.
    mix: MixBook,
    /// The samples the instruments are made with.
    sounds: Library,
    /// The same for the presets of the added SF2 files.
    sf2: Sf2Store,
    /// The frames and the rate asked of the PC (the advanced sound settings).
    wish_frames: u32,
    wish_rate: u32,
    /// The seats that sound the input, for the same as `seats`.
    voices: Vec<(usize, VoiceSpec)>,
    /// The vocal clips of the seats, for every stream that opens and every map that comes.
    clips: Vec<SeatClips>,
    /// The song's audio table as last told.
    sources: Arc<Vec<Arc<meloet_model::AudioSource>>>,
    store: Arc<AudioStore>,
    readahead: ReadAhead,
    /// `EngineMeters::stream_missing` as last written to the log.
    missing_logged: u64,
    /// `AudioStore::generation` as last looked at.
    store_seen: u64,
    /// The song's tempo as last told: what the engine's map was made of.
    timeline: Option<Arc<meloet_model::Timeline>>,
    /// The thread that makes the audio of stretched and pitched clips (22-19 pass 2).
    maker: Option<Maker>,
    /// The audio it said is in the cache, of what the clips need now.
    made: HashSet<MadeKey>,
    /// What it was last asked for: the key and the clip of every plan, and the song's audio.
    wanted: (Vec<(MadeKey, u32)>, Vec<[u8; 16]>),
    /// The plans may be other ones than `wanted`.
    plans_changed: bool,
    /// A test runs the engine's blocks itself, as a stream of this rate would
    /// (`Audio::open_by_hand`).
    #[cfg(test)]
    by_hand: Option<u32>,
}

/// The vocal clips of a seat.
struct SeatClips {
    seat: usize,
    clips: Vec<Arc<meloet_model::song::Clip>>,
    /// What is to be made for them, for the stream that runs.
    plans: Vec<MadePlan>,
    /// What the read-ahead thread has of them, while `live`.
    feeds: Vec<FeedClip>,
    /// The engine of the stream that runs has a ring for the seat.
    live: bool,
}

/// What the engine was given: the instrument of each of its tracks (the audition track is the
/// last), the instrument tried out on one of them, and the track the keys play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SeatBook {
    pub own: Vec<Option<InstrumentRef>>,
    pub trial: Option<(usize, InstrumentRef)>,
    pub target: Option<usize>,
}

impl SeatBook {
    pub(crate) fn new() -> SeatBook {
        SeatBook {
            own: vec![None; AUDITION_SEAT + 1],
            trial: None,
            target: Some(0),
        }
    }

    /// Takes in an op about instruments or the play target. Returns false for any other op.
    pub(crate) fn note(&mut self, op: &EngineOp) -> bool {
        match op {
            EngineOp::Instrument { seat, instrument } => {
                // As in the engine: on a seat with a trial, it takes the place of the tried one.
                match (&mut self.trial, self.own.get_mut(*seat)) {
                    (Some((s, tried)), _) if s == seat => *tried = instrument.clone(),
                    (_, Some(own)) => *own = Some(instrument.clone()),
                    _ => {}
                }
            }
            EngineOp::TryInstrument { seat, instrument } => match self.own.get_mut(*seat) {
                Some(Some(_)) => self.trial = Some((*seat, instrument.clone())),
                // As in the engine: with nothing to set aside, it is the seat's own.
                Some(own) => *own = Some(instrument.clone()),
                None => {}
            },
            EngineOp::EndTrial { seat, keep } => {
                if let Some((_, tried)) = self.trial.take_if(|(s, _)| s == seat)
                    && *keep
                {
                    self.own[*seat] = Some(tried);
                }
            }
            EngineOp::MoveInstrument { from, to } => {
                if from != to && *from < self.own.len() && *to < self.own.len() {
                    self.own[*to] = self.own[*from].take();
                    if self.target == Some(*from) {
                        self.target = Some(*to);
                    }
                }
            }
            EngineOp::ClearSeat { seat } => {
                if let Some(own) = self.own.get_mut(*seat) {
                    *own = None;
                    self.trial = self.trial.take().filter(|(s, _)| s != seat);
                }
            }
            EngineOp::PlayTarget { seat } => self.target = *seat,
            _ => return false,
        }
        true
    }

    /// The ops that bring an empty engine to this.
    fn ops(&self) -> Vec<EngineOp> {
        let mut ops: Vec<EngineOp> = self
            .own
            .iter()
            .enumerate()
            .filter_map(|(seat, own)| {
                own.clone()
                    .map(|instrument| EngineOp::Instrument { seat, instrument })
            })
            .collect();
        if let Some((seat, instrument)) = self.trial.clone() {
            ops.push(EngineOp::TryInstrument { seat, instrument });
        }
        ops.push(EngineOp::PlayTarget { seat: self.target });
        ops
    }
}

/// How long a command about instruments waits for room in the engine's ring.
const SEAT_COMMAND_WAIT: Duration = Duration::from_millis(100);

impl Audio {
    fn new(
        engine: Engine,
        handle: EngineHandle,
        allowed: bool,
        sounds: Library,
        sf2: Sf2Store,
        seats: Vec<EngineOp>,
        (wish_frames, wish_rate): (u32, u32),
    ) -> Audio {
        // Until `with_store` gives the core's own: a store that has no file.
        let store = Arc::new(AudioStore::new(None));
        let mut audio = Audio {
            handle,
            engine: Some(engine),
            stream: None,
            allowed,
            rebuilt: None,
            seats: SeatBook::new(),
            mix: MixBook::default(),
            sounds,
            sf2,
            wish_frames,
            wish_rate,
            voices: Vec::new(),
            clips: Vec::new(),
            sources: Arc::default(),
            readahead: ReadAhead::spawn(store.clone()),
            store,
            missing_logged: 0,
            store_seen: 0,
            timeline: None,
            maker: None,
            made: HashSet::new(),
            wanted: Default::default(),
            plans_changed: false,
            #[cfg(test)]
            by_hand: None,
        };
        if let Some(wants) = audio.handle.take_stream_wants() {
            audio.readahead.set_wants(wants);
        }
        for op in seats {
            audio.op(op);
        }
        audio
    }

    /// The rate of the stream that runs the engine; `None`: none does.
    fn running(&self) -> Option<u32> {
        #[cfg(test)]
        if self.by_hand.is_some() {
            return self.by_hand;
        }
        self.stream.as_ref().map(|s| s.sample_rate())
    }

    /// For a test that is the stream: from now on the engine is told everything a running
    /// one is, and the test calls its blocks (`self.engine`).
    #[cfg(test)]
    pub(crate) fn open_by_hand(&mut self, rate: u32) {
        if let Some(engine) = &mut self.engine {
            engine.prepare(rate);
        }
        self.readahead.clear();
        for entry in self.clips.iter_mut() {
            entry.live = false;
        }
        self.by_hand = Some(rate);
    }

    /// The place the audio files are opened from, shared with the rest of the core.
    fn with_store(mut self, store: Arc<AudioStore>) -> Audio {
        store.set_sources(source_names(&self.sources));
        self.readahead.set_store(store.clone());
        self.store = store;
        self
    }

    fn with_maker(mut self, maker: Option<Maker>) -> Audio {
        self.maker = maker;
        self
    }

    /// Asks the maker for what the clips need now, when that is another list than it was
    /// last asked for. What is no longer in it is given up there.
    fn want_made(&mut self) {
        if !std::mem::take(&mut self.plans_changed) {
            return;
        }
        let plans: Vec<MadePlan> = (self.clips.iter())
            .flat_map(|c| c.plans.iter().cloned())
            .collect();
        let uids: Vec<[u8; 16]> = self.sources.iter().map(|s| s.uid).collect();
        let wanted = (plans.iter().map(|p| (p.key, p.clip)).collect(), uids);
        if wanted == self.wanted {
            return;
        }
        // Only what is wanted counts as made: the maker says again what the cache has.
        self.made.retain(|key| plans.iter().any(|p| p.key == *key));
        if let Some(maker) = &self.maker {
            maker.want(plans, wanted.1.clone());
        }
        self.wanted = wanted;
    }

    /// Audio the clips wait for is in the cache: the seats that play it get it, laid over
    /// what sounds.
    fn made_ready(&mut self, keys: Vec<MadeKey>) {
        let wanted = |key: &MadeKey| self.wanted.0.iter().any(|(k, _)| k == key);
        let new: Vec<MadeKey> = (keys.into_iter())
            .filter(|key| wanted(key) && !self.made.contains(key))
            .collect();
        self.made.extend(new.iter().copied());
        let seats: Vec<usize> = (self.clips.iter())
            .filter(|c| c.plans.iter().any(|p| new.contains(&p.key)))
            .map(|c| c.seat)
            .collect();
        for seat in seats {
            self.send_clips(seat, false);
        }
    }

    /// Made audio that a clip plays was not in the cache any more (somebody emptied it):
    /// the clips play their files again, and the maker looks at what is there.
    fn follow_made(&mut self) {
        let stats = self.readahead.stats();
        if stats.made_lost.swap(false, Relaxed) {
            self.made.clear();
            self.resend_clips();
            if let Some(maker) = &self.maker {
                maker.retry();
            }
        }
    }

    /// The vocal clips of a seat as the song has them now.
    fn clips_op(&mut self, seat: usize, clips: Vec<Arc<meloet_model::song::Clip>>) {
        match self.clips.iter_mut().find(|c| c.seat == seat) {
            Some(entry) => entry.clips = clips,
            None => self.clips.push(SeatClips {
                seat,
                clips,
                plans: Vec::new(),
                feeds: Vec::new(),
                live: false,
            }),
        }
        self.send_clips(seat, false);
    }

    /// Makes the engine's table and the read-ahead thread's clips of a seat for the stream
    /// that runs, and sends both: the thread first, so that it has the clips when the
    /// engine asks for them. `anew`: the thread starts again whatever the clips are (another
    /// map, files that can be opened now).
    fn send_clips(&mut self, seat: usize, anew: bool) {
        let Some(rate) = self.running() else {
            return;
        };
        let Some(at) = self.clips.iter().position(|c| c.seat == seat) else {
            return;
        };
        let map = self.handle.tempo_map().0.clone();
        let entry = &mut self.clips[at];
        let timeline = self.timeline.as_deref();
        let seat_audio = seat_audio(timeline, &map, rate, &entry.clips, &self.made);
        let (table, feeds) = (seat_audio.table, seat_audio.feeds);
        entry.plans = seat_audio.plans;
        self.plans_changed = true;
        if feeds.is_empty() {
            let was_live = std::mem::take(&mut entry.live);
            entry.feeds.clear();
            if entry.clips.is_empty() {
                self.clips.remove(at);
            }
            if was_live {
                self.readahead.set_track(seat, None, Vec::new());
                self.push(EngineCommand::SetClips {
                    track: seat,
                    table: None,
                    refeed: true,
                });
                self.push(EngineCommand::SetStream {
                    track: seat,
                    reader: None,
                });
            }
            return;
        }
        // Other audio for the same clips comes in over what sounds, by the thread alone.
        let splice = !anew && entry.live && only_audio_differs(&entry.feeds, &feeds);
        let refeed = !splice && (anew || !entry.live || feeds != entry.feeds);
        let mut reader = None;
        if !entry.live {
            let (writer, engine_end) = stream_ring(rate);
            self.readahead
                .set_track(seat, Some((writer, rate)), feeds.clone());
            reader = Some(Box::new(engine_end));
            entry.live = true;
        } else if splice {
            self.readahead.splice_clips(seat, feeds.clone());
        } else if refeed {
            self.readahead.set_clips(seat, feeds.clone());
        }
        entry.feeds = feeds;
        if reader.is_some() {
            self.push(EngineCommand::SetStream {
                track: seat,
                reader,
            });
        }
        self.push(EngineCommand::SetClips {
            track: seat,
            table: Some(Box::new(table)),
            refeed,
        });
        if splice {
            self.push(EngineCommand::SpliceClips { track: seat });
        }
    }

    /// Every seat's clips again, from their start: after another map, and when files may
    /// be opened that could not be before.
    fn resend_clips(&mut self) {
        let seats: Vec<usize> = self.clips.iter().map(|c| c.seat).collect();
        for seat in seats {
            self.send_clips(seat, true);
        }
    }

    /// The seat was emptied, its ring with it.
    fn clips_gone(&mut self, seat: usize) {
        if let Some(at) = self.clips.iter().position(|c| c.seat == seat)
            && self.clips.remove(at).live
        {
            self.readahead.set_track(seat, None, Vec::new());
        }
        self.plans_changed = true;
    }

    /// The store may have files now that it did not have (one was decoded or found, the
    /// song's folder is another one): the seats that were fed silence for a file that was
    /// not there are read again from where the song is. The others go on as they are.
    fn follow_store(&mut self) {
        let now = self.store.generation();
        if now == self.store_seen {
            return;
        }
        self.store_seen = now;
        // A file that was not there to make audio from may be there now.
        if let Some(maker) = &self.maker {
            maker.retry();
        }
        let unopened = self
            .readahead
            .stats()
            .unopened
            .load(std::sync::atomic::Ordering::Relaxed);
        if unopened == 0 {
            return;
        }
        self.readahead.store_changed();
        for seat in (0..64).filter(|seat| unopened & (1u64 << seat) != 0) {
            self.send_clips(seat, true);
        }
    }

    /// Writes down the frames of vocal clips that were not read in time, when there are
    /// more than at the last look. Returns whether there were.
    fn log_missing(&mut self) -> bool {
        let missing = self.handle.meters().stream_missing();
        if missing <= self.missing_logged {
            return false;
        }
        let more = missing - self.missing_logged;
        eprintln!("meloet: {more} frames of vocal clips were not read in time ({missing})");
        self.missing_logged = missing;
        true
    }

    /// While the engine is here: lets it take in the commands that wait, so that the
    /// instruments are on their tracks before the next stream prepares them.
    fn settle(&mut self) {
        if let Some(engine) = &mut self.engine {
            loop {
                engine.settle();
                if self.handle.drop_retired() == 0 {
                    break;
                }
            }
        }
    }

    /// An op about instruments, the play target or the mixer: for the engine also when no
    /// stream runs.
    fn seat_op(&mut self, op: EngineOp) {
        self.seats.note(&op);
        self.mix.note(&op);
        if let EngineOp::ClearSeat { seat } = &op {
            self.clips_gone(*seat);
        }
        if self.stream.is_none() && self.engine.is_none() {
            // The engine is lost; the one made in its place gets what the book says.
            return;
        }
        self.send_seat_op(op);
        self.settle();
    }

    /// An instrument as it can sound now: silent while its samples are still to be read.
    fn make(&self, instrument: &InstrumentRef) -> Box<dyn Instrument> {
        let samples = match instrument {
            InstrumentRef::Builtin(b) => self.sounds.samples(*b),
            InstrumentRef::Sf2(preset) => self.sf2.samples(preset),
            InstrumentRef::Plugin(_) => instruments::Samples::NotRead,
        };
        instruments::make(instrument, samples)
    }

    /// `EngineOp::Plug` for an instrument: a loaded plugin goes on the seat, as its own or,
    /// with `trial`, as the one tried on it.
    fn plug_instrument(&mut self, seat: usize, trial: bool, plugin: Box<dyn Instrument>) {
        if self.stream.is_none() && self.engine.is_none() {
            // The engine is lost. The plugin is let go of here, and loaded again for the
            // engine made in its place.
            return;
        }
        let sent = if trial {
            self.handle.try_instrument(seat, plugin)
        } else {
            self.handle.swap_instrument(seat, plugin)
        };
        if let Err(command) = sent {
            self.push(command);
        }
        self.settle();
    }

    /// `EngineOp::Plug` for an effect.
    fn plug_effect(
        &mut self,
        strip: crate::play::StripSeat,
        slot: usize,
        plugin: Box<dyn meloet_engine::Effect>,
    ) {
        if self.stream.is_none() && self.engine.is_none() {
            return;
        }
        let strip = mixbook::mix_strip(strip);
        if let Err(command) = self.handle.set_effect(strip, slot, Some(plugin)) {
            self.push(command);
        }
        self.settle();
    }

    fn send_seat_op(&mut self, op: EngineOp) {
        let command = match op {
            EngineOp::Instrument { seat, instrument } => {
                match self.handle.swap_instrument(seat, self.make(&instrument)) {
                    Ok(()) => return,
                    Err(command) => command,
                }
            }
            EngineOp::TryInstrument { seat, instrument } => {
                match self.handle.try_instrument(seat, self.make(&instrument)) {
                    Ok(()) => return,
                    Err(command) => command,
                }
            }
            EngineOp::EndTrial { seat, keep } => EngineCommand::EndTrial { track: seat, keep },
            EngineOp::MoveInstrument { from, to } => EngineCommand::MoveInstrument { from, to },
            EngineOp::ClearSeat { seat } => EngineCommand::ClearTrack { track: seat },
            EngineOp::PlayTarget { seat: Some(track) } => EngineCommand::SetPlayTarget { track },
            EngineOp::PlayTarget { seat: None } => EngineCommand::ClearPlayTarget,
            EngineOp::Insert {
                strip,
                slot,
                insert,
            } => {
                let effect = insert.as_ref().and_then(mixbook::make);
                let strip = mixbook::mix_strip(strip);
                match self.handle.set_effect(strip, slot, effect) {
                    Ok(()) => return,
                    Err(command) => command,
                }
            }
            op => match mixbook::command(&op) {
                Some(command) => command,
                None => return,
            },
        };
        self.push(command);
    }

    /// Sends a command that must not be lost, or the engine and the song no longer agree.
    /// A full ring empties within a block or two.
    fn push(&mut self, mut command: EngineCommand) {
        let until = Instant::now() + SEAT_COMMAND_WAIT;
        loop {
            command = match self.handle.send(command) {
                Ok(()) => return,
                Err(command) => command,
            };
            self.handle.drop_retired();
            if self.stream.is_none() {
                self.settle();
            } else if Instant::now() < until {
                std::thread::sleep(Duration::from_millis(1));
            } else {
                eprintln!("meloet: the engine's command ring stayed full");
                return;
            }
        }
    }

    fn open(&mut self, node_name: &str, facts: &StreamFacts) -> Option<u32> {
        self.close(facts);
        if !self.allowed {
            return None;
        }
        let engine = self.engine.take().unwrap_or_else(|| {
            eprintln!("meloet: the engine was lost with an earlier stream; making a new one");
            self.handle.drop_retired();
            let (engine, feed) = self.handle.rebuild();
            self.rebuilt = Some(feed);
            self.engine = Some(engine);
            if let Some(wants) = self.handle.take_stream_wants() {
                self.readahead.set_wants(wants);
            }
            for op in self.mix.ops().into_iter().chain(self.seats.ops()) {
                self.send_seat_op(op);
                self.settle();
            }
            for (seat, voice) in self.voices.clone() {
                self.push(voice_command(seat, Some(voice)));
                self.settle();
            }
            self.engine.take().expect("put there above")
        });
        let target = OutputTarget {
            node_name: node_name.to_owned(),
        };
        match AudioOutput::open_at(&target, self.wish_frames, Some(self.wish_rate), engine) {
            Ok(stream) => {
                let sample_rate = stream.sample_rate();
                let meters = self.handle.meters();
                facts.callbacks_at_open.store(meters.callbacks(), Relaxed);
                facts.lost_at_open.store(meters.streams_lost(), Relaxed);
                facts.xruns_at_open.store(meters.xruns(), Relaxed);
                facts.epoch.fetch_add(1, Relaxed);
                facts.open.store(true, Relaxed);
                self.stream = Some(stream);
                // The rings of the stream before are for its sample rate. The new ones are
                // made with the tempo map that the rules send next.
                self.readahead.clear();
                for entry in self.clips.iter_mut() {
                    entry.live = false;
                }
                Some(sample_rate)
            }
            Err(e) => {
                eprintln!("meloet: cannot open {node_name}: {e}");
                self.engine = e.engine.map(|engine| *engine);
                None
            }
        }
    }

    fn close(&mut self, facts: &StreamFacts) {
        if let Some(stream) = self.stream.take() {
            // Nothing is made for a stream that is gone: the next one may run at another rate.
            for entry in self.clips.iter_mut() {
                entry.plans.clear();
            }
            self.plans_changed = true;
            facts.open.store(false, Relaxed);
            facts.epoch.fetch_add(1, Relaxed);
            self.engine = stream.close();
            self.settle();
        }
    }

    fn command(&mut self, command: EngineCommand) {
        // Without a stream the command would wait in the ring and act when one opens.
        if self.running().is_some() && self.handle.send(command).is_err() {
            eprintln!("meloet: the engine's command ring is full");
        }
    }

    fn op(&mut self, op: EngineOp) {
        let command = match op {
            EngineOp::Instrument { .. }
            | EngineOp::TryInstrument { .. }
            | EngineOp::EndTrial { .. }
            | EngineOp::MoveInstrument { .. }
            | EngineOp::ClearSeat { .. }
            | EngineOp::PlayTarget { .. }
            | EngineOp::Strip { .. }
            | EngineOp::Sends { .. }
            | EngineOp::Insert { .. }
            | EngineOp::Param { .. }
            | EngineOp::Bypass { .. }
            | EngineOp::MoveInsert { .. } => {
                self.seat_op(op);
                return;
            }
            EngineOp::Play { from } => EngineCommand::Play { from },
            EngineOp::Record {
                from,
                count_in_beats,
                muted,
            } => EngineCommand::Record {
                from,
                count_in_beats,
                muted,
            },
            EngineOp::RecordHere { muted } => EngineCommand::RecordHere { muted },
            EngineOp::RecordMuted { muted } => EngineCommand::SetRecordMuted { muted },
            EngineOp::Stop { t_us } => EngineCommand::Stop { t_us },
            EngineOp::Locate { tick } => EngineCommand::Locate { tick },
            EngineOp::Timeline { timeline } => {
                // Made here, off the audio thread; the engine only swaps it in.
                let map = Arc::new(TempoMap::new(&timeline));
                self.timeline = Some(timeline);
                if self.running().is_some() {
                    if self.handle.set_tempo_map(map).is_err() {
                        eprintln!("meloet: the engine did not take the tempo map");
                    } else {
                        // The clips are at other frames now.
                        let (map, map_gen) = self.handle.tempo_map();
                        self.readahead.set_map(map.clone(), map_gen);
                        self.resend_clips();
                    }
                }
                return;
            }
            EngineOp::Loop { start, end, on } => EngineCommand::SetLoop { start, end, on },
            EngineOp::Metronome { on } => EngineCommand::SetMetronome { on },
            EngineOp::Audition {
                track,
                keys,
                velocity,
            } => EngineCommand::Audition {
                track,
                keys: Chord::new(&keys),
                velocity,
            },
            EngineOp::AuditionOff => EngineCommand::AuditionOff,
            EngineOp::PlaySample { instrument } => {
                if self.running().is_some() {
                    let phrase = phrases::for_group(instrument.group());
                    let player = PhrasePlayer::new(self.make(&instrument), phrase);
                    if self.handle.play_sample(Box::new(player)).is_err() {
                        eprintln!("meloet: the engine did not take the sample");
                    }
                }
                return;
            }
            // The worker gives these the loaded plugin (`Worker::plug`).
            EngineOp::Plug { .. } | EngineOp::PlugSample { .. } => return,
            EngineOp::Voice { seat, voice } => {
                self.voices.retain(|(s, _)| *s != seat);
                self.voices.extend(voice.map(|v| (seat, v)));
                if self.stream.is_some() || self.engine.is_some() {
                    self.push(voice_command(seat, voice));
                    self.settle();
                }
                return;
            }
            EngineOp::Clips { seat, clips } => {
                self.clips_op(seat, clips);
                return;
            }
            EngineOp::AudioSources { sources } => {
                self.store.set_sources(source_names(&sources));
                self.sources = sources;
                self.plans_changed = true;
                return;
            }
            EngineOp::StopSample => EngineCommand::StopSample,
            EngineOp::SilentTracks { tracks } => EngineCommand::SetSilentTracks {
                tracks: tracks.into_iter().collect::<TrackSet>(),
            },
            EngineOp::Automation { seat, lanes } => {
                if self.running().is_some()
                    && self.handle.set_automation(seat, auto_lanes(lanes)).is_err()
                {
                    eprintln!("meloet: the engine did not take the lanes of track {seat}");
                }
                return;
            }
            EngineOp::HoldLane { seat, target, on } => EngineCommand::HoldLane {
                track: seat,
                target,
                on,
            },
            EngineOp::Sequence { track, events } => {
                if self.running().is_some() {
                    let sequence = Sequence::new(events.into_iter().map(sequence_event).collect());
                    if self.handle.swap_sequence(track, sequence).is_err() {
                        eprintln!("meloet: the engine did not take the sequence of track {track}");
                    }
                }
                return;
            }
            EngineOp::GrowSequence { track, events } => {
                if self.running().is_some() {
                    let sequence = Sequence::new(events.into_iter().map(sequence_event).collect());
                    if self.handle.grow_sequence(track, sequence).is_err() {
                        eprintln!("meloet: the engine did not take the sequence of track {track}");
                    }
                }
                return;
            }
        };
        self.command(command);
    }
}

/// The audio table as the place that opens the files takes it.
fn source_names(sources: &[Arc<meloet_model::AudioSource>]) -> Vec<([u8; 16], String)> {
    sources.iter().map(|s| (s.uid, s.file.clone())).collect()
}

fn voice_command(seat: usize, voice: Option<VoiceSpec>) -> EngineCommand {
    EngineCommand::SetVoice {
        track: seat,
        voice: voice.map(|v| {
            let channels = v.input.channels();
            meloet_engine::Voice {
                left: channels[0],
                right: channels[channels.len() - 1],
                on: v.on,
            }
        }),
    }
}

/// The lanes of a track as the engine takes them.
pub(crate) fn auto_lanes(lanes: Vec<crate::play::SeatLane>) -> Vec<meloet_engine::AutoLane> {
    (lanes.into_iter())
        .map(|lane| meloet_engine::AutoLane {
            target: lane.target,
            step: lane.step,
            points: lane.points,
            slot: lane.slot,
            effect: lane.effect,
        })
        .collect()
}

pub(crate) fn sequence_event(e: PlayEvent) -> SequenceEvent {
    SequenceEvent {
        tick: e.tick,
        event: match e.kind {
            PlayKind::NoteOn { key, velocity } => InstrumentEvent::NoteOn {
                note: key,
                velocity,
            },
            PlayKind::NoteOff { key } => InstrumentEvent::NoteOff { note: key },
            PlayKind::Control(ControlValue::Cc { number, value }) => InstrumentEvent::Control {
                controller: number,
                value,
            },
            PlayKind::Control(ControlValue::PitchBend(value)) => {
                InstrumentEvent::PitchBend { value }
            }
        },
    }
}

fn record_report(r: Recorded, held: &Held, file_frame: Option<f64>) -> Option<RecordReport> {
    Some(match r {
        Recorded::Started { from, .. } => RecordReport::Started {
            from,
            held: HeldControls {
                sustain: held.sustain.load(Relaxed),
                bend: held.bend.load(Relaxed),
                modulation: held.modulation.load(Relaxed),
            },
        },
        Recorded::Event {
            event,
            t_us,
            tick,
            lap,
        } => RecordReport::Event {
            kind: match event {
                InstrumentEvent::NoteOn { note, velocity } => RecEventKind::NoteOn {
                    key: note,
                    velocity,
                },
                InstrumentEvent::NoteOff { note } => RecEventKind::NoteOff { key: note },
                InstrumentEvent::Control { controller, value } => RecEventKind::Cc {
                    number: controller,
                    value,
                },
                InstrumentEvent::PitchBend { value } => RecEventKind::PitchBend(value),
                InstrumentEvent::AllNotesOff => return None,
            },
            t_us,
            tick,
            lap,
        },
        Recorded::Rolling { frame, tick } => RecordReport::Rolling {
            frame,
            tick,
            file_frame,
        },
        Recorded::LapChanged { lap, frame, .. } => RecordReport::LapChanged {
            lap,
            frame,
            file_frame,
        },
        Recorded::Stopped {
            t_us, tick, lap, ..
        } => RecordReport::Stopped { t_us, tick, lap },
    })
}

fn transport_state(s: meloet_engine::TransportState) -> TransportState {
    match s {
        meloet_engine::TransportState::Stopped => TransportState::Stopped,
        meloet_engine::TransportState::CountIn => TransportState::CountIn,
        meloet_engine::TransportState::Playing => TransportState::Playing,
        meloet_engine::TransportState::Recording => TransportState::Recording,
    }
}

struct Worker {
    model: Model,
    hub: Arc<Hub>,
    shared: Arc<Shared>,
    audio: Audio,
    config_dir: Option<PathBuf>,
    songs_dir: Option<PathBuf>,
    state_dir: Option<PathBuf>,
    recovery: Option<recovery::Writer>,
    /// Waiting for the recovery file, for the PC ending.
    ending: Vec<Sender<()>>,
    #[cfg(feature = "dev-hooks")]
    key_to_sound: Option<KeyToSound>,
    #[cfg(feature = "dev-hooks")]
    plugin_log: Option<PluginLog>,
    #[cfg(feature = "dev-hooks")]
    memory_log: Option<MemoryLog>,
    links: Links,
    midi: Option<Midi>,
    devices: Option<DevicesState>,
    audio_retry_at: Option<Instant>,
    plugins: Option<PluginSide>,
    input: input::InputSide,
    /// Shared with the read-ahead thread (22-8) and with the thread of the audio files.
    audio_store: Arc<AudioStore>,
    /// The thread that reads audio files in.
    keeper: Option<crate::audiofiles::keeper::Keeper>,
    /// The song and the folder of its file that `audio_store` was last told.
    song_told: Option<(u32, Option<PathBuf>)>,
    /// The jobs of the export, each on its thread.
    exports: export::Exports,
    /// The thread that fetches the sound pack.
    fetcher: Option<Fetcher>,
    /// Where the switch 「AI とつなぐ」 goes.
    ai_switch: Option<AiSwitch>,
    /// The AI link's jobs given out.
    ai_jobs: u64,
    /// Who waits for the rev of an intent that waits for a batch of the AI link, with the
    /// intent's number among those that ever waited.
    held_replies: VecDeque<(u64, mpsc::Sender<u64>)>,
}

/// The threads behind the plugins. Dropped when the core ends: scanning children are killed,
/// the scan file is written and every plugin process is ended.
struct PluginSide {
    scanner: Scanner,
    hosts: Hosts,
    /// What `hosts` was started with: an export starts processes of its own the same way.
    options: HostOptions,
    /// Given to the rules when the core thread starts.
    start: Option<Input>,
    /// `Some` until the screen is up (`PluginOptions::wait_for_screen`): the scans asked for
    /// by then, in order.
    held: Option<Vec<ScanJob>>,
    /// The plugins that are loaded and not in the engine yet, by the ticket of their load.
    ready: HashMap<u64, Loaded>,
    /// Which plugin a ticket stands for, from its load until the engine handed it back.
    ids: HashMap<u64, Which>,
    /// Where a plugin the engine handed back tells so (`plugins::housed`).
    gone_tx: Sender<Which>,
    gone_rx: Receiver<Which>,
}

impl PluginSide {
    /// Gives the scans held for the screen to the scanner; from now on none are held.
    fn release(&mut self) {
        for job in self.held.take().into_iter().flatten() {
            self.scanner.scan(job);
        }
    }

    /// The plugin of a ticket, wrapped for the engine. `None` when it is not waiting.
    fn take(&mut self, ticket: u64) -> Option<(meloet_engine::PluginSlot, Which, Sender<Which>)> {
        let loaded = self.ready.remove(&ticket)?;
        let which = *self.ids.get(&ticket)?;
        Some((loaded.slot, which, self.gone_tx.clone()))
    }

    /// News of one plugin, for the rules: which ticket it was loaded under, and what it
    /// said, in order. `None` for a plugin no ticket stands for any more.
    fn said(
        &self,
        news: HostNews,
        heard_at: impl Fn(u64) -> Option<Heard>,
    ) -> Option<(u64, Vec<PluginSaid>)> {
        let (slot, said) = match news {
            HostNews::Stopped { .. } => return None,
            HostNews::WindowOpened {
                slot,
                width,
                height,
                larger_than_screen,
            } => {
                let said = PluginSaid::WindowOpened {
                    width,
                    height,
                    larger_than_screen,
                };
                (slot, said)
            }
            HostNews::WindowFailed { slot, message } => {
                eprintln!("meloet: a plugin's window did not open: {message}");
                (slot, PluginSaid::WindowFailed)
            }
            HostNews::WindowResized {
                slot,
                width,
                height,
                larger_than_screen,
            } => {
                let said = PluginSaid::WindowResized {
                    width,
                    height,
                    larger_than_screen,
                };
                (slot, said)
            }
            HostNews::WindowClosed { slot } => (slot, PluginSaid::WindowClosed),
            HostNews::WindowEntered { slot } => (slot, PluginSaid::WindowEntered),
            HostNews::WindowLeft { slot, touches } => {
                let said = vec![
                    PluginSaid::WindowTouches { touches },
                    PluginSaid::WindowLeft,
                ];
                return self.ticket(slot).map(|ticket| (ticket, said));
            }
            HostNews::Notice { slot, notice } => match notice {
                Notice::ParamChanged {
                    id,
                    value,
                    text,
                    t_us,
                    ..
                } => {
                    let now = ParamNow {
                        id,
                        value: Some(value),
                        text,
                    };
                    let moved = PluginSaid::ParamMoved {
                        param: id,
                        value,
                        heard: heard_at(t_us),
                    };
                    let said = vec![PluginSaid::Param(now), moved];
                    return self.ticket(slot).map(|ticket| (ticket, said));
                }
                Notice::ParamBegin { id, t_us, .. } | Notice::ParamEnd { id, t_us, .. } => {
                    let on = matches!(notice, Notice::ParamBegin { .. });
                    let said = PluginSaid::ParamTouch {
                        param: id,
                        on,
                        t_us,
                    };
                    (slot, said)
                }
                Notice::ParamsChanged { .. } => (slot, PluginSaid::ParamsChanged),
                Notice::ParamAutomated {
                    id, value, text, ..
                } => {
                    let now = ParamNow {
                        id,
                        value: Some(value),
                        text,
                    };
                    (slot, PluginSaid::Param(now))
                }
                // The engine counts the delays by itself.
                _ => return None,
            },
        };
        Some((self.ticket(slot)?, vec![said]))
    }

    fn ticket(&self, slot: SlotId) -> Option<u64> {
        let mut tickets = self.ids.iter();
        tickets.find_map(|(ticket, (id, _))| (*id == slot).then_some(*ticket))
    }

    /// Lets go of the plugins the engine handed back since the last look.
    fn unload_gone(&mut self) {
        while let Ok(which) = self.gone_rx.try_recv() {
            let ticket = (self.ids.iter()).find_map(|(t, w)| (*w == which).then_some(*t));
            // A plugin that was loaded again has another number by now: it stays.
            if let Some(ticket) = ticket {
                self.ids.remove(&ticket);
                self.hosts.unload(which.0, Box::new(|_| {}));
            }
        }
    }
}

impl Worker {
    fn run(mut self, rx: Receiver<Msg>) {
        match self.state_dir.as_deref().and_then(recovery_start) {
            Some(start) => {
                if matches!(
                    &start,
                    RecoveryStart::Read {
                        vocals: Some(_),
                        ..
                    }
                ) {
                    self.input.kept_note = input::KeptNote::Waiting;
                }
                self.handle(Input::RecoveryFound(Box::new(start)));
            }
            // Nothing names what the work place holds (docs/architecture.md 22-4).
            None => self.audio_store.clear_work(),
        }
        // The rules ask for the samples of the song they start with.
        self.handle(Input::Tick);
        if let Some(start) = self.plugins.as_mut().and_then(|p| p.start.take()) {
            self.handle(start);
        }
        let began = Instant::now();
        let mut kept = Instant::now();
        let mut polled = Instant::now();
        loop {
            let wait = if self.model.is_moving() {
                MOVING_TICK
            } else {
                HOUSEKEEPING
            };
            let wake = self.model.wake_at_us();
            let wait = wake.map_or(wait, |at| {
                wait.min(Duration::from_micros(at.saturating_sub(monotonic_us())))
            });
            let wait = if self.input.click_out() {
                wait.min(input::CLICK_TICK)
            } else {
                wait
            };
            match rx.recv_timeout(wait) {
                Ok(Msg::Input(input, reply)) => {
                    let (waited, _) = self.model.held_counts();
                    let rev = self.handle(input);
                    let (waits, served) = self.model.held_counts();
                    match reply {
                        // Answered with the rev after the intent is served.
                        Some(reply) if waits > waited && served <= waited => {
                            self.held_replies.push_back((waits, reply));
                        }
                        Some(reply) => drop(reply.send(rev)),
                        None => {}
                    }
                }
                Ok(Msg::Frames { epoch, actual }) => {
                    if self.current(epoch) {
                        self.handle(Input::AudioFrames(actual));
                    }
                }
                Ok(Msg::StreamLost { epoch }) => {
                    if self.current(epoch) {
                        eprintln!("meloet: the output stream ended by itself");
                        self.handle(Input::AudioLost);
                    }
                }
                Ok(Msg::Xruns { epoch, count }) => {
                    if self.current(epoch) {
                        self.handle(Input::AudioXruns { count });
                    }
                }
                Ok(Msg::Fader) => {
                    let value = self.shared.fader.swap(NO_FADER, Relaxed);
                    if let Ok(value) = u8::try_from(value) {
                        self.handle(Input::KeyboardFader { value });
                    }
                }
                Ok(Msg::MidiStopped { reason }) => {
                    if let Some(midi) = &mut self.midi {
                        midi.stopped(&reason);
                    }
                    self.handle(Input::MidiStopped);
                }
                Ok(Msg::DevicesLost) => {
                    eprintln!("meloet: lost the connection to PipeWire");
                    self.devices = Some(DevicesState::Waiting {
                        attempt: 1,
                        at: Instant::now() + retry_delay(1),
                    });
                    self.handle(Input::AudioSystemLost);
                }
                Ok(Msg::LiveNotes(reply)) => {
                    let _ = reply.send(self.model.live_notes());
                }
                Ok(Msg::TakeNotes(id, reply)) => {
                    let _ = reply.send(self.model.take_notes(id));
                }
                Ok(Msg::Lane(id, reply)) => {
                    let _ = reply.send(self.model.lane(id));
                }
                Ok(Msg::MadeCounts(reply)) => {
                    let audio = &self.audio;
                    let _ = reply.send(MadeCounts {
                        waiting: audio.maker.as_ref().map_or(0, |m| m.waiting()),
                        playing: audio.made.len(),
                        splices: audio.readahead.stats().splices.load(Relaxed),
                    });
                }
                Ok(Msg::PeaksPlace(audio, reply)) => {
                    let growing = self.input.growing_peaks(audio);
                    let _ = reply.send(growing.or_else(|| self.model.peaks_place(audio)));
                }
                Ok(Msg::PcmStore(store)) => {
                    self.audio.readahead.set_store(store);
                    self.audio_files_changed();
                }
                Ok(Msg::HistoryRows(from, to, reply)) => {
                    let _ = reply.send(self.model.history_rows(from, to));
                }
                Ok(Msg::PluginParams(table, reply)) => {
                    let _ = reply.send(self.model.plugin_params(table));
                }
                Ok(Msg::PcEnding(reply)) => {
                    self.ending.push(reply);
                    self.handle(Input::PcEnding);
                }
                Ok(Msg::SoundRead { instrument, read }) => {
                    let input = match read {
                        Ok(bank) => {
                            self.audio.sounds.keep(instrument, bank);
                            Input::SoundLoaded(instrument)
                        }
                        Err(e) => {
                            eprintln!(
                                "meloet: the sound of {} does not read: {e}",
                                instrument.name()
                            );
                            self.audio.sounds.unreadable(instrument);
                            Input::SoundUnreadable(instrument)
                        }
                    };
                    self.handle(input);
                }
                Ok(Msg::Scan(news)) => {
                    self.handle(Input::PluginScan(news));
                }
                Ok(Msg::Plugins(HostNews::Stopped { key, slots })) => {
                    eprintln!(
                        "meloet: the process of {key} stopped ({} plugins)",
                        slots.len()
                    );
                    let ids = self.plugins.iter().flat_map(|side| &side.ids);
                    let mut tickets: Vec<u64> = ids
                        .filter(|(_, (id, _))| slots.contains(id))
                        .map(|(ticket, _)| *ticket)
                        .collect();
                    tickets.sort_unstable();
                    for ticket in tickets {
                        self.handle(Input::PluginStopped { ticket });
                    }
                }
                Ok(Msg::Plugins(news)) => {
                    let heard_at = |t_us| self.heard_at(t_us);
                    let said = (self.plugins.as_ref()).and_then(|side| side.said(news, heard_at));
                    for (ticket, said) in said
                        .into_iter()
                        .flat_map(|(t, all)| all.into_iter().map(move |said| (t, said)))
                    {
                        self.handle(Input::PluginSaid { ticket, said });
                    }
                }
                Ok(Msg::PluginLoaded { ticket, loaded }) => {
                    let input = match (loaded, &mut self.plugins) {
                        (Ok(loaded), Some(side)) => {
                            // Loaded again, it is the same plugin under another number.
                            let which = match side.ids.get(&ticket) {
                                Some((_, times)) => (loaded.id, times + 1),
                                None => (loaded.id, 0),
                            };
                            side.ids.insert(ticket, which);
                            let said = PluginSaid::Editor(match loaded.plugin.editor {
                                EditorKind::X11 => PluginWindowKind::Opens,
                                EditorKind::Unsupported => PluginWindowKind::NotYet,
                                EditorKind::None => PluginWindowKind::None,
                            });
                            side.ready.insert(ticket, loaded);
                            self.handle(Input::PluginSaid { ticket, said });
                            Input::PluginLoaded { ticket }
                        }
                        (Ok(_), None) => Input::PluginUnreadable { ticket },
                        (Err(e), _) => {
                            eprintln!("meloet: a plugin did not load: {e}");
                            Input::PluginUnreadable { ticket }
                        }
                    };
                    self.handle(input);
                }
                Ok(Msg::Made(MadeNews::Ready(keys))) => {
                    self.audio.made_ready(keys);
                    self.audio.want_made();
                }
                Ok(Msg::Made(MadeNews::Work(jobs))) => {
                    self.handle(Input::AudioMaking(jobs));
                }
                Ok(Msg::Sf2(done)) => {
                    let input = match done {
                        Sf2Done::Added(Ok(info)) => {
                            self.audio.sf2.added(info.clone());
                            Input::Sf2Added(Box::new(info))
                        }
                        Sf2Done::Added(Err(file)) => Input::Sf2NotAdded { file },
                        Sf2Done::Loaded(key, Ok(bank)) => {
                            self.audio.sf2.keep(key.clone(), bank);
                            Input::Sf2Loaded(key)
                        }
                        Sf2Done::Loaded(key, Err(e)) => {
                            eprintln!(
                                "meloet: preset {}:{} of {} does not read: {e}",
                                key.bank, key.program, key.file
                            );
                            self.audio.sf2.unreadable(&key);
                            Input::Sf2Unreadable(key)
                        }
                    };
                    self.handle(input);
                }
                Ok(Msg::ScreenReady) => {
                    if let Some(plugins) = &mut self.plugins {
                        plugins.release();
                    }
                    self.handle(Input::ScreenUp);
                }
                Ok(Msg::Pack(news)) => {
                    if let PackNews::Installed {
                        instrument: Some(instrument),
                        json,
                        ..
                    } = &news
                    {
                        self.audio.sounds.add(*instrument, json.clone());
                    }
                    self.handle(Input::Pack(news));
                }
                #[cfg(test)]
                Ok(Msg::Peek(look)) => look(&mut self),
                Ok(Msg::AiSong(reply)) => {
                    let _ = reply.send(self.model.ai_song());
                }
                Ok(Msg::Ai(AiAsk::Preset { place, ask }, reply)) => {
                    self.ask_preset(place, ask, reply);
                }
                Ok(Msg::Ai(ask, reply)) => {
                    self.handle(Input::Ai(ask));
                    let answer = (self.model.take_ai_answer()).map(|a| {
                        crate::play::keynames::heard(a, &self.audio.sounds, &self.audio.sf2)
                    });
                    let _ = reply.send(answer.unwrap_or(AiAnswer::Refused(AiRefusal::Closing)));
                }
                Ok(Msg::AiMeasure { ask, news, reply }) => {
                    let _ = reply.send(self.ai_measure(&ask, news));
                }
                Ok(Msg::AiRender { ask, news, reply }) => {
                    let _ = reply.send(self.ai_render(&ask, news));
                }
                Ok(Msg::AiDrop(job)) => {
                    self.export_ask(ExportAsk::Drop { job });
                }
                Ok(Msg::AiSwitch(switch)) => {
                    switch(self.model.ai_on());
                    self.ai_switch = Some(switch);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            }
            if (self.model.wake_at_us()).is_some_and(|at| at <= monotonic_us()) {
                self.handle(Input::Tick);
            }
            if self.input.click_out() {
                self.poll_input();
            }
            if self.model.is_moving() && polled.elapsed() >= MOVING_TICK {
                polled = Instant::now();
                self.poll_engine();
            }
            if kept.elapsed() >= HOUSEKEEPING {
                kept = Instant::now();
                if began.elapsed() >= SCAN_WAITS_FOR_SCREEN
                    && let Some(plugins) = &mut self.plugins
                {
                    plugins.release();
                }
                self.housekeeping(false);
                self.retries(kept);
                if self.model.recovery_waiting() || self.model.sample_playing() {
                    self.handle(Input::Tick);
                }
            }
        }
        self.housekeeping(true);
        self.close_input();
        // Waits for the recovery file asked for so far.
        drop(self.recovery.take());
        drop(self.midi.take());
        drop(self.devices.take());
        self.audio.close(&self.shared.stream);
        self.exports.end();
        drop(self.plugins.take());
        // What the AI link still waits for gets no answer now, rather than never.
        drop(rx);
        drop(self.ai_switch.take());
    }

    fn ai_measure(
        &mut self,
        ask: &AiMeasure,
        news: Sender<ExportNews>,
    ) -> Result<AiJob, AiRefusal> {
        if self.ai_jobs() > 0 {
            return Err(AiRefusal::Measuring);
        }
        self.ai_jobs += 1;
        let id = export::AI_JOBS + self.ai_jobs;
        let (job, tracks) = self.model.ai_measure_job(ask, id)?;
        let sample_rate = job.sample_rate;
        if let Some(input) = self.export_render(Box::new(job), Some(news)) {
            self.handle(input);
        }
        Ok(AiJob {
            id,
            sample_rate,
            tracks,
        })
    }

    fn ai_render(&mut self, ask: &AiRender, news: Sender<ExportNews>) -> Result<AiJob, AiRefusal> {
        if self.ai_jobs() > 0 {
            return Err(AiRefusal::Measuring);
        }
        self.ai_jobs += 1;
        let id = export::AI_JOBS + self.ai_jobs;
        let (job, target) = self.model.ai_render_job(ask, id)?;
        let sample_rate = target.sample_rate;
        if let Some(input) = self.export_render(Box::new(job), Some(news)) {
            self.handle(input);
        }
        if let Some(input) = self.export_ask(ExportAsk::Finish { job: id, target }) {
            self.handle(input);
        }
        Ok(AiJob {
            id,
            sample_rate,
            tracks: Vec::new(),
        })
    }

    /// Takes in what the engine said about the recording, and what the listener hears now.
    fn poll_engine(&mut self) {
        while let Some(r) = self.audio.handle.pop_recorded() {
            let file_frame = self.vocal_mark(&r);
            if let Some(report) = record_report(r, &self.shared.held, file_frame) {
                self.handle(Input::Record(report));
            }
        }
        if self.model.is_moving() {
            self.handle(Input::Tick);
        }
        self.poll_input();
    }

    fn heard(&self) -> Option<Heard> {
        self.heard_at(monotonic_us())
    }

    /// What was heard at `t_us` (`monotonic_us`): for a value moved in a plugin's own window
    /// at the time the plugin process gave it (docs/architecture.md 21-9).
    fn heard_at(&self, t_us: u64) -> Option<Heard> {
        if !self.shared.stream.open.load(Relaxed) {
            return None;
        }
        self.audio.handle.heard_at(t_us).map(|h| Heard {
            tick: h.tick,
            state: transport_state(h.state),
            lap: h.lap,
        })
    }

    fn current(&self, epoch: u64) -> bool {
        epoch == self.shared.stream.epoch.load(Relaxed)
    }

    /// Starts again what is due to be started again.
    fn retries(&mut self, now: Instant) {
        if let Some(midi) = &mut self.midi {
            midi.tick(now);
        }
        if let Some(DevicesState::Waiting { attempt, at }) = self.devices
            && at <= now
        {
            self.devices = Some(match self.links.device_watcher() {
                Ok(watcher) => DevicesState::Running { _watcher: watcher },
                Err(e) => {
                    if attempt == 1 {
                        eprintln!("meloet: no connection to PipeWire yet: {e}");
                    }
                    DevicesState::Waiting {
                        attempt: attempt + 1,
                        at: now + retry_delay(attempt + 1),
                    }
                }
            });
        }
        if self.audio_retry_at.is_some_and(|at| at <= now) {
            self.audio_retry_at = None;
            self.handle(Input::AudioRetry);
        }
    }

    /// One input, and after it what waited for a batch of the AI link that is closed now:
    /// each as an input of its own, with its own rev. Returns the last rev.
    fn handle(&mut self, input: Input) -> u64 {
        let mut rev = self.handle_now(input);
        while let Some(held) = self.model.take_held() {
            rev = self.handle_now(held);
            let (_, served) = self.model.held_counts();
            while self.held_replies.front().is_some_and(|(n, _)| *n <= served) {
                if let Some((_, reply)) = self.held_replies.pop_front() {
                    let _ = reply.send(rev);
                }
            }
        }
        rev
    }

    /// One input, the effects it asks for, and the inputs those answer with. Returns the rev.
    fn handle_now(&mut self, input: Input) -> u64 {
        let mut inputs = VecDeque::from([input]);
        while let Some(input) = inputs.pop_front() {
            self.model.set_now(monotonic_us(), self.heard());
            let effects = self.model.handle(input);
            // Before anything is asked about the song's audio: where it is looked for.
            self.tell_song_dir();
            for effect in effects {
                match effect {
                    Effect::OpenOutput { node_name } => {
                        self.close_input();
                        inputs.push_back(match self.audio.open(&node_name, &self.shared.stream) {
                            Some(sample_rate) => Input::AudioOpened {
                                node_name,
                                sample_rate,
                            },
                            None => Input::AudioOpenFailed { node_name },
                        });
                        if let Some(feed) = self.audio.rebuilt.take() {
                            *lock(&self.shared.clock) = self.audio.handle.clock().clone();
                            *lock(&self.shared.tempo_map) = map_of(&self.audio.handle);
                            if let Some(midi) = &mut self.midi {
                                midi.feed(self.links.midi_sink(feed));
                            }
                            inputs.push_back(Input::PluginsLost);
                        }
                    }
                    Effect::CloseOutput => {
                        self.close_input();
                        self.audio.close(&self.shared.stream);
                    }
                    Effect::OpenInput {
                        node_name,
                        sink_monitor,
                    } => inputs.push_back(self.open_input(node_name, sink_monitor)),
                    Effect::CloseInput => self.close_input(),
                    Effect::StartVocalRecording {
                        files,
                        format,
                        x_us,
                    } => inputs.push_back(self.start_vocals(files, format, x_us)),
                    Effect::StopVocalRecording => inputs.push_back(self.stop_vocals()),
                    Effect::WriteRecNote(note) => {
                        // The note on the disk is another recording's from here on.
                        self.input.kept_note = input::KeptNote::None;
                        if let Some(dir) = &self.state_dir
                            && let Err(e) = crate::vocalrec::write_note(dir, &note)
                        {
                            eprintln!("meloet: the note about the recording was not written: {e}");
                        }
                    }
                    Effect::RemoveRecNote => {
                        self.input.kept_note = input::KeptNote::None;
                        if let Some(dir) = &self.state_dir {
                            crate::vocalrec::remove_note(dir);
                        }
                    }
                    Effect::DropVocalFiles(names) => self.drop_vocal_files(&names),
                    Effect::LatencyListen(on) => self.latency_listen(on),
                    Effect::LatencyClick => self.latency_click(),
                    Effect::TestTone => self.audio.command(EngineCommand::TestTone),
                    Effect::AllNotesOff => {
                        self.shared.held.release_all();
                        self.audio.command(EngineCommand::AllNotesOff);
                    }
                    Effect::RetryAudio { after } => {
                        self.audio_retry_at = Some(Instant::now() + after);
                    }
                    Effect::Engine(EngineOp::Plug { ticket, spot }) => self.plug(ticket, spot),
                    Effect::Engine(EngineOp::PlugSample { ticket, group }) => {
                        self.plug_sample(ticket, group);
                    }
                    Effect::Engine(op) => {
                        let timeline = matches!(op, EngineOp::Timeline { .. });
                        self.audio.op(op);
                        if timeline {
                            *lock(&self.shared.tempo_map) = map_of(&self.audio.handle);
                        }
                    }
                    Effect::LiveNotes(notes) => self.hub.live_notes(notes),
                    Effect::SaveSettings(settings) => {
                        self.audio.wish_frames = settings.sound.buffer;
                        self.audio.wish_rate = settings.sound.sample_rate;
                        if let Some(dir) = &self.config_dir
                            && let Err(e) = settings::save(dir, &settings)
                        {
                            eprintln!("meloet: settings not saved in {}: {e}", dir.display());
                        }
                    }
                    Effect::SaveRecentSongs(songs) => {
                        if let Some(dir) = &self.state_dir
                            && let Err(e) = crate::recent::save(dir, &songs)
                        {
                            eprintln!("meloet: recent songs not saved in {}: {e}", dir.display());
                        }
                    }
                    Effect::SaveSong { song, to } => {
                        if let Some(saved) = self.save_song(song, to) {
                            inputs.push_back(saved);
                        }
                    }
                    Effect::ReadSong { path, previous } => {
                        inputs.push_back(read_song(path, previous));
                    }
                    Effect::PickSongFile => {
                        let folder = self.start_folder();
                        if !self.ask_shell(ShellRequest::PickSongFile { folder }) {
                            inputs.push_back(Input::SongFilePicked(None));
                        }
                    }
                    Effect::PickFolder => {
                        let folder = self.start_folder();
                        if !self.ask_shell(ShellRequest::PickFolder { folder }) {
                            inputs.push_back(Input::FolderPicked(None));
                        }
                    }
                    Effect::PickSaveAs { name } => {
                        let folder = self.start_folder();
                        if !self.ask_shell(ShellRequest::PickSaveAs { folder, name }) {
                            inputs.push_back(Input::SaveAsPicked(None));
                        }
                    }
                    Effect::Exit => {
                        self.ask_shell(ShellRequest::Exit);
                    }
                    Effect::WriteRecovery {
                        song,
                        path,
                        cause,
                        ending,
                    } => {
                        let mut done = if ending {
                            std::mem::take(&mut self.ending)
                        } else {
                            Vec::new()
                        };
                        if matches!(self.input.kept_note, input::KeptNote::Waiting) {
                            let (tx, written) = mpsc::channel();
                            done.push(tx);
                            self.input.kept_note = input::KeptNote::Writing(written);
                        }
                        match &self.recovery {
                            Some(writer) => writer.write(song, path, cause, done),
                            None => {
                                for d in done {
                                    let _ = d.send(());
                                }
                            }
                        }
                    }
                    Effect::RemoveRecovery => {
                        if let Some(writer) = &self.recovery {
                            writer.remove();
                        }
                        // The song brought back was let go of, the kept recording with it.
                        if !matches!(self.input.kept_note, input::KeptNote::None) {
                            self.input.kept_note = input::KeptNote::None;
                            if let Some(dir) = &self.state_dir {
                                crate::vocalrec::remove_note(dir);
                            }
                        }
                    }
                    Effect::EndingReady => {
                        for d in self.ending.drain(..) {
                            let _ = d.send(());
                        }
                    }
                    Effect::ShowFile(path) => {
                        self.ask_shell(ShellRequest::ShowFile { path });
                    }
                    Effect::LoadSound(instrument) => self.audio.sounds.load(instrument),
                    Effect::DropSound(instrument) => self.audio.sounds.let_go(instrument),
                    Effect::ScanPlugins(job) => {
                        if let Some(plugins) = &mut self.plugins {
                            match &mut plugins.held {
                                Some(held) => held.push(*job),
                                None => plugins.scanner.scan(*job),
                            }
                        }
                    }
                    Effect::SavePluginScan(cache) => {
                        if let Some(plugins) = &self.plugins {
                            plugins.scanner.save(*cache);
                        }
                    }
                    Effect::PickPluginPlace => {
                        let folder = dirs::home_dir().filter(|d| d.is_dir());
                        if !self.ask_shell(ShellRequest::PickPluginPlace { folder }) {
                            inputs.push_back(Input::PluginPlacePicked(None));
                        }
                    }
                    Effect::LoadPlugin {
                        ticket,
                        key,
                        bundle,
                        state,
                        sample_rate,
                    } => {
                        if !self.load_plugin(ticket, &key, bundle, state, sample_rate) {
                            inputs.push_back(Input::PluginUnreadable { ticket });
                        }
                    }
                    Effect::DropPlugin { ticket } => {
                        if let Some(side) = &mut self.plugins
                            && let Some(loaded) = side.ready.remove(&ticket)
                        {
                            side.ids.remove(&ticket);
                            // The engine's end first; it never was in the engine.
                            let id = loaded.id;
                            drop(loaded);
                            side.hosts.unload(id, Box::new(|_| {}));
                        }
                    }
                    Effect::SetPluginState { ticket, state } => {
                        if let Some(side) = &self.plugins
                            && let Some((id, _)) = side.ids.get(&ticket)
                        {
                            side.hosts.set_state(*id, state.to_vec(), Box::new(|_| {}));
                        }
                    }
                    Effect::ReloadPlugin { ticket } => {
                        let id = (self.plugins.as_ref())
                            .and_then(|side| Some((side, side.ids.get(&ticket)?.0)));
                        match id {
                            Some((side, id)) => {
                                let block = self.audio.handle.plugin_block().clone();
                                side.hosts.reload(id, block, self.loaded_sink(ticket));
                            }
                            None => inputs.push_back(Input::PluginUnreadable { ticket }),
                        }
                    }
                    Effect::AskPlugin { ticket, ask } => {
                        if let Some(lost) = self.ask_plugin(ticket, ask) {
                            inputs.push_back(Input::PluginSaid { ticket, said: lost });
                        }
                    }
                    Effect::ParamValues(values) => self.hub.param_values(values),
                    Effect::FileEvent(event) => self.hub.event(event),
                    Effect::LoadSf2(key) => {
                        let rate = self.audio.handle.meters().sample_rate();
                        self.audio.sf2.load(key, rate);
                    }
                    Effect::DropSf2(key) => self.audio.sf2.let_go(&key),
                    Effect::PickSf2 => {
                        let folder = dirs::home_dir().filter(|d| d.is_dir());
                        if !self.ask_shell(ShellRequest::PickSf2 { folder }) {
                            inputs.push_back(Input::Sf2Picked(None));
                        }
                    }
                    Effect::AddSf2(path) => {
                        if !self.audio.sf2.add(path.clone()) {
                            let file = path.file_name().unwrap_or_default();
                            inputs.push_back(Input::Sf2NotAdded {
                                file: file.to_string_lossy().into_owned(),
                            });
                        }
                    }
                    Effect::PickAudio { many } => {
                        let folder =
                            (dirs::audio_dir().or_else(dirs::home_dir)).filter(|d| d.is_dir());
                        if !self.ask_shell(ShellRequest::PickAudio { folder, many }) {
                            inputs.push_back(Input::AudioPicked(Vec::new()));
                        }
                    }
                    Effect::BringAudio {
                        ticket,
                        paths,
                        taken,
                    } => match &self.keeper {
                        Some(keeper) => keeper.bring(ticket, paths, taken),
                        None => inputs.push_back(Input::Audio(AudioNews::Brought {
                            ticket,
                            files: Vec::new(),
                        })),
                    },
                    Effect::WatchAudio {
                        serial,
                        sources,
                        sample_rate,
                    } => {
                        // First the peaks a recording made while it was written: the thread
                        // of the audio files then finds them and does not make them again.
                        self.keep_recorded_peaks();
                        if let Some(keeper) = &self.keeper {
                            keeper.watch(serial, sources, sample_rate);
                        }
                    }
                    Effect::FindAudio { source, path } => {
                        if let Some(keeper) = &self.keeper {
                            keeper.find(source, path);
                        }
                    }
                    // Here and now: meloet may end right after.
                    Effect::ClearWorkAudio => self.audio_store.clear_work(),
                    Effect::Export(ask) => inputs.extend(self.export_ask(ask)),
                    Effect::AiLink(on) => {
                        if let Some(switch) = &self.ai_switch {
                            switch(on);
                        }
                    }
                    Effect::Pack(ask) => {
                        if let Some(fetcher) = &self.fetcher {
                            match ask {
                                PackAsk::Job(job) => fetcher.ask(job),
                                PackAsk::Stop { round } => fetcher.stop(round),
                            }
                        }
                    }
                }
            }
            if let Some(side) = &mut self.plugins {
                side.unload_gone();
            }
        }
        self.audio.want_made();
        self.shared
            .stream
            .wants_sound
            .store(self.model.wants_sound(), Relaxed);
        self.shared
            .stream
            .wants_loading_key
            .store(self.model.wants_loading_key(), Relaxed);
        self.shared
            .fixed_velocity
            .store(self.model.fixed_velocity().unwrap_or(0), Relaxed);
        let seats = self.model.track_seats();
        let mut told = lock(&self.shared.stream.track_seats);
        if *told != seats {
            *told = seats;
        }
        drop(told);
        self.sync_mics();
        self.hub.publish(self.model.view())
    }

    /// Where the answer to a load goes: to the core thread, from the plugins' thread.
    fn loaded_sink(&self, ticket: u64) -> Box<dyn FnOnce(Result<Loaded, HostError>) + Send> {
        let tx = self.links.tx.clone();
        Box::new(move |loaded| {
            let _ = tx.send(Msg::PluginLoaded { ticket, loaded });
        })
    }

    /// [`Effect::AskPlugin`]: the answer comes from the plugins' thread, as an input. Returns
    /// the answer when there is no plugin to ask and the rules wait for one.
    fn ask_plugin(&self, ticket: u64, ask: PluginAsk) -> Option<PluginSaid> {
        let side = self.plugins.as_ref();
        let Some((side, id)) = side.and_then(|side| Some((side, side.ids.get(&ticket)?.0))) else {
            return match ask {
                PluginAsk::State(ask) => Some(PluginSaid::State { ask, state: None }),
                PluginAsk::PutState { .. } => Some(PluginSaid::StatePut(Err(HostError::Gone))),
                PluginAsk::OpenWindow { .. } => Some(PluginSaid::WindowFailed),
                _ => None,
            };
        };
        let tx = self.links.tx.clone();
        let say = move |said| {
            let _ = tx.send(Msg::Input(Input::PluginSaid { ticket, said }, None));
        };
        let hosts = &side.hosts;
        match ask {
            PluginAsk::OpenWindow { title } => {
                let done = move |open: Result<_, HostError>| {
                    if let Err(e) = open {
                        eprintln!("meloet: a plugin's window was not opened: {e}");
                        say(PluginSaid::WindowFailed);
                    }
                };
                hosts.open_window(id, title, Box::new(done));
            }
            PluginAsk::CloseWindow => hosts.close_window(id, Box::new(|_| {})),
            PluginAsk::Params { ask } => {
                let done = move |params: Result<_, HostError>| match params {
                    Ok(params) => say(PluginSaid::Params { ask, params }),
                    Err(e) => eprintln!("meloet: a plugin did not list its parameters: {e}"),
                };
                hosts.params(id, Box::new(done));
            }
            PluginAsk::SetParam { param, value } => {
                let done = move |now: Result<_, HostError>| {
                    if let Ok(now) = now {
                        say(PluginSaid::Param(now));
                    }
                };
                hosts.set_param(id, param, value, Box::new(done));
            }
            PluginAsk::State(ask) => {
                let done = move |state: Result<Vec<u8>, HostError>| {
                    let state = state.ok().map(Arc::from);
                    say(PluginSaid::State { ask, state });
                };
                hosts.state(id, Box::new(done));
            }
            PluginAsk::KeyNames => {
                let done = move |names: Result<_, HostError>| match names {
                    Ok(names) => say(PluginSaid::KeyNames(names)),
                    Err(e) => eprintln!("meloet: a plugin did not name its keys: {e}"),
                };
                hosts.key_names(id, Box::new(done));
            }
            PluginAsk::PutState { state } => {
                // Answers come in order: the state is asked for once the plugin has taken it.
                let refused = Arc::new(AtomicBool::new(false));
                let (refusing, say) = (refused.clone(), Arc::new(say));
                let say_set = say.clone();
                let set = move |put: Result<(), HostError>| {
                    if let Err(e) = put {
                        refusing.store(true, Relaxed);
                        say_set(PluginSaid::StatePut(Err(e)));
                    }
                };
                let got = move |state: Result<Vec<u8>, HostError>| {
                    if !refused.load(Relaxed) {
                        say(PluginSaid::StatePut(state.map(Arc::from)));
                    }
                };
                hosts.set_state(id, state.to_vec(), Box::new(set));
                hosts.state(id, Box::new(got));
            }
            PluginAsk::ReadText { param, text } => {
                let done = move |read: Result<_, HostError>| {
                    if let Ok(read) = read {
                        say(PluginSaid::TextRead { param, read });
                    }
                };
                hosts.read_text(id, param, text, Box::new(done));
            }
            PluginAsk::Texts { ask, param, values } => {
                let done = move |texts: Result<_, HostError>| {
                    if let Ok(texts) = texts {
                        say(PluginSaid::Texts { ask, texts });
                    }
                };
                hosts.texts(id, param, values, Box::new(done));
            }
            PluginAsk::Watch { on } => hosts.watch(id, on),
        }
        None
    }

    /// [`AiAsk::Preset`]: the plugin's host answers the AI link from the plugins' thread. It
    /// changes nothing of the song, so the rules are not asked.
    fn ask_preset(&self, place: PluginPlace, ask: PresetAsk, reply: Sender<AiAnswer>) {
        let ticket = (self.model.plugin_tickets().into_iter()).find(|(p, _)| *p == place);
        let side = self.plugins.as_ref();
        let id = ticket.and_then(|(_, ticket)| Some(side?.ids.get(&ticket)?.0));
        let (Some(side), Some(id)) = (side, id) else {
            let _ = reply.send(AiAnswer::Refused(AiRefusal::PluginNotLoaded));
            return;
        };
        let done = move |said: Result<PresetSaid, HostError>| {
            let _ = reply.send(match said {
                Ok(said) => AiAnswer::Preset(said),
                Err(HostError::Refused(why)) => AiAnswer::Refused(AiRefusal::Cannot(why)),
                Err(_) => AiAnswer::Refused(AiRefusal::PluginNotLoaded),
            });
        };
        side.hosts.preset(id, ask, Box::new(done));
    }

    /// [`Effect::LoadPlugin`]. Returns whether the load was asked for.
    fn load_plugin(
        &self,
        ticket: u64,
        key: &str,
        bundle: PathBuf,
        state: PlugState,
        sample_rate: u32,
    ) -> bool {
        let (Some(side), Ok(key)) = (&self.plugins, key.parse::<PluginKey>()) else {
            return false;
        };
        let start = match state {
            PlugState::Fresh => StartState::Fresh,
            PlugState::Bytes(state) => StartState::Bytes(state.to_vec()),
            PlugState::Of(other) => match side.ids.get(&other) {
                Some((id, _)) => StartState::Of(*id),
                None => StartState::Fresh,
            },
        };
        let spec = LoadSpec {
            key,
            bundle,
            sample_rate,
        };
        let block = self.audio.handle.plugin_block().clone();
        (side.hosts).load_with(spec, start, block, self.loaded_sink(ticket));
        true
    }

    /// `EngineOp::Plug`: the plugin loaded under the ticket goes into the engine.
    fn plug(&mut self, ticket: u64, spot: Spot) {
        let Some((slot, which, tell)) = self.plugins.as_mut().and_then(|s| s.take(ticket)) else {
            return;
        };
        match spot {
            Spot::Seat(seat) | Spot::Trial(seat) => {
                let plugin = Box::new(HousedInstrument::new(slot, which, tell));
                (self.audio).plug_instrument(seat, matches!(spot, Spot::Trial(_)), plugin);
            }
            Spot::Insert(strip, slot_at) => {
                let plugin = Box::new(HousedEffect::new(slot, which, tell));
                self.audio.plug_effect(strip, slot_at, plugin);
            }
            // A sample comes with its phrase (`EngineOp::PlugSample`).
            Spot::Sample => {}
        }
    }

    /// `EngineOp::PlugSample`: the plugin plays the phrase of the group on the sample seat.
    fn plug_sample(&mut self, ticket: u64, group: InstrumentGroup) {
        let Some((slot, which, tell)) = self.plugins.as_mut().and_then(|s| s.take(ticket)) else {
            return;
        };
        let plugin = Box::new(HousedInstrument::new(slot, which, tell));
        if self.audio.stream.is_some() {
            let player = PhrasePlayer::new(plugin, phrases::for_group(group));
            if self.audio.handle.play_sample(Box::new(player)).is_err() {
                eprintln!("meloet: the engine did not take the sample");
            }
        }
    }

    /// Tells the audio store where the song's file is, when that changed: another song
    /// (opened, new), or the same song in another folder (saved, saved under another name).
    fn tell_song_dir(&mut self) {
        let now = (self.model.song_serial(), self.model.song_dir());
        if self.song_told.as_ref() == Some(&now) {
            return;
        }
        let same_song = self
            .song_told
            .as_ref()
            .is_some_and(|(serial, _)| *serial == now.0);
        match (same_song, &now.1) {
            (true, Some(dir)) => self.audio_store.song_moved(dir.clone()),
            (_, dir) => self.audio_store.set_song_dir(dir.clone()),
        }
        self.song_told = Some(now);
    }

    /// [`Effect::SaveSong`]. With plugins in the song their states are asked for first, all
    /// at once, and the song is written on a thread of its own once they have answered or
    /// [`STATE_WAIT`] is over: the answer then comes later, as a message. A song with audio
    /// is written on that thread too, saying how far the files are put in place
    /// (`Input::SavingAudio`). Without either the song is written here and the answer is
    /// returned.
    fn save_song(&mut self, song: Box<Song>, to: SaveTo) -> Option<Input> {
        let songs_dir = self.songs_dir.clone();
        let asked: Vec<(PluginPlace, SlotId)> = match &self.plugins {
            Some(side) => (self.model.plugin_tickets().into_iter())
                .filter_map(|(place, ticket)| Some((place, side.ids.get(&ticket)?.0)))
                .collect(),
            None => Vec::new(),
        };
        let store = self.audio_store.clone();
        let side = self.plugins.as_ref().filter(|_| !asked.is_empty());
        if side.is_none() && songfile::used_audio(&song.doc).is_empty() {
            return Some(save_song(
                songs_dir.as_deref(),
                &song,
                to,
                &store,
                &mut |_| {},
            ));
        }
        let (states, answers) = mpsc::channel();
        if let Some(side) = side {
            for &(place, id) in &asked {
                let states = states.clone();
                let done = move |state| {
                    let _ = states.send((place, state));
                };
                side.hosts.state(id, Box::new(done));
            }
        }
        drop(states);
        let tx = self.links.tx.clone();
        let saver = std::thread::Builder::new()
            .name("meloet-save".into())
            .spawn(move || {
                let until = Instant::now() + STATE_WAIT;
                let mut song = *song;
                // Ends early when every plugin has answered: the senders are gone then.
                while let Ok((place, state)) =
                    answers.recv_timeout(until.saturating_duration_since(Instant::now()))
                {
                    if let Ok(state) = state {
                        with_plugin_state(&mut song, place, state);
                    }
                }
                let mut tell = |p: Settling| {
                    let input = Input::SavingAudio {
                        done: p.done,
                        total: p.total,
                        done_bytes: p.done_bytes,
                        total_bytes: p.total_bytes,
                    };
                    let _ = tx.send(Msg::Input(input, None));
                };
                let saved = save_song(songs_dir.as_deref(), &song, to, &store, &mut tell);
                let _ = tx.send(Msg::Input(saved, None));
            });
        match saver {
            Ok(_) => None,
            Err(e) => {
                eprintln!("meloet: the song was not saved: {e}");
                Some(Input::SongNotSaved)
            }
        }
    }

    /// Returns whether there was a shell to ask.
    fn ask_shell(&self, request: ShellRequest) -> bool {
        match &*lock(&self.shared.shell) {
            Some(shell) => {
                shell(request);
                true
            }
            None => false,
        }
    }

    /// The folder of the song file in use, or else the songs folder.
    fn start_folder(&self) -> Option<PathBuf> {
        self.model
            .song_path()
            .and_then(|p| p.parent())
            .map(PathBuf::from)
            .or_else(|| self.songs_dir.clone())
            .filter(|d| d.is_dir())
    }
}

/// The song with `state` as the state of the plugin at `place`: what a save writes. The
/// song itself keeps the state it knows.
fn with_plugin_state(song: &mut Song, place: PluginPlace, state: Vec<u8>) {
    let Some(key) = song.doc.plugin(place).map(|p| p.key.clone()) else {
        return;
    };
    if state.is_empty() {
        return;
    }
    if let Ok(edit) = set_plugin_state(&song.doc, place, &key, state.into(), PluginChange::Window) {
        song.doc = edit.doc;
    }
}

/// Does [`Effect::SaveSong`] and gives its answer. The audio the song names is put into
/// the song's folder first, then the song's file is written (docs/architecture.md 22-4).
/// `tell` hears how far that is ([`AudioStore::settle`](crate::audiofiles::AudioStore)).
pub(crate) fn save_song(
    songs_dir: Option<&Path>,
    song: &meloet_model::Song,
    to: SaveTo,
    store: &crate::audiofiles::AudioStore,
    tell: &mut dyn FnMut(Settling),
) -> Input {
    // Only what a clip plays: the audio of deleted clips stays in the work place.
    let names: Vec<String> = (songfile::used_audio(&song.doc).iter())
        .map(|a| a.file.clone())
        .collect();
    let saved = match to {
        SaveTo::File(path) => {
            let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
            store
                .settle(dir.unwrap_or(Path::new(".")), &names, tell)
                .and_then(|()| songfile::save(&path, song))
                .map(|()| (path, song.doc.name.clone()))
        }
        SaveTo::NewFolder(folder) => match folder.as_deref().or(songs_dir) {
            Some(folder) => {
                songfile::save_new_with(folder, song, |dir| store.settle(dir, &names, tell))
            }
            None => Err(std::io::Error::other("no songs folder")),
        },
    };
    match saved {
        Ok((path, name)) => Input::SongSaved { path, name },
        Err(e) => {
            eprintln!("meloet: the song was not saved: {e}");
            Input::SongNotSaved
        }
    }
}

/// Does [`Effect::ReadSong`] and gives its answer.
pub(crate) fn read_song(path: PathBuf, previous: bool) -> Input {
    let from = if previous {
        songfile::previous_path(&path)
    } else {
        path.clone()
    };
    match songfile::read(&from) {
        Ok(song) => Input::SongRead {
            path,
            song: Box::new(song),
            from_previous: previous,
        },
        Err(e) => {
            eprintln!("meloet: cannot read {}: {e:?}", from.display());
            let previous = !previous && songfile::read(&songfile::previous_path(&path)).is_ok();
            Input::SongUnreadable { path, previous }
        }
    }
}

/// Reads the recovery file in `dir`, and the song's file it names.
pub(crate) fn recovery_start(dir: &Path) -> Option<RecoveryStart> {
    let read_saved = |path: &Path| {
        songfile::read(path)
            .inspect_err(|e| eprintln!("meloet: cannot read {}: {e:?}", path.display()))
            .ok()
            .map(Box::new)
    };
    let found = recovery::find(dir);
    // A recording of the input that was going on when meloet ended (docs/architecture.md
    // 22-7). With a recovery file that does not read, the note waits for the next start.
    let note = match found {
        Some(recovery::Found::Unreadable { .. }) => None,
        _ => crate::vocalrec::read_note(dir),
    };
    let found = match (found, &note) {
        (Some(found), _) => found,
        (None, None) => return None,
        // Only the recording was left: as a recovery file written now, of the song as saved.
        (None, Some(note)) => {
            let path = note.song.as_ref().map(PathBuf::from);
            let song =
                (path.as_deref().and_then(read_saved)).map_or_else(Song::new_default, |s| *s);
            recovery::Found::Read(Box::new(recovery::Recovered {
                song,
                path,
                cause: meloet_protocol::RecoveryCause::Sudden,
                written_ms: recovery::now_ms(),
            }))
        }
    };
    Some(match found {
        recovery::Found::Read(r) => {
            let mut song = r.song;
            // The note stays while it is the only place that knows where the clips belong:
            // until the recovery file holds them (`input::KeptNote`).
            let vocals = note.and_then(|note| {
                let kept = input::keep_vocals(dir, &note, &mut song.doc);
                if kept.is_none() {
                    crate::vocalrec::remove_note(dir);
                }
                kept
            });
            RecoveryStart::Read {
                saved: r.path.as_deref().and_then(read_saved),
                song: Box::new(song),
                path: r.path,
                cause: r.cause,
                written_ms: r.written_ms,
                vocals,
            }
        }
        recovery::Found::Unreadable { kept, unread } => {
            eprintln!(
                "meloet: the recovery file did not read; kept as {}",
                kept.display()
            );
            RecoveryStart::Unreadable {
                kept,
                saved: unread
                    .path
                    .and_then(|p| read_saved(&p).map(|song| (p, song))),
            }
        }
    })
}

impl Worker {
    /// The place the audio files are opened from has other files now (a file was brought
    /// in or made ready, the song's folder is another one): the vocal clips are read
    /// again from where the song is.
    fn audio_files_changed(&mut self) {
        self.audio.readahead.store_changed();
        self.audio.resend_clips();
    }

    #[cfg_attr(not(feature = "dev-hooks"), allow(unused_variables))]
    fn housekeeping(&mut self, last: bool) {
        self.poll_engine();
        self.audio.handle.drop_retired();
        self.audio.sounds.give_back_freed();
        if self.audio.log_missing() {
            self.handle(Input::DiskTooSlow);
        }
        self.audio.follow_store();
        self.audio.follow_made();
        if let (Some(maker), Some(heard)) = (&self.audio.maker, self.audio.handle.heard_now()) {
            maker.set_near(heard.tick.max(0.0) as u64);
        }
        self.audio.want_made();
        if let Some(side) = &mut self.plugins {
            side.unload_gone();
        }
        if let Some(input) = self.export_tick() {
            self.handle(input);
        }
        let mut samples = Vec::new();
        while let Some(record) = self.audio.handle.pop_latency() {
            if let Some(ms) = key_to_sound_ms(record) {
                samples.push(ms);
                #[cfg(feature = "dev-hooks")]
                if let Some(log) = &mut self.key_to_sound {
                    log.add(record);
                }
            }
        }
        for ms in samples {
            self.handle(Input::LatencySample { ms });
        }
        #[cfg(feature = "dev-hooks")]
        if let Some(log) = &mut self.key_to_sound {
            let state = self.hub.snapshot().state.audio;
            log.write(
                last,
                state.sample_rate.unwrap_or(0),
                state.requested_frames,
                state.actual_frames,
            );
        }
        #[cfg(feature = "dev-hooks")]
        if let (Some(log), Some(side)) = (&mut self.plugin_log, &self.plugins) {
            log.look(last, || side.hosts.processes());
        }
        #[cfg(feature = "dev-hooks")]
        if let Some(log) = &mut self.memory_log {
            let (model, audio) = (&self.model, &self.audio);
            log.look(last, || {
                let bank = |name: String, bytes: usize, holders: usize| {
                    serde_json::json!({"name": name, "mb": bytes as f64 / 1048576.0, "holders": holders})
                };
                let sounds = audio.sounds.held().into_iter();
                let sf2 = audio.sf2.held().into_iter();
                serde_json::json!({
                    "sounds": sounds.map(|(i, b, h)| bank(i.name().to_owned(), b, h)).collect::<Vec<_>>(),
                    "sf2": sf2.map(|(k, b, h)| bank(k, b, h)).collect::<Vec<_>>(),
                    "model": model.memory_counts(),
                    "malloc": meloet_sound::memory::malloc_counts().map(|m| serde_json::json!({
                        "inUseMb": m.in_use as f64 / 1048576.0,
                        "freeMb": m.free as f64 / 1048576.0,
                        "mappedMb": m.mapped as f64 / 1048576.0,
                    })),
                })
            });
        }
    }
}

/// The map the engine was last sent, and its number.
fn map_of(handle: &EngineHandle) -> (Arc<TempoMap>, u32) {
    let (map, map_gen) = handle.tempo_map();
    (map.clone(), map_gen)
}

fn key_to_sound_ms(record: LatencyRecord) -> Option<f32> {
    let us = record.playback_us.checked_sub(record.key_us)?;
    (us < LATENCY_SANE_US).then(|| us as f32 / 1000.0)
}

/// 60 Hz: sends the readings unless they repeat what was sent last, and tells the core thread
/// what it sees of the stream.
fn telemetry_loop(hub: &Hub, shared: &Shared, meters: &EngineMeters, tx: &Sender<Msg>) {
    let mut seq = 0u32;
    let mut last: Option<Telemetry> = None;
    let mut bytes = Vec::new();
    let mut told_frames = (0u64, None);
    let mut told_lost = None;
    let mut told_xruns = (0u64, 0u64);
    let mut next = Instant::now();
    let mut seat_peaks = vec![[0.0f32; 2]; meters.track_count() + 1];
    let mut said_loud = None;

    while !shared.stop.load(Relaxed) {
        let stream = &shared.stream;
        // The epoch first: if the stream changes while the rest is read, the core thread
        // sees an old epoch and drops the report.
        let epoch = stream.epoch.load(Relaxed);
        let open = stream.open.load(Relaxed);
        let frames = (open && meters.callbacks() > stream.callbacks_at_open.load(Relaxed))
            .then(|| meters.last_frames());
        if told_frames != (epoch, frames) {
            told_frames = (epoch, frames);
            let _ = tx.send(Msg::Frames {
                epoch,
                actual: frames,
            });
        }
        if open {
            if meters.streams_lost() > stream.lost_at_open.load(Relaxed) && told_lost != Some(epoch)
            {
                told_lost = Some(epoch);
                let _ = tx.send(Msg::StreamLost { epoch });
            }
            let xruns = meters
                .xruns()
                .saturating_sub(stream.xruns_at_open.load(Relaxed));
            if xruns > 0 && told_xruns != (epoch, xruns) {
                told_xruns = (epoch, xruns);
                let _ = tx.send(Msg::Xruns {
                    epoch,
                    count: xruns,
                });
            }
        }

        // Every track of the engine is read, so that a seat with no track of the song (the
        // audition, a track just deleted) leaves nothing behind for the track that gets it next.
        for (seat, peak) in seat_peaks.iter_mut().enumerate() {
            *peak = meters.take_track_peak(seat);
        }
        let track_peaks = peaks_in_song_order(&lock(&stream.track_seats), &seat_peaks);
        // Track peaks, not the master: the test tone is not what the keys played.
        let heard = seat_peaks.iter().flatten().any(|p| *p > SOUND_THRESHOLD);
        if open && heard && stream.wants_sound.swap(false, Relaxed) {
            let _ = tx.send(Msg::Input(Input::SoundHeard, None));
        }

        let transport = if open {
            let clock = lock(&shared.clock).clone();
            let (map, map_gen) = lock(&shared.tempo_map).clone();
            clock.heard_now(&map, map_gen).map(|h| TransportReading {
                tick: h.tick,
                ticks_per_us: h.ticks_per_us,
                state: transport_state(h.state),
                count_in_beats_left: h.count_in_beats_left.min(u32::from(u16::MAX)) as u16,
                lap: h.lap,
            })
        } else {
            None
        };
        let now = Telemetry {
            seq,
            sent_ns: monotonic_us() * 1000,
            keys_low: shared.held.keys_low.load(Relaxed),
            keys_high: shared.held.keys_high.load(Relaxed),
            last_velocity: shared.held.last_velocity.load(Relaxed),
            sustain: shared.held.sustain.load(Relaxed),
            modulation: shared.held.modulation.load(Relaxed),
            running: open,
            master_clipped: meters.take_clipped(),
            pitch_bend: shared.held.bend.load(Relaxed),
            load_permille: steady_load(open, meters.load_permille(), last.as_ref()),
            master_peak: meters.take_master_peak(),
            transport,
            return_peaks: [meters.take_return_peak(0), meters.take_return_peak(1)],
            track_peaks,
            mics: {
                let (mics, clipped) = input::mic_meters(shared);
                if clipped && said_loud.is_none_or(|at: Instant| at.elapsed() >= MIC_LOUD_EVERY) {
                    said_loud = Some(Instant::now());
                    let _ = tx.send(Msg::Input(Input::MicClipped, None));
                }
                mics
            },
        };
        let fresh = hub.take_telemetry_fresh();
        if fresh || !last.as_ref().is_some_and(|l| l.same_readings(&now)) {
            now.encode_into(&mut bytes);
            hub.telemetry(&bytes);
            seq = seq.wrapping_add(1);
            last = Some(now);
        }

        next += TELEMETRY_TICK;
        let wait = next.saturating_duration_since(Instant::now());
        if wait.is_zero() {
            next = Instant::now();
        }
        std::thread::sleep(wait);
    }
}

/// The meters of the song's tracks in the order of the song, from the peaks of the engine's
/// tracks. A track with no seat reads 0.
fn peaks_in_song_order(seats: &[Option<usize>], seat_peaks: &[[f32; 2]]) -> Vec<[f32; 2]> {
    seats
        .iter()
        .map(|seat| {
            seat.and_then(|s| seat_peaks.get(s))
                .copied()
                .unwrap_or_default()
        })
        .collect()
}

/// The load to send. It moves a little from block to block even in silence; a change smaller
/// than `LOAD_STEP` keeps the value sent last, so that silence sends nothing.
fn steady_load(open: bool, permille: u32, last: Option<&Telemetry>) -> u16 {
    if !open {
        return 0;
    }
    let now = permille.min(u32::from(u16::MAX)) as u16;
    match last {
        Some(last) if now.abs_diff(last.load_permille) < LOAD_STEP => last.load_permille,
        _ => now,
    }
}

mod export;
#[cfg(test)]
mod export_tests;
mod input;
mod keys;
#[cfg(test)]
mod plug_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod window_tests;
