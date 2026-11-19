//! VST3. Empty until the second step of round 5.

use std::path::Path;

use meloet_plugin::PluginKey;
use meloet_plugin::wire::ScanOutput;

use super::{Activation, Loaded};

pub fn load(_key: &PluginKey, _bundle: &Path, _activation: Activation) -> Result<Loaded, String> {
    Err("VST3 plugins cannot be loaded yet".into())
}

pub fn scan(_bundle: &Path) -> ScanOutput {
    ScanOutput::Broken {
        message: "VST3 bundles cannot be scanned yet".into(),
    }
}
