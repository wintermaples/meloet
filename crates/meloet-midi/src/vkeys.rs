//! A virtual keyboard: a sequencer client with one readable port, and a second one for buttons
//! when asked. For tests and measurements, used by the `meloet-vkeys` tool. Not part of the
//! product's own behavior.

use std::ffi::CString;

use alsa::Direction;
use alsa::seq::{EvCtrl, EvNote, Event, EventType, PortCap, PortType, Seq};

use crate::{MidiError, MidiEvent, monotonic_us};

pub struct VirtualKeyboard {
    seq: Seq,
    client: i32,
    port: i32,
    buttons: Option<i32>,
}

impl VirtualKeyboard {
    /// Appears as `client_name` with one port of the same name. Dropping it makes it disappear.
    pub fn open(client_name: &str) -> Result<Self, MidiError> {
        Self::open_as(client_name, false)
    }

    /// [`VirtualKeyboard::open`] with a second readable port, "`client_name` buttons", which
    /// meloet hears as a button port (Mackie Control notes, [`VirtualKeyboard::press_button`]).
    pub fn open_with_buttons(client_name: &str) -> Result<Self, MidiError> {
        Self::open_as(client_name, true)
    }

    fn open_as(client_name: &str, with_buttons: bool) -> Result<Self, MidiError> {
        let name = CString::new(client_name)
            .map_err(|e| MidiError::io("read the client name")(e.into()))?;
        let seq = Seq::open(None, Some(Direction::Playback), false)
            .map_err(MidiError::alsa("open the ALSA sequencer"))?;
        seq.set_client_name(&name)
            .map_err(MidiError::alsa("name the sequencer client"))?;
        let client = seq
            .client_id()
            .map_err(MidiError::alsa("read the sequencer client id"))?;
        let port = seq
            .create_simple_port(
                &name,
                PortCap::READ | PortCap::SUBS_READ,
                PortType::MIDI_GENERIC | PortType::APPLICATION,
            )
            .map_err(MidiError::alsa("create the output port"))?;
        let buttons = if with_buttons {
            let name = CString::new(format!("{client_name} buttons"))
                .map_err(|e| MidiError::io("read the client name")(e.into()))?;
            Some(
                seq.create_simple_port(
                    &name,
                    PortCap::READ | PortCap::SUBS_READ,
                    PortType::MIDI_GENERIC | PortType::APPLICATION,
                )
                .map_err(MidiError::alsa("create the button port"))?,
            )
        } else {
            None
        };
        Ok(Self {
            seq,
            client,
            port,
            buttons,
        })
    }

    /// The ALSA address, e.g. "129:0".
    pub fn address(&self) -> String {
        format!("{}:{}", self.client, self.port)
    }

    /// Sends to whoever is subscribed (nobody is fine) and returns the time just before sending.
    /// A `NoteOn` with velocity 0 goes out as it is, like a real keyboard would send it.
    pub fn send(&self, event: MidiEvent) -> Result<u64, MidiError> {
        self.send_from(self.port, event)
    }

    /// Sends `event` from the button port. Errors when the keyboard was opened without one.
    pub fn send_button_port(&self, event: MidiEvent) -> Result<u64, MidiError> {
        let port = self.buttons.ok_or(MidiError::Io {
            doing: "send from the button port",
            source: std::io::Error::other("the virtual keyboard has no button port"),
        })?;
        self.send_from(port, event)
    }

    /// Presses and lets go of the Mackie Control button of `note` (94 is play) on the button
    /// port, as a real keyboard does: note-on with velocity 127, then note-off.
    pub fn press_button(&self, note: u8) -> Result<u64, MidiError> {
        let t_us = self.send_button_port(MidiEvent::NoteOn {
            note,
            velocity: 127,
        })?;
        self.send_button_port(MidiEvent::NoteOff { note, velocity: 0 })?;
        Ok(t_us)
    }

    fn send_from(&self, port: i32, event: MidiEvent) -> Result<u64, MidiError> {
        let note = |note, velocity| EvNote {
            channel: 0,
            note,
            velocity,
            off_velocity: 0,
            duration: 0,
        };
        let mut ev = match event {
            MidiEvent::NoteOn { note: n, velocity } => {
                Event::new(EventType::Noteon, &note(n, velocity))
            }
            MidiEvent::NoteOff { note: n, velocity } => {
                Event::new(EventType::Noteoff, &note(n, velocity))
            }
            MidiEvent::Control { controller, value } => Event::new(
                EventType::Controller,
                &EvCtrl {
                    channel: 0,
                    param: controller.into(),
                    value: value.into(),
                },
            ),
            MidiEvent::PitchBend { value } => Event::new(
                EventType::Pitchbend,
                &EvCtrl {
                    channel: 0,
                    param: 0,
                    value: value.into(),
                },
            ),
        };
        ev.set_source(port);
        ev.set_subs();
        ev.set_direct();
        let t_us = monotonic_us();
        self.seq
            .event_output_direct(&mut ev)
            .map_err(MidiError::alsa("send the event"))?;
        Ok(t_us)
    }
}
