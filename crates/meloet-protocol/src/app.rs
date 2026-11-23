//! What belongs to meloet on this PC rather than to a song (protocol 31; round 12 A5 and D1):
//! the songs opened lately and the check for a new version.

use serde::{Deserialize, Serialize};

/// A row of 「ファイル ▸ 最近ひらいた曲 ▸」 (`AppState::recent`). The full path stays in Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecentSong {
    /// `Command::OpenRecent` names the row by it.
    pub id: u32,
    /// The song's name.
    pub name: String,
    /// Where it is, as places are shown elsewhere ("~/ミュージック/はじめての曲/").
    pub place: String,
}

/// How many rows the menu shows at most (spec: 「新しい順に 10 個まで」).
pub const RECENT_SHOWN: usize = 10;

/// `AppState::updates` (`Change::Updates`): 「新しい版をたしかめる」 and what it found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct UpdatesView {
    /// The switch (`Command::SetCheckUpdates`). A meloet setting, on until the person turns
    /// it off; while off meloet does not ask the internet.
    pub check: bool,
    /// A version newer than this meloet was found ("0.4.0"): 「meloet について」 says so.
    /// `Command::DismissNewVersion` does not take it away.
    pub new_version: Option<String>,
}

impl Default for UpdatesView {
    fn default() -> Self {
        UpdatesView {
            check: true,
            new_version: None,
        }
    }
}
