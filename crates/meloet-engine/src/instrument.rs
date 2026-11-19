//! The interface every sound source sits behind.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentEvent {
    NoteOn {
        note: u8,
        velocity: u8,
    },
    NoteOff {
        note: u8,
    },
    Control {
        controller: u8,
        value: u8,
    },
    /// -8192..=8191, 0 is centre.
    PitchBend {
        value: i16,
    },
    /// Let every sounding note go into its release.
    AllNotesOff,
}

/// One sound source on one track.
///
/// Promises the engine keeps, so that a sample player or a proxy for an out-of-process plugin
/// can sit behind the same interface:
///
/// - `prepare` is called outside the audio thread, before the instrument is handed to the audio
///   thread, and again (also outside the audio thread) whenever the sample rate changes. It is
///   the only place an instrument may allocate, free, lock or block. It may be called more than
///   once; after it returns the instrument is silent.
/// - `event`, `render` and `reset` are called on the audio thread, never at the same time as
///   each other or as `prepare`. They must not allocate, free, lock or block.
/// - `render` gets `left.len() == right.len()`, at least 1 and at most the `max_frames` given to
///   the last `prepare`. The length may differ from call to call. The instrument overwrites both
///   slices completely (it does not add to what is there).
/// - Events take effect at the first frame of the next `render`.
/// - The engine never drops an instrument on the audio thread: a replaced instrument travels
///   back to the control side and is dropped there, so `Drop` may free memory and join threads.
pub trait Instrument: Send {
    fn prepare(&mut self, sample_rate: f32, max_frames: usize);
    fn event(&mut self, ev: InstrumentEvent);
    fn render(&mut self, left: &mut [f32], right: &mut [f32]);
    /// Silence at once: all voices and all held state (pedal, bend, modulation) are cleared.
    fn reset(&mut self);
    /// On the audio thread, as soon as the engine knows the length of the next `render` and
    /// has sent every event that comes before it. An instrument that renders somewhere else
    /// (a plugin in another process) starts now, so that all of them work at the same time
    /// while the engine goes through its tracks. No `event` comes between this and the
    /// `render` it announces; a `render` may also come unannounced.
    fn ask(&mut self, _frames: usize) {}
    /// Frames by which the sound comes out late (a plugin that looks ahead). Read on the
    /// audio thread at every block; the engine sends the track's sequence early by this much
    /// (docs/architecture.md 20-8).
    fn latency(&self) -> u32 {
        0
    }
}
