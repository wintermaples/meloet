use std::io::{PipeReader, PipeWriter, Write};
use std::os::fd::AsRawFd;
use std::thread::JoinHandle;

use alsa::Direction;
use alsa::poll::{Descriptors, pollfd};
use alsa::seq::{Addr, ClientIter, EventType, PortCap, PortIter, PortSubscribe, PortType, Seq};

use crate::rules::{PortFacts, button_of, convert, keyboard_id, keyboard_ports};
use crate::{Keyboard, MidiError, MidiEvent, MidiMessage, MidiOptions, monotonic_us};

/// The sink in the form [`MidiInput::into_sink`] gives back.
pub type MidiSink = Box<dyn FnMut(MidiMessage) + Send>;

/// The keyboard input. Dropping it stops the thread, the same as [`MidiInput::stop`].
pub struct MidiInput {
    wake: Option<PipeWriter>,
    thread: Option<JoinHandle<MidiSink>>,
}

impl MidiInput {
    /// Joins the sequencer as "meloet" and starts the MIDI thread.
    ///
    /// The sink first gets one `Added` for every keyboard that is already there, then `Ready`,
    /// then events and plug changes as they happen. See the crate docs for what the sink may do.
    pub fn start(
        options: MidiOptions,
        sink: impl FnMut(MidiMessage) + Send + 'static,
    ) -> Result<MidiInput, MidiError> {
        Self::start_boxed(options, Box::new(sink)).map_err(|(e, _)| e)
    }

    /// [`MidiInput::start`] with a boxed sink. When the sequencer cannot be opened the sink
    /// comes back with the error; when the thread cannot be started it is gone.
    pub fn start_boxed(
        options: MidiOptions,
        sink: MidiSink,
    ) -> Result<MidiInput, (MidiError, Option<MidiSink>)> {
        let (parts, fds, wake_rx, wake_tx) = match Self::open(options) {
            Ok(opened) => opened,
            Err(e) => return Err((e, Some(sink))),
        };
        let listener = Listener {
            seq: parts.seq,
            me: parts.me,
            dest: parts.dest,
            options: parts.options,
            active: parts.active,
            events: parts.events,
            sink,
        };
        std::thread::Builder::new()
            .name("meloet-midi".into())
            .spawn(move || listener.run(fds, wake_rx))
            .map(|thread| MidiInput {
                wake: Some(wake_tx),
                thread: Some(thread),
            })
            .map_err(|e| (MidiError::io("start the MIDI thread")(e), None))
    }

    #[allow(clippy::type_complexity)]
    fn open(
        options: MidiOptions,
    ) -> Result<(Listener<()>, Vec<pollfd>, PipeReader, PipeWriter), MidiError> {
        // Non-blocking, so that the thread sleeps in poll() together with the wake pipe.
        let seq =
            Seq::open(None, None, true).map_err(MidiError::alsa("open the ALSA sequencer"))?;
        seq.set_client_name(c"meloet")
            .map_err(MidiError::alsa("name the sequencer client"))?;
        let me = seq
            .client_id()
            .map_err(MidiError::alsa("read the sequencer client id"))?;
        let port = seq
            .create_simple_port(
                c"in",
                PortCap::WRITE | PortCap::SUBS_WRITE,
                PortType::MIDI_GENERIC | PortType::APPLICATION,
            )
            .map_err(MidiError::alsa("create the input port"))?;
        let dest = Addr { client: me, port };
        // Subscribe before counting: a keyboard plugged in while we count is then still announced.
        subscribe(&seq, Addr::system_announce(), dest)
            .map_err(MidiError::alsa("subscribe to System:Announce"))?;
        let mut fds = (&seq, Some(Direction::Capture))
            .get()
            .map_err(MidiError::alsa("read the sequencer poll descriptors"))?;
        let (wake_rx, wake_tx) = std::io::pipe().map_err(MidiError::io("create the wake pipe"))?;
        fds.push(pollfd {
            fd: wake_rx.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });

        let listener = Listener {
            seq,
            me,
            dest,
            options,
            active: Vec::new(),
            events: 0,
            sink: (),
        };
        Ok((listener, fds, wake_rx, wake_tx))
    }

    /// Wakes the thread, lets it end, and waits for it. The sink is not called after this returns.
    pub fn stop(self) {}

    /// Stops the thread if it still runs, waits for it, and gives the sink back. `None` if the
    /// sink panicked.
    pub fn into_sink(mut self) -> Option<MidiSink> {
        self.wake_thread();
        self.thread.take()?.join().ok()
    }

    fn wake_thread(&mut self) {
        if let Some(mut wake) = self.wake.take() {
            // If the write fails, closing the pipe wakes the thread just the same (POLLHUP).
            let _ = wake.write_all(&[1]);
        }
    }
}

impl Drop for MidiInput {
    fn drop(&mut self) {
        self.wake_thread();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The keyboard ports that exist right now, in client order.
pub fn list_ports(options: MidiOptions) -> Result<Vec<Keyboard>, MidiError> {
    let seq = Seq::open(None, None, false).map_err(MidiError::alsa("open the ALSA sequencer"))?;
    let me = seq
        .client_id()
        .map_err(MidiError::alsa("read the sequencer client id"))?;
    let mut found: Vec<Keyboard> = Vec::new();
    let clients: Vec<i32> = ClientIter::new(&seq).map(|c| c.get_client()).collect();
    for client in clients {
        if let Some(found_one) = find_keyboard(&seq, client, me, &options, &found) {
            found.push(found_one.keyboard);
        }
    }
    Ok(found)
}

fn subscribe(seq: &Seq, sender: Addr, dest: Addr) -> alsa::Result<()> {
    let s = PortSubscribe::empty()?;
    s.set_sender(sender);
    s.set_dest(dest);
    seq.subscribe_port(&s)
}

/// A keyboard and the ports it has besides its key port.
#[derive(Clone)]
struct Active {
    keyboard: Keyboard,
    buttons: Vec<i32>,
}

impl Active {
    fn ports(&self) -> impl Iterator<Item = Addr> + '_ {
        let client = self.keyboard.client;
        std::iter::once(self.keyboard.port)
            .chain(self.buttons.iter().copied())
            .map(move |port| Addr { client, port })
    }
}

/// The keyboard of one client, if it is one. `others` are the keyboards whose ids are taken.
fn find_keyboard(
    seq: &Seq,
    client: i32,
    me: i32,
    options: &MidiOptions,
    others: &[Keyboard],
) -> Option<Active> {
    if client == me || client == Addr::system_announce().client {
        return None;
    }
    let info = seq.get_any_client_info(client).ok()?;
    let name = info.get_name().ok()?.trim_end();
    let ports: Vec<_> = PortIter::new(seq, client).collect();
    let facts: Vec<_> = ports
        .iter()
        .map(|p| PortFacts {
            port: p.get_port(),
            cap: p.get_capability(),
            kind: p.get_type(),
        })
        .collect();
    let found = keyboard_ports(name, &facts, options)?;
    let port = found.keys;
    let port_name = ports
        .iter()
        .find(|p| p.get_port() == port)?
        .get_name()
        .ok()?
        .trim_end();
    let id = keyboard_id(name, port_name, |id| others.iter().any(|k| k.id == id));
    Some(Active {
        keyboard: Keyboard {
            id,
            name: name.to_string(),
            port_name: port_name.to_string(),
            client,
            port,
        },
        buttons: found.buttons,
    })
}

enum Incoming {
    Midi {
        source: Addr,
        event: MidiEvent,
        t_us: u64,
    },
    /// Something about this client's ports changed.
    Client(i32),
    Other,
}

struct Listener<S> {
    seq: Seq,
    me: i32,
    dest: Addr,
    options: MidiOptions,
    active: Vec<Active>,
    /// Key events read so far, for [`MidiOptions::fail_after_events`].
    events: u32,
    sink: S,
}

impl Listener<MidiSink> {
    fn run(mut self, fds: Vec<pollfd>, wake: PipeReader) -> MidiSink {
        if let Err(reason) = self.listen(fds, wake) {
            (self.sink)(MidiMessage::Stopped { reason });
        }
        self.sink
    }
}

impl<S: FnMut(MidiMessage)> Listener<S> {
    /// `Ok` when woken to stop, `Err` with the reason when the sequencer failed.
    fn listen(&mut self, mut fds: Vec<pollfd>, _wake: PipeReader) -> Result<(), String> {
        self.sync_all();
        (self.sink)(MidiMessage::Ready);
        let wake = fds.len() - 1;
        loop {
            match alsa::poll::poll(&mut fds, -1) {
                Ok(_) => {}
                Err(e) if e.errno() == libc::EINTR => continue,
                Err(e) => return Err(format!("poll the sequencer: {e}")),
            }
            if fds[wake].revents != 0 {
                return Ok(());
            }
            loop {
                match self.read() {
                    Ok(Incoming::Midi {
                        source,
                        event,
                        t_us,
                    }) => {
                        let Some(a) = self
                            .active
                            .iter()
                            .find(|a| a.keyboard.client == source.client)
                        else {
                            continue;
                        };
                        if a.keyboard.port == source.port {
                            (self.sink)(MidiMessage::Event {
                                keyboard: a.keyboard.id.clone(),
                                event,
                                t_us,
                            });
                            self.events = self.events.saturating_add(1);
                            if self.options.fail_after_events == Some(self.events) {
                                return Err("failure injected for a test".into());
                            }
                        } else if a.buttons.contains(&source.port)
                            && let Some(button) = button_of(event)
                        {
                            (self.sink)(MidiMessage::Button {
                                keyboard: a.keyboard.id.clone(),
                                button,
                            });
                        }
                    }
                    Ok(Incoming::Client(client)) => self.sync_client(client),
                    Ok(Incoming::Other) => {}
                    Err(e) if e.errno() == libc::EAGAIN => break,
                    // The kernel dropped events because our queue was full. A plug or unplug may
                    // be among them, so count everything again.
                    Err(e) if e.errno() == libc::ENOSPC => self.sync_all(),
                    Err(e) => return Err(format!("read the sequencer: {e}")),
                }
            }
        }
    }

    fn read(&self) -> alsa::Result<Incoming> {
        let mut input = self.seq.input();
        let event = input.event_input()?;
        let t_us = monotonic_us();
        Ok(match event.get_type() {
            EventType::PortStart
            | EventType::PortExit
            | EventType::PortChange
            | EventType::ClientExit => event
                .get_data::<Addr>()
                .map_or(Incoming::Other, |a| Incoming::Client(a.client)),
            _ => convert(&event).map_or(Incoming::Other, |e| Incoming::Midi {
                source: event.get_source(),
                event: e,
                t_us,
            }),
        })
    }

    fn sync_all(&mut self) {
        let mut clients: Vec<i32> = self.active.iter().map(|a| a.keyboard.client).collect();
        clients.extend(ClientIter::new(&self.seq).map(|c| c.get_client()));
        clients.sort_unstable();
        clients.dedup();
        for client in clients {
            self.sync_client(client);
        }
    }

    /// Makes `active` match what the client looks like now. Safe to call any number of times, so
    /// a port that is both counted at start and announced right after is reported once.
    fn sync_client(&mut self, client: i32) {
        let have = self.active.iter().position(|a| a.keyboard.client == client);
        let others: Vec<Keyboard> = self
            .active
            .iter()
            .filter(|a| a.keyboard.client != client)
            .map(|a| a.keyboard.clone())
            .collect();
        let want = find_keyboard(&self.seq, client, self.me, &self.options, &others);
        if let (Some(i), Some(w)) = (have, &want) {
            let k = &self.active[i].keyboard;
            if k.port == w.keyboard.port
                && k.name == w.keyboard.name
                && k.port_name == w.keyboard.port_name
            {
                if self.active[i].buttons != w.buttons {
                    // Only a button port came or went: the keyboard stays as it is.
                    let old = std::mem::replace(&mut self.active[i], w.clone());
                    for port in old.buttons.iter().filter(|p| !w.buttons.contains(p)) {
                        let gone = Addr {
                            client,
                            port: *port,
                        };
                        let _ = self.seq.unsubscribe_port(gone, self.dest);
                    }
                    self.subscribe_buttons(i, &old.buttons);
                }
                return;
            }
        }
        if let Some(i) = have {
            let old = self.active.remove(i);
            for port in old.ports() {
                // Fails when the port is already gone, which is the usual case.
                let _ = self.seq.unsubscribe_port(port, self.dest);
            }
            (self.sink)(MidiMessage::Removed {
                id: old.keyboard.id,
            });
        }
        if let Some(new) = want
            && subscribe(
                &self.seq,
                Addr {
                    client: new.keyboard.client,
                    port: new.keyboard.port,
                },
                self.dest,
            )
            .is_ok()
        {
            let keyboard = new.keyboard.clone();
            self.active.push(new);
            self.subscribe_buttons(self.active.len() - 1, &[]);
            (self.sink)(MidiMessage::Added(keyboard));
        }
    }

    /// Subscribes to the button ports of `active[i]` that are not in `already`. A port that
    /// cannot be subscribed to is left out; the keys work without it.
    fn subscribe_buttons(&mut self, i: usize, already: &[i32]) {
        let (seq, dest) = (&self.seq, self.dest);
        let client = self.active[i].keyboard.client;
        self.active[i].buttons.retain(|port| {
            let addr = Addr {
                client,
                port: *port,
            };
            already.contains(port) || subscribe(seq, addr, dest).is_ok()
        });
    }
}
