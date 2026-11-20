//! The COM objects a VST3 plugin is handed by its host.
//!
//! Two kinds. Those of the main thread (the host application, messages, the state stream, the
//! component handler) may allocate and lock. Those of the audio thread (the event list and the
//! parameter changes of a block) are made with all their room at load time and never allocate
//! after; they are touched only by the audio thread, and by the plugin while that thread is
//! inside its `process`.
#![allow(non_snake_case)]

use std::cell::{Cell, RefCell, UnsafeCell};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::Mutex;

use vst3::Steinberg::Linux::{
    FileDescriptor, IEventHandler, IRunLoop, IRunLoopTrait, ITimerHandler, TimerInterval,
};
use vst3::Steinberg::Vst::{
    Event, IAttributeList, IAttributeListTrait, IComponentHandler, IComponentHandlerTrait,
    IEventList, IEventListTrait, IHostApplication, IHostApplicationTrait, IMessage, IMessageTrait,
    IParamValueQueue, IParamValueQueueTrait, IParameterChanges, IParameterChangesTrait, ParamID,
    ParamValue, String128, TChar,
};
use vst3::Steinberg::{
    FIDString, IBStream, IBStreamTrait, IPlugFrame, IPlugFrameTrait, IPlugView, IPlugViewTrait,
    TUID, ViewRect, int32, int64, kInvalidArgument, kNoInterface, kResultFalse, kResultOk,
    kResultTrue, tresult, uint32,
};
use vst3::{Class, ComRef, ComWrapper, Interface};

use super::runloop;
use crate::formats::Said;

pub fn write_string128(to: &mut String128, text: &str) {
    let mut end = 0;
    for unit in text.encode_utf16().take(to.len() - 1) {
        to[end] = unit;
        end += 1;
    }
    to[end] = 0;
}

pub fn read_utf16(text: &[TChar]) -> String {
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..end])
}

/// A fixed `char8` field, as far as its first 0.
pub fn read_chars(text: &[c_char]) -> String {
    let bytes: Vec<u8> = text
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// # Safety
/// `tuid` points to 16 readable bytes.
unsafe fn is<I: Interface>(tuid: *const TUID) -> bool {
    !tuid.is_null()
        && unsafe { *tuid }
            .iter()
            .zip(I::IID)
            .all(|(a, b)| *a as u8 == b)
}

/// What a plugin learns about its host, and where it gets the messages its two halves send
/// each other (Surge XT, as every JUCE plugin, needs them).
pub struct HostApp;

impl Class for HostApp {
    type Interfaces = (IHostApplication, IRunLoop);
}

macro_rules! run_loop {
    ($class:ty) => {
        impl IRunLoopTrait for $class {
            unsafe fn registerEventHandler(
                &self,
                handler: *mut IEventHandler,
                fd: FileDescriptor,
            ) -> tresult {
                // SAFETY: the plugin hands in its own handler.
                unsafe { runloop::watch(handler, fd) }
            }

            unsafe fn unregisterEventHandler(&self, handler: *mut IEventHandler) -> tresult {
                runloop::unwatch(handler)
            }

            unsafe fn registerTimer(
                &self,
                handler: *mut ITimerHandler,
                milliseconds: TimerInterval,
            ) -> tresult {
                // SAFETY: as above.
                unsafe { runloop::start_timer(handler, milliseconds) }
            }

            unsafe fn unregisterTimer(&self, handler: *mut ITimerHandler) -> tresult {
                runloop::stop_timer(handler)
            }
        }
    };
}

run_loop!(HostApp);

/// What a plugin's view is put into, as the plugin sees it: where it asks for another size,
/// and for the run loop.
#[derive(Default)]
pub struct Frame {
    /// The size the plugin last asked for, until the main thread takes it.
    pub asked: Cell<Option<(u32, u32)>>,
}

impl Class for Frame {
    type Interfaces = (IPlugFrame, IRunLoop);
}

run_loop!(Frame);

impl IPlugFrameTrait for Frame {
    unsafe fn resizeView(&self, view: *mut IPlugView, new_size: *mut ViewRect) -> tresult {
        // SAFETY: the plugin hands in its own view and a rectangle.
        let (Some(view), Some(rect)) = (unsafe { ComRef::from_raw(view) }, unsafe {
            new_size.as_ref()
        }) else {
            return kInvalidArgument;
        };
        let size = (rect.right - rect.left, rect.bottom - rect.top);
        if size.0 < 1 || size.1 < 1 {
            return kInvalidArgument;
        }
        self.asked.set(Some((size.0 as u32, size.1 as u32)));
        // The window itself follows on the main thread's next round; the view is told now, as
        // VST3 has the host do from inside this call.
        // SAFETY: the view is the plugin's own, alive for this call.
        unsafe { view.onSize(new_size) };
        kResultTrue
    }
}

impl IHostApplicationTrait for HostApp {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kInvalidArgument;
        }
        write_string128(unsafe { &mut *name }, "meloet");
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        obj: *mut *mut c_void,
    ) -> tresult {
        if obj.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: the plugin hands in two class ids and a place for the result.
        unsafe {
            *obj = std::ptr::null_mut();
            if is::<IMessage>(cid) && is::<IMessage>(iid) {
                let message = ComWrapper::new(Message::default());
                if let Some(ptr) = message.to_com_ptr::<IMessage>() {
                    *obj = ptr.into_raw().cast();
                    return kResultOk;
                }
            }
            if is::<IAttributeList>(cid) && is::<IAttributeList>(iid) {
                let attributes = ComWrapper::new(Attributes::default());
                if let Some(ptr) = attributes.to_com_ptr::<IAttributeList>() {
                    *obj = ptr.into_raw().cast();
                    return kResultOk;
                }
            }
        }
        kNoInterface
    }
}

enum Attribute {
    Int(i64),
    Float(f64),
    /// With its closing 0.
    Text(Vec<TChar>),
    Binary(Vec<u8>),
}

#[derive(Default)]
pub struct Attributes(Mutex<HashMap<CString, Attribute>>);

impl Class for Attributes {
    type Interfaces = (IAttributeList,);
}

impl Attributes {
    fn set(&self, id: *const c_char, value: Attribute) -> tresult {
        if id.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: an attribute id is a C string.
        let key = unsafe { CStr::from_ptr(id) }.to_owned();
        match self.0.lock() {
            Ok(mut map) => {
                map.insert(key, value);
                kResultOk
            }
            Err(_) => kResultFalse,
        }
    }

    fn get<R>(&self, id: *const c_char, read: impl FnOnce(&Attribute) -> Option<R>) -> Option<R> {
        if id.is_null() {
            return None;
        }
        // SAFETY: as in `set`.
        let key = unsafe { CStr::from_ptr(id) };
        let map = self.0.lock().ok()?;
        read(map.get(key)?)
    }
}

impl IAttributeListTrait for Attributes {
    unsafe fn setInt(&self, id: *const c_char, value: int64) -> tresult {
        self.set(id, Attribute::Int(value))
    }

    unsafe fn getInt(&self, id: *const c_char, value: *mut int64) -> tresult {
        match self.get(id, |a| match a {
            Attribute::Int(x) => Some(*x),
            _ => None,
        }) {
            Some(x) if !value.is_null() => {
                unsafe { *value = x };
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setFloat(&self, id: *const c_char, value: f64) -> tresult {
        self.set(id, Attribute::Float(value))
    }

    unsafe fn getFloat(&self, id: *const c_char, value: *mut f64) -> tresult {
        match self.get(id, |a| match a {
            Attribute::Float(x) => Some(*x),
            _ => None,
        }) {
            Some(x) if !value.is_null() => {
                unsafe { *value = x };
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setString(&self, id: *const c_char, string: *const TChar) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        let mut text = Vec::new();
        // SAFETY: a 0-terminated UTF-16 string.
        unsafe {
            let mut at = string;
            while *at != 0 {
                text.push(*at);
                at = at.add(1);
            }
        }
        text.push(0);
        self.set(id, Attribute::Text(text))
    }

    unsafe fn getString(&self, id: *const c_char, string: *mut TChar, size: uint32) -> tresult {
        let room = size as usize / std::mem::size_of::<TChar>();
        if string.is_null() || room == 0 {
            return kInvalidArgument;
        }
        let copied = self.get(id, |a| match a {
            Attribute::Text(text) => {
                let n = text.len().min(room);
                // SAFETY: `string` has room for `room` units, as the plugin said.
                unsafe {
                    std::ptr::copy_nonoverlapping(text.as_ptr(), string, n);
                    *string.add(n - 1) = 0;
                }
                Some(())
            }
            _ => None,
        });
        if copied.is_some() {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn setBinary(&self, id: *const c_char, data: *const c_void, size: uint32) -> tresult {
        if data.is_null() && size > 0 {
            return kInvalidArgument;
        }
        let bytes = if size == 0 {
            Vec::new()
        } else {
            // SAFETY: `size` readable bytes, as the plugin said.
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size as usize) }.to_vec()
        };
        self.set(id, Attribute::Binary(bytes))
    }

    unsafe fn getBinary(
        &self,
        id: *const c_char,
        data: *mut *const c_void,
        size: *mut uint32,
    ) -> tresult {
        if data.is_null() || size.is_null() {
            return kInvalidArgument;
        }
        // The bytes stay where they are until the attribute is set again or the list goes,
        // which is as long as VST3 lets the plugin keep the pointer.
        match self.get(id, |a| match a {
            Attribute::Binary(bytes) => Some((bytes.as_ptr(), bytes.len())),
            _ => None,
        }) {
            Some((ptr, len)) => {
                unsafe {
                    *data = ptr.cast();
                    *size = len as uint32;
                }
                kResultOk
            }
            None => kResultFalse,
        }
    }
}

pub struct Message {
    id: Mutex<Option<CString>>,
    attributes: ComWrapper<Attributes>,
}

impl Default for Message {
    fn default() -> Message {
        Message {
            id: Mutex::new(None),
            attributes: ComWrapper::new(Attributes::default()),
        }
    }
}

impl Class for Message {
    type Interfaces = (IMessage,);
}

impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> FIDString {
        match self.id.lock() {
            // The text stays where it is until the id is set again.
            Ok(id) => id.as_ref().map_or(std::ptr::null(), |text| text.as_ptr()),
            Err(_) => std::ptr::null(),
        }
    }

    unsafe fn setMessageID(&self, id: FIDString) {
        if let Ok(mut mine) = self.id.lock() {
            // SAFETY: a message id is a C string.
            *mine = (!id.is_null()).then(|| unsafe { CStr::from_ptr(id) }.to_owned());
        }
    }

    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes
            .as_com_ref::<IAttributeList>()
            .map_or(std::ptr::null_mut(), |list| list.as_ptr())
    }
}

/// A plugin's state on its way out or in. Main thread only.
#[derive(Default)]
pub struct MemStream {
    bytes: RefCell<Vec<u8>>,
    at: Cell<usize>,
}

impl MemStream {
    pub fn reading(bytes: &[u8]) -> MemStream {
        MemStream {
            bytes: RefCell::new(bytes.to_vec()),
            at: Cell::new(0),
        }
    }

    pub fn rewind(&self) {
        self.at.set(0);
    }

    pub fn take(&self) -> Vec<u8> {
        self.at.set(0);
        self.bytes.take()
    }
}

impl Class for MemStream {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for MemStream {
    unsafe fn read(&self, buffer: *mut c_void, count: int32, read: *mut int32) -> tresult {
        let bytes = self.bytes.borrow();
        let at = self.at.get().min(bytes.len());
        let n = (count.max(0) as usize).min(bytes.len() - at);
        if n > 0 {
            if buffer.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: `buffer` has room for `count` bytes, as the plugin said.
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr().add(at), buffer.cast(), n) };
        }
        self.at.set(at + n);
        if !read.is_null() {
            unsafe { *read = n as int32 };
        }
        kResultOk
    }

    unsafe fn write(&self, buffer: *mut c_void, count: int32, written: *mut int32) -> tresult {
        let n = count.max(0) as usize;
        if n > 0 && buffer.is_null() {
            return kInvalidArgument;
        }
        let mut bytes = self.bytes.borrow_mut();
        let at = self.at.get().min(bytes.len());
        if at + n > bytes.len() {
            bytes.resize(at + n, 0);
        }
        if n > 0 {
            // SAFETY: `count` readable bytes, as the plugin said.
            let from = unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), n) };
            bytes[at..at + n].copy_from_slice(from);
        }
        self.at.set(at + n);
        if !written.is_null() {
            unsafe { *written = n as int32 };
        }
        kResultOk
    }

    unsafe fn seek(&self, pos: int64, mode: int32, result: *mut int64) -> tresult {
        let len = self.bytes.borrow().len() as i64;
        let base = match mode {
            0 => 0,
            1 => self.at.get() as i64,
            2 => len,
            _ => return kInvalidArgument,
        };
        let at = base.saturating_add(pos).clamp(0, len);
        self.at.set(at as usize);
        if !result.is_null() {
            unsafe { *result = at };
        }
        kResultOk
    }

    unsafe fn tell(&self, pos: *mut int64) -> tresult {
        if pos.is_null() {
            return kInvalidArgument;
        }
        unsafe { *pos = self.at.get() as int64 };
        kResultOk
    }
}

/// The events of one block, or a place for the events a plugin sends out, which are dropped.
pub struct EventList {
    events: UnsafeCell<Vec<Event>>,
}

// SAFETY: only ever touched by one thread at a time (see the module's text).
unsafe impl Send for EventList {}
unsafe impl Sync for EventList {}

impl EventList {
    pub fn with_capacity(capacity: usize) -> EventList {
        EventList {
            events: UnsafeCell::new(Vec::with_capacity(capacity)),
        }
    }

    // The cell is this type's whole point; see the `Send` and `Sync` above.
    #[allow(clippy::mut_from_ref)]
    fn events(&self) -> &mut Vec<Event> {
        // SAFETY: see the `Send` and `Sync` above; no call here hands a reference out.
        unsafe { &mut *self.events.get() }
    }

    pub fn clear(&self) {
        self.events().clear();
    }

    /// False when the list is full; the event is left out.
    pub fn push(&self, event: Event) -> bool {
        let events = self.events();
        let fits = events.len() < events.capacity();
        if fits {
            events.push(event);
        }
        fits
    }
}

impl Class for EventList {
    type Interfaces = (IEventList,);
}

impl IEventListTrait for EventList {
    unsafe fn getEventCount(&self) -> int32 {
        self.events().len() as int32
    }

    unsafe fn getEvent(&self, index: int32, e: *mut Event) -> tresult {
        match usize::try_from(index)
            .ok()
            .and_then(|i| self.events().get(i))
        {
            Some(event) if !e.is_null() => {
                unsafe { *e = *event };
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn addEvent(&self, e: *mut Event) -> tresult {
        if e.is_null() {
            return kInvalidArgument;
        }
        // A full list takes no more; the plugin is not told, it could do nothing about it.
        self.push(unsafe { *e });
        kResultOk
    }
}

/// Points one parameter gets in one block.
const QUEUE_POINTS: usize = 16;

pub struct ParamQueue {
    id: Cell<ParamID>,
    len: Cell<usize>,
    points: UnsafeCell<[(int32, ParamValue); QUEUE_POINTS]>,
}

// SAFETY: as for `EventList`.
unsafe impl Send for ParamQueue {}
unsafe impl Sync for ParamQueue {}

impl ParamQueue {
    #[allow(clippy::mut_from_ref)]
    fn points(&self) -> &mut [(int32, ParamValue); QUEUE_POINTS] {
        // SAFETY: as for `EventList::events`.
        unsafe { &mut *self.points.get() }
    }

    /// When the queue is full the last point is overwritten: the value the parameter ends the
    /// block with is the one that must not be lost.
    fn add(&self, offset: int32, value: ParamValue) -> usize {
        let at = self.len.get().min(QUEUE_POINTS - 1);
        // The points of a queue go forward in the block.
        let offset = match at.checked_sub(1) {
            Some(before) => offset.max(self.points()[before].0),
            None => offset,
        };
        self.points()[at] = (offset, value);
        self.len.set(at + 1);
        at
    }

    fn last(&self) -> Option<ParamValue> {
        let len = self.len.get();
        (len > 0).then(|| self.points()[len - 1].1)
    }
}

impl Class for ParamQueue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for ParamQueue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id.get()
    }

    unsafe fn getPointCount(&self) -> int32 {
        self.len.get() as int32
    }

    unsafe fn getPoint(&self, index: int32, offset: *mut int32, value: *mut ParamValue) -> tresult {
        match usize::try_from(index).ok().filter(|&i| i < self.len.get()) {
            Some(i) if !offset.is_null() && !value.is_null() => {
                let (at, x) = self.points()[i];
                unsafe {
                    *offset = at;
                    *value = x;
                }
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn addPoint(&self, offset: int32, value: ParamValue, index: *mut int32) -> tresult {
        let at = self.add(offset, value);
        if !index.is_null() {
            unsafe { *index = at as int32 };
        }
        kResultOk
    }
}

/// The parameter changes of one block, either way. A fixed number of parameters fit; changes
/// of more than that in one block are left out.
pub struct ParamChanges {
    queues: Box<[ComWrapper<ParamQueue>]>,
    used: Cell<usize>,
}

// SAFETY: as for `EventList`.
unsafe impl Send for ParamChanges {}
unsafe impl Sync for ParamChanges {}

impl ParamChanges {
    pub fn with_capacity(parameters: usize) -> ParamChanges {
        let queue = || {
            ComWrapper::new(ParamQueue {
                id: Cell::new(0),
                len: Cell::new(0),
                points: UnsafeCell::new([(0, 0.0); QUEUE_POINTS]),
            })
        };
        ParamChanges {
            queues: (0..parameters).map(|_| queue()).collect(),
            used: Cell::new(0),
        }
    }

    pub fn clear(&self) {
        self.used.set(0);
    }

    fn queue(&self, id: ParamID) -> Option<usize> {
        let used = self.used.get();
        if let Some(at) = self.queues[..used].iter().position(|q| q.id.get() == id) {
            return Some(at);
        }
        let queue = self.queues.get(used)?;
        queue.id.set(id);
        queue.len.set(0);
        self.used.set(used + 1);
        Some(used)
    }

    /// False when no more parameters fit in this block.
    pub fn add(&self, id: ParamID, offset: int32, value: ParamValue) -> bool {
        match self.queue(id) {
            Some(at) => {
                self.queues[at].add(offset, value);
                true
            }
            None => false,
        }
    }

    /// Every parameter that has points, with its last one.
    pub fn last_values(&self) -> impl Iterator<Item = (ParamID, ParamValue)> + '_ {
        self.queues[..self.used.get()]
            .iter()
            .filter_map(|q| Some((q.id.get(), q.last()?)))
    }
}

impl Class for ParamChanges {
    type Interfaces = (IParameterChanges,);
}

impl IParameterChangesTrait for ParamChanges {
    unsafe fn getParameterCount(&self) -> int32 {
        self.used.get() as int32
    }

    unsafe fn getParameterData(&self, index: int32) -> *mut IParamValueQueue {
        usize::try_from(index)
            .ok()
            .filter(|&i| i < self.used.get())
            .and_then(|i| self.queues[i].as_com_ref::<IParamValueQueue>())
            .map_or(std::ptr::null_mut(), |queue| queue.as_ptr())
    }

    unsafe fn addParameterData(
        &self,
        id: *const ParamID,
        index: *mut int32,
    ) -> *mut IParamValueQueue {
        if id.is_null() {
            return std::ptr::null_mut();
        }
        let Some(at) = self.queue(unsafe { *id }) else {
            return std::ptr::null_mut();
        };
        if !index.is_null() {
            unsafe { *index = at as int32 };
        }
        self.queues[at]
            .as_com_ref::<IParamValueQueue>()
            .map_or(std::ptr::null_mut(), |queue| queue.as_ptr())
    }
}

/// What the plugin told its host since the main thread last looked.
#[derive(Default)]
pub struct Told {
    pub said: Vec<Said>,
    /// `RestartFlags`, or-ed together.
    pub restart: int32,
}

/// Where the plugin's controller reports what the person does in its window, and asks for
/// restarts. VST3 has these calls come on the main thread; a plugin that calls from another
/// thread while the main thread is reading loses that one report, and nothing waits.
#[derive(Default)]
pub struct Handler {
    pub told: Mutex<Told>,
}

impl Handler {
    fn tell(&self, change: impl FnOnce(&mut Told)) -> tresult {
        match self.told.try_lock() {
            Ok(mut told) => {
                change(&mut told);
                kResultOk
            }
            Err(_) => kResultFalse,
        }
    }
}

impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, id: ParamID) -> tresult {
        let at = super::super::now_us();
        self.tell(|told| told.said.push(Said::ParamBegin(id, at)))
    }

    unsafe fn performEdit(&self, id: ParamID, value: ParamValue) -> tresult {
        if !value.is_finite() {
            return kInvalidArgument;
        }
        let at = super::super::now_us();
        self.tell(|told| {
            told.said
                .push(Said::ParamChanged(id, value.clamp(0.0, 1.0), at))
        })
    }

    unsafe fn endEdit(&self, id: ParamID) -> tresult {
        let at = super::super::now_us();
        self.tell(|told| told.said.push(Said::ParamEnd(id, at)))
    }

    unsafe fn restartComponent(&self, flags: int32) -> tresult {
        self.tell(|told| told.restart |= flags)
    }
}
