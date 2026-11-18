//! Audio engine: instrument interface, placeholder synth, mixer, meters, transport (song
//! position, loop, metronome, count-in, recording stamps), cpal output.
//!
//! The audio thread never allocates, never frees, never locks. Everything crossing the thread
//! boundary goes through `rtrb` rings or atomics.

mod clock;
mod effect;
mod engine;
mod instrument;
mod output;
pub mod rt;
mod synth;
mod transport;

pub use clock::{HeardPosition, TransportClock, TransportState};
pub use effect::{Effect, EffectContext};
pub use engine::{
    BlockTiming, Chord, Engine, EngineCommand, EngineConfig, EngineHandle, EngineMeters,
    EngineParts, FADE_S, LatencyRecord, MidiFeed, MidiIn, Recorded, monotonic_us,
};
pub use instrument::{Instrument, InstrumentEvent};
pub use output::{AudioOutput, OpenError, OutputTarget, RunningOutput};
pub use synth::PlaceholderSynth;
pub use transport::{Sequence, SequenceEvent, TrackSet};
