//! Changes to the notes of a clip's shown take. Each one makes a new take (a new id) and puts it
//! in place of the shown one, so the clip keeps its number of takes; the old take stays as it
//! was, for undo.
//!
//! Notes given and returned here are in song positions; the take holds them from its tick 0.
//! When the shown take's tick 0 is before the song's (its head trimmed off and the clip moved
//! to the song's start), the work is done on the clip [`lifted`] by that much, so that every
//! note of the take has a position.
//! A changed take keeps its notes in order of start, then key, then the order they had. The take
//! grows (never shrinks) to hold them, and its offset follows.
//!
//! Notes written, moved or resized out of the clip make the clip grow by whole bars, as the same
//! step (B-82): back to the head of the bar with the earliest start, on to the end of the bar
//! with the latest end. Not before the song's start; not over another clip of the track, which
//! does nothing and says [`EditError::Overlap`]. Pasted notes do the same. The other edits keep
//! notes inside the clip.

use std::sync::Arc;

use crate::TrackId;
use crate::arrange::ClipEdge;
use crate::chunked::ChunkedVec;
use crate::edit::{Edit, EditError, Ids, MAX_TICK, keys_clip_index, track_index, with_track};
use crate::history::StepKind;
use crate::song::{
    Clip, ClipId, ClipTake, ClipTakes, ControlPoint, Doc, Note, Take, TakeId, TakeOrigin, Track,
};
use crate::tempo_map::TempoMap;

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
    if !clip.is_keys() {
        return Err(EditError::NotKeys);
    }
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

/// The write tool. Outside the clip given, the clip grows to hold the note.
pub fn write_note(doc: &Doc, at: WriteAt, note: Note) -> Result<NotesEdited, EditError> {
    write_notes(doc, at, &[note])
}

/// Where a written note was cut shorter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutAt {
    /// The end of the clip made for the write.
    ClipEnd,
    /// [`MAX_TICK`].
    SongEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteCut {
    pub index: usize,
    /// The length it was written with.
    pub length: u64,
    pub at: CutAt,
}

/// What [`write_notes_told`] did with the notes, by their indexes in the slice given.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteTold {
    pub clip_made: bool,
    /// Not written: they start at or after the end of the clip made for the write.
    pub dropped: Vec<usize>,
    pub cut: Vec<NoteCut>,
}

/// The write tool and the chord stamp: every note as one step. Outside the clip given, the
/// clip grows to hold them; with [`WriteAt::Track`] the clip is the one that holds the
/// earliest start, or else a new one.
pub fn write_notes(doc: &Doc, at: WriteAt, notes: &[Note]) -> Result<NotesEdited, EditError> {
    write_notes_told(doc, at, notes).map(|(edited, _)| edited)
}

/// [`write_notes`], and which notes were left out or cut.
pub fn write_notes_told(
    doc: &Doc,
    at: WriteAt,
    notes: &[Note],
) -> Result<(NotesEdited, WriteTold), EditError> {
    let bad = |n: &Note| n.length == 0 || n.key > 127 || !(1..=127).contains(&n.velocity);
    if notes.iter().any(bad) {
        return Err(EditError::BadNote);
    }
    let Some(first) = notes.iter().map(|n| n.start).min() else {
        return Err(EditError::Unchanged);
    };
    if notes.iter().any(|n| n.start >= MAX_TICK) {
        return Err(EditError::OffTheSong);
    }
    let notes: Vec<Note> = notes.iter().map(|n| Note { from: None, ..*n }).collect();
    let count = notes.len();
    let clip = match at {
        WriteAt::Clip(id) => id,
        WriteAt::Track(track) => {
            let t = &doc.tracks[track_index(doc, track)?];
            match t.clips.iter().find(|c| c.start <= first && first < c.end) {
                Some(c) => c.id,
                None => return write_in_new_clip(doc, track, notes),
            }
        }
    };
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let (c, l) = lifted(&doc.tracks[ti].clips[ci]);
    let written: Vec<Note> = notes
        .iter()
        .map(|n| Note {
            start: n.start + l,
            length: n.length.min(MAX_TICK - n.start),
            ..*n
        })
        .collect();
    let told = WriteTold {
        clip_made: false,
        dropped: Vec::new(),
        cut: (notes.iter().zip(&written).enumerate())
            .filter(|(_, (asked, w))| w.length < asked.length)
            .map(|(index, (_, w))| NoteCut {
                index,
                length: w.length,
                at: CutAt::SongEnd,
            })
            .collect(),
    };
    let reach = Reach {
        start: written.iter().map(|n| n.start).min(),
        end: written.iter().map(|n| n.end()).max(),
    };
    let mut out = unmarked(&c);
    out.extend(written.into_iter().map(|n| (n, true)));
    let edited = put_notes(
        doc,
        (ti, ci),
        out,
        StepKind::WriteNote {
            clip,
            clip_made: false,
            count,
        },
        reach,
    )?;
    Ok((edited, told))
}

/// Where a clip made at `tick` on the track at `ti` goes: from the head of the bar (or the end
/// of the clip before, when later), `NEW_CLIP_BARS` bars long or up to the next clip. Only for
/// a `tick` inside no clip of the track.
fn new_clip_place(doc: &Doc, ti: usize, tick: u64) -> (u64, u64) {
    let clips = &doc.tracks[ti].clips;
    let map = TempoMap::new(&doc.timeline);
    let bar = map.meter_at(tick);
    let start = (clips.iter())
        .filter(|c| c.end <= tick)
        .map(|c| c.end)
        .fold(bar.bar_tick, u64::max);
    let end = (clips.iter())
        .filter(|c| c.start > tick)
        .map(|c| c.start)
        .fold(map.tick_of_bar(bar.bar + NEW_CLIP_BARS), u64::min)
        .min(MAX_TICK);
    (start, end)
}

/// A new keys clip on the track at `ti` from `start` to `end`, named after the track, with one
/// take holding `notes` (song positions inside the clip, in order of start), put in place.
fn with_new_clip(
    doc: &Doc,
    ti: usize,
    (start, end): (u64, u64),
    notes: Vec<Note>,
) -> Result<(Doc, ClipId), EditError> {
    let old = &doc.tracks[ti];
    let mut ids = Ids(doc.next_id);
    let id = ClipId(ids.next()?);
    let take = Take {
        id: TakeId(ids.next()?),
        length: end - start,
        notes: notes
            .into_iter()
            .map(|n| Note {
                start: n.start - start,
                ..n
            })
            .collect::<Vec<_>>()
            .into(),
        controls: ChunkedVec::default(),
        origin: None,
    };
    let clip = Arc::new(Clip {
        id,
        name: old.name.clone(),
        start,
        end,
        takes: ClipTakes::Keys(vec![ClipTake {
            take: Arc::new(take),
            offset: 0,
        }]),
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
    Ok((new, id))
}

/// The notes in a clip made for them, from the bar of the earliest start (see
/// [`new_clip_place`]), named after the track. Notes running past its end are cut there.
fn write_in_new_clip(
    doc: &Doc,
    track: TrackId,
    mut notes: Vec<Note>,
) -> Result<(NotesEdited, WriteTold), EditError> {
    let ti = track_index(doc, track)?;
    if doc.tracks[ti].kind.is_vocal() {
        return Err(EditError::NotKeys);
    }
    let first = notes
        .iter()
        .map(|n| n.start)
        .min()
        .expect("one note at least");
    let (start, end) = new_clip_place(doc, ti, first);
    let mut told = WriteTold {
        clip_made: true,
        ..WriteTold::default()
    };
    for (index, n) in notes.iter().enumerate() {
        if n.start >= end {
            told.dropped.push(index);
        } else if n.length > end - n.start {
            told.cut.push(NoteCut {
                index,
                length: end - n.start,
                at: CutAt::ClipEnd,
            });
        }
    }
    notes.retain(|n| n.start < end);
    for n in &mut notes {
        n.length = n.length.min(end - n.start);
    }
    notes.sort_by_key(|n| (n.start, n.key));
    let count = notes.len();
    let (new, id) = with_new_clip(doc, ti, (start, end), notes)?;
    let edited = NotesEdited {
        edit: Edit {
            doc: new,
            step: StepKind::WriteNote {
                clip: id,
                clip_made: true,
                count,
            },
        },
        clip: id,
        selected: (0..count).collect(),
        of_controls: false,
    };
    Ok((edited, told))
}

/// An empty keys clip on `track` at `tick` (see [`new_clip_place`]), named after the track.
/// `Overlap` when `tick` is inside a clip; `NotKeys` on a vocal track.
pub fn create_clip(doc: &Doc, track: TrackId, tick: u64) -> Result<(Edit, ClipId), EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    if t.kind.is_vocal() {
        return Err(EditError::NotKeys);
    }
    if tick >= MAX_TICK {
        return Err(EditError::OffTheSong);
    }
    if t.clips.iter().any(|c| c.start <= tick && tick < c.end) {
        return Err(EditError::Overlap);
    }
    let place = new_clip_place(doc, ti, tick);
    let (new, clip) = with_new_clip(doc, ti, place, Vec::new())?;
    Ok((
        Edit {
            doc: new,
            step: StepKind::CreateClip { clip },
        },
        clip,
    ))
}

/// [`create_clip`] with the end given: from the head of the bar of `tick` (or the end of the
/// clip before, when later) to `end`. `Overlap` when a clip of the track is in that range.
pub fn create_clip_to(
    doc: &Doc,
    track: TrackId,
    tick: u64,
    end: u64,
) -> Result<(Edit, ClipId), EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    if t.kind.is_vocal() {
        return Err(EditError::NotKeys);
    }
    if tick >= MAX_TICK || end > MAX_TICK {
        return Err(EditError::OffTheSong);
    }
    if t.clips.iter().any(|c| c.start <= tick && tick < c.end) {
        return Err(EditError::Overlap);
    }
    let (start, _) = new_clip_place(doc, ti, tick);
    if end <= tick || t.clips.iter().any(|c| c.start < end && start < c.end) {
        return Err(EditError::Overlap);
    }
    let (new, clip) = with_new_clip(doc, ti, (start, end), Vec::new())?;
    Ok((
        Edit {
            doc: new,
            step: StepKind::CreateClip { clip },
        },
        clip,
    ))
}

/// Moves the notes by `ticks` and `keys`, or with `copy` adds moved copies. The notes move
/// together and stop where the first of them would leave the song or the keys 0..=127; the clip
/// grows to hold them.
pub fn move_notes(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    ticks: i64,
    keys: i32,
    copy: bool,
) -> Result<NotesEdited, EditError> {
    move_or_copy(doc, clip, indexes, ticks, keys, copy, true, |count| {
        StepKind::MoveNotes { clip, count, copy }
    })
}

/// `grow`: the clip grows to hold the notes moved; else they stop at its edges.
#[allow(clippy::too_many_arguments)]
fn move_or_copy(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    ticks: i64,
    keys: i32,
    copy: bool,
    grow: bool,
    step: impl FnOnce(usize) -> StepKind,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let (c, l) = lifted(&doc.tracks[ti].clips[ci]);
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
    let (floor, ceiling) = if grow {
        (l, MAX_TICK + l)
    } else {
        (c.start, c.end)
    };
    let before = floor as i64 - first as i64;
    let after = ceiling as i64 - last as i64;
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
    let reach = if grow {
        Reach {
            start: (ticks < 0).then(|| (first as i64 + ticks) as u64),
            end: (ticks > 0).then(|| (last as i64 + ticks) as u64),
        }
    } else {
        Reach::NONE
    };
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
    put_notes(doc, (ti, ci), out, step(count), reach)
}

/// Every note moves its `edge` by `ticks`, the other edge staying, each note staying 1 tick or
/// longer and inside the song. The clip grows to hold the notes made longer.
pub fn resize_notes(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    edge: ClipEdge,
    ticks: i64,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let (c, l) = lifted(&doc.tracks[ti].clips[ci]);
    let l = l as i64;
    let resized = |n: Note| match edge {
        ClipEdge::End => {
            let length = n.length as i64 + ticks;
            let length = if ticks >= 0 {
                length.min((MAX_TICK as i64 + l - n.start as i64).max(n.length as i64))
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
                start.max(l.min(n.start as i64))
            } else {
                start.min(end - 1)
            };
            Note {
                start: start as u64,
                length: (end - start) as u64,
                ..n
            }
        }
    };
    let notes = song_notes(&c);
    let picked = pick(indexes, notes.len())?;
    let changed = || {
        notes
            .iter()
            .zip(&picked)
            .filter(|&(_, &p)| p)
            .map(|(n, _)| resized(*n))
    };
    let reach = match edge {
        ClipEdge::End if ticks > 0 => Reach {
            start: None,
            end: changed().map(|n| n.end()).max(),
        },
        ClipEdge::Start if ticks < 0 => Reach {
            start: changed().map(|n| n.start).min(),
            end: None,
        },
        _ => Reach::NONE,
    };
    let count = indexes_count(&picked);
    let out = notes
        .iter()
        .zip(&picked)
        .map(|(n, &p)| if p { (resized(*n), true) } else { (*n, false) })
        .collect();
    put_notes(
        doc,
        (ti, ci),
        out,
        StepKind::ResizeNotes { clip, count },
        reach,
    )
}

/// Each velocity moves by `delta` and stops at 1 and 127.
pub fn change_velocity(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    delta: i32,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = keys_clip_index(doc, clip)?;
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
    let (ti, ci) = keys_clip_index(doc, clip)?;
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
        Reach::NONE,
    )
}

/// Puts `notes` (from [`clipboard`]) in the clip with the earliest at `at`. The clip grows by
/// bars to hold them (B-82); notes past the song's end are left out, and those running past it
/// are cut there.
pub fn paste_notes(
    doc: &Doc,
    clip: ClipId,
    notes: &[Note],
    at: u64,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let (c, l) = lifted(&doc.tracks[ti].clips[ci]);
    let ceiling = MAX_TICK + l;
    let pasted: Vec<(Note, bool)> = notes
        .iter()
        .filter_map(|n| {
            let start = at.checked_add(l)?.checked_add(n.start)?;
            let fits =
                start < ceiling && n.length > 0 && n.key <= 127 && (1..=127).contains(&n.velocity);
            fits.then(|| {
                (
                    Note {
                        start,
                        length: n.length.min(ceiling - start),
                        from: None,
                        ..*n
                    },
                    true,
                )
            })
        })
        .collect();
    if pasted.is_empty() {
        return Err(EditError::Unchanged);
    }
    let count = pasted.len();
    let reach = Reach {
        start: pasted.iter().map(|(n, _)| n.start).min(),
        end: pasted.iter().map(|(n, _)| n.end()).max(),
    };
    let mut out = unmarked(&c);
    out.extend(pasted);
    put_notes(
        doc,
        (ti, ci),
        out,
        StepKind::PasteNotes { clip, count },
        reach,
    )
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
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let (c, l) = lifted(&doc.tracks[ti].clips[ci]);
    let step = |count| StepKind::SetNotesField { clip, count, field };
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
            move_or_copy(doc, clip, indexes, ticks, 0, false, false, step)
        }
        NoteField::Key => {
            let low = chosen().map(|n| n.key).min().expect("picked one");
            let keys = value.saturating_sub(i64::from(low));
            let keys = keys.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            move_or_copy(doc, clip, indexes, 0, keys, false, false, step)
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
    put_notes(doc, (ti, ci), out, step(count), Reach::NONE)
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

/// Positions (of the [`lifted`] clip) that the clip must hold after an edit: it grows by bars to
/// those outside it.
#[derive(Debug, Clone, Copy)]
struct Reach {
    start: Option<u64>,
    /// The end of a note: held when the clip ends there or later.
    end: Option<u64>,
}

impl Reach {
    const NONE: Reach = Reach {
        start: None,
        end: None,
    };
}

/// The new take of the clip at `(ti, ci)` holds `notes` (positions of the [`lifted`] clip;
/// marked ones are selected afterwards), with the shown take's control points and origin.
fn put_notes(
    doc: &Doc,
    (ti, ci): (usize, usize),
    notes: Vec<(Note, bool)>,
    step: StepKind,
    reach: Reach,
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
    put_take_reaching(doc, (ti, ci), content, false, step, reach)
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
    at: (usize, usize),
    content: Content,
    of_controls: bool,
    step: StepKind,
) -> Result<NotesEdited, EditError> {
    put_take_reaching(doc, at, content, of_controls, step, Reach::NONE)
}

/// [`put_take`], the clip grown to hold `reach`.
fn put_take_reaching(
    doc: &Doc,
    (ti, ci): (usize, usize),
    mut content: Content,
    of_controls: bool,
    step: StepKind,
    reach: Reach,
) -> Result<NotesEdited, EditError> {
    let track = &doc.tracks[ti];
    let clip = &track.clips[ci];
    let (up, l) = lifted(clip);
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
    let (start, clip_end) = grown(doc, (ti, ci), reach, l)?;
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
    // A start moved back leaves every take where it is in the song.
    let back = clip.start as i64 - start as i64;
    let mut takes: Vec<ClipTake> = clip
        .keys()
        .iter()
        .map(|ct| ClipTake {
            take: ct.take.clone(),
            offset: ct.offset + back,
        })
        .collect();
    takes[clip.active] = ClipTake {
        take: Arc::new(take),
        offset: origin_new as i64 - (start + l) as i64,
    };
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        start,
        end: clip_end,
        takes: ClipTakes::Keys(takes),
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

/// The start and end (song positions) of the clip at `(ti, ci)` grown by bars to hold `reach`
/// (positions of the clip lifted by `l`). `Overlap` when it would then overlap another clip.
fn grown(
    doc: &Doc,
    (ti, ci): (usize, usize),
    reach: Reach,
    l: u64,
) -> Result<(u64, u64), EditError> {
    let track = &doc.tracks[ti];
    let c = &track.clips[ci];
    let map = TempoMap::new(&doc.timeline);
    let start = reach
        .start
        .map(|s| s.saturating_sub(l))
        .filter(|&s| s < c.start)
        .map_or(c.start, |s| map.meter_at(s).bar_tick);
    let end = reach
        .end
        .map(|e| e.saturating_sub(l))
        .filter(|&e| e > c.end)
        .map_or(c.end, |e| map.bar_at_or_after(e).min(MAX_TICK));
    let overlaps = track
        .clips
        .iter()
        .enumerate()
        .any(|(i, o)| i != ci && o.start < end && start < o.end);
    if overlaps {
        return Err(EditError::Overlap);
    }
    Ok((start, end))
}

#[cfg(test)]
mod tests;
