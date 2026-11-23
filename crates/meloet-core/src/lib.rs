//! The owner of the state: intents and device news come in, rev-numbered patches go out.
//!
//! - [`state`]: the pure rules. No threads, no devices.
//! - [`hub`]: the state the screens see, its rev, and the sinks, behind one lock.
//! - [`Core`]: the threads (MIDI, device watcher, audio, telemetry) around the two.
//! - [`plugins`]: the scan for plugins and the processes they sound in.

mod added;
pub mod ai;
pub mod audiofiles;
mod catalog;
pub mod export;
mod hub;
mod instruments;
pub mod layout;
pub mod made;
pub mod maker;
#[cfg(feature = "dev-hooks")]
pub mod measure;
mod mixbook;
pub mod pcm;
pub mod peaks;
pub mod play;
pub mod plugins;
pub mod presets;
pub mod readahead;
pub mod recent;
pub mod recovery;
#[cfg(test)]
mod seen;
mod service;
pub mod settings;
pub mod sf2store;
pub mod songfile;
mod sounds;
pub mod state;
pub mod updates;
pub mod vocalrec;

pub use hub::{MessageSink, TelemetrySink};
pub use service::{Core, CoreOptions, MadeCounts, ShellRequest, ShellSink};
