//! The color themes (protocol 30; the owner's request, user:2026-11-23): 「表示 ▸ テーマ ▸」.
//! Rust only keeps which one is chosen; the colors of each are the screen's
//! (`ui/src/design/themes.ts`, whose `THEME_IDS` are the JSON names here).

use serde::{Deserialize, Serialize};

/// `WorkspaceView::theme` (`Command::SetTheme`). A meloet setting (settings.json), not part of
/// the song, and not a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// 「ダーク」: the spec's own colors.
    #[default]
    Dark,
    /// 「ライト」
    Light,
    /// 「ピンク ・ ダーク」
    PinkDark,
    /// 「ピンク ・ ライト」
    PinkLight,
    /// 「ミント ・ ダーク」
    MintDark,
    /// 「ミント ・ ライト」
    MintLight,
}

impl Theme {
    /// In the order of the menu.
    pub const ALL: [Theme; 6] = [
        Theme::Dark,
        Theme::Light,
        Theme::PinkDark,
        Theme::PinkLight,
        Theme::MintDark,
        Theme::MintLight,
    ];

    /// The JSON name (`"pinkDark"`, …).
    pub fn name(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
            Theme::PinkDark => "pinkDark",
            Theme::PinkLight => "pinkLight",
            Theme::MintDark => "mintDark",
            Theme::MintLight => "mintLight",
        }
    }

    pub fn from_name(name: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|t| t.name() == name)
    }
}
