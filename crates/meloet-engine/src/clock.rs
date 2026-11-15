//! What the listener hears when: a ring of the stretches the audio thread rendered, each with the
//! time its first frame reaches the DAC.
//!
//! The audio thread is the only writer. Readers (the audio thread itself when it stamps keyboard
//! events, the meter thread for the play line) never wait for it: every slot is a small seqlock
//! made of atomics, and a reader that catches a slot half-written reads it again.

use std::sync::atomic::{AtomicU64, Ordering, fence};

use crate::engine::monotonic_us;

const SLOTS: usize = 512;
const WORDS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TransportState {
    #[default]
    Stopped,
    CountIn,
    Playing,
    Recording,
}

impl TransportState {
    fn to_bits(self) -> u64 {
        match self {
            TransportState::Stopped => 0,
            TransportState::CountIn => 1,
            TransportState::Playing => 2,
            TransportState::Recording => 3,
        }
    }

    fn from_bits(bits: u64) -> TransportState {
        match bits & 0xff {
            1 => TransportState::CountIn,
            2 => TransportState::Playing,
            3 => TransportState::Recording,
            _ => TransportState::Stopped,
        }
    }

    pub fn is_moving(self) -> bool {
        self != TransportState::Stopped
    }
}

/// One stretch of frames with a straight line from frames to song position.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Segment {
    /// When the first frame reaches the DAC, [`monotonic_us`] clock, with the fraction kept.
    pub start_us: f64,
    pub frames: u32,
    pub sample_rate: u32,
    /// Song position of the first frame, in ticks. Negative during a count-in from near 0.
    pub tick: f64,
    /// 0 while stopped.
    pub ticks_per_frame: f64,
    pub state: TransportState,
    /// Recording session the stretch belongs to.
    pub session: u32,
    pub lap: u32,
    pub record_from: i64,
    pub ticks_per_beat: u32,
}

impl Segment {
    fn tick_at(&self, t_us: f64) -> f64 {
        self.tick + (t_us - self.start_us) * self.ticks_per_us()
    }

    fn ticks_per_us(&self) -> f64 {
        self.ticks_per_frame * f64::from(self.sample_rate) / 1e6
    }

    fn encode(&self) -> [u64; WORDS] {
        [
            self.start_us.to_bits(),
            u64::from(self.frames) | u64::from(self.sample_rate) << 32,
            self.tick.to_bits(),
            self.ticks_per_frame.to_bits(),
            self.state.to_bits() | u64::from(self.lap) << 32,
            self.record_from as u64,
            u64::from(self.ticks_per_beat) | u64::from(self.session) << 32,
        ]
    }

    fn decode(w: [u64; WORDS]) -> Segment {
        Segment {
            start_us: f64::from_bits(w[0]),
            frames: w[1] as u32,
            sample_rate: (w[1] >> 32) as u32,
            tick: f64::from_bits(w[2]),
            ticks_per_frame: f64::from_bits(w[3]),
            state: TransportState::from_bits(w[4]),
            lap: (w[4] >> 32) as u32,
            record_from: w[5] as i64,
            ticks_per_beat: w[6] as u32,
            session: (w[6] >> 32) as u32,
        }
    }
}

/// Where the listener is in the song at some moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeardPosition {
    /// Song position in ticks, with the fraction. Negative during a count-in from near 0.
    pub tick: f64,
    pub state: TransportState,
    /// Loop lap of the current recording, 0 for the first pass.
    pub lap: u32,
    /// Beats still to come before recording starts; 0 outside a count-in.
    pub count_in_beats_left: u32,
    /// How fast `tick` moves; 0 while stopped.
    pub ticks_per_us: f64,
}

#[derive(Default)]
struct Slot {
    seq: AtomicU64,
    /// Which segment (counting from 0) the slot holds, so a reader can tell it was overwritten.
    serial: AtomicU64,
    words: [AtomicU64; WORDS],
}

/// Shared between the audio thread (writer) and any number of readers.
pub struct TransportClock {
    slots: Box<[Slot]>,
    written: AtomicU64,
}

impl Default for TransportClock {
    fn default() -> Self {
        TransportClock {
            slots: (0..SLOTS).map(|_| Slot::default()).collect(),
            written: AtomicU64::new(0),
        }
    }
}

impl TransportClock {
    /// Only the engine that owns this clock calls this, from one thread at a time.
    pub(crate) fn push(&self, segment: &Segment) {
        let serial = self.written.load(Ordering::Relaxed);
        let slot = &self.slots[serial as usize % SLOTS];
        let seq = slot.seq.load(Ordering::Relaxed);
        slot.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        slot.serial.store(serial, Ordering::Relaxed);
        for (cell, word) in slot.words.iter().zip(segment.encode()) {
            cell.store(word, Ordering::Relaxed);
        }
        slot.seq.store(seq.wrapping_add(2), Ordering::Release);
        self.written.store(serial + 1, Ordering::Release);
    }

    fn read(&self, serial: u64) -> Option<Segment> {
        let slot = &self.slots[serial as usize % SLOTS];
        for _ in 0..8 {
            let before = slot.seq.load(Ordering::Acquire);
            if before & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let held = slot.serial.load(Ordering::Relaxed);
            let words: [u64; WORDS] =
                std::array::from_fn(|i| slot.words[i].load(Ordering::Relaxed));
            fence(Ordering::Acquire);
            if slot.seq.load(Ordering::Relaxed) != before {
                continue;
            }
            return (held == serial).then(|| Segment::decode(words));
        }
        None
    }

    /// The newest segment that starts at or before `t_us`.
    pub(crate) fn segment_at(&self, t_us: f64) -> Option<Segment> {
        let written = self.written.load(Ordering::Acquire);
        let oldest = written.saturating_sub(SLOTS as u64);
        (oldest..written)
            .rev()
            .filter_map(|serial| self.read(serial))
            .find(|segment| segment.start_us <= t_us)
    }

    /// Where the listener was in the song at `t_us` ([`monotonic_us`] clock). `None` before the
    /// engine has rendered anything, or for a moment older than the ring remembers (about 512
    /// blocks).
    pub fn heard_at(&self, t_us: u64) -> Option<HeardPosition> {
        self.segment_at(t_us as f64).map(|s| heard(&s, t_us as f64))
    }

    pub fn heard_now(&self) -> Option<HeardPosition> {
        self.heard_at(monotonic_us())
    }
}

pub(crate) fn heard(segment: &Segment, t_us: f64) -> HeardPosition {
    let tick = segment.tick_at(t_us);
    let count_in_beats_left = if segment.state == TransportState::CountIn {
        let left = (segment.record_from as f64 - tick) / f64::from(segment.ticks_per_beat.max(1));
        left.ceil().max(0.0) as u32
    } else {
        0
    };
    HeardPosition {
        tick,
        state: segment.state,
        lap: segment.lap,
        count_in_beats_left,
        ticks_per_us: segment.ticks_per_us(),
    }
}
