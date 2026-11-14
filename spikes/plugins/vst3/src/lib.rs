//! VST3 behind pk_core::Plugin. All unsafe code lives in this crate; nothing COM-shaped leaves it.
#![allow(non_snake_case)]
mod com;

use std::cell::RefCell;
use std::ffi::{CStr, c_void};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use com::{EventList, Frame, Handler, HostApp, MemStream, ParamChanges, SharedLoop};
use pk_core::{Format, Kind, MidiEvent, ParamInfo, Plugin, PluginInfo, Result, UiKind};
use vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, BusDirections_, BusInfo, Event, Event__type0, Event_::EventTypes_, IAudioProcessor,
    IComponent, IConnectionPoint, IEditController, MediaTypes_, NoteOffEvent, NoteOnEvent, ParameterInfo, ProcessData,
    ProcessModes_, ProcessSetup, SymbolicSampleSizes_,
};
use vst3::Steinberg::{
    FUnknown, IPlugView, IPluginBase, IPluginFactory, IPluginFactory2, IPluginFactory3, PClassInfo, PClassInfo2, TUID, ViewRect,
    kPlatformTypeX11EmbedWindowID, kResultOk, kResultTrue,
};
use vst3::{ComPtr, ComWrapper, Interface};

macro_rules! call {
    ($p:expr, $m:ident $(, $a:expr)*) => {{ let p = $p.as_ptr(); unsafe { ((*(*p).vtbl).$m)(p $(, $a)*) } }};
}
macro_rules! base {
    ($p:expr, $iface:ident, $m:ident $(, $a:expr)*) => {{
        let b: ComPtr<$iface> = $p.cast::<$iface>().expect(stringify!($iface));
        call!(b, $m $(, $a)*)
    }};
}

/// The shared library. ModuleEntry/ModuleExit bracket its life on Linux.
struct Module {
    lib: libloading::os::unix::Library,
}
impl Module {
    fn open(bundle: &Path) -> Result<Module> {
        let name = bundle.file_stem().ok_or("bad bundle")?.to_string_lossy().to_string();
        let so = bundle.join("Contents").join(format!("{}-linux", std::env::consts::ARCH)).join(format!("{name}.so"));
        let lib = unsafe { libloading::os::unix::Library::open(Some(&so), libc::RTLD_LAZY | libc::RTLD_LOCAL) }
            .map_err(|e| format!("dlopen {}: {e}", so.display()))?;
        unsafe {
            if let Ok(entry) = lib.get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"ModuleEntry\0") {
                // RTLD_NOLOAD hands back the handle of the copy we just opened (and bumps its count).
                let c = std::ffi::CString::new(so.as_os_str().as_encoded_bytes()).unwrap();
                let handle = libc::dlopen(c.as_ptr(), libc::RTLD_LAZY | libc::RTLD_NOLOAD);
                if !entry(handle) { return Err("ModuleEntry returned false".into()) }
            }
        }
        Ok(Module { lib })
    }
    fn factory(&self) -> Result<ComPtr<IPluginFactory>> {
        unsafe {
            let f = self.lib.get::<unsafe extern "system" fn() -> *mut IPluginFactory>(b"GetPluginFactory\0").map_err(|e| e.to_string())?;
            ComPtr::from_raw(f()).ok_or_else(|| "GetPluginFactory returned null".into())
        }
    }
}
impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            if let Ok(exit) = self.lib.get::<unsafe extern "C" fn() -> bool>(b"ModuleExit\0") { exit(); }
        }
    }
}

fn cstr(b: &[std::ffi::c_char]) -> String {
    unsafe { CStr::from_ptr(b.as_ptr()) }.to_string_lossy().into_owned()
}

fn bus_channels(c: &ComPtr<IComponent>, dir: i32) -> Vec<i32> {
    let n = call!(c, getBusCount, MediaTypes_::kAudio as i32, dir);
    (0..n).map(|i| {
        let mut info: BusInfo = unsafe { std::mem::zeroed() };
        call!(c, getBusInfo, MediaTypes_::kAudio as i32, dir, i, &mut info);
        info.channelCount
    }).collect()
}

/// Lists the audio classes in one bundle. Instantiates each component once to count buses.
pub fn scan(bundle: &Path) -> Result<Vec<PluginInfo>> {
    scan_only(bundle, None)
}

fn scan_only(bundle: &Path, only: Option<&str>) -> Result<Vec<PluginInfo>> {
    let module = Module::open(bundle)?;
    let factory = module.factory()?;
    let lp: SharedLoop = Rc::default();
    let host = ComWrapper::new(HostApp { lp });
    let host_unknown = host.to_com_ptr::<FUnknown>().unwrap();
    if let Some(f3) = factory.cast::<IPluginFactory3>() { call!(f3, setHostContext, host_unknown.as_ptr()); }
    let f2 = factory.cast::<IPluginFactory2>();
    let mut out = Vec::new();
    for i in 0..call!(factory, countClasses) {
        let mut ci: PClassInfo = unsafe { std::mem::zeroed() };
        call!(factory, getClassInfo, i, &mut ci);
        if cstr(&ci.category) != "Audio Module Class" { continue; }
        let hex: String = ci.cid.iter().map(|b| format!("{:02X}", *b as u8)).collect();
        if only.is_some_and(|o| o != hex) { continue; }
        let (mut sub, mut vendor) = (String::new(), String::new());
        if let Some(f2) = &f2 {
            let mut c2: PClassInfo2 = unsafe { std::mem::zeroed() };
            if call!(f2, getClassInfo2, i, &mut c2) == kResultOk { sub = cstr(&c2.subCategories); vendor = cstr(&c2.vendor); }
        }
        let comp = create::<IComponent>(&factory, &ci.cid)?;
        base!(comp, IPluginBase, initialize, host_unknown.as_ptr());
        let ins = bus_channels(&comp, BusDirections_::kInput as i32);
        let outs = bus_channels(&comp, BusDirections_::kOutput as i32);
        base!(comp, IPluginBase, terminate);
        out.push(PluginInfo {
            format: Format::Vst3,
            id: hex,
            name: cstr(&ci.name),
            vendor,
            kind: if sub.contains("Instrument") { Kind::Instrument } else { Kind::Effect },
            path: bundle.to_path_buf(),
            uis: vec![UiKind::X11Embed],
            audio_ins: ins.first().copied().unwrap_or(0) as u32,
            audio_outs: outs.first().copied().unwrap_or(0) as u32,
        });
    }
    Ok(out)
}

fn create<I: Interface>(factory: &ComPtr<IPluginFactory>, cid: &TUID) -> Result<ComPtr<I>> {
    let mut obj = std::ptr::null_mut();
    let r = call!(factory, createInstance, cid.as_ptr(), I::IID.as_ptr() as *const _, &mut obj);
    if r != kResultOk || obj.is_null() { return Err(format!("createInstance failed ({r})")); }
    unsafe { ComPtr::from_raw(obj as *mut I) }.ok_or_else(|| "null instance".into())
}

pub struct Vst3Plugin {
    info: PluginInfo,
    component: ComPtr<IComponent>,
    processor: Option<ComPtr<IAudioProcessor>>,
    controller: ComPtr<IEditController>,
    separate_controller: bool,
    view: Option<ComPtr<IPlugView>>,
    lp: SharedLoop,
    edits: Rc<RefCell<Vec<(u32, f64)>>>,
    host: ComWrapper<HostApp>,
    frame: ComWrapper<Frame>,
    _handler: ComWrapper<Handler>,
    out_buses: Vec<i32>,
    in_buses: Vec<i32>,
    zero: Vec<f32>,
    scratch: Vec<Vec<f32>>,
    _module: Module,
}

impl Vst3Plugin {
    pub fn load(bundle: &Path, class_id_hex: &str) -> Result<Vst3Plugin> {
        let info = scan_only(bundle, Some(class_id_hex))?.into_iter().next().ok_or("class not found")?;
        let module = Module::open(bundle)?;
        let factory = module.factory()?;
        let lp: SharedLoop = Rc::default();
        let host = ComWrapper::new(HostApp { lp: lp.clone() });
        let host_unknown = host.to_com_ptr::<FUnknown>().unwrap();
        if let Some(f3) = factory.cast::<IPluginFactory3>() { call!(f3, setHostContext, host_unknown.as_ptr()); }
        let mut cid: TUID = [0; 16];
        for (i, b) in cid.iter_mut().enumerate() { *b = u8::from_str_radix(&class_id_hex[i * 2..i * 2 + 2], 16).unwrap() as _; }
        let component = create::<IComponent>(&factory, &cid)?;
        if base!(component, IPluginBase, initialize, host_unknown.as_ptr()) != kResultOk { return Err("component initialize".into()); }
        let (controller, separate) = match component.cast::<IEditController>() {
            Some(c) => (c, false),
            None => {
                let mut ccid: TUID = [0; 16];
                if call!(component, getControllerClassId, &mut ccid) != kResultTrue && call!(component, getControllerClassId, &mut ccid) != kResultOk {
                    return Err("no controller".into());
                }
                let c = create::<IEditController>(&factory, &ccid)?;
                base!(c, IPluginBase, initialize, host_unknown.as_ptr());
                (c, true)
            }
        };
        if separate {
            if let (Some(a), Some(b)) = (component.cast::<IConnectionPoint>(), controller.cast::<IConnectionPoint>()) {
                call!(a, connect, b.as_ptr());
                call!(b, connect, a.as_ptr());
            }
        }
        let edits: Rc<RefCell<Vec<(u32, f64)>>> = Rc::default();
        let handler = ComWrapper::new(Handler(edits.clone()));
        call!(controller, setComponentHandler, handler.to_com_ptr::<vst3::Steinberg::Vst::IComponentHandler>().unwrap().as_ptr());
        // Controller starts from the component's state.
        let s = ComWrapper::new(MemStream::default());
        let sp = s.to_com_ptr::<vst3::Steinberg::IBStream>().unwrap();
        if call!(component, getState, sp.as_ptr()) == kResultOk {
            s.0.borrow_mut().1 = 0;
            call!(controller, setComponentState, sp.as_ptr());
        }
        let in_buses = bus_channels(&component, BusDirections_::kInput as i32);
        let out_buses = bus_channels(&component, BusDirections_::kOutput as i32);
        let frame = ComWrapper::new(Frame { lp: lp.clone() });
        Ok(Vst3Plugin {
            info, component, processor: None, controller, separate_controller: separate, view: None, lp, edits, host, frame,
            _handler: handler, out_buses, in_buses, zero: Vec::new(), scratch: Vec::new(), _module: module,
        })
    }

    pub fn run_loop_once(&self, wait: Duration) {
        com::pump(&self.lp, wait);
    }

    pub fn take_resize(&self) -> Option<(i32, i32)> {
        self.lp.borrow_mut().resize.take()
    }
}

impl Plugin for Vst3Plugin {
    fn info(&self) -> &PluginInfo { &self.info }

    fn activate(&mut self, sample_rate: f64, max_block: usize) -> Result<()> {
        let p = self.component.cast::<IAudioProcessor>().ok_or("no IAudioProcessor")?;
        // Only the main buses get buffers, so every other bus is switched off explicitly.
        for (dir, buses) in [(BusDirections_::kInput as i32, &self.in_buses), (BusDirections_::kOutput as i32, &self.out_buses)] {
            for i in 0..buses.len() as i32 { call!(self.component, activateBus, MediaTypes_::kAudio as i32, dir, i, (i == 0) as u8); }
        }
        if call!(self.component, getBusCount, MediaTypes_::kEvent as i32, BusDirections_::kInput as i32) > 0 {
            call!(self.component, activateBus, MediaTypes_::kEvent as i32, BusDirections_::kInput as i32, 0, 1);
        }
        let mut setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime as i32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            maxSamplesPerBlock: max_block as i32,
            sampleRate: sample_rate,
        };
        if call!(p, setupProcessing, &mut setup) != kResultOk { return Err("setupProcessing".into()); }
        call!(self.component, setActive, 1);
        call!(p, setProcessing, 1);
        self.zero = vec![0.0; max_block];
        self.scratch = vec![vec![0.0; max_block]; 2];
        self.processor = Some(p);
        Ok(())
    }

    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], midi: &[MidiEvent]) {
        let Some(p) = &self.processor else { return };
        let n = outputs.first().map_or(0, |o| o.len());
        let events = ComWrapper::new(EventList::default());
        for m in midi {
            let on = m.data[0] & 0xF0 == 0x90 && m.data[2] > 0;
            let off = m.data[0] & 0xF0 == 0x80 || (m.data[0] & 0xF0 == 0x90 && m.data[2] == 0);
            if !on && !off { continue; }
            let ch = (m.data[0] & 0x0F) as i16;
            let e = Event {
                busIndex: 0, sampleOffset: m.frame as i32, ppqPosition: 0.0, flags: 0,
                r#type: if on { EventTypes_::kNoteOnEvent } else { EventTypes_::kNoteOffEvent } as u16,
                __field0: if on {
                    Event__type0 { noteOn: NoteOnEvent { channel: ch, pitch: m.data[1] as i16, tuning: 0.0, velocity: m.data[2] as f32 / 127.0, length: 0, noteId: -1 } }
                } else {
                    Event__type0 { noteOff: NoteOffEvent { channel: ch, pitch: m.data[1] as i16, velocity: 0.0, noteId: -1, tuning: 0.0 } }
                },
            };
            events.0.borrow_mut().push(e);
        }
        let changes = ComWrapper::new(ParamChanges::default());
        for (id, v) in self.edits.borrow_mut().drain(..) {
            let mut idx = 0;
            let q = unsafe { vst3::Steinberg::Vst::IParameterChangesTrait::addParameterData(&*changes, &id, &mut idx) };
            unsafe { ((*(*q).vtbl).addPoint)(q, 0, v, &mut idx) };
        }
        let in_ch = self.in_buses.first().copied().unwrap_or(0) as usize;
        let out_ch = self.out_buses.first().copied().unwrap_or(0) as usize;
        let mut in_ptrs: Vec<*mut f32> = (0..in_ch).map(|c| inputs.get(c).map_or(self.zero.as_ptr(), |s| s.as_ptr()) as *mut f32).collect();
        let mut out_ptrs: Vec<*mut f32> = (0..out_ch)
            .map(|c| match outputs.get_mut(c) { Some(s) => s.as_mut_ptr(), None => self.scratch[c.min(1)].as_mut_ptr() })
            .collect();
        let mut ins = AudioBusBuffers { numChannels: in_ch as i32, silenceFlags: 0, __field0: AudioBusBuffers__type0 { channelBuffers32: in_ptrs.as_mut_ptr() } };
        let mut outs = AudioBusBuffers { numChannels: out_ch as i32, silenceFlags: 0, __field0: AudioBusBuffers__type0 { channelBuffers32: out_ptrs.as_mut_ptr() } };
        let mut data = ProcessData {
            processMode: ProcessModes_::kRealtime as i32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            numSamples: n as i32,
            numInputs: (in_ch > 0) as i32,
            numOutputs: (out_ch > 0) as i32,
            inputs: if in_ch > 0 { &mut ins } else { std::ptr::null_mut() },
            outputs: if out_ch > 0 { &mut outs } else { std::ptr::null_mut() },
            inputParameterChanges: changes.to_com_ptr::<vst3::Steinberg::Vst::IParameterChanges>().unwrap().as_ptr(),
            outputParameterChanges: std::ptr::null_mut(),
            inputEvents: events.to_com_ptr::<vst3::Steinberg::Vst::IEventList>().unwrap().as_ptr(),
            outputEvents: std::ptr::null_mut(),
            processContext: std::ptr::null_mut(),
        };
        call!(p, process, &mut data);
    }

    fn params(&mut self) -> Vec<ParamInfo> {
        (0..call!(self.controller, getParameterCount)).filter_map(|i| {
            let mut pi: ParameterInfo = unsafe { std::mem::zeroed() };
            (call!(self.controller, getParameterInfo, i, &mut pi) == kResultOk).then(|| ParamInfo {
                id: pi.id,
                name: com::read_str128(&pi.title),
                default: pi.defaultNormalizedValue,
                automatable: pi.flags & 1 != 0,
            })
        }).collect()
    }

    fn get_param(&mut self, id: u32) -> f64 { call!(self.controller, getParamNormalized, id) }

    fn set_param(&mut self, id: u32, value: f64) {
        call!(self.controller, setParamNormalized, id, value);
        self.edits.borrow_mut().push((id, value));
    }

    fn save_state(&mut self) -> Result<Vec<u8>> {
        let a = ComWrapper::new(MemStream::default());
        let b = ComWrapper::new(MemStream::default());
        call!(self.component, getState, a.to_com_ptr::<vst3::Steinberg::IBStream>().unwrap().as_ptr());
        call!(self.controller, getState, b.to_com_ptr::<vst3::Steinberg::IBStream>().unwrap().as_ptr());
        let (a, b) = (a.0.borrow().0.clone(), b.0.borrow().0.clone());
        let mut out = (a.len() as u64).to_le_bytes().to_vec();
        out.extend(a);
        out.extend(b);
        Ok(out)
    }

    fn load_state(&mut self, data: &[u8]) -> Result<()> {
        let n = u64::from_le_bytes(data.get(..8).ok_or("short state")?.try_into().unwrap()) as usize;
        let (a, b) = data[8..].split_at(n);
        let sa = ComWrapper::new(MemStream(RefCell::new((a.to_vec(), 0))));
        let pa = sa.to_com_ptr::<vst3::Steinberg::IBStream>().unwrap();
        if call!(self.component, setState, pa.as_ptr()) != kResultOk { return Err("component setState".into()); }
        sa.0.borrow_mut().1 = 0;
        call!(self.controller, setComponentState, pa.as_ptr());
        let sb = ComWrapper::new(MemStream(RefCell::new((b.to_vec(), 0))));
        call!(self.controller, setState, sb.to_com_ptr::<vst3::Steinberg::IBStream>().unwrap().as_ptr());
        Ok(())
    }

    fn open_editor(&mut self, parent: u32) -> Result<(u32, u32)> {
        let raw = call!(self.controller, createView, c"editor".as_ptr());
        let view = unsafe { ComPtr::from_raw(raw) }.ok_or("createView returned null")?;
        if call!(view, isPlatformTypeSupported, kPlatformTypeX11EmbedWindowID) != kResultTrue {
            return Err("X11EmbedWindowID not supported".into());
        }
        call!(view, setFrame, self.frame.to_com_ptr::<vst3::Steinberg::IPlugFrame>().unwrap().as_ptr());
        if call!(view, attached, parent as usize as *mut c_void, kPlatformTypeX11EmbedWindowID) != kResultOk {
            return Err("attached failed".into());
        }
        let mut r: ViewRect = unsafe { std::mem::zeroed() };
        call!(view, getSize, &mut r);
        self.view = Some(view);
        Ok(((r.right - r.left) as u32, (r.bottom - r.top) as u32))
    }

    fn close_editor(&mut self) {
        if let Some(v) = self.view.take() {
            call!(v, removed);
            call!(v, setFrame, std::ptr::null_mut());
        }
    }

    fn editor_idle(&mut self) { com::pump(&self.lp, Duration::ZERO); }
}

impl Drop for Vst3Plugin {
    fn drop(&mut self) {
        self.close_editor();
        if let Some(p) = self.processor.take() {
            call!(p, setProcessing, 0);
            call!(self.component, setActive, 0);
        }
        if self.separate_controller {
            if let (Some(a), Some(b)) = (self.component.cast::<IConnectionPoint>(), self.controller.cast::<IConnectionPoint>()) {
                call!(a, disconnect, b.as_ptr());
                call!(b, disconnect, a.as_ptr());
            }
            base!(self.controller, IPluginBase, terminate);
        }
        base!(self.component, IPluginBase, terminate);
        let _ = &self.host;
    }
}

pub fn default_paths() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/usr/lib/vst3"), PathBuf::from("/usr/local/lib/vst3")];
    if let Some(h) = std::env::var_os("HOME") { v.push(PathBuf::from(h).join(".vst3")); }
    v
}
