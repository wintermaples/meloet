//! round12 stub: M6. Gate, de-esser and saturation pass the sound through unchanged until
//! they are built; this file goes away with them.

use meloet_engine::{Effect, EffectContext};

pub(super) struct PassThrough;

impl Effect for PassThrough {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}

    fn set_param(&mut self, _index: u32, _value: f32) {}

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        if left.iter().chain(right.iter()).any(|v| !v.is_finite()) {
            left.fill(0.0);
            right.fill(0.0);
        }
    }

    fn reset(&mut self) {}
}
