//! COM objects the host has to provide to a VST3 plugin. Single-threaded on purpose (RefCell):
//! every call into these comes from the thread that owns the plugin.
#![allow(non_snake_case)]
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void};
use std::rc::Rc;
use std::time::{Duration, Instant};

use vst3::Steinberg::Linux::{IEventHandler, IRunLoop, IRunLoopTrait, ITimerHandler, FileDescriptor, TimerInterval};
use vst3::Steinberg::Vst::{
    Event, IAttributeList, IAttributeListTrait, IComponentHandler, IComponentHandlerTrait, IEventList, IEventListTrait,
    IHostApplication, IHostApplicationTrait, IMessage, IMessageTrait, IParamValueQueue, IParamValueQueueTrait,
    IParameterChanges, IParameterChangesTrait, ParamID, ParamValue, String128, TChar,
};
use vst3::Steinberg::{
    FIDString, IBStream, IBStreamTrait, IPlugFrame, IPlugFrameTrait, IPlugView, TUID, ViewRect, int32, int64, kInvalidArgument,
    kNoInterface, kResultFalse, kResultOk, tresult, uint32,
};
use vst3::{Class, ComPtr, ComRef, ComWrapper, Interface};

pub fn write_str128(dst: &mut String128, s: &str) {
    for (i, c) in s.encode_utf16().take(127).chain(std::iter::once(0)).enumerate() {
        dst[i] = c as TChar;
    }
}

pub fn read_str128(s: &[TChar]) -> String {
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..end].iter().map(|&c| c as u16).collect::<Vec<_>>())
}

fn same_iid(a: &TUID, b: &[u8; 16]) -> bool {
    a.iter().zip(b).all(|(x, y)| *x as u8 == *y)
}

// ---- run loop shared by the host context and the plug frame ----

#[derive(Default)]
pub struct LoopState {
    fds: Vec<(FileDescriptor, ComPtr<IEventHandler>)>,
    timers: Vec<(Duration, Instant, ComPtr<ITimerHandler>)>,
    pub resize: Option<(i32, i32)>,
}

pub type SharedLoop = Rc<RefCell<LoopState>>;

/// Runs everything that is due. Handlers are cloned out first: plugins re-register from inside callbacks.
pub fn pump(state: &SharedLoop, wait: Duration) {
    let (fds, timers) = {
        let s = state.borrow();
        (s.fds.clone(), s.timers.iter().map(|t| (t.1, t.2.clone())).collect::<Vec<_>>())
    };
    let now = Instant::now();
    let next_timer = timers.iter().map(|t| t.0.saturating_duration_since(now)).min().unwrap_or(wait).min(wait);
    let mut pfds: Vec<libc::pollfd> = fds.iter().map(|(fd, _)| libc::pollfd { fd: *fd, events: libc::POLLIN, revents: 0 }).collect();
    unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as _, next_timer.as_millis() as i32) };
    for (p, (fd, h)) in pfds.iter().zip(&fds) {
        if p.revents & libc::POLLIN != 0 {
            unsafe { ((*(*h.as_ptr()).vtbl).onFDIsSet)(h.as_ptr(), *fd) };
        }
    }
    let now = Instant::now();
    for (due, h) in timers {
        if due <= now {
            if let Some(t) = state.borrow_mut().timers.iter_mut().find(|t| t.2.as_ptr() == h.as_ptr()) {
                t.1 = now + t.0;
            }
            unsafe { ((*(*h.as_ptr()).vtbl).onTimer)(h.as_ptr()) };
        }
    }
}

macro_rules! impl_runloop {
    ($t:ty) => {
        impl IRunLoopTrait for $t {
            unsafe fn registerEventHandler(&self, handler: *mut IEventHandler, fd: FileDescriptor) -> tresult {
                let Some(h) = (unsafe { ComRef::from_raw(handler) }) else { return kInvalidArgument };
                self.lp.borrow_mut().fds.push((fd, h.to_com_ptr()));
                kResultOk
            }
            unsafe fn unregisterEventHandler(&self, handler: *mut IEventHandler) -> tresult {
                self.lp.borrow_mut().fds.retain(|(_, h)| h.as_ptr() != handler);
                kResultOk
            }
            unsafe fn registerTimer(&self, handler: *mut ITimerHandler, ms: TimerInterval) -> tresult {
                let Some(h) = (unsafe { ComRef::from_raw(handler) }) else { return kInvalidArgument };
                let d = Duration::from_millis(ms.max(1));
                self.lp.borrow_mut().timers.push((d, Instant::now() + d, h.to_com_ptr()));
                kResultOk
            }
            unsafe fn unregisterTimer(&self, handler: *mut ITimerHandler) -> tresult {
                self.lp.borrow_mut().timers.retain(|t| t.2.as_ptr() != handler);
                kResultOk
            }
        }
    };
}

// ---- host application (also handed to IPluginFactory3::setHostContext) ----

pub struct HostApp {
    pub lp: SharedLoop,
}
impl Class for HostApp {
    type Interfaces = (IHostApplication, IRunLoop);
}
impl IHostApplicationTrait for HostApp {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        unsafe { write_str128(&mut *name, "meloet spike") };
        kResultOk
    }
    unsafe fn createInstance(&self, cid: *mut TUID, iid: *mut TUID, obj: *mut *mut c_void) -> tresult {
        let (cid, iid) = unsafe { (&*cid, &*iid) };
        if same_iid(cid, &IMessage::IID) && same_iid(iid, &IMessage::IID) {
            let m = ComWrapper::new(Message::default());
            unsafe { *obj = m.to_com_ptr::<IMessage>().unwrap().into_raw().cast() };
            return kResultOk;
        }
        if same_iid(cid, &IAttributeList::IID) && same_iid(iid, &IAttributeList::IID) {
            let a = ComWrapper::new(Attributes::default());
            unsafe { *obj = a.to_com_ptr::<IAttributeList>().unwrap().into_raw().cast() };
            return kResultOk;
        }
        unsafe { *obj = std::ptr::null_mut() };
        kNoInterface
    }
}
impl_runloop!(HostApp);

// ---- plug frame: resize requests + the run loop the GUI uses ----

pub struct Frame {
    pub lp: SharedLoop,
}
impl Class for Frame {
    type Interfaces = (IPlugFrame, IRunLoop);
}
impl IPlugFrameTrait for Frame {
    unsafe fn resizeView(&self, view: *mut IPlugView, new_size: *mut ViewRect) -> tresult {
        let r = unsafe { &mut *new_size };
        self.lp.borrow_mut().resize = Some((r.right - r.left, r.bottom - r.top));
        unsafe { ((*(*view).vtbl).onSize)(view, new_size) };
        kResultOk
    }
}
impl_runloop!(Frame);

// ---- messages between component and controller ----

#[derive(Clone)]
enum Attr {
    Int(i64),
    Float(f64),
    Str(Vec<TChar>),
    Bin(Vec<u8>),
}

#[derive(Default)]
pub struct Attributes(RefCell<HashMap<CString, Attr>>);
impl Class for Attributes {
    type Interfaces = (IAttributeList,);
}
unsafe fn key(id: *const std::ffi::c_char) -> CString {
    unsafe { CStr::from_ptr(id).to_owned() }
}
impl IAttributeListTrait for Attributes {
    unsafe fn setInt(&self, id: *const std::ffi::c_char, v: int64) -> tresult {
        self.0.borrow_mut().insert(unsafe { key(id) }, Attr::Int(v));
        kResultOk
    }
    unsafe fn getInt(&self, id: *const std::ffi::c_char, v: *mut int64) -> tresult {
        match self.0.borrow().get(&unsafe { key(id) }) {
            Some(Attr::Int(x)) => { unsafe { *v = *x }; kResultOk }
            _ => kResultFalse,
        }
    }
    unsafe fn setFloat(&self, id: *const std::ffi::c_char, v: f64) -> tresult {
        self.0.borrow_mut().insert(unsafe { key(id) }, Attr::Float(v));
        kResultOk
    }
    unsafe fn getFloat(&self, id: *const std::ffi::c_char, v: *mut f64) -> tresult {
        match self.0.borrow().get(&unsafe { key(id) }) {
            Some(Attr::Float(x)) => { unsafe { *v = *x }; kResultOk }
            _ => kResultFalse,
        }
    }
    unsafe fn setString(&self, id: *const std::ffi::c_char, s: *const TChar) -> tresult {
        let mut v = Vec::new();
        let mut p = s;
        unsafe { while *p != 0 { v.push(*p); p = p.add(1); } }
        v.push(0);
        self.0.borrow_mut().insert(unsafe { key(id) }, Attr::Str(v));
        kResultOk
    }
    unsafe fn getString(&self, id: *const std::ffi::c_char, s: *mut TChar, size_bytes: uint32) -> tresult {
        match self.0.borrow().get(&unsafe { key(id) }) {
            Some(Attr::Str(v)) => {
                let n = v.len().min(size_bytes as usize / 2);
                unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), s, n) };
                kResultOk
            }
            _ => kResultFalse,
        }
    }
    unsafe fn setBinary(&self, id: *const std::ffi::c_char, data: *const c_void, size: uint32) -> tresult {
        let b = unsafe { std::slice::from_raw_parts(data as *const u8, size as usize) }.to_vec();
        self.0.borrow_mut().insert(unsafe { key(id) }, Attr::Bin(b));
        kResultOk
    }
    unsafe fn getBinary(&self, id: *const std::ffi::c_char, data: *mut *const c_void, size: *mut uint32) -> tresult {
        // The pointer must stay valid after the borrow ends: the Vec lives in the map until it is replaced.
        match self.0.borrow().get(&unsafe { key(id) }) {
            Some(Attr::Bin(b)) => { unsafe { *data = b.as_ptr().cast(); *size = b.len() as u32 }; kResultOk }
            _ => kResultFalse,
        }
    }
}

pub struct Message {
    id: RefCell<Option<CString>>,
    attrs: ComWrapper<Attributes>,
}
impl Default for Message {
    fn default() -> Self {
        Message { id: RefCell::new(None), attrs: ComWrapper::new(Attributes::default()) }
    }
}
impl Class for Message {
    type Interfaces = (IMessage,);
}
impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> FIDString {
        self.id.borrow().as_ref().map_or(std::ptr::null(), |c| c.as_ptr())
    }
    unsafe fn setMessageID(&self, id: FIDString) {
        *self.id.borrow_mut() = if id.is_null() { None } else { Some(unsafe { CStr::from_ptr(id) }.to_owned()) };
    }
    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attrs.as_com_ref::<IAttributeList>().unwrap().as_ptr()
    }
}

// ---- byte stream for state ----

#[derive(Default)]
pub struct MemStream(pub RefCell<(Vec<u8>, usize)>);
impl Class for MemStream {
    type Interfaces = (IBStream,);
}
impl IBStreamTrait for MemStream {
    unsafe fn read(&self, buf: *mut c_void, n: int32, read: *mut int32) -> tresult {
        let mut s = self.0.borrow_mut();
        let (data, pos) = &mut *s;
        let k = (n.max(0) as usize).min(data.len().saturating_sub(*pos));
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr().add(*pos), buf as *mut u8, k) };
        *pos += k;
        if !read.is_null() { unsafe { *read = k as i32 } }
        kResultOk
    }
    unsafe fn write(&self, buf: *mut c_void, n: int32, written: *mut int32) -> tresult {
        let mut s = self.0.borrow_mut();
        let (data, pos) = &mut *s;
        let src = unsafe { std::slice::from_raw_parts(buf as *const u8, n.max(0) as usize) };
        if *pos + src.len() > data.len() { data.resize(*pos + src.len(), 0) }
        data[*pos..*pos + src.len()].copy_from_slice(src);
        *pos += src.len();
        if !written.is_null() { unsafe { *written = n } }
        kResultOk
    }
    unsafe fn seek(&self, p: int64, mode: int32, result: *mut int64) -> tresult {
        let mut s = self.0.borrow_mut();
        let base = match mode { 0 => 0, 1 => s.1 as i64, 2 => s.0.len() as i64, _ => return kInvalidArgument };
        s.1 = (base + p).clamp(0, s.0.len() as i64) as usize;
        if !result.is_null() { unsafe { *result = s.1 as i64 } }
        kResultOk
    }
    unsafe fn tell(&self, p: *mut int64) -> tresult {
        if !p.is_null() { unsafe { *p = self.0.borrow().1 as i64 } }
        kResultOk
    }
}

// ---- process() inputs ----

#[derive(Default)]
pub struct EventList(pub RefCell<Vec<Event>>);
impl Class for EventList {
    type Interfaces = (IEventList,);
}
impl IEventListTrait for EventList {
    unsafe fn getEventCount(&self) -> int32 {
        self.0.borrow().len() as i32
    }
    unsafe fn getEvent(&self, i: int32, e: *mut Event) -> tresult {
        match self.0.borrow().get(i as usize) { Some(x) => { unsafe { *e = *x }; kResultOk } None => kInvalidArgument }
    }
    unsafe fn addEvent(&self, e: *mut Event) -> tresult {
        self.0.borrow_mut().push(unsafe { *e });
        kResultOk
    }
}

pub struct ParamQueue {
    pub id: ParamID,
    pub points: RefCell<Vec<(i32, ParamValue)>>,
}
impl Class for ParamQueue {
    type Interfaces = (IParamValueQueue,);
}
impl IParamValueQueueTrait for ParamQueue {
    unsafe fn getParameterId(&self) -> ParamID { self.id }
    unsafe fn getPointCount(&self) -> int32 { self.points.borrow().len() as i32 }
    unsafe fn getPoint(&self, i: int32, off: *mut int32, v: *mut ParamValue) -> tresult {
        match self.points.borrow().get(i as usize) {
            Some(&(o, x)) => { unsafe { *off = o; *v = x }; kResultOk }
            None => kInvalidArgument,
        }
    }
    unsafe fn addPoint(&self, off: int32, v: ParamValue, index: *mut int32) -> tresult {
        let mut p = self.points.borrow_mut();
        p.push((off, v));
        if !index.is_null() { unsafe { *index = p.len() as i32 - 1 } }
        kResultOk
    }
}

#[derive(Default)]
pub struct ParamChanges(pub RefCell<Vec<ComWrapper<ParamQueue>>>);
impl Class for ParamChanges {
    type Interfaces = (IParameterChanges,);
}
impl IParameterChangesTrait for ParamChanges {
    unsafe fn getParameterCount(&self) -> int32 { self.0.borrow().len() as i32 }
    unsafe fn getParameterData(&self, i: int32) -> *mut IParamValueQueue {
        self.0.borrow().get(i as usize).map_or(std::ptr::null_mut(), |q| q.as_com_ref::<IParamValueQueue>().unwrap().as_ptr())
    }
    unsafe fn addParameterData(&self, id: *const ParamID, index: *mut int32) -> *mut IParamValueQueue {
        let id = unsafe { *id };
        let mut v = self.0.borrow_mut();
        let i = match v.iter().position(|q| q.id == id) {
            Some(i) => i,
            None => { v.push(ComWrapper::new(ParamQueue { id, points: RefCell::new(Vec::new()) })); v.len() - 1 }
        };
        if !index.is_null() { unsafe { *index = i as i32 } }
        v[i].as_com_ref::<IParamValueQueue>().unwrap().as_ptr()
    }
}

/// Edits made in the plugin GUI. They must reach the processor through the next process() call.
#[derive(Default)]
pub struct Handler(pub Rc<RefCell<Vec<(ParamID, ParamValue)>>>);
impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}
impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult { kResultOk }
    unsafe fn performEdit(&self, id: ParamID, v: ParamValue) -> tresult {
        self.0.borrow_mut().push((id, v));
        kResultOk
    }
    unsafe fn endEdit(&self, _id: ParamID) -> tresult { kResultOk }
    unsafe fn restartComponent(&self, _flags: int32) -> tresult { kResultOk }
}
