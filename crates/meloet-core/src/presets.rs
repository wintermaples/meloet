//! 「自分で残したもの」 of the effects that come with meloet (round 12, M1): `presets.json` next
//! to the settings. They belong to the PC, not to a song, and keeping or removing one is no
//! step.
//!
//! The file: `{"version": 1, "effects": {"compressor": [{"name": "いつもの", "params": {…}}]}}`,
//! the values by the parameters' names as `InsertFile.params` of the song file has them. A
//! parameter this meloet does not know is skipped, one that is missing stays as the insert has
//! it when the preset is used. An effect this meloet does not know is written back as it was
//! read.

use std::io::Write;
use std::path::Path;

use meloet_model::effects::EffectKind;
use meloet_protocol::{PRESET_NAME_MAX, PRESETS_PER_EFFECT, PresetsView, UserPresets as ViewOf};
use serde::{Deserialize, Serialize};

use crate::play::mixer::effect_kind;
use crate::songfile::{ParamFile, param_file, param_value};

const FILE_NAME: &str = "presets.json";
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct UserPreset {
    pub name: String,
    /// A position in the effect's table and its value, each parameter once.
    pub values: Vec<(u32, f32)>,
}

/// Every effect's 「自分で残したもの」, in the order the menu lists them (the order they were
/// first kept).
#[derive(Debug, Clone, PartialEq)]
pub struct UserPresets {
    /// One list for each of `EffectKind::ALL`, in its order.
    kinds: Vec<Vec<UserPreset>>,
    /// Effects this meloet does not know, as read.
    other: serde_json::Map<String, serde_json::Value>,
}

/// What [`UserPresets::keep`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    New,
    /// The preset of that name has these values now.
    Overwritten,
    /// `PRESETS_PER_EFFECT` already: nothing was kept.
    TooMany,
}

/// A name as it is kept: without spaces at its ends, at most `PRESET_NAME_MAX` characters.
/// `None` when nothing is left.
pub fn name_of(text: &str) -> Option<String> {
    let name: String = text.trim().chars().take(PRESET_NAME_MAX).collect();
    let name = name.trim_end();
    (!name.is_empty()).then(|| name.to_owned())
}

fn at(kind: EffectKind) -> usize {
    EffectKind::ALL
        .iter()
        .position(|&k| k == kind)
        .unwrap_or_default()
}

impl Default for UserPresets {
    fn default() -> UserPresets {
        UserPresets {
            kinds: vec![Vec::new(); EffectKind::ALL.len()],
            other: serde_json::Map::new(),
        }
    }
}

impl UserPresets {
    pub fn of(&self, kind: EffectKind) -> &[UserPreset] {
        self.kinds.get(at(kind)).map_or(&[], Vec::as_slice)
    }

    pub fn find(&self, kind: EffectKind, name: &str) -> Option<&UserPreset> {
        self.of(kind).iter().find(|p| p.name == name)
    }

    fn list_mut(&mut self, kind: EffectKind) -> &mut Vec<UserPreset> {
        &mut self.kinds[at(kind)]
    }

    /// Keeps `values` under `name` (already [`name_of`]). The same name is written over in its
    /// place in the list.
    pub fn keep(&mut self, kind: EffectKind, name: String, values: Vec<(u32, f32)>) -> Kept {
        let list = self.list_mut(kind);
        if let Some(there) = list.iter_mut().find(|p| p.name == name) {
            there.values = values;
            return Kept::Overwritten;
        }
        if list.len() >= PRESETS_PER_EFFECT {
            return Kept::TooMany;
        }
        list.push(UserPreset { name, values });
        Kept::New
    }

    /// Whether there was one of that name.
    pub fn remove(&mut self, kind: EffectKind, name: &str) -> bool {
        let list = self.list_mut(kind);
        let before = list.len();
        list.retain(|p| p.name != name);
        list.len() != before
    }

    pub fn view(&self) -> PresetsView {
        PresetsView {
            user: (EffectKind::ALL.into_iter())
                .filter(|&kind| !self.of(kind).is_empty())
                .map(|kind| ViewOf {
                    effect: effect_kind(kind),
                    names: self.of(kind).iter().map(|p| p.name.clone()).collect(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    #[serde(default)]
    effects: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct PresetFile {
    name: String,
    #[serde(default)]
    params: std::collections::BTreeMap<String, ParamFile>,
}

/// A missing, unreadable or broken file, or one of another version, has no presets. A preset
/// that is broken, has no name, or has a name kept already is left out, and so is one past
/// `PRESETS_PER_EFFECT`; a value out of range is brought into range.
pub fn load(dir: &Path) -> UserPresets {
    let Ok(text) = std::fs::read_to_string(dir.join(FILE_NAME)) else {
        return UserPresets::default();
    };
    let disk = match serde_json::from_str::<OnDisk>(&text) {
        Ok(disk) if disk.version == VERSION => disk,
        _ => return UserPresets::default(),
    };
    let mut presets = UserPresets::default();
    for (effect, list) in disk.effects {
        let Some(kind) = EffectKind::from_name(&effect) else {
            presets.other.insert(effect, list);
            continue;
        };
        let serde_json::Value::Array(list) = list else {
            continue;
        };
        for one in list {
            let Ok(file) = serde_json::from_value::<PresetFile>(one) else {
                continue;
            };
            let Some(name) = name_of(&file.name) else {
                continue;
            };
            if presets.find(kind, &name).is_some() {
                continue;
            }
            let mut values: Vec<(u32, f32)> = (file.params.into_iter())
                .filter_map(|(param, value)| {
                    let index = kind.param_index(&param)?;
                    let spec = kind.param(index)?;
                    Some((index, spec.clamp(param_value(spec, value)?)))
                })
                .collect();
            values.sort_unstable_by_key(|&(index, _)| index);
            presets.keep(kind, name, values);
        }
    }
    presets
}

/// As the settings are written: a whole temporary file, synced, renamed over the old one.
pub fn save(dir: &Path, presets: &UserPresets) -> std::io::Result<()> {
    let mut effects = presets.other.clone();
    for kind in EffectKind::ALL {
        let list: Vec<PresetFile> = (presets.of(kind).iter())
            .map(|p| PresetFile {
                name: p.name.clone(),
                params: (p.values.iter())
                    .filter_map(|&(index, v)| {
                        let spec = kind.param(index)?;
                        Some((spec.name.to_owned(), param_file(spec, v)))
                    })
                    .collect(),
            })
            .collect();
        if !list.is_empty() {
            effects.insert(
                kind.name().to_owned(),
                serde_json::to_value(list).map_err(std::io::Error::other)?,
            );
        }
    }
    let disk = OnDisk {
        version: VERSION,
        effects,
    };
    let mut text = serde_json::to_string_pretty(&disk).map_err(std::io::Error::other)?;
    text.push('\n');
    std::fs::create_dir_all(dir)?;
    let temporary = dir.join(format!("{FILE_NAME}.tmp"));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, dir.join(FILE_NAME))
}

#[cfg(test)]
mod tests;
