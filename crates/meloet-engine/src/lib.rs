//! Audio engine: instrument interface, placeholder synth, mixer, meters, cpal output.
//!
//! The audio thread never allocates, never frees, never locks. Everything crossing the thread
//! boundary goes through `rtrb` rings or atomics.

mod engine;
mod instrument;
mod output;
pub mod rt;
mod synth;

pub use engine::{
    BlockTiming, Engine, EngineCommand, EngineConfig, EngineHandle, EngineMeters, EngineParts,
    LatencyRecord, MidiIn, monotonic_us,
};
pub use instrument::{Instrument, InstrumentEvent};
pub use output::{AudioOutput, OpenError, OutputTarget, RunningOutput};
pub use synth::PlaceholderSynth;
