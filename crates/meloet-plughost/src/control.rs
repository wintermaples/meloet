//! The main thread in `host` mode: answers the requests on the control socket
//! (`meloet_plugin::wire`) and owns the controller half of every plugin, with its window.
//!
//! The process ends when the socket closes: meloet is gone, or it let this process go.
//!
//! The thread sleeps in `poll`, never in a read: on the socket, on the X11 connection of the
//! windows, and on what the plugins asked to have watched (`formats::runloop`), for no longer
//! than a tick while a plugin is loaded, so that every plugin is looked after many times a
//! second ([`Controller::poll`]). The audio threads (`crate::rt`) never wait for this thread:
//! a plugin whose window hangs still sounds.
//!
//! A window is asked for with `OpenEditor` and answered at once; it is made after the answer
//! is written, so meloet can count how long the making takes.

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_plugin::ipc::{Mapping, PluginEnd};
use meloet_plugin::wire::{
    EditorOpen, FromHost, MAX_BLOB_LEN, MAX_TEXT_LEN, Notice, ParamNow, Reply, Request, TextValue,
    ToHost, WIRE_VERSION, WireError, read_frame, write_frame,
};

use crate::formats::outputs::Capture;
use crate::formats::{self, Activation, Controller, Said, half_width, past_the_end, runloop};
use crate::rt::{self, Moved, SlotRun};
use crate::window::{Display, FocusAt, WindowEvent};

/// How long the thread sleeps at most while a plugin is loaded.
const TICK: Duration = Duration::from_millis(10);
/// How often a plugin with a window gets its idle call.
const IDLE: Duration = Duration::from_millis(16);
/// One parameter moved in a window is told to meloet no more often than this.
const TELL: Duration = Duration::from_millis(33);
/// While the person is in a window: how often it is looked whether they still are. The
/// keyboard going from a plugin's own menu to another program tells the window nothing.
const FOCUS_LOOK: Duration = Duration::from_millis(100);
/// What a window has when its plugin names no size and has made no window inside it.
const SIZE_UNKNOWN: (u32, u32) = (640, 480);
/// A window the desktop has not put on the screen after this long is told as open anyway
/// (it may be on another workspace).
const SHOW_WAIT: Duration = Duration::from_millis(500);
/// A parameter the plugin changes with no word of the person taking hold of it (no VST3
/// `beginEdit`, no LV2 `ui:touch`, a `patch` parameter) is let go this long after its last
/// change (docs/history/research/automation-lv2.md).
const LET_GO: Duration = Duration::from_millis(500);
/// How long a state waits for a block to take the values set before it: two of the longest
/// blocks the engine asks for (1024 frames at 44.1 kHz) and more.
const PARAMS_WAIT: Duration = Duration::from_millis(100);
/// A state put while the slot's blocks run is in the plugin once this many blocks have ended
/// since: one that was under way when it was put, the one the plugin takes it in with
/// (Surge XT), and one more, in which Surge XT's state still changed after a patch.
const STATE_BLOCKS: u64 = 3;
/// The slot's blocks run when the last one ended no longer ago than this: two of the longest
/// blocks the engine asks for (1024 frames at 44.1 kHz) and more.
const RUNNING: Duration = Duration::from_millis(50);
/// The longest wait of one request for [`STATE_BLOCKS`], well inside the time meloet gives an
/// answer (`ANSWER_WAIT` of `meloet_core::plugins::hosts`, 5 s).
const STATE_WAIT: Duration = Duration::from_millis(250);

/// A plugin's window on the screen.
struct Top {
    id: u32,
    size: (u32, u32),
    resizable: bool,
    /// The plugin never named a size: the window takes the size of the plugin's own window
    /// inside it.
    follows_child: bool,
    /// The keyboard is in it.
    inside: bool,
    /// When the desktop was asked to put it on the screen, until meloet is told that it is
    /// open.
    asked_to_show: Option<Instant>,
}

/// A parameter being moved in a window, as far as meloet was told.
struct Told {
    at: Instant,
    /// The newest value and its time, when it came too soon after the last one told.
    held: Option<(f64, u64)>,
}

/// A parameter the person has hold of in a window.
struct Hold {
    /// The plugin said so (`ParamBegin`); else it is let go [`LET_GO`] after `last`.
    said: bool,
    last: Instant,
}

struct Host {
    map: Arc<Mapping>,
    audio: rt::Handle,
    /// The controller of the plugin in each slot.
    slots: Vec<Option<Box<dyn Controller>>>,
    greeted: bool,
    /// The process is for an export.
    offline: bool,
    /// Made with the first window.
    display: Option<Display>,
    /// By slot.
    tops: Vec<Option<Top>>,
    /// Windows asked for and not made yet: slot and title.
    asked: VecDeque<(u32, String)>,
    /// By slot and parameter.
    told: HashMap<(u32, u32), Told>,
    /// By slot and parameter.
    holds: HashMap<(u32, u32), Hold>,
    /// By slot: parameters taken hold of and programs loaded since the person came into the
    /// window.
    touches: Vec<u32>,
    /// By slot: what meloet's automation did, for the plugins meloet watches.
    moved: Vec<Option<Moved>>,
    /// By slot: when a value was last given to the audio thread from here.
    params_set: Vec<Option<Instant>>,
    /// By slot: the slot's block count when a state was put that it may not have taken yet.
    state_put: Vec<Option<u64>>,
    /// The slots meloet watches, and when it was last told of them.
    watched: Vec<u32>,
    watch_told: Instant,
    /// What meloet is told next.
    notices: Vec<Notice>,
    idled: Instant,
    focus_looked: Instant,
}

/// What a request is answered with: the reply and the frame's blob.
type Answer = (Reply, Vec<u8>);

fn ok() -> Answer {
    (Reply::Ok, Vec::new())
}

fn error(message: impl Into<String>) -> Answer {
    (
        Reply::Error {
            message: message.into(),
        },
        Vec::new(),
    )
}

impl Host {
    fn controller(&mut self, slot: u32) -> Result<&mut Box<dyn Controller>, Answer> {
        self.slots
            .get_mut(slot as usize)
            .and_then(Option::as_mut)
            .ok_or_else(|| error(format!("no plugin in slot {slot}")))
    }

    fn load(&mut self, slot: u32, request: &Request) -> Answer {
        let Request::Load {
            key,
            bundle,
            sample_rate,
            max_frames,
            outputs,
            ..
        } = request
        else {
            return error("not a load");
        };
        let geometry = self.map.geometry();
        match self.slots.get(slot as usize) {
            None => return error(format!("no slot {slot}")),
            Some(Some(_)) => return error(format!("slot {slot} is taken")),
            Some(None) => {}
        }
        if *sample_rate == 0 || *max_frames == 0 || *max_frames > geometry.max_frames {
            return error("sample rate or block size out of range");
        }
        let activation = Activation {
            sample_rate: f64::from(*sample_rate),
            max_frames: *max_frames as usize,
            offline: self.offline,
        };
        let mut loaded = match formats::load(key, bundle, activation, outputs) {
            Ok(loaded) => loaded,
            Err(message) => return error(message),
        };
        if let Some(sets) = loaded.processor.outputs() {
            sets.set(outputs, false);
        }
        let end = match PluginEnd::new(self.map.clone(), slot) {
            Ok(end) => end,
            Err(e) => return error(e.to_string()),
        };
        let (run, moved) = SlotRun::new(&self.map, end, loaded.processor, loaded.plugin.latency);
        if let Err(message) = self.audio.install(run) {
            return error(message);
        }
        self.moved[slot as usize] = Some(moved);
        let mut plugin = loaded.plugin;
        let mut controller = loaded.controller;
        plugin.key_names = controller.key_names();
        self.slots[slot as usize] = Some(controller);
        (Reply::Loaded { plugin }, Vec::new())
    }

    fn top(&self, slot: u32) -> Option<&Top> {
        self.tops.get(slot as usize).and_then(Option::as_ref)
    }

    /// Takes the plugin's window off the screen; true when there was one. The plugin lets go
    /// of it first: a plugin whose window is pulled from under it ends the process.
    fn close_window(&mut self, slot: u32) -> Option<Top> {
        self.asked.retain(|(asked, _)| *asked != slot);
        let top = self.tops.get_mut(slot as usize)?.take()?;
        if let Some(controller) = self.slots.get_mut(slot as usize).and_then(Option::as_mut) {
            controller.close_editor();
        }
        if let Some(display) = &self.display {
            display.destroy(top.id);
        }
        let held: Vec<(u32, u32)> = (self.holds.keys())
            .filter(|(of, _)| *of == slot)
            .copied()
            .collect();
        for (slot, id) in held {
            self.let_go(slot, id, formats::now_us());
        }
        self.told.retain(|(of, _), _| *of != slot);
        Some(top)
    }

    /// The person took hold of a parameter: told once, and its automation is passed over
    /// from the next block until it is let go.
    fn take_hold(&mut self, slot: u32, id: u32, t_us: u64, said: bool) {
        let last = Instant::now();
        if let Some(hold) = self.holds.get_mut(&(slot, id)) {
            hold.said |= said;
            hold.last = last;
            return;
        }
        self.holds.insert((slot, id), Hold { said, last });
        if let Some(touches) = self.touches.get_mut(slot as usize) {
            *touches += 1;
        }
        let _ = self.audio.touch(slot, id, true);
        self.notices.push(Notice::ParamBegin { slot, id, t_us });
    }

    /// The person let go of a parameter: the value held back is told first.
    fn let_go(&mut self, slot: u32, id: u32, t_us: u64) {
        if let Some((value, at)) = self.told.remove(&(slot, id)).and_then(|t| t.held) {
            self.tell_param(slot, id, value, at);
        }
        if self.holds.remove(&(slot, id)).is_some() {
            let _ = self.audio.touch(slot, id, false);
            self.notices.push(Notice::ParamEnd { slot, id, t_us });
        }
    }

    fn left(&mut self, slot: u32) -> Notice {
        let touches = self
            .touches
            .get_mut(slot as usize)
            .map_or(0, std::mem::take);
        Notice::EditorLeft { slot, touches }
    }

    /// Makes the window asked for and puts the plugin's own into it. meloet is told when the
    /// desktop has put it on the screen ([`Host::shown`]).
    fn open_window(&mut self, slot: u32, title: &str) -> Result<(), String> {
        if self.display.is_none() {
            self.display = Some(Display::open()?);
        }
        let (Some(display), Some(controller)) = (
            &self.display,
            self.slots.get_mut(slot as usize).and_then(Option::as_mut),
        ) else {
            return Err(format!("no plugin in slot {slot}"));
        };
        let id = display.create(title)?;
        let opened = match controller.open_editor(id) {
            Ok(opened) => opened,
            Err(message) => {
                display.destroy(id);
                return Err(message);
            }
        };
        let size = (opened.size)
            .or_else(|| display.child_size(id))
            .unwrap_or(SIZE_UNKNOWN);
        display.show(id, size, opened.resizable);
        self.tops[slot as usize] = Some(Top {
            id,
            size,
            resizable: opened.resizable,
            follows_child: opened.size.is_none(),
            inside: false,
            asked_to_show: Some(Instant::now()),
        });
        Ok(())
    }

    /// The window asked for is on the screen: meloet is told, once.
    fn shown(&mut self, slot: u32) {
        let (Some(display), Some(top)) = (
            &self.display,
            self.tops.get_mut(slot as usize).and_then(Option::as_mut),
        ) else {
            return;
        };
        if top.asked_to_show.take().is_some() {
            self.notices.push(Notice::EditorOpened {
                slot,
                width: top.size.0,
                height: top.size.1,
                larger_than_screen: display.larger_than_screen(top.id, top.size),
            });
        }
    }

    /// Makes the windows asked for since the last call.
    fn open_asked(&mut self) {
        while let Some((slot, title)) = self.asked.pop_front() {
            if let Err(message) = self.open_window(slot, &title) {
                self.notices.push(Notice::EditorFailed { slot, message });
            }
        }
    }

    /// The window has another size from now on, by the plugin's will.
    fn resized(&mut self, slot: u32, size: (u32, u32)) {
        let (Some(display), Some(top)) = (
            &self.display,
            self.tops.get_mut(slot as usize).and_then(Option::as_mut),
        ) else {
            return;
        };
        if size == top.size || size.0 < 2 || size.1 < 2 {
            return;
        }
        top.size = size;
        display.resize(top.id, size, top.resizable);
        if top.asked_to_show.is_some() {
            // `EditorOpened` will tell this size.
            return;
        }
        self.notices.push(Notice::EditorResized {
            slot,
            width: size.0,
            height: size.1,
            larger_than_screen: display.larger_than_screen(top.id, size),
        });
    }

    /// The person closed the window, or the plugin did.
    fn closed(&mut self, slot: u32) {
        if let Some(top) = self.close_window(slot) {
            if top.inside {
                let left = self.left(slot);
                self.notices.push(left);
                if let Some(controller) = self.slots.get_mut(slot as usize).and_then(Option::as_mut)
                {
                    controller.editor_left();
                }
            }
            self.notices.push(if top.asked_to_show.is_some() {
                let message = "closed before it was on the screen".into();
                Notice::EditorFailed { slot, message }
            } else {
                Notice::EditorClosed { slot }
            });
        }
    }

    /// Looks where the keyboard is and tells the windows it came to or left.
    fn look_at_focus(&mut self) {
        self.focus_looked = Instant::now();
        let Some(display) = &self.display else {
            return;
        };
        let ids: Vec<u32> = self.tops.iter().flatten().map(|top| top.id).collect();
        if ids.is_empty() {
            return;
        }
        let at = display.focus(&ids);
        let mut entered = None;
        let mut gone = Vec::new();
        for (slot, top) in self.tops.iter_mut().enumerate() {
            let Some(top) = top else {
                continue;
            };
            let inside = match at {
                FocusAt::Window(id) => id == top.id,
                // A menu or a dialog of the plugin the person is in.
                FocusAt::Client(client) => top.inside && display.clients(top.id).contains(&client),
                FocusAt::Elsewhere => false,
            };
            if inside == top.inside {
                continue;
            }
            top.inside = inside;
            let slot = slot as u32;
            if inside {
                entered = Some(slot);
            } else {
                gone.push(slot);
            }
        }
        for slot in gone {
            let left = self.left(slot);
            self.notices.push(left);
            if let Some(controller) = self.slots[slot as usize].as_mut() {
                controller.editor_left();
            }
        }
        // Leaving one window is told before entering the next.
        if let Some(slot) = entered {
            if let Some(touches) = self.touches.get_mut(slot as usize) {
                *touches = 0;
            }
            self.notices.push(Notice::EditorEntered { slot });
        }
    }

    /// The X connection is gone: every window is closed as if its plugin had closed it, and
    /// the connection is let go. The next window asked for makes a new one.
    fn display_lost(&mut self) {
        for slot in 0..self.tops.len() as u32 {
            self.closed(slot);
        }
        self.display = None;
    }

    /// What happened to the windows from outside the plugins.
    fn window_events(&mut self, events: &mut Vec<WindowEvent>) {
        events.clear();
        match &self.display {
            Some(display) => display.events(events),
            None => return,
        }
        if self.display.as_ref().is_some_and(Display::broken) {
            self.display_lost();
            return;
        }
        let slot_of = |tops: &[Option<Top>], id: u32| {
            (tops.iter())
                .position(|top| top.as_ref().is_some_and(|top| top.id == id))
                .map(|slot| slot as u32)
        };
        let mut focus = false;
        for &event in events.iter() {
            match event {
                WindowEvent::Shown(id) => {
                    if let Some(slot) = slot_of(&self.tops, id) {
                        self.shown(slot);
                    }
                }
                WindowEvent::CloseAsked(id) => {
                    if let Some(slot) = slot_of(&self.tops, id) {
                        self.closed(slot);
                    }
                }
                WindowEvent::Sized(id, width, height) => {
                    let Some(slot) = slot_of(&self.tops, id) else {
                        continue;
                    };
                    let resizable = self.top(slot).is_some_and(|top| {
                        top.resizable && top.size != (width, height) && width > 1 && height > 1
                    });
                    if resizable && let Some(controller) = self.slots[slot as usize].as_mut() {
                        let size = controller.resize_editor(width, height);
                        let shown = self.tops[slot as usize].as_mut().is_some_and(|top| {
                            // What the person made it; `resized` then makes it what the
                            // plugin allows.
                            top.size = (width, height);
                            top.asked_to_show.is_none()
                        });
                        if size != (width, height) {
                            self.resized(slot, size);
                        } else if shown && let Some(display) = &self.display {
                            let larger_than_screen = display.larger_than_screen(id, size);
                            self.notices.push(Notice::EditorResized {
                                slot,
                                width,
                                height,
                                larger_than_screen,
                            });
                        }
                    }
                }
                WindowEvent::ChildSized(id, width, height) => {
                    if let Some(slot) = slot_of(&self.tops, id)
                        && self.top(slot).is_some_and(|top| top.follows_child)
                    {
                        self.resized(slot, (width, height));
                    }
                }
                WindowEvent::Focus => focus = true,
            }
        }
        let late = |top: &Option<Top>| {
            (top.as_ref().and_then(|top| top.asked_to_show))
                .is_some_and(|asked| asked.elapsed() >= SHOW_WAIT)
        };
        for slot in 0..self.tops.len() {
            if late(&self.tops[slot]) {
                self.shown(slot as u32);
            }
        }
        let inside = self.tops.iter().flatten().any(|top| top.inside);
        if focus || (inside && self.focus_looked.elapsed() >= FOCUS_LOOK) {
            self.look_at_focus();
        }
    }

    /// The idle call of every plugin with a window, when it is time.
    fn idle(&mut self) {
        if self.idled.elapsed() < IDLE {
            return;
        }
        self.idled = Instant::now();
        for (top, controller) in self.tops.iter().zip(&mut self.slots) {
            if let (Some(_), Some(controller)) = (top, controller) {
                controller.idle();
            }
        }
    }

    fn tell_param(&mut self, slot: u32, id: u32, value: f64, t_us: u64) {
        let text = (self.slots[slot as usize].as_mut()).and_then(|c| c.text(id, value));
        self.notices.push(Notice::ParamChanged {
            slot,
            id,
            value,
            text,
            t_us,
        });
    }

    /// Looks after every plugin and gathers what they did by themselves.
    fn look_after(&mut self, said: &mut Vec<Said>) {
        for slot in 0..self.slots.len() {
            let Some(controller) = self.slots[slot].as_mut() else {
                continue;
            };
            said.clear();
            controller.poll(said);
            let slot = slot as u32;
            for &event in said.iter() {
                match event {
                    Said::ParamBegin(id, t_us) => self.take_hold(slot, id, t_us, true),
                    Said::ParamChanged(id, value, t_us) => {
                        self.take_hold(slot, id, t_us, false);
                        // What the person did in the plugin's window reaches the plugin's
                        // sound with the next block, however often meloet is told.
                        let _ = self.audio.set_param(slot, id, value);
                        let now = Instant::now();
                        self.params_set[slot as usize] = Some(now);
                        match self.told.get_mut(&(slot, id)) {
                            Some(told) if now.duration_since(told.at) < TELL => {
                                told.held = Some((value, t_us));
                            }
                            _ => {
                                self.told.insert(
                                    (slot, id),
                                    Told {
                                        at: now,
                                        held: None,
                                    },
                                );
                                self.tell_param(slot, id, value, t_us);
                            }
                        }
                    }
                    Said::ParamEnd(id, t_us) => self.let_go(slot, id, t_us),
                    Said::Preset => {
                        if let Some(touches) = (self.touches.get_mut(slot as usize))
                            .filter(|_| self.tops[slot as usize].is_some())
                        {
                            *touches += 1;
                        }
                    }
                    Said::Latency(frames) => {
                        self.notices.push(Notice::LatencyChanged { slot, frames });
                    }
                    Said::ParamsChanged => self.notices.push(Notice::ParamsChanged { slot }),
                    Said::Resized(width, height) => {
                        if let Some(top) = self.tops[slot as usize].as_mut() {
                            top.follows_child = false;
                        }
                        self.resized(slot, (width, height));
                    }
                    Said::Closed => self.closed(slot),
                    Said::KeyNames => {
                        let names =
                            (self.slots[slot as usize].as_mut()).and_then(|c| c.key_names());
                        self.notices.push(Notice::KeyNames { slot, names });
                    }
                }
            }
        }
        // The values held back, once they have waited long enough; and nothing is kept of a
        // parameter that was let go without a word.
        let now = Instant::now();
        let due: Vec<(u32, u32, (f64, u64))> = (self.told.iter_mut())
            .filter(|(_, told)| now.duration_since(told.at) >= TELL)
            .filter_map(|(&(slot, id), told)| {
                told.at = now;
                Some((slot, id, told.held.take()?))
            })
            .collect();
        for (slot, id, (value, t_us)) in due {
            self.tell_param(slot, id, value, t_us);
        }
        let quiet: Vec<(u32, u32)> = (self.holds.iter())
            .filter(|(_, hold)| !hold.said && now.duration_since(hold.last) >= LET_GO)
            .map(|(&key, _)| key)
            .collect();
        for (slot, id) in quiet {
            self.let_go(slot, id, formats::now_us());
        }
        self.told
            .retain(|_, told| told.held.is_some() || now.duration_since(told.at) < TELL * 4);
        self.tell_automated();
    }

    /// What meloet's automation gave the watched plugins since the last time: the newest
    /// value of each parameter.
    fn tell_automated(&mut self) {
        if self.watched.is_empty() || self.watch_told.elapsed() < TELL {
            return;
        }
        self.watch_told = Instant::now();
        let mut newest: Vec<(u32, f64)> = Vec::new();
        for at in 0..self.watched.len() {
            let slot = self.watched[at];
            let Some(moved) = self.moved[slot as usize].as_mut() else {
                continue;
            };
            newest.clear();
            for (id, value) in moved.take() {
                match newest.iter_mut().find(|(known, _)| *known == id) {
                    Some(known) => known.1 = value,
                    None => newest.push((id, value)),
                }
            }
            for &(id, value) in &newest {
                let text = (self.slots[slot as usize].as_mut()).and_then(|c| c.text(id, value));
                self.notices.push(Notice::ParamAutomated {
                    slot,
                    id,
                    value,
                    text,
                });
            }
        }
    }

    /// A VST3 processor (ZamComp's, of DPF) learns a value set from outside in its next block
    /// only, and its state says the old value until then. So a state is taken once a block
    /// has taken the values set before it; while meloet's engine runs no blocks, from
    /// [`PARAMS_WAIT`] after the last value on, as it is.
    fn wait_for_params(&self, slot: u32) {
        let Some(set) = self.params_set.get(slot as usize).copied().flatten() else {
            return;
        };
        while set.elapsed() < PARAMS_WAIT && !self.audio.params_taken(slot).unwrap_or(true) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// A state was put into the plugin. Once a plugin has run blocks, some take a state in
    /// only with their next block (Surge XT, docs/architecture.md 20-3), and a state taken
    /// before that says the old one, or a patch half loaded. One that never ran a block takes
    /// it at once.
    fn state_was_put(&mut self, slot: u32) {
        let blocks = self.audio.blocks(slot).count;
        self.state_put[slot as usize] = (blocks > 0).then_some(blocks);
    }

    /// Everything asked of a plugin after a state was put into it waits until the plugin has
    /// it: [`STATE_BLOCKS`] blocks, for [`STATE_WAIT`] at most. Not while the slot's blocks
    /// do not run: the state is still to come then, and is waited for when they run again.
    fn wait_for_state(&mut self, slot: u32) {
        let Some(&Some(put)) = self.state_put.get(slot as usize) else {
            return;
        };
        let started = Instant::now();
        loop {
            let blocks = self.audio.blocks(slot);
            if blocks.count.wrapping_sub(put) >= STATE_BLOCKS || started.elapsed() >= STATE_WAIT {
                self.state_put[slot as usize] = None;
                return;
            }
            if !blocks.in_one && blocks.since.is_none_or(|since| since > RUNNING) {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn handle(&mut self, request: Request, blob: Vec<u8>) -> Answer {
        if !self.greeted && !matches!(request, Request::Hello { .. }) {
            return error("hello comes first");
        }
        match request {
            Request::Load { slot, .. } | Request::Unload { slot } => {
                if let Some(put) = self.state_put.get_mut(slot as usize) {
                    *put = None;
                }
            }
            _ => {
                if let Some(slot) = slot_of(&request) {
                    self.wait_for_state(slot);
                }
            }
        }
        match request {
            Request::Hello { version } => {
                self.greeted = version == WIRE_VERSION;
                (
                    Reply::Hello {
                        version: WIRE_VERSION,
                        offline: self.offline,
                    },
                    Vec::new(),
                )
            }
            Request::Load { slot, .. } => self.load(slot, &request),
            Request::Unload { slot } => {
                self.close_window(slot);
                self.watched.retain(|watched| *watched != slot);
                if let Some(moved) = self.moved.get_mut(slot as usize) {
                    *moved = None;
                }
                let Some(controller) = self.slots.get_mut(slot as usize).and_then(Option::take)
                else {
                    return error(format!("no plugin in slot {slot}"));
                };
                match self.audio.remove(slot) {
                    Ok(()) => {
                        drop(controller);
                        ok()
                    }
                    Err(message) => {
                        // The audio thread is stuck inside a plugin and still holds this
                        // one's other half: ending it now would pull it from under that
                        // thread.
                        std::mem::forget(controller);
                        error(message)
                    }
                }
            }
            Request::GetState { slot } => {
                self.wait_for_params(slot);
                match self.controller(slot).map(|c| c.save_state()) {
                    Ok(Ok(state)) => (Reply::State, state),
                    Ok(Err(message)) => error(message),
                    Err(answer) => answer,
                }
            }
            Request::SetState { slot } => {
                if let Err(message) = self.audio.forget_params(slot) {
                    return error(message);
                }
                match self.controller(slot).map(|c| c.load_state(&blob)) {
                    Ok(Ok(())) => {
                        self.state_was_put(slot);
                        ok()
                    }
                    Ok(Err(message)) => error(message),
                    Err(answer) => answer,
                }
            }
            Request::Presets { slot, bundles } => {
                match self.controller(slot).map(|c| c.presets(&bundles)) {
                    Ok(Ok(presets)) => (Reply::Presets { presets }, Vec::new()),
                    Ok(Err(message)) => error(message),
                    Err(answer) => answer,
                }
            }
            Request::PresetState { slot, uri, bundles } => {
                match self
                    .controller(slot)
                    .map(|c| c.preset_state(&uri, &bundles))
                {
                    Ok(Ok(state)) => (Reply::State, state),
                    Ok(Err(message)) => error(message),
                    Err(answer) => answer,
                }
            }
            Request::SavePreset {
                slot,
                dir,
                file,
                label,
            } => {
                self.wait_for_params(slot);
                let saved = self.controller(slot);
                match saved.map(|c| c.save_preset(&dir, &file, &label)) {
                    Ok(Ok(preset)) => (
                        Reply::Presets {
                            presets: vec![preset],
                        },
                        Vec::new(),
                    ),
                    Ok(Err(message)) => error(message),
                    Err(answer) => answer,
                }
            }
            Request::Params { slot } => match self.controller(slot) {
                Ok(controller) => (
                    Reply::Params {
                        params: controller.params(),
                    },
                    Vec::new(),
                ),
                Err(answer) => answer,
            },
            Request::GetParam { slot, id } => match self.controller(slot).map(|c| c.param(id)) {
                Ok(Some(now)) => (Reply::Param { now }, Vec::new()),
                Ok(None) => error(format!("no parameter {id}")),
                Err(answer) => answer,
            },
            Request::SetParam { slot, id, value } => {
                if !value.is_finite() {
                    return error("not a number");
                }
                let value = value.clamp(0.0, 1.0);
                match self.controller(slot) {
                    Ok(controller) => controller.set_param(id, value),
                    Err(answer) => return answer,
                }
                if let Err(message) = self.audio.set_param(slot, id, value) {
                    return error(message);
                }
                self.params_set[slot as usize] = Some(Instant::now());
                let now = (self.slots[slot as usize].as_mut())
                    .and_then(|c| c.param(id))
                    .unwrap_or(ParamNow {
                        id,
                        value: None,
                        text: None,
                    });
                (Reply::Param { now }, Vec::new())
            }
            Request::Texts { slot, id, values } => match self.controller(slot) {
                Ok(controller) => {
                    let texts = (values.iter())
                        .map(|value| {
                            (value.is_finite())
                                .then(|| controller.text(id, value.clamp(0.0, 1.0)))
                                .flatten()
                        })
                        .collect();
                    (Reply::Texts { texts }, Vec::new())
                }
                Err(answer) => answer,
            },
            Request::ReadText { slot, id, text } => match self.controller(slot) {
                Ok(controller) => {
                    let text = half_width(&text);
                    let read = controller.read_text(id, &text).map(|read| {
                        let value = read.value.clamp(0.0, 1.0);
                        let (low, high) = (controller.text(id, 0.0), controller.text(id, 1.0));
                        TextValue {
                            value,
                            clamped: read.clamped.unwrap_or_else(|| {
                                past_the_end(&text, value, low.as_deref(), high.as_deref())
                            }),
                            text: controller.text(id, value),
                            low,
                            high,
                        }
                    });
                    (Reply::Read { read }, Vec::new())
                }
                Err(answer) => answer,
            },
            Request::OpenEditor { slot, title } => {
                if let Err(answer) = self.controller(slot) {
                    return answer;
                }
                let open = match (self.top(slot), &self.display) {
                    (Some(top), Some(display)) => {
                        display.raise(top.id);
                        EditorOpen::Raised
                    }
                    _ if self.asked.iter().any(|(asked, _)| *asked == slot) => EditorOpen::Raised,
                    _ => {
                        self.asked.push_back((slot, title));
                        EditorOpen::Opening
                    }
                };
                (Reply::Editor { open }, Vec::new())
            }
            Request::CloseEditor { slot } => {
                if let Err(answer) = self.controller(slot) {
                    return answer;
                }
                self.close_window(slot);
                ok()
            }
            Request::Watch { slot, on } => {
                if let Err(answer) = self.controller(slot) {
                    return answer;
                }
                self.watched.retain(|watched| *watched != slot);
                if on {
                    self.watched.push(slot);
                }
                if let Some(moved) = self.moved[slot as usize].as_mut() {
                    moved.watch(on);
                }
                ok()
            }
            Request::SetOutputs { slot, outputs } => {
                match self.controller(slot).map(|c| c.set_outputs(&outputs)) {
                    Ok(Ok(())) => {}
                    Ok(Err(message)) => return error(message),
                    Err(answer) => return answer,
                }
                match self.audio.outputs(slot, |sets| sets.set(&outputs, true)) {
                    Ok(_) => ok(),
                    Err(message) => error(message),
                }
            }
            Request::Capture { slot, frames } => {
                if let Err(answer) = self.controller(slot) {
                    return answer;
                }
                if !self.offline {
                    return error("only a host for an export keeps the sound of output sets");
                }
                let capture = match self.audio.outputs(slot, |sets| sets.ids()) {
                    Ok(Some(ids)) => match Capture::new(ids, frames) {
                        Ok(capture) => capture,
                        Err(message) => return error(message),
                    },
                    Ok(None) => return error("this plugin has no output sets"),
                    Err(message) => return error(message),
                };
                // The capture kept before is dropped here, not at the seat.
                match self.audio.outputs(slot, |sets| sets.arm(capture)) {
                    Ok(_) => ok(),
                    Err(message) => error(message),
                }
            }
            Request::Captured { slot } => {
                if let Err(answer) = self.controller(slot) {
                    return answer;
                }
                match self.audio.outputs(slot, |sets| sets.take_capture()) {
                    Ok(Some(Some(capture))) => {
                        let (sets, blob) = capture.reply();
                        (Reply::Captured { sets }, blob)
                    }
                    Ok(_) => error("no capture was asked for"),
                    Err(message) => error(message),
                }
            }
            Request::Quit => ok(),
        }
    }

    /// How long the thread may sleep.
    fn nap(&self) -> i32 {
        let mut nap = (self.slots.iter().any(Option::is_some)).then_some(TICK);
        let mut sooner = |other: Duration| nap = Some(nap.map_or(other, |nap| nap.min(other)));
        if self.tops.iter().any(Option::is_some) {
            sooner(IDLE.saturating_sub(self.idled.elapsed()));
        }
        if let Some(timer) = runloop::next_timer() {
            sooner(timer);
        }
        if !self.asked.is_empty() || !self.notices.is_empty() {
            sooner(Duration::ZERO);
        }
        if let Some(last) = self
            .holds
            .values()
            .filter(|h| !h.said)
            .map(|h| h.last)
            .min()
        {
            sooner(LET_GO.saturating_sub(last.elapsed()));
        }
        // Rounded up: a nap cut short by a millisecond would spin until the time has come.
        nap.map_or(-1, |nap| nap.as_micros().div_ceil(1000) as i32)
    }
}

/// The slot a request is about.
fn slot_of(request: &Request) -> Option<u32> {
    match *request {
        Request::Load { slot, .. }
        | Request::Unload { slot }
        | Request::GetState { slot }
        | Request::SetState { slot }
        | Request::Presets { slot, .. }
        | Request::PresetState { slot, .. }
        | Request::SavePreset { slot, .. }
        | Request::Params { slot }
        | Request::GetParam { slot, .. }
        | Request::SetParam { slot, .. }
        | Request::Texts { slot, .. }
        | Request::ReadText { slot, .. }
        | Request::OpenEditor { slot, .. }
        | Request::CloseEditor { slot }
        | Request::Watch { slot, .. }
        | Request::SetOutputs { slot, .. }
        | Request::Capture { slot, .. }
        | Request::Captured { slot } => Some(slot),
        Request::Hello { .. } | Request::Quit => None,
    }
}

/// What has arrived on the socket and is not a whole frame yet.
#[derive(Default)]
struct Inbox {
    bytes: Vec<u8>,
    closed: bool,
}

impl Inbox {
    /// Takes what is there without waiting.
    fn fill(&mut self, socket: &UnixStream) -> Result<(), WireError> {
        let mut chunk = [0u8; 64 * 1024];
        loop {
            // SAFETY: `chunk` is writable for its whole length.
            let n = unsafe {
                libc::recv(
                    socket.as_raw_fd(),
                    chunk.as_mut_ptr().cast(),
                    chunk.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            match n {
                0 => {
                    self.closed = true;
                    return Ok(());
                }
                n if n > 0 => self.bytes.extend_from_slice(&chunk[..n as usize]),
                _ => {
                    let e = std::io::Error::last_os_error();
                    return match e.kind() {
                        std::io::ErrorKind::WouldBlock => Ok(()),
                        std::io::ErrorKind::Interrupted => continue,
                        _ => Err(WireError::Io(e)),
                    };
                }
            }
        }
    }

    /// The next whole frame, if all of it is here.
    fn next(&mut self) -> Result<Option<(ToHost, Vec<u8>)>, WireError> {
        let Some(head) = self.bytes.first_chunk::<8>() else {
            return Ok(None);
        };
        let text = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
        let blob = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        if text > MAX_TEXT_LEN || blob > MAX_BLOB_LEN {
            return Err(WireError::Malformed(format!("lengths {text} and {blob}")));
        }
        let len = 8 + text + blob;
        if self.bytes.len() < len {
            return Ok(None);
        }
        let frame = read_frame::<ToHost>(&mut &self.bytes[..len])?;
        self.bytes.drain(..len);
        Ok(Some(frame))
    }
}

/// Writes the answer to request `id`. One too long for a frame is answered with an error: a
/// frame that is too long is refused before any of it is written.
fn answer(to: &mut impl Write, id: u64, reply: Reply, blob: &[u8]) -> Result<(), WireError> {
    match write_frame(to, &FromHost::Reply { id, reply }, blob) {
        Err(WireError::Malformed(what)) => {
            let reply = Reply::Error {
                message: format!("the answer cannot be sent: {what}"),
            };
            write_frame(to, &FromHost::Reply { id, reply }, &[])
        }
        written => written,
    }
}

/// Writes what the plugins did by themselves since the last call.
fn tell(host: &mut Host, writer: &mut UnixStream) -> Result<(), WireError> {
    for notice in host.notices.drain(..) {
        write_frame(writer, &FromHost::Notice { notice }, &[])?;
    }
    Ok(())
}

/// Runs until meloet says `Quit` or the socket closes. The process's exit code.
pub fn run(control: UnixStream, memory: OwnedFd, offline: bool) -> i32 {
    let map = match Mapping::attach(memory) {
        Ok(map) => Arc::new(map),
        Err(e) => {
            eprintln!("meloet-plughost: {e}");
            return 2;
        }
    };
    let audio = match rt::start(map.clone(), offline) {
        Ok(audio) => audio,
        Err(e) => {
            eprintln!("meloet-plughost: audio thread: {e}");
            return 2;
        }
    };
    let slots = map.geometry().slots;
    let mut host = Host {
        slots: (0..slots).map(|_| None).collect(),
        map,
        audio,
        greeted: false,
        offline,
        display: None,
        tops: (0..slots).map(|_| None).collect(),
        asked: VecDeque::new(),
        told: HashMap::new(),
        holds: HashMap::new(),
        touches: vec![0; slots as usize],
        moved: (0..slots).map(|_| None).collect(),
        params_set: vec![None; slots as usize],
        state_put: vec![None; slots as usize],
        watched: Vec::new(),
        watch_told: Instant::now(),
        notices: Vec::new(),
        idled: Instant::now(),
        focus_looked: Instant::now(),
    };
    let mut writer = control;
    let mut inbox = Inbox::default();
    let mut said = Vec::new();
    let mut events = Vec::new();
    let mut polled: Vec<libc::pollfd> = Vec::new();
    let code = 'run: loop {
        polled.clear();
        let watch = |fd| libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        polled.push(watch(writer.as_raw_fd()));
        if let Some(display) = &host.display {
            polled.push(watch(display.fd()));
        }
        let own = polled.len();
        runloop::watched(&mut polled);
        // SAFETY: `polled` holds that many valid pollfds.
        unsafe {
            libc::poll(
                polled.as_mut_ptr(),
                polled.len() as libc::nfds_t,
                host.nap(),
            )
        };
        if polled[0].revents != 0
            && let Err(e) = inbox.fill(&writer)
        {
            eprintln!("meloet-plughost: {e}");
            break 2;
        }
        loop {
            let (message, blob) = match inbox.next() {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(e) => {
                    eprintln!("meloet-plughost: {e}");
                    break 'run 2;
                }
            };
            let quit = matches!(message.request, Request::Quit);
            let (reply, blob) = host.handle(message.request, blob);
            if let Err(e) = answer(&mut writer, message.id, reply, &blob) {
                eprintln!("meloet-plughost: {e}");
                break 'run 2;
            }
            if quit {
                break 'run 0;
            }
        }
        if inbox.closed {
            break 0;
        }
        host.open_asked();
        runloop::turn(&polled[own..]);
        host.window_events(&mut events);
        host.idle();
        host.look_after(&mut said);
        if let Err(e) = tell(&mut host, &mut writer) {
            eprintln!("meloet-plughost: {e}");
            break 2;
        }
    };
    for slot in 0..slots {
        host.close_window(slot);
    }
    // As when one plugin is unloaded: the audio thread lets go first. If it cannot (it is
    // stuck inside a plugin), no plugin is ended; the process is about to go anyway.
    let Host { audio, slots, .. } = host;
    if audio.stop() {
        drop(slots);
    } else {
        std::mem::forget(slots);
    }
    code
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use meloet_plugin::ipc::Geometry;
    use meloet_plugin::wire::ParamInfo;

    use super::*;
    use crate::formats::{Opened, Read};

    /// A plugin's main-thread half that says what it is given to say and counts the times the
    /// person was said to have left its window.
    #[derive(Default)]
    struct Told {
        says: Rc<RefCell<Vec<Said>>>,
        left: Rc<Cell<u32>>,
    }

    impl Controller for Told {
        fn params(&mut self) -> Vec<ParamInfo> {
            Vec::new()
        }
        fn param(&mut self, _id: u32) -> Option<ParamNow> {
            None
        }
        fn text(&mut self, _id: u32, _value: f64) -> Option<String> {
            None
        }
        fn read_text(&mut self, _id: u32, _text: &str) -> Option<Read> {
            None
        }
        fn set_param(&mut self, _id: u32, _value: f64) {}
        fn save_state(&mut self) -> Result<Vec<u8>, String> {
            Ok(Vec::new())
        }
        fn load_state(&mut self, _state: &[u8]) -> Result<(), String> {
            Ok(())
        }
        fn open_editor(&mut self, _parent: u32) -> Result<Opened, String> {
            Err("no window".into())
        }
        fn close_editor(&mut self) {}
        fn editor_left(&mut self) {
            self.left.set(self.left.get() + 1);
        }
        fn idle(&mut self) {}
        fn poll(&mut self, said: &mut Vec<Said>) {
            said.append(&mut self.says.borrow_mut());
        }
    }

    /// A host of two slots with no display, the first with a plugin whose window is open.
    fn host(controller: Told) -> Host {
        let map = Arc::new(
            Mapping::create(Geometry {
                max_frames: 64,
                channels: 2,
                slots: 2,
                to_plugin_events: 16,
                from_plugin_events: 16,
            })
            .unwrap(),
        );
        let audio = rt::start(map.clone(), false).unwrap();
        let top = || Top {
            id: 7,
            size: (320, 200),
            resizable: false,
            follows_child: false,
            inside: false,
            asked_to_show: None,
        };
        Host {
            map,
            audio,
            slots: vec![Some(Box::new(controller)), None],
            greeted: true,
            offline: false,
            display: None,
            tops: vec![Some(top()), None],
            asked: VecDeque::new(),
            told: HashMap::new(),
            holds: HashMap::new(),
            touches: vec![0; 2],
            moved: vec![None, None],
            params_set: vec![None; 2],
            state_put: vec![None; 2],
            watched: Vec::new(),
            watch_told: Instant::now(),
            notices: Vec::new(),
            idled: Instant::now(),
            focus_looked: Instant::now(),
        }
    }

    fn end(host: Host) {
        assert!(host.audio.stop());
    }

    #[test]
    fn the_value_held_back_is_told_when_the_window_closes() {
        let plugin = Told::default();
        let says = plugin.says.clone();
        let mut host = host(plugin);
        // Two moves within one look: the second is held back.
        says.borrow_mut().extend([
            Said::ParamChanged(5, 0.5, 10),
            Said::ParamChanged(5, 0.75, 20),
        ]);
        host.look_after(&mut Vec::new());
        host.notices.clear();
        host.close_window(0);
        let notices = std::mem::take(&mut host.notices);
        end(host);
        assert!(
            matches!(
                notices[..],
                [
                    Notice::ParamChanged {
                        slot: 0,
                        id: 5,
                        value: 0.75,
                        text: None,
                        t_us: 20
                    },
                    Notice::ParamEnd { slot: 0, id: 5, .. },
                ]
            ),
            "{notices:?}"
        );
    }

    #[test]
    fn closing_the_window_the_person_is_in_is_leaving_it() {
        let plugin = Told::default();
        let left = plugin.left.clone();
        let mut host = host(plugin);
        host.tops[0].as_mut().unwrap().inside = true;
        host.closed(0);
        let notices = std::mem::take(&mut host.notices);
        end(host);
        assert_eq!(
            notices,
            [
                Notice::EditorLeft {
                    slot: 0,
                    touches: 0
                },
                Notice::EditorClosed { slot: 0 }
            ]
        );
        assert_eq!(left.get(), 1);
    }

    #[test]
    fn when_the_display_is_lost_every_window_is_closed() {
        let plugin = Told::default();
        let left = plugin.left.clone();
        let mut host = host(plugin);
        host.tops[0].as_mut().unwrap().inside = true;
        host.display_lost();
        let notices = std::mem::take(&mut host.notices);
        let closed = host.tops.iter().all(Option::is_none) && host.display.is_none();
        end(host);
        assert_eq!(
            notices,
            [
                Notice::EditorLeft {
                    slot: 0,
                    touches: 0
                },
                Notice::EditorClosed { slot: 0 }
            ]
        );
        assert!(closed);
        assert_eq!(left.get(), 1);
    }

    #[test]
    fn an_answer_too_long_for_a_frame_is_an_error() {
        let mut written = Vec::new();
        answer(&mut written, 3, Reply::State, &[1, 2, 3]).unwrap();
        let blob = vec![0u8; MAX_BLOB_LEN + 1];
        answer(&mut written, 4, Reply::State, &blob).unwrap();
        let mut from = &written[..];
        let (first, blob) = read_frame::<FromHost>(&mut from).unwrap();
        assert_eq!(
            (first, blob),
            (
                FromHost::Reply {
                    id: 3,
                    reply: Reply::State
                },
                vec![1, 2, 3]
            )
        );
        let (second, blob) = read_frame::<FromHost>(&mut from).unwrap();
        assert!(
            matches!(
                second,
                FromHost::Reply {
                    id: 4,
                    reply: Reply::Error { .. }
                }
            ),
            "{second:?}"
        );
        assert!(blob.is_empty() && from.is_empty());
    }
}
