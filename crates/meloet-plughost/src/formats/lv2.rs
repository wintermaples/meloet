//! LV2. Empty until the second step of round 5.

use std::path::Path;

use meloet_plugin::{PluginInfo, PluginKey};

use super::{Activation, Candidate, Loaded};

pub fn load(_key: &PluginKey, _bundle: &Path, _activation: Activation) -> Result<Loaded, String> {
    Err("LV2 plugins cannot be loaded yet".into())
}

/// The plugins of the bundle; `Err` when the bundle cannot be read.
pub fn list(_bundle: &Path) -> Result<Vec<Candidate>, String> {
    Err("LV2 bundles cannot be scanned yet".into())
}

/// Reads one plugin of [`list`]; `Err` when that plugin cannot be read.
pub fn probe(_bundle: &Path, _id: &str) -> Result<PluginInfo, String> {
    Err("LV2 plugins cannot be scanned yet".into())
}
