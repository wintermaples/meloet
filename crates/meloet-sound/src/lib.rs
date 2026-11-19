//! The sounds meloet ships with: the sample player, the synth voices, the electro kit and the
//! built-in effects (docs/architecture.md 19-6).
//!
//! Everything here is called on the audio thread through `meloet_engine::Instrument` and the
//! effect interface, so the engine's rules hold once a sound is prepared: no allocation, no
//! locks, no blocking. Sample files are read on another thread, which core owns.

pub mod effects;
pub mod phrases;
pub mod sampler;
pub mod sf2;
pub mod synth;
