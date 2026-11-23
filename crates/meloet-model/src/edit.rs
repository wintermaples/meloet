//! Changes to the song: an old `Doc` in, a new `Doc` out. Untouched tracks, clips and takes
//! stay shared with the old `Doc`.

use std::sync::Arc;

use crate::arp::Arp;
use crate::automation::{AutoTarget, Lane, LaneId};
use crate::history::StepKind;
use crate::knobs::SoundKnobs;
use crate::mixer::{self, Channel, InsertId};
use crate::song::{
    Clip, ClipId, ClipTake, ClipTakes, ControlPoint, ControlValue, Doc, Note, Take, TakeId, Track,
};
use crate::time::TickRange;
pub use crate::timeline::{set_meter, set_tempo};
use crate::{InstrumentRef, TrackColor, TrackId, TrackKind, VOCAL_TRACK_NAME, VocalInput};

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
    /// No effect of that kind in that insert slot, or no such parameter.
    NoSuchInsert,
    /// No plugin of that key at that place.
    NoSuchPlugin,
    /// The insert slot holds an effect already.
    SlotTaken,
    /// No tempo point at that tick, no meter change at that bar, or no marker of that id.
    NoSuchPoint,
    /// Notes for a vocal clip or a vocal track, which hold audio.
    NotKeys,
    /// A clip of notes would go on a vocal track, or a clip of audio on a keys track.
    WrongKind,
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
    pub(crate) fn range(&self) -> TickRange {
        TickRange::new(self.origin, self.origin + self.length).expect("checked")
    }

    pub(crate) fn is_valid(&self) -> bool {
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
    if doc.tracks[ti].kind.is_vocal() {
        return Err(EditError::NotKeys);
    }
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
            c.keys().iter().map(|ct| ClipTake {
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
        takes: ClipTakes::Keys(merged),
        active: first_new + active,
        comp: Vec::new(),
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

/// Shows take number `index` (0-based) of `clip`. A comp of the clip goes in the same step.
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
        // The take sounds whole (round 12, A8).
        comp: Vec::new(),
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
/// it was the first. A clip keeps at least one take, and one with 「いいとこ取り」 all of them.
pub fn delete_take(doc: &Doc, clip: ClipId) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let track = &doc.tracks[ti];
    let old = &track.clips[ci];
    if old.takes.len() < 2 || !old.comp.is_empty() {
        return Err(EditError::Unchanged);
    }
    let index = old.active;
    let takes = old.takes.without(index);
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

/// Surrounding spaces are left out. An empty name keeps the old one (`Unchanged`).
pub fn rename_track(doc: &Doc, track: TrackId, name: &str) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    let name = name.trim();
    if name.is_empty() || name == t.name {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: with_track(
            doc,
            ti,
            Track {
                name: name.to_owned(),
                ..Track::clone(t)
            },
        ),
        step: StepKind::RenameTrack { track },
    })
}

pub fn set_track_color(doc: &Doc, track: TrackId, color: TrackColor) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    if color == t.color {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: with_track(
            doc,
            ti,
            Track {
                color,
                ..Track::clone(t)
            },
        ),
        step: StepKind::SetTrackColor { track },
    })
}

/// The track goes with its clips. The last track can go too: a song may have none. `next_id`
/// goes past the track's id, so that [`add_track`] never gives it to another track.
pub fn delete_track(doc: &Doc, track: TrackId) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let mut doc = doc.clone();
    doc.tracks.remove(ti);
    doc.next_id = doc.next_id.max(track.0.saturating_add(1));
    Ok(Edit {
        doc,
        step: StepKind::DeleteTrack { track },
    })
}

/// Puts the track at index `to` of the tracks as they are afterwards; past the end is the
/// end. The other tracks keep their order, and every track its color.
pub fn move_track(doc: &Doc, track: TrackId, to: usize) -> Result<Edit, EditError> {
    let from = track_index(doc, track)?;
    let to = to.min(doc.tracks.len() - 1);
    if to == from {
        return Err(EditError::Unchanged);
    }
    let mut doc = doc.clone();
    let t = doc.tracks.remove(from);
    doc.tracks.insert(to, t);
    Ok(Edit {
        doc,
        step: StepKind::MoveTrack { track },
    })
}

/// A new, empty track right below `below` (at the end with `None`). Its name is the short name
/// of the instrument's group, with the smallest number from 2 that is free when a track has
/// that name already ("ベース 2"); its color is the one of its place, its sends the ones of the
/// instrument's group ([`mixer::new_track_sends`]). The id is one no track,
/// clip or take of this song has had: `next_id`, or past the highest track id when that is
/// higher (the tracks of a song from a file are not counted in its `next_id`).
pub fn add_track(
    doc: &Doc,
    below: Option<TrackId>,
    instrument: InstrumentRef,
) -> Result<(Edit, TrackId), EditError> {
    let name = instrument.track_name().to_owned();
    add_track_of(doc, below, TrackKind::Keys, instrument, &name)
}

/// A vocal track named 「歌」 (then 「歌 2」), with 「入口 1 ・ モノ」 (spec 3, 23-A). Its
/// sends are off, and it has the color its place gives, as a keys track.
pub fn add_vocal_track(doc: &Doc, below: Option<TrackId>) -> Result<(Edit, TrackId), EditError> {
    let kind = TrackKind::Vocal {
        input: VocalInput::default(),
    };
    add_track_of(doc, below, kind, InstrumentRef::SINE, VOCAL_TRACK_NAME)
}

/// The vocal track records from other channels of the input. `NotKeys` has no opposite:
/// a keys track gives `NoSuchTrack`, as a track that is not there.
pub fn set_vocal_input(doc: &Doc, track: TrackId, input: VocalInput) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    match t.kind {
        TrackKind::Keys => Err(EditError::NoSuchTrack(track)),
        TrackKind::Vocal { input: old } if old == input => Err(EditError::Unchanged),
        TrackKind::Vocal { .. } => Ok(Edit {
            doc: with_track(
                doc,
                ti,
                Track {
                    kind: TrackKind::Vocal { input },
                    ..Track::clone(t)
                },
            ),
            step: StepKind::SetVocalInput { track },
        }),
    }
}

fn add_track_of(
    doc: &Doc,
    below: Option<TrackId>,
    kind: TrackKind,
    instrument: InstrumentRef,
    base: &str,
) -> Result<(Edit, TrackId), EditError> {
    let at = match below {
        Some(id) => track_index(doc, id)? + 1,
        None => doc.tracks.len(),
    };
    let mut ids = track_ids(doc)?;
    let id = TrackId(ids.next()?);
    let taken = |name: &str| doc.tracks.iter().any(|t| t.name == name);
    let name = if taken(base) {
        (2u32..)
            .map(|n| format!("{base} {n}"))
            .find(|name| !taken(name))
            .expect("fewer tracks than numbers")
    } else {
        base.to_owned()
    };
    let mut doc = doc.clone();
    doc.tracks.insert(
        at,
        Arc::new(Track {
            id,
            name,
            color: TrackColor::for_index(at),
            kind,
            sends: match kind {
                TrackKind::Keys => mixer::new_track_sends(&instrument),
                TrackKind::Vocal { .. } => [mixer::Gain::SILENT; 2],
            },
            instrument,
            channel: Channel::default(),
            muted: false,
            clips: Vec::new(),
            automation: Vec::new(),
            arp: Arp::default(),
            knobs: SoundKnobs::default(),
        }),
    );
    doc.next_id = ids.0;
    Ok((
        Edit {
            doc,
            step: StepKind::AddTrack { track: id },
        },
        id,
    ))
}

/// The ids a new track takes its own from: `next_id`, or past the highest track id when that
/// is higher.
pub(crate) fn track_ids(doc: &Doc) -> Result<Ids, EditError> {
    let highest = doc.tracks.iter().map(|t| t.id.0).max();
    Ok(Ids(match highest {
        Some(h) => doc
            .next_id
            .max(h.checked_add(1).ok_or(EditError::OutOfIds)?),
        None => doc.next_id,
    }))
}

/// What [`duplicate_track`] made.
#[derive(Debug, Clone, PartialEq)]
pub struct Duplicated {
    pub edit: Edit,
    /// The track copied.
    pub from: TrackId,
    /// The copy.
    pub track: TrackId,
    /// Each insert of the track, and the one in the same slot of the copy.
    pub inserts: Vec<(InsertId, InsertId)>,
}

impl Duplicated {
    /// What a lane of the track moves, as a lane of the copy.
    pub fn target(&self, target: AutoTarget) -> AutoTarget {
        match target {
            AutoTarget::Insert { insert, param } => {
                let copied = self.inserts.iter().find(|(from, _)| *from == insert);
                AutoTarget::Insert {
                    insert: copied.map_or(insert, |(_, to)| *to),
                    param,
                }
            }
            other => other,
        }
    }

    /// Of the folded lanes `folded` (`Session::folded_lanes`), those of the track, as lanes of
    /// the copy.
    pub fn folded(&self, folded: &[(TrackId, AutoTarget)]) -> Vec<(TrackId, AutoTarget)> {
        (folded.iter())
            .filter(|(track, _)| *track == self.from)
            .map(|&(_, target)| (self.track, self.target(target)))
            .collect()
    }
}

/// 「トラックを複製」 (round 12, A3): a copy of `track` right below it. The copy has every
/// field of the track as it is, its color too, but a name of its own (`copy_name`) and new
/// ids: its own (as [`add_track`] gives one), and those of its clips (which share the takes),
/// its inserts and its lanes, whose targets name the copy's inserts.
pub fn duplicate_track(doc: &Doc, track: TrackId) -> Result<Duplicated, EditError> {
    let ti = track_index(doc, track)?;
    let from = &doc.tracks[ti];
    let mut ids = track_ids(doc)?;
    let id = TrackId(ids.next()?);
    let mut copy = Track::clone(from);
    copy.id = id;
    copy.name = copy_name(doc, &from.name);
    for clip in &mut copy.clips {
        *clip = Arc::new(Clip {
            id: ClipId(ids.next()?),
            ..Clip::clone(clip)
        });
    }
    let mut inserts = Vec::new();
    for insert in copy.channel.inserts.iter_mut().flatten() {
        let new = InsertId(ids.next()?);
        inserts.push((insert.id, new));
        insert.id = new;
    }
    let mut made = Duplicated {
        edit: Edit {
            doc: doc.clone(),
            step: StepKind::DuplicateTrack { track: id },
        },
        from: track,
        track: id,
        inserts,
    };
    for lane in &mut copy.automation {
        *lane = Arc::new(Lane {
            id: LaneId(ids.next()?),
            target: made.target(lane.target),
            ..Lane::clone(lane)
        });
    }
    let new = &mut made.edit.doc;
    new.tracks.insert(ti + 1, Arc::new(copy));
    new.next_id = ids.0;
    Ok(made)
}

/// The name of a copy of the track named `name`: the name without the number [`add_track`]
/// puts at its end, with the smallest number from 2 that no track has ("ピアノ 2" of
/// "ピアノ", "ピアノ 3" of "ピアノ 2").
fn copy_name(doc: &Doc, name: &str) -> String {
    let numbered = |n: &str| {
        !n.starts_with('0')
            && n.bytes().all(|b| b.is_ascii_digit())
            && n.parse::<u32>().is_ok_and(|n| n >= 2)
    };
    let base = match name.rsplit_once(' ') {
        Some((base, n)) if !base.is_empty() && numbered(n) => base,
        _ => name,
    };
    let taken = |name: &str| doc.tracks.iter().any(|t| t.name == name);
    (2u32..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken(name))
        .expect("fewer tracks than numbers")
}

/// The track sounds with another instrument. Its notes and its name stay.
pub fn set_instrument(
    doc: &Doc,
    track: TrackId,
    instrument: InstrumentRef,
) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    if instrument == t.instrument {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: with_track(doc, ti, with_instrument(t, instrument)),
        step: StepKind::SetInstrument { track },
    })
}

/// The track with another instrument. The lanes of the old one's parameters go, unless both
/// are the same plugin.
fn with_instrument(t: &Track, instrument: InstrumentRef) -> Track {
    let same_plugin = match (&t.instrument, &instrument) {
        (InstrumentRef::Plugin(a), InstrumentRef::Plugin(b)) => a.key == b.key,
        _ => false,
    };
    let mut new = Track {
        instrument,
        ..Track::clone(t)
    };
    if !same_plugin {
        new.automation
            .retain(|l| !matches!(l.target, AutoTarget::Instrument { .. }));
    }
    new
}

/// 「外す」 on the stopped plugin that is the track's instrument (spec 3, 89-5): the track
/// sounds as the sine wave, and the lanes of the plugin's parameters go. `Unchanged` when
/// its instrument is no plugin.
pub fn remove_instrument(doc: &Doc, track: TrackId) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    if !matches!(t.instrument, InstrumentRef::Plugin(_)) {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: with_track(doc, ti, with_instrument(t, InstrumentRef::SINE)),
        step: StepKind::RemoveInstrument { track },
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

/// As [`clip_index`], of a clip that holds notes.
pub(crate) fn keys_clip_index(doc: &Doc, id: ClipId) -> Result<(usize, usize), EditError> {
    let (ti, ci) = clip_index(doc, id)?;
    if doc.tracks[ti].clips[ci].is_keys() {
        Ok((ti, ci))
    } else {
        Err(EditError::NotKeys)
    }
}

pub(crate) fn with_track(doc: &Doc, index: usize, track: Track) -> Doc {
    let mut new = doc.clone();
    new.tracks[index] = Arc::new(track);
    new
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod duplicate_tests;
