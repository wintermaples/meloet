//! Changes to the song: an old `Doc` in, a new `Doc` out. Untouched tracks, clips and takes
//! stay shared with the old `Doc`.

use std::sync::Arc;

use crate::TrackId;
use crate::history::StepKind;
use crate::song::{
    Clip, ClipId, ClipTake, ControlPoint, ControlValue, Doc, Note, Take, TakeId, Track,
};
use crate::time::{Meter, Tempo, TickRange};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    NoSuchTrack(TrackId),
    NoSuchClip(ClipId),
    /// `add_recording` got no takes.
    NoTakes,
    /// A take number past the end.
    NoSuchTake(usize),
    /// The new take at this place in the list has a value that cannot be.
    BadTake(usize),
    /// The change would leave the song as it is.
    Unchanged,
    /// Every id has been given out.
    OutOfIds,
    /// A note number past the end of the take.
    NoSuchNote(usize),
    /// A note that cannot be: length 0, key over 127, velocity 0 or over 127.
    BadNote,
    /// A control point that cannot be: a CC number over 127.
    BadControl,
    /// A note written outside the clip.
    OutOfClip,
    /// A clip would overlap another clip of its track.
    Overlap,
    /// A clip would go above the first track, below the last, or past [`MAX_TICK`].
    OffTheSong,
}

/// No clip ends after this, so that every song position of a take fits in an `i64`.
pub const MAX_TICK: u64 = i64::MAX as u64 / 2;

/// A new `Doc` and the step to remember it by.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    pub doc: Doc,
    pub step: StepKind,
}

/// A recorded take before it goes into the song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTake {
    /// Where the take's tick 0 is in the song.
    pub origin: u64,
    pub length: u64,
    /// In order of start, inside `[0, length)`.
    pub notes: Vec<Note>,
    /// In order of tick, inside `[0, length)`.
    pub controls: Vec<ControlPoint>,
}

impl NewTake {
    fn range(&self) -> TickRange {
        TickRange::new(self.origin, self.origin + self.length).expect("checked")
    }

    fn is_valid(&self) -> bool {
        self.length > 0
            && self
                .origin
                .checked_add(self.length)
                .is_some_and(|e| e <= MAX_TICK)
            && self.notes.is_sorted_by_key(|n| n.start)
            && self.notes.iter().all(|n| {
                n.length > 0
                    && n.start
                        .checked_add(n.length)
                        .is_some_and(|e| e <= self.length)
                    && n.key <= 127
                    && (1..=127).contains(&n.velocity)
            })
            && self.controls.is_sorted_by_key(|c| c.tick)
            && self.controls.iter().all(|c| {
                c.tick < self.length
                    && match c.value {
                        ControlValue::Cc { number, value } => number <= 127 && value <= 127,
                        ControlValue::PitchBend(v) => (-8192..=8191).contains(&v),
                    }
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordingAdded {
    pub clip: ClipId,
    /// The shown take's number in the clip (0-based).
    pub active: usize,
}

pub(crate) struct Ids(pub(crate) u32);

impl Ids {
    pub(crate) fn next(&mut self) -> Result<u32, EditError> {
        let id = self.0;
        self.0 = id.checked_add(1).ok_or(EditError::OutOfIds)?;
        Ok(id)
    }
}

/// Puts recorded takes on `track`. Clips that overlap the takes are merged with them into
/// one clip; the merged clip keeps the id and name of the earliest of them.
pub fn add_recording(
    doc: &Doc,
    track: TrackId,
    takes: Vec<NewTake>,
    active: usize,
) -> Result<(Edit, RecordingAdded), EditError> {
    let ti = track_index(doc, track)?;
    if takes.is_empty() {
        return Err(EditError::NoTakes);
    }
    if active >= takes.len() {
        return Err(EditError::NoSuchTake(active));
    }
    if let Some(bad) = takes.iter().position(|t| !t.is_valid()) {
        return Err(EditError::BadTake(bad));
    }
    let new_range = takes
        .iter()
        .map(NewTake::range)
        .reduce(TickRange::union)
        .expect("not empty");

    let old = &doc.tracks[ti];
    let (hit, mut clips): (Vec<_>, Vec<_>) = old
        .clips
        .iter()
        .cloned()
        .partition(|c| c.range().overlaps(new_range));
    let range = hit
        .iter()
        .map(|c| c.range())
        .fold(new_range, TickRange::union);

    let mut ids = Ids(doc.next_id);
    let (id, name) = match hit.first() {
        Some(c) => (c.id, c.name.clone()),
        None => (ClipId(ids.next()?), old.name.clone()),
    };
    let mut merged: Vec<ClipTake> = hit
        .iter()
        .flat_map(|c| {
            c.takes.iter().map(|ct| ClipTake {
                take: ct.take.clone(),
                offset: c.zero(ct) - range.start() as i64,
            })
        })
        .collect();
    let first_new = merged.len();
    let added = takes.len();
    for t in takes {
        merged.push(ClipTake {
            offset: (t.origin - range.start()) as i64,
            take: Arc::new(Take {
                origin: None,
                id: TakeId(ids.next()?),
                length: t.length,
                notes: t.notes.into(),
                controls: t.controls.into(),
            }),
        });
    }

    let clip = Arc::new(Clip {
        id,
        name,
        start: range.start(),
        end: range.end(),
        takes: merged,
        active: first_new + active,
    });
    let at = clips.partition_point(|c| c.start < clip.start);
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
    Ok((
        Edit {
            doc: new,
            step: StepKind::Record {
                track,
                clip: id,
                takes: added,
            },
        },
        RecordingAdded {
            clip: id,
            active: first_new + active,
        },
    ))
}

/// Shows take number `index` (0-based) of `clip`.
pub fn switch_take(doc: &Doc, clip: ClipId, index: usize) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let track = &doc.tracks[ti];
    let old = &track.clips[ci];
    if index >= old.takes.len() {
        return Err(EditError::NoSuchTake(index));
    }
    if index == old.active {
        return Err(EditError::Unchanged);
    }
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        active: index,
        ..Clip::clone(old)
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
        step: StepKind::SwitchTake {
            clip,
            from: old.active,
            to: index,
        },
    })
}

/// Removes the take `clip` shows. The clip then shows the take before it, or the next one when
/// it was the first. A clip keeps at least one take.
pub fn delete_take(doc: &Doc, clip: ClipId) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let track = &doc.tracks[ti];
    let old = &track.clips[ci];
    if old.takes.len() < 2 {
        return Err(EditError::Unchanged);
    }
    let index = old.active;
    let mut takes = old.takes.clone();
    takes.remove(index);
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        takes,
        active: index.saturating_sub(1),
        ..Clip::clone(old)
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
        step: StepKind::DeleteTake { clip, index },
    })
}

/// Removes the clips, all or none. `cut`: for Mod X.
pub fn delete_clips(doc: &Doc, ids: &[ClipId], cut: bool) -> Result<Edit, EditError> {
    if ids.is_empty() {
        return Err(EditError::Unchanged);
    }
    if let Some(&missing) = ids.iter().find(|&&id| doc.clip(id).is_none()) {
        return Err(EditError::NoSuchClip(missing));
    }
    let mut count = 0;
    let tracks = doc
        .tracks
        .iter()
        .map(|t| {
            let before = t.clips.len();
            let clips: Vec<_> = t
                .clips
                .iter()
                .filter(|c| !ids.contains(&c.id))
                .cloned()
                .collect();
            if clips.len() == before {
                return t.clone();
            }
            count += before - clips.len();
            Arc::new(Track {
                clips,
                ..Track::clone(t)
            })
        })
        .collect();
    Ok(Edit {
        doc: Doc {
            tracks,
            ..doc.clone()
        },
        step: StepKind::DeleteClips { count, cut },
    })
}

pub fn set_tempo(doc: &Doc, tempo: Tempo) -> Result<Edit, EditError> {
    if tempo == doc.tempo {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: Doc {
            tempo,
            ..doc.clone()
        },
        step: StepKind::Tempo {
            from: doc.tempo,
            to: tempo,
        },
    })
}

pub fn set_meter(doc: &Doc, meter: Meter) -> Result<Edit, EditError> {
    if meter == doc.meter {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: Doc {
            meter,
            ..doc.clone()
        },
        step: StepKind::Meter {
            from: doc.meter,
            to: meter,
        },
    })
}

/// Mutes the track, or lets it play again.
pub fn toggle_mute(doc: &Doc, track: TrackId) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    let muted = !t.muted;
    Ok(Edit {
        doc: with_track(
            doc,
            ti,
            Track {
                muted,
                ..Track::clone(t)
            },
        ),
        step: StepKind::Mute { track, muted },
    })
}

pub(crate) fn track_index(doc: &Doc, id: TrackId) -> Result<usize, EditError> {
    doc.tracks
        .iter()
        .position(|t| t.id == id)
        .ok_or(EditError::NoSuchTrack(id))
}

pub(crate) fn clip_index(doc: &Doc, id: ClipId) -> Result<(usize, usize), EditError> {
    doc.tracks
        .iter()
        .enumerate()
        .find_map(|(ti, t)| t.clips.iter().position(|c| c.id == id).map(|ci| (ti, ci)))
        .ok_or(EditError::NoSuchClip(id))
}

pub(crate) fn with_track(doc: &Doc, index: usize, track: Track) -> Doc {
    let mut new = doc.clone();
    new.tracks[index] = Arc::new(track);
    new
}

#[cfg(test)]
mod tests;
