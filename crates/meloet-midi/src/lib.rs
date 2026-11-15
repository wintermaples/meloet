//! Keyboard input and hot-plug for meloet. On Linux this is the ALSA sequencer.
//!
//! [`MidiInput::start`] owns one thread. Everything it learns goes to the sink as a
//! [`MidiMessage`], in the order it happened.
//!
//! # The sink contract
//!
//! The sink runs on the MIDI thread, once per message. While it runs, no other key is read, so
//! it must return fast: push to a wait-free ring, send on a channel, store to an atomic. No file
//! or network work, no lock that another thread may hold for long. A panic in the sink ends the
//! MIDI thread without a [`MidiMessage::Stopped`].
//!
//! When the thread ends on its own (the sequencer gave an error), the last message is
//! [`MidiMessage::Stopped`]. [`MidiInput::into_sink`] then gives the sink back for a new
//! [`MidiInput::start_boxed`].

mod input;
mod rules;
pub mod vkeys;

use std::fmt;

pub use input::{MidiInput, MidiSink, list_ports};

/// Client name prefix of the virtual test keyboard. Clients whose name starts with this count as
/// keyboards only when [`MidiOptions::accept_virtual`] is set.
pub const VKEYS_CLIENT_NAME: &str = "meloet-vkeys";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiEvent {
    NoteOn {
        note: u8,
        velocity: u8,
    },
    NoteOff {
        note: u8,
        velocity: u8,
    },
    Control {
        controller: u8,
        value: u8,
    },
    /// −8192..=8191, 0 is the centre.
    PitchBend {
        value: i16,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyboard {
    /// Made from the client name and the port name, so it stays the same when the keyboard is
    /// plugged in again (the ALSA address does not). A second keyboard with the same names gets
    /// `#2`, `#3`, … in the order it was seen.
    pub id: String,
    /// The ALSA client name, e.g. "Keystation 49 MK3". This is the name shown to people.
    pub name: String,
    pub port_name: String,
    pub client: i32,
    pub port: i32,
}

impl Keyboard {
    /// The ALSA address, e.g. "28:0".
    pub fn address(&self) -> String {
        format!("{}:{}", self.client, self.port)
    }

    /// "client name:port name client:port", the form shown as the MIDI input in the settings.
    pub fn label(&self) -> String {
        format!("{}:{} {}", self.name, self.port_name, self.address())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MidiMessage {
    Event {
        /// [`Keyboard::id`] of the sender.
        keyboard: String,
        event: MidiEvent,
        /// CLOCK_MONOTONIC in microseconds, taken when the event was read.
        t_us: u64,
    },
    Added(Keyboard),
    Removed {
        id: String,
    },
    /// Sent once: every keyboard that was already there at start has been reported as `Added`.
    Ready,
    /// The thread ended on its own; nothing follows. Keys and pedals held at this point get no
    /// release. Not sent when the input is stopped on purpose.
    Stopped {
        reason: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MidiOptions {
    /// Also treat `meloet-vkeys*` clients as keyboards (for tests and measurements).
    pub accept_virtual: bool,
    /// Treat only `meloet-vkeys*` clients as keyboards and leave real ones out (to show the
    /// screens without a keyboard while one is plugged in).
    pub only_virtual: bool,
    /// When not empty, a virtual client counts only when its name is one of these, so that the
    /// virtual keyboards of other tests running on the same PC stay out.
    pub virtual_names: Vec<String>,
    /// For tests of what follows a failure: after this many key events the thread ends as if
    /// the sequencer had given an error, and sends [`MidiMessage::Stopped`].
    pub fail_after_events: Option<u32>,
}

impl MidiOptions {
    /// `accept_virtual` is on when the environment has `MELOET_ACCEPT_VIRTUAL=1`,
    /// `only_virtual` when it has `MELOET_ONLY_VIRTUAL=1`, `virtual_names` is
    /// `MELOET_VKEYS_NAME=<name>[,<name>…]`, and `fail_after_events` is
    /// `MELOET_MIDI_FAIL_AFTER=<events>` (a development aid).
    pub fn from_env() -> Self {
        let on = |name: &str| std::env::var_os(name).is_some_and(|v| v == "1");
        Self {
            accept_virtual: on("MELOET_ACCEPT_VIRTUAL"),
            only_virtual: on("MELOET_ONLY_VIRTUAL"),
            virtual_names: std::env::var("MELOET_VKEYS_NAME")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(String::from)
                .collect(),
            fail_after_events: std::env::var("MELOET_MIDI_FAIL_AFTER")
                .ok()
                .and_then(|v| v.trim().parse().ok()),
        }
    }
}

#[derive(Debug)]
pub enum MidiError {
    Alsa {
        doing: &'static str,
        source: alsa::Error,
    },
    Io {
        doing: &'static str,
        source: std::io::Error,
    },
}

impl MidiError {
    pub(crate) fn alsa(doing: &'static str) -> impl FnOnce(alsa::Error) -> Self {
        move |source| Self::Alsa { doing, source }
    }

    pub(crate) fn io(doing: &'static str) -> impl FnOnce(std::io::Error) -> Self {
        move |source| Self::Io { doing, source }
    }
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alsa { doing, source } => write!(f, "{doing}: {source}"),
            Self::Io { doing, source } => write!(f, "{doing}: {source}"),
        }
    }
}

impl std::error::Error for MidiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Alsa { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
        }
    }
}

/// CLOCK_MONOTONIC in microseconds. The same clock the audio side uses for its timestamps.
pub fn monotonic_us() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid timespec to write into. CLOCK_MONOTONIC always exists on Linux, so
    // the call cannot fail.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1_000
}
