//! P4 drum patterns of the shelf's 「パターン」 (protocol 32, round 12). A pattern is notes
//! only (no sound files); the data is `tools/patterns/drums.json`, built into meloet. The
//! screen names a pattern `pattern.<id>` and a group `pattern.group.<group>`.

use serde::{Deserialize, Serialize};

use crate::SamplePlaying;

/// A row of the shelf's 「パターン」.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PatternView {
    pub id: String,
    pub group: PatternGroup,
    /// How long the pattern is, in bars of 4/4 (「1 小節」).
    pub bars: u32,
}

/// The headings of the shelf's 「パターン」, in their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PatternGroup {
    /// 「基本」.
    Basic,
    /// 「フィルイン」.
    Fill,
}

impl PatternGroup {
    pub const ALL: [PatternGroup; 2] = [PatternGroup::Basic, PatternGroup::Fill];
}

/// The pattern the ▶ plays (`Command::PlayPattern`), as `SoundSampleView` for a sound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PatternSampleView {
    pub id: String,
    /// `None` while the kit it plays with is still being read.
    pub playing: Option<SamplePlaying>,
}
