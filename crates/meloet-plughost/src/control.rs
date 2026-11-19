//! The main thread in `host` mode: answers the requests on the control socket
//! (`meloet_plugin::wire`) and owns the controller half of every plugin.
//!
//! The process ends when the socket closes: meloet is gone, or it let this process go.
//!
//! The thread sleeps in `poll`, never in a read: on the socket, and for no longer than a tick
//! while a plugin is loaded, so that every plugin is looked after many times a second
//! ([`Controller::poll`]). What a plugin's window needs later (the X11 connection, the
//! descriptors and timers a VST3 view registers, the idle call of an LV2 UI) joins the same
//! `poll`.

use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use meloet_plugin::ipc::{Mapping, PluginEnd};
use meloet_plugin::wire::{
    FromHost, MAX_BLOB_LEN, MAX_TEXT_LEN, Notice, Reply, Request, ToHost, WIRE_VERSION, WireError,
    read_frame, write_frame,
};

use crate::formats::{self, Activation, Controller, Said};
use crate::rt::{self, SlotRun};

/// How long the thread sleeps at most while a plugin is loaded.
const TICK_MS: i32 = 10;

struct Host {
    map: Arc<Mapping>,
    audio: rt::Handle,
    /// The controller of the plugin in each slot.
    slots: Vec<Option<Box<dyn Controller>>>,
    greeted: bool,
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
        };
        let loaded = match formats::load(key, bundle, activation) {
            Ok(loaded) => loaded,
            Err(message) => return error(message),
        };
        let end = match PluginEnd::new(self.map.clone(), slot) {
            Ok(end) => end,
            Err(e) => return error(e.to_string()),
        };
        let run = SlotRun::new(&self.map, end, loaded.processor, loaded.plugin.latency);
        if let Err(message) = self.audio.install(run) {
            return error(message);
        }
        self.slots[slot as usize] = Some(loaded.controller);
        (
            Reply::Loaded {
                plugin: loaded.plugin,
            },
            Vec::new(),
        )
    }

    fn handle(&mut self, request: Request, blob: Vec<u8>) -> Answer {
        if !self.greeted && !matches!(request, Request::Hello { .. }) {
            return error("hello comes first");
        }
        match request {
            Request::Hello { version } => {
                self.greeted = version == WIRE_VERSION;
                (
                    Reply::Hello {
                        version: WIRE_VERSION,
                    },
                    Vec::new(),
                )
            }
            Request::Load { slot, .. } => self.load(slot, &request),
            Request::Unload { slot } => {
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
            Request::GetState { slot } => match self.controller(slot).map(|c| c.save_state()) {
                Ok(Ok(state)) => (Reply::State, state),
                Ok(Err(message)) => error(message),
                Err(answer) => answer,
            },
            Request::SetState { slot } => {
                if let Err(message) = self.audio.forget_params(slot) {
                    return error(message);
                }
                match self.controller(slot).map(|c| c.load_state(&blob)) {
                    Ok(Ok(())) => ok(),
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
                Ok(Some((value, text))) => (Reply::Param { value, text }, Vec::new()),
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
                match self.audio.set_param(slot, id, value) {
                    Ok(()) => ok(),
                    Err(message) => error(message),
                }
            }
            Request::OpenEditor { .. } | Request::CloseEditor { .. } => error("no windows yet"),
            Request::Quit => ok(),
        }
    }
}

/// Looks after every plugin and tells meloet what they did by themselves.
fn look_after(
    host: &mut Host,
    said: &mut Vec<Said>,
    writer: &mut UnixStream,
) -> Result<(), WireError> {
    for slot in 0..host.slots.len() {
        let Some(controller) = host.slots[slot].as_mut() else {
            continue;
        };
        said.clear();
        controller.poll(said);
        let slot = slot as u32;
        for &event in said.iter() {
            let notice = match event {
                Said::ParamBegin(id) => Notice::ParamBegin { slot, id },
                Said::ParamChanged(id, value) => {
                    // What the person did in the plugin's window reaches the plugin's sound
                    // with the next block.
                    let _ = host.audio.set_param(slot, id, value);
                    Notice::ParamChanged { slot, id, value }
                }
                Said::ParamEnd(id) => Notice::ParamEnd { slot, id },
                Said::Latency(frames) => Notice::LatencyChanged { slot, frames },
                Said::ParamsChanged => Notice::ParamsChanged { slot },
            };
            write_frame(writer, &FromHost::Notice { notice }, &[])?;
        }
    }
    Ok(())
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

/// Runs until meloet says `Quit` or the socket closes. The process's exit code.
pub fn run(control: UnixStream, memory: OwnedFd) -> i32 {
    let map = match Mapping::attach(memory) {
        Ok(map) => Arc::new(map),
        Err(e) => {
            eprintln!("meloet-plughost: {e}");
            return 2;
        }
    };
    let audio = match rt::start(map.clone()) {
        Ok(audio) => audio,
        Err(e) => {
            eprintln!("meloet-plughost: audio thread: {e}");
            return 2;
        }
    };
    let mut host = Host {
        slots: (0..map.geometry().slots).map(|_| None).collect(),
        map,
        audio,
        greeted: false,
    };
    let mut writer = control;
    let mut inbox = Inbox::default();
    let mut said = Vec::new();
    let code = 'run: loop {
        let mut poll = libc::pollfd {
            fd: writer.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let tick = if host.slots.iter().any(Option::is_some) {
            TICK_MS
        } else {
            -1
        };
        // SAFETY: one valid pollfd.
        unsafe { libc::poll(&mut poll, 1, tick) };
        if poll.revents != 0
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
            let answer = FromHost::Reply {
                id: message.id,
                reply,
            };
            if let Err(e) = write_frame(&mut writer, &answer, &blob) {
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
        if let Err(e) = look_after(&mut host, &mut said, &mut writer) {
            eprintln!("meloet-plughost: {e}");
            break 2;
        }
    };
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
