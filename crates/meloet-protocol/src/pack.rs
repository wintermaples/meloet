//! The sound pack, fetched after the first start (protocol 27; spec 3 「音の素材の包み」 Y26,
//! 43-B, 44-A, 45-A; docs/architecture.md 23-1 to 23-3 and 23-7).

use serde::{Deserialize, Serialize};

use crate::InstrumentKind;

/// `AppState::sounds` (`Change::Sounds`): the pack as this PC has it, and its fetch. Apart from
/// `shelf` and `workspace` because the numbers of a fetch change a few times a second; Rust
/// sends them at most 4 times a second.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SoundsView {
    /// The band asks "音の素材（約 240 MB）を取りにいきます" with 「取りにいく」
    /// (`Command::FetchPack`) and 「あとで」 (`Command::PackLater`): something of the pack is
    /// missing and the person has not answered yet, at the first start and at each start
    /// after 「あとで」. False again once answered. The screen puts it after the hints of
    /// flows 1 to 4: while one of them is up it waits.
    pub ask: bool,
    pub fetch: PackFetchView,
    /// The instruments of the shelf that sound with their own sound now, the ones that are
    /// computed (synths, the sine wave) counted: "まだ 12 / 31". `have == total`: the pack is
    /// all there ("ぜんぶあります ・ 240 MB").
    pub have: u32,
    /// Every instrument of the shelf (31).
    pub total: u32,
    /// The whole pack, in bytes: "約 240 MB", "ぜんぶあります ・ 240 MB".
    pub pack_bytes: u64,
    /// A newer pack is out, found by the check at a start (at most once in 24 hours): what
    /// fetching it would take, in bytes ("音の素材の新しい包みがあります（12 MB）。"). `None`
    /// again once it is fetched.
    pub new_pack_bytes: Option<u64>,
    /// Where the pack is put, as shown ("~/.local/share/meloet/sounds"): the row 「音の素材」
    /// of the advanced sound settings, and the heading 「音の素材」 of 「meloet について」.
    pub place: String,
}

/// The fetch of the pack. The band of the input meter shows it (44-A) unless it is `Idle`;
/// while a hint of flows 1 to 4 is up, the status line does (the form of 44-B).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PackFetchView {
    /// Nothing is being fetched: all is there, the person has not answered or said
    /// 「あとで」, or pressed 「やめる」.
    #[default]
    Idle,
    /// "音の素材を取りにいっています ・ 38 % ・ 92 / 240 MB ・ のこり 約 2 分", with 「やめる」
    /// (`Command::StopPackFetch`). The percent is `done_bytes / total_bytes`.
    Fetching {
        /// The instrument whose file is coming now: its row of the shelf says
        /// 「取ってきています」. `None` for the small file the instruments share.
        instrument: Option<InstrumentKind>,
        /// What is on the disk already, files of earlier runs and the part of the one coming.
        done_bytes: u64,
        /// What the pack takes in all: `SoundsView::pack_bytes`, or for a new pack what it
        /// changes.
        total_bytes: u64,
        /// `None` until there is something to guess from.
        remaining_s: Option<u32>,
    },
    /// The band says why, with 「もう一度」 (`Command::FetchPack`; 「続きから」 for `CutOff`)
    /// and 「あとで」 (`Command::PackLater`, which makes it `Idle`). The status line says it
    /// too (`StatusMessage::PackFailed`).
    Failed {
        failure: PackFailure,
        /// "92 / 240 MB まで取ってあります".
        done_bytes: u64,
        total_bytes: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PackFailure {
    /// "音の素材を取りにいけませんでした（インターネットにつながっていません）。…"
    Offline,
    /// "とちゅうで切れました。92 / 240 MB まで取ってあります。続きから取ります。"
    CutOff,
    /// "ディスクがいっぱいで、音の素材を置けません（あと 150 MB 要ります）。": the room the rest
    /// needs and what the disk has free; "あと" is their difference.
    DiskFull { needed_bytes: u64, free_bytes: u64 },
    /// "取った音の素材がこわれていました。取りなおしても直りませんでした。": after one more try
    /// that was not said.
    Corrupt,
}
