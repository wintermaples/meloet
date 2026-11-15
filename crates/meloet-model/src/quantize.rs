//! Quantize, and back to where the notes were recorded. Both are edits like the others in
//! [`crate::notes`]: a new take in place of the shown one. A quantized take keeps the recorded
//! take as its origin, and each note the index of the note it came from, so quantizing again
//! always starts from where the notes were played.

use std::sync::Arc;

use crate::edit::{EditError, clip_index};
use crate::history::StepKind;
use crate::notes::{
    Content, NotesEdited, indexes_count, lifted, pick, put_take, song_controls, song_notes,
};
use crate::roll::{QuantizeAlign, QuantizeSettings};
use crate::song::{ClipId, Doc, Note, OriginIndex, TakeOrigin};
use crate::time::Meter;

/// Where `tick` goes with `settings`: toward the nearest step of the grid from the song's
/// tick 0 (odd steps later by the swing), by the strength.
pub fn quantized_tick(tick: u64, settings: QuantizeSettings, meter: Meter) -> u64 {
    let g = settings.grid.ticks(meter);
    let k = (tick + g / 2) / g;
    let swing = if k % 2 == 1 {
        (g * u64::from(settings.swing()) + 150) / 300
    } else {
        0
    };
    let target = k * g + swing;
    let d = (i128::from(target) - i128::from(tick)) * i128::from(settings.strength());
    // Half away from 0.
    let moved = (d + 50 * d.signum()) / 100;
    (i128::from(tick) + moved) as u64
}

/// Quantizes the notes at `indexes` of the clip's shown take with `settings`; with no indexes,
/// every note that starts inside the clip. Returns the edit and how many notes it quantized.
pub fn quantize(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    settings: QuantizeSettings,
) -> Result<(NotesEdited, usize), EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let (c, l) = lifted(&doc.tracks[ti].clips[ci]);
    let shown = c.active_take();
    let zero = c.zero(shown) as u64;
    let mut notes = song_notes(&c);
    let picked = if indexes.is_empty() {
        notes
            .iter()
            .map(|n| (c.start..c.end).contains(&n.start))
            .collect()
    } else {
        pick(indexes, notes.len())?
    };
    let count = indexes_count(&picked);
    if count == 0 {
        return Err(EditError::Unchanged);
    }
    let origin = match &shown.take.origin {
        Some(o) => o.take.clone(),
        None => {
            for (i, n) in notes.iter_mut().enumerate() {
                n.from = OriginIndex::new(i as u32);
            }
            shown.take.clone()
        }
    };
    let g = settings.grid.ticks(doc.meter);
    let q = |tick: u64| quantized_tick(tick.saturating_sub(l), settings, doc.meter) + l;
    let mut moved = false;
    let out = notes
        .into_iter()
        .zip(&picked)
        .map(|(n, &p)| {
            if !p {
                return (n, false);
            }
            let (start, length) = match n.from.and_then(|j| origin.notes.get(j.get() as usize)) {
                Some(o) => (zero + o.start, o.length),
                None => (n.start, n.length),
            };
            let (start, length) = match settings.align {
                QuantizeAlign::Start => {
                    let length = length.min(c.end - c.start);
                    (q(start).clamp(c.start, c.end - length), length)
                }
                QuantizeAlign::StartAndEnd => {
                    let s = q(start).clamp(c.start, c.end - 1);
                    let e = q(start + length);
                    let length = if e > s { e - s } else { g };
                    (s, length.min(c.end - s))
                }
            };
            moved |= start != n.start || length != n.length;
            (Note { start, length, ..n }, !indexes.is_empty())
        })
        .collect();
    if !moved {
        return Err(EditError::Unchanged);
    }
    let content = Content {
        notes: out,
        controls: song_controls(&c).into_iter().map(|p| (p, false)).collect(),
        origin: Some(TakeOrigin {
            take: origin,
            settings,
        }),
    };
    let edited = put_take(
        doc,
        (ti, ci),
        content,
        false,
        StepKind::Quantize { clip, count },
    )?;
    Ok((edited, count))
}

/// "もとの位置へ戻す": the notes at `indexes` (with none, all that start inside the clip) that
/// came from the origin take go back to the start and length they have there; key and velocity
/// stay. When every such note is back, the take is no longer quantized: it loses its origin.
pub fn unquantize(doc: &Doc, clip: ClipId, indexes: &[usize]) -> Result<NotesEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let (c, _) = lifted(&doc.tracks[ti].clips[ci]);
    let shown = c.active_take();
    let Some(origin) = shown.take.origin.clone() else {
        return Err(EditError::Unchanged);
    };
    let zero = c.zero(shown) as u64;
    let notes = song_notes(&c);
    let picked = if indexes.is_empty() {
        notes
            .iter()
            .map(|n| (c.start..c.end).contains(&n.start))
            .collect()
    } else {
        pick(indexes, notes.len())?
    };
    let recorded = |n: &Note| n.from.and_then(|j| origin.take.notes.get(j.get() as usize));
    let mut count = 0;
    let mut out: Vec<(Note, bool)> = notes
        .into_iter()
        .zip(picked)
        .map(|(n, p)| match recorded(&n) {
            Some(o) if p => {
                count += 1;
                let start = (zero + o.start).clamp(c.start, c.end - 1);
                let length = o.length.min(c.end - start);
                (Note { start, length, ..n }, !indexes.is_empty())
            }
            _ => (n, false),
        })
        .collect();
    let all_back = out.iter().all(|(n, _)| {
        recorded(n).is_none_or(|o| n.start == zero + o.start && n.length == o.length)
    });
    let origin = if all_back {
        for (n, _) in &mut out {
            n.from = None;
        }
        None
    } else {
        Some(TakeOrigin {
            take: Arc::clone(&origin.take),
            settings: origin.settings,
        })
    };
    let content = Content {
        notes: out,
        controls: song_controls(&c).into_iter().map(|p| (p, false)).collect(),
        origin,
    };
    put_take(
        doc,
        (ti, ci),
        content,
        false,
        StepKind::Unquantize { clip, count },
    )
}

#[cfg(test)]
mod tests;
