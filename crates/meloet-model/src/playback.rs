//! What a track plays: the shown takes as events at song positions.

use crate::TrackId;
use crate::song::{CC_MODULATION, CC_SUSTAIN, ControlValue, Doc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayKind {
    NoteOff { key: u8 },
    Control(ControlValue),
    NoteOn { key: u8, velocity: u8 },
}

impl PlayKind {
    /// At the same tick: releases, then controls, then presses.
    fn rank(self) -> u8 {
        match self {
            PlayKind::NoteOff { .. } => 0,
            PlayKind::Control(_) => 1,
            PlayKind::NoteOn { .. } => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayEvent {
    pub tick: u64,
    pub kind: PlayKind,
}

/// The events of `track` in order. Only what is inside a clip sounds: a note that starts
/// before the clip's start does not (as when play starts in the middle of a note), and one that
/// goes past its end is cut there. The last pedal, bend and modulation before the start, when
/// not at rest, are put at the start (as when play starts in the middle), and whatever is left
/// on at the end is put back to 0 there. Empty for a track that is not in `doc`.
pub fn track_events(doc: &Doc, track: TrackId) -> Vec<PlayEvent> {
    let Some(track) = doc.track(track) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for clip in &track.clips {
        let Some(ct) = clip.keys().get(clip.active) else {
            continue;
        };
        let zero = clip.zero(ct);
        let (start, end) = (clip.start as i64, clip.end as i64);
        let at = |tick: u64| zero + tick as i64;
        for n in &ct.take.notes {
            let on = at(n.start);
            if on < start || on >= end {
                continue;
            }
            out.push(PlayEvent {
                tick: on as u64,
                kind: PlayKind::NoteOn {
                    key: n.key,
                    velocity: n.velocity,
                },
            });
            out.push(PlayEvent {
                tick: at(n.end()).min(end) as u64,
                kind: PlayKind::NoteOff { key: n.key },
            });
        }
        let (mut sustain, mut modulation, mut bend) = (0, 0, 0);
        let mut head = true;
        for c in &ct.take.controls {
            let tick = at(c.tick);
            if tick >= end {
                break;
            }
            if head && tick >= start {
                head = false;
                out.extend(non_rest(sustain, modulation, bend).map(|value| PlayEvent {
                    tick: clip.start,
                    kind: PlayKind::Control(value),
                }));
            }
            match c.value {
                ControlValue::Cc {
                    number: CC_SUSTAIN,
                    value,
                } => sustain = value,
                ControlValue::Cc {
                    number: CC_MODULATION,
                    value,
                } => modulation = value,
                ControlValue::PitchBend(v) => bend = v,
                ControlValue::Cc { .. } => {}
            }
            if !head {
                out.push(PlayEvent {
                    tick: tick as u64,
                    kind: PlayKind::Control(c.value),
                });
            }
        }
        if head {
            out.extend(non_rest(sustain, modulation, bend).map(|value| PlayEvent {
                tick: clip.start,
                kind: PlayKind::Control(value),
            }));
        }
        let resets = [
            (sustain != 0).then_some(ControlValue::Cc {
                number: CC_SUSTAIN,
                value: 0,
            }),
            (bend != 0).then_some(ControlValue::PitchBend(0)),
            (modulation != 0).then_some(ControlValue::Cc {
                number: CC_MODULATION,
                value: 0,
            }),
        ];
        out.extend(resets.into_iter().flatten().map(|value| PlayEvent {
            tick: clip.end,
            kind: PlayKind::Control(value),
        }));
    }
    out.sort_by_key(|e| (e.tick, e.kind.rank()));
    out
}

/// Pedal, modulation and bend, the ones not at rest.
fn non_rest(sustain: u8, modulation: u8, bend: i16) -> impl Iterator<Item = ControlValue> {
    [
        (sustain != 0).then_some(ControlValue::Cc {
            number: CC_SUSTAIN,
            value: sustain,
        }),
        (modulation != 0).then_some(ControlValue::Cc {
            number: CC_MODULATION,
            value: modulation,
        }),
        (bend != 0).then_some(ControlValue::PitchBend(bend)),
    ]
    .into_iter()
    .flatten()
}

#[cfg(test)]
mod tests;
