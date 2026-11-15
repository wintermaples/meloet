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

/// The events of `track` in order. Nothing sounds after a clip's end: notes are cut there, and a pedal, bend or modulation left on is put back to 0 there. Empty for a track that
/// is not in `doc`.
pub fn track_events(doc: &Doc, track: TrackId) -> Vec<PlayEvent> {
    let Some(track) = doc.track(track) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for clip in &track.clips {
        let ct = clip.active_take();
        let zero = clip.start + ct.offset;
        for n in &ct.take.notes {
            let on = zero + n.start;
            if on >= clip.end {
                continue;
            }
            out.push(PlayEvent {
                tick: on,
                kind: PlayKind::NoteOn {
                    key: n.key,
                    velocity: n.velocity,
                },
            });
            out.push(PlayEvent {
                tick: (zero + n.end()).min(clip.end),
                kind: PlayKind::NoteOff { key: n.key },
            });
        }
        let (mut sustain, mut modulation, mut bend) = (0, 0, 0);
        for c in &ct.take.controls {
            let at = zero + c.tick;
            if at >= clip.end {
                continue;
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
            out.push(PlayEvent {
                tick: at,
                kind: PlayKind::Control(c.value),
            });
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

#[cfg(test)]
mod tests;
