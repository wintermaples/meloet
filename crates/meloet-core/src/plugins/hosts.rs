//! The processes plugins sound in (docs/architecture.md 20-1, 20-2).
//!
//! One thread, alive as long as [`Hosts`], starts every `meloet-plughost` process, talks to
//! it over its socket and watches it. Plugins with the same key share a process. The thread
//! must be the one that starts the children: a child is killed when the thread that started
//! it ends (`launch::die_with_parent`), so none outlives a meloet that crashed.
//!
//! The way a plugin gets into the engine: [`Hosts::load`] makes the shared memory and the
//! process when the key has none yet, makes the engine's end of a slot, has the process load
//! the plugin into it and hands back a [`PluginSlot`]. The caller wraps that as a
//! `PluginInstrument` or a `PluginEffect` and gives it to the engine. To take a plugin out,
//! the caller first takes it out of the engine and drops what comes back, then calls
//! [`Hosts::unload`]: a slot the engine still runs after its plugin is gone would look like
//! a plugin that stopped.
//!
//! Watching: a process has stopped when it has ended (its pidfd says so) or when the engine
//! gave up on one of its slots (`PluginWatch::stopped`: two blocks without an answer). It is
//! then killed through its pidfd, so a process that hangs does not stay, its shared memory
//! is marked as stopped, so the engine waits for none of its slots again, and every slot it
//! had is told as stopped with [`HostNews::Stopped`]. [`Hosts::reload`] puts such a slot into
//! a new process, with the state remembered for it.
//!
//! Windows and parameters (20-13): a plugin's window is a window of its process, on that
//! process's main thread, which is not the thread its sound runs on. Every call here is
//! answered with a callback on the hosts' thread and none waits for a window: `open_window`
//! is answered as soon as the process has heard it, and the window itself is told later with
//! [`HostNews::WindowOpened`]. What the person does in a window comes as news. Parameter
//! values are 0..=1 everywhere here, whatever the plugin's own range.
//!
//! An export has `Hosts` of its own ([`Hosts::start_offline`], docs/architecture.md 22-11):
//! other processes, started the same way and told that they are for an export, and slots
//! that wait for every answer.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::Child;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_engine::{PluginBlock, PluginSlot, PluginWatch};
pub use meloet_plugin::EditorKind;
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::{EngineEnd, Geometry, Mapping};
use meloet_plugin::launch::{self, PidFd};
pub use meloet_plugin::wire::{EditorOpen, KeyName, ParamNow, ParamShape, PresetInfo, TextValue};
use meloet_plugin::wire::{
    FromHost, LoadedPlugin, Notice, ParamInfo, Reply, Request, ToHost, WIRE_VERSION, read_frame,
    write_frame,
};

/// How often the processes are looked at. A plugin that stopped is told at most this long
/// after the engine gave up on it.
pub const WATCH_TICK: Duration = Duration::from_millis(10);
/// A process that does not answer on its socket for this long is taken for stopped.
const ANSWER_WAIT: Duration = Duration::from_secs(5);
/// Loading reads the plugin's files; big ones take seconds.
const LOAD_WAIT: Duration = Duration::from_secs(30);
/// Presets are read from files of their own, a bundle of them the first time it is asked
/// for (ZynAddSubFX's: 1149 presets).
const PRESET_WAIT: Duration = Duration::from_secs(15);
/// How long a process that was told to end may take before it is killed.
const QUIT_WAIT: Duration = Duration::from_millis(300);

/// The shared memory of one process: 16 slots, so the 16 tracks of a song with the same
/// instrument fit one process; a 17th starts another.
pub const DEFAULT_GEOMETRY: Geometry = Geometry {
    max_frames: 1024,
    channels: 2,
    slots: 16,
    to_plugin_events: 256,
    from_plugin_events: 64,
};

#[derive(Debug, Clone)]
pub struct HostOptions {
    /// The `meloet-plughost` binary.
    pub exe: PathBuf,
    /// Set for every process, on top of meloet's own environment.
    pub env: Vec<(OsString, OsString)>,
    pub geometry: Geometry,
}

/// One plugin, from its load to its unload, whatever process it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadSpec {
    pub key: PluginKey,
    pub bundle: PathBuf,
    pub sample_rate: u32,
}

/// The state a plugin has when its load is answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartState {
    /// As the plugin starts.
    Fresh,
    Bytes(Vec<u8>),
    /// What this plugin says its state is now; when it cannot say (it stopped), what is
    /// remembered of it.
    Of(SlotId),
}

/// A plugin ready for the engine.
pub struct Loaded {
    pub id: SlotId,
    pub slot: PluginSlot,
    pub watch: Arc<PluginWatch>,
    pub plugin: LoadedPlugin,
}

/// What is asked about the presets of an LV2 plugin.
#[derive(Debug, Clone, PartialEq)]
pub enum PresetAsk {
    /// Its presets, with those of these bundles of presets (folders).
    List { bundles: Vec<PathBuf> },
    /// The state of the preset `uri` of `List`, as [`Hosts::state`] gives a state, made
    /// without changing the plugin.
    State { uri: String, bundles: Vec<PathBuf> },
    /// Writes the plugin's state now as the preset `label`: the file `file` of the bundle
    /// folder `dir`, which is made when it is not there.
    Save {
        dir: PathBuf,
        file: String,
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PresetSaid {
    Listed(Vec<PresetInfo>),
    State(Arc<[u8]>),
    /// The preset written, as it is listed from now on.
    Saved(PresetInfo),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// The process could not be started, or its shared memory could not be made.
    Start(String),
    /// The process ended, or answered nothing in time.
    Gone,
    /// The process said no. The message is for the log.
    Refused(String),
    UnknownSlot,
    /// `reload` of a plugin whose process runs.
    NotStopped,
    /// `Hosts` has ended.
    Ended,
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::Start(why) => write!(f, "the plugin process did not start: {why}"),
            HostError::Gone => write!(f, "the plugin process is gone"),
            HostError::Refused(why) => write!(f, "the plugin process refused: {why}"),
            HostError::UnknownSlot => write!(f, "no such plugin"),
            HostError::NotStopped => write!(f, "the plugin has not stopped"),
            HostError::Ended => write!(f, "the plugin processes have ended"),
        }
    }
}

impl std::error::Error for HostError {}

/// What happens without being asked for. Told on the hosts' thread, at most one
/// [`WATCH_TICK`] after the process said it.
#[derive(Debug, Clone, PartialEq)]
pub enum HostNews {
    /// The process of these plugins stopped and is gone. The engine plays silence for an
    /// instrument and lets the sound through an effect until they are loaded again. Their
    /// windows went with the process, without a `WindowClosed` or a `WindowLeft`; a window
    /// that was being opened will not be told either.
    Stopped { key: PluginKey, slots: Vec<SlotId> },
    /// The window asked for with [`Hosts::open_window`] is on the screen. The size is in
    /// pixels of the screen, as the plugin made it.
    WindowOpened {
        slot: SlotId,
        width: u32,
        height: u32,
        /// It does not fit the screen it is on.
        larger_than_screen: bool,
    },
    /// The window asked for with [`Hosts::open_window`] could not be made; there is none.
    /// `message` is for the log.
    WindowFailed { slot: SlotId, message: String },
    /// The plugin gave its open window another size (also when the person resized a window
    /// whose plugin allows that).
    WindowResized {
        slot: SlotId,
        width: u32,
        height: u32,
        larger_than_screen: bool,
    },
    /// The person closed the window with its own close button, or the plugin closed it. Not
    /// told after [`Hosts::close_window`] or [`Hosts::unload`]. When the person was in the
    /// window, `WindowLeft` comes first.
    WindowClosed { slot: SlotId },
    /// The keyboard came to the plugin's window: the person is in the plugin now. A menu or
    /// a dialog the plugin opens from its window counts as the window.
    WindowEntered { slot: SlotId },
    /// The keyboard went somewhere else. Not told when the window goes away under the
    /// person by [`Hosts::close_window`], [`Hosts::unload`] or `Stopped`: the visit ends
    /// with the window then. `touches`: how often the person took hold of a parameter or
    /// loaded a program in the window meanwhile (docs/architecture.md 21-10).
    WindowLeft { slot: SlotId, touches: u32 },
    /// The rest of what a plugin does by itself:
    /// - `ParamBegin`, `ParamChanged`, `ParamEnd`: the person takes hold of a parameter in
    ///   the plugin's window, moves it, lets it go. The plugin's sound has the value
    ///   already. `ParamChanged` comes at most 30 times a second for one parameter, the last
    ///   value before `ParamEnd` always, each with the plugin's text for the value. LV2
    ///   windows often send no `ParamBegin` and `ParamEnd`, and may move parameters that are
    ///   not in [`Hosts::params`] (the plugin hides them); those are to be passed over. The
    ///   value is remembered for `reload` as with [`Hosts::set_param`].
    /// - `ParamsChanged`: any parameter may have another value (the plugin loaded a program
    ///   by itself): [`Hosts::params`] reads them again.
    /// - `LatencyChanged`: the plugin's delay changed outside a block.
    ///
    /// The notices about windows never come this way; they are the news above.
    Notice { slot: SlotId, notice: Notice },
}

/// Called once, on the hosts' thread, with the answer.
pub type Done<T> = Box<dyn FnOnce(Result<T, HostError>) + Send>;

/// A process as [`Hosts::processes`] tells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub key: PluginKey,
    pub pid: u32,
    pub slots: Vec<SlotId>,
    /// Blocks of the engine in which a plugin of this process gave no answer in time (each
    /// such block is a gap in that plugin's sound), added up over its plugins.
    pub unanswered: u64,
    /// How long the audio thread waited for the answers of these plugins in all.
    pub waited_ns: u64,
    /// The largest delay, in frames, a plugin of this process says it has.
    pub latency: u32,
}

enum Ask {
    Load(LoadSpec, StartState, Arc<PluginBlock>, Done<Loaded>),
    Reload(SlotId, Arc<PluginBlock>, Done<Loaded>),
    Unload(SlotId, Done<()>),
    State(SlotId, Done<Vec<u8>>),
    SetState(SlotId, Vec<u8>, Done<()>),
    KeyNames(SlotId, Done<Option<Vec<KeyName>>>),
    Preset(SlotId, PresetAsk, Done<PresetSaid>),
    Params(SlotId, Done<Vec<ParamInfo>>),
    Param(SlotId, u32, Done<(f64, String)>),
    SetParam(SlotId, u32, f64, Done<ParamNow>),
    Texts(SlotId, u32, Vec<f64>, Done<Vec<Option<String>>>),
    ReadText(SlotId, u32, String, Done<Option<TextValue>>),
    OpenWindow(SlotId, String, Done<EditorOpen>),
    CloseWindow(SlotId, Done<()>),
    Watch(SlotId, bool),
    Processes(Done<Vec<ProcessInfo>>),
}

pub struct Hosts {
    tx: Option<Sender<Ask>>,
    thread: Option<JoinHandle<()>>,
}

/// Asks and waits for the answer: `wait(|done| hosts.load(spec, block, done))`. Not for the
/// core thread, which must not wait.
pub fn wait<T: Send + 'static>(ask: impl FnOnce(Done<T>)) -> Result<T, HostError> {
    let (tx, rx) = mpsc::channel();
    ask(Box::new(move |answer| {
        let _ = tx.send(answer);
    }));
    rx.recv().unwrap_or(Err(HostError::Ended))
}

impl Hosts {
    pub fn start(options: HostOptions, news: impl Fn(HostNews) + Send + 'static) -> Hosts {
        Hosts::start_as(options, false, news)
    }

    /// For an export: processes of their own, which run their plugins as offline and take no
    /// real-time priority. The `PluginSlot` of every `Loaded` waits for each answer until it
    /// is there (`PluginSlot::for_export`); the engine it goes into is not on an audio
    /// thread.
    pub fn start_offline(options: HostOptions, news: impl Fn(HostNews) + Send + 'static) -> Hosts {
        Hosts::start_as(options, true, news)
    }

    fn start_as(
        options: HostOptions,
        offline: bool,
        news: impl Fn(HostNews) + Send + 'static,
    ) -> Hosts {
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("meloet-plugins".into())
            .spawn(move || {
                let mut run = Run {
                    options,
                    offline,
                    news: Box::new(news),
                    procs: Vec::new(),
                    slots: HashMap::new(),
                    next: 1,
                };
                run.serve(&rx);
                run.end();
            })
            .expect("failed to start the plugin thread");
        Hosts {
            tx: Some(tx),
            thread: Some(thread),
        }
    }

    fn ask(&self, ask: Ask) {
        // When the thread is gone the ask is dropped, and with it its `Done`: `wait` then
        // answers `Ended`.
        if let Some(tx) = &self.tx {
            let _ = tx.send(ask);
        }
    }

    /// Loads the plugin into a process of its key. `block` is the engine's
    /// (`EngineHandle::plugin_block`).
    pub fn load(&self, spec: LoadSpec, block: Arc<PluginBlock>, done: Done<Loaded>) {
        self.load_with(spec, StartState::Fresh, block, done);
    }

    /// As [`Hosts::load`], and the plugin has `state` before the answer comes. The state is
    /// remembered for `reload`.
    pub fn load_with(
        &self,
        spec: LoadSpec,
        state: StartState,
        block: Arc<PluginBlock>,
        done: Done<Loaded>,
    ) {
        self.ask(Ask::Load(spec, state, block, done));
    }

    /// A plugin that stopped, into a new process, with the state and the parameter values
    /// remembered for it. The `Loaded` has the same id; it takes the place of the stopped
    /// slot in the engine.
    pub fn reload(&self, id: SlotId, block: Arc<PluginBlock>, done: Done<Loaded>) {
        self.ask(Ask::Reload(id, block, done));
    }

    /// Lets the plugin go; its id is of no more use. The engine must not run its slot any
    /// more. A process without plugins ends.
    pub fn unload(&self, id: SlotId, done: Done<()>) {
        self.ask(Ask::Unload(id, done));
    }

    /// The plugin's whole state. It is remembered for `reload`.
    pub fn state(&self, id: SlotId, done: Done<Vec<u8>>) {
        self.ask(Ask::State(id, done));
    }

    /// Puts back a state taken from the same plugin. It is remembered for `reload`. The
    /// plugin takes the state in with its next block, which costs it part of that block
    /// (20-3), so two of these for plugins of one process are kept a block apart: the second
    /// is asked for that much later, and so is everything asked after it.
    pub fn set_state(&self, id: SlotId, state: Vec<u8>, done: Done<()>) {
        self.ask(Ask::SetState(id, state, done));
    }

    /// The names the plugin gives its keys, lowest key first (an LV2 plugin's midnam, read
    /// when it was loaded and when a state was put back). `None`: it has no way to name them.
    /// A stopped plugin answers `Gone`.
    pub fn key_names(&self, id: SlotId, done: Done<Option<Vec<KeyName>>>) {
        self.ask(Ask::KeyNames(id, done));
    }

    /// An ask about the presets of an LV2 plugin. A stopped plugin answers `Gone`.
    pub fn preset(&self, id: SlotId, ask: PresetAsk, done: Done<PresetSaid>) {
        self.ask(Ask::Preset(id, ask, done));
    }

    /// Every parameter a person can move, in the plugin's order: name, group (the top of a
    /// VST3 unit tree, an LV2 port group; `None` for those in no group), shape (with the
    /// texts of the steps of a stepped one), default, and the value now with the plugin's
    /// text for it. Parameters the plugin hides, only reports, or passes itself by with are
    /// not in it. A `value` of `None` is one the plugin does not tell (an LV2 `patch`
    /// parameter nobody has set yet).
    ///
    /// Answered on the hosts' thread, when the process's main thread has listed them: a few
    /// milliseconds, tens for a plugin with thousands (Surge XT). A stopped plugin answers
    /// `Gone`. The list of one loaded plugin stays as it is; its values change, and
    /// `Notice::ParamsChanged` says when all of them may have.
    pub fn params(&self, id: SlotId, done: Done<Vec<ParamInfo>>) {
        self.ask(Ask::Params(id, done));
    }

    /// One parameter's value, 0..=1, and the text the plugin shows for it. `Refused` for a
    /// parameter the plugin does not have and for a value it does not tell; [`Hosts::params`]
    /// tells those as `None`.
    pub fn param(&self, id: SlotId, param: u32, done: Done<(f64, String)>) {
        self.ask(Ask::Param(id, param, done));
    }

    /// Sets a parameter over the socket, outside a block; the plugin's sound has it with its
    /// next block and its window shows it. Answered with the value as the plugin took it
    /// (a stepped parameter lands on a step) and its text, so a dragged value can show its
    /// text. The value is remembered for `reload` until a newer state is. No news follows:
    /// `ParamChanged` is only for what the person does in the plugin's window. A stopped
    /// plugin answers `Gone` and remembers nothing.
    pub fn set_param(&self, id: SlotId, param: u32, value: f64, done: Done<ParamNow>) {
        self.ask(Ask::SetParam(id, param, value, done));
    }

    /// The plugin's texts for some values of a parameter, each 0..=1, without setting
    /// anything: `vec![0.0, 1.0]` asks for both ends of its range. One answer for each
    /// value, `None` where the plugin has no text. A stopped plugin answers `Gone`.
    pub fn texts(&self, id: SlotId, param: u32, values: Vec<f64>, done: Done<Vec<Option<String>>>) {
        self.ask(Ask::Texts(id, param, values, done));
    }

    /// What value a typed text stands for, as the plugin reads it ("440 Hz", "Hall", "on"),
    /// without setting anything. `None`: it cannot be read. Otherwise the value, 0..=1,
    /// whether the text named a value outside the range (`value` is then the nearest end),
    /// and the plugin's texts for the value and for both ends. For VST3 "outside" is told
    /// from the numbers in the texts and may be missed when a plugin's texts are not
    /// numbers. A stopped plugin answers `Gone`.
    pub fn read_text(&self, id: SlotId, param: u32, text: String, done: Done<Option<TextValue>>) {
        self.ask(Ask::ReadText(id, param, text, done));
    }

    /// Opens the plugin's window, headed `title`, or brings it to the front when it is open
    /// (or being opened). Only a plugin whose `Loaded::plugin.editor` is `EditorKind::X11`
    /// has a window to open; for the others `Opening` is followed by `WindowFailed`.
    ///
    /// Answered on the hosts' thread as soon as the process has heard it, a millisecond or
    /// so: `Opening` means the window is being made now, and
    /// [`HostNews::WindowOpened`] or [`HostNews::WindowFailed`] follows when the plugin is
    /// done (30 to 300 ms for the plugins measured; the caller counts the 0.2 s from
    /// `Opening`). `Raised` means there was a window already; nothing follows.
    ///
    /// The window is made on the main thread of the plugin's process. The plugin's sound
    /// goes on meanwhile, but that thread answers nothing until the window is up, so other
    /// asks for plugins of the same process are answered after it. A plugin that hangs in
    /// its window is found by the next ask that goes unanswered for 5 s; the process is then
    /// killed and told as `Stopped`. A stopped plugin answers `Gone`.
    pub fn open_window(&self, id: SlotId, title: String, done: Done<EditorOpen>) {
        self.ask(Ask::OpenWindow(id, title, done));
    }

    /// Closes the plugin's window; `Ok` also when it has none. The window is gone when the
    /// answer comes, and no `WindowClosed` or `WindowLeft` follows. `unload` closes the
    /// window too. A stopped plugin answers `Gone` (its window went with its process).
    pub fn close_window(&self, id: SlotId, done: Done<()>) {
        self.ask(Ask::CloseWindow(id, done));
    }

    /// The parameters panel shows this plugin now (`on`), or no longer does. While it does,
    /// the values meloet's automation gives the plugin's parameters come as
    /// `Notice::ParamAutomated`, the newest of each parameter at most 30 times a second
    /// (docs/architecture.md 21-8). Remembered over `reload`; a stopped plugin tells nothing.
    pub fn watch(&self, id: SlotId, on: bool) {
        self.ask(Ask::Watch(id, on));
    }

    /// The processes that run now.
    pub fn processes(&self) -> Vec<ProcessInfo> {
        wait(|done| self.ask(Ask::Processes(done))).unwrap_or_default()
    }

    /// Ends every process and the thread. Dropping does the same.
    pub fn stop(&mut self) {
        drop(self.tx.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Hosts {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Proc {
    id: u64,
    key: PluginKey,
    child: Child,
    pidfd: PidFd,
    control: UnixStream,
    map: Arc<Mapping>,
    next_request: u64,
    /// By the slot's place in the shared memory.
    places: Vec<Option<SlotId>>,
    /// When a state was last put back into one of its plugins.
    state_put: Option<Instant>,
}

struct Slot {
    spec: LoadSpec,
    /// The process and the place in its shared memory; `None` once it stopped.
    home: Option<(u64, u32)>,
    watch: Arc<PluginWatch>,
    /// The last state taken or put back.
    state: Option<Vec<u8>>,
    /// The parameters set since then.
    params: BTreeMap<u32, f64>,
    /// `Hosts::watch`.
    watched: bool,
}

struct Run {
    options: HostOptions,
    /// These are the processes of an export.
    offline: bool,
    news: Box<dyn Fn(HostNews) + Send>,
    procs: Vec<Proc>,
    slots: HashMap<SlotId, Slot>,
    next: u64,
}

impl Run {
    fn serve(&mut self, rx: &Receiver<Ask>) {
        loop {
            // With no process there is nothing to watch.
            let ask = if self.procs.is_empty() {
                match rx.recv() {
                    Ok(ask) => Some(ask),
                    Err(_) => return,
                }
            } else {
                match rx.recv_timeout(WATCH_TICK) {
                    Ok(ask) => Some(ask),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            };
            if let Some(ask) = ask {
                self.answer(ask);
            }
            self.listen();
            self.watch();
        }
    }

    fn answer(&mut self, ask: Ask) {
        match ask {
            Ask::Load(spec, start, block, done) => {
                let id = SlotId(self.next);
                self.next += 1;
                let (state, params) = match start {
                    StartState::Fresh => (None, BTreeMap::new()),
                    StartState::Bytes(state) => (Some(state), BTreeMap::new()),
                    StartState::Of(other) => self.state_now(other),
                };
                done(self.place(&spec, state.as_deref(), &params, &block).map(
                    |(home, slot, watch, plugin)| {
                        self.slots.insert(
                            id,
                            Slot {
                                spec,
                                home: Some(home),
                                watch: watch.clone(),
                                state,
                                params,
                                watched: false,
                            },
                        );
                        self.seat(home, Some(id));
                        Loaded {
                            id,
                            slot,
                            watch,
                            plugin,
                        }
                    },
                ));
            }
            Ask::Reload(id, block, done) => done(self.reload(id, &block)),
            Ask::Unload(id, done) => done(self.unload(id)),
            Ask::State(id, done) => {
                let state = self
                    .ask_slot(id, |slot| Request::GetState { slot }, &[])
                    .and_then(|answer| match answer {
                        (Reply::State, state) => Ok(state),
                        other => Err(refused(other.0)),
                    });
                if let (Ok(state), Some(slot)) = (&state, self.slots.get_mut(&id)) {
                    slot.state = Some(state.clone());
                    slot.params.clear();
                }
                done(state);
            }
            Ask::SetState(id, state, done) => {
                self.space_states(id);
                let set = self
                    .ask_slot(id, |slot| Request::SetState { slot }, &state)
                    .and_then(ok);
                if let (Ok(()), Some(slot)) = (&set, self.slots.get_mut(&id)) {
                    slot.state = Some(state);
                    slot.params.clear();
                }
                done(set);
            }
            Ask::KeyNames(id, done) => done(
                self.ask_slot(id, |slot| Request::KeyNames { slot }, &[])
                    .and_then(|answer| match answer.0 {
                        Reply::KeyNames { names } => Ok(names),
                        other => Err(refused(other)),
                    }),
            ),
            Ask::Preset(id, ask, done) => done(self.preset(id, ask)),
            Ask::Params(id, done) => done(
                self.ask_slot(id, |slot| Request::Params { slot }, &[])
                    .and_then(|answer| match answer.0 {
                        Reply::Params { params } => Ok(params),
                        other => Err(refused(other)),
                    }),
            ),
            Ask::Param(id, param, done) => done(
                self.ask_slot(id, |slot| Request::GetParam { slot, id: param }, &[])
                    .and_then(|answer| match answer.0 {
                        Reply::Param {
                            now:
                                ParamNow {
                                    value: Some(value),
                                    text,
                                    ..
                                },
                        } => Ok((value, text.unwrap_or_default())),
                        Reply::Param { .. } => Err(HostError::Refused(
                            "the plugin does not tell this value".into(),
                        )),
                        other => Err(refused(other)),
                    }),
            ),
            Ask::SetParam(id, param, value, done) => {
                let request = |slot| Request::SetParam {
                    slot,
                    id: param,
                    value,
                };
                let set = self.ask_slot(id, request, &[]).and_then(param_now);
                if let (Ok(_), Some(slot)) = (&set, self.slots.get_mut(&id)) {
                    slot.params.insert(param, value);
                }
                done(set);
            }
            Ask::Texts(id, param, values, done) => {
                let request = |slot| Request::Texts {
                    slot,
                    id: param,
                    values,
                };
                done(
                    self.ask_slot(id, request, &[])
                        .and_then(|answer| match answer.0 {
                            Reply::Texts { texts } => Ok(texts),
                            other => Err(refused(other)),
                        }),
                );
            }
            Ask::ReadText(id, param, text, done) => {
                let request = |slot| Request::ReadText {
                    slot,
                    id: param,
                    text,
                };
                done(
                    self.ask_slot(id, request, &[])
                        .and_then(|answer| match answer.0 {
                            Reply::Read { read } => Ok(read),
                            other => Err(refused(other)),
                        }),
                );
            }
            Ask::OpenWindow(id, title, done) => {
                let request = |slot| Request::OpenEditor { slot, title };
                done(
                    self.ask_slot(id, request, &[])
                        .and_then(|answer| match answer.0 {
                            Reply::Editor { open } => Ok(open),
                            other => Err(refused(other)),
                        }),
                );
            }
            Ask::CloseWindow(id, done) => done(
                self.ask_slot(id, |slot| Request::CloseEditor { slot }, &[])
                    .and_then(ok),
            ),
            Ask::Watch(id, on) => {
                if let Some(slot) = self.slots.get_mut(&id) {
                    slot.watched = on;
                    let _ = self.ask_slot(id, |slot| Request::Watch { slot, on }, &[]);
                }
            }
            Ask::Processes(done) => done(Ok(self
                .procs
                .iter()
                .map(|p| {
                    let slots: Vec<SlotId> = p.places.iter().flatten().copied().collect();
                    let watches = || slots.iter().filter_map(|id| self.slots.get(id));
                    ProcessInfo {
                        key: p.key.clone(),
                        pid: p.child.id(),
                        unanswered: watches().map(|s| s.watch.unanswered()).sum(),
                        waited_ns: watches().map(|s| s.watch.waited_ns()).sum(),
                        latency: watches().map(|s| s.watch.latency()).max().unwrap_or(0),
                        slots,
                    }
                })
                .collect())),
        }
    }

    /// The state of a plugin as it says it now. When it does not answer: the last state
    /// taken or put back, with the parameters set since.
    fn state_now(&mut self, id: SlotId) -> (Option<Vec<u8>>, BTreeMap<u32, f64>) {
        match self.ask_slot(id, |slot| Request::GetState { slot }, &[]) {
            Ok((Reply::State, state)) => (Some(state), BTreeMap::new()),
            _ => (self.slots.get(&id))
                .map(|slot| (slot.state.clone(), slot.params.clone()))
                .unwrap_or_default(),
        }
    }

    /// Waits until the longest block the engine may run has passed since a state was last
    /// put back into a plugin of the same process.
    fn space_states(&mut self, id: SlotId) {
        let Some(slot) = self.slots.get(&id) else {
            return;
        };
        let rate = f64::from(slot.spec.sample_rate.max(1));
        let block = Duration::from_secs_f64(f64::from(self.options.geometry.max_frames) / rate);
        let Some(at) = slot.home.and_then(|(proc, _)| self.proc_at(proc)) else {
            return;
        };
        let proc = &mut self.procs[at];
        if let Some(left) = (proc.state_put).and_then(|put| block.checked_sub(put.elapsed())) {
            std::thread::sleep(left);
        }
        proc.state_put = Some(Instant::now());
    }

    fn proc_at(&self, id: u64) -> Option<usize> {
        self.procs.iter().position(|p| p.id == id)
    }

    fn seat(&mut self, (proc, place): (u64, u32), slot: Option<SlotId>) {
        if let Some(at) = self.proc_at(proc) {
            self.procs[at].places[place as usize] = slot;
        }
    }

    /// Whether the process has ended or the engine gave up on one of its plugins.
    fn has_stopped(&self, at: usize) -> bool {
        let proc = &self.procs[at];
        proc.pidfd.exited(Duration::ZERO)
            || proc
                .places
                .iter()
                .flatten()
                .any(|id| self.slots.get(id).is_some_and(|s| s.watch.stopped()))
    }

    /// One request to a process and its answer, with the frame's blob. Notices that come in
    /// between are told. Gives up when the process stops or `within` has passed; a process
    /// that does not answer in time is killed.
    fn request(
        &mut self,
        at: usize,
        request: Request,
        blob: &[u8],
        within: Duration,
    ) -> Result<(Reply, Vec<u8>), HostError> {
        let proc = &mut self.procs[at];
        let id = proc.next_request;
        proc.next_request += 1;
        write_frame(&mut proc.control, &ToHost { id, request }, blob)
            .map_err(|_| HostError::Gone)?;
        let until = Instant::now() + within;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                let _ = self.procs[at].pidfd.kill();
                return Err(HostError::Gone);
            }
            if !launch::wait_readable(self.procs[at].control.as_fd(), left.min(WATCH_TICK)) {
                if self.has_stopped(at) {
                    return Err(HostError::Gone);
                }
                continue;
            }
            let proc = &mut self.procs[at];
            let _ = proc.control.set_read_timeout(Some(left));
            match read_frame::<FromHost>(&mut proc.control) {
                Ok((FromHost::Reply { id: got, reply }, blob)) if got == id => {
                    return Ok((reply, blob));
                }
                Ok((FromHost::Reply { .. }, _)) => {}
                Ok((FromHost::Notice { notice }, _)) => self.heard(at, notice),
                Err(_) => return Err(HostError::Gone),
            }
        }
    }

    /// Something a plugin of this process did by itself: remembered where `reload` needs
    /// it, and told.
    fn heard(&mut self, at: usize, notice: Notice) {
        let place = notice_slot(&notice) as usize;
        let Some(slot) = self.procs[at].places.get(place).copied().flatten() else {
            return;
        };
        let news = match notice {
            Notice::EditorOpened {
                width,
                height,
                larger_than_screen,
                ..
            } => HostNews::WindowOpened {
                slot,
                width,
                height,
                larger_than_screen,
            },
            Notice::EditorFailed { message, .. } => HostNews::WindowFailed { slot, message },
            Notice::EditorResized {
                width,
                height,
                larger_than_screen,
                ..
            } => HostNews::WindowResized {
                slot,
                width,
                height,
                larger_than_screen,
            },
            Notice::EditorClosed { .. } => HostNews::WindowClosed { slot },
            Notice::EditorEntered { .. } => HostNews::WindowEntered { slot },
            Notice::EditorLeft { touches, .. } => HostNews::WindowLeft { slot, touches },
            notice => {
                if let (Notice::ParamChanged { id, value, .. }, Some(kept)) =
                    (&notice, self.slots.get_mut(&slot))
                {
                    kept.params.insert(*id, *value);
                }
                HostNews::Notice { slot, notice }
            }
        };
        (self.news)(news);
    }

    /// Takes what the processes said by themselves since the last look. A process that
    /// starts a notice and does not finish it within the answer time is killed; `watch`
    /// then finds it stopped.
    fn listen(&mut self) {
        for at in 0..self.procs.len() {
            while launch::wait_readable(self.procs[at].control.as_fd(), Duration::ZERO) {
                let proc = &mut self.procs[at];
                let _ = proc.control.set_read_timeout(Some(ANSWER_WAIT));
                match read_frame::<FromHost>(&mut proc.control) {
                    Ok((FromHost::Notice { notice }, _)) => self.heard(at, notice),
                    // An answer nobody waits for any more.
                    Ok((FromHost::Reply { .. }, _)) => {}
                    Err(_) => {
                        let _ = proc.pidfd.kill();
                        break;
                    }
                }
            }
        }
    }

    fn ask_slot(
        &mut self,
        id: SlotId,
        request: impl FnOnce(u32) -> Request,
        blob: &[u8],
    ) -> Result<(Reply, Vec<u8>), HostError> {
        let slot = self.slots.get(&id).ok_or(HostError::UnknownSlot)?;
        let (proc, place) = slot.home.ok_or(HostError::Gone)?;
        let at = self.proc_at(proc).ok_or(HostError::Gone)?;
        self.request(at, request(place), blob, ANSWER_WAIT)
    }

    fn preset(&mut self, id: SlotId, ask: PresetAsk) -> Result<PresetSaid, HostError> {
        let slot = self.slots.get(&id).ok_or(HostError::UnknownSlot)?;
        let (proc, slot) = slot.home.ok_or(HostError::Gone)?;
        let at = self.proc_at(proc).ok_or(HostError::Gone)?;
        let request = match &ask {
            PresetAsk::List { bundles } => {
                let bundles = bundles.clone();
                Request::Presets { slot, bundles }
            }
            PresetAsk::State { uri, bundles } => {
                let (uri, bundles) = (uri.clone(), bundles.clone());
                Request::PresetState { slot, uri, bundles }
            }
            PresetAsk::Save { dir, file, label } => Request::SavePreset {
                slot,
                dir: dir.clone(),
                file: file.clone(),
                label: label.clone(),
            },
        };
        match (ask, self.request(at, request, &[], PRESET_WAIT)?) {
            (PresetAsk::List { .. }, (Reply::Presets { presets }, _)) => {
                Ok(PresetSaid::Listed(presets))
            }
            (PresetAsk::State { .. }, (Reply::State, state)) => Ok(PresetSaid::State(state.into())),
            (PresetAsk::Save { .. }, (Reply::Presets { mut presets }, _)) if presets.len() == 1 => {
                Ok(PresetSaid::Saved(presets.remove(0)))
            }
            (_, (other, _)) => Err(refused(other)),
        }
    }

    fn start_proc(&mut self, key: &PluginKey) -> Result<usize, HostError> {
        let start = |e: &dyn std::fmt::Display| HostError::Start(e.to_string());
        let map = Arc::new(Mapping::create(self.options.geometry).map_err(|e| start(&e))?);
        let (ours, theirs) = UnixStream::pair().map_err(|e| start(&e))?;
        let log = host_log(self.next);
        let mut command = launch::host_command(
            &self.options.exe,
            theirs.as_fd(),
            map.fd(),
            log.as_ref().map(AsFd::as_fd),
            self.offline,
        )
        .map_err(|e| start(&e))?;
        drop(log);
        command.envs(self.options.env.iter().map(|(name, value)| (name, value)));
        let mut child = launch::spawn(&mut command).map_err(|e| start(&e))?;
        drop((command, theirs));
        let pidfd = match PidFd::open(child.id()) {
            Ok(pidfd) => pidfd,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(start(&e));
            }
        };
        let id = self.next;
        self.next += 1;
        self.procs.push(Proc {
            id,
            key: key.clone(),
            child,
            pidfd,
            control: ours,
            map,
            next_request: 1,
            places: vec![None; self.options.geometry.slots as usize],
            state_put: None,
        });
        let at = self.procs.len() - 1;
        let hello = Request::Hello {
            version: WIRE_VERSION,
        };
        match self.request(at, hello, &[], ANSWER_WAIT) {
            Ok((Reply::Hello { version, offline }, _))
                if version == WIRE_VERSION && offline == self.offline =>
            {
                Ok(at)
            }
            answer => {
                self.bury(at);
                Err(match answer {
                    Ok((reply, _)) => refused(reply),
                    Err(e) => e,
                })
            }
        }
    }

    /// Loads a plugin into a free place of a process of its key, starting one when there is
    /// none, and puts `state` and `params` into it. The place is not taken yet.
    #[allow(clippy::type_complexity)]
    fn place(
        &mut self,
        spec: &LoadSpec,
        state: Option<&[u8]>,
        params: &BTreeMap<u32, f64>,
        block: &Arc<PluginBlock>,
    ) -> Result<((u64, u32), PluginSlot, Arc<PluginWatch>, LoadedPlugin), HostError> {
        let free = |p: &Proc| p.key == spec.key && p.places.iter().any(Option::is_none);
        let at = match self.procs.iter().position(free) {
            Some(at) => at,
            None => self.start_proc(&spec.key)?,
        };
        let proc = &self.procs[at];
        let home = proc.id;
        let place = proc.places.iter().position(Option::is_none).unwrap_or(0) as u32;
        let loaded = EngineEnd::new(proc.map.clone(), place)
            .map_err(|e| HostError::Start(e.to_string()))
            .and_then(|end| {
                let load = Request::Load {
                    slot: place,
                    key: spec.key.clone(),
                    bundle: spec.bundle.clone(),
                    sample_rate: spec.sample_rate,
                    max_frames: self.options.geometry.max_frames,
                };
                match self.request(at, load, &[], LOAD_WAIT)?.0 {
                    Reply::Loaded { plugin } => Ok((end, plugin)),
                    other => Err(refused(other)),
                }
            })
            .and_then(|loaded| {
                if let Some(state) = state {
                    let set = Request::SetState { slot: place };
                    self.request(at, set, state, ANSWER_WAIT).and_then(ok)?;
                }
                for (&id, &value) in params {
                    let set = Request::SetParam {
                        slot: place,
                        id,
                        value,
                    };
                    (self.request(at, set, &[], ANSWER_WAIT)).and_then(param_now)?;
                }
                Ok(loaded)
            });
        match loaded {
            Ok((end, plugin)) => {
                let (slot, watch) = if self.offline {
                    PluginSlot::for_export(end, block.clone(), plugin.latency)
                } else {
                    PluginSlot::new(end, block.clone(), plugin.latency)
                };
                Ok(((home, place), slot, watch, plugin))
            }
            Err(e) => {
                // The process is of no use to anyone when this was to be its first plugin.
                if let Some(at) = self.proc_at(home)
                    && !self.has_stopped(at)
                {
                    let _ = self.request(at, Request::Unload { slot: place }, &[], ANSWER_WAIT);
                    self.retire_if_empty(home);
                }
                Err(e)
            }
        }
    }

    fn reload(&mut self, id: SlotId, block: &Arc<PluginBlock>) -> Result<Loaded, HostError> {
        let slot = self.slots.get(&id).ok_or(HostError::UnknownSlot)?;
        if slot.home.is_some() {
            return Err(HostError::NotStopped);
        }
        let (spec, state, params) = (slot.spec.clone(), slot.state.clone(), slot.params.clone());
        let (home, engine_slot, watch, plugin) =
            self.place(&spec, state.as_deref(), &params, block)?;
        let mut watched = false;
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.home = Some(home);
            slot.watch = watch.clone();
            watched = slot.watched;
        }
        self.seat(home, Some(id));
        if watched {
            let _ = self.ask_slot(id, |slot| Request::Watch { slot, on: true }, &[]);
        }
        Ok(Loaded {
            id,
            slot: engine_slot,
            watch,
            plugin,
        })
    }

    fn unload(&mut self, id: SlotId) -> Result<(), HostError> {
        let slot = self.slots.remove(&id).ok_or(HostError::UnknownSlot)?;
        let Some((proc, place)) = slot.home else {
            return Ok(());
        };
        if let Some(at) = self.proc_at(proc) {
            let _ = self.request(at, Request::Unload { slot: place }, &[], ANSWER_WAIT);
        }
        self.seat((proc, place), None);
        self.retire_if_empty(proc);
        Ok(())
    }

    /// Ends a process that has no plugin left.
    fn retire_if_empty(&mut self, proc: u64) {
        if let Some(at) = self.proc_at(proc)
            && self.procs[at].places.iter().all(Option::is_none)
        {
            let proc = self.procs.remove(at);
            quit(proc);
        }
    }

    /// Kills the process and takes it off the list. Returns the plugins it had; they are
    /// stopped from now on.
    fn bury(&mut self, at: usize) -> Vec<SlotId> {
        let mut proc = self.procs.remove(at);
        // The engine reads this at its next block and does not wait for an answer that
        // cannot come.
        proc.map.mark_stopped();
        let ended = proc.pidfd.exited(Duration::ZERO);
        let _ = proc.pidfd.kill();
        let status = proc.child.wait();
        // What the process died of, unless that was the SIGKILL just sent. A crash that
        // writes a core file is still going when the engine gives up on it.
        const SIGKILL: i32 = 9;
        if let Ok(status) = status
            && (ended || ExitStatusExt::signal(&status) != Some(SIGKILL))
        {
            eprintln!("meloet: the process of {} ended: {status}", proc.key);
        }
        let slots: Vec<SlotId> = proc.places.iter().flatten().copied().collect();
        for id in &slots {
            if let Some(slot) = self.slots.get_mut(id) {
                slot.home = None;
            }
        }
        slots
    }

    fn watch(&mut self) {
        let mut at = 0;
        while at < self.procs.len() {
            if self.has_stopped(at) {
                let key = self.procs[at].key.clone();
                let slots = self.bury(at);
                (self.news)(HostNews::Stopped { key, slots });
            } else {
                at += 1;
            }
        }
    }

    fn end(&mut self) {
        for proc in self.procs.drain(..) {
            quit(proc);
        }
    }
}

/// Development only: with `MELOET_PLUGHOST_LOG_DIR`, what a host process and its plugins print
/// goes to `<dir>/plughost-<meloet's pid>-<n>.log` instead of `/dev/null`.
#[cfg(feature = "dev-hooks")]
fn host_log(n: u64) -> Option<std::fs::File> {
    let dir = std::env::var_os("MELOET_PLUGHOST_LOG_DIR").filter(|v| !v.is_empty())?;
    let path = PathBuf::from(dir).join(format!("plughost-{}-{n}.log", std::process::id()));
    std::fs::File::create(path).ok()
}

#[cfg(not(feature = "dev-hooks"))]
fn host_log(_: u64) -> Option<std::fs::File> {
    None
}

/// Tells the process to end, and kills it when it takes too long.
fn quit(mut proc: Proc) {
    let quit = ToHost {
        id: proc.next_request,
        request: Request::Quit,
    };
    let _ = write_frame(&mut proc.control, &quit, &[]);
    if !proc.pidfd.exited(QUIT_WAIT) {
        let _ = proc.pidfd.kill();
    }
    let _ = proc.child.wait();
}

fn refused(reply: Reply) -> HostError {
    match reply {
        Reply::Error { message } => HostError::Refused(message),
        other => HostError::Refused(format!("an answer that does not fit: {other:?}")),
    }
}

fn ok((reply, _): (Reply, Vec<u8>)) -> Result<(), HostError> {
    match reply {
        Reply::Ok => Ok(()),
        other => Err(refused(other)),
    }
}

fn param_now((reply, _): (Reply, Vec<u8>)) -> Result<ParamNow, HostError> {
    match reply {
        Reply::Param { now } => Ok(now),
        other => Err(refused(other)),
    }
}

fn notice_slot(notice: &Notice) -> u32 {
    match *notice {
        Notice::EditorOpened { slot, .. }
        | Notice::EditorFailed { slot, .. }
        | Notice::EditorResized { slot, .. }
        | Notice::EditorClosed { slot }
        | Notice::EditorEntered { slot }
        | Notice::EditorLeft { slot, .. }
        | Notice::ParamBegin { slot, .. }
        | Notice::ParamChanged { slot, .. }
        | Notice::ParamEnd { slot, .. }
        | Notice::ParamAutomated { slot, .. }
        | Notice::ParamsChanged { slot }
        | Notice::LatencyChanged { slot, .. } => slot,
    }
}
