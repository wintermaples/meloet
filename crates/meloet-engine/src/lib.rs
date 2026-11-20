//! Audio engine: instrument and effect interfaces, placeholder synth, mixer (strips, sends,
//! returns, master), meters, transport (song
//! position, loop, metronome, count-in, recording stamps), cpal output.
//!
//! The audio thread never allocates, never frees, never locks. Everything crossing the thread
//! boundary goes through `rtrb` rings or atomics.

mod automation;
mod clock;
mod effect;
mod engine;
mod instrument;
mod mixer;
mod output;
mod plugin;
pub mod rt;
mod synth;
mod transport;

pub use automation::{AutoLane, AutoSet, EFFECT_STEP, HOLDS, MIX_STEP, PLUGIN_POINTS, place_at};
pub use clock::{HeardPosition, TransportClock, TransportState};
pub use effect::{Effect, EffectContext};
pub use engine::{
    BlockTiming, Chord, Engine, EngineCommand, EngineConfig, EngineHandle, EngineMeters,
    EngineParts, FADE_S, LatencyRecord, MidiFeed, MidiIn, Recorded, monotonic_us,
};
pub use instrument::{Instrument, InstrumentEvent};
pub use mixer::{INSERT_SLOTS, MixStrip, RAMP_S, pan_gains};
pub use output::{AudioOutput, OpenError, OutputTarget, RunningOutput};
pub use plugin::{
    PLUGIN_WAIT_SHARE, PluginBlock, PluginEffect, PluginInstrument, PluginSlot, PluginWatch,
};
pub use synth::PlaceholderSynth;
pub use transport::{Sequence, SequenceEvent, TrackSet};
