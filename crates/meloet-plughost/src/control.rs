//! The main thread in `host` mode: answers the requests on the control socket
//! (`meloet_plugin::wire`) and owns the controller half of every plugin.
//!
//! The process ends when the socket closes: meloet is gone, or it let this process go.

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use meloet_plugin::ipc::{Mapping, PluginEnd};
use meloet_plugin::wire::{
    FromHost, Reply, Request, ToHost, WIRE_VERSION, WireError, read_frame, write_frame,
};

use crate::formats::{self, Activation, Controller};
use crate::rt::{self, SlotRun};

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
                let removed = self.audio.remove(slot);
                drop(controller);
                match removed {
                    Ok(()) => ok(),
                    Err(message) => error(message),
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
            // The windows and the scan inside this process come with the formats.
            Request::OpenEditor { .. } | Request::CloseEditor { .. } => error("no windows yet"),
            Request::Scan { .. } => error("no scan in a host process yet"),
            Request::Quit => ok(),
        }
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
    let (mut reader, mut writer) = match control.try_clone() {
        Ok(reader) => (reader, control),
        Err(e) => {
            eprintln!("meloet-plughost: control socket: {e}");
            return 2;
        }
    };
    let code = loop {
        let (message, blob) = match read_frame::<ToHost>(&mut reader) {
            Ok(frame) => frame,
            Err(WireError::Closed) => break 0,
            Err(e) => {
                eprintln!("meloet-plughost: {e}");
                break 2;
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
            break 2;
        }
        if quit {
            break 0;
        }
    };
    // As when one plugin is unloaded: the audio thread lets go first.
    host.audio.stop();
    host.slots.clear();
    code
}
