//! Changes to the control points (CC and pitch bend) of a clip's shown take: a new take in
//! place of the shown one, like the note edits in [`crate::notes`]. Points are named by their
//! index in the take's controls (in order of tick; at the same tick, in the order they have).
//! Points stay inside the clip.

use crate::edit::{EditError, keys_clip_index};
use crate::history::StepKind;
use crate::notes::{
    Content, NotesEdited, indexes_count, lifted, pick, put_take, song_controls, unmarked,
};
use crate::song::{ClipId, ControlPoint, ControlValue, Doc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Cc(u8),
    PitchBend,
}

/// CCs that are only on or off: sustain, portamento, sostenuto, soft, legato, hold 2.
pub fn is_switch(number: u8) -> bool {
    (64..=69).contains(&number)
}

/// `value` brought into the range of the kind: CC 0..=127 (a switch only 0 or 127, 64 and
/// over is on), pitch bend -8192..=8191.
pub fn control_value(kind: ControlKind, value: i64) -> ControlValue {
    match kind {
        ControlKind::Cc(number) if is_switch(number) => ControlValue::Cc {
            number,
            value: if value >= 64 { 127 } else { 0 },
        },
        ControlKind::Cc(number) => ControlValue::Cc {
            number,
            value: value.clamp(0, 127) as u8,
        },
        ControlKind::PitchBend => ControlValue::PitchBend(value.clamp(-8192, 8191) as i16),
    }
}

fn kind_and_value(v: ControlValue) -> (ControlKind, i64) {
    match v {
        ControlValue::Cc { number, value } => (ControlKind::Cc(number), i64::from(value)),
        ControlValue::PitchBend(b) => (ControlKind::PitchBend, i64::from(b)),
    }
}

/// The write tool in a CC lane. `tick` is a song position inside the clip.
pub fn add_control(
    doc: &Doc,
    clip: ClipId,
    tick: u64,
    kind: ControlKind,
    value: i64,
) -> Result<NotesEdited, EditError> {
    if matches!(kind, ControlKind::Cc(n) if n > 127) {
        return Err(EditError::BadControl);
    }
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let c = &doc.tracks[ti].clips[ci];
    if tick < c.start || tick >= c.end {
        return Err(EditError::OutOfClip);
    }
    let (c, l) = lifted(c);
    let mut controls: Vec<(ControlPoint, bool)> =
        song_controls(&c).into_iter().map(|p| (p, false)).collect();
    controls.push((
        ControlPoint {
            tick: tick + l,
            value: control_value(kind, value),
        },
        true,
    ));
    put(doc, (ti, ci), controls, StepKind::AddControl { clip })
}

/// Moves the points by `ticks` and changes each value by `value`. The points move together and
/// stop where the first of them would leave the clip; each value stops at its range.
pub fn move_controls(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    ticks: i64,
    value: i64,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let (c, _) = lifted(&doc.tracks[ti].clips[ci]);
    let points = song_controls(&c);
    let picked = pick(indexes, points.len())?;
    let chosen = || {
        points
            .iter()
            .zip(&picked)
            .filter_map(|(p, &x)| x.then_some(p))
    };
    let first = chosen().map(|p| p.tick).min().expect("picked one at least");
    let last = chosen().map(|p| p.tick).max().expect("picked one at least");
    // A point already outside is not pulled in; it only cannot go further out.
    let before = c.start as i64 - first as i64;
    let after = (c.end - 1) as i64 - last as i64;
    let ticks = ticks.clamp(before.min(0), after.max(0));
    let count = indexes_count(&picked);
    let controls = points
        .iter()
        .zip(&picked)
        .map(|(p, &x)| {
            if !x {
                return (*p, false);
            }
            let (kind, v) = kind_and_value(p.value);
            (
                ControlPoint {
                    tick: (p.tick as i64 + ticks) as u64,
                    value: control_value(kind, v.saturating_add(value)),
                },
                true,
            )
        })
        .collect();
    put(
        doc,
        (ti, ci),
        controls,
        StepKind::MoveControls { clip, count },
    )
}

/// The erase tool and Delete in a CC lane.
pub fn delete_controls(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let points = song_controls(&lifted(&doc.tracks[ti].clips[ci]).0);
    let picked = pick(indexes, points.len())?;
    let count = indexes_count(&picked);
    let controls = points
        .into_iter()
        .zip(picked)
        .filter_map(|(p, x)| (!x).then_some((p, false)))
        .collect();
    put(
        doc,
        (ti, ci),
        controls,
        StepKind::DeleteControls { clip, count },
    )
}

fn put(
    doc: &Doc,
    (ti, ci): (usize, usize),
    controls: Vec<(ControlPoint, bool)>,
    step: StepKind,
) -> Result<NotesEdited, EditError> {
    let (c, _) = lifted(&doc.tracks[ti].clips[ci]);
    let content = Content {
        notes: unmarked(&c),
        controls,
        origin: c.active_take().take.origin.clone(),
    };
    put_take(doc, (ti, ci), content, true, step)
}

#[cfg(test)]
mod tests;
