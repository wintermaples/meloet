//! The AI link (protocol 29; spec 3 「AI link」, A-27, the mock P10-AiLink option B): a
//! switch in the advanced settings that opens a socket inside the PC for an AI app
//! (`meloet-mcp`), and what that socket is doing.

use serde::{Deserialize, Serialize};

/// `AppState::ai_link` (`Change::AiLink`). Its own section: the port changes when an AI app
/// connects or goes away, from outside and at any time, and then nothing else should be sent
/// again. The advanced settings show it as the row 「AI とつなぐ」.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiLinkView {
    /// The switch (`Command::SetAiLink`): 「オン」 / 「オフ」. A meloet setting
    /// (settings.json), not part of the song.
    pub on: bool,
    pub port: AiPort,
}

/// The right of the row: 「閉じています」, 「待っています」, 「つながっています」.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum AiPort {
    /// No socket: the switch is off.
    #[default]
    Closed,
    /// The socket is open and no AI app is connected.
    Waiting,
    /// At least one AI app is connected.
    Connected,
}
