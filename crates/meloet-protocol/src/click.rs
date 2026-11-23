//! The metronome's settings (protocol 31; round 12 R3): 「メトロノーム ▾」. Kept with the PC's
//! settings, not with the song, and never in an export.

use serde::{Deserialize, Serialize};

/// The range of `TransportView::click_volume_db`.
pub const CLICK_VOLUME_MIN_DB: f32 = -24.0;
pub const CLICK_VOLUME_MAX_DB: f32 = 6.0;

/// 「音」: every one is made by calculation, and the head of a bar is higher or louder than
/// the other beats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ClickSound {
    /// 「ピッ」: the click as it always was.
    #[default]
    Beep,
    /// 「カチ」.
    Tick,
    /// 「カウベル」.
    Cowbell,
}

impl ClickSound {
    /// In the order of the window.
    pub const ALL: [ClickSound; 3] = [ClickSound::Beep, ClickSound::Tick, ClickSound::Cowbell];

    /// The JSON name (`"cowbell"`, …).
    pub fn name(self) -> &'static str {
        match self {
            ClickSound::Beep => "beep",
            ClickSound::Tick => "tick",
            ClickSound::Cowbell => "cowbell",
        }
    }

    pub fn from_name(name: &str) -> Option<ClickSound> {
        ClickSound::ALL.into_iter().find(|s| s.name() == name)
    }
}

/// 「鳴らすとき」.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ClickWhen {
    /// 「いつも」: while playing and while recording, as it always was.
    #[default]
    Always,
    /// 「録音のときだけ」: not while playing.
    RecordingOnly,
}

impl ClickWhen {
    pub const ALL: [ClickWhen; 2] = [ClickWhen::Always, ClickWhen::RecordingOnly];

    /// The JSON name.
    pub fn name(self) -> &'static str {
        match self {
            ClickWhen::Always => "always",
            ClickWhen::RecordingOnly => "recordingOnly",
        }
    }

    pub fn from_name(name: &str) -> Option<ClickWhen> {
        ClickWhen::ALL.into_iter().find(|w| w.name() == name)
    }
}
