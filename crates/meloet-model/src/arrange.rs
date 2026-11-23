//! Changes to clips in the arrange view: move (also to other tracks), copy, duplicate right
//! after, copy and paste, trim an edge. Takes are never changed: a copy shares them, and a trim
//! only moves the clip's edges and the takes' offsets, so what is trimmed off comes back when
//! pulled out.
//!
//! Clips moved or copied together keep their distances and their rows. A group that would put a
//! clip off the tracks (or past [`MAX_TICK`]) does nothing and says [`EditError::OffTheSong`]; one
//! that would overlap a clip outside it says [`EditError::Overlap`].

use std::sync::Arc;

use crate::TrackId;
use crate::audio::AudioShape;
use crate::audio::clips::fit_fades;
use crate::edit::{Edit, EditError, Ids, MAX_TICK, clip_index, track_index, with_track};
use crate::history::StepKind;
use crate::song::{
    Clip, ClipId, ClipTake, ClipTakes, ControlPoint, Doc, Note, Take, TakeId, Track,
};
use crate::time::TickRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipEdge {
    Start,
    End,
}

/// An edit and the clips it placed: one for each clip given (or kept), in that order; the copies
/// when it made them.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipEdited {
    pub edit: Edit,
    pub clips: Vec<ClipId>,
    /// For each of `clips`, the clip it came from (itself when moved).
    pub from: Vec<ClipId>,
}

/// A clip kept by Mod C or Mod X, as it was then.
#[derive(Debug, Clone, PartialEq)]
pub struct KeptClip {
    pub clip: Arc<Clip>,
    /// Rows of the tracks below (above, when negative) the first clip kept.
    pub row: i64,
    /// Ticks after the start of the first clip kept.
    pub after: u64,
}

/// The clips there, each once, in the order given, with where they are. `NoSuchClip` when none
/// of them is there.
fn found(doc: &Doc, clips: &[ClipId]) -> Result<Vec<(ClipId, usize, usize)>, EditError> {
    let mut out: Vec<(ClipId, usize, usize)> = Vec::new();
    for &id in clips {
        if out.iter().any(|&(c, _, _)| c == id) {
            continue;
        }
        if let Ok((ti, ci)) = clip_index(doc, id) {
            out.push((id, ti, ci));
        }
    }
    match (out.is_empty(), clips.first()) {
        (true, Some(&first)) => Err(EditError::NoSuchClip(first)),
        (true, None) => Err(EditError::Unchanged),
        _ => Ok(out),
    }
}

/// Moves the clips together by `ticks` and by `rows` rows of the tracks; with `copy`, leaves
/// them and puts copies (new ids, the same takes) there. The group stops at the song's start.
/// Clips that are not there are left out.
pub fn move_clips(
    doc: &Doc,
    clips: &[ClipId],
    ticks: i64,
    rows: i32,
    copy: bool,
) -> Result<ClipEdited, EditError> {
    let group = found(doc, clips)?;
    let first = group
        .iter()
        .map(|&(_, ti, ci)| doc.tracks[ti].clips[ci].start)
        .min()
        .expect("one clip at least");
    let ticks = ticks.max(-(first as i64));
    if ticks == 0 && rows == 0 && !copy {
        return Err(EditError::Unchanged);
    }
    let mut placing = Vec::with_capacity(group.len());
    for &(_, ti, ci) in &group {
        let c = &doc.tracks[ti].clips[ci];
        let to = row_from(doc, ti, i64::from(rows))?;
        placing.push((c.clone(), to, c.start.saturating_add_signed(ticks)));
    }
    let out: Vec<ClipId> = if copy {
        Vec::new()
    } else {
        group.iter().map(|&(c, _, _)| c).collect()
    };
    let step = StepKind::MoveClips {
        count: group.len(),
        copy,
    };
    place(doc, &out, placing, step, Ids(doc.next_id))
}

/// Mod D: copies of the clips right after the group (the latest end among them), on their own
/// tracks, keeping their distances.
pub fn duplicate_clips(doc: &Doc, clips: &[ClipId]) -> Result<ClipEdited, EditError> {
    let group = found(doc, clips)?;
    let clip = |&(_, ti, ci): &(ClipId, usize, usize)| &doc.tracks[ti].clips[ci];
    let first = group.iter().map(|g| clip(g).start).min().expect("one");
    let last = group.iter().map(|g| clip(g).end).max().expect("one");
    let placing = group
        .iter()
        .map(|g| {
            let c = clip(g);
            (c.clone(), g.1, c.start - first + last)
        })
        .collect();
    place(
        doc,
        &[],
        placing,
        StepKind::DuplicateClips { count: group.len() },
        Ids(doc.next_id),
    )
}

/// 「くり返す」: `count` copies of the clips as one block (the earliest start to the latest end),
/// each right after the one before, on their own tracks, keeping their distances; new ids, the
/// same takes. All or nothing: `Overlap` when a copy would overlap a clip, `OffTheSong` past
/// [`MAX_TICK`]. Positions are ticks, so the copies follow on whatever the meter or the tempo.
pub fn repeat_clips(doc: &Doc, clips: &[ClipId], count: usize) -> Result<ClipEdited, EditError> {
    if count == 0 {
        return Err(EditError::Unchanged);
    }
    let group = found(doc, clips)?;
    let clip = |&(_, ti, ci): &(ClipId, usize, usize)| &doc.tracks[ti].clips[ci];
    let first = group.iter().map(|g| clip(g).start).min().expect("one");
    let last = group.iter().map(|g| clip(g).end).max().expect("one");
    let length = last - first;
    let count = count as u64;
    length
        .checked_mul(count)
        .and_then(|by| last.checked_add(by))
        .filter(|&end| end <= MAX_TICK)
        .ok_or(EditError::OffTheSong)?;
    let mut placing = Vec::new();
    for k in 1..=count {
        for g in &group {
            let c = clip(g);
            placing.push((c.clone(), g.1, c.start + length * k));
        }
    }
    let step = StepKind::RepeatClips {
        count: placing.len(),
    };
    place(doc, &[], placing, step, Ids(doc.next_id))
}

/// What Mod C keeps of the clips, the first being the earliest (the upper one of those that
/// start together).
pub fn copy_clips(doc: &Doc, clips: &[ClipId]) -> Result<Vec<KeptClip>, EditError> {
    let mut group = found(doc, clips)?;
    group.sort_by_key(|&(_, ti, ci)| (doc.tracks[ti].clips[ci].start, ti));
    let (_, t0, c0) = group[0];
    let first = doc.tracks[t0].clips[c0].start;
    Ok(group
        .into_iter()
        .map(|(_, ti, ci)| {
            let c = &doc.tracks[ti].clips[ci];
            KeptClip {
                clip: c.clone(),
                row: ti as i64 - t0 as i64,
                after: c.start - first,
            }
        })
        .collect())
}

/// Puts copies (new ids, the same takes) of the kept clips in, the first on `track` at `at`.
/// The takes kept must be of this song: [`History`](crate::History) never gives their ids again.
pub fn paste_clips(
    doc: &Doc,
    kept: &[KeptClip],
    track: TrackId,
    at: u64,
) -> Result<ClipEdited, EditError> {
    let base = track_index(doc, track)?;
    if kept.is_empty() {
        return Err(EditError::Unchanged);
    }
    let mut placing = Vec::with_capacity(kept.len());
    for k in kept {
        let start = at.checked_add(k.after).ok_or(EditError::OffTheSong)?;
        placing.push((k.clip.clone(), row_from(doc, base, k.row)?, start));
    }
    let step = StepKind::PasteClips { count: kept.len() };
    place(doc, &[], placing, step, Ids(doc.next_id))
}

/// The index of the track `rows` rows from the one at `ti`; `OffTheSong` past the first or the
/// last.
fn row_from(doc: &Doc, ti: usize, rows: i64) -> Result<usize, EditError> {
    usize::try_from(ti as i64 + rows)
        .ok()
        .filter(|&to| to < doc.tracks.len())
        .ok_or(EditError::OffTheSong)
}

/// Takes the clips `out` away, then puts each of `placing` (a clip, the index of its track, its
/// start) in: the clip itself when it was taken away, else a copy with a new id from `ids`.
fn place(
    doc: &Doc,
    out: &[ClipId],
    placing: Vec<(Arc<Clip>, usize, u64)>,
    step: StepKind,
    mut ids: Ids,
) -> Result<ClipEdited, EditError> {
    let mut tracks: Vec<Vec<Arc<Clip>>> = doc.tracks.iter().map(|t| t.clips.clone()).collect();
    let mut changed = vec![false; doc.tracks.len()];
    for (ti, clips) in tracks.iter_mut().enumerate() {
        let before = clips.len();
        clips.retain(|c| !out.contains(&c.id));
        changed[ti] = clips.len() != before;
    }
    let mut placed = Vec::with_capacity(placing.len());
    let mut from = Vec::with_capacity(placing.len());
    for (c, to, start) in placing {
        let end = start
            .checked_add(c.end - c.start)
            .filter(|&e| e <= MAX_TICK)
            .ok_or(EditError::OffTheSong)?;
        let range = TickRange::new(start, end).expect("a clip is never empty");
        if doc.tracks[to].kind.is_vocal() == c.is_keys() {
            return Err(EditError::WrongKind);
        }
        let clips = &mut tracks[to];
        if clips.iter().any(|o| o.range().overlaps(range)) {
            return Err(EditError::Overlap);
        }
        let id = if out.contains(&c.id) {
            c.id
        } else {
            ClipId(ids.next()?)
        };
        let at = clips.partition_point(|o| o.start < start);
        clips.insert(
            at,
            Arc::new(Clip {
                id,
                start,
                end,
                ..Clip::clone(&c)
            }),
        );
        changed[to] = true;
        placed.push(id);
        from.push(c.id);
    }
    let mut new = doc.clone();
    for (ti, clips) in tracks.into_iter().enumerate() {
        if changed[ti] {
            new.tracks[ti] = Arc::new(Track {
                clips,
                ..Track::clone(&doc.tracks[ti])
            });
        }
    }
    new.next_id = ids.0;
    Ok(ClipEdited {
        edit: Edit { doc: new, step },
        clips: placed,
        from,
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
    Ok(Edit {
        doc: with_edges(doc, (ti, ci), start, end),
        step: StepKind::TrimClip { clip, edge },
    })
}

/// The clip with other edges; what it holds stays where it is in the song. The fades of a
/// vocal clip give way to a shorter clip.
fn with_edges(doc: &Doc, (ti, ci): (usize, usize), start: u64, end: u64) -> Doc {
    let track = &doc.tracks[ti];
    let c = &track.clips[ci];
    let shift = start as i64 - c.start as i64;
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        start,
        end,
        takes: fitted(c.takes.with_offsets(|offset| offset - shift), end - start),
        ..Clip::clone(c)
    });
    let mut new = doc.clone();
    new.tracks[ti] = Arc::new(Track {
        clips,
        ..Track::clone(track)
    });
    new
}

fn fitted(takes: ClipTakes, length: u64) -> ClipTakes {
    match takes {
        ClipTakes::Vocal { takes, shape } => ClipTakes::Vocal {
            takes,
            shape: fit_fades(shape, length),
        },
        keys => keys,
    }
}

/// 「位置」 typed: the clip starts at `start`, as it is. `Overlap` when another clip of its
/// track is in the way.
pub fn set_clip_start(doc: &Doc, clip: ClipId, start: u64) -> Result<ClipEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let from = doc.tracks[ti].clips[ci].start;
    let ticks = i64::try_from(start).map_err(|_| EditError::OffTheSong)? - from as i64;
    move_clips(doc, &[clip], ticks, 0, false)
}

/// 「長さ」 typed: the clip's end moves so that it is `length` ticks long (1 at least).
/// `Overlap` when the next clip of its track is in the way.
pub fn set_clip_length(doc: &Doc, clip: ClipId, length: u64) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let track = &doc.tracks[ti];
    let c = &track.clips[ci];
    let end = c.start.saturating_add(length.max(1)).min(MAX_TICK);
    if end == c.end {
        return Err(EditError::Unchanged);
    }
    if track.clips.get(ci + 1).is_some_and(|next| next.start < end) {
        return Err(EditError::Overlap);
    }
    Ok(Edit {
        doc: with_edges(doc, (ti, ci), c.start, end),
        step: StepKind::TrimClip {
            clip,
            edge: ClipEdge::End,
        },
    })
}

/// What [`split_clips`] made: for each clip cut, in the order of the tracks, the left half
/// (the clip's id) and the right half (a new id).
#[derive(Debug, Clone, PartialEq)]
pub struct ClipsSplit {
    pub edit: Edit,
    pub left: Vec<ClipId>,
    pub right: Vec<ClipId>,
}

/// S: cuts each of the clips that `at` is inside of in two there, as one step. Clips that
/// are not there, or that `at` is not inside of (on an edge is not inside), are left as
/// they are; `Unchanged` when that is all of them.
///
/// Both halves of a vocal clip play the same takes (the same `Arc`s); the fade-in stays on
/// the left half and the fade-out on the right one. Of a keys clip the right half keeps
/// the takes too, and the left half gets a take with a new id for every take that has a
/// note over the cut: there the note ends at the cut.
pub fn split_clips(doc: &Doc, clips: &[ClipId], at: u64) -> Result<ClipsSplit, EditError> {
    let mut new = doc.clone();
    let mut ids = Ids(doc.next_id);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (ti, track) in doc.tracks.iter().enumerate() {
        if !(track.clips.iter()).any(|c| clips.contains(&c.id) && c.start < at && at < c.end) {
            continue;
        }
        let mut halves = Vec::with_capacity(track.clips.len() + 1);
        for c in &track.clips {
            if !(clips.contains(&c.id) && c.start < at && at < c.end) {
                halves.push(c.clone());
                continue;
            }
            let moved = (at - c.start) as i64;
            let (left_takes, right_takes) = match &c.takes {
                ClipTakes::Vocal { takes, shape } => (
                    ClipTakes::Vocal {
                        takes: takes.clone(),
                        shape: fit_fades(
                            AudioShape {
                                fade_out: 0,
                                ..*shape
                            },
                            at - c.start,
                        ),
                    },
                    ClipTakes::Vocal {
                        takes: takes.clone(),
                        shape: fit_fades(
                            AudioShape {
                                fade_in: 0,
                                ..*shape
                            },
                            c.end - at,
                        ),
                    },
                ),
                ClipTakes::Keys(takes) => {
                    let mut ended = Vec::with_capacity(takes.len());
                    for ct in takes {
                        ended.push(ClipTake {
                            take: ended_at(
                                &ct.take,
                                i128::from(at) - i128::from(c.zero(ct)),
                                &mut ids,
                            )?,
                            offset: ct.offset,
                        });
                    }
                    (ClipTakes::Keys(ended), ClipTakes::Keys(takes.clone()))
                }
            };
            let id = ClipId(ids.next()?);
            left.push(c.id);
            right.push(id);
            halves.push(Arc::new(Clip {
                end: at,
                takes: left_takes,
                ..Clip::clone(c)
            }));
            halves.push(Arc::new(Clip {
                id,
                start: at,
                takes: right_takes.with_offsets(|offset| offset - moved),
                ..Clip::clone(c)
            }));
        }
        new.tracks[ti] = Arc::new(Track {
            clips: halves,
            ..Track::clone(track)
        });
    }
    if left.is_empty() {
        return Err(EditError::Unchanged);
    }
    new.next_id = ids.0;
    let step = StepKind::SplitClips { count: left.len() };
    Ok(ClipsSplit {
        edit: Edit { doc: new, step },
        left,
        right,
    })
}

/// The take with every note that sounds over `cut` (a tick of the take) ended there: the
/// take itself when it has none, else one with a new id.
fn ended_at(take: &Arc<Take>, cut: i128, ids: &mut Ids) -> Result<Arc<Take>, EditError> {
    let over = |n: &Note| i128::from(n.start) < cut && i128::from(n.end()) > cut;
    if !take.notes.iter().any(over) {
        return Ok(take.clone());
    }
    let notes: Vec<Note> = (take.notes.iter())
        .map(|n| match over(n) {
            true => Note {
                length: (cut - i128::from(n.start)) as u64,
                ..*n
            },
            false => *n,
        })
        .collect();
    Ok(Arc::new(Take {
        id: TakeId(ids.next()?),
        notes: crate::ChunkedVec::sharing(&notes, &take.notes),
        ..Take::clone(take)
    }))
}

/// A clip's name, keys or vocal. Spaces around it are left out; an empty name or the same one
/// is `Unchanged`.
pub fn rename_clip(doc: &Doc, clip: ClipId, name: &str) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let c = &doc.tracks[ti].clips[ci];
    let name = name.trim();
    if name.is_empty() || name == c.name {
        return Err(EditError::Unchanged);
    }
    let track = &doc.tracks[ti];
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        name: name.to_owned(),
        ..Clip::clone(c)
    });
    Ok(Edit {
        doc: with_track(
            doc,
            ti,
            Track {
                clips,
                ..Track::clone(track)
            },
        ),
        step: StepKind::RenameClip { clip },
    })
}

/// Two or more keys clips of one track become one, from the start of the earliest to the end
/// of the latest, with the id and the name of the earliest. Its one take (a new id) holds the
/// notes and control points of each clip's shown take that are inside that clip; a note past
/// its clip's end is cut there. The other takes are not kept.
///
/// `NotKeys` with a vocal clip among them. `Unchanged` with fewer than two, clips of different
/// tracks, or a clip not given lying between them. `NoSuchClip` for a clip that is not there.
pub fn join_clips(doc: &Doc, clips: &[ClipId]) -> Result<(Edit, ClipId), EditError> {
    let mut at: Vec<(usize, usize)> = Vec::with_capacity(clips.len());
    for &id in clips {
        let place = clip_index(doc, id)?;
        if !at.contains(&place) {
            at.push(place);
        }
    }
    let Some(&(ti, _)) = at.first() else {
        return Err(EditError::Unchanged);
    };
    if at.len() < 2 || at.iter().any(|&(t, _)| t != ti) {
        return Err(EditError::Unchanged);
    }
    let track = &doc.tracks[ti];
    if at.iter().any(|&(_, ci)| !track.clips[ci].is_keys()) {
        return Err(EditError::NotKeys);
    }
    // The clips of a track are in order of start and never overlap: the ones given must be a
    // run of neighbours.
    let mut indexes: Vec<usize> = at.iter().map(|&(_, ci)| ci).collect();
    indexes.sort_unstable();
    let (low, high) = (indexes[0], indexes[indexes.len() - 1]);
    if high - low + 1 != indexes.len() {
        return Err(EditError::Unchanged);
    }
    let joined = &track.clips[low..=high];
    let first = &joined[0];
    let (start, end) = (first.start, joined[joined.len() - 1].end);
    let mut notes: Vec<Note> = Vec::new();
    let mut controls: Vec<ControlPoint> = Vec::new();
    for c in joined {
        let shown = c.active_take();
        let zero = c.zero(shown);
        let inside = |t: u64| {
            let at = zero + t as i64;
            (c.start as i64..c.end as i64)
                .contains(&at)
                .then_some(at as u64)
        };
        for n in shown.take.notes.iter() {
            if let Some(s) = inside(n.start) {
                notes.push(Note {
                    start: s - start,
                    length: n.length.min(c.end - s),
                    ..*n
                });
            }
        }
        for p in shown.take.controls.iter() {
            if let Some(t) = inside(p.tick) {
                controls.push(ControlPoint {
                    tick: t - start,
                    ..*p
                });
            }
        }
    }
    notes.sort_by_key(|n| (n.start, n.key));
    controls.sort_by_key(|p| p.tick);
    let mut ids = Ids(doc.next_id);
    let take = Take {
        id: TakeId(ids.next()?),
        length: end - start,
        notes: notes.into(),
        controls: controls.into(),
        origin: None,
    };
    let clip = Arc::new(Clip {
        id: first.id,
        name: first.name.clone(),
        start,
        end,
        takes: ClipTakes::Keys(vec![ClipTake {
            take: Arc::new(take),
            offset: 0,
        }]),
        active: 0,
    });
    let mut kept = track.clips.clone();
    kept.splice(low..=high, [clip]);
    let mut new = with_track(
        doc,
        ti,
        Track {
            clips: kept,
            ..Track::clone(track)
        },
    );
    new.next_id = ids.0;
    let step = StepKind::JoinClips {
        clip: first.id,
        count: joined.len(),
    };
    Ok((Edit { doc: new, step }, first.id))
}

#[cfg(test)]
mod tests;
