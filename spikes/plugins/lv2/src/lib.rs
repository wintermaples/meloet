//! LV2 behind pk_core::Plugin. DSP through livi (lilv); the UI loader is ours, for X11UI only.
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use livi::event::LV2AtomSequence;
use livi::PortType;
use pk_core::{Format, Kind, MidiEvent, ParamInfo, Plugin, PluginInfo, Result, UiKind};

const X11UI: &str = "http://lv2plug.in/ns/extensions/ui#X11UI";
const UI_CLASSES: [(&str, UiKind); 6] = [
    (X11UI, UiKind::X11Embed),
    ("http://lv2plug.in/ns/extensions/ui#Gtk3UI", UiKind::Gtk3),
    ("http://lv2plug.in/ns/extensions/ui#Qt5UI", UiKind::Qt5),
    ("http://lv2plug.in/ns/extensions/ui#Qt6UI", UiKind::Qt6),
    ("http://kxstudio.sf.net/ns/lv2ext/external-ui#Widget", UiKind::External),
    ("http://lv2plug.in/ns/extensions/ui#external", UiKind::External),
];

fn uri_to_path(uri: &str) -> PathBuf {
    let p = uri.strip_prefix("file://").unwrap_or(uri);
    // lilv percent-encodes spaces ("Surge XT.lv2").
    let mut out = Vec::new();
    let b = p.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&p[i + 1..i + 3], 16) { out.push(v); i += 3; continue; }
        }
        out.push(b[i]);
        i += 1;
    }
    PathBuf::from(String::from_utf8_lossy(&out).into_owned())
}

fn info_of(p: &livi::Plugin) -> PluginInfo {
    let raw = p.raw();
    let uis = raw.uis().map(|u| {
        let mut v = Vec::new();
        for ui in u.iter() {
            for c in ui.classes().iter() {
                if let Some(k) = UI_CLASSES.iter().find(|(s, _)| c.as_uri() == Some(*s)).map(|x| x.1) {
                    if !v.contains(&k) { v.push(k) }
                }
            }
        }
        v
    }).unwrap_or_default();
    let counts = p.port_counts();
    PluginInfo {
        format: Format::Lv2,
        id: p.uri(),
        name: p.name(),
        vendor: raw.author_name().and_then(|n| n.as_str().map(str::to_string)).unwrap_or_default(),
        kind: if p.is_instrument() { Kind::Instrument } else { Kind::Effect },
        path: uri_to_path(raw.bundle_uri().as_uri().unwrap_or("")),
        uis,
        audio_ins: counts.audio_inputs as u32,
        audio_outs: counts.audio_outputs as u32,
    }
}

pub fn scan_all() -> Vec<PluginInfo> {
    livi::World::new().iter_plugins().map(|p| info_of(&p)).collect()
}

struct Control {
    index: livi::PortIndex,
    name: String,
    min: f32,
    max: f32,
    default: f32,
}

pub struct Lv2Plugin {
    info: PluginInfo,
    world: livi::World,
    features: Arc<livi::Features>,
    instance: Option<livi::Instance>,
    controls: Vec<Control>,
    midi_in: Option<LV2AtomSequence>,
    atom_outs: Vec<LV2AtomSequence>,
    ui: Option<UiHost>,
    ui_writes: Rc<RefCell<Vec<(u32, f32)>>>,
}

impl Lv2Plugin {
    pub fn load(uri: &str) -> Result<Lv2Plugin> {
        let world = livi::World::new();
        let plugin = world.plugin_by_uri(uri).ok_or("plugin not found")?;
        let info = info_of(&plugin);
        let features = world.try_build_features(livi::FeaturesBuilder { min_block_length: 1, max_block_length: 4096 })
            .map_err(|e| format!("features: {e:?}"))?;
        let controls = plugin.ports_with_type(PortType::ControlInput).map(|p| Control {
            index: p.index,
            name: p.name.clone(),
            min: p.min_value.unwrap_or(0.0),
            max: p.max_value.unwrap_or(1.0),
            default: p.default_value,
        }).collect();
        Ok(Lv2Plugin { info, world, features, instance: None, controls, midi_in: None, atom_outs: Vec::new(), ui: None, ui_writes: Rc::default() })
    }

    fn norm(c: &Control, v: f32) -> f64 { if c.max > c.min { ((v - c.min) / (c.max - c.min)) as f64 } else { 0.0 } }
}

impl Plugin for Lv2Plugin {
    fn info(&self) -> &PluginInfo { &self.info }

    fn activate(&mut self, sample_rate: f64, max_block: usize) -> Result<()> {
        let plugin = self.world.plugin_by_uri(&self.info.id).ok_or("plugin gone")?;
        let inst = unsafe { plugin.instantiate(self.features.clone(), sample_rate) }.map_err(|e| format!("instantiate: {e:?}"))?;
        let counts = inst.port_counts();
        self.midi_in = (counts.atom_sequence_inputs > 0).then(|| LV2AtomSequence::new(&self.features, 64 * 1024));
        self.atom_outs = (0..counts.atom_sequence_outputs).map(|_| LV2AtomSequence::new(&self.features, 64 * 1024)).collect();
        let _ = max_block;
        self.instance = Some(inst);
        Ok(())
    }

    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], midi: &[MidiEvent]) {
        let Some(inst) = self.instance.as_mut() else { return };
        for (port, v) in self.ui_writes.borrow_mut().drain(..) {
            inst.set_control_input(livi::PortIndex(port as usize), v);
        }
        let n = outputs.first().map_or(0, |o| o.len());
        if let Some(seq) = self.midi_in.as_mut() {
            seq.clear();
            for m in midi {
                let _ = seq.push_midi_event::<3>(m.frame as i64, self.features.midi_urid(), &m.data);
            }
        }
        for o in &mut self.atom_outs { o.clear_as_chunk(); }
        let counts = inst.port_counts();
        let zero = vec![0.0f32; n];
        let ins: Vec<&[f32]> = (0..counts.audio_inputs).map(|i| inputs.get(i).copied().unwrap_or(&zero)).collect();
        let mut spare: Vec<Vec<f32>> = (outputs.len()..counts.audio_outputs).map(|_| vec![0.0; n]).collect();
        let outs: Vec<&mut [f32]> = outputs.iter_mut().map(|s| &mut **s).take(counts.audio_outputs).chain(spare.iter_mut().map(|v| v.as_mut_slice())).collect();
        let ports = livi::EmptyPortConnections::new()
            .with_audio_inputs(ins.into_iter())
            .with_audio_outputs(outs.into_iter())
            .with_atom_sequence_inputs(self.midi_in.iter())
            .with_atom_sequence_outputs(self.atom_outs.iter_mut());
        if let Err(e) = unsafe { inst.run(n, ports) } { eprintln!("lv2 run: {e:?}"); }
        self.features.worker_manager().run_workers();
    }

    fn params(&mut self) -> Vec<ParamInfo> {
        self.controls.iter().map(|c| ParamInfo { id: c.index.0 as u32, name: c.name.clone(), default: Self::norm(c, c.default), automatable: true }).collect()
    }

    fn get_param(&mut self, id: u32) -> f64 {
        let (Some(inst), Some(c)) = (&self.instance, self.controls.iter().find(|c| c.index.0 as u32 == id)) else { return 0.0 };
        Self::norm(c, inst.control_input(c.index).unwrap_or(c.default))
    }

    fn set_param(&mut self, id: u32, value: f64) {
        let Some(c) = self.controls.iter().find(|c| c.index.0 as u32 == id) else { return };
        let v = c.min + (c.max - c.min) * value as f32;
        if let Some(inst) = self.instance.as_mut() { inst.set_control_input(c.index, v); }
        if let Some(ui) = &self.ui { ui.port_event(id, v); }
    }

    fn save_state(&mut self) -> Result<Vec<u8>> {
        let inst = self.instance.as_ref().ok_or("not active")?;
        let st = inst.state().map_err(|e| format!("state: {e:?}"))?;
        Ok(st.to_string("urn:meloet:state").map_err(|e| format!("to_string: {e:?}"))?.into_bytes())
    }

    fn load_state(&mut self, data: &[u8]) -> Result<()> {
        let s = std::str::from_utf8(data).map_err(|e| e.to_string())?;
        let st = self.world.state_from_string(&self.features, s).map_err(|e| format!("parse: {e:?}"))?;
        self.instance.as_mut().ok_or("not active")?.restore_state(&st).map_err(|e| format!("restore: {e:?}"))
    }

    fn open_editor(&mut self, parent: u32) -> Result<(u32, u32)> {
        let inst = self.instance.as_ref().ok_or("activate first")?;
        let plugin = self.world.plugin_by_uri(&self.info.id).ok_or("plugin gone")?;
        let uis = plugin.raw().uis().ok_or("no UI")?;
        let x11 = self.world.raw().new_uri(X11UI);
        let ui = uis.iter().find(|u| u.is_a(&x11)).ok_or("no X11UI")?;
        let handle = inst.raw().instance().handle();
        let host = unsafe {
            UiHost::open(
                &self.info.id,
                ui.uri().as_uri().ok_or("ui uri")?,
                &uri_to_path(ui.bundle_uri().and_then(|n| n.as_uri().map(str::to_string)).as_deref().unwrap_or("")),
                &uri_to_path(ui.binary_uri().and_then(|n| n.as_uri().map(str::to_string)).as_deref().unwrap_or("")),
                parent,
                handle as *mut c_void,
                &self.features,
                self.ui_writes.clone(),
            )?
        };
        for c in &self.controls { host.port_event(c.index.0 as u32, inst.control_input(c.index).unwrap_or(c.default)); }
        let size = host.size.get();
        self.ui = Some(host);
        Ok(size)
    }

    fn close_editor(&mut self) { self.ui = None; }

    fn editor_idle(&mut self) {
        if let Some(ui) = &self.ui { ui.idle(); }
    }
}

impl Drop for Lv2Plugin {
    fn drop(&mut self) {
        self.ui = None; // UI first: with instance-access it points into the DSP instance
        self.instance = None;
    }
}

// ---- LV2 UI ABI (lv2/ui/ui.h). Declared here to keep the unsafe surface in one file. ----

#[repr(C)]
struct Feature { uri: *const c_char, data: *mut c_void }

type WriteFn = unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void);

#[repr(C)]
struct UiDescriptor {
    uri: *const c_char,
    instantiate: unsafe extern "C" fn(*const UiDescriptor, *const c_char, *const c_char, WriteFn, *mut c_void, *mut *mut c_void, *const *const Feature) -> *mut c_void,
    cleanup: unsafe extern "C" fn(*mut c_void),
    port_event: Option<unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void)>,
    extension_data: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
}

#[repr(C)]
struct IdleInterface { idle: unsafe extern "C" fn(*mut c_void) -> i32 }

#[repr(C)]
struct Resize { handle: *mut c_void, ui_resize: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32 }

struct UiCtx { writes: Rc<RefCell<Vec<(u32, f32)>>>, size: std::cell::Cell<(u32, u32)>, other_writes: std::cell::Cell<u32> }

unsafe extern "C" fn write_fn(controller: *mut c_void, port: u32, size: u32, protocol: u32, buf: *const c_void) {
    let ctx = unsafe { &*(controller as *const UiCtx) };
    if protocol == 0 && size == 4 {
        ctx.writes.borrow_mut().push((port, unsafe { *(buf as *const f32) }));
    } else {
        ctx.other_writes.set(ctx.other_writes.get() + 1);
    }
}

unsafe extern "C" fn resize_fn(handle: *mut c_void, w: i32, h: i32) -> i32 {
    let ctx = unsafe { &*(handle as *const UiCtx) };
    ctx.size.set((w as u32, h as u32));
    0
}

struct UiHost {
    desc: *const UiDescriptor,
    handle: *mut c_void,
    idle_if: *const IdleInterface,
    ctx: Box<UiCtx>,
    size: std::cell::Cell<(u32, u32)>,
    _resize: Box<Resize>,
    _lib: libloading::Library,
}

impl UiHost {
    #[allow(clippy::too_many_arguments)]
    unsafe fn open(plugin_uri: &str, ui_uri: &str, bundle: &std::path::Path, binary: &std::path::Path, parent: u32,
        dsp: *mut c_void, features: &livi::Features, writes: Rc<RefCell<Vec<(u32, f32)>>>) -> Result<UiHost> {
        unsafe {
            let lib = libloading::Library::new(binary).map_err(|e| format!("dlopen {}: {e}", binary.display()))?;
            let get = lib.get::<unsafe extern "C" fn(u32) -> *const UiDescriptor>(b"lv2ui_descriptor\0").map_err(|e| e.to_string())?;
            let mut desc = std::ptr::null();
            for i in 0.. {
                let d = get(i);
                if d.is_null() { break }
                if CStr::from_ptr((*d).uri).to_str() == Ok(ui_uri) { desc = d; break }
            }
            if desc.is_null() { return Err("UI descriptor not found".into()) }
            let ctx = Box::new(UiCtx { writes, size: std::cell::Cell::new((0, 0)), other_writes: std::cell::Cell::new(0) });
            let ctx_ptr = &*ctx as *const UiCtx as *mut c_void;
            let mut resize = Box::new(Resize { handle: ctx_ptr, ui_resize: resize_fn });
            let dummy = lv2_raw::LV2Feature { uri: c"urn:meloet:none".as_ptr(), data: std::ptr::null_mut() };
            let host_feats: Vec<&lv2_raw::LV2Feature> = features.iter_features(&dummy).collect();
            let u = |s: &'static CStr| s.as_ptr();
            let mut feats = vec![
                Feature { uri: u(c"http://lv2plug.in/ns/extensions/ui#parent"), data: parent as usize as *mut c_void },
                Feature { uri: u(c"http://lv2plug.in/ns/ext/instance-access"), data: dsp },
                Feature { uri: u(c"http://lv2plug.in/ns/extensions/ui#resize"), data: &mut *resize as *mut Resize as *mut c_void },
                Feature { uri: u(c"http://lv2plug.in/ns/extensions/ui#idleInterface"), data: std::ptr::null_mut() },
            ];
            for f in host_feats.iter().take(3) { feats.push(Feature { uri: f.uri, data: f.data }); } // map, unmap, options
            let ptrs: Vec<*const Feature> = feats.iter().map(|f| f as *const Feature).chain(std::iter::once(std::ptr::null())).collect();
            let plugin_c = CString::new(plugin_uri).unwrap();
            let mut bundle_s = bundle.to_string_lossy().into_owned();
            if !bundle_s.ends_with('/') { bundle_s.push('/') }
            let bundle_c = CString::new(bundle_s).unwrap();
            let mut widget = std::ptr::null_mut();
            let handle = ((*desc).instantiate)(desc, plugin_c.as_ptr(), bundle_c.as_ptr(), write_fn, ctx_ptr, &mut widget, ptrs.as_ptr());
            if handle.is_null() { return Err("UI instantiate returned null".into()) }
            let idle_if = (*desc).extension_data
                .map(|f| f(c"http://lv2plug.in/ns/extensions/ui#idleInterface".as_ptr()) as *const IdleInterface)
                .unwrap_or(std::ptr::null());
            let size = std::cell::Cell::new(ctx.size.get());
            Ok(UiHost { desc, handle, idle_if, ctx, size, _resize: resize, _lib: lib })
        }
    }

    fn port_event(&self, port: u32, v: f32) {
        unsafe {
            if let Some(pe) = (*self.desc).port_event { pe(self.handle, port, 4, 0, &v as *const f32 as *const c_void) }
        }
    }

    fn idle(&self) {
        if !self.idle_if.is_null() { unsafe { ((*self.idle_if).idle)(self.handle) }; }
        self.size.set(self.ctx.size.get());
    }
}

impl Drop for UiHost {
    fn drop(&mut self) {
        unsafe { ((*self.desc).cleanup)(self.handle) }
    }
}
