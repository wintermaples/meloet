//! Presets of plugins: what a source of presets gives and takes. The tools are in
//! `tools/presets.rs`; a source reads files or asks the plugin, and in the end a preset is
//! a state put into the plugin (`Tools::put_plugin_state`).

pub mod lv2;
pub mod vst3;

use std::path::{Path, PathBuf};

/// Characters of a preset's name or category.
pub const MAX_NAME: usize = 64;

/// The folders presets are looked for in and written to. Made once, from the environment,
/// or by a test that points them at folders of its own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresetDirs {
    /// The person's VST3 presets (`$HOME/.vst3/presets`): VST3 presets are written there.
    pub vst3_user: Option<PathBuf>,
    /// VST3 presets that came with plugins.
    pub vst3_shared: Vec<PathBuf>,
    /// The person's LV2 folder (`$HOME/.lv2`): LV2 presets are written there.
    pub lv2_user: Option<PathBuf>,
    /// The other folders of LV2 bundles.
    pub lv2_shared: Vec<PathBuf>,
}

impl PresetDirs {
    /// Where the VST3 SDK and lilv look (docs/research/2026-11-23-plugin-presets-state.md
    /// Q8, Q9; `LV2_PATH` as meloet's scan reads it).
    pub fn from_env() -> PresetDirs {
        let home = (std::env::var_os("HOME"))
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .filter(|h| h.is_absolute());
        let mut lv2_shared: Vec<PathBuf> = ["/usr/lib/lv2", "/usr/local/lib/lv2"]
            .map(PathBuf::from)
            .to_vec();
        let lv2_user = home.as_ref().map(|h| h.join(".lv2"));
        if let Some(paths) = std::env::var_os("LV2_PATH") {
            for path in std::env::split_paths(&paths) {
                if path.is_absolute()
                    && !lv2_shared.contains(&path)
                    && Some(&path) != lv2_user.as_ref()
                {
                    lv2_shared.push(path);
                }
            }
        }
        PresetDirs {
            vst3_user: home.as_ref().map(|h| h.join(".vst3/presets")),
            vst3_shared: ["/usr/share/vst3/presets", "/usr/local/share/vst3/presets"]
                .map(PathBuf::from)
                .to_vec(),
            lv2_user,
            lv2_shared,
        }
    }

    /// Only the person's folders, under `home`, as `from_env` makes them.
    pub fn home(home: &Path) -> PresetDirs {
        PresetDirs {
            vst3_user: Some(home.join(".vst3/presets")),
            vst3_shared: Vec::new(),
            lv2_user: Some(home.join(".lv2")),
            lv2_shared: Vec::new(),
        }
    }
}

/// A name a preset is saved under, or its category: 1 to [`MAX_NAME`] characters, no path
/// separators and no control characters, not only dots. Spaces at the ends are cut.
pub fn name_ok(what: &str, name: &str) -> Result<String, String> {
    let name = name.trim();
    let count = name.chars().count();
    if count == 0
        || count > MAX_NAME
        || name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
        || name.chars().all(|c| c == '.')
    {
        return Err(format!(
            "a preset's {what} is 1 to {MAX_NAME} characters, with no / or \\ and no control \
             characters"
        ));
    }
    Ok(name.to_owned())
}

/// Whether `path` is under `root` with no symbolic link between them (`root` itself may be
/// one), and has no `..`.
pub fn inside(root: &Path, path: &Path) -> bool {
    let Ok(rest) = path.strip_prefix(root) else {
        return false;
    };
    let mut at = root.to_path_buf();
    for part in rest.components() {
        let std::path::Component::Normal(part) = part else {
            return false;
        };
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return false,
            _ => {}
        }
    }
    true
}

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
