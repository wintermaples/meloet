//! What the song plays: per-track event lists ([`Sequence`]), the per-track cursor that walks
//! them on the audio thread ([`Lane`]), and the metronome click.

use std::f32::consts::TAU;

use crate::instrument::{Instrument, InstrumentEvent};

const CC_MODULATION: u8 = 1;
const CC_SUSTAIN: u8 = 64;

const MOVED_SUSTAIN: u8 = 1;
const MOVED_MODULATION: u8 = 2;
const MOVED_BEND: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequenceEvent {
    /// Song position, 480 ticks to a quarter note.
    pub tick: u64,
    pub event: InstrumentEvent,
}

fn rank(event: InstrumentEvent) -> u8 {
    match event {
        InstrumentEvent::NoteOff { .. } | InstrumentEvent::NoteOn { velocity: 0, .. } => 0,
        InstrumentEvent::NoteOn { .. } => 2,
        _ => 1,
    }
}

/// One track's events in song order. Built outside the audio thread.
#[derive(Debug, Default)]
pub struct Sequence {
    events: Box<[SequenceEvent]>,
}

impl Sequence {
    /// Sorts by position and, at the same position, releases before controls before presses.
    /// `AllNotesOff` is dropped: the engine itself lets go of what a sequence leaves sounding.
    pub fn new(mut events: Vec<SequenceEvent>) -> Sequence {
        events.retain(|e| e.event != InstrumentEvent::AllNotesOff);
        events.sort_by_key(|e| (e.tick, rank(e.event)));
        Sequence {
            events: events.into_boxed_slice(),
        }
    }

    pub fn events(&self) -> &[SequenceEvent] {
        &self.events
    }
}

/// A set of track indexes (the tracks that do not play their sequence).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrackSet {
    bits: [u64; 4],
}

impl TrackSet {
    /// Tracks 0 to 255; others are ignored.
    pub const CAPACITY: usize = 256;

    pub fn insert(&mut self, track: usize) {
        if track < Self::CAPACITY {
            self.bits[track / 64] |= 1 << (track % 64);
        }
    }

    pub fn contains(&self, track: usize) -> bool {
        track < Self::CAPACITY && self.bits[track / 64] & 1 << (track % 64) != 0
    }
}

impl FromIterator<usize> for TrackSet {
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Self {
        let mut set = TrackSet::default();
        iter.into_iter().for_each(|t| set.insert(t));
        set
    }
}

/// A track's place in its sequence and what the sequence has left switched on.
#[derive(Default)]
pub(crate) struct Lane {
    pub sequence: Option<Box<Sequence>>,
    cursor: usize,
    /// Notes the sequence pressed and has not released, one bit per key.
    sounding: [u64; 2],
    moved: u8,
}

impl Lane {
    fn send(
        &mut self,
        event: InstrumentEvent,
        instrument: Option<&mut (dyn Instrument + 'static)>,
    ) {
        match event {
            InstrumentEvent::NoteOn { note, velocity } if velocity > 0 => {
                self.sounding[usize::from(note >> 6) & 1] |= 1 << (note & 63);
            }
            InstrumentEvent::NoteOn { note, .. } | InstrumentEvent::NoteOff { note } => {
                self.sounding[usize::from(note >> 6) & 1] &= !(1 << (note & 63));
            }
            InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                ..
            } => self.moved |= MOVED_SUSTAIN,
            InstrumentEvent::Control {
                controller: CC_MODULATION,
                ..
            } => self.moved |= MOVED_MODULATION,
            InstrumentEvent::PitchBend { .. } => self.moved |= MOVED_BEND,
            _ => {}
        }
        if let Some(instrument) = instrument {
            instrument.event(event);
        }
    }

    /// Releases the notes the sequence holds and puts back to 0 the pedal, wheel and bend it
    /// moved. Keyboard notes on the same instrument are left alone.
    pub fn silence(&mut self, mut instrument: Option<&mut (dyn Instrument + 'static)>) {
        for (half, bits) in self.sounding.iter_mut().enumerate() {
            while *bits != 0 {
                let low = bits.trailing_zeros();
                *bits &= *bits - 1;
                if let Some(i) = instrument.as_deref_mut() {
                    i.event(InstrumentEvent::NoteOff {
                        note: (half as u32 * 64 + low) as u8,
                    });
                }
            }
        }
        if let Some(i) = instrument {
            if self.moved & MOVED_SUSTAIN != 0 {
                i.event(InstrumentEvent::Control {
                    controller: CC_SUSTAIN,
                    value: 0,
                });
            }
            if self.moved & MOVED_MODULATION != 0 {
                i.event(InstrumentEvent::Control {
                    controller: CC_MODULATION,
                    value: 0,
                });
            }
            if self.moved & MOVED_BEND != 0 {
                i.event(InstrumentEvent::PitchBend { value: 0 });
            }
        }
        self.moved = 0;
    }

    /// Forgets what is sounding without sending anything (the instrument was replaced or reset).
    pub fn forget(&mut self) {
        self.sounding = [0; 2];
        self.moved = 0;
    }

    /// Puts the cursor at the first event at or after `pos` (ticks × `scale`) and applies the
    /// last pedal, wheel and bend before it.
    pub fn seek(
        &mut self,
        pos: i128,
        scale: i128,
        instrument: Option<&mut (dyn Instrument + 'static)>,
    ) {
        let Some(sequence) = self.sequence.as_deref() else {
            self.cursor = 0;
            return;
        };
        let events = sequence.events();
        let cursor = events.partition_point(|e| i128::from(e.tick) * scale < pos);
        self.cursor = cursor;
        let (mut sustain, mut modulation, mut bend) = (None, None, None);
        for e in events[..cursor].iter().rev() {
            match e.event {
                InstrumentEvent::Control {
                    controller: CC_SUSTAIN,
                    value,
                } if sustain.is_none() => sustain = Some(value),
                InstrumentEvent::Control {
                    controller: CC_MODULATION,
                    value,
                } if modulation.is_none() => modulation = Some(value),
                InstrumentEvent::PitchBend { value } if bend.is_none() => bend = Some(value),
                _ => {}
            }
            if sustain.is_some() && modulation.is_some() && bend.is_some() {
                break;
            }
        }
        let mut instrument = instrument;
        let chased = [
            sustain
                .filter(|&v| v != 0)
                .map(|value| InstrumentEvent::Control {
                    controller: CC_SUSTAIN,
                    value,
                }),
            modulation
                .filter(|&v| v != 0)
                .map(|value| InstrumentEvent::Control {
                    controller: CC_MODULATION,
                    value,
                }),
            bend.filter(|&v| v != 0)
                .map(|value| InstrumentEvent::PitchBend { value }),
        ];
        for event in chased.into_iter().flatten() {
            self.send(event, instrument.as_deref_mut());
        }
    }

    /// Sends every event at or before `pos` that has not been sent yet.
    pub fn fire_due(
        &mut self,
        pos: i128,
        scale: i128,
        mut instrument: Option<&mut (dyn Instrument + 'static)>,
    ) {
        let Some(sequence) = self.sequence.take() else {
            return;
        };
        let events = sequence.events();
        while let Some(e) = events.get(self.cursor) {
            if i128::from(e.tick) * scale > pos {
                break;
            }
            self.send(e.event, instrument.as_deref_mut());
            self.cursor += 1;
        }
        self.sequence = Some(sequence);
    }

    /// Position of the next event not yet sent, in ticks.
    pub fn next_tick(&self) -> Option<u64> {
        self.sequence
            .as_deref()
            .and_then(|s| s.events().get(self.cursor))
            .map(|e| e.tick)
    }
}

/// A short decaying tone, started exactly on a beat.
pub(crate) struct Click {
    frame: usize,
    total: usize,
    inc: f32,
    phase: f32,
    level: f32,
    decay: f32,
}

impl Click {
    const LENGTH_S: f32 = 0.035;
    const DECAY_S: f32 = 0.008;
    const BAR_HZ: f32 = 1760.0;
    const BEAT_HZ: f32 = 1046.5;
    /// The test tone peaks around 0.2 to 0.3; the click stays a little under.
    const BAR_LEVEL: f32 = 0.17;
    const BEAT_LEVEL: f32 = 0.13;

    pub fn idle() -> Self {
        Click {
            frame: 0,
            total: 0,
            inc: 0.0,
            phase: 0.0,
            level: 0.0,
            decay: 0.0,
        }
    }

    pub fn start(&mut self, bar_head: bool, sample_rate: f32) {
        let (hz, level) = if bar_head {
            (Self::BAR_HZ, Self::BAR_LEVEL)
        } else {
            (Self::BEAT_HZ, Self::BEAT_LEVEL)
        };
        *self = Click {
            frame: 0,
            total: (Self::LENGTH_S * sample_rate) as usize,
            inc: hz / sample_rate,
            phase: 0.0,
            level,
            decay: (-1.0 / (Self::DECAY_S * sample_rate)).exp(),
        };
    }

    /// Adds to `out` (the same on the left and the right). The first frame after `start` is
    /// the loudest.
    pub fn add(&mut self, out: &mut [f32]) {
        for o in out.iter_mut() {
            if self.frame >= self.total {
                return;
            }
            let fade = ((self.total - self.frame) as f32 / 48.0).min(1.0);
            *o += self.level * fade * (TAU * self.phase).cos();
            self.phase = (self.phase + self.inc).fract();
            self.level *= self.decay;
            self.frame += 1;
        }
    }
}
