//! The master's sound for an analysis thread (round 12: the spectrum of E7, the loudness of
//! M3), taken after the master's inserts and before the test tone, the latency click and the
//! metronome. Only for listening: nothing here changes what comes out.
//!
//! The ring is made with the engine; the audio thread writes into it only while the reader has
//! been taken ([`crate::EngineHandle::take_analysis`]) and not dropped, so an engine nobody
//! analyses (an export) copies nothing. A piece that does not fit is dropped whole.
//!
//! With the frames the reader learns whether the song is heard playing (playing or recording,
//! not counting in) and how many times it started to ([`AnalysisReader::playing`]), as of the
//! last piece written. The reader applies it to all it takes at one wake, so a start or a stop
//! is placed within about one wake (10 ms) and one piece.

use std::sync::Arc;
use std::sync::atomic::{
    AtomicBool, AtomicU32, AtomicU64, Ordering::Acquire, Ordering::Relaxed, Ordering::Release,
};

use rtrb::{Consumer, Producer, RingBuffer};

/// Frames the ring holds: 341 ms at 48 kHz, 171 ms at 96 kHz.
pub const ANALYSIS_RING_FRAMES: usize = 1 << 14;

struct Shared {
    /// The reader is out of the handle and not dropped.
    reading: AtomicBool,
    /// Frames the audio thread could not put into the ring.
    dropped: AtomicU64,
    sample_rate: AtomicU32,
    /// The pieces written last were heard playing.
    playing: AtomicBool,
    /// Times the song started to be heard playing.
    plays: AtomicU32,
}

/// The audio thread's end.
pub(crate) struct AnalysisTap {
    ring: Producer<[f32; 2]>,
    shared: Arc<Shared>,
    playing: bool,
}

/// The analysis thread's end: left and right frames, in order, with gaps where the ring was
/// full ([`AnalysisReader::dropped`]).
pub struct AnalysisReader {
    ring: Consumer<[f32; 2]>,
    shared: Arc<Shared>,
}

pub(crate) fn analysis_ring(sample_rate: u32) -> (AnalysisTap, AnalysisReader) {
    let (tx, rx) = RingBuffer::new(ANALYSIS_RING_FRAMES);
    let shared = Arc::new(Shared {
        reading: AtomicBool::new(false),
        dropped: AtomicU64::new(0),
        sample_rate: AtomicU32::new(sample_rate),
        playing: AtomicBool::new(false),
        plays: AtomicU32::new(0),
    });
    (
        AnalysisTap {
            ring: tx,
            shared: shared.clone(),
            playing: false,
        },
        AnalysisReader { ring: rx, shared },
    )
}

impl AnalysisTap {
    /// Outside the audio thread, with the engine (`Engine::prepare`).
    pub fn set_sample_rate(&self, sample_rate: u32) {
        self.shared.sample_rate.store(sample_rate, Relaxed);
    }

    /// On the audio thread, once a piece. `playing`: the song is heard playing at its end. A
    /// frame that is not a number goes in as silence, as it comes out.
    pub fn write(&mut self, left: &[f32], right: &[f32], playing: bool) {
        if playing != self.playing {
            self.playing = playing;
            if playing {
                self.shared.plays.fetch_add(1, Release);
            }
            self.shared.playing.store(playing, Release);
        }
        if !self.shared.reading.load(Relaxed) {
            return;
        }
        let frames = left.len().min(right.len());
        let Ok(chunk) = self.ring.write_chunk_uninit(frames) else {
            self.shared.dropped.fetch_add(frames as u64, Relaxed);
            return;
        };
        chunk.fill_from_iter(left.iter().zip(right).map(|(&l, &r)| {
            if l.is_finite() && r.is_finite() {
                [l, r]
            } else {
                [0.0, 0.0]
            }
        }));
    }
}

impl AnalysisReader {
    /// From now on the audio thread writes (it wrote nothing while the reader was in the handle).
    pub(crate) fn start(self) -> AnalysisReader {
        self.shared.reading.store(true, Relaxed);
        self
    }

    /// Hands every frame that is in the ring to `take`, in at most two slices, and frees them.
    /// Returns how many there were.
    pub fn read(&mut self, mut take: impl FnMut(&[[f32; 2]])) -> usize {
        let Ok(chunk) = self.ring.read_chunk(self.ring.slots()) else {
            return 0;
        };
        let (first, second) = chunk.as_slices();
        let frames = first.len() + second.len();
        for part in [first, second] {
            if !part.is_empty() {
                take(part);
            }
        }
        chunk.commit_all();
        frames
    }

    /// Frames lost so far because the ring was full: when this grows, what was read before
    /// and what is read now do not join.
    pub fn dropped(&self) -> u64 {
        self.shared.dropped.load(Relaxed)
    }

    /// Whether the song is heard playing, and how many times it started to, as of the last
    /// piece written. Read before [`AnalysisReader::read`]: the frames it hands over are those
    /// pieces, and perhaps a few written since.
    pub fn playing(&self) -> (bool, u32) {
        let plays = self.shared.plays.load(Acquire);
        (self.shared.playing.load(Acquire), plays)
    }

    /// The rate of the engine's stream. It changes only while no stream runs; a few frames
    /// of the rate before may still be in the ring then.
    pub fn sample_rate(&self) -> u32 {
        self.shared.sample_rate.load(Relaxed)
    }
}

impl Drop for AnalysisReader {
    fn drop(&mut self) {
        self.shared.reading.store(false, Relaxed);
    }
}
