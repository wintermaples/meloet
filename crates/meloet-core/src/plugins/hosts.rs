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

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Child;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_engine::{PluginBlock, PluginSlot, PluginWatch};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::{EngineEnd, Geometry, Mapping};
use meloet_plugin::launch::{self, PidFd};
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

/// What happens without being asked for. Told on the hosts' thread.
#[derive(Debug, Clone, PartialEq)]
pub enum HostNews {
    /// The process of these plugins stopped and is gone. The engine plays silence for an
    /// instrument and lets the sound through an effect until they are loaded again.
    Stopped {
        key: PluginKey,
        slots: Vec<SlotId>,
    },
    Notice {
        slot: SlotId,
        notice: Notice,
    },
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
    Params(SlotId, Done<Vec<ParamInfo>>),
    Param(SlotId, u32, Done<(f64, String)>),
    SetParam(SlotId, u32, f64, Done<()>),
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
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("meloet-plugins".into())
            .spawn(move || {
                let mut run = Run {
                    options,
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

    /// Puts back a state taken from the same plugin. It is remembered for `reload`.
    pub fn set_state(&self, id: SlotId, state: Vec<u8>, done: Done<()>) {
        self.ask(Ask::SetState(id, state, done));
    }

    pub fn params(&self, id: SlotId, done: Done<Vec<ParamInfo>>) {
        self.ask(Ask::Params(id, done));
    }

    /// A parameter's value, 0..=1, and the text the plugin shows for it.
    pub fn param(&self, id: SlotId, param: u32, done: Done<(f64, String)>) {
        self.ask(Ask::Param(id, param, done));
    }

    /// Sets a parameter over the socket, outside a block. The value is remembered for
    /// `reload` until a newer state is.
    pub fn set_param(&self, id: SlotId, param: u32, value: f64, done: Done<()>) {
        self.ask(Ask::SetParam(id, param, value, done));
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
}

struct Run {
    options: HostOptions,
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
                let set = self
                    .ask_slot(id, |slot| Request::SetState { slot }, &state)
                    .and_then(ok);
                if let (Ok(()), Some(slot)) = (&set, self.slots.get_mut(&id)) {
                    slot.state = Some(state);
                    slot.params.clear();
                }
                done(set);
            }
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
                        Reply::Param { value, text } => Ok((value, text)),
                        other => Err(refused(other)),
                    }),
            ),
            Ask::SetParam(id, param, value, done) => {
                let request = |slot| Request::SetParam {
                    slot,
                    id: param,
                    value,
                };
                let set = self.ask_slot(id, request, &[]).and_then(ok);
                if let (Ok(()), Some(slot)) = (&set, self.slots.get_mut(&id)) {
                    slot.params.insert(param, value);
                }
                done(set);
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
                Ok((FromHost::Notice { notice }, _)) => {
                    let place = notice_slot(&notice) as usize;
                    if let Some(slot) = proc.places.get(place).copied().flatten() {
                        (self.news)(HostNews::Notice { slot, notice });
                    }
                }
                Err(_) => return Err(HostError::Gone),
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

    fn start_proc(&mut self, key: &PluginKey) -> Result<usize, HostError> {
        let start = |e: &dyn std::fmt::Display| HostError::Start(e.to_string());
        let map = Arc::new(Mapping::create(self.options.geometry).map_err(|e| start(&e))?);
        let (ours, theirs) = UnixStream::pair().map_err(|e| start(&e))?;
        let mut command = launch::host_command(&self.options.exe, theirs.as_fd(), map.fd(), None)
            .map_err(|e| start(&e))?;
        command.envs(self.options.env.iter().map(|(name, value)| (name, value)));
        let mut child = command.spawn().map_err(|e| start(&e))?;
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
        });
        let at = self.procs.len() - 1;
        let hello = Request::Hello {
            version: WIRE_VERSION,
        };
        match self.request(at, hello, &[], ANSWER_WAIT) {
            Ok((Reply::Hello { version }, _)) if version == WIRE_VERSION => Ok(at),
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
                    self.request(at, set, &[], ANSWER_WAIT).and_then(ok)?;
                }
                Ok(loaded)
            });
        match loaded {
            Ok((end, plugin)) => {
                let (slot, watch) = PluginSlot::new(end, block.clone(), plugin.latency);
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
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.home = Some(home);
            slot.watch = watch.clone();
        }
        self.seat(home, Some(id));
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
        let _ = proc.pidfd.kill();
        let _ = proc.child.wait();
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

fn notice_slot(notice: &Notice) -> u32 {
    match *notice {
        Notice::EditorOpened { slot }
        | Notice::EditorClosed { slot }
        | Notice::EditorEntered { slot }
        | Notice::EditorLeft { slot }
        | Notice::ParamBegin { slot, .. }
        | Notice::ParamChanged { slot, .. }
        | Notice::ParamEnd { slot, .. }
        | Notice::ParamsChanged { slot }
        | Notice::LatencyChanged { slot, .. } => slot,
    }
}
