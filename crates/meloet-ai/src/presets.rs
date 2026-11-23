//! Presets of plugins: what a source of presets gives and takes. The tools are in
//! `tools/presets.rs`; a source reads files or asks the plugin, and in the end a preset is
//! a state put into the plugin (`Tools::put_plugin_state`).

/// One preset a plugin can load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    /// What `load_preset` takes; unique among the presets of one plugin and stable between
    /// calls. Starts with the source's name and a colon.
    pub id: String,
    pub name: String,
    /// A category, bank or folder.
    pub category: Option<String>,
    /// Shipped with the plugin (not written by the person or by meloet).
    pub factory: bool,
}

/// Why a preset was not loaded or saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetError {
    /// No preset of that id.
    NoSuch,
    /// A preset of that name is there and `overwrite` was not given.
    Exists(std::path::PathBuf),
    /// Anything else, in words for the AI.
    Other(String),
}
