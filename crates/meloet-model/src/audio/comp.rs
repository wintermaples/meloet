//! 「いいとこ取り」 of a vocal clip (round 12, A8): which take sounds where.
//!
//! `Clip::comp` is empty (the shown take sounds all along) or covers the whole clip: the
//! pieces are in the order of `from` (ticks from the clip's start), the first at 0, each
//! before the clip's end and naming a take the clip has, and no two in a row name one take.
//! Every take sounds at its own offset, as it would as the shown take, so the parts stay in
//! time. The takes never change; only the pieces do.

use std::sync::Arc;

use super::CompPiece;
use crate::edit::{Edit, EditError, clip_index, with_track};
use crate::history::StepKind;
use crate::song::{Clip, ClipId, Doc, Track};

/// Whether `pieces` may be the comp of a clip `length` ticks long with `takes` takes. An
/// empty list is no comp, which every clip may have.
pub fn comp_is_valid(pieces: &[CompPiece], length: u64, takes: usize) -> bool {
    pieces.first().is_none_or(|p| p.from == 0)
        && pieces.iter().all(|p| p.from < length && p.take < takes)
        && (pieces.windows(2)).all(|w| w[0].from < w[1].from && w[0].take != w[1].take)
}

/// The pieces of a comp for a clip whose start moved by `shift` ticks (later is above 0) and
/// that is `length` long now: what sounded at a place of the song still sounds there. A
/// piece that is no longer inside the clip goes, and the first one reaches back to the
/// clip's start.
pub(crate) fn comp_window(comp: &[CompPiece], shift: i64, length: u64) -> Vec<CompPiece> {
    let mut out: Vec<CompPiece> = Vec::with_capacity(comp.len());
    for (i, piece) in comp.iter().enumerate() {
        let from = match i {
            0 => 0,
            _ => (i128::from(piece.from) - i128::from(shift)).clamp(0, i128::from(u64::MAX)) as u64,
        };
        if from >= length {
            break;
        }
        // One that starts where the one before it does took its place.
        if out.last().is_some_and(|last| last.from == from) {
            out.pop();
        }
        if out.last().is_none_or(|last| last.take != piece.take) {
            out.push(CompPiece {
                from,
                take: piece.take,
            });
        }
    }
    out
}

/// The comp of a clip that lost some of its takes: `kept[i]` is the new place of take `i`.
/// No comp when a piece names a take that went.
pub(crate) fn comp_renumbered(comp: &[CompPiece], kept: &[Option<usize>]) -> Vec<CompPiece> {
    (comp.iter())
        .map(|p| {
            Some(CompPiece {
                from: p.from,
                take: (*kept.get(p.take)?)?,
            })
        })
        .collect::<Option<Vec<CompPiece>>>()
        .unwrap_or_default()
}

/// The comp of the one clip that `clips` become (a recording went over them): it starts at
/// `start` and has the takes of each in turn. A clip without a comp sounds as its shown
/// take. None when none of them has a comp.
pub(crate) fn comp_joined(clips: &[Arc<Clip>], start: u64) -> Vec<CompPiece> {
    if clips.iter().all(|c| c.comp.is_empty()) {
        return Vec::new();
    }
    let mut out: Vec<CompPiece> = Vec::new();
    let mut base = 0;
    for c in clips {
        let whole = [CompPiece {
            from: 0,
            take: c.active,
        }];
        let pieces = if c.comp.is_empty() {
            &whole[..]
        } else {
            &c.comp
        };
        for p in pieces {
            let piece = CompPiece {
                from: match out.is_empty() {
                    true => 0,
                    false => c.start.saturating_sub(start) + p.from,
                },
                take: base + p.take,
            };
            if out.last().is_none_or(|last| last.take != piece.take) {
                out.push(piece);
            }
        }
        base += c.takes.len();
    }
    match out.windows(2).all(|w| w[0].from < w[1].from) {
        true => out,
        false => Vec::new(),
    }
}

fn with_comp(doc: &Doc, (ti, ci): (usize, usize), comp: Vec<CompPiece>) -> Doc {
    let track = &doc.tracks[ti];
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        comp,
        ..Clip::clone(&clips[ci])
    });
    with_track(
        doc,
        ti,
        Track {
            clips,
            ..Track::clone(track)
        },
    )
}

/// 「いいとこ取り」: the comp of `clip` becomes `pieces`, when the clip still starts at `start`
/// and ends at `end` (what the pieces were laid out for).
///
/// `WrongKind` for a keys clip. `Unchanged` for a clip with fewer than two takes, other
/// edges, pieces that break the rules of a comp or are none, and pieces that say what the
/// clip plays already.
pub fn set_comp(
    doc: &Doc,
    clip: ClipId,
    (start, end): (u64, u64),
    pieces: &[CompPiece],
) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let c = &doc.tracks[ti].clips[ci];
    if c.is_keys() {
        return Err(EditError::WrongKind);
    }
    let now = [CompPiece {
        from: 0,
        take: c.active,
    }];
    let plays = if c.comp.is_empty() { &now[..] } else { &c.comp };
    if c.takes.len() < 2
        || (c.start, c.end) != (start, end)
        || pieces.is_empty()
        || !comp_is_valid(pieces, end - start, c.takes.len())
        || pieces == plays
    {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: with_comp(doc, (ti, ci), pieces.to_vec()),
        step: StepKind::SetComp { clip },
    })
}

/// 「もとに戻す」 of 「いいとこ取り」: the comp goes and the shown take sounds whole.
/// `Unchanged` for a clip with no comp.
pub fn clear_comp(doc: &Doc, clip: ClipId) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    if doc.tracks[ti].clips[ci].comp.is_empty() {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: with_comp(doc, (ti, ci), Vec::new()),
        step: StepKind::ClearComp { clip },
    })
}

#[cfg(test)]
mod tests;
