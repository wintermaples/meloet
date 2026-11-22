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

/// A press that comes from the song's sequence, with its place in the song. What an instrument
/// would pick by chance or by turns (one of the samples that take turns, the start of a wave)
/// it picks from here for such a press, so that a note of the song sounds the same wherever
/// playing started and in the export (docs/architecture.md 22-25). Keys someone plays now
/// have no place and come through [`Instrument::event`].
#[derive(Clone, Copy, Debug)]
pub struct SongPlace<'a> {
    seed: u32,
    /// Cumulative counts by velocity (`[v]`: presses at velocity `v` or lower) of the earlier
    /// presses of the key up to `earlier`.
    tally: Option<&'a [u32; 128]>,
    /// The velocities of the earlier presses of the key that the tally does not hold, in song
    /// order.
    earlier: &'a [u8],
}

impl<'a> SongPlace<'a> {
    /// `earlier`: the velocities of every earlier press of the same key in the song.
    pub fn new(seed: u32, earlier: &'a [u8]) -> SongPlace<'a> {
        SongPlace {
            seed,
            tally: None,
            earlier,
        }
    }

    pub(crate) fn tallied(seed: u32, tally: Option<&'a [u32; 128]>, earlier: &'a [u8]) -> Self {
        SongPlace {
            seed,
            tally,
            earlier,
        }
    }

    /// The same for the same note of the same song: made from its tick, its key and its order
    /// among the presses of that key at that tick.
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// The earlier presses of the same key in the song with a velocity in `lo..=hi`.
    pub fn earlier(&self, lo: u8, hi: u8) -> u32 {
        let (lo, hi) = (lo.max(1), hi.min(127));
        if lo > hi {
            return 0;
        }
        let tallied = self.tally.map_or(0, |t| {
            t[usize::from(hi)].wrapping_sub(t[usize::from(lo) - 1])
        });
        let rest = self.earlier.iter().filter(|&&v| lo <= v && v <= hi).count();
        tallied.wrapping_add(rest as u32)
    }
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
    /// A press from the song's sequence (`velocity` > 0), on the audio thread like
    /// [`Instrument::event`]. An instrument that picks nothing by chance or by turns takes it
    /// as that event.
    fn song_note_on(&mut self, note: u8, velocity: u8, _place: SongPlace<'_>) {
        self.event(InstrumentEvent::NoteOn { note, velocity });
    }
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
    /// An automation point of a parameter of a plugin: `value` 0..=1 at `frame` of the next
    /// `render`, a straight line from the point before. Comes before the `ask` of that
    /// `render`. An instrument without parameters passes it over.
    fn set_param_at(&mut self, _frame: u32, _id: u32, _value: f32) {}
}
