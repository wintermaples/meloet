//! Changes to the notes of a clip's shown take. Each one makes a new take (a new id) and puts it
//! in place of the shown one, so the clip keeps its number of takes; the old take stays as it
//! was, for undo.
//!
//! Notes given and returned here are in song positions; the take holds them from its tick 0.
//! A changed take keeps its notes in order of start, then key, then the order they had. Notes
//! stay inside the clip; the take grows (never shrinks) to hold them, and its offset follows.

use std::sync::Arc;

use crate::TrackId;
use crate::edit::{Edit, EditError, Ids, clip_index, track_index, with_track};
use crate::history::StepKind;
use crate::song::{Clip, ClipId, ClipTake, ControlPoint, Doc, Note, Take, TakeId, Track};

/// Bars a clip made for a written note gets, when nothing comes before that.
pub const NEW_CLIP_BARS: u64 = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct NotesEdited {
    pub edit: Edit,
    pub clip: ClipId,
    /// Indexes, in the new take, of the notes written, moved, copied or pasted.
    pub selected: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAt {
    Clip(ClipId),
    /// The clip of the track that holds the note's start, or else a new one.
    Track(TrackId),
}

/// The notes of the clip's shown take, in song positions, in the take's order.
pub fn song_notes(clip: &Clip) -> Vec<Note> {
    let shown = clip.active_take();
    let origin = clip.start + shown.offset;
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

/// What Mod C keeps: the notes at `indexes`, each start counted from the earliest of them.
pub fn clipboard(clip: &Clip, indexes: &[usize]) -> Result<Vec<Note>, EditError> {
    let notes = song_notes(clip);
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
            ..n
        })
        .collect())
}

/// The write tool. The note is cut at the end of the clip.
pub fn write_note(doc: &Doc, at: WriteAt, note: Note) -> Result<NotesEdited, EditError> {
    if note.length == 0 || note.key > 127 || !(1..=127).contains(&note.velocity) {
        return Err(EditError::BadNote);
    }
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
    let note = Note {
        length: note.length.min(c.end - note.start),
        ..note
    };
    let mut notes = unmarked(c);
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
        }],
        controls: Vec::new(),
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
    let (ti, ci) = clip_index(doc, clip)?;
    let c = &doc.tracks[ti].clips[ci];
    let notes = song_notes(c);
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
            .chain(chosen().map(|n| (moved(n), true)))
            .collect()
    } else {
        notes
            .iter()
            .zip(&picked)
            .map(|(n, &p)| if p { (moved(n), true) } else { (*n, false) })
            .collect()
    };
    put_notes(
        doc,
        (ti, ci),
        out,
        StepKind::MoveNotes { clip, count, copy },
    )
}

/// Every note grows or shrinks by `ticks`, each one staying 1 tick or longer and ending by the
/// end of the clip.
pub fn resize_notes(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    ticks: i64,
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let c = &doc.tracks[ti].clips[ci];
    let end = c.end;
    change_each(
        doc,
        (ti, ci),
        indexes,
        |count| StepKind::ResizeNotes { clip, count },
        |n| {
            let length = n.length as i64 + ticks;
            let length = if ticks >= 0 {
                length.min(n.length.max(end.saturating_sub(n.start)) as i64)
            } else {
                length.max(1)
            };
            Note {
                length: length as u64,
                ..n
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
    let c = &doc.tracks[ti].clips[ci];
    let notes = song_notes(c);
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
    let c = &doc.tracks[ti].clips[ci];
    let pasted: Vec<(Note, bool)> = notes
        .iter()
        .filter_map(|n| {
            let start = at.checked_add(n.start)?;
            let fits = start >= c.start
                && start.checked_add(n.length)? <= c.end
                && n.length > 0
                && n.key <= 127
                && (1..=127).contains(&n.velocity);
            fits.then_some((Note { start, ..*n }, true))
        })
        .collect();
    if pasted.is_empty() {
        return Err(EditError::Unchanged);
    }
    let count = pasted.len();
    let mut out = unmarked(c);
    out.extend(pasted);
    put_notes(doc, (ti, ci), out, StepKind::PasteNotes { clip, count })
}

fn change_each(
    doc: &Doc,
    (ti, ci): (usize, usize),
    indexes: &[usize],
    step: impl FnOnce(usize) -> StepKind,
    change: impl Fn(Note) -> Note,
) -> Result<NotesEdited, EditError> {
    let notes = song_notes(&doc.tracks[ti].clips[ci]);
    let picked = pick(indexes, notes.len())?;
    let count = indexes_count(&picked);
    let out = notes
        .into_iter()
        .zip(picked)
        .map(|(n, p)| if p { (change(n), true) } else { (n, false) })
        .collect();
    put_notes(doc, (ti, ci), out, step(count))
}

fn unmarked(clip: &Clip) -> Vec<(Note, bool)> {
    song_notes(clip).into_iter().map(|n| (n, false)).collect()
}

/// One flag per note. Repeated indexes count once.
fn pick(indexes: &[usize], len: usize) -> Result<Vec<bool>, EditError> {
    if indexes.is_empty() {
        return Err(EditError::Unchanged);
    }
    let mut picked = vec![false; len];
    for &i in indexes {
        *picked.get_mut(i).ok_or(EditError::NoSuchNote(i))? = true;
    }
    Ok(picked)
}

fn indexes_count(picked: &[bool]) -> usize {
    picked.iter().filter(|&&p| p).count()
}

fn sort(notes: &mut [(Note, bool)]) {
    notes.sort_by_key(|(n, _)| (n.start, n.key));
}

/// The new take of the clip at `(ti, ci)` holds `notes` (song positions; marked ones are
/// selected afterwards). The old take stays in no clip but in the old `Doc`.
fn put_notes(
    doc: &Doc,
    (ti, ci): (usize, usize),
    mut notes: Vec<(Note, bool)>,
    step: StepKind,
) -> Result<NotesEdited, EditError> {
    let track = &doc.tracks[ti];
    let clip = &track.clips[ci];
    sort(&mut notes);
    let mut old = unmarked(clip);
    sort(&mut old);
    if notes.iter().map(|(n, _)| n).eq(old.iter().map(|(n, _)| n)) {
        return Err(EditError::Unchanged);
    }
    let shown = clip.active_take();
    let origin = clip.start + shown.offset;
    let origin_new = notes.iter().map(|(n, _)| n.start).fold(origin, u64::min);
    let end = notes
        .iter()
        .map(|(n, _)| n.end())
        .fold(origin + shown.take.length, u64::max);
    let shift = origin - origin_new;
    let mut ids = Ids(doc.next_id);
    let take = Take {
        id: TakeId(ids.next()?),
        length: end - origin_new,
        notes: notes
            .iter()
            .map(|(n, _)| Note {
                start: n.start - origin_new,
                ..*n
            })
            .collect(),
        controls: shown
            .take
            .controls
            .iter()
            .map(|c| ControlPoint {
                tick: c.tick + shift,
                ..*c
            })
            .collect(),
    };
    let selected = notes
        .iter()
        .enumerate()
        .filter_map(|(i, (_, marked))| marked.then_some(i))
        .collect();
    let mut takes = clip.takes.clone();
    takes[clip.active] = ClipTake {
        take: Arc::new(take),
        offset: origin_new - clip.start,
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
    })
}

#[cfg(test)]
mod tests;
