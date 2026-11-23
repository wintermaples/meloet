//! M1 presets of the effects that come with meloet (protocol 32, round 12; the round's spec
//! calls itself round 11). 「はじめから入っているもの」 are a table of the model
//! (`meloet_model::presets`), written for the screen as `FACTORY_PRESETS`
//! (`ui/src/bridge/generated/presets.ts`); the screen names them `preset.<effect>.<id>`.
//! 「自分で残したもの」 belong to the PC (`presets.json` next to the settings), not to a song.

use serde::{Deserialize, Serialize};

use crate::EffectKind;

/// 「いまの設定を残す…」: a name is at most this many characters.
pub const PRESET_NAME_MAX: usize = 40;
/// 「自分で残したもの」 of one kind of effect are at most this many.
pub const PRESETS_PER_EFFECT: usize = 100;

/// A preset of a built-in effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PresetRef {
    /// 「はじめから入っているもの」: an `id` of `FACTORY_PRESETS` for the effect.
    Factory { id: String },
    /// 「自分で残したもの」, by its name.
    User { name: String },
}

/// `AppState::presets`: what the menu of 「プリセット ▾」 lists besides the factory ones.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PresetsView {
    /// 「自分で残したもの」, one entry for each kind of effect that has any, in the order of
    /// `EffectKind::ALL`.
    pub user: Vec<UserPresets>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct UserPresets {
    pub effect: EffectKind,
    /// In the order the menu lists them; each name once.
    pub names: Vec<String>,
}
