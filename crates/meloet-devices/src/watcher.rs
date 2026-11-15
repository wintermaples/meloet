//! The thread that listens to PipeWire and hands out a fresh picture whenever something changes.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fmt,
    rc::Rc,
    sync::mpsc,
    thread::{self, JoinHandle},
};

use pipewire as pw;
use pw::{
    core::{CoreRc, PW_ID_CORE},
    device::{Device, DeviceInfoRef, DeviceListener},
    metadata::{Metadata, MetadataListener},
    node::{Node, NodeInfoRef, NodeListener},
    registry::{GlobalObject, RegistryRc},
    spa::{
        param::ParamType,
        pod::Pod,
        utils::{dict::DictRef, result::AsyncSeq},
    },
    types::ObjectType,
};

use crate::{
    ClockSettings, DevicesPicture,
    facts::{DeviceFacts, NodeClass, NodeFacts, RouteParam, build_picture},
    route_pod::route_from_pod_bytes,
};

#[derive(Debug)]
pub enum DevicesError {
    /// The watcher thread could not be started.
    Thread(String),
    /// PipeWire could not be reached.
    PipeWire(String),
}

impl fmt::Display for DevicesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Thread(e) => write!(f, "could not start the device watcher thread: {e}"),
            Self::PipeWire(e) => write!(f, "could not connect to PipeWire: {e}"),
        }
    }
}

impl std::error::Error for DevicesError {}

struct Stop;

/// What the watcher thread hands to its sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicesEvent {
    Picture(DevicesPicture),
    /// The connection to PipeWire broke (the daemon stopped or restarted). This is the last
    /// event; the thread has ended. Start a new watcher to connect again.
    Lost,
}

pub struct DeviceWatcher {
    stop: pw::channel::Sender<Stop>,
    thread: Option<JoinHandle<()>>,
}

impl DeviceWatcher {
    /// `sink` is called on the watcher thread: once when the first picture is complete,
    /// then after every change (changes that come close together arrive as one picture), and
    /// with [`DevicesEvent::Lost`] if the connection breaks. Not called after [`Self::stop`].
    pub fn start(
        sink: impl FnMut(DevicesEvent) + Send + 'static,
    ) -> Result<DeviceWatcher, DevicesError> {
        let (stop, stop_rx) = pw::channel::channel::<Stop>();
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("meloet-devices".into())
            .spawn(move || run(Box::new(sink), stop_rx, ready_tx))
            .map_err(|e| DevicesError::Thread(e.to_string()))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(DeviceWatcher {
                stop,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(DevicesError::Thread(
                    "the watcher thread ended early".into(),
                ))
            }
        }
    }

    /// Ends the thread and waits for it. Dropping the watcher does the same.
    pub fn stop(self) {}
}

impl Drop for DeviceWatcher {
    fn drop(&mut self) {
        let _ = self.stop.send(Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

type Sink = Box<dyn FnMut(DevicesEvent)>;

// In the entries below the listener is declared before its proxy so that it is dropped first.
struct DeviceEntry {
    facts: DeviceFacts,
    /// Answers so far to the running `enum_params` calls (EnumRoute, Route). `None`: nothing is being asked.
    pending: Option<[Vec<RouteParam>; 2]>,
    /// The params changed again while an answer was still on its way; ask once more after it.
    ask_again: bool,
    _listener: DeviceListener,
    proxy: Device,
}

impl DeviceEntry {
    /// The seq that comes back with a param is PipeWire's own, not the one passed here, so answers
    /// cannot be told apart: never have two questions out at once.
    fn ask_routes(&mut self) {
        if self.pending.is_some() {
            self.ask_again = true;
            return;
        }
        self.ask_again = false;
        self.pending = Some([Vec::new(), Vec::new()]);
        self.proxy
            .enum_params(0, Some(ParamType::EnumRoute), 0, u32::MAX);
        self.proxy
            .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
    }
}

struct NodeEntry {
    facts: Option<NodeFacts>,
    _listener: NodeListener,
    _proxy: Node,
}

struct SettingsEntry {
    id: u32,
    _listener: MetadataListener,
    _proxy: Metadata,
}

#[derive(Default)]
struct State {
    devices: BTreeMap<u32, DeviceEntry>,
    nodes: BTreeMap<u32, NodeEntry>,
    settings: Option<SettingsEntry>,
    clock: ClockSettings,
    /// The newest roundtrip we are waiting for. When it comes back, everything asked before it has been answered.
    waiting: Option<AsyncSeq>,
    dirty: bool,
    last_sent: Option<DevicesPicture>,
}

struct Shared {
    state: RefCell<State>,
    sink: RefCell<Sink>,
    registry: RegistryRc,
    core: CoreRc,
}

fn run(
    sink: Sink,
    stop_rx: pw::channel::Receiver<Stop>,
    ready: mpsc::Sender<Result<(), DevicesError>>,
) {
    pw::init();

    let connect = || -> Result<_, pw::Error> {
        let main_loop = pw::main_loop::MainLoopRc::new(None)?;
        let context = pw::context::ContextRc::new(&main_loop, None)?;
        let core = context.connect_rc(None)?;
        let registry = core.get_registry_rc()?;
        Ok((main_loop, core, registry))
    };
    let (main_loop, core, registry) = match connect() {
        Ok(parts) => parts,
        Err(e) => {
            let _ = ready.send(Err(DevicesError::PipeWire(e.to_string())));
            return;
        }
    };

    // Callbacks hold only weak references; this `Rc` owns every proxy and is dropped before the core.
    let shared = Rc::new(Shared {
        state: RefCell::new(State::default()),
        sink: RefCell::new(sink),
        registry,
        core,
    });

    let stopped = Rc::new(Cell::new(false));
    let _stop = stop_rx.attach(main_loop.loop_(), {
        let main_loop = main_loop.clone();
        let stopped = stopped.clone();
        move |_| {
            stopped.set(true);
            main_loop.quit();
        }
    });

    let _core_listener = shared
        .core
        .add_listener_local()
        .done({
            let weak = Rc::downgrade(&shared);
            move |id, seq| {
                if let Some(shared) = weak.upgrade() {
                    shared.on_done(id, seq);
                }
            }
        })
        .error({
            let main_loop = main_loop.downgrade();
            move |id, _seq, _res, _message| {
                // An error on the core itself means the connection is gone.
                if id == PW_ID_CORE
                    && let Some(main_loop) = main_loop.upgrade()
                {
                    main_loop.quit();
                }
            }
        })
        .register();

    let _registry_listener = shared
        .registry
        .add_listener_local()
        .global({
            let weak = Rc::downgrade(&shared);
            move |global| {
                if let Some(shared) = weak.upgrade() {
                    shared.on_global(global);
                }
            }
        })
        .global_remove({
            let weak = Rc::downgrade(&shared);
            move |id| {
                if let Some(shared) = weak.upgrade() {
                    shared.on_global_remove(id);
                }
            }
        })
        .register();

    // Guarantees a first picture even when there is nothing to bind.
    shared.touch();

    let _ = ready.send(Ok(()));
    main_loop.run();
    if !stopped.get() {
        (shared.sink.borrow_mut())(DevicesEvent::Lost);
    }
}

impl Shared {
    /// Marks the picture as stale and asks for a roundtrip; the picture is rebuilt when it returns.
    fn touch(&self) {
        let mut state = self.state.borrow_mut();
        state.dirty = true;
        state.waiting = self.core.sync(0).ok();
    }

    fn on_global(self: &Rc<Self>, global: &GlobalObject<&DictRef>) {
        let Some(props) = global.props else { return };
        let id = global.id;
        match global.type_ {
            ObjectType::Device if props.get("media.class") == Some("Audio/Device") => {
                let Ok(proxy) = self.registry.bind::<Device, _>(global) else {
                    return;
                };
                let listener = proxy
                    .add_listener_local()
                    .info({
                        let weak = Rc::downgrade(self);
                        move |info| {
                            if let Some(shared) = weak.upgrade() {
                                shared.on_device_info(id, info);
                            }
                        }
                    })
                    .param({
                        let weak = Rc::downgrade(self);
                        move |_seq, param, _index, _next, pod| {
                            if let Some(shared) = weak.upgrade() {
                                shared.on_device_param(id, param, pod);
                            }
                        }
                    })
                    .register();
                let facts = DeviceFacts {
                    id,
                    name: props.get("device.name").unwrap_or_default().to_owned(),
                    ..Default::default()
                };
                self.state.borrow_mut().devices.insert(
                    id,
                    DeviceEntry {
                        facts,
                        pending: None,
                        ask_again: false,
                        _listener: listener,
                        proxy,
                    },
                );
            }
            ObjectType::Node
                if props
                    .get("media.class")
                    .is_some_and(|c| NodeClass::from_media_class(c).is_some()) =>
            {
                let Ok(proxy) = self.registry.bind::<Node, _>(global) else {
                    return;
                };
                let listener = proxy
                    .add_listener_local()
                    .info({
                        let weak = Rc::downgrade(self);
                        move |info| {
                            if let Some(shared) = weak.upgrade() {
                                shared.on_node_info(id, info);
                            }
                        }
                    })
                    .register();
                self.state.borrow_mut().nodes.insert(
                    id,
                    NodeEntry {
                        facts: None,
                        _listener: listener,
                        _proxy: proxy,
                    },
                );
            }
            ObjectType::Metadata if props.get("metadata.name") == Some("settings") => {
                let Ok(proxy) = self.registry.bind::<Metadata, _>(global) else {
                    return;
                };
                let listener = proxy
                    .add_listener_local()
                    .property({
                        let weak = Rc::downgrade(self);
                        move |subject, key, _type, value| {
                            if subject == 0
                                && let Some(shared) = weak.upgrade()
                            {
                                shared.on_setting(key, value);
                            }
                            0
                        }
                    })
                    .register();
                self.state.borrow_mut().settings = Some(SettingsEntry {
                    id,
                    _listener: listener,
                    _proxy: proxy,
                });
            }
            _ => return,
        }
        self.touch();
    }

    fn on_global_remove(&self, id: u32) {
        {
            let mut state = self.state.borrow_mut();
            let was_device = state.devices.remove(&id).is_some();
            let was_node = state.nodes.remove(&id).is_some();
            let was_settings = state.settings.as_ref().is_some_and(|s| s.id == id);
            if was_settings {
                state.settings = None;
                state.clock = ClockSettings::default();
            }
            if !(was_device || was_node || was_settings) {
                return;
            }
        }
        self.touch();
    }

    fn on_device_info(&self, id: u32, info: &DeviceInfoRef) {
        {
            let mut state = self.state.borrow_mut();
            let Some(entry) = state.devices.get_mut(&id) else {
                return;
            };
            if let Some(props) = info.props() {
                if let Some(name) = props.get("device.name") {
                    entry.facts.name = name.to_owned();
                }
                entry.facts.description = props.get("device.description").map(str::to_owned);
                entry.facts.bus = props.get("device.bus").map(str::to_owned);
                entry.facts.api = props.get("device.api").map(str::to_owned);
            }
            // Read the raw mask: `DeviceInfoRef::change_mask` panics on bits it does not know.
            let params_changed =
                info.as_raw().change_mask & u64::from(pw::sys::PW_DEVICE_CHANGE_MASK_PARAMS) != 0;
            if params_changed {
                entry.ask_routes();
            }
        }
        self.touch();
    }

    fn on_device_param(&self, id: u32, param: ParamType, pod: Option<&Pod>) {
        let slot = match param {
            ParamType::EnumRoute => 0,
            ParamType::Route => 1,
            _ => return,
        };
        let mut state = self.state.borrow_mut();
        let Some(entry) = state.devices.get_mut(&id) else {
            return;
        };
        if let Some(pending) = &mut entry.pending
            && let Some(route) = pod.and_then(|p| route_from_pod_bytes(p.as_bytes()))
        {
            pending[slot].push(route);
        }
    }

    fn on_node_info(&self, id: u32, info: &NodeInfoRef) {
        let Some(props) = info.props() else { return };
        let Some(class) = props
            .get("media.class")
            .and_then(NodeClass::from_media_class)
        else {
            return;
        };
        let number = |key: &str| props.get(key).and_then(|v| v.parse::<u32>().ok());
        let facts = Some(NodeFacts {
            id,
            class,
            name: props.get("node.name").unwrap_or_default().to_owned(),
            description: props.get("node.description").map(str::to_owned),
            device_id: number("device.id"),
            profile_device: number("card.profile.device"),
        });
        {
            let mut state = self.state.borrow_mut();
            let Some(entry) = state.nodes.get_mut(&id) else {
                return;
            };
            // Node info also arrives on every state change (running, suspended); those are not news here.
            if entry.facts == facts {
                return;
            }
            entry.facts = facts;
        }
        self.touch();
    }

    fn on_setting(&self, key: Option<&str>, value: Option<&str>) {
        {
            let mut state = self.state.borrow_mut();
            let before = state.clock;
            state.clock.apply(key, value);
            if state.clock == before {
                return;
            }
        }
        self.touch();
    }

    fn on_done(&self, id: u32, seq: AsyncSeq) {
        let picture = {
            let mut state = self.state.borrow_mut();
            if id != PW_ID_CORE || state.waiting != Some(seq) {
                return;
            }
            state.waiting = None;

            let mut asked_again = false;
            for entry in state.devices.values_mut() {
                if let Some([enum_routes, active_routes]) = entry.pending.take() {
                    entry.facts.enum_routes = enum_routes;
                    entry.facts.active_routes = active_routes;
                }
                if entry.ask_again {
                    entry.ask_routes();
                    asked_again = true;
                }
            }
            if asked_again {
                // What we hold is already out of date; the picture follows the next roundtrip.
                state.dirty = true;
                state.waiting = self.core.sync(0).ok();
                return;
            }
            if !state.dirty {
                return;
            }
            state.dirty = false;

            let devices: Vec<DeviceFacts> =
                state.devices.values().map(|e| e.facts.clone()).collect();
            let nodes: Vec<NodeFacts> = state
                .nodes
                .values()
                .filter_map(|e| e.facts.clone())
                .collect();
            let picture = build_picture(&devices, &nodes, state.clock);
            if state.last_sent.as_ref() == Some(&picture) {
                return;
            }
            state.last_sent = Some(picture.clone());
            picture
        };
        (self.sink.borrow_mut())(DevicesEvent::Picture(picture));
    }
}
