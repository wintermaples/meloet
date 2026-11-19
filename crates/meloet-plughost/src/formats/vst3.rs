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
use std::collections::HashMap;
use std::ffi::{CString, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

use meloet_plugin::ipc::{FromPlugin, MAX_LATENCY_FRAMES, ToPlugin, Transport};
use meloet_plugin::wire::{LoadedPlugin, ParamInfo, ParamNow, ParamShape};
use meloet_plugin::{EditorKind, PluginInfo, PluginKey, PluginKind};
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
use super::{
    Activation, Candidate, Controller, GATE_WAIT, Gate, Loaded, Opened, ProcessBlock, Processor,
    Read, Said,
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
            })
        }
    }

    /// Channels of each audio bus in one direction.
    fn buses(&self, direction: i32) -> Vec<i32> {
        // SAFETY: plain questions to the component; `info` is filled by it.
        unsafe {
            let audio = MediaTypes_::kAudio as i32;
            (0..self.component.getBusCount(audio, direction))
                .map(|index| {
                    let mut info: BusInfo = std::mem::zeroed();
                    self.component
                        .getBusInfo(audio, direction, index, &mut info);
                    info.channelCount.max(0)
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
        let channels = instance.buses(direction).first().copied().unwrap_or(0);
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

pub fn load(key: &PluginKey, bundle: &Path, activation: Activation) -> Result<Loaded, String> {
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
        // Only the main buses get sound; every other bus is switched off.
        for (direction, buses) in [(input, &in_buses), (output, &out_buses)] {
            for index in 0..buses.len() as i32 {
                instance
                    .component
                    .activateBus(audio, direction, index, u8::from(index == 0));
            }
        }
        let event_input = instance.component.getBusCount(event, input) > 0;
        if event_input {
            instance.component.activateBus(event, input, 0, 1);
        }
        let mut setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime as i32,
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
        instance.latency.store(instance.read_latency(), Relaxed);
        instance.processor.setProcessing(1);
        event_input
    };

    let frames = activation.max_frames;
    let bus = |channels: &[i32]| -> Box<[AudioBusBuffers]> {
        channels
            .iter()
            .enumerate()
            .map(|(index, &count)| AudioBusBuffers {
                numChannels: if index == 0 { count } else { 0 },
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: std::ptr::null_mut(),
                },
            })
            .collect()
    };
    let main = |channels: &[i32]| channels.first().copied().unwrap_or(0) as usize;
    let (in_channels, out_channels) = (main(&in_buses), main(&out_buses));
    let (to_controller, from_audio) = rtrb::RingBuffer::new(TO_CONTROLLER);
    let instance = Arc::new(instance);
    let plugin = LoadedPlugin {
        latency: instance.latency.load(Relaxed),
        audio_inputs: in_channels as u32,
        audio_outputs: out_channels as u32,
        editor: instance.editor(),
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
        out_ptrs: vec![std::ptr::null_mut(); out_channels].into_boxed_slice(),
        silence: vec![0.0; frames].into_boxed_slice(),
        spare: vec![0.0; frames].into_boxed_slice(),
        mono: vec![0.0; frames].into_boxed_slice(),
        cc,
        to_controller,
        sample_rate: activation.sample_rate,
        frames_run: 0,
    };
    let controller = Vst3Controller {
        view: None,
        instance,
        from_audio,
        units: None,
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
    /// One for every audio bus; only the first has channels.
    in_buses: Box<[AudioBusBuffers]>,
    out_buses: Box<[AudioBusBuffers]>,
    /// The channels of the main buses, pointed anew at every block.
    in_ptrs: Box<[*mut f32]>,
    out_ptrs: Box<[*mut f32]>,
    silence: Box<[f32]>,
    /// Where the outputs past the second go. Never read.
    spare: Box<[f32]>,
    /// The input of a plugin with one input channel.
    mono: Box<[f32]>,
    cc: Box<[u32; CONTROLLERS]>,
    to_controller: rtrb::Producer<(u32, f64)>,
    sample_rate: f64,
    frames_run: i64,
}

// SAFETY: the pointers point into buffers this struct owns or are set anew before each use,
// and the struct is used by one thread at a time (the audio thread, then the main thread that
// drops it).
unsafe impl Send for Vst3Processor {}

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
                let _ = self.to_controller.push((id, value));
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
            return;
        }
        self.events.clear();
        self.changes.clear();
        self.changes_out.clear();
        for event in block.events {
            self.take(event);
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
        let [out_left, out_right] = &mut block.outputs;
        for (channel, ptr) in self.out_ptrs.iter_mut().enumerate() {
            *ptr = match channel {
                0 => out_left.as_mut_ptr(),
                1 => out_right.as_mut_ptr(),
                _ => self.spare.as_mut_ptr(),
            };
        }
        if let Some(bus) = self.in_buses.first_mut() {
            bus.silenceFlags = 0;
            bus.__field0.channelBuffers32 = self.in_ptrs.as_mut_ptr();
        }
        if let Some(bus) = self.out_buses.first_mut() {
            bus.silenceFlags = 0;
            bus.__field0.channelBuffers32 = self.out_ptrs.as_mut_ptr();
        }
        let mut context = context(block.transport, self.sample_rate, self.frames_run);
        self.frames_run = self.frames_run.wrapping_add(frames as i64);
        let mut data = ProcessData {
            processMode: ProcessModes_::kRealtime as i32,
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

        match self.out_ptrs.len() {
            0 => {
                for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
                    output.copy_from_slice(input);
                }
            }
            1 => {
                let [left, right] = &mut block.outputs;
                right.copy_from_slice(left);
            }
            _ => {}
        }
        for (id, value) in self.changes_out.last_values() {
            if value.is_finite() {
                let value = value.clamp(0.0, 1.0);
                let _ = self.to_controller.push((id, value));
                block.sink.push(FromPlugin::Param { id, value });
            }
        }
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
    }

    fn latency(&self) -> u32 {
        self.instance.latency.load(Relaxed)
    }
}

struct Vst3Controller {
    // Before `instance`: the view goes before the controller that made it.
    view: Option<(ComPtr<IPlugView>, ComWrapper<Frame>)>,
    instance: Arc<Instance>,
    from_audio: rtrb::Consumer<(u32, f64)>,
    /// What each parameter is measured in, by id; filled when the parameters are first listed.
    units: Option<HashMap<u32, String>>,
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
                params.push(ParamInfo {
                    id: info.id,
                    name: com::read_utf16(&info.title),
                    group: groups.get(&info.unitId).filter(|n| !n.is_empty()).cloned(),
                    default: normal(info.defaultNormalizedValue),
                    shape,
                    labels,
                    automatable: info.flags & ParameterFlags_::kCanAutomate != 0,
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
        [Some(text), bare].into_iter().flatten().find_map(|text| {
            let mut wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
            let mut value = f64::NAN;
            // SAFETY: a 0-terminated UTF-16 string and a place for the answer.
            let read =
                unsafe { controller.getParamValueByString(id, wide.as_mut_ptr(), &mut value) };
            (read == kResultOk && value.is_finite()).then(|| Read {
                value: value.clamp(0.0, 1.0),
                clamped: (!(-1e-9..=1.0 + 1e-9).contains(&value)).then_some(true),
            })
        })
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

    // A VST3 view lives on the run loop (`runloop`), not on an idle call.
    fn idle(&mut self) {}

    fn poll(&mut self, said: &mut Vec<Said>) {
        self.hear_audio();
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
            said.push(Said::ParamsChanged);
        }
    }
}

impl Drop for Vst3Controller {
    fn drop(&mut self) {
        self.close_editor();
    }
}

const STATE_MAGIC: &[u8; 4] = b"MV3S";
const STATE_HEAD: usize = 12;
