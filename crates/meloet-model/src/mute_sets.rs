//! Mute sets: the mutes of the song (and which tracks were soloed) kept under a name, to
//! switch between combinations of tracks in one go. A set names tracks by id: a track deleted
//! since is skipped, a track added since is left as it is.
//!
//! Saving, recalling, deleting and renaming a set are steps. Solo is the session's and no
//! step: a set keeps it, and a recall hands it back for the session to take.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::TrackId;
use crate::automation::{self, AutoTarget};
use crate::edit::Edit;
use crate::history::StepKind;
use crate::mixer::{Bus, Return};
use crate::song::{Doc, Track};

/// Sets one song holds at most.
pub const MAX_MUTE_SETS: usize = 64;
/// A set's name has at most this many characters.
pub const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuteSet {
    /// Not empty, no spaces around it, unique in the song.
    pub name: String,
    /// Every track of the song when the set was saved, in order of id.
    pub tracks: Vec<TrackId>,
    /// Those of `tracks` that were muted, in order of id.
    pub muted: Vec<TrackId>,
    /// Return A and return B.
    pub muted_returns: [bool; 2],
    /// Those of `tracks` that were soloed, in order of id.
    pub soloed: Vec<TrackId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuteSetError {
    NoSuchSet,
    /// A set of that name is there, and it was not to be written over.
    Taken,
    /// The song has [`MAX_MUTE_SETS`] sets.
    TooMany,
    /// No name: empty, longer than [`MAX_NAME_CHARS`], or with a control character.
    BadName,
    /// The set would be as it is.
    Unchanged,
}

/// What a recall does.
#[derive(Debug, Clone, PartialEq)]
pub struct Recall {
    /// `None` when every mute is as the set says already.
    pub edit: Option<Edit>,
    /// The soloed tracks the session is to have.
    pub soloed: BTreeSet<TrackId>,
    /// Tracks of the set that the song does not have any more.
    pub gone: Vec<TrackId>,
    /// Tracks of the song that the set does not name; left as they are.
    pub not_in_set: Vec<TrackId>,
    /// Tracks whose mute lane moved the mute: the lane rests in the same step, as when M is
    /// pressed on such a track, so that the set is heard.
    pub rested: Vec<TrackId>,
}

impl Doc {
    pub fn mute_set(&self, name: &str) -> Option<&MuteSet> {
        self.mute_sets.iter().find(|s| s.name == name.trim())
    }
}

impl MuteSet {
    /// Whether a recall would change nothing: the mutes, the returns and the solo
    /// (`soloed`, the session's) are as the set says ([`recall_mute_set`]).
    pub fn is_now(&self, doc: &Doc, soloed: &BTreeSet<TrackId>) -> bool {
        let named = |id: &TrackId| self.tracks.binary_search(id).is_ok();
        let there = |id: &TrackId| doc.track(*id).is_some();
        let mutes = (doc.tracks.iter())
            .filter(|t| named(&t.id))
            .all(|t| t.muted == self.muted.binary_search(&t.id).is_ok());
        let returns = Bus::ALL
            .into_iter()
            .all(|b| doc.returns[b.index()].muted == self.muted_returns[b.index()]);
        let solo_kept = (soloed.iter())
            .filter(|id| named(id))
            .all(|id| self.soloed.binary_search(id).is_ok() && there(id));
        let solo_given = (self.soloed.iter())
            .filter(|id| there(id))
            .all(|id| soloed.contains(id));
        mutes && returns && solo_kept && solo_given
    }
}

fn index_of(doc: &Doc, name: &str) -> Option<usize> {
    doc.mute_sets.iter().position(|s| s.name == name.trim())
}

/// `name` as a set keeps it: without the spaces at its ends.
fn name_of(name: &str) -> Result<&str, MuteSetError> {
    let name = name.trim();
    let count = name.chars().count();
    if count == 0 || count > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(MuteSetError::BadName);
    }
    Ok(name)
}

/// The mutes of `doc` and the solo of the session (`soloed`) as a set named `name`.
pub fn mute_set_now(doc: &Doc, soloed: &BTreeSet<TrackId>, name: &str) -> MuteSet {
    let mut tracks: Vec<TrackId> = doc.tracks.iter().map(|t| t.id).collect();
    tracks.sort_unstable();
    let picked = |f: &dyn Fn(&Track) -> bool| {
        let mut ids: Vec<TrackId> = (doc.tracks.iter()).filter(|t| f(t)).map(|t| t.id).collect();
        ids.sort_unstable();
        ids
    };
    MuteSet {
        name: name.trim().to_owned(),
        muted: picked(&|t| t.muted),
        muted_returns: Bus::ALL.map(|b| doc.returns[b.index()].muted),
        soloed: picked(&|t| soloed.contains(&t.id)),
        tracks,
    }
}

/// Keeps the mutes now (and the session's `soloed`) as the set `name`. A set of that name is
/// written over only with `overwrite`, in its place; a new one goes last.
pub fn save_mute_set(
    doc: &Doc,
    soloed: &BTreeSet<TrackId>,
    name: &str,
    overwrite: bool,
) -> Result<Edit, MuteSetError> {
    let name = name_of(name)?;
    let set = mute_set_now(doc, soloed, name);
    let mut sets = Vec::clone(&doc.mute_sets);
    let index = match index_of(doc, name) {
        Some(_) if !overwrite => return Err(MuteSetError::Taken),
        Some(i) if sets[i] == set => return Err(MuteSetError::Unchanged),
        Some(i) => {
            sets[i] = set;
            i
        }
        None if sets.len() >= MAX_MUTE_SETS => return Err(MuteSetError::TooMany),
        None => {
            sets.push(set);
            sets.len() - 1
        }
    };
    Ok(with_sets(doc, sets, StepKind::SaveMuteSet { index }))
}

pub fn delete_mute_set(doc: &Doc, name: &str) -> Result<Edit, MuteSetError> {
    let index = index_of(doc, name).ok_or(MuteSetError::NoSuchSet)?;
    let mut sets = Vec::clone(&doc.mute_sets);
    sets.remove(index);
    Ok(with_sets(doc, sets, StepKind::DeleteMuteSet { index }))
}

/// Gives the set `name` the name `to`, in its place. The name of another set is taken, as
/// for a save that is not to write over it.
pub fn rename_mute_set(doc: &Doc, name: &str, to: &str) -> Result<Edit, MuteSetError> {
    let index = index_of(doc, name).ok_or(MuteSetError::NoSuchSet)?;
    let to = name_of(to)?;
    match index_of(doc, to) {
        Some(i) if i == index => return Err(MuteSetError::Unchanged),
        Some(_) => return Err(MuteSetError::Taken),
        None => {}
    }
    let mut sets = Vec::clone(&doc.mute_sets);
    sets[index].name = to.to_owned();
    Ok(with_sets(doc, sets, StepKind::RenameMuteSet { index }))
}

/// Mutes the tracks and the returns as the set says. `soloed`: the session's solo now.
pub fn recall_mute_set(
    doc: &Doc,
    soloed: &BTreeSet<TrackId>,
    name: &str,
) -> Result<Recall, MuteSetError> {
    let index = index_of(doc, name).ok_or(MuteSetError::NoSuchSet)?;
    let set = &doc.mute_sets[index];
    let named = |id: &TrackId| set.tracks.binary_search(id).is_ok();
    let mut new = doc.clone();
    let mut changed = false;
    let mut rested = Vec::new();
    for track in &mut new.tracks {
        if !named(&track.id) {
            continue;
        }
        let muted = set.muted.binary_search(&track.id).is_ok();
        if track.muted != muted {
            if track.lane(AutoTarget::Mute).is_some_and(|l| l.moves()) {
                rested.push(track.id);
            }
            Arc::make_mut(track).muted = muted;
            changed = true;
        }
    }
    for &id in &rested {
        automation::set_resting(&mut new, id, AutoTarget::Mute, true);
    }
    for bus in Bus::ALL {
        let muted = set.muted_returns[bus.index()];
        let r = &mut new.returns[bus.index()];
        if r.muted != muted {
            *r = Arc::new(Return {
                muted,
                ..Return::clone(r)
            });
            changed = true;
        }
    }
    let mut now: BTreeSet<TrackId> = soloed.iter().copied().filter(|id| !named(id)).collect();
    now.extend(
        (set.soloed.iter())
            .copied()
            .filter(|id| doc.track(*id).is_some()),
    );
    Ok(Recall {
        edit: changed.then_some(Edit {
            doc: new,
            step: StepKind::RecallMuteSet { index },
        }),
        soloed: now,
        gone: (set.tracks.iter())
            .copied()
            .filter(|id| doc.track(*id).is_none())
            .collect(),
        not_in_set: (doc.tracks.iter())
            .map(|t| t.id)
            .filter(|id| !named(id))
            .collect(),
        rested,
    })
}

fn with_sets(doc: &Doc, sets: Vec<MuteSet>, step: StepKind) -> Edit {
    Edit {
        doc: Doc {
            mute_sets: Arc::new(sets),
            ..doc.clone()
        },
        step,
    }
}

#[cfg(test)]
mod tests;
