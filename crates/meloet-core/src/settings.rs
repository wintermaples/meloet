//! What is kept between runs: `settings.json` in the config directory.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use meloet_model::BuiltinInstrument;
use meloet_model::effects::EffectKind as ModelEffect;
use meloet_protocol::{DEFAULT_FIXED_VELOCITY, EffectKind, EffectRef, SoundRef};
use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "settings.json";
const VERSION: u32 = 1;
pub const SCALES: [u16; 3] = [100, 125, 150];
pub const DEFAULT_SCALE: u16 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The id of the chosen output.
    pub output: Option<String>,
    /// Its name as last seen, for the status line when the output is not in the list at all.
    pub output_name: Option<String>,
    pub scale: u16,
    pub metronome: bool,
    /// 0, 1 or 2.
    pub count_in_bars: u8,
    /// "ベロシティ固定" (B-66): every key played sounds and is recorded at `fixed_velocity`.
    pub velocity_fixed: bool,
    /// 1..=127, kept while off.
    pub fixed_velocity: u8,
    /// The starred instruments of the shelf.
    pub favorites: Vec<BuiltinInstrument>,
    /// The places the person added to look for plugins in, in the order they were added.
    pub plugin_places: Vec<PathBuf>,
    pub shelf: ShelfSettings,
}

/// What the shelf keeps of 「あとから入れたもの」 and of its effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfSettings {
    /// The starred SF2 presets, then the starred plugin instruments (each by a key of its
    /// row). The starred sounds of meloet are `Settings::favorites`.
    pub favorite_sounds: Vec<SoundRef>,
    /// The starred effects of meloet, then the starred plugin effects.
    pub favorite_effects: Vec<EffectRef>,
    /// The format the person chose for a plugin's row, as the exact key of that format.
    pub plugin_formats: Vec<String>,
}

/// Spec 3 (86-4): 「はじめはリバーブとディレイに印」.
pub const DEFAULT_FAVORITE_EFFECTS: [EffectKind; 2] = [EffectKind::Reverb, EffectKind::Delay];

impl Default for ShelfSettings {
    fn default() -> Self {
        ShelfSettings {
            favorite_sounds: Vec::new(),
            favorite_effects: (DEFAULT_FAVORITE_EFFECTS.into_iter())
                .map(|effect| EffectRef::Builtin { effect })
                .collect(),
            plugin_formats: Vec::new(),
        }
    }
}

pub const DEFAULT_COUNT_IN_BARS: u8 = 1;
/// Spec 3: 「お気に入りの印は、はじめはグランドピアノとスタジオキットについている」.
pub const DEFAULT_FAVORITES: [BuiltinInstrument; 2] =
    [BuiltinInstrument::GrandPiano, BuiltinInstrument::StudioKit];

impl Default for Settings {
    fn default() -> Self {
        Settings {
            output: None,
            output_name: None,
            scale: DEFAULT_SCALE,
            metronome: false,
            count_in_bars: DEFAULT_COUNT_IN_BARS,
            velocity_fixed: false,
            fixed_velocity: DEFAULT_FIXED_VELOCITY,
            favorites: DEFAULT_FAVORITES.to_vec(),
            plugin_places: Vec::new(),
            shelf: ShelfSettings::default(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output_name: Option<String>,
    scale: u16,
    #[serde(default)]
    metronome: bool,
    #[serde(default = "default_count_in_bars")]
    count_in_bars: u8,
    #[serde(default)]
    velocity_fixed: bool,
    #[serde(default = "default_fixed_velocity")]
    fixed_velocity: u8,
    /// The names the song file uses. Missing (a file from before the shelf): the defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    favorites: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    plugin_places: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    favorite_sf2: Vec<DiskSf2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    favorite_plugins: Vec<String>,
    /// The names the song file uses. Missing (a file from before the star): the defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    favorite_effects: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    favorite_plugin_effects: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    plugin_formats: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct DiskSf2 {
    file: String,
    bank: u16,
    program: u8,
}

fn once<T: PartialEq>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut kept = Vec::new();
    for item in items {
        if !kept.contains(&item) {
            kept.push(item);
        }
    }
    kept
}

fn shelf_from_disk(disk: &mut OnDisk) -> ShelfSettings {
    let sf2 = std::mem::take(&mut disk.favorite_sf2).into_iter();
    let sf2 = sf2.map(|s| SoundRef::Sf2 {
        file: s.file,
        bank: s.bank,
        program: s.program,
    });
    let plugins = std::mem::take(&mut disk.favorite_plugins).into_iter();
    let effects = match disk.favorite_effects.take() {
        // Names this meloet does not know are left out.
        Some(names) => (names.iter())
            .filter_map(|n| ModelEffect::from_name(n))
            .map(crate::play::mixer::effect_kind)
            .collect(),
        None => DEFAULT_FAVORITE_EFFECTS.to_vec(),
    };
    let plugin_effects = std::mem::take(&mut disk.favorite_plugin_effects).into_iter();
    ShelfSettings {
        favorite_sounds: once(sf2.chain(plugins.map(|key| SoundRef::Plugin { key }))),
        favorite_effects: once(
            (effects
                .into_iter()
                .map(|effect| EffectRef::Builtin { effect }))
            .chain(plugin_effects.map(|key| EffectRef::Plugin { key })),
        ),
        plugin_formats: once(std::mem::take(&mut disk.plugin_formats)),
    }
}

fn default_fixed_velocity() -> u8 {
    DEFAULT_FIXED_VELOCITY
}

fn default_count_in_bars() -> u8 {
    DEFAULT_COUNT_IN_BARS
}

/// `$MELOET_CONFIG_DIR`, else `$XDG_CONFIG_HOME/meloet`, else `$HOME/.config/meloet`.
pub fn config_dir() -> Option<PathBuf> {
    config_dir_from(|name| std::env::var_os(name))
}

fn config_dir_from(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    set("MELOET_CONFIG_DIR")
        .or_else(|| set("XDG_CONFIG_HOME").map(|dir| dir.join("meloet")))
        .or_else(|| set("HOME").map(|dir| dir.join(".config").join("meloet")))
}

/// A missing, unreadable or broken file gives the defaults.
pub fn load(dir: &Path) -> Settings {
    let Ok(text) = std::fs::read_to_string(dir.join(FILE_NAME)) else {
        return Settings::default();
    };
    match serde_json::from_str::<OnDisk>(&text) {
        Ok(mut disk) if disk.version == VERSION => Settings {
            shelf: shelf_from_disk(&mut disk),
            output_name: disk.output_name.filter(|_| disk.output.is_some()),
            output: disk.output,
            scale: if SCALES.contains(&disk.scale) {
                disk.scale
            } else {
                DEFAULT_SCALE
            },
            metronome: disk.metronome,
            count_in_bars: if crate::play::COUNT_IN_BARS.contains(&disk.count_in_bars) {
                disk.count_in_bars
            } else {
                DEFAULT_COUNT_IN_BARS
            },
            velocity_fixed: disk.velocity_fixed,
            fixed_velocity: if (1..=127).contains(&disk.fixed_velocity) {
                disk.fixed_velocity
            } else {
                DEFAULT_FIXED_VELOCITY
            },
            favorites: match disk.favorites {
                // Names this meloet does not know are left out; each instrument once.
                Some(names) => {
                    let mut favorites = Vec::new();
                    for b in names.iter().filter_map(|n| BuiltinInstrument::from_name(n)) {
                        if !favorites.contains(&b) {
                            favorites.push(b);
                        }
                    }
                    favorites
                }
                None => DEFAULT_FAVORITES.to_vec(),
            },
            plugin_places: {
                let mut places: Vec<PathBuf> = Vec::new();
                for place in disk.plugin_places {
                    if place.is_absolute() && !places.contains(&place) {
                        places.push(place);
                    }
                }
                places
            },
        },
        _ => Settings::default(),
    }
}

/// Writes a temporary file next to the real one and renames it over it.
pub fn save(dir: &Path, settings: &Settings) -> std::io::Result<()> {
    let shelf = &settings.shelf;
    let disk = OnDisk {
        version: VERSION,
        output: settings.output.clone(),
        output_name: settings.output_name.clone(),
        scale: settings.scale,
        metronome: settings.metronome,
        count_in_bars: settings.count_in_bars,
        velocity_fixed: settings.velocity_fixed,
        fixed_velocity: settings.fixed_velocity,
        favorites: Some(
            settings
                .favorites
                .iter()
                .map(|b| b.name().to_owned())
                .collect(),
        ),
        plugin_places: settings.plugin_places.clone(),
        favorite_sf2: (shelf.favorite_sounds.iter())
            .filter_map(|s| match s {
                SoundRef::Sf2 {
                    file,
                    bank,
                    program,
                } => Some(DiskSf2 {
                    file: file.clone(),
                    bank: *bank,
                    program: *program,
                }),
                _ => None,
            })
            .collect(),
        favorite_plugins: (shelf.favorite_sounds.iter())
            .filter_map(|s| match s {
                SoundRef::Plugin { key } => Some(key.clone()),
                _ => None,
            })
            .collect(),
        favorite_effects: Some(
            (shelf.favorite_effects.iter())
                .filter_map(|e| match e {
                    EffectRef::Builtin { effect } => {
                        Some(crate::play::mixer::model_effect(*effect).name().to_owned())
                    }
                    EffectRef::Plugin { .. } => None,
                })
                .collect(),
        ),
        favorite_plugin_effects: (shelf.favorite_effects.iter())
            .filter_map(|e| match e {
                EffectRef::Plugin { key } => Some(key.clone()),
                EffectRef::Builtin { .. } => None,
            })
            .collect(),
        plugin_formats: shelf.plugin_formats.clone(),
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
pub(crate) fn temp_dir(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "meloet-core-test-{}-{label}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create a temporary directory");
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_settings_read_back() {
        let dir = temp_dir("settings");
        let settings = Settings {
            output: Some("alsa_card.pci-0000_0c_00.6:analog-output-headphones".into()),
            output_name: Some("アナログヘッドフォン".into()),
            scale: 125,
            metronome: true,
            count_in_bars: 2,
            velocity_fixed: true,
            fixed_velocity: 64,
            favorites: vec![BuiltinInstrument::Sine, BuiltinInstrument::Organ],
            plugin_places: vec!["/home/me/Plugins".into(), "/opt/vst3".into()],
            shelf: ShelfSettings {
                favorite_sounds: vec![
                    SoundRef::Sf2 {
                        file: "FluidR3_GM.sf2".into(),
                        bank: 128,
                        program: 8,
                    },
                    SoundRef::Plugin {
                        key: "lv2:urn:x:synth".into(),
                    },
                ],
                favorite_effects: vec![
                    EffectRef::Builtin {
                        effect: EffectKind::Chorus,
                    },
                    EffectRef::Plugin {
                        key: "vst3:ABCDEF0123456789ABCDEF0123456789".into(),
                    },
                ],
                plugin_formats: vec!["lv2:urn:x:synth".into()],
            },
        };
        save(&dir, &settings).unwrap();
        assert_eq!(load(&dir), settings);
        assert!(!dir.join("settings.json.tmp").exists());

        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["scale"], 125);
        assert_eq!(json["metronome"], true);
        assert_eq!(json["count_in_bars"], 2);
        assert_eq!(json["velocity_fixed"], true);
        assert_eq!(json["fixed_velocity"], 64);
        assert_eq!(json["favorites"], serde_json::json!(["sine", "organ"]));
        assert_eq!(
            json["output"],
            "alsa_card.pci-0000_0c_00.6:analog-output-headphones"
        );

        save(&dir, &Settings::default()).unwrap();
        let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(text.contains("\"output\": null"), "{text}");
        assert_eq!(load(&dir), Settings::default());
    }

    #[test]
    fn the_plain_file_of_the_spec_is_read() {
        let dir = temp_dir("plain");
        std::fs::write(
            dir.join("settings.json"),
            r#"{ "version": 1, "output": "x:y", "scale": 150 }"#,
        )
        .unwrap();
        assert_eq!(
            load(&dir),
            Settings {
                output: Some("x:y".into()),
                output_name: None,
                scale: 150,
                metronome: false,
                count_in_bars: 1,
                velocity_fixed: false,
                fixed_velocity: 100,
                favorites: vec![BuiltinInstrument::GrandPiano, BuiltinInstrument::StudioKit],
                plugin_places: vec![],
                shelf: ShelfSettings::default(),
            }
        );
    }

    #[test]
    fn favorites_may_be_empty_and_unknown_names_are_left_out() {
        let dir = temp_dir("favorites");
        let write = |favorites: &str| {
            std::fs::write(
                dir.join("settings.json"),
                format!(r#"{{"version":1,"output":null,"scale":100,"favorites":{favorites}}}"#),
            )
            .unwrap();
        };
        write("[]");
        assert_eq!(load(&dir).favorites, []);
        write(r#"["warmPad", "theremin", "sine", "warmPad"]"#);
        assert_eq!(
            load(&dir).favorites,
            [BuiltinInstrument::WarmPad, BuiltinInstrument::Sine]
        );
    }

    #[test]
    fn missing_or_broken_files_give_the_defaults() {
        let dir = temp_dir("broken");
        assert_eq!(load(&dir), Settings::default());
        assert_eq!(load(&dir.join("not-there")), Settings::default());
        for broken in [
            "",
            "{",
            "[]",
            r#"{"version":2,"output":"x","scale":100}"#,
            r#"{"version":1,"output":7,"scale":100}"#,
        ] {
            std::fs::write(dir.join("settings.json"), broken).unwrap();
            assert_eq!(load(&dir), Settings::default(), "{broken:?}");
        }
        std::fs::write(
            dir.join("settings.json"),
            r#"{"version":1,"output":"x","scale":110}"#,
        )
        .unwrap();
        assert_eq!(load(&dir).scale, 100);
        assert_eq!(load(&dir).output.as_deref(), Some("x"));

        std::fs::write(
            dir.join("settings.json"),
            r#"{"version":1,"output":null,"scale":100,"metronome":true,"count_in_bars":3}"#,
        )
        .unwrap();
        assert!(load(&dir).metronome);
        assert_eq!(load(&dir).count_in_bars, 1, "out of range: the default");

        std::fs::write(
            dir.join("settings.json"),
            r#"{"version":1,"output":null,"scale":100,"velocity_fixed":true,"fixed_velocity":0}"#,
        )
        .unwrap();
        assert!(load(&dir).velocity_fixed);
        assert_eq!(load(&dir).fixed_velocity, 100, "out of range: the default");
    }

    #[test]
    fn config_dir_order() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| OsString::from(v))
            }
        };
        assert_eq!(
            config_dir_from(env(&[
                ("MELOET_CONFIG_DIR", "/t"),
                ("XDG_CONFIG_HOME", "/x"),
                ("HOME", "/h")
            ])),
            Some(PathBuf::from("/t"))
        );
        assert_eq!(
            config_dir_from(env(&[("XDG_CONFIG_HOME", "/x"), ("HOME", "/h")])),
            Some(PathBuf::from("/x/meloet"))
        );
        assert_eq!(
            config_dir_from(env(&[("XDG_CONFIG_HOME", ""), ("HOME", "/h")])),
            Some(PathBuf::from("/h/.config/meloet"))
        );
        assert_eq!(config_dir_from(env(&[])), None);
    }
}
