//! Surge XT's patch files as presets: the factory and third-party patches, and the
//! person's own. All of these read and write files: call them on the blocking pool.

use meloet_model::PluginRef;

use crate::presets::{Preset, PresetError};

/// The patches Surge XT would list, ids starting with `surge:`. `None`: the plugin is not
/// Surge XT.
pub fn list(_plugin: &PluginRef) -> Option<Vec<Preset>> {
    None
}

/// The state to put into the plugin to load the patch `id`: `state` is the plugin's state
/// now (meloet's container of its format), which the answer keeps but for the patch.
pub fn load(_plugin: &PluginRef, _id: &str, _state: &[u8]) -> Result<Vec<u8>, PresetError> {
    Err(PresetError::NoSuch)
}

/// Writes the patch inside `state` as a patch of the person, named `name` in the folder
/// `category` (`None`: Surge XT's folder for patches without one). An existing file only
/// with `overwrite`.
pub fn save(
    _plugin: &PluginRef,
    _state: &[u8],
    _name: &str,
    _category: Option<&str>,
    _overwrite: bool,
) -> Result<Preset, PresetError> {
    Err(PresetError::Other("not built yet".into()))
}
