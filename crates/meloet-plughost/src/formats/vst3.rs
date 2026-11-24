//! VST3 behind [`Processor`] and [`Controller`]. Nothing COM-shaped leaves this module.
//!
//! A plugin is one [`Instance`] (its component, its controller and what connects them) that
//! both halves hold. The processor half calls `process` and nothing else that the main thread
//! also calls; the controller half does everything else. The one call that needs the audio
//! thread out of the plugin, switching it off and on again after `kLatencyChanged`, goes
//! through the instance's [`Gate`].
//!
//! A module (the bundle's shared library) is loaded once and stays loaded until the process
//! ends: unloading a plugin's library is where hosts crash, and this process ends when its
//! last plugin goes.

mod com;
pub mod runloop;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{CString, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{
    AtomicBool, AtomicU32, AtomicU64, Ordering::Acquire, Ordering::Relaxed, Ordering::Release,
};
use std::time::{Duration, Instant};

use meloet_plugin::ipc::{FromPlugin, MAX_LATENCY_FRAMES, ToPlugin, Transport};
use meloet_plugin::wire::{
    LoadedPlugin, OutputSet, OutputSettings, ParamInfo, ParamNow, ParamShape,
};
use meloet_plugin::{EditorKind, PluginInfo, PluginKey, PluginKind, PluginRole};
use vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, BusDirections_, BusInfo, ControllerNumbers_, Event,
    Event_::EventTypes_, Event__type0, IAudioProcessor, IAudioProcessorTrait, IComponent,
    IComponentHandler, IComponentTrait, IConnectionPoint, IConnectionPointTrait, IEditController,
    IEditControllerTrait, IEventList, IMidiMapping, IMidiMappingTrait, IParameterChanges,
    IUnitInfo, IUnitInfoTrait, MediaTypes_, NoteOffEvent, NoteOnEvent, ParameterInfo,
    ParameterInfo_::ParameterFlags_, ProcessContext, ProcessContext_::StatesAndFlags_, ProcessData,
    ProcessModes_, ProcessSetup, RestartFlags_, String128, SymbolicSampleSizes_, UnitInfo,
};
use vst3::Steinberg::{
    FUnknown, IBStream, IPlugFrame, IPlugView, IPlugViewTrait, IPluginBaseTrait, IPluginFactory,
    IPluginFactory2, IPluginFactory2Trait, IPluginFactory3, IPluginFactory3Trait,
    IPluginFactoryTrait, PClassInfo, PClassInfo2, PClassInfoW, PFactoryInfo, TUID, ViewRect,
    kNotImplemented, kPlatformTypeX11EmbedWindowID, kResultOk, kResultTrue,
};
use vst3::{ComPtr, ComWrapper, Interface};

use self::com::{EventList, Frame, Handler, HostApp, MemStream, ParamChanges};
use super::outputs::{Outputs, Ports, SetShape, Side, is_on};
use super::{
    Activation, Candidate, Controller, GATE_WAIT, Gate, Loaded, Opened, ProcessBlock, Processor,
    Read, Said, judge_typed,
};
use crate::rtcheck;

const AUDIO_MODULE_CLASS: &str = "Audio Module Class";
/// Events of one block that reach the plugin; a block with more loses the rest.
const BLOCK_EVENTS: usize = 1024;
/// Parameters that change in one block, each way.
const BLOCK_PARAMS: usize = 128;
/// Changes on their way from the audio thread to the controller.
const TO_CONTROLLER: usize = 1024;
/// Controller numbers a plugin can map to parameters: 0..=127, after-touch, pitch bend.
const CONTROLLERS: usize = 130;
const NO_PARAM: u32 = u32::MAX;
/// A parameter with more steps than this is shown as a smooth one.
const MOST_STEPS: i32 = 128;
/// How many blocks the audio thread is given to take in a state or an edit before the
/// controller is brought up to date with it, and how long when no blocks run.
const SETTLE_BLOCKS: u32 = 2;
const SETTLE_WAIT: Duration = Duration::from_millis(200);
/// Of an instrument, the audio output buses that can be output sets: the first ones, one bit
/// each of [`Instance::buses_on`].
const MOST_BUSES: usize = 64;

struct Module {
    factory: ComPtr<IPluginFactory>,
    host: ComWrapper<HostApp>,
    /// Asked once: a factory with hundreds of classes (LSP) answers slowly.
    classes: Vec<ClassInfo>,
}

thread_local! {
    /// The modules of this process, by bundle. Main thread only; never emptied.
    static MODULES: RefCell<HashMap<PathBuf, Rc<Module>>> = RefCell::new(HashMap::new());
}

fn library(bundle: &Path) -> Result<PathBuf, String> {
    let stem = bundle.file_stem().ok_or("a bundle without a name")?;
    let mut file = stem.to_os_string();
    file.push(".so");
    Ok(bundle
        .join("Contents")
        .join(format!("{}-linux", std::env::consts::ARCH))
        .join(file))
}

fn open(bundle: &Path) -> Result<Module, String> {
    let path = library(bundle)?;
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    // SAFETY: loading a plugin's library runs its code; that is what this process is for.
    // The symbols are the ones VST3 names, with the signatures VST3 gives them.
    unsafe {
        let handle = libc::dlopen(c_path.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
        if handle.is_null() {
            let why = libc::dlerror();
            let why = if why.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(why).to_string_lossy().into_owned()
            };
            return Err(format!("dlopen {}: {why}", path.display()));
        }
        let entry = libc::dlsym(handle, c"ModuleEntry".as_ptr());
        if !entry.is_null() {
            let entry: unsafe extern "C" fn(*mut c_void) -> bool = std::mem::transmute(entry);
            if !entry(handle) {
                return Err("ModuleEntry said no".into());
            }
        }
        let get = libc::dlsym(handle, c"GetPluginFactory".as_ptr());
        if get.is_null() {
            return Err("no GetPluginFactory".into());
        }
        let get: unsafe extern "system" fn() -> *mut IPluginFactory = std::mem::transmute(get);
        let factory = ComPtr::from_raw(get()).ok_or("GetPluginFactory gave nothing")?;
        let host = ComWrapper::new(HostApp);
        if let (Some(factory3), Some(context)) = (
            factory.cast::<IPluginFactory3>(),
            host.as_com_ref::<FUnknown>(),
        ) {
            factory3.setHostContext(context.as_ptr());
        }
        let classes = classes(&factory);
        Ok(Module {
            factory,
            host,
            classes,
        })
    }
}

fn module(bundle: &Path) -> Result<Rc<Module>, String> {
    if let Some(module) = MODULES.with_borrow(|m| m.get(bundle).cloned()) {
        return Ok(module);
    }
    let module = Rc::new(open(bundle)?);
    MODULES.with_borrow_mut(|m| m.insert(bundle.to_path_buf(), module.clone()));
    Ok(module)
}

struct ClassInfo {
    cid: TUID,
    id: String,
    name: String,
    vendor: String,
    version: String,
    sub_categories: String,
}

fn hex(cid: &TUID) -> String {
    cid.iter().map(|b| format!("{:02X}", *b as u8)).collect()
}

/// The Audio Module Classes of a factory.
fn classes(factory: &ComPtr<IPluginFactory>) -> Vec<ClassInfo> {
    let factory2 = factory.cast::<IPluginFactory2>();
    let factory3 = factory.cast::<IPluginFactory3>();
    let mut found = Vec::new();
    // SAFETY: the structs are plain data the factory fills; the calls are the factory's own.
    unsafe {
        let mut maker: PFactoryInfo = std::mem::zeroed();
        factory.getFactoryInfo(&mut maker);
        for index in 0..factory.countClasses() {
            let mut info: PClassInfo = std::mem::zeroed();
            if factory.getClassInfo(index, &mut info) != kResultOk
                || com::read_chars(&info.category) != AUDIO_MODULE_CLASS
            {
                continue;
            }
            let mut class = ClassInfo {
                cid: info.cid,
                id: hex(&info.cid),
                name: com::read_chars(&info.name),
                vendor: com::read_chars(&maker.vendor),
                version: String::new(),
                sub_categories: String::new(),
            };
            let mut wide: PClassInfoW = std::mem::zeroed();
            let mut narrow: PClassInfo2 = std::mem::zeroed();
            if let Some(factory3) = &factory3
                && factory3.getClassInfoUnicode(index, &mut wide) == kResultOk
            {
                class.name = com::read_utf16(&wide.name);
                class.version = com::read_utf16(&wide.version);
                class.sub_categories = com::read_chars(&wide.subCategories);
                let vendor = com::read_utf16(&wide.vendor);
                if !vendor.is_empty() {
                    class.vendor = vendor;
                }
            } else if let Some(factory2) = &factory2
                && factory2.getClassInfo2(index, &mut narrow) == kResultOk
            {
                class.version = com::read_chars(&narrow.version);
                class.sub_categories = com::read_chars(&narrow.subCategories);
                let vendor = com::read_chars(&narrow.vendor);
                if !vendor.is_empty() {
                    class.vendor = vendor;
                }
            }
            found.push(class);
        }
    }
    found
}

/// The classes `moduleinfo.json` lists, when the bundle has one that can be read. The module
/// is not loaded for this.
fn listed(bundle: &Path) -> Option<Vec<Candidate>> {
    let text = [
        "Contents/Resources/moduleinfo.json",
        "Contents/moduleinfo.json",
    ]
    .iter()
    .find_map(|at| std::fs::read(bundle.join(at)).ok())?;
    let info: serde_json::Value = serde_json::from_slice(&text).ok()?;
    let mut found = Vec::new();
    for class in info.get("Classes")?.as_array()? {
        if class.get("Category")?.as_str()? != AUDIO_MODULE_CLASS {
            continue;
        }
        let key = PluginKey::vst3(class.get("CID")?.as_str()?).ok()?;
        found.push(Candidate {
            id: key.id().to_string(),
            name: class
                .get("Name")
                .and_then(|n| n.as_str())
                .map(str::to_owned),
        });
    }
    Some(found)
}

/// The plugins of the bundle; `Err` when the bundle cannot be read.
pub fn list(bundle: &Path) -> Result<Vec<Candidate>, String> {
    if let Some(found) = listed(bundle) {
        return Ok(found);
    }
    let module = module(bundle)?;
    Ok(module
        .classes
        .iter()
        .map(|class| Candidate {
            id: class.id.clone(),
            name: Some(class.name.clone()),
        })
        .collect())
}

fn create<I: Interface>(factory: &ComPtr<IPluginFactory>, cid: &TUID) -> Result<ComPtr<I>, String> {
    let mut obj = std::ptr::null_mut();
    // SAFETY: a class id of this factory, the id of the interface asked for, and a place for
    // the result, which is checked.
    unsafe {
        let result = factory.createInstance(cid.as_ptr(), I::IID.as_ptr().cast(), &mut obj);
        ComPtr::from_raw(obj.cast::<I>())
            .filter(|_| result == kResultOk)
            .ok_or_else(|| format!("createInstance failed ({result})"))
    }
}

/// One plugin: its component, its controller, and what the two halves share.
struct Instance {
    // In the order they are let go of: the component last (DPF complains otherwise).
    connection: Option<(ComPtr<IConnectionPoint>, ComPtr<IConnectionPoint>)>,
    controller: Option<ComPtr<IEditController>>,
    processor: ComPtr<IAudioProcessor>,
    component: ComPtr<IComponent>,
    /// The controller is an object of its own, to be ended by itself.
    separate: bool,
    handler: ComWrapper<Handler>,
    /// Kept for as long as the plugin, which holds a pointer to it.
    _host: ComWrapper<HostApp>,
    active: AtomicBool,
    gate: Gate,
    latency: AtomicU32,
    /// The blocks the plugin has processed.
    blocks: AtomicU32,
    /// Of an instrument's audio output buses, one bit each: the bus is active. Changed by
    /// the main thread while it holds `gate`.
    buses_on: AtomicU64,
    /// Of an instrument's audio output buses, one bit each: the bus's output set is silent
    /// ([`Outputs::silent`]) after the last block.
    silent: AtomicU64,
}

impl Instance {
    /// Makes the component and its controller and connects them. Main thread.
    fn create(module: &Module, cid: &TUID) -> Result<Instance, String> {
        let context = module
            .host
            .as_com_ref::<FUnknown>()
            .ok_or("no host context")?
            .as_ptr();
        let component = create::<IComponent>(&module.factory, cid)?;
        // SAFETY: calls of VST3 in the order VST3 gives, on objects just made.
        unsafe {
            if component.initialize(context) != kResultOk {
                return Err("the component did not initialize".into());
            }
            let Some(processor) = component.cast::<IAudioProcessor>() else {
                component.terminate();
                return Err("no IAudioProcessor".into());
            };
            let mut separate = false;
            let mut controller = component.cast::<IEditController>();
            if controller.is_none() {
                let mut class: TUID = [0; 16];
                if component.getControllerClassId(&mut class) == kResultOk
                    && let Ok(made) = create::<IEditController>(&module.factory, &class)
                    && made.initialize(context) == kResultOk
                {
                    separate = true;
                    controller = Some(made);
                }
            }
            let mut connection = None;
            if separate
                && let Some(controller) = &controller
                && let (Some(a), Some(b)) = (
                    component.cast::<IConnectionPoint>(),
                    controller.cast::<IConnectionPoint>(),
                )
            {
                a.connect(b.as_ptr());
                b.connect(a.as_ptr());
                connection = Some((a, b));
            }
            Ok(Instance {
                component,
                processor,
                controller,
                separate,
                connection,
                handler: ComWrapper::new(Handler::default()),
                _host: module.host.clone(),
                active: AtomicBool::new(false),
                gate: Gate::default(),
                latency: AtomicU32::new(0),
                blocks: AtomicU32::new(0),
                buses_on: AtomicU64::new(0),
                silent: AtomicU64::new(0),
            })
        }
    }

    /// The name and the channels of each audio bus in one direction.
    fn buses(&self, direction: i32) -> Vec<(String, i32)> {
        // SAFETY: plain questions to the component; `info` is filled by it.
        unsafe {
            let audio = MediaTypes_::kAudio as i32;
            (0..self.component.getBusCount(audio, direction))
                .map(|index| {
                    let mut info: BusInfo = std::mem::zeroed();
                    self.component
                        .getBusInfo(audio, direction, index, &mut info);
                    (com::read_utf16(&info.name), info.channelCount.max(0))
                })
                .collect()
        }
    }

    /// Asks for the view without attaching it anywhere, so no window appears.
    fn editor(&self) -> EditorKind {
        let Some(controller) = &self.controller else {
            return EditorKind::None;
        };
        // SAFETY: `createView` gives an owned view or null.
        unsafe {
            match ComPtr::from_raw(controller.createView(c"editor".as_ptr())) {
                None => EditorKind::None,
                Some(view) => {
                    if view.isPlatformTypeSupported(kPlatformTypeX11EmbedWindowID) == kResultTrue {
                        EditorKind::X11
                    } else {
                        EditorKind::Unsupported
                    }
                }
            }
        }
    }

    fn read_latency(&self) -> u32 {
        // SAFETY: a plain question.
        unsafe { self.processor.getLatencySamples() }.min(MAX_LATENCY_FRAMES)
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: the way VST3 ends a plugin, on the main thread, after the audio thread gave
        // its half back.
        unsafe {
            if self.active.load(Relaxed) {
                self.processor.setProcessing(0);
                self.component.setActive(0);
            }
            if let Some(controller) = &self.controller {
                controller.setComponentHandler(std::ptr::null_mut());
            }
            if let Some((a, b)) = self.connection.take() {
                a.disconnect(b.as_ptr());
                b.disconnect(a.as_ptr());
            }
            if self.separate
                && let Some(controller) = &self.controller
            {
                controller.terminate();
            }
            self.component.terminate();
        }
    }
}

fn find<'a>(module: &'a Module, id: &str) -> Result<&'a ClassInfo, String> {
    module
        .classes
        .iter()
        .find(|class| class.id == id)
        .ok_or_else(|| format!("no class {id} in the bundle"))
}

/// Reads one plugin of [`list`]; `Err` when that plugin cannot be read.
pub fn probe(bundle: &Path, id: &str) -> Result<PluginInfo, String> {
    let module = module(bundle)?;
    let class = find(&module, id)?;
    let instance = Instance::create(&module, &class.cid)?;
    let main = |direction| {
        let channels = instance.buses(direction).first().map_or(0, |bus| bus.1);
        channels as u32
    };
    let info = PluginInfo {
        key: PluginKey::vst3(&class.id).map_err(|e| e.to_string())?,
        name: class.name.clone(),
        vendor: class.vendor.clone(),
        version: class.version.clone(),
        kind: PluginKind::from_vst3(&[class.sub_categories.as_str()]),
        editor: instance.editor(),
        audio_inputs: main(BusDirections_::kInput as i32),
        audio_outputs: main(BusDirections_::kOutput as i32),
    };
    Ok(info)
}

/// What the plugin is told of how its blocks come, when it is made ready and again with
/// every block: both, because plugins read it in either place (JUCE's wrapper in both).
fn process_mode(offline: bool) -> i32 {
    if offline {
        ProcessModes_::kOffline as i32
    } else {
        ProcessModes_::kRealtime as i32
    }
}

/// Where one audio output bus of an instrument is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BusPlace {
    channels: i32,
    /// Its first channel among the processor's `out_ptrs`.
    first: usize,
    /// Its output set, and the port of its first channel in [`BusSets::out`].
    set: Option<(usize, usize)>,
}

/// The output sets of an instrument's audio output buses (docs/architecture.md 20-15), from
/// each bus's name and channels: one for each bus that has channels (of the first
/// [`MOST_BUSES`]), `bus<index>`, of its first two channels, heard by default when it is the
/// first. Also where each bus is, and what the default rule hears.
fn bus_sets(buses: &[(String, i32)]) -> (Vec<SetShape>, Vec<(usize, Side)>, Vec<BusPlace>) {
    let (mut sets, mut heard, mut places) = (Vec::new(), Vec::new(), Vec::new());
    let (mut first, mut port) = (0, 0);
    for (index, (name, channels)) in buses.iter().enumerate() {
        let channels = (*channels).max(0);
        let mut set = None;
        if channels > 0 && index < MOST_BUSES {
            let ports = if channels == 1 {
                Ports::Mono(port)
            } else {
                Ports::Stereo {
                    left: port,
                    right: port + 1,
                }
            };
            let on = index == 0;
            if on {
                match ports {
                    Ports::Mono(at) => heard.push((at, Side::Both)),
                    Ports::Stereo { left, right } => {
                        heard.extend([(left, Side::Left), (right, Side::Right)]);
                    }
                }
            }
            let id = format!("bus{index}");
            set = Some((sets.len(), port));
            sets.push(SetShape {
                name: if name.is_empty() {
                    id.clone()
                } else {
                    name.clone()
                },
                id,
                ports,
                on,
                pan: 0.0,
            });
            port += channels.min(2) as usize;
        }
        places.push(BusPlace {
            channels,
            first,
            set,
        });
        first += channels as usize;
    }
    (sets, heard, places)
}

/// `outputs`: the settings of the output sets of an instrument; the buses they have heard are
/// active from the start.
pub fn load(
    key: &PluginKey,
    bundle: &Path,
    activation: Activation,
    outputs: &OutputSettings,
) -> Result<Loaded, String> {
    let module = module(bundle)?;
    let class = find(&module, key.id())?;
    let instance = Instance::create(&module, &class.cid)?;
    let (audio, event) = (MediaTypes_::kAudio as i32, MediaTypes_::kEvent as i32);
    let (input, output) = (
        BusDirections_::kInput as i32,
        BusDirections_::kOutput as i32,
    );
    let in_buses = instance.buses(input);
    let out_buses = instance.buses(output);
    let instrument =
        PluginKind::from_vst3(&[class.sub_categories.as_str()]).role() == PluginRole::Instrument;
    let sets = instrument.then(|| {
        let (shapes, heard, places) = bus_sets(&out_buses);
        let ports: usize = (shapes.iter())
            .map(|set| match set.ports {
                Ports::Mono(_) => 1,
                Ports::Stereo { .. } => 2,
            })
            .sum();
        let mut sets = Outputs::new(shapes, heard, true, activation.sample_rate);
        sets.set(outputs, false);
        BusSets {
            outputs: sets,
            out: vec![0.0; ports * activation.max_frames].into_boxed_slice(),
            frames: activation.max_frames,
            places: places.into_boxed_slice(),
        }
    });
    let list = sets.as_ref().map_or(Vec::new(), |sets| sets.outputs.list());
    // Each output set with its bus.
    let set_buses: Vec<(OutputSet, usize)> = (sets.iter())
        .flat_map(|sets| sets.places.iter().enumerate())
        .filter_map(|(bus, place)| Some((list[place.set?.0].clone(), bus)))
        .collect();
    // The first bus, and those the settings have heard.
    let buses_on = (set_buses.iter())
        .filter(|(set, bus)| *bus > 0 && is_on(set, outputs))
        .fold(1u64, |on, (_, bus)| on | 1 << bus);
    let mut cc = Box::new([NO_PARAM; CONTROLLERS]);
    // SAFETY: the calls VST3 makes a plugin ready with, in its order, on the main thread.
    let event_input = unsafe {
        if let Some(controller) = &instance.controller {
            if let Some(handler) = instance.handler.as_com_ref::<IComponentHandler>() {
                controller.setComponentHandler(handler.as_ptr());
            }
            // The controller starts from the component's state.
            let stream = ComWrapper::new(MemStream::default());
            if let Some(stream_ref) = stream.as_com_ref::<IBStream>()
                && instance.component.getState(stream_ref.as_ptr()) == kResultOk
            {
                stream.rewind();
                controller.setComponentState(stream_ref.as_ptr());
            }
            if let Some(mapping) = controller.cast::<IMidiMapping>() {
                for (number, id) in cc.iter_mut().enumerate() {
                    let mut param = 0;
                    if mapping.getMidiControllerAssignment(0, 0, number as i16, &mut param)
                        == kResultOk
                    {
                        *id = param;
                    }
                }
            }
        }
        // Only the main buses get sound, and the output buses whose sets are heard; every
        // other bus is switched off.
        for (direction, buses, on) in [(input, &in_buses, 1), (output, &out_buses, buses_on)] {
            for index in 0..buses.len() as i32 {
                let active = index == 0 || (index < MOST_BUSES as i32 && on & 1 << index != 0);
                instance
                    .component
                    .activateBus(audio, direction, index, u8::from(active));
            }
        }
        let event_input = instance.component.getBusCount(event, input) > 0;
        if event_input {
            instance.component.activateBus(event, input, 0, 1);
        }
        let mut setup = ProcessSetup {
            processMode: process_mode(activation.offline),
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            maxSamplesPerBlock: activation.max_frames as i32,
            sampleRate: activation.sample_rate,
        };
        let result = instance.processor.setupProcessing(&mut setup);
        if result != kResultOk && result != kNotImplemented {
            return Err(format!("setupProcessing failed ({result})"));
        }
        let result = instance.component.setActive(1);
        if result != kResultOk && result != kNotImplemented {
            return Err(format!("setActive failed ({result})"));
        }
        instance.active.store(true, Relaxed);
        instance.buses_on.store(buses_on, Relaxed);
        instance.latency.store(instance.read_latency(), Relaxed);
        instance.processor.setProcessing(1);
        event_input
    };

    let frames = activation.max_frames;
    let bus = |buses: &[(String, i32)]| -> Box<[AudioBusBuffers]> {
        buses
            .iter()
            .enumerate()
            .map(|(index, &(_, count))| AudioBusBuffers {
                numChannels: if index == 0 { count } else { 0 },
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: std::ptr::null_mut(),
                },
            })
            .collect()
    };
    let main = |buses: &[(String, i32)]| buses.first().map_or(0, |bus| bus.1) as usize;
    let (in_channels, out_channels) = (main(&in_buses), main(&out_buses));
    // Of an instrument, every channel of every output bus has a place.
    let out_ptrs = match &sets {
        Some(_) => out_buses.iter().map(|bus| bus.1 as usize).sum(),
        None => out_channels,
    };
    let (to_controller, from_audio) = rtrb::RingBuffer::new(TO_CONTROLLER);
    let instance = Arc::new(instance);
    let plugin = LoadedPlugin {
        latency: instance.latency.load(Relaxed),
        audio_inputs: in_channels as u32,
        audio_outputs: out_channels as u32,
        editor: instance.editor(),
        // VST3 has no word for it.
        live: false,
        outputs: list,
        // Read from the controller once it is made (`crate::control`).
        key_names: None,
    };
    let processor = Vst3Processor {
        instance: instance.clone(),
        events: ComWrapper::new(EventList::with_capacity(if event_input {
            BLOCK_EVENTS
        } else {
            0
        })),
        events_out: ComWrapper::new(EventList::with_capacity(0)),
        changes: ComWrapper::new(ParamChanges::with_capacity(BLOCK_PARAMS)),
        changes_out: ComWrapper::new(ParamChanges::with_capacity(BLOCK_PARAMS)),
        in_buses: bus(&in_buses),
        out_buses: bus(&out_buses),
        in_ptrs: vec![std::ptr::null_mut(); in_channels].into_boxed_slice(),
        out_ptrs: vec![std::ptr::null_mut(); out_ptrs].into_boxed_slice(),
        silence: vec![0.0; frames].into_boxed_slice(),
        spare: vec![0.0; frames].into_boxed_slice(),
        mono: vec![0.0; frames].into_boxed_slice(),
        cc,
        to_controller,
        told: Vec::with_capacity(BLOCK_PARAMS),
        sample_rate: activation.sample_rate,
        mode: process_mode(activation.offline),
        frames_run: 0,
        sets,
    };
    let controller = Vst3Controller {
        view: None,
        instance,
        from_audio,
        units: None,
        settle: None,
        sets: set_buses,
        fading: 0,
        fading_from: 0,
        later: Vec::new(),
    };
    Ok(Loaded {
        plugin,
        controller: Box::new(controller),
        processor: Box::new(processor),
    })
}

struct Vst3Processor {
    instance: Arc<Instance>,
    events: ComWrapper<EventList>,
    events_out: ComWrapper<EventList>,
    changes: ComWrapper<ParamChanges>,
    changes_out: ComWrapper<ParamChanges>,
    /// One for every audio bus; only the active ones have channels.
    in_buses: Box<[AudioBusBuffers]>,
    out_buses: Box<[AudioBusBuffers]>,
    /// The channels of the main input bus, and of the main output bus of an effect or every
    /// output bus of an instrument, pointed anew at every block.
    in_ptrs: Box<[*mut f32]>,
    out_ptrs: Box<[*mut f32]>,
    silence: Box<[f32]>,
    /// Where the outputs past the second go. Never read.
    spare: Box<[f32]>,
    /// The input of a plugin with one input channel.
    mono: Box<[f32]>,
    cc: Box<[u32; CONTROLLERS]>,
    to_controller: rtrb::Producer<(u32, f64)>,
    /// The parameters whose value the window was told in this block.
    told: Vec<u32>,
    sample_rate: f64,
    /// `ProcessData::processMode` of every block.
    mode: i32,
    frames_run: i64,
    /// Of an instrument: its output buses as output sets. An effect's main output bus goes
    /// straight to the pair.
    sets: Option<BusSets>,
}

// SAFETY: the pointers point into buffers this struct owns or are set anew before each use,
// and the struct is used by one thread at a time (the audio thread, then the main thread that
// drops it).
unsafe impl Send for Vst3Processor {}

/// An instrument's audio output buses as output sets.
struct BusSets {
    outputs: Outputs,
    /// The ports of the sets, `frames` each: the first two channels of each bus (of a bus of
    /// one channel, that one).
    out: Box<[f32]>,
    frames: usize,
    /// By audio output bus.
    places: Box<[BusPlace]>,
}

impl BusSets {
    fn active(active: u64, bus: usize) -> bool {
        bus < MOST_BUSES && active & 1 << bus != 0
    }

    /// Points the active output buses (`active`, a bit each) at the ports. The ports of the
    /// others are silence, and their sets are not measured.
    fn point(
        &mut self,
        active: u64,
        ptrs: &mut [*mut f32],
        buses: &mut [AudioBusBuffers],
        spare: *mut f32,
    ) {
        self.not_measured(active);
        let frames = self.frames;
        for (index, place) in self.places.iter().enumerate() {
            if let Some((_, port)) = place.set
                && !Self::active(active, index)
            {
                let ports = place.channels.min(2) as usize;
                self.out[port * frames..(port + ports) * frames].fill(0.0);
            }
        }
        // One pointer for all: a write through a slice of `out` would end the ones before.
        let base = self.out.as_mut_ptr();
        for (index, (place, bus)) in self.places.iter().zip(buses.iter_mut()).enumerate() {
            let channels = &mut ptrs[place.first..place.first + place.channels as usize];
            for (channel, ptr) in channels.iter_mut().enumerate() {
                *ptr = match place.set {
                    Some((_, port)) if channel < 2 => base.wrapping_add((port + channel) * frames),
                    _ => spare,
                };
            }
            let on = Self::active(active, index);
            bus.numChannels = if on { place.channels } else { 0 };
            bus.silenceFlags = 0;
            bus.__field0.channelBuffers32 = if on {
                channels.as_mut_ptr()
            } else {
                std::ptr::null_mut()
            };
        }
    }

    fn not_measured(&mut self, active: u64) {
        for (index, place) in self.places.iter().enumerate() {
            if let Some((set, _)) = place.set
                && !Self::active(active, index)
            {
                self.outputs.not_measured(set);
            }
        }
    }

    /// Mixes what the plugin made down to the pair, and tells which sets are silent.
    fn hear(&mut self, block: &mut ProcessBlock<'_>, silent: &AtomicU64) {
        if self.outputs.heard().is_empty() {
            for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
                output.copy_from_slice(input);
            }
            self.outputs.missed(block.outputs[0].len());
        } else {
            let [left, right] = &mut block.outputs;
            self.outputs.mix(&self.out, self.frames, left, right);
        }
        let mut quiet = 0u64;
        for (index, place) in self.places.iter().enumerate() {
            if let Some((set, _)) = place.set
                && self.outputs.silent(set)
            {
                quiet |= 1 << index;
            }
        }
        silent.store(quiet, Relaxed);
    }
}

/// The object as the plugin is handed it, without touching its count of owners.
fn raw<C: vst3::Class, I: Interface>(object: &ComWrapper<C>) -> *mut I {
    object
        .as_com_ref::<I>()
        .map_or(std::ptr::null_mut(), |object| object.as_ptr())
}

fn note_event(frame: u32, on: bool, channel: u8, key: u8, velocity: u8) -> Event {
    let velocity = f32::from(velocity) / 127.0;
    let (channel, pitch) = (i16::from(channel), i16::from(key));
    Event {
        busIndex: 0,
        sampleOffset: frame as i32,
        ppqPosition: 0.0,
        flags: 0,
        r#type: if on {
            EventTypes_::kNoteOnEvent
        } else {
            EventTypes_::kNoteOffEvent
        } as u16,
        __field0: if on {
            Event__type0 {
                noteOn: NoteOnEvent {
                    channel,
                    pitch,
                    tuning: 0.0,
                    velocity,
                    length: 0,
                    noteId: -1,
                },
            }
        } else {
            Event__type0 {
                noteOff: NoteOffEvent {
                    channel,
                    pitch,
                    velocity,
                    noteId: -1,
                    tuning: 0.0,
                },
            }
        },
    }
}

fn context(transport: &Transport, sample_rate: f64, frames_run: i64) -> ProcessContext {
    // SAFETY: plain data, for which all zeroes is a value.
    let mut context: ProcessContext = unsafe { std::mem::zeroed() };
    let mut state = StatesAndFlags_::kTempoValid
        | StatesAndFlags_::kTimeSigValid
        | StatesAndFlags_::kProjectTimeMusicValid
        | StatesAndFlags_::kBarPositionValid
        | StatesAndFlags_::kContTimeValid;
    if transport.flags & Transport::PLAYING != 0 {
        state |= StatesAndFlags_::kPlaying;
    }
    if transport.flags & Transport::RECORDING != 0 {
        state |= StatesAndFlags_::kRecording;
    }
    let number = |x: f64| if x.is_finite() { x } else { 0.0 };
    if transport.flags & Transport::LOOPING != 0 {
        state |= StatesAndFlags_::kCycleActive | StatesAndFlags_::kCycleValid;
        context.cycleStartMusic = number(transport.loop_start_quarters);
        context.cycleEndMusic = number(transport.loop_end_quarters);
    }
    context.state = state;
    context.sampleRate = sample_rate;
    context.projectTimeSamples = transport.position_frames;
    context.continousTimeSamples = frames_run;
    context.projectTimeMusic = number(transport.position_quarters);
    context.barPositionMusic = number(transport.bar_start_quarters);
    context.tempo = if transport.tempo_bpm.is_finite() && transport.tempo_bpm > 0.0 {
        transport.tempo_bpm
    } else {
        120.0
    };
    let meter = |n: u16| if n == 0 { 4 } else { i32::from(n) };
    context.timeSigNumerator = meter(transport.meter_num);
    context.timeSigDenominator = meter(transport.meter_den);
    context
}

impl Vst3Processor {
    fn take(&mut self, event: &ToPlugin) {
        match *event {
            ToPlugin::Midi { frame, data } => {
                let (status, channel) = (data[0] & 0xf0, data[0] & 0x0f);
                let controller = |number: usize, value: f64| (number, value);
                let (number, value) = match status {
                    0x90 if data[2] > 0 => {
                        self.events
                            .push(note_event(frame, true, channel, data[1], data[2]));
                        return;
                    }
                    0x80 | 0x90 => {
                        self.events
                            .push(note_event(frame, false, channel, data[1], data[2]));
                        return;
                    }
                    0xb0 => controller(usize::from(data[1] & 0x7f), f64::from(data[2]) / 127.0),
                    0xd0 => controller(
                        ControllerNumbers_::kAfterTouch as usize,
                        f64::from(data[1]) / 127.0,
                    ),
                    0xe0 => {
                        let bend = u16::from(data[1] & 0x7f) | u16::from(data[2] & 0x7f) << 7;
                        controller(
                            ControllerNumbers_::kPitchBend as usize,
                            f64::from(bend) / 16_383.0,
                        )
                    }
                    _ => return,
                };
                // VST3 has no controller events: a plugin names the parameter that stands for
                // each controller, and gets the controller as a change of that parameter.
                let id = self.cc[number];
                if id != NO_PARAM {
                    self.changes.add(id, frame as i32, value.clamp(0.0, 1.0));
                }
            }
            ToPlugin::Param { frame, id, value } => {
                self.changes.add(id, frame as i32, value);
            }
        }
    }
}

impl Processor for Vst3Processor {
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.outputs[0].len();
        if !self.instance.gate.enter() {
            // The main thread is switching the plugin off and on: this block goes round it.
            for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
                output.copy_from_slice(input);
            }
            if let Some(sets) = &mut self.sets {
                sets.not_measured(self.instance.buses_on.load(Relaxed));
                sets.outputs.missed(frames);
            }
            return;
        }
        self.events.clear();
        self.changes.clear();
        self.changes_out.clear();
        for event in block.events {
            self.take(event);
        }
        // The window is told the last value of each parameter of the block, once.
        self.told.clear();
        for event in block.events.iter().rev() {
            if let ToPlugin::Param { id, value, .. } = *event
                && !self.told.contains(&id)
                && self.told.len() < self.told.capacity()
            {
                self.told.push(id);
                let _ = self.to_controller.push((id, value));
            }
        }

        let [in_left, in_right] = block.inputs;
        let mono_in = self.in_ptrs.len() == 1;
        if mono_in {
            for (mono, (l, r)) in self.mono.iter_mut().zip(in_left.iter().zip(in_right)) {
                *mono = (l + r) * 0.5;
            }
        }
        for (channel, ptr) in self.in_ptrs.iter_mut().enumerate() {
            // The plugin only reads its inputs.
            *ptr = match channel {
                0 if mono_in => self.mono.as_ptr(),
                0 => in_left.as_ptr(),
                1 => in_right.as_ptr(),
                _ => self.silence.as_ptr(),
            }
            .cast_mut();
        }
        if let Some(sets) = &mut self.sets {
            let active = self.instance.buses_on.load(Relaxed);
            let spare = self.spare.as_mut_ptr();
            sets.point(active, &mut self.out_ptrs, &mut self.out_buses, spare);
        } else {
            let [out_left, out_right] = &mut block.outputs;
            for (channel, ptr) in self.out_ptrs.iter_mut().enumerate() {
                *ptr = match channel {
                    0 => out_left.as_mut_ptr(),
                    1 => out_right.as_mut_ptr(),
                    _ => self.spare.as_mut_ptr(),
                };
            }
            if let Some(bus) = self.out_buses.first_mut() {
                bus.silenceFlags = 0;
                bus.__field0.channelBuffers32 = self.out_ptrs.as_mut_ptr();
            }
        }
        if let Some(bus) = self.in_buses.first_mut() {
            bus.silenceFlags = 0;
            bus.__field0.channelBuffers32 = self.in_ptrs.as_mut_ptr();
        }
        let mut context = context(block.transport, self.sample_rate, self.frames_run);
        self.frames_run = self.frames_run.wrapping_add(frames as i64);
        let mut data = ProcessData {
            processMode: self.mode,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            numSamples: frames as i32,
            numInputs: self.in_buses.len() as i32,
            numOutputs: self.out_buses.len() as i32,
            inputs: self.in_buses.as_mut_ptr(),
            outputs: self.out_buses.as_mut_ptr(),
            inputParameterChanges: raw::<_, IParameterChanges>(&self.changes),
            outputParameterChanges: raw::<_, IParameterChanges>(&self.changes_out),
            inputEvents: raw::<_, IEventList>(&self.events),
            outputEvents: raw::<_, IEventList>(&self.events_out),
            processContext: &mut context,
        };
        // SAFETY: every pointer of `data` points at something that lives until the call is
        // over, with room for `frames` frames; the plugin was made ready for blocks this long.
        rtcheck::plugin(|| unsafe { self.instance.processor.process(&mut data) });

        match (&mut self.sets, self.out_ptrs.len()) {
            (Some(sets), _) => sets.hear(block, &self.instance.silent),
            (None, 0) => {
                for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
                    output.copy_from_slice(input);
                }
            }
            (None, 1) => {
                let [left, right] = &mut block.outputs;
                right.copy_from_slice(left);
            }
            (None, _) => {}
        }
        for (id, value) in self.changes_out.last_values() {
            if value.is_finite() {
                let value = value.clamp(0.0, 1.0);
                let _ = self.to_controller.push((id, value));
                block.sink.push(FromPlugin::Param { id, value });
            }
        }
        // After `silent`: a count that moved on tells of it.
        self.instance.blocks.fetch_add(1, Release);
        self.instance.gate.leave();
    }

    fn reset(&mut self) {
        if self.instance.gate.enter() {
            // SAFETY: VST3 lets the audio thread stop and start the processing; a plugin drops
            // what it holds (voices, tails) there.
            rtcheck::plugin(|| unsafe {
                self.instance.processor.setProcessing(0);
                self.instance.processor.setProcessing(1);
            });
            self.instance.gate.leave();
        }
        if let Some(sets) = &mut self.sets {
            sets.outputs.snap();
        }
    }

    fn latency(&self) -> u32 {
        self.instance.latency.load(Relaxed)
    }

    fn outputs(&mut self) -> Option<&mut Outputs> {
        self.sets.as_mut().map(|sets| &mut sets.outputs)
    }
}

struct Vst3Controller {
    // Before `instance`: the view goes before the controller that made it.
    view: Option<(ComPtr<IPlugView>, ComWrapper<Frame>)>,
    instance: Arc<Instance>,
    from_audio: rtrb::Consumer<(u32, f64)>,
    /// What each parameter is measured in, by id; filled when the parameters are first listed.
    units: Option<HashMap<u32, String>>,
    settle: Option<Settle>,
    /// Of an instrument: each output set with its audio output bus.
    sets: Vec<(OutputSet, usize)>,
    /// Output buses that are switched off once their sets are silent, a bit each.
    fading: u64,
    /// `Instance::blocks` when `fading` was set.
    fading_from: u32,
    /// What `poll` tells next.
    later: Vec<Said>,
}

/// The controller's values are to be read anew from the component. Some plugins take a
/// state in with the next block (Surge XT) and change values in their window (a patch)
/// without a word, and their controller (JUCE's) goes on showing the values of before.
struct Settle {
    /// `Instance::blocks` and the time when this was asked for.
    blocks: u32,
    since: Instant,
    /// The list is told to have changed even when no value did (the names may have).
    say: bool,
}

/// The text the plugin shows for a value, with the parameter's unit.
fn value_text(
    controller: &ComPtr<IEditController>,
    id: u32,
    value: f64,
    unit: &str,
) -> Option<String> {
    let mut text: String128 = [0; 128];
    // SAFETY: a plain question on the main thread; `text` is filled by the plugin.
    if unsafe { controller.getParamStringByValue(id, value, &mut text) } != kResultOk {
        return None;
    }
    let mut text = com::read_utf16(&text);
    if !unit.is_empty() {
        text.push(' ');
        text.push_str(unit);
    }
    (!text.is_empty()).then_some(text)
}

fn normal(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

impl Vst3Controller {
    /// The controller learns what the audio thread gave the processor (automation, and the
    /// parameters the plugin moves by itself), as VST3 has the host do.
    fn hear_audio(&mut self) {
        let Some(controller) = &self.instance.controller else {
            while self.from_audio.pop().is_ok() {}
            return;
        };
        while let Ok((id, value)) = self.from_audio.pop() {
            // SAFETY: a plain call on the main thread.
            unsafe { controller.setParamNormalized(id, value) };
        }
    }

    /// The parameters the plugin takes a MIDI controller with (`IMidiMapping`): the first
    /// event bus, every channel, every controller number.
    fn mapped_to_midi(controller: &ComPtr<IEditController>) -> HashSet<u32> {
        let mut mapped = HashSet::new();
        let Some(mapping) = controller.cast::<IMidiMapping>() else {
            return mapped;
        };
        for channel in 0..16 {
            for number in 0..ControllerNumbers_::kCountCtrlNumber as i16 {
                let mut param = 0;
                // SAFETY: a plain question on the main thread, with a place for the answer.
                let told =
                    unsafe { mapping.getMidiControllerAssignment(0, channel, number, &mut param) };
                if told == kResultOk {
                    mapped.insert(param);
                }
            }
        }
        mapped
    }

    fn group_names(controller: &ComPtr<IEditController>) -> HashMap<i32, String> {
        // Each unit's parent and name.
        let mut tree: HashMap<i32, (i32, String)> = HashMap::new();
        if let Some(units) = controller.cast::<IUnitInfo>() {
            // SAFETY: plain questions; `info` is filled by the plugin.
            unsafe {
                for index in 0..units.getUnitCount() {
                    let mut info: UnitInfo = std::mem::zeroed();
                    if units.getUnitInfo(index, &mut info) == kResultOk && info.id != 0 {
                        tree.insert(info.id, (info.parentUnitId, com::read_utf16(&info.name)));
                    }
                }
            }
        }
        // A plugin may hang its whole tree under one unit of its own (JUCE's "Root"): that
        // one is the tree's root then, and names no group.
        let tops: Vec<i32> = (tree.iter())
            .filter(|(_, (parent, _))| !tree.contains_key(parent))
            .map(|(&id, _)| id)
            .collect();
        let root = match tops[..] {
            [only] if tree.values().any(|(parent, _)| *parent == only) => Some(only),
            _ => None,
        };
        // A unit deep in the tree goes by the name of the one at the top above it.
        (tree.keys())
            .filter(|&&id| Some(id) != root)
            .map(|&id| {
                let mut top = id;
                for _ in 0..tree.len() {
                    match tree.get(&top) {
                        Some((parent, _)) if tree.contains_key(parent) && Some(*parent) != root => {
                            top = *parent;
                        }
                        _ => break,
                    }
                }
                (id, tree[&top].1.clone())
            })
            .collect()
    }

    fn settle_later(&mut self, say: bool) {
        self.settle = Some(Settle {
            blocks: self.instance.blocks.load(Relaxed),
            since: Instant::now(),
            say: say || self.settle.as_ref().is_some_and(|settle| settle.say),
        });
    }

    /// Gives the controller the component's state as it is now. Whether a listed parameter
    /// has another value after it.
    fn resync(&mut self) -> bool {
        let Some(controller) = &self.instance.controller else {
            return false;
        };
        let ids: Vec<u32> = (self.units.iter().flatten().map(|(&id, _)| id)).collect();
        let values = || -> Vec<u64> {
            // SAFETY: plain questions on the main thread.
            (ids.iter())
                .map(|&id| unsafe { controller.getParamNormalized(id) }.to_bits())
                .collect()
        };
        let before = values();
        let stream = ComWrapper::new(MemStream::default());
        let Some(at) = stream.as_com_ref::<IBStream>() else {
            return false;
        };
        // SAFETY: as in `save_state` and `load_state`.
        unsafe {
            if self.instance.component.getState(at.as_ptr()) != kResultOk {
                return false;
            }
            stream.rewind();
            controller.setComponentState(at.as_ptr());
        }
        self.forget_values_changed();
        before != values()
    }

    /// A controller says `kParamValuesChanged` from inside `setComponentState` (JUCE):
    /// meloet asked for that itself.
    fn forget_values_changed(&self) {
        if let Ok(mut told) = self.instance.handler.told.lock() {
            told.restart &= !RestartFlags_::kParamValuesChanged;
        }
    }

    /// Makes the output buses of `on` (a bit each) the active ones. A bus is switched on or
    /// off only while the plugin is set up but not active (`activateBus` is "[UI-thread & Setup
    /// Done]" in the SDK's `ivstcomponent.h`), so the plugin is switched off and on again
    /// around it; the audio thread passes the plugin by meanwhile.
    fn switch_buses(&mut self, on: u64) -> Result<(), String> {
        let instance = self.instance.clone();
        let before = instance.buses_on.load(Relaxed);
        let claim = (instance.gate.claim(GATE_WAIT)).ok_or("the plugin is not let go of")?;
        let (audio, output) = (MediaTypes_::kAudio as i32, BusDirections_::kOutput as i32);
        // SAFETY: the audio thread is out of the plugin for as long as the claim is held; the
        // calls leave the processing state and come back to it in the order VST3 gives.
        unsafe {
            instance.processor.setProcessing(0);
            instance.component.setActive(0);
            for &(_, bus) in &self.sets {
                let bit = 1u64 << bus;
                if (on ^ before) & bit != 0 {
                    let state = u8::from(on & bit != 0);
                    instance
                        .component
                        .activateBus(audio, output, bus as i32, state);
                }
            }
            instance.component.setActive(1);
            instance.processor.setProcessing(1);
        }
        instance.buses_on.store(on, Relaxed);
        drop(claim);
        // A plugin that asked to be switched off and on again has just been.
        if let Ok(mut told) = instance.handler.told.lock() {
            told.restart &= !(RestartFlags_::kLatencyChanged | RestartFlags_::kReloadComponent);
        }
        let latency = instance.read_latency();
        if instance.latency.swap(latency, Relaxed) != latency {
            self.later.push(Said::Latency(latency));
        }
        Ok(())
    }

    /// After `kLatencyChanged`: off and on again, as VST3 asks, then the new delay.
    fn reactivate(&mut self) -> Option<u32> {
        let instance = &self.instance;
        let _claim = instance.gate.claim(GATE_WAIT)?;
        // SAFETY: the audio thread is out of the plugin for as long as the claim is held.
        unsafe {
            instance.processor.setProcessing(0);
            instance.component.setActive(0);
            instance.component.setActive(1);
            instance.processor.setProcessing(1);
        }
        Some(instance.read_latency())
    }
}

impl Controller for Vst3Controller {
    fn params(&mut self) -> Vec<ParamInfo> {
        self.hear_audio();
        let Some(controller) = &self.instance.controller else {
            return Vec::new();
        };
        let groups = Self::group_names(controller);
        let mapped = Self::mapped_to_midi(controller);
        let mut units = HashMap::new();
        let mut params = Vec::new();
        // SAFETY: plain questions on the main thread; `info` is filled by the plugin.
        unsafe {
            for index in 0..controller.getParameterCount() {
                let mut info: ParameterInfo = std::mem::zeroed();
                if controller.getParameterInfo(index, &mut info) != kResultOk {
                    continue;
                }
                // Hidden ones are not for people, read-only ones are meters, and passing the
                // plugin by is meloet's own switch.
                let not_shown = ParameterFlags_::kIsHidden
                    | ParameterFlags_::kIsReadOnly
                    | ParameterFlags_::kIsBypass;
                if info.flags & not_shown != 0 {
                    continue;
                }
                let unit = com::read_utf16(&info.units);
                let shape = match info.stepCount {
                    1 => ParamShape::Toggle,
                    steps @ 2..=MOST_STEPS => ParamShape::Stepped {
                        steps: steps as u32 + 1,
                    },
                    _ => ParamShape::Continuous,
                };
                let labels = match shape {
                    ParamShape::Stepped { steps } if steps <= MOST_STEPS as u32 => (0..steps)
                        .map(|step| {
                            let at = f64::from(step) / f64::from(steps - 1);
                            value_text(controller, info.id, at, &unit).unwrap_or_default()
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let value = normal(controller.getParamNormalized(info.id));
                let name = com::read_utf16(&info.title);
                let automatable = info.flags & ParameterFlags_::kCanAutomate != 0;
                // A plugin may map a controller to a parameter of its sound (CC 7 to its
                // volume): that one stays in the list. The ones made only for the knobs are
                // not for automation.
                let midi_cc =
                    (mapped.contains(&info.id) && !automatable) || super::named_midi_cc(&name);
                params.push(ParamInfo {
                    id: info.id,
                    name,
                    group: groups.get(&info.unitId).filter(|n| !n.is_empty()).cloned(),
                    default: normal(info.defaultNormalizedValue),
                    shape,
                    labels,
                    automatable,
                    midi_cc,
                    value: Some(value),
                    text: value_text(controller, info.id, value, &unit),
                });
                units.insert(info.id, unit);
            }
        }
        self.units = Some(units);
        params
    }

    fn param(&mut self, id: u32) -> Option<ParamNow> {
        if self.units.is_none() {
            self.params();
        }
        self.hear_audio();
        let unit = self.units.as_ref()?.get(&id)?;
        let controller = self.instance.controller.as_ref()?;
        // SAFETY: a plain question on the main thread.
        let value = normal(unsafe { controller.getParamNormalized(id) });
        Some(ParamNow {
            id,
            value: Some(value),
            text: value_text(controller, id, value, unit),
        })
    }

    fn text(&mut self, id: u32, value: f64) -> Option<String> {
        if self.units.is_none() {
            self.params();
        }
        let unit = self.units.as_ref()?.get(&id)?;
        value_text(self.instance.controller.as_ref()?, id, value, unit)
    }

    fn read_text(&mut self, id: u32, text: &str) -> Option<Read> {
        if self.units.is_none() {
            self.params();
        }
        let unit = self.units.as_ref()?.get(&id)?;
        let controller = self.instance.controller.as_ref()?;
        let text = text.trim();
        // As typed, and without the unit: plugins differ in what they can read.
        let bare = (!unit.is_empty())
            .then(|| text.strip_suffix(unit.as_str()))
            .flatten()
            .map(str::trim_end);
        let answer = [Some(text), bare].into_iter().flatten().find_map(|text| {
            let mut wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
            let mut value = f64::NAN;
            // SAFETY: a 0-terminated UTF-16 string and a place for the answer.
            let read =
                unsafe { controller.getParamValueByString(id, wide.as_mut_ptr(), &mut value) };
            (read == kResultOk && value.is_finite()).then_some(value)
        });
        let shown = |value| value_text(controller, id, value, unit);
        let (low, high) = (shown(0.0), shown(1.0));
        judge_typed(text, answer, low.as_deref(), high.as_deref(), shown)
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if let Some(controller) = &self.instance.controller {
            // SAFETY: a plain call on the main thread.
            unsafe { controller.setParamNormalized(id, value) };
        }
    }

    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        self.hear_audio();
        let take = |get: &dyn Fn(*mut IBStream) -> i32| -> Result<Vec<u8>, String> {
            let stream = ComWrapper::new(MemStream::default());
            let at = stream.as_com_ref::<IBStream>().ok_or("no stream")?;
            let result = get(at.as_ptr());
            if result != kResultOk && result != kNotImplemented {
                return Err(format!("getState failed ({result})"));
            }
            Ok(stream.take())
        };
        // SAFETY: VST3 has the host take the state on the main thread, also while the audio
        // thread runs the plugin.
        let component = take(&|stream| unsafe { self.instance.component.getState(stream) })?;
        let controller = match &self.instance.controller {
            Some(controller) => take(&|stream| unsafe { controller.getState(stream) })?,
            None => Vec::new(),
        };
        let mut state = Vec::with_capacity(STATE_HEAD + component.len() + controller.len());
        state.extend_from_slice(STATE_MAGIC);
        state.extend_from_slice(&(component.len() as u64).to_le_bytes());
        state.extend_from_slice(&component);
        state.extend_from_slice(&controller);
        Ok(state)
    }

    fn load_state(&mut self, state: &[u8]) -> Result<(), String> {
        let bad = || "not a VST3 state of meloet".to_string();
        let rest = state.strip_prefix(STATE_MAGIC).ok_or_else(bad)?;
        let (len, rest) = rest.split_first_chunk::<8>().ok_or_else(bad)?;
        let len = usize::try_from(u64::from_le_bytes(*len)).map_err(|_| bad())?;
        if len > rest.len() {
            return Err(bad());
        }
        let (component, controller) = rest.split_at(len);
        let stream = ComWrapper::new(MemStream::reading(component));
        let at = stream.as_com_ref::<IBStream>().ok_or("no stream")?;
        // SAFETY: as in `save_state`.
        unsafe {
            let result = self.instance.component.setState(at.as_ptr());
            if result != kResultOk {
                return Err(format!("setState failed ({result})"));
            }
            if let Some(edit) = &self.instance.controller {
                stream.rewind();
                edit.setComponentState(at.as_ptr());
                if !controller.is_empty() {
                    let stream = ComWrapper::new(MemStream::reading(controller));
                    if let Some(at) = stream.as_com_ref::<IBStream>() {
                        edit.setState(at.as_ptr());
                    }
                }
            }
        }
        // What the audio thread reported before belongs to the state that was replaced.
        while self.from_audio.pop().is_ok() {}
        self.forget_values_changed();
        self.settle_later(false);
        Ok(())
    }

    fn open_editor(&mut self, parent: u32) -> Result<Opened, String> {
        self.close_editor();
        let controller = self.instance.controller.as_ref().ok_or("no controller")?;
        // SAFETY: the calls VST3 opens a view with, in its order, on the main thread.
        unsafe {
            let view = ComPtr::from_raw(controller.createView(c"editor".as_ptr()))
                .ok_or("the plugin has no view")?;
            if view.isPlatformTypeSupported(kPlatformTypeX11EmbedWindowID) != kResultTrue {
                return Err("the view does not go into an X11 window".into());
            }
            let frame = ComWrapper::new(Frame::default());
            view.setFrame(raw::<_, IPlugFrame>(&frame));
            let attached = view.attached(
                parent as usize as *mut c_void,
                kPlatformTypeX11EmbedWindowID,
            );
            if attached != kResultOk {
                view.setFrame(std::ptr::null_mut());
                return Err(format!("attached failed ({attached})"));
            }
            let mut rect: ViewRect = std::mem::zeroed();
            let size = (view.getSize(&mut rect) == kResultOk)
                .then(|| (rect.right - rect.left, rect.bottom - rect.top))
                .filter(|size| size.0 > 1 && size.1 > 1)
                .map(|size| (size.0 as u32, size.1 as u32));
            // What the plugin asked for while it was being attached is this size or older.
            let asked = frame.asked.take();
            let resizable = view.canResize() == kResultTrue;
            self.view = Some((view, frame));
            Ok(Opened {
                size: size.or(asked),
                resizable,
            })
        }
    }

    fn close_editor(&mut self) {
        if let Some((view, frame)) = self.view.take() {
            // SAFETY: the way VST3 closes a view, on the main thread.
            unsafe {
                view.removed();
                view.setFrame(std::ptr::null_mut());
            }
            drop(view);
            drop(frame);
        }
    }

    fn resize_editor(&mut self, width: u32, height: u32) -> (u32, u32) {
        let Some((view, _)) = &self.view else {
            return (width, height);
        };
        let mut rect = ViewRect {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        // SAFETY: plain calls on the main thread; the plugin may change `rect`.
        unsafe {
            view.checkSizeConstraint(&mut rect);
            view.onSize(&mut rect);
        }
        let size = (rect.right - rect.left, rect.bottom - rect.top);
        if size.0 > 1 && size.1 > 1 {
            (size.0 as u32, size.1 as u32)
        } else {
            (width, height)
        }
    }

    fn editor_left(&mut self) {
        self.settle_later(false);
    }

    // A VST3 view lives on the run loop (`runloop`), not on an idle call.
    fn idle(&mut self) {}

    fn poll(&mut self, said: &mut Vec<Said>) {
        self.hear_audio();
        // `silent` tells of the gains after the last block, so it is only believed once a
        // block has ended since the gains were last set.
        let ended = self.instance.blocks.load(Acquire) != self.fading_from;
        let faded = self.fading & self.instance.silent.load(Relaxed);
        if ended && faded != 0 {
            let on = self.instance.buses_on.load(Relaxed) & !faded;
            match self.switch_buses(on) {
                Ok(()) => self.fading &= !faded,
                Err(message) => eprintln!("meloet-plughost: output buses: {message}"),
            }
        }
        said.append(&mut self.later);
        if let Some((width, height)) = self.view.as_ref().and_then(|(_, frame)| frame.asked.take())
        {
            said.push(Said::Resized(width, height));
        }
        let (told, restart) = match self.instance.handler.told.lock() {
            Ok(mut told) => (
                std::mem::take(&mut told.said),
                std::mem::take(&mut told.restart),
            ),
            Err(_) => return,
        };
        said.extend(told);
        let again = RestartFlags_::kLatencyChanged | RestartFlags_::kReloadComponent;
        if restart & again != 0
            && let Some(latency) = self.reactivate()
        {
            // A plugin that asks again from inside the restart has just been answered.
            if let Ok(mut told) = self.instance.handler.told.lock() {
                told.restart &= !again;
            }
            if self.instance.latency.swap(latency, Relaxed) != latency {
                said.push(Said::Latency(latency));
            }
        }
        if restart & RestartFlags_::kParamValuesChanged != 0 {
            said.push(Said::Preset);
            said.push(Said::ParamsChanged);
        }
        if restart & RestartFlags_::kParamTitlesChanged != 0 {
            said.push(Said::Preset);
            self.settle_later(true);
        }
        let blocks = self.instance.blocks.load(Relaxed);
        if let Some(settle) = (self.settle).take_if(|settle| {
            blocks.wrapping_sub(settle.blocks) >= SETTLE_BLOCKS
                || settle.since.elapsed() >= SETTLE_WAIT
        }) && (self.resync() || settle.say)
        {
            said.push(Said::ParamsChanged);
        }
    }

    /// A bus whose set is heard now is switched on at once, before its gains move up from 0;
    /// one whose set is not heard any more is switched off by `poll` once the gains have come
    /// down to 0. The first bus is never switched off.
    fn set_outputs(&mut self, settings: &OutputSettings) -> Result<(), String> {
        let active = self.instance.buses_on.load(Relaxed);
        let wanted = (self.sets.iter())
            .filter(|(set, bus)| *bus > 0 && is_on(set, settings))
            .fold(active & 1, |on, (_, bus)| on | 1 << bus);
        self.fading = active & !wanted;
        self.fading_from = self.instance.blocks.load(Acquire);
        if wanted & !active != 0 {
            self.switch_buses(active | wanted)?;
        }
        Ok(())
    }
}

impl Drop for Vst3Controller {
    fn drop(&mut self) {
        self.close_editor();
    }
}

const STATE_MAGIC: &[u8; 4] = b"MV3S";
const STATE_HEAD: usize = 12;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_export_is_told_as_offline() {
        assert_eq!(process_mode(true), ProcessModes_::kOffline as i32);
        assert_eq!(process_mode(false), ProcessModes_::kRealtime as i32);
    }

    fn buses(list: &[(&str, i32)]) -> Vec<(String, i32)> {
        list.iter()
            .map(|&(name, n)| (name.to_string(), n))
            .collect()
    }

    /// The sets as (id, name, ports, on, pan).
    fn shapes(sets: &[SetShape]) -> Vec<(&str, &str, Ports, bool, f32)> {
        (sets.iter())
            .map(|s| (s.id.as_str(), s.name.as_str(), s.ports, s.on, s.pan))
            .collect()
    }

    fn place(channels: i32, first: usize, set: Option<(usize, usize)>) -> BusPlace {
        BusPlace {
            channels,
            first,
            set,
        }
    }

    #[test]
    fn every_output_bus_with_channels_is_an_output_set() {
        let stereo = |left| Ports::Stereo {
            left,
            right: left + 1,
        };
        // As Surge XT has them.
        let (sets, heard, places) =
            bus_sets(&buses(&[("Output", 2), ("Scene A", 2), ("Scene B", 2)]));
        assert_eq!(
            shapes(&sets),
            [
                ("bus0", "Output", stereo(0), true, 0.0),
                ("bus1", "Scene A", stereo(2), false, 0.0),
                ("bus2", "Scene B", stereo(4), false, 0.0),
            ]
        );
        assert_eq!(heard, [(0, Side::Left), (1, Side::Right)]);
        assert_eq!(
            places,
            [
                place(2, 0, Some((0, 0))),
                place(2, 2, Some((1, 2))),
                place(2, 4, Some((2, 4)))
            ]
        );

        // One channel: a set of one, heard on both sides. Past two: the first two, the rest
        // have a place but no port. No name: the id. No channels: no set.
        let (sets, heard, places) = bus_sets(&buses(&[
            ("Mono", 1),
            ("Surround", 6),
            ("", 2),
            ("Nothing", 0),
            ("Last", 1),
        ]));
        assert_eq!(
            shapes(&sets),
            [
                ("bus0", "Mono", Ports::Mono(0), true, 0.0),
                ("bus1", "Surround", stereo(1), false, 0.0),
                ("bus2", "bus2", stereo(3), false, 0.0),
                ("bus4", "Last", Ports::Mono(5), false, 0.0),
            ]
        );
        assert_eq!(heard, [(0, Side::Both)]);
        assert_eq!(
            places,
            [
                place(1, 0, Some((0, 0))),
                place(6, 1, Some((1, 1))),
                place(2, 7, Some((2, 3))),
                place(0, 9, None),
                place(1, 9, Some((3, 5))),
            ]
        );

        // A first bus without channels is heard by nobody; no buses, no sets.
        let (sets, heard, _) = bus_sets(&buses(&[("Empty", 0), ("Aux", 2)]));
        assert_eq!(shapes(&sets), [("bus1", "Aux", stereo(0), false, 0.0)]);
        assert!(heard.is_empty());
        assert_eq!(bus_sets(&[]), (Vec::new(), Vec::new(), Vec::new()));
        // Past the buses a bit each can tell of: no sets.
        let many: Vec<(String, i32)> = (0..70).map(|n| (format!("B{n}"), 2)).collect();
        let (sets, _, places) = bus_sets(&many);
        assert_eq!((sets.len(), places.len()), (MOST_BUSES, 70));
        assert_eq!(places[69], place(2, 138, None));
    }

    /// Surge XT takes being made ready as offline, and sounds in blocks told as offline.
    #[test]
    #[ignore = "needs Surge XT (VST3)"]
    fn surge_xt_is_made_ready_and_run_as_offline_for_an_export() {
        let bundle = Path::new("/usr/lib/vst3/Surge XT.vst3");
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return;
        }
        let key: PluginKey = "vst3:ABCDEF019182FAEB566D624153675854".parse().unwrap();
        for offline in [false, true] {
            let activation = Activation {
                sample_rate: 48_000.0,
                max_frames: 256,
                offline,
            };
            let mut loaded = load(&key, bundle, activation, &OutputSettings::default()).unwrap();
            let silence = [0.0f32; 256];
            let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
            let mut sink = super::super::EventSink::with_capacity(16);
            let note = ToPlugin::Midi {
                frame: 0,
                data: [0x90, 60, 100],
            };
            let mut loudest = 0.0f32;
            for block in 0..40 {
                loaded.processor.process(&mut ProcessBlock {
                    inputs: [&silence, &silence],
                    outputs: [&mut left, &mut right],
                    events: if block == 0 {
                        std::slice::from_ref(&note)
                    } else {
                        &[]
                    },
                    transport: &Transport::default(),
                    sink: &mut sink,
                });
                loudest = left.iter().fold(loudest, |m, x| m.max(x.abs()));
            }
            assert!(loudest > 0.01, "offline {offline}: {loudest}");
        }
    }

    const SURGE: (&str, &str) = (
        "vst3:ABCDEF019182FAEB566D624153675854",
        "/usr/lib/vst3/Surge XT.vst3",
    );

    fn settings(sets: Vec<(&str, f32, Option<f32>, Option<bool>)>) -> OutputSettings {
        OutputSettings {
            rule: 1,
            sets: (sets.into_iter())
                .map(
                    |(id, gain_db, pan, on)| meloet_plugin::wire::OutputSetting {
                        id: id.into(),
                        gain_db,
                        pan,
                        on,
                    },
                )
                .collect(),
        }
    }

    /// Surge XT (VST3) made ready with `outputs`, as the host process does it: the buses at
    /// once, the gains before the first block. `None` when it is not installed.
    fn surge(offline: bool, outputs: &OutputSettings) -> Option<Loaded> {
        let bundle = Path::new(SURGE.1);
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return None;
        }
        let activation = Activation {
            sample_rate: 48_000.0,
            max_frames: 256,
            offline,
        };
        let key: PluginKey = SURGE.0.parse().unwrap();
        let mut loaded = load(&key, bundle, activation, outputs).unwrap();
        loaded.processor.outputs().unwrap().set(outputs, false);
        Some(loaded)
    }

    /// The person's settings while the plugin runs, as the host process takes them: the
    /// controller first, then the gains.
    fn set_outputs(loaded: &mut Loaded, outputs: &OutputSettings) {
        loaded.controller.set_outputs(outputs).unwrap();
        loaded.processor.outputs().unwrap().set(outputs, true);
    }

    /// Plays a fixed tune on Surge XT, `blocks` blocks of 256: every oscillator restarts its
    /// wave with each note, so that two runs give the same samples (else Surge XT starts them
    /// where it likes), then C4 from block 1, G4 from frame 17 of block 20, C4 let go at frame
    /// 100 of block 30. `between(block, loaded)` comes before each block, and the controller's
    /// `poll` after it. Of each block, its left, then its right.
    fn play_surge(
        loaded: &mut Loaded,
        blocks: usize,
        mut between: impl FnMut(usize, &mut Loaded),
    ) -> Vec<f32> {
        let restart: Vec<ToPlugin> = (loaded.controller.params().iter())
            .filter(|p| p.name.contains("Retrigger"))
            .map(|p| ToPlugin::Param {
                frame: 0,
                id: p.id,
                value: 1.0,
            })
            .collect();
        assert_eq!(restart.len(), 6);
        let midi = |frame, data| vec![ToPlugin::Midi { frame, data }];
        let silence = [0.0f32; 256];
        let mut sink = super::super::EventSink::with_capacity(16);
        let mut said = Vec::new();
        let mut pair = Vec::new();
        for block in 0..blocks {
            between(block, loaded);
            let events = match block {
                0 => restart.clone(),
                1 => midi(0, [0x90, 60, 100]),
                20 => midi(17, [0x90, 67, 90]),
                30 => midi(100, [0x80, 60, 0]),
                _ => Vec::new(),
            };
            let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
            loaded.processor.process(&mut ProcessBlock {
                inputs: [&silence, &silence],
                outputs: [&mut left, &mut right],
                events: &events,
                transport: &Transport::default(),
                sink: &mut sink,
            });
            pair.extend(left);
            pair.extend(right);
            loaded.controller.poll(&mut said);
        }
        for latency in said.iter().filter(|s| matches!(s, Said::Latency(_))) {
            eprintln!("Surge XT told another delay: {latency:?}");
        }
        pair
    }

    fn arm(loaded: &mut Loaded, frames: u64) {
        let outputs = loaded.processor.outputs().unwrap();
        let capture = super::super::outputs::Capture::new(outputs.ids(), frames).unwrap();
        outputs.arm(capture);
    }

    /// What the capture kept, by set id; each set left, then right of each block of 256, as
    /// [`play_surge`] gives the pair.
    fn captured(loaded: &mut Loaded) -> Vec<(String, Vec<f32>)> {
        let capture = loaded.processor.outputs().unwrap().take_capture().unwrap();
        (capture.sounds().into_iter())
            .map(|(id, sound)| {
                let blocks = sound.chunks(512).flat_map(|block| {
                    let (left, right): (Vec<f32>, Vec<f32>) =
                        block.chunks(2).map(|p| (p[0], p[1])).unzip();
                    left.into_iter().chain(right)
                });
                (id, blocks.collect())
            })
            .collect()
    }

    fn bits(x: &[f32]) -> Vec<u32> {
        x.iter().map(|x| x.to_bits()).collect()
    }

    fn energy(x: &[f32]) -> f64 {
        x.iter().map(|&x| f64::from(x) * f64::from(x)).sum()
    }

    /// FNV-1a over every bit of the samples.
    fn hash(x: &[f32]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in x.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    #[test]
    #[ignore = "needs Surge XT (VST3)"]
    fn surge_xt_vst3_tells_its_output_buses_as_sets() {
        let Some(loaded) = surge(false, &OutputSettings::default()) else {
            return;
        };
        let set = |id: &str, name: &str, on| OutputSet {
            id: id.into(),
            name: name.into(),
            channels: 2,
            on,
            pan: 0.0,
        };
        assert_eq!(
            loaded.plugin.outputs,
            [
                set("bus0", "Output", true),
                set("bus1", "Scene A", false),
                set("bus2", "Scene B", false),
            ]
        );
        assert_eq!(loaded.plugin.audio_outputs, 2);
    }

    /// The tune of [`play_surge`] with every set at its default, as the code before the output
    /// sets played it with Surge XT 1.3.4 (`hash` of its 61 blocks, playing and in an export).
    const BEFORE_OUTPUT_SETS: u64 = 0x9092_3217_d120_1901;

    #[test]
    #[ignore = "needs Surge XT (VST3)"]
    fn surge_xt_vst3_sounds_as_before_with_every_set_at_its_default() {
        let defaults = OutputSettings::default();
        let Some(mut playing) = surge(false, &defaults) else {
            return;
        };
        let mut export = surge(true, &defaults).unwrap();
        let (a, b) = (
            play_surge(&mut playing, 61, |_, _| {}),
            play_surge(&mut export, 61, |_, _| {}),
        );
        let module = module(Path::new(SURGE.1)).unwrap();
        let version = &find(&module, SURGE.0.strip_prefix("vst3:").unwrap())
            .unwrap()
            .version;
        eprintln!(
            "Surge XT {version}: playing {:016x}, export {:016x}, energy {}",
            hash(&a),
            hash(&b),
            energy(&a)
        );
        assert!(energy(&a) > 1.0);
        assert!(bits(&a) == bits(&b), "playing and export differ");
        if version.starts_with("1.3.4") {
            assert_eq!(hash(&a), BEFORE_OUTPUT_SETS);
        }
    }

    #[test]
    #[ignore = "needs Surge XT (VST3)"]
    fn a_scene_bus_of_surge_xt_vst3_switched_on_and_off_while_notes_play() {
        let Some(mut loaded) = surge(false, &OutputSettings::default()) else {
            return;
        };
        let on = settings(vec![("bus1", 0.0, None, Some(true))]);
        let (mut while_on, mut after) = (Vec::new(), Vec::new());
        let pair = play_surge(&mut loaded, 140, |block, loaded| match block {
            40 => {
                set_outputs(loaded, &on);
                arm(loaded, 256 * 20);
            }
            60 => while_on = captured(loaded),
            80 => set_outputs(loaded, &OutputSettings::default()),
            100 => arm(loaded, 256 * 20),
            120 => after = captured(loaded),
            _ => {}
        });
        assert!(pair.iter().all(|x| x.is_finite()));
        let ids = |sets: &[(String, Vec<f32>)]| -> Vec<String> {
            sets.iter().map(|(id, _)| id.clone()).collect()
        };
        // Scene B was never on; scene A only while it was.
        assert_eq!(ids(&while_on), ["bus0", "bus1"]);
        assert_eq!(ids(&after), ["bus0"]);
        let (main, scene) = (&while_on[0].1, &while_on[1].1);
        // From block 40: the main bus, and scene A coming up from 0 over 960 frames.
        let block = |at: usize| &pair[at * 512..(at + 20) * 512];
        let mut most = 0.0f32;
        for (at, ((&got, &main), &scene)) in block(40).iter().zip(main).zip(scene).enumerate() {
            let frame = (at / 512) * 256 + at % 256;
            let gain = ((frame + 1) as f32 / 960.0).min(1.0);
            most = most.max((got - (main + gain * scene)).abs());
        }
        let step = |x: &[f32], from: usize, to: usize| {
            (from..to).fold(0.0f32, |m, at| m.max((x[at] - x[at - 1]).abs()))
        };
        eprintln!(
            "scene A on: main {:e}, scene A {:e}; the pair against main + ramp × scene A: {most:e}",
            energy(main),
            energy(scene),
        );
        eprintln!(
            "largest step of the left: blocks 35..40 {:e}, at block 40 {:e}, blocks 41..45 {:e}",
            step(&pair, 35 * 512 + 1, 39 * 512 + 256),
            (pair[40 * 512] - pair[39 * 512 + 255]).abs(),
            step(&pair, 41 * 512 + 1, 44 * 512 + 256),
        );
        assert!(energy(scene) > 1.0, "scene A says nothing");
        assert!(most < 1e-5, "{most}");
        // Back to the default: the main bus alone, bit for bit, once scene A is down.
        let main = &after[0].1;
        assert!(bits(block(100)) == bits(main));
        assert!(energy(main) > 1.0);
    }

    #[test]
    #[ignore = "needs Surge XT (VST3)"]
    fn surge_xt_vst3_with_a_scene_on_sounds_the_same_playing_and_in_an_export() {
        let on = settings(vec![("bus1", -6.0, Some(0.5), Some(true))]);
        let mut runs = Vec::new();
        for offline in [false, true] {
            let Some(mut loaded) = surge(offline, &on) else {
                return;
            };
            let pair = play_surge(&mut loaded, 61, |block, loaded| {
                if block == 0 {
                    arm(loaded, 256 * 61);
                }
            });
            let sets = captured(&mut loaded);
            runs.push((pair, sets));
        }
        let (playing, sets) = &runs[0];
        let ids: Vec<&str> = sets.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["bus0", "bus1"]);
        // The pair is the main bus and scene A at −6 dB, its balance to the right.
        let [left_gain, _] = super::super::outputs::pan_gains(0.5);
        let level = 10f32.powf(-6.0 / 20.0);
        let (main, scene) = (&sets[0].1, &sets[1].1);
        let mut most = 0.0f32;
        for (at, ((&got, &main), &scene)) in playing.iter().zip(main).zip(scene).enumerate() {
            let gain = if at % 512 < 256 {
                left_gain * level
            } else {
                level
            };
            most = most.max((got - (main + scene * gain)).abs());
        }
        eprintln!(
            "scene A from the load: main {:e}, scene A {:e}, off by {most:e}",
            energy(main),
            energy(scene)
        );
        assert!(energy(scene) > 1.0 && most < 1e-5, "{most}");
        assert!(
            bits(playing) == bits(&runs[1].0),
            "playing and export differ"
        );
        assert!(runs[0].1 == runs[1].1);
    }

    /// Which VST3 plugins of this PC have programs a host can list (`IUnitInfo` program
    /// lists) and choose (a `kIsProgramChange` parameter: its name, whether it is hidden, and
    /// the texts of its steps). Prints what it finds.
    #[test]
    #[ignore = "reads the VST3 plugins of this PC"]
    fn vst3_programs_of_this_pc() {
        let Ok(bundles) = std::fs::read_dir("/usr/lib/vst3") else {
            return;
        };
        for bundle in bundles.flatten().map(|e| e.path()) {
            let Ok(module) = module(&bundle) else {
                continue;
            };
            for class in &module.classes {
                let Ok(instance) = Instance::create(&module, &class.cid) else {
                    continue;
                };
                let Some(controller) = &instance.controller else {
                    continue;
                };
                let mut lists = Vec::new();
                let mut changers = Vec::new();
                // SAFETY: plain questions on the main thread, into zeroed structs.
                unsafe {
                    if let Some(units) = controller.cast::<IUnitInfo>() {
                        for index in 0..units.getProgramListCount() {
                            let mut info: vst3::Steinberg::Vst::ProgramListInfo =
                                std::mem::zeroed();
                            if units.getProgramListInfo(index, &mut info) != kResultOk {
                                continue;
                            }
                            let mut names = Vec::new();
                            for program in 0..info.programCount.min(8) {
                                let mut name = [0u16; 128];
                                units.getProgramName(info.id, program, &mut name);
                                names.push(com::read_utf16(&name));
                            }
                            lists.push((com::read_utf16(&info.name), info.programCount, names));
                        }
                    }
                    for index in 0..controller.getParameterCount() {
                        let mut info: ParameterInfo = std::mem::zeroed();
                        if controller.getParameterInfo(index, &mut info) == kResultOk
                            && info.flags & ParameterFlags_::kIsProgramChange != 0
                        {
                            let texts: Vec<Option<String>> = (0..=info.stepCount.max(0))
                                .map(|step| {
                                    let at = f64::from(step) / f64::from(info.stepCount.max(1));
                                    value_text(controller, info.id, at, "")
                                })
                                .collect();
                            let hidden = info.flags & ParameterFlags_::kIsHidden != 0;
                            changers.push((com::read_utf16(&info.title), hidden, texts));
                        }
                    }
                }
                println!(
                    "{} ({}): lists {lists:?}; program change {changers:?}",
                    class.name,
                    bundle.display()
                );
            }
        }
    }
}
