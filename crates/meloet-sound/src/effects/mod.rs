//! The built-in effects: EQ 4 bands, compressor, reverb, delay, chorus, limiter, gate,
//! de-esser, saturation (docs/architecture.md 19-2, 19-3).
//!
//! Each one keeps the promises of `meloet_engine::Effect`. Parameter positions, ranges and
//! defaults come from `meloet_model::effects`.

mod chorus;
mod compressor;
mod de_esser;
mod delay;
pub(crate) mod dsp;
mod eq4;
mod gate;
mod limiter;
mod reverb;
mod saturation;

use meloet_engine::Effect;
use meloet_model::effects::EffectKind;

pub use chorus::Chorus;
pub use compressor::{Compressor, KNEE_DB, static_gain_db};
pub use de_esser::DeEsser;
pub use delay::Delay;
pub use eq4::Eq4;
pub use gate::Gate;
pub use limiter::Limiter;
pub use reverb::Reverb;
pub use saturation::{REFERENCE as SATURATION_REFERENCE, Saturation, bend_for as saturation_bend};

/// A new effect with the defaults of its table. Allocates: call it outside the audio thread,
/// then `prepare`.
pub fn create(kind: EffectKind) -> Box<dyn Effect> {
    match kind {
        EffectKind::Eq4 => Box::new(Eq4::new()),
        EffectKind::Compressor => Box::new(Compressor::new()),
        EffectKind::Reverb => Box::new(Reverb::new()),
        EffectKind::Delay => Box::new(Delay::new()),
        EffectKind::Chorus => Box::new(Chorus::new()),
        EffectKind::Limiter => Box::new(Limiter::new()),
        EffectKind::Gate => Box::new(Gate::new()),
        EffectKind::DeEsser => Box::new(DeEsser::new()),
        EffectKind::Saturation => Box::new(Saturation::new()),
    }
}
