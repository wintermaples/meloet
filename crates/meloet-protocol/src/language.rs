//! The language of the screen (protocol 33, round 12 D4).

use serde::{Deserialize, Serialize};

/// 「ことば / Language」. The screen takes it from `HelloReply::language` before its first
/// paint and keeps it until the page is loaded again: when `WorkspaceView::language` comes
/// to differ from the one of the greeting (`Command::SetLanguage`), the screen loads itself
/// again ("つなぎ直す"). Japanese is the source text; English follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Language {
    /// 「日本語」.
    #[default]
    Ja,
    /// 「English」.
    En,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::Ja, Language::En];

    /// The JSON name (`"ja"`), as the settings keep it.
    pub fn name(self) -> &'static str {
        match self {
            Language::Ja => "ja",
            Language::En => "en",
        }
    }

    pub fn from_name(name: &str) -> Option<Language> {
        Language::ALL.into_iter().find(|l| l.name() == name)
    }
}
