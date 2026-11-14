//! The owner of the state: intents and device news come in, rev-numbered patches go out.
//!
//! - [`state`]: the pure rules. No threads, no devices.
//! - [`hub`]: the state the screens see, its rev, and the sinks, behind one lock.
//! - [`Core`]: the threads (MIDI, device watcher, audio, telemetry) around the two.

mod hub;
pub mod layout;
pub mod measure;
mod service;
pub mod settings;
pub mod state;

pub use hub::{MessageSink, TelemetrySink};
pub use service::{Core, CoreOptions};
