//! Changes to clips in the arrange view: move (also to another track), copy, duplicate right
//! after, trim an edge. Takes are never changed: a copy shares them, and a trim only moves the
//! clip's edges and the takes' offsets, so what is trimmed off comes back when pulled out.

use std::sync::Arc;

use crate::TrackId;
use crate::edit::{Edit, EditError, Ids, MAX_TICK, clip_index, track_index};
use crate::history::StepKind;
use crate::song::{Clip, ClipId, ClipTake, Doc, Track};
use crate::time::TickRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipEdge {
    Start,
    End,
}

/// An edit and the clip it leaves selected (the copy when it made one).
#[derive(Debug, Clone, PartialEq)]
pub struct ClipEdited {
    pub edit: Edit,
    pub clip: ClipId,
}

/// Moves the clip by `ticks` onto `track`; with `copy`, leaves it and puts a copy (a new id,
/// the same takes) there. It stops at the song's start. Nothing happens where it would overlap
/// another clip.
pub fn move_clip(
    doc: &Doc,
    clip: ClipId,
    ticks: i64,
    track: TrackId,
    copy: bool,
) -> Result<ClipEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let to = track_index(doc, track)?;
    let c = &doc.tracks[ti].clips[ci];
    let start = c.start.saturating_add_signed(ticks);
    if start == c.start && to == ti && !copy {
        return Err(EditError::Unchanged);
    }
    let (out, step) = if copy {
        (None, StepKind::CopyClip { clip, track })
    } else {
        (Some(clip), StepKind::MoveClip { clip, track })
    };
    put(doc, (ti, ci), start, to, out, step)
}

/// Mod D: a copy right after the clip, on its track. Nothing happens where it would overlap.
pub fn duplicate_clip(doc: &Doc, clip: ClipId) -> Result<ClipEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let c = &doc.tracks[ti].clips[ci];
    put(
        doc,
        (ti, ci),
        c.end,
        ti,
        None,
        StepKind::DuplicateClip { clip },
    )
}

/// Puts the clip at `(ti, ci)`, from `start`, on the track at `to`. `out`: the clip taken away
/// from where it was (a move), or `None` for a copy.
fn put(
    doc: &Doc,
    (ti, ci): (usize, usize),
    start: u64,
    to: usize,
    out: Option<ClipId>,
    step: StepKind,
) -> Result<ClipEdited, EditError> {
    let c = &doc.tracks[ti].clips[ci];
    let end = start
        .checked_add(c.end - c.start)
        .filter(|&e| e <= MAX_TICK)
        .ok_or(EditError::Overlap)?;
    let range = TickRange::new(start, end).expect("a clip is never empty");
    let dest = &doc.tracks[to];
    if dest
        .clips
        .iter()
        .any(|o| Some(o.id) != out && o.range().overlaps(range))
    {
        return Err(EditError::Overlap);
    }
    let mut ids = Ids(doc.next_id);
    let id = match out {
        Some(id) => id,
        None => ClipId(ids.next()?),
    };
    let placed = Arc::new(Clip {
        id,
        start,
        end,
        ..Clip::clone(c)
    });
    let mut new = doc.clone();
    if out.is_some() {
        let mut clips = doc.tracks[ti].clips.clone();
        clips.remove(ci);
        new.tracks[ti] = Arc::new(Track {
            clips,
            ..Track::clone(&doc.tracks[ti])
        });
    }
    let mut clips = new.tracks[to].clips.clone();
    let at = clips.partition_point(|o| o.start < start);
    clips.insert(at, placed);
    new.tracks[to] = Arc::new(Track {
        clips,
        ..Track::clone(&new.tracks[to])
    });
    new.next_id = ids.0;
    Ok(ClipEdited {
        edit: Edit { doc: new, step },
        clip: id,
    })
}

/// Moves one edge of the clip by `ticks`, in or out. The notes stay where they are in the song:
/// moving the start changes every take's offset by as much. The clip stays 1 tick or longer,
/// at the song's start or later, and stops short of the clips next to it.
pub fn trim_clip(doc: &Doc, clip: ClipId, edge: ClipEdge, ticks: i64) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let track = &doc.tracks[ti];
    let c = &track.clips[ci];
    let (start, end) = match edge {
        ClipEdge::Start => {
            let floor = ci.checked_sub(1).map_or(0, |p| track.clips[p].end);
            let start = c.start.saturating_add_signed(ticks).clamp(floor, c.end - 1);
            (start, c.end)
        }
        ClipEdge::End => {
            let ceiling = track.clips.get(ci + 1).map_or(MAX_TICK, |n| n.start);
            let end = c
                .end
                .saturating_add_signed(ticks)
                .clamp(c.start + 1, ceiling);
            (c.start, end)
        }
    };
    if (start, end) == (c.start, c.end) {
        return Err(EditError::Unchanged);
    }
    let shift = start as i64 - c.start as i64;
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        start,
        end,
        takes: c
            .takes
            .iter()
            .map(|ct| ClipTake {
                take: ct.take.clone(),
                offset: ct.offset - shift,
            })
            .collect(),
        ..Clip::clone(c)
    });
    let mut new = doc.clone();
    new.tracks[ti] = Arc::new(Track {
        clips,
        ..Track::clone(track)
    });
    Ok(Edit {
        doc: new,
        step: StepKind::TrimClip { clip, edge },
    })
}

#[cfg(test)]
mod tests;
