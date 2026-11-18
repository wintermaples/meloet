//! The instruments made by computation: synth lead, synth bass, warm pad, sine and the electro kit
//! (docs/architecture.md 19-6).

mod dsp;
mod kit;
mod poly;

pub use kit::ElectroKit;
pub use poly::PolySynth;

/// The sine is the engine's own soft tone, used as it is.
pub use meloet_engine::PlaceholderSynth as SineSynth;
