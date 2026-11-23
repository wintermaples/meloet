//! Mute sets on the screen (protocol 34, round 12 after the features; the owner's answers of
//! 2026-11-24, B-97-3 and B-97-5): the mutes of the tracks and the returns, and which tracks
//! are soloed, kept in the song under a name (`meloet_model::mute_sets`). The menu at the head
//! of the mixer band lists them (`SongView::mute_sets`) and sends `Command::SaveMuteSet`,
//! `RecallMuteSet`, `DeleteMuteSet` and `RenameMuteSet`. Sets are named by their name: a song
//! has one set of a name.

use serde::{Deserialize, Serialize};

/// A song holds at most this many sets.
pub const MUTE_SETS_MAX: usize = 64;
/// A set's name has at most this many characters (after the spaces at its ends are taken off).
pub const MUTE_SET_NAME_MAX: usize = 64;

/// A set as the menu lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MuteSetView {
    pub name: String,
    /// A recall would change nothing: the tracks the set names that the song still has are
    /// muted and soloed as it says, and so are the returns. Tracks added since do not count.
    pub as_now: bool,
}
