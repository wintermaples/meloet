//! The moving part: the threads around the pure rules.
//!
//! Only the core thread changes the state. The MIDI thread, the device watcher and the
//! telemetry thread send it what they learn over one channel. The core thread owns the MIDI
//! input and the device watcher, and starts them again when they stop by themselves.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{
    AtomicBool, AtomicI16, AtomicU8, AtomicU64, AtomicUsize, Ordering::Relaxed,
};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_devices::{DeviceWatcher, DevicesError, DevicesEvent};
use meloet_engine::{
    AudioOutput, BlockTiming, Engine, EngineCommand, EngineConfig, EngineHandle, EngineMeters,
    InstrumentEvent, LatencyRecord, MidiFeed, MidiIn, OutputTarget, PlaceholderSynth,
    RunningOutput, monotonic_us,
};
use meloet_midi::{MidiEvent, MidiInput, MidiMessage, MidiOptions, MidiSink};
use meloet_protocol::{Command, Event, Snapshot, Telemetry};

use crate::hub::{Hub, MessageSink, TelemetrySink};
use crate::measure::{self, KeyToSound};
use crate::settings::{self, Settings};
use crate::state::{Effect, Input, Model, REQUESTED_FRAMES, retry_delay};

const TELEMETRY_TICK: Duration = Duration::from_micros(16_667);
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
}

impl CoreOptions {
    /// Everything on, with the directories and the MIDI options the environment gives.
    pub fn from_env() -> CoreOptions {
        CoreOptions {
            config_dir: settings::config_dir(),
            measure_dir: measure::dir_from_env(),
            midi: Some(MidiOptions::from_env()),
            devices: true,
            audio: true,
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
    track_count: AtomicUsize,
    /// The core thread wants one `SoundHeard`.
    wants_sound: AtomicBool,
}

struct Shared {
    held: Held,
    stream: StreamFacts,
    stop: AtomicBool,
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
        let model = Model::new(settings);
        let hub = Arc::new(Hub::new(model.view()));
        let shared = Arc::new(Shared {
            held: Held::default(),
            stream: StreamFacts::default(),
            stop: AtomicBool::new(false),
        });
        let (tx, rx) = mpsc::channel();

        let parts = Engine::new(EngineConfig {
            sample_rate: 48_000,
            ..EngineConfig::default()
        });
        let meters = parts.handle.meters().clone();
        let track_count = model.view().song.tracks.len();
        shared.stream.track_count.store(track_count, Relaxed);

        let links = Links {
            hub: hub.clone(),
            shared: shared.clone(),
            tx: tx.clone(),
        };
        let midi = options.midi.and_then(|midi_options| {
            MidiInput::start_boxed(midi_options, links.midi_sink(parts.midi_in))
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
                model,
                hub: hub.clone(),
                shared: shared.clone(),
                audio: Audio::new(parts.engine, parts.handle, options.audio),
                config_dir: options.config_dir.clone(),
                key_to_sound: options.measure_dir.clone().map(KeyToSound::new),
                links,
                midi,
                devices,
                audio_retry_at: None,
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
                // Keys played while no stream runs must not sound when one opens.
                if shared.stream.open.load(Relaxed) {
                    let _ = midi_in.push(MidiIn {
                        event: instrument_event(event),
                        t_us,
                    });
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
        self.state = match MidiInput::start_boxed(self.options, sink) {
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
                match MidiInput::start_boxed(self.options, sink) {
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
}

/// Puts the song's instruments on `engine`.
fn install_instruments(engine: &mut Engine, handle: &mut EngineHandle) {
    if handle
        .swap_instrument(0, Box::new(PlaceholderSynth::new()))
        .is_err()
    {
        eprintln!("meloet: the engine did not take the instrument");
    }
    // One silent block on this thread puts the instrument on its track, so that every
    // `Engine::prepare` from now on reaches it.
    engine.process(&mut [0.0; 128], BlockTiming::default());
}

impl Audio {
    fn new(mut engine: Engine, mut handle: EngineHandle, allowed: bool) -> Audio {
        install_instruments(&mut engine, &mut handle);
        Audio {
            handle,
            engine: Some(engine),
            stream: None,
            allowed,
            rebuilt: None,
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
            let (mut engine, feed) = self.handle.rebuild();
            install_instruments(&mut engine, &mut self.handle);
            self.rebuilt = Some(feed);
            engine
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
        }
    }

    fn command(&mut self, command: EngineCommand) {
        // Without a stream the command would wait in the ring and act when one opens.
        if self.stream.is_some() {
            let _ = self.handle.send(command);
        }
    }
}

struct Worker {
    model: Model,
    hub: Arc<Hub>,
    shared: Arc<Shared>,
    audio: Audio,
    config_dir: Option<PathBuf>,
    key_to_sound: Option<KeyToSound>,
    links: Links,
    midi: Option<Midi>,
    devices: Option<DevicesState>,
    audio_retry_at: Option<Instant>,
}

impl Worker {
    fn run(mut self, rx: Receiver<Msg>) {
        let mut kept = Instant::now();
        loop {
            match rx.recv_timeout(HOUSEKEEPING) {
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
                Err(RecvTimeoutError::Timeout) => {}
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            }
            if kept.elapsed() >= HOUSEKEEPING {
                kept = Instant::now();
                self.housekeeping(false);
                self.retries(kept);
            }
        }
        self.housekeeping(true);
        drop(self.midi.take());
        drop(self.devices.take());
        self.audio.close(&self.shared.stream);
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
                        if let Some(feed) = self.audio.rebuilt.take()
                            && let Some(midi) = &mut self.midi
                        {
                            midi.feed(self.links.midi_sink(feed));
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
                    Effect::SaveSettings(settings) => {
                        if let Some(dir) = &self.config_dir
                            && let Err(e) = settings::save(dir, &settings)
                        {
                            eprintln!("meloet: settings not saved in {}: {e}", dir.display());
                        }
                    }
                }
            }
        }
        self.shared
            .stream
            .wants_sound
            .store(self.model.wants_sound(), Relaxed);
        self.hub.publish(self.model.view())
    }

    fn housekeeping(&mut self, last: bool) {
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

        let track_peaks: Vec<[f32; 2]> = (0..stream.track_count.load(Relaxed))
            .map(|track| meters.take_track_peak(track))
            .collect();
        // Track peaks, not the master: the test tone is not what the keys played.
        let heard = track_peaks.iter().flatten().any(|p| *p > SOUND_THRESHOLD);
        if open && heard && stream.wants_sound.swap(false, Relaxed) {
            let _ = tx.send(Msg::Input(Input::SoundHeard, None));
        }

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
            transport: None,
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
