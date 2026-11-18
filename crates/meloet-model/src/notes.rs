//! Changes to the notes of a clip's shown take. Each one makes a new take (a new id) and puts it
//! in place of the shown one, so the clip keeps its number of takes; the old take stays as it
//! was, for undo.
//!
//! Notes given and returned here are in song positions; the take holds them from its tick 0.
//! When the shown take's tick 0 is before the song's (its head trimmed off and the clip moved
//! to the song's start), the work is done on the clip [`lifted`] by that much, so that every
//! note of the take has a position.
//! A changed take keeps its notes in order of start, then key, then the order they had. Notes
//! stay inside the clip; the take grows (never shrinks) to hold them, and its offset follows.

use std::sync::Arc;

use crate::TrackId;
use crate::arrange::ClipEdge;
use crate::chunked::ChunkedVec;
use crate::edit::{Edit, EditError, Ids, clip_index, track_index, with_track};
use crate::history::StepKind;
use crate::song::{
    Clip, ClipId, ClipTake, ControlPoint, Doc, Note, Take, TakeId, TakeOrigin, Track,
};

/// Bars a clip made for a written note gets, when nothing comes before that.
pub const NEW_CLIP_BARS: u64 = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct NotesEdited {
    pub edit: Edit,
    pub clip: ClipId,
    /// Indexes, in the new take, of the notes written, moved, copied or pasted; of control
    /// points when `of_controls`.
    pub selected: Vec<usize>,
    pub of_controls: bool,
}

/// The fields of the note list and the "選んだノート" panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteField {
    Start,
    Key,
    Length,
    Velocity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAt {
    Clip(ClipId),
    /// The clip of the track that holds the note's start, or else a new one.
    Track(TrackId),
}

/// How far the shown take's tick 0 is before the song's tick 0: 0 when it is not.
pub(crate) fn lift(clip: &Clip) -> u64 {
    (-clip.zero(clip.active_take())).max(0) as u64
}

/// The clip later by [`lift`], and the lift. Its shown take's tick 0 is at 0 or later.
pub(crate) fn lifted(clip: &Clip) -> (Clip, u64) {
    let l = lift(clip);
    (
        Clip {
            start: clip.start + l,
            end: clip.end + l,
            ..clip.clone()
        },
        l,
    )
}

/// Where the shown take's tick 0 is in the song. Only for a clip whose [`lift`] is 0.
fn zero(clip: &Clip) -> u64 {
    let z = clip.zero(clip.active_take());
    debug_assert!(z >= 0, "lift the clip first");
    z as u64
}

/// The notes of the clip's shown take, in song positions, in the take's order. Only for a clip
/// whose [`lift`] is 0.
pub fn song_notes(clip: &Clip) -> Vec<Note> {
    let shown = clip.active_take();
    let origin = zero(clip);
    shown
        .take
        .notes
        .iter()
        .map(|n| Note {
            start: origin + n.start,
            ..*n
        })
        .collect()
}

/// The control points of the clip's shown take, in song positions, in the take's order. Only
/// for a clip whose [`lift`] is 0.
pub fn song_controls(clip: &Clip) -> Vec<ControlPoint> {
    let shown = clip.active_take();
    let origin = zero(clip);
    shown
        .take
        .controls
        .iter()
        .map(|c| ControlPoint {
            tick: origin + c.tick,
            ..*c
        })
        .collect()
}

/// What Mod C keeps: the notes at `indexes`, each start counted from the earliest of them.
pub fn clipboard(clip: &Clip, indexes: &[usize]) -> Result<Vec<Note>, EditError> {
    let notes = song_notes(&lifted(clip).0);
    let picked = pick(indexes, notes.len())?;
    let chosen: Vec<Note> = notes
        .into_iter()
        .zip(picked)
        .filter_map(|(n, p)| p.then_some(n))
        .collect();
    let first = chosen.iter().map(|n| n.start).min().unwrap_or(0);
    Ok(chosen
        .into_iter()
        .map(|n| Note {
            start: n.start - first,
            from: None,
            ..n
        })
        .collect())
}

/// The write tool. The note is cut at the end of the clip.
pub fn write_note(doc: &Doc, at: WriteAt, note: Note) -> Result<NotesEdited, EditError> {
    if note.length == 0 || note.key > 127 || !(1..=127).contains(&note.velocity) {
        return Err(EditError::BadNote);
    }
    let note = Note { from: None, ..note };
    let clip = match at {
        WriteAt::Clip(id) => id,
        WriteAt::Track(track) => {
            let t = &doc.tracks[track_index(doc, track)?];
            match t
                .clips
                .iter()
                .find(|c| c.start <= note.start && note.start < c.end)
            {
                Some(c) => c.id,
                None => return write_in_new_clip(doc, track, note),
            }
        }
    };
    let (ti, ci) = clip_index(doc, clip)?;
    let c = &doc.tracks[ti].clips[ci];
    if note.start < c.start || note.start >= c.end {
        return Err(EditError::OutOfClip);
    }
    let (c, l) = lifted(c);
    let note = Note {
        start: note.start + l,
        length: note.length.min(c.end - note.start - l),
        ..note
    };
    let mut notes = unmarked(&c);
    notes.push((note, true));
    put_notes(
        doc,
        (ti, ci),
        notes,
        StepKind::WriteNote {
            clip,
            clip_made: false,
        },
    )
}

/// From the bar of the note (or the end of the clip before it), `NEW_CLIP_BARS` bars long or up
/// to the next clip, named after the track.
fn write_in_new_clip(doc: &Doc, track: TrackId, note: Note) -> Result<NotesEdited, EditError> {
    let ti = track_index(doc, track)?;
    let old = &doc.tracks[ti];
    let per_bar = doc.meter.ticks_per_bar();
    let head = note.start / per_bar * per_bar;
    let start = old
        .clips
        .iter()
        .filter(|c| c.end <= note.start)
        .map(|c| c.end)
        .fold(head, u64::max);
    let end = old
        .clips
        .iter()
        .filter(|c| c.start > note.start)
        .map(|c| c.start)
        .fold(head.saturating_add(NEW_CLIP_BARS * per_bar), u64::min);
    let mut ids = Ids(doc.next_id);
    let id = ClipId(ids.next()?);
    let take = Take {
        id: TakeId(ids.next()?),
        length: end - start,
        notes: vec![Note {
            start: note.start - start,
            length: note.length.min(end - note.start),
            ..note
        }]
        .into(),
        controls: ChunkedVec::default(),
        origin: None,
    };
    let clip = Arc::new(Clip {
        id,
        name: old.name.clone(),
        start,
        end,
        takes: vec![ClipTake {
            take: Arc::new(take),
            offset: 0,
        }],
        active: 0,
    });
    let mut clips = old.clips.clone();
    let at = clips.partition_point(|c| c.start < start);
    clips.insert(at, clip);
    let mut new = with_track(
        doc,
        ti,
        Track {
            clips,
            ..Track::clone(old)
        },
    );
    new.next_id = ids.0;
    Ok(NotesEdited {
        edit: Edit {
            doc: new,
            step: StepKind::WriteNote {
                clip: id,
                clip_made: true,
            },
        },
        clip: id,
        selected: vec![0],
        of_controls: false,
    })
}

/// Moves the notes by `ticks` and `keys`, or with `copy` adds moved copies. The notes move
/// together and stop where the first of them would leave the clip or the keys 0..=127.
pub fn move_notes(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    ticks: i64,
    keys: i32,
    copy: bool,
) -> Result<NotesEdited, EditError> {
    move_or_copy(doc, clip, indexes, ticks, keys, copy, |count| {
        StepKind::MoveNotes { clip, count, copy }
    })
}

fn move_or_copy(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    ticks: i64,
    keys: i32,
    copy: bool,
    step: impl FnOnce(usize) -> StepKind,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let (c, _) = lifted(&doc.tracks[ti].clips[ci]);
    let notes = song_notes(&c);
    let picked = pick(indexes, notes.len())?;
    let chosen = || {
        notes
            .iter()
            .zip(&picked)
            .filter_map(|(n, &p)| p.then_some(n))
    };
    let first = chosen()
        .map(|n| n.start)
        .min()
        .expect("picked one at least");
    let last = chosen()
        .map(|n| n.end())
        .max()
        .expect("picked one at least");
    let low = chosen().map(|n| n.key).min().expect("picked one at least");
    let high = chosen().map(|n| n.key).max().expect("picked one at least");
    // A note already outside is not pulled in; it only cannot go further out.
    let before = c.start as i64 - first as i64;
    let after = c.end as i64 - last as i64;
    let ticks = ticks.clamp(before.min(0), after.max(0));
    let keys = keys.clamp(-i32::from(low), 127 - i32::from(high));
    if ticks == 0 && keys == 0 && !copy {
        return Err(EditError::Unchanged);
    }
    let moved = |n: &Note| Note {
        start: (n.start as i64 + ticks) as u64,
        key: (i32::from(n.key) + keys) as u8,
        ..*n
    };
    let count = indexes_count(&picked);
    let out: Vec<(Note, bool)> = if copy {
        notes
            .iter()
            .map(|n| (*n, false))
            .chain(chosen().map(|n| {
                (
                    Note {
                        from: None,
                        ..moved(n)
                    },
                    true,
                )
            }))
            .collect()
    } else {
        notes
            .iter()
            .zip(&picked)
            .map(|(n, &p)| if p { (moved(n), true) } else { (*n, false) })
            .collect()
    };
    put_notes(doc, (ti, ci), out, step(count))
}

/// Every note moves its `edge` by `ticks`, the other edge staying, each note staying 1 tick or
/// longer and inside the clip. A note already past the clip's edge only cannot go further out.
pub fn resize_notes(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    edge: ClipEdge,
    ticks: i64,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let c = lifted(&doc.tracks[ti].clips[ci]).0;
    change_each(
        doc,
        (ti, ci),
        indexes,
        |count| StepKind::ResizeNotes { clip, count },
        |n| match edge {
            ClipEdge::End => {
                let length = n.length as i64 + ticks;
                let length = if ticks >= 0 {
                    length.min(n.length.max(c.end.saturating_sub(n.start)) as i64)
                } else {
                    length.max(1)
                };
                Note {
                    length: length as u64,
                    ..n
                }
            }
            ClipEdge::Start => {
                let end = n.end() as i64;
                let start = n.start as i64 + ticks;
                let start = if ticks <= 0 {
                    start.max(n.start.min(c.start) as i64)
                } else {
                    start.min(end - 1)
                };
                Note {
                    start: start as u64,
                    length: (end - start) as u64,
                    ..n
                }
            }
        },
    )
}

/// Each velocity moves by `delta` and stops at 1 and 127.
pub fn change_velocity(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    delta: i32,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    change_each(
        doc,
        (ti, ci),
        indexes,
        |count| StepKind::ChangeVelocity { clip, count },
        |n| Note {
            velocity: (i32::from(n.velocity) + delta).clamp(1, 127) as u8,
            ..n
        },
    )
}

/// `cut`: Mod X; the caller keeps the notes first with [`clipboard`]. Nothing stays selected.
pub fn delete_notes(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    cut: bool,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let notes = song_notes(&lifted(&doc.tracks[ti].clips[ci]).0);
    let picked = pick(indexes, notes.len())?;
    let count = indexes_count(&picked);
    let out = notes
        .into_iter()
        .zip(picked)
        .filter_map(|(n, p)| (!p).then_some((n, false)))
        .collect();
    put_notes(
        doc,
        (ti, ci),
        out,
        StepKind::DeleteNotes { clip, count, cut },
    )
}

/// Puts `notes` (from [`clipboard`]) in the clip with the earliest at `at`. Notes that would not
/// be wholly inside the clip are left out.
pub fn paste_notes(
    doc: &Doc,
    clip: ClipId,
    notes: &[Note],
    at: u64,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let (c, l) = lifted(&doc.tracks[ti].clips[ci]);
    let pasted: Vec<(Note, bool)> = notes
        .iter()
        .filter_map(|n| {
            let start = at.checked_add(l)?.checked_add(n.start)?;
            let fits = start >= c.start
                && start.checked_add(n.length)? <= c.end
                && n.length > 0
                && n.key <= 127
                && (1..=127).contains(&n.velocity);
            fits.then_some((
                Note {
                    start,
                    from: None,
                    ..*n
                },
                true,
            ))
        })
        .collect();
    if pasted.is_empty() {
        return Err(EditError::Unchanged);
    }
    let count = pasted.len();
    let mut out = unmarked(&c);
    out.extend(pasted);
    put_notes(doc, (ti, ci), out, StepKind::PasteNotes { clip, count })
}

/// The note list and the "選んだノート" panel. `Length` and `Velocity` set every note to
/// `value`; `Start` puts the earliest note at `value` (a song position), `Key` the lowest on
/// `value`, the others keeping their distance. Values stop at what the clip and the keys allow.
pub fn set_notes_field(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    field: NoteField,
    value: i64,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let (c, l) = lifted(&doc.tracks[ti].clips[ci]);
    let step = |count| StepKind::SetNotesField { clip, count };
    let notes = song_notes(&c);
    let picked = pick(indexes, notes.len())?;
    let chosen = || {
        notes
            .iter()
            .zip(&picked)
            .filter_map(|(n, &p)| p.then_some(n))
    };
    match field {
        NoteField::Start => {
            let first = chosen().map(|n| n.start).min().expect("picked one");
            let ticks = value.saturating_add(l as i64).saturating_sub(first as i64);
            move_or_copy(doc, clip, indexes, ticks, 0, false, step)
        }
        NoteField::Key => {
            let low = chosen().map(|n| n.key).min().expect("picked one");
            let keys = value.saturating_sub(i64::from(low));
            let keys = keys.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            move_or_copy(doc, clip, indexes, 0, keys, false, step)
        }
        NoteField::Length => {
            let end = c.end;
            change_each(doc, (ti, ci), indexes, step, |n| {
                let room = end.saturating_sub(n.start).clamp(1, i64::MAX as u64) as i64;
                Note {
                    length: value.clamp(1, room) as u64,
                    ..n
                }
            })
        }
        NoteField::Velocity => change_each(doc, (ti, ci), indexes, step, |n| Note {
            velocity: value.clamp(1, 127) as u8,
            ..n
        }),
    }
}

fn change_each(
    doc: &Doc,
    (ti, ci): (usize, usize),
    indexes: &[usize],
    step: impl FnOnce(usize) -> StepKind,
    change: impl Fn(Note) -> Note,
) -> Result<NotesEdited, EditError> {
    let notes = song_notes(&lifted(&doc.tracks[ti].clips[ci]).0);
    let picked = pick(indexes, notes.len())?;
    let count = indexes_count(&picked);
    let out = notes
        .into_iter()
        .zip(picked)
        .map(|(n, p)| if p { (change(n), true) } else { (n, false) })
        .collect();
    put_notes(doc, (ti, ci), out, step(count))
}

pub(crate) fn unmarked(clip: &Clip) -> Vec<(Note, bool)> {
    song_notes(clip).into_iter().map(|n| (n, false)).collect()
}

/// One flag per note. Repeated indexes count once.
pub(crate) fn pick(indexes: &[usize], len: usize) -> Result<Vec<bool>, EditError> {
    if indexes.is_empty() {
        return Err(EditError::Unchanged);
    }
    let mut picked = vec![false; len];
    for &i in indexes {
        *picked.get_mut(i).ok_or(EditError::NoSuchNote(i))? = true;
    }
    Ok(picked)
}

pub(crate) fn indexes_count(picked: &[bool]) -> usize {
    picked.iter().filter(|&&p| p).count()
}

/// The new take of the clip at `(ti, ci)` holds `notes` (positions of the [`lifted`] clip;
/// marked ones are selected afterwards), with the shown take's control points and origin.
fn put_notes(
    doc: &Doc,
    (ti, ci): (usize, usize),
    notes: Vec<(Note, bool)>,
    step: StepKind,
) -> Result<NotesEdited, EditError> {
    let clip = &doc.tracks[ti].clips[ci];
    let content = Content {
        notes,
        controls: song_controls(&lifted(clip).0)
            .into_iter()
            .map(|c| (c, false))
            .collect(),
        origin: clip.active_take().take.origin.clone(),
    };
    put_take(doc, (ti, ci), content, false, step)
}

/// What a changed take holds, in positions of the [`lifted`] clip. Marked notes or points are selected afterwards.
pub(crate) struct Content {
    pub(crate) notes: Vec<(Note, bool)>,
    pub(crate) controls: Vec<(ControlPoint, bool)>,
    pub(crate) origin: Option<TakeOrigin>,
}

fn same_origin(a: &Option<TakeOrigin>, b: &Option<TakeOrigin>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(&a.take, &b.take) && a.settings == b.settings,
        _ => false,
    }
}

fn marked<T>(items: &[(T, bool)]) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter_map(|(i, (_, m))| m.then_some(i))
        .collect()
}

/// Puts a new take holding `content` in place of the shown take of the clip at `(ti, ci)`.
/// The old take stays in no clip but in the old `Doc`. `of_controls`: the marked control
/// points are the ones to select, else the marked notes.
pub(crate) fn put_take(
    doc: &Doc,
    (ti, ci): (usize, usize),
    mut content: Content,
    of_controls: bool,
    step: StepKind,
) -> Result<NotesEdited, EditError> {
    let track = &doc.tracks[ti];
    let clip = &track.clips[ci];
    let (up, _) = lifted(clip);
    content.notes.sort_by_key(|(n, _)| (n.start, n.key));
    content.controls.sort_by_key(|(c, _)| c.tick);
    let shown = clip.active_take();
    if content.notes.iter().map(|(n, _)| n).eq(&song_notes(&up))
        && content
            .controls
            .iter()
            .map(|(c, _)| c)
            .eq(&song_controls(&up))
        && same_origin(&content.origin, &shown.take.origin)
    {
        return Err(EditError::Unchanged);
    }
    let origin = zero(&up);
    let origin_new = content
        .notes
        .iter()
        .map(|(n, _)| n.start)
        .chain(content.controls.iter().map(|(c, _)| c.tick))
        .fold(origin, u64::min);
    let end = content
        .notes
        .iter()
        .map(|(n, _)| n.end())
        .chain(content.controls.iter().map(|(c, _)| c.tick + 1))
        .fold(origin + shown.take.length, u64::max);
    let shift = origin - origin_new;
    let mut ids = Ids(doc.next_id);
    let id = TakeId(ids.next()?);
    let take_origin = match content.origin {
        // The origin's tick 0 stays at the take's tick 0: the screen draws both from there.
        Some(o) if shift > 0 => Some(TakeOrigin {
            take: Arc::new(Take {
                id: TakeId(ids.next()?),
                length: o.take.length + shift,
                notes: o
                    .take
                    .notes
                    .iter()
                    .map(|n| Note {
                        start: n.start + shift,
                        ..*n
                    })
                    .collect(),
                controls: o
                    .take
                    .controls
                    .iter()
                    .map(|c| ControlPoint {
                        tick: c.tick + shift,
                        ..*c
                    })
                    .collect(),
                origin: None,
            }),
            settings: o.settings,
        }),
        o => o,
    };
    let selected = if of_controls {
        marked(&content.controls)
    } else {
        marked(&content.notes)
    };
    let take = Take {
        id,
        length: end - origin_new,
        notes: ChunkedVec::sharing(
            &content
                .notes
                .iter()
                .map(|(n, _)| Note {
                    start: n.start - origin_new,
                    ..*n
                })
                .collect::<Vec<_>>(),
            &shown.take.notes,
        ),
        controls: ChunkedVec::sharing(
            &content
                .controls
                .iter()
                .map(|(c, _)| ControlPoint {
                    tick: c.tick - origin_new,
                    ..*c
                })
                .collect::<Vec<_>>(),
            &shown.take.controls,
        ),
        origin: take_origin,
    };
    let mut takes = clip.takes.clone();
    takes[clip.active] = ClipTake {
        take: Arc::new(take),
        offset: origin_new as i64 - up.start as i64,
    };
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        takes,
        ..Clip::clone(clip)
    });
    let mut new = with_track(
        doc,
        ti,
        Track {
            clips,
            ..Track::clone(track)
        },
    );
    new.next_id = ids.0;
    Ok(NotesEdited {
        edit: Edit { doc: new, step },
        clip: clip.id,
        selected,
        of_controls,
    })
}

#[cfg(test)]
mod tests;
