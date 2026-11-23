//! Surge XT: its patch inside a plugin state, its patch files (.fxp) as presets, and its
//! modulation routing (docs/research/2026-11-23-plugin-presets-state.md).

pub mod presets;

use meloet_model::PluginRef;

/// Whether the plugin is Surge XT (the synth, VST3 or LV2).
pub fn is_surge(_plugin: &PluginRef) -> bool {
    false
}
