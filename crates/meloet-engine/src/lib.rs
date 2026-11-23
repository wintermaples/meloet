//! Audio engine: instrument and effect interfaces, placeholder synth, mixer (strips, sends,
//! returns, master), meters, transport (song
//! position, loop, metronome, count-in, recording stamps), cpal output.
//!
//! The audio thread never allocates, never frees, never locks. Everything crossing the thread
//! boundary goes through `rtrb` rings or atomics.

mod analysis;
mod automation;
mod clipplay;
mod clips;
mod clock;
mod effect;
mod engine;
mod input;
mod instrument;
mod mixer;
mod output;
mod plugin;
mod priority;
mod probe;
pub mod rt;
mod stream;
mod synth;
mod transport;

pub use analysis::{ANALYSIS_RING_FRAMES, AnalysisReader};
pub use automation::{AutoLane, AutoSet, EFFECT_STEP, HOLDS, MIX_STEP, PLUGIN_POINTS, place_at};
pub use clips::{ClipRegion, ClipTable, song_frame, song_frame_at};
pub use clock::{HeardPosition, TransportClock, TransportState};
pub use effect::{Effect, EffectContext};
pub use engine::{
    BlockTiming, Chord, Engine, EngineCommand, EngineConfig, EngineHandle, EngineMeters,
    EngineParts, FADE_S, LatencyRecord, MidiFeed, MidiIn, ProbeSent, Recorded, Voice, monotonic_us,
};
pub use input::{
    AudioInput, InputBlock, InputEnds, InputError, InputFeed, InputMeters, InputRingReader,
    InputRingWriter, InputTap, InputTarget, MAX_INPUT_CHANNELS, MONITOR_RING_SECONDS,
    RECORD_RING_SECONDS, RunningInput, input_ends, input_ring,
};
pub use instrument::{Instrument, InstrumentEvent, SongPlace};
pub use mixer::{INSERT_SLOTS, MixStrip, RAMP_S, pan_gains};
pub use output::{AudioOutput, OpenError, OutputTarget, RunningOutput};
pub use plugin::{
    PLUGIN_WAIT_SHARE, PluginBlock, PluginEffect, PluginInstrument, PluginSlot, PluginWatch,
};
pub use probe::{
    FrameClock, PROBE_LISTEN_S, PROBE_PEAK, ProbeFound, ProbeHeard, ProbeVerdict, find_probe,
    judge_probes, probe_wave,
};
pub use stream::{
    STREAM_CHANNELS, STREAM_REFILL_SECONDS, STREAM_RING_SECONDS, StreamChunk, StreamLayout,
    StreamRead, StreamReader, StreamWant, StreamWants, StreamWriter, stream_ring, want_ring,
};
pub use synth::PlaceholderSynth;
pub use transport::{ClickSound, Sequence, SequenceEvent, TrackSet};
