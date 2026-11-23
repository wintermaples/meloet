//! Turns what was played while recording into takes.

use crate::edit::NewTake;
use crate::song::{CC_MODULATION, CC_SUSTAIN, ControlPoint, ControlValue, Note};
use crate::time::TickRange;

mod overdub;
pub use overdub::overdub_recording;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecEventKind {
    /// Velocity 0 counts as a release.
    NoteOn {
        key: u8,
        velocity: u8,
    },
    NoteOff {
        key: u8,
    },
    Cc {
        number: u8,
        value: u8,
    },
    PitchBend(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecEvent {
    /// Song position as it came, not rounded. Can be before 0.
    pub tick: i64,
    /// Pass of the loop, from 0. Not looked at without a loop.
    pub lap: u32,
    pub kind: RecEventKind,
}

/// Controller values as they were when recording started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeldControls {
    pub sustain: u8,
    pub bend: i16,
    pub modulation: u8,
}

impl HeldControls {
    /// The ones that are not at rest, to write at the head of a take.
    fn non_rest(self) -> impl Iterator<Item = ControlValue> {
        [
            (self.sustain != 0).then_some(ControlValue::Cc {
                number: CC_SUSTAIN,
                value: self.sustain,
            }),
            (self.bend != 0).then_some(ControlValue::PitchBend(self.bend)),
            (self.modulation != 0).then_some(ControlValue::Cc {
                number: CC_MODULATION,
                value: self.modulation,
            }),
        ]
        .into_iter()
        .flatten()
    }
}

/// What a stopped recording gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub takes: Vec<NewTake>,
    /// The take to show: the last pass that has notes.
    pub active: usize,
}

#[derive(Debug, Clone, Copy)]
struct RawNote {
    start: i64,
    end: i64,
    key: u8,
    velocity: u8,
}

#[derive(Debug, Clone, Copy)]
struct Held {
    /// Index into `laps`.
    lap: usize,
    start: i64,
    velocity: u8,
}

#[derive(Debug, Clone)]
struct Lap {
    number: u32,
    /// Where the pass starts in the song. Without a loop, where recording started.
    start: i64,
    /// Written at the take's tick 0.
    head: Vec<ControlValue>,
    notes: Vec<RawNote>,
    controls: Vec<(i64, ControlValue)>,
}

/// Fed one event at a time while recording.
#[derive(Debug, Clone)]
pub struct Recorder {
    loop_range: Option<TickRange>,
    laps: Vec<Lap>,
    held: [Option<Held>; 128],
    controls: HeldControls,
    /// The first pass that [`Recorder::close_laps`] has not ended.
    open_from: u32,
}

impl Recorder {
    /// `loop_range` is the loop while it is on. A loop that ends at or before `start` is not
    /// looked at.
    pub fn new(start: u64, loop_range: Option<TickRange>, held: HeldControls) -> Recorder {
        let start = start as i64;
        let loop_range = loop_range.filter(|r| (r.end() as i64) > start);
        Recorder {
            loop_range,
            laps: vec![Lap {
                number: 0,
                start,
                head: held.non_rest().collect(),
                notes: Vec::new(),
                controls: Vec::new(),
            }],
            held: [None; 128],
            controls: held,
            open_from: 0,
        }
    }

    pub fn push(&mut self, event: RecEvent) {
        self.enter_lap(event.lap);
        let tick = event.tick;
        match event.kind {
            RecEventKind::NoteOn { key, velocity } if key <= 127 && velocity > 0 => {
                self.release(key, tick);
                self.held[usize::from(key)] = Some(Held {
                    lap: self.laps.len() - 1,
                    start: tick,
                    velocity: velocity.min(127),
                });
            }
            RecEventKind::NoteOn { key, .. } | RecEventKind::NoteOff { key } => {
                if key <= 127 {
                    self.release(key, tick);
                }
            }
            RecEventKind::Cc { number, value } if number <= 127 && value <= 127 => {
                match number {
                    CC_SUSTAIN => self.controls.sustain = value,
                    CC_MODULATION => self.controls.modulation = value,
                    _ => {}
                }
                self.control(event.lap, tick, ControlValue::Cc { number, value });
            }
            RecEventKind::PitchBend(v) if (-8192..=8191).contains(&v) => {
                self.controls.bend = v;
                self.control(event.lap, tick, ControlValue::PitchBend(v));
            }
            RecEventKind::Cc { .. } | RecEventKind::PitchBend(_) => {}
        }
    }

    /// The notes of pass `lap` so far, at their song positions. Keys still down reach `now`.
    pub fn live_notes(&self, now: i64, lap: u32) -> Vec<Note> {
        let lap = if self.loop_range.is_some() { lap } else { 0 };
        let Some(li) = self.laps.iter().position(|l| l.number == lap) else {
            return Vec::new();
        };
        let ceil = self.loop_range.map_or(i64::MAX, |r| r.end() as i64);
        let floor = if lap == 0 { 0 } else { self.laps[li].start };
        // A key still down from an earlier pass was cut at the loop end.
        let end = if li == self.laps.len() - 1 { now } else { ceil };
        let held = self.held.iter().enumerate().filter_map(|(key, h)| {
            h.filter(|h| h.lap == li).map(|h| RawNote {
                start: h.start,
                end,
                key: key as u8,
                velocity: h.velocity,
            })
        });
        let mut notes: Vec<Note> = self.laps[li]
            .notes
            .iter()
            .copied()
            .chain(held)
            .map(|n| {
                let (s, e) = clamp_note(n, floor, ceil);
                Note {
                    from: None,
                    start: s as u64,
                    length: (e - s) as u64,
                    key: n.key,
                    velocity: n.velocity,
                }
            })
            .collect();
        notes.sort_by_key(|n| n.start);
        notes
    }

    /// The person stopped at `tick` in pass `lap`, and nothing after that is kept: notes that
    /// start later are dropped, later passes too, and a note that reaches past it ends
    /// there. Notes the engine makes itself (an arpeggio) are said when they are rendered,
    /// a little before they are heard, so some can lie after the place the stop was heard at.
    /// Call it before [`Recorder::finish`] with the same place.
    pub fn drop_after(&mut self, tick: i64, lap: u32) {
        let lap = if self.loop_range.is_some() { lap } else { 0 };
        let Some(at) = self.laps.iter().position(|l| l.number == lap) else {
            return;
        };
        self.laps.truncate(at + 1);
        for held in &mut self.held {
            if held.is_some_and(|h| h.lap > at || h.start > tick) {
                *held = None;
            }
        }
        let notes = &mut self.laps[at].notes;
        notes.retain(|n| n.start <= tick);
        for n in notes {
            n.end = n.end.min(tick);
        }
    }

    /// Stops at `tick` in pass `lap`. `None` when no note was played.
    pub fn finish(mut self, tick: i64, lap: u32) -> Option<Recorded> {
        self.enter_lap(lap);
        for key in 0..128u8 {
            self.release(key, tick);
        }
        let last = self.laps.len() - 1;
        let takes: Vec<NewTake> = self
            .laps
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.notes.is_empty())
            .map(|(i, l)| self.take(l, i == last, tick))
            .collect();
        (!takes.is_empty()).then(|| Recorded {
            active: takes.len() - 1,
            takes,
        })
    }

    /// Ends the passes up to `upto` of a loop recording: keys still held from them are cut at
    /// the loop's end, as when let go in a later pass. Gives the take of each of them that has
    /// notes, and forgets them, so that [`Recorder::finish`] does not give them again. Keys
    /// that come late for an ended pass go into the pass that is going on. Nothing without a
    /// loop.
    pub fn close_laps(&mut self, upto: u32) -> Vec<NewTake> {
        let Some(range) = self.loop_range else {
            return Vec::new();
        };
        let ceil = range.end() as i64;
        self.open_from = self.open_from.max(upto.saturating_add(1));
        for key in 0..128u8 {
            let slot = &mut self.held[usize::from(key)];
            if let Some(h) = slot.filter(|h| self.laps[h.lap].number <= upto) {
                *slot = None;
                self.laps[h.lap].notes.push(RawNote {
                    start: h.start,
                    end: ceil,
                    key,
                    velocity: h.velocity,
                });
            }
        }
        let mut takes = Vec::new();
        for i in 0..self.laps.len() {
            if self.laps[i].number > upto {
                break;
            }
            if !self.laps[i].notes.is_empty() {
                takes.push(self.take(&self.laps[i], false, ceil));
            }
            let lap = &mut self.laps[i];
            lap.notes.clear();
            lap.controls.clear();
            lap.head.clear();
        }
        takes
    }

    fn take(&self, lap: &Lap, last: bool, stop: i64) -> NewTake {
        let ceil = self.loop_range.map_or(i64::MAX, |r| r.end() as i64);
        // The first take reaches back to a key pressed before recording started; later passes
        // start at the loop start.
        let origin = if lap.number == 0 {
            let first = lap.notes.iter().map(|n| n.start).min().unwrap_or(lap.start);
            lap.start.min(first).max(0)
        } else {
            lap.start
        };
        let mut notes: Vec<(i64, i64, u8, u8)> = lap
            .notes
            .iter()
            .map(|&n| {
                let (s, e) = clamp_note(n, origin, ceil);
                (s, e, n.key, n.velocity)
            })
            .collect();
        notes.sort_by_key(|n| n.0);
        let reach = notes.iter().map(|n| n.1).max().unwrap_or(origin + 1);
        let end = if last || self.loop_range.is_none() {
            stop.max(reach).min(ceil)
        } else {
            ceil
        };
        let mut controls: Vec<ControlPoint> = lap
            .head
            .iter()
            .map(|&value| ControlPoint { tick: 0, value })
            .chain(lap.controls.iter().map(|&(t, value)| ControlPoint {
                tick: (t.clamp(origin, end - 1) - origin) as u64,
                value,
            }))
            .collect();
        controls.sort_by_key(|c| c.tick);
        NewTake {
            origin: origin as u64,
            length: (end - origin) as u64,
            notes: notes
                .into_iter()
                .map(|(s, e, key, velocity)| Note {
                    from: None,
                    start: (s - origin) as u64,
                    length: (e - s) as u64,
                    key,
                    velocity,
                })
                .collect(),
            controls,
        }
    }

    fn enter_lap(&mut self, lap: u32) {
        let Some(range) = self.loop_range else {
            return;
        };
        while self.laps.last().expect("never empty").number < lap {
            let number = self.laps.last().expect("never empty").number + 1;
            self.laps.push(Lap {
                number,
                start: range.start() as i64,
                head: self.controls.non_rest().collect(),
                notes: Vec::new(),
                controls: Vec::new(),
            });
        }
    }

    fn release(&mut self, key: u8, tick: i64) {
        let Some(h) = self.held[usize::from(key)].take() else {
            return;
        };
        let end = match self.loop_range {
            Some(r) if h.lap != self.laps.len() - 1 => r.end() as i64,
            _ => tick,
        };
        self.laps[h.lap].notes.push(RawNote {
            start: h.start,
            end,
            key,
            velocity: h.velocity,
        });
    }

    /// A controller move of pass `lap` goes into that pass while it is still open, also when
    /// notes of a later pass came first (an arpeggio's notes are said before they are heard);
    /// the passes after it then start with the value it left. Else into the pass going on.
    fn control(&mut self, lap: u32, tick: i64, value: ControlValue) {
        let last = self.laps.len() - 1;
        let open = self.open_from;
        let at = match self.loop_range {
            Some(_) if lap >= open => (self.laps.iter())
                .position(|l| l.number == lap)
                .unwrap_or(last),
            _ => last,
        };
        self.laps[at].controls.push((tick, value));
        for later in &mut self.laps[at + 1..] {
            set_head(&mut later.head, value);
        }
    }
}

/// The value a pass starts with, for the controllers that are written at a take's head
/// ([`HeldControls`]); at rest, nothing is written.
fn set_head(head: &mut Vec<ControlValue>, value: ControlValue) {
    let rest = match value {
        ControlValue::Cc { number, value } if number == CC_SUSTAIN || number == CC_MODULATION => {
            head.retain(|h| !matches!(h, ControlValue::Cc { number: n, .. } if *n == number));
            value == 0
        }
        ControlValue::PitchBend(v) => {
            head.retain(|h| !matches!(h, ControlValue::PitchBend(_)));
            v == 0
        }
        ControlValue::Cc { .. } => return,
    };
    if !rest {
        head.push(value);
    }
}

/// Keeps a note inside `[floor, ceil)` and at least 1 tick long.
fn clamp_note(n: RawNote, floor: i64, ceil: i64) -> (i64, i64) {
    let s = n.start.clamp(floor, ceil - 1);
    (s, n.end.clamp(s + 1, ceil))
}

#[cfg(test)]
mod tests;
