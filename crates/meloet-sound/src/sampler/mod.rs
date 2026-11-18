//! The sample player: reads an instrument of a sound pack into memory and plays it
//! (docs/architecture.md 19-7 and 19-8).
//!
//! Reading happens outside the audio thread and gives an `Arc<SampleBank>`; any number of
//! [`Sampler`]s can point at the same bank. The audio thread only reads the bank's arrays.

mod bank;
mod load;
mod player;

pub use bank::SampleBank;
pub use load::{LoadError, Pack, load_instrument, read_pack};
pub use player::Sampler;
