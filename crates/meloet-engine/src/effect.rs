//! The interface every effect sits behind.

/// What the engine tells an effect about the block it is about to process.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectContext {
    /// Quarter notes per minute at the first frame of the block. An effect that follows the
    /// tempo (the delay) must take a change here without a click.
    pub tempo_bpm: f32,
}

/// One effect in one slot (an insert, a return, the master).
///
/// The same promises as [`crate::Instrument`], so a built-in effect or a proxy for an
/// out-of-process plugin can sit behind the same interface:
///
/// - `prepare` is called outside the audio thread, before the effect is handed to the audio
///   thread, and again (also outside the audio thread) whenever the sample rate changes. It is
///   the only place an effect may allocate, free, lock or block. It may be called more than
///   once. After it returns the effect is silent (as after `reset`) and keeps the parameter
///   values it was given so far.
/// - `set_param`, `process` and `reset` are called on the audio thread, never at the same time
///   as each other or as `prepare`. They must not allocate, free, lock or block.
/// - `set_param` takes the parameter's position in the effect's parameter table
///   (`meloet_model::effects`) and the value in that parameter's own unit (Hz, dB, ms, s, %,
///   a step number, 0 or 1 for off and on). A position the effect does not have is ignored; a
///   value outside the range is brought into it; a value that is not a number is ignored. The
///   new value is the target: while the effect is running it gets there smoothly, without a
///   click, within about 30 ms. A value set before `prepare`, or between `prepare`/`reset` and
///   the next `process`, applies from the first frame with no glide.
/// - `process` gets `left.len() == right.len()`, at least 1 and at most the `max_frames` given
///   to the last `prepare`. The length may differ from call to call. The effect reads both
///   slices and overwrites them in place. It adds no delay to the dry signal.
/// - The output is always finite. If the input or the effect's own state turns into NaN or
///   infinity, the effect clears its state and gives silence for that block.
/// - `reset` clears everything that sounds (delay lines, filter state, envelopes) and keeps the
///   parameters.
/// - The engine never drops an effect on the audio thread: a replaced effect travels back to
///   the control side and is dropped there.
pub trait Effect: Send {
    fn prepare(&mut self, sample_rate: f32, max_frames: usize);
    fn set_param(&mut self, index: u32, value: f32);
    fn process(&mut self, left: &mut [f32], right: &mut [f32], ctx: &EffectContext);
    fn reset(&mut self);
}
