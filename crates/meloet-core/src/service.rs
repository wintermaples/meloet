//! The moving part: the threads around the pure rules.
//!
//! Only the core thread changes the state. The MIDI thread, the device watcher and the
//! telemetry thread send it what they learn over one channel. The core thread owns the MIDI
//! input and the device watcher, and starts them again when they stop by themselves.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU8, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_devices::{DeviceWatcher, DevicesError, DevicesEvent};
use meloet_engine::{
    AudioOutput, Chord, Engine, EngineCommand, EngineConfig, EngineHandle, EngineMeters,
    Instrument, InstrumentEvent, LatencyRecord, MidiFeed, MidiIn, OutputTarget, Recorded,
    RunningOutput, Sequence, SequenceEvent, TrackSet, TransportClock, monotonic_us,
};
use meloet_midi::{MidiEvent, MidiInput, MidiMessage, MidiOptions, MidiSink};
use meloet_model::playback::{PlayEvent, PlayKind};
use meloet_model::record::{HeldControls, RecEventKind};
use meloet_model::{BuiltinInstrument, ControlValue, InstrumentRef};
use meloet_plugin::ScanCache;
use meloet_protocol::{
    Command, Event, LiveNotesSnapshot, Snapshot, Telemetry, TransportReading, TransportState,
};
use meloet_sound::phrases::{self, PhrasePlayer};
use meloet_sound::sampler::{LoadError, SampleBank};

use crate::hub::{Hub, MessageSink, TelemetrySink};
use crate::instruments;
use crate::measure::{self, KeyToSound};
use crate::mixbook::{self, MixBook};
use crate::play::{AUDITION_SEAT, EngineOp, Heard, MAX_TRACKS, RecordReport};
use crate::plugins::hosts::{DEFAULT_GEOMETRY, HostNews, HostOptions, Hosts};
use crate::plugins::scanner::Scanner;
use crate::plugins::{PluginOptions, ScanNews};
use crate::recovery;
use crate::settings::{self, Settings};
use crate::songfile;
use crate::sounds::{self, Library};
use crate::state::{Effect, Input, Model, REQUESTED_FRAMES, RecoveryStart, SaveTo, retry_delay};

const TELEMETRY_TICK: Duration = Duration::from_micros(16_667);
/// How often the core thread reads the engine's reports while the transport moves.
const MOVING_TICK: Duration = Duration::from_millis(15);
/// How often the core thread looks at the engine's rings when no message wakes it.
const HOUSEKEEPING: Duration = Duration::from_millis(100);
/// -60 dBFS.
const SOUND_THRESHOLD: f32 = 0.001;
const LOAD_STEP: u16 = 10;
/// A key-to-sound time above this is a key that waited for a stream, not a measurement.
const LATENCY_SANE_US: u64 = 1_000_000;

#[derive(Debug, Clone)]
pub struct CoreOptions {
    /// Where `settings.json` lives. `None`: nothing is read or saved.
    pub config_dir: Option<PathBuf>,
    /// Where `key-to-sound.json` goes. `None`: nothing is recorded.
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
    /// The folder of the sound pack. `None`: only the computed instruments sound.
    pub sounds_dir: Option<PathBuf>,
    /// `None`: no plugins; nothing is scanned and no process is started.
    pub plugins: Option<PluginOptions>,
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
}

/// Called on the core thread; must not wait for the main thread.
pub type ShellSink = Box<dyn Fn(ShellRequest) + Send>;

impl CoreOptions {
    /// Everything on, with the directories and the MIDI options the environment gives.
    pub fn from_env() -> CoreOptions {
        CoreOptions {
            config_dir: settings::config_dir(),
            measure_dir: measure::dir_from_env(),
            midi: Some(MidiOptions::from_env()),
            devices: true,
            audio: true,
            songs_dir: songfile::songs_dir(),
            state_dir: recovery::dir(),
            sounds_dir: sounds::find_dir(),
            plugins: PluginOptions::from_env(),
        }
    }

    /// No device and no file: only the rules and the hub.
    pub fn offline() -> CoreOptions {
        CoreOptions {
            config_dir: None,
            measure_dir: None,
            midi: None,
            devices: false,
            audio: false,
            songs_dir: None,
            state_dir: None,
            sounds_dir: None,
            plugins: None,
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
    /// The MIDI thread ended by itself.
    MidiStopped {
        reason: String,
    },
    /// The device watcher lost PipeWire and ended.
    DevicesLost,
    LiveNotes(Sender<LiveNotesSnapshot>),
    TakeNotes(u32, Sender<Option<Vec<u8>>>),
    /// Told once the recovery file holds the song, or there was nothing to write.
    PcEnding(Sender<()>),
    /// From the thread that reads the sounds: the answer to [`Effect::LoadSound`].
    SoundRead {
        instrument: BuiltinInstrument,
        read: Result<Arc<SampleBank>, LoadError>,
    },
    /// From the scanner's threads.
    Scan(ScanNews),
    /// From the thread of the plugin processes.
    Plugins(HostNews),
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
    shell: Mutex<Option<ShellSink>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

struct Running {
    core: JoinHandle<()>,
    telemetry: JoinHandle<()>,
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
        let library = {
            let tx = tx.clone();
            let done = move |instrument, read| {
                let _ = tx.send(Msg::SoundRead { instrument, read });
            };
            Library::open(options.sounds_dir.as_deref(), Box::new(done))
        };
        let mut model = Model::new(settings).with_sounds(library.installed());
        if let Some(plugin_options) = &options.plugins {
            model = model.with_plugin_places(plugin_options.usual_places.clone());
        }
        let hub = Arc::new(Hub::new(model.view()));
        let shared = Arc::new(Shared {
            held: Held::default(),
            fixed_velocity: AtomicU8::new(model.fixed_velocity().unwrap_or(0)),
            stream: StreamFacts::default(),
            stop: AtomicBool::new(false),
            clock: Mutex::new(Arc::new(TransportClock::default())),
            shell: Mutex::new(None),
        });

        let parts = Engine::new(EngineConfig {
            sample_rate: 48_000,
            max_tracks: MAX_TRACKS,
            ..EngineConfig::default()
        });
        *lock(&shared.clock) = parts.handle.clock().clone();
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
            let hosts = Hosts::start(
                HostOptions {
                    exe: plugin_options.host_exe.clone(),
                    env: plugin_options.child_env.clone(),
                    geometry: DEFAULT_GEOMETRY,
                },
                move |news| {
                    let _ = host_tx.send(Msg::Plugins(news));
                },
            );
            let dir = plugin_options.cache_dir.as_deref();
            PluginSide {
                scanner,
                _hosts: hosts,
                start: Some(Input::PluginsStart {
                    cache: Box::new(dir.map_or_else(ScanCache::default, ScanCache::load)),
                    first: !dir.is_some_and(ScanCache::is_saved),
                }),
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
                    model.engine_seats(),
                ),
                model,
                config_dir: options.config_dir.clone(),
                songs_dir: options.songs_dir.clone(),
                state_dir: options.state_dir.clone(),
                recovery: options.state_dir.clone().map(recovery::Writer::start),
                ending: Vec::new(),
                key_to_sound: options.measure_dir.clone().map(KeyToSound::new),
                links,
                midi,
                devices,
                audio_retry_at: None,
                plugins,
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

    /// The answer to [`ShellRequest::PickSaveAs`]: the folder and the name typed, as one path.
    /// Does not wait.
    pub fn save_as_picked(&self, path: Option<PathBuf>) {
        let _ = self.tx.send(Msg::Input(Input::SaveAsPicked(path), None));
    }

    /// From this call on, every patch reaches `messages` in rev order: a snapshot taken after
    /// `subscribe` returns misses nothing.
    pub fn subscribe(&self, messages: MessageSink, telemetry: TelemetrySink) {
        self.hub.subscribe(messages, telemetry);
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

/// What the sinks of the MIDI thread and the device watcher need, to make new ones.
struct Links {
    hub: Arc<Hub>,
    shared: Arc<Shared>,
    tx: Sender<Msg>,
}

impl Links {
    /// Keys go straight to the engine and the screen; plug changes go to the core thread.
    fn midi_sink(&self, mut midi_in: MidiFeed) -> MidiSink {
        let (hub, shared, tx) = (self.hub.clone(), self.shared.clone(), self.tx.clone());
        let mut last_played = String::new();
        Box::new(move |message: MidiMessage| match message {
            MidiMessage::Event {
                keyboard,
                event,
                t_us,
            } => {
                let event = fixed(event, shared.fixed_velocity.load(Relaxed));
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
            MidiMessage::Removed { id } => {
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
        seats: Vec<EngineOp>,
    ) -> Audio {
        let mut audio = Audio {
            handle,
            engine: Some(engine),
            stream: None,
            allowed,
            rebuilt: None,
            seats: SeatBook::new(),
            mix: MixBook::default(),
            sounds,
        };
        for op in seats {
            audio.op(op);
        }
        audio
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
        if self.stream.is_none() && self.engine.is_none() {
            // The engine is lost; the one made in its place gets what the book says.
            return;
        }
        self.send_seat_op(op);
        self.settle();
    }

    /// An instrument as it can sound now: silent while its samples are still to be read.
    fn make(&self, instrument: &InstrumentRef) -> Box<dyn Instrument> {
        let InstrumentRef::Builtin(builtin) = instrument;
        instruments::make(instrument, self.sounds.samples(*builtin))
    }

    fn send_seat_op(&mut self, op: EngineOp) {
        let until = Instant::now() + SEAT_COMMAND_WAIT;
        let mut command = match op {
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
                let effect = insert.as_ref().map(mixbook::make);
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
        // A full ring empties within a block or two; these commands must not be lost, or the
        // engine and the song no longer agree.
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
            for op in self.mix.ops().into_iter().chain(self.seats.ops()) {
                self.send_seat_op(op);
                self.settle();
            }
            self.engine.take().expect("put there above")
        });
        let target = OutputTarget {
            node_name: node_name.to_owned(),
        };
        match AudioOutput::open(&target, REQUESTED_FRAMES, engine) {
            Ok(stream) => {
                let sample_rate = stream.sample_rate();
                let meters = self.handle.meters();
                facts.callbacks_at_open.store(meters.callbacks(), Relaxed);
                facts.lost_at_open.store(meters.streams_lost(), Relaxed);
                facts.xruns_at_open.store(meters.xruns(), Relaxed);
                facts.epoch.fetch_add(1, Relaxed);
                facts.open.store(true, Relaxed);
                self.stream = Some(stream);
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
            facts.open.store(false, Relaxed);
            facts.epoch.fetch_add(1, Relaxed);
            self.engine = stream.close();
            self.settle();
        }
    }

    fn command(&mut self, command: EngineCommand) {
        // Without a stream the command would wait in the ring and act when one opens.
        if self.stream.is_some() && self.handle.send(command).is_err() {
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
            | EngineOp::Param { .. } => {
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
                muted: [muted].into_iter().collect(),
            },
            EngineOp::RecordHere { muted } => EngineCommand::RecordHere {
                muted: [muted].into_iter().collect(),
            },
            EngineOp::Stop { t_us } => EngineCommand::Stop { t_us },
            EngineOp::Locate { tick } => EngineCommand::Locate { tick },
            EngineOp::Tempo {
                ticks_per_beat,
                beats_per_bar,
                bpm_centi,
            } => EngineCommand::SetTempo {
                ticks_per_beat,
                beats_per_bar,
                bpm_centi,
            },
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
                if self.stream.is_some() {
                    let InstrumentRef::Builtin(builtin) = &instrument;
                    let phrase = phrases::for_group(builtin.group());
                    let player = PhrasePlayer::new(self.make(&instrument), phrase);
                    if self.handle.play_sample(Box::new(player)).is_err() {
                        eprintln!("meloet: the engine did not take the sample");
                    }
                }
                return;
            }
            EngineOp::StopSample => EngineCommand::StopSample,
            EngineOp::SilentTracks { tracks } => EngineCommand::SetSilentTracks {
                tracks: tracks.into_iter().collect::<TrackSet>(),
            },
            EngineOp::Sequence { track, events } => {
                if self.stream.is_some() {
                    let sequence = Sequence::new(events.into_iter().map(sequence_event).collect());
                    if self.handle.swap_sequence(track, sequence).is_err() {
                        eprintln!("meloet: the engine did not take the sequence of track {track}");
                    }
                }
                return;
            }
        };
        self.command(command);
    }
}

fn sequence_event(e: PlayEvent) -> SequenceEvent {
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

fn record_report(r: Recorded, held: &Held) -> Option<RecordReport> {
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
        Recorded::LapChanged { lap, .. } => RecordReport::LapChanged { lap },
        Recorded::Stopped { t_us, tick, lap } => RecordReport::Stopped { t_us, tick, lap },
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
    key_to_sound: Option<KeyToSound>,
    links: Links,
    midi: Option<Midi>,
    devices: Option<DevicesState>,
    audio_retry_at: Option<Instant>,
    plugins: Option<PluginSide>,
}

/// The threads behind the plugins. Dropped when the core ends: scanning children are killed,
/// the scan file is written and every plugin process is ended.
struct PluginSide {
    scanner: Scanner,
    /// No track or insert holds a plugin yet; the processes are started through this.
    _hosts: Hosts,
    /// Given to the rules when the core thread starts.
    start: Option<Input>,
}

impl Worker {
    fn run(mut self, rx: Receiver<Msg>) {
        if let Some(start) = self.state_dir.as_deref().and_then(recovery_start) {
            self.handle(Input::RecoveryFound(Box::new(start)));
        }
        // The rules ask for the samples of the song they start with.
        self.handle(Input::Tick);
        if let Some(start) = self.plugins.as_mut().and_then(|p| p.start.take()) {
            self.handle(start);
        }
        let mut kept = Instant::now();
        let mut polled = Instant::now();
        loop {
            let wait = if self.model.is_moving() {
                MOVING_TICK
            } else {
                HOUSEKEEPING
            };
            match rx.recv_timeout(wait) {
                Ok(Msg::Input(input, reply)) => {
                    let rev = self.handle(input);
                    if let Some(reply) = reply {
                        let _ = reply.send(rev);
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
                Ok(Msg::Plugins(news)) => {
                    // Until tracks and inserts hold plugins there is no slot to name.
                    if let HostNews::Stopped { key, slots } = news {
                        eprintln!(
                            "meloet: the process of {key} stopped ({} plugins)",
                            slots.len()
                        );
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            }
            if self.model.is_moving() && polled.elapsed() >= MOVING_TICK {
                polled = Instant::now();
                self.poll_engine();
            }
            if kept.elapsed() >= HOUSEKEEPING {
                kept = Instant::now();
                self.housekeeping(false);
                self.retries(kept);
                if self.model.recovery_waiting() || self.model.sample_playing() {
                    self.handle(Input::Tick);
                }
            }
        }
        self.housekeeping(true);
        // Waits for the recovery file asked for so far.
        drop(self.recovery.take());
        drop(self.midi.take());
        drop(self.devices.take());
        self.audio.close(&self.shared.stream);
        drop(self.plugins.take());
    }

    /// Takes in what the engine said about the recording, and what the listener hears now.
    fn poll_engine(&mut self) {
        while let Some(r) = self.audio.handle.pop_recorded() {
            if let Some(report) = record_report(r, &self.shared.held) {
                self.handle(Input::Record(report));
            }
        }
        if self.model.is_moving() {
            self.handle(Input::Tick);
        }
    }

    fn heard(&self) -> Option<Heard> {
        if !self.shared.stream.open.load(Relaxed) {
            return None;
        }
        self.audio.handle.clock().heard_now().map(|h| Heard {
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

    /// One input, the effects it asks for, and the inputs those answer with. Returns the rev.
    fn handle(&mut self, input: Input) -> u64 {
        let mut inputs = VecDeque::from([input]);
        while let Some(input) = inputs.pop_front() {
            self.model.set_now(monotonic_us(), self.heard());
            for effect in self.model.handle(input) {
                match effect {
                    Effect::OpenOutput { node_name } => {
                        inputs.push_back(match self.audio.open(&node_name, &self.shared.stream) {
                            Some(sample_rate) => Input::AudioOpened {
                                node_name,
                                sample_rate,
                            },
                            None => Input::AudioOpenFailed { node_name },
                        });
                        if let Some(feed) = self.audio.rebuilt.take() {
                            *lock(&self.shared.clock) = self.audio.handle.clock().clone();
                            if let Some(midi) = &mut self.midi {
                                midi.feed(self.links.midi_sink(feed));
                            }
                        }
                    }
                    Effect::CloseOutput => self.audio.close(&self.shared.stream),
                    Effect::TestTone => self.audio.command(EngineCommand::TestTone),
                    Effect::AllNotesOff => {
                        self.shared.held.release_all();
                        self.audio.command(EngineCommand::AllNotesOff);
                    }
                    Effect::RetryAudio { after } => {
                        self.audio_retry_at = Some(Instant::now() + after);
                    }
                    Effect::Engine(op) => self.audio.op(op),
                    Effect::LiveNotes(notes) => self.hub.live_notes(notes),
                    Effect::SaveSettings(settings) => {
                        if let Some(dir) = &self.config_dir
                            && let Err(e) = settings::save(dir, &settings)
                        {
                            eprintln!("meloet: settings not saved in {}: {e}", dir.display());
                        }
                    }
                    Effect::SaveSong { song, to } => {
                        inputs.push_back(save_song(self.songs_dir.as_deref(), &song, to));
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
                        let done = if ending {
                            std::mem::take(&mut self.ending)
                        } else {
                            Vec::new()
                        };
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
                        if let Some(plugins) = &self.plugins {
                            plugins.scanner.scan(*job);
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
                    // No track or insert holds a plugin yet, so there is no process to ask.
                    Effect::ReloadPlugin { .. } | Effect::UnloadPlugin { .. } => {}
                }
            }
        }
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
        self.hub.publish(self.model.view())
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

/// Does [`Effect::SaveSong`] and gives its answer.
pub(crate) fn save_song(songs_dir: Option<&Path>, song: &meloet_model::Song, to: SaveTo) -> Input {
    let saved = match to {
        SaveTo::File(path) => songfile::save(&path, song).map(|()| (path, song.doc.name.clone())),
        SaveTo::NewFolder(folder) => match folder.as_deref().or(songs_dir) {
            Some(folder) => songfile::save_new(folder, song),
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
    Some(match recovery::find(dir)? {
        recovery::Found::Read(r) => RecoveryStart::Read {
            saved: r.path.as_deref().and_then(read_saved),
            song: Box::new(r.song),
            path: r.path,
            cause: r.cause,
            written_ms: r.written_ms,
        },
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
    fn housekeeping(&mut self, last: bool) {
        self.poll_engine();
        self.audio.handle.drop_retired();
        let mut samples = Vec::new();
        while let Some(record) = self.audio.handle.pop_latency() {
            if let Some(ms) = key_to_sound_ms(record) {
                samples.push(ms);
                if let Some(log) = &mut self.key_to_sound {
                    log.add(record);
                }
            }
        }
        for ms in samples {
            self.handle(Input::LatencySample { ms });
        }
        if let Some(log) = &mut self.key_to_sound {
            let state = self.hub.snapshot().state.audio;
            log.write(
                last,
                state.sample_rate.unwrap_or(0),
                state.requested_frames,
                state.actual_frames,
            );
        }
    }
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
            lock(&shared.clock)
                .clone()
                .heard_now()
                .map(|h| TransportReading {
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

#[cfg(test)]
mod tests;
