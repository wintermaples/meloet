//! The owner of the state: intents and device news come in, rev-numbered patches go out.
//!
//! - [`state`]: the pure rules. No threads, no devices.
//! - [`hub`]: the state the screens see, its rev, and the sinks, behind one lock.
//! - [`Core`]: the threads (MIDI, device watcher, audio, telemetry) around the two.
//! - [`plugins`]: the scan for plugins and the processes they sound in.

mod added;
mod catalog;
mod hub;
mod instruments;
pub mod layout;
pub mod measure;
mod mixbook;
pub mod play;
pub mod plugins;
pub mod recovery;
#[cfg(test)]
mod seen;
mod service;
pub mod settings;
pub mod sf2store;
pub mod songfile;
mod sounds;
pub mod state;
pub mod vocalrec;

pub use hub::{MessageSink, TelemetrySink};
pub use service::{Core, CoreOptions, ShellRequest, ShellSink};
