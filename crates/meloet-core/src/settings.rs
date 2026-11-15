//! What is kept between runs: `settings.json` in the config directory.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

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
}

pub const DEFAULT_COUNT_IN_BARS: u8 = 1;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            output: None,
            output_name: None,
            scale: DEFAULT_SCALE,
            metronome: false,
            count_in_bars: DEFAULT_COUNT_IN_BARS,
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
        Ok(disk) if disk.version == VERSION => Settings {
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
        },
        _ => Settings::default(),
    }
}

/// Writes a temporary file next to the real one and renames it over it.
pub fn save(dir: &Path, settings: &Settings) -> std::io::Result<()> {
    let disk = OnDisk {
        version: VERSION,
        output: settings.output.clone(),
        output_name: settings.output_name.clone(),
        scale: settings.scale,
        metronome: settings.metronome,
        count_in_bars: settings.count_in_bars,
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
            }
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
