//! Which plugin: the exact key a song keeps, and the looser key the shelf merges rows by
//! (docs/architecture.md 20-5).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::category::PluginRole;

/// CLAP is not here (20-9). A new format is a new arm here, a new prefix in [`PluginKey`] and a
/// new place in [`PluginFormat::PREFERENCE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginFormat {
    Vst3,
    Lv2,
}

impl PluginFormat {
    pub const ALL: [PluginFormat; 2] = [PluginFormat::Vst3, PluginFormat::Lv2];
    /// Spec 3: of the formats a plugin comes in, the one used when nothing else decides (the
    /// song already uses one, only one opens a window) is the first here.
    pub const PREFERENCE: [PluginFormat; 2] = [PluginFormat::Vst3, PluginFormat::Lv2];

    /// The prefix of [`PluginKey`]'s text and the word on the command line of the host.
    pub fn prefix(self) -> &'static str {
        match self {
            PluginFormat::Vst3 => "vst3",
            PluginFormat::Lv2 => "lv2",
        }
    }

    pub fn from_prefix(prefix: &str) -> Option<PluginFormat> {
        PluginFormat::ALL.into_iter().find(|f| f.prefix() == prefix)
    }
}

/// One plugin in one format: `vst3:<cid of the Audio Module Class, 32 hex digits>` or
/// `lv2:<URI>`. Songs and the scan file keep it as that text. The same plugin in another
/// format is another key: neither its parameters nor its state carry over.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct PluginKey {
    format: PluginFormat,
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadKey(pub String);

impl fmt::Display for BadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a plugin key: {}", self.0)
    }
}

impl std::error::Error for BadKey {}

impl PluginKey {
    /// `cid` as the 32 hex digits VST3 writes it with (`moduleinfo.json`'s "CID"); either case.
    pub fn vst3(cid: &str) -> Result<PluginKey, BadKey> {
        if cid.len() == 32 && cid.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(PluginKey {
                format: PluginFormat::Vst3,
                id: cid.to_ascii_uppercase(),
            })
        } else {
            Err(BadKey(format!("vst3:{cid}")))
        }
    }

    pub fn vst3_bytes(cid: [u8; 16]) -> PluginKey {
        PluginKey {
            format: PluginFormat::Vst3,
            id: cid.iter().map(|b| format!("{b:02X}")).collect(),
        }
    }

    pub fn lv2(uri: &str) -> Result<PluginKey, BadKey> {
        if uri.is_empty() || uri.chars().any(char::is_whitespace) {
            Err(BadKey(format!("lv2:{uri}")))
        } else {
            Ok(PluginKey {
                format: PluginFormat::Lv2,
                id: uri.to_string(),
            })
        }
    }

    pub fn format(&self) -> PluginFormat {
        self.format
    }

    /// The cid in upper-case hex, or the URI.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for PluginKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.format.prefix(), self.id)
    }
}

impl FromStr for PluginKey {
    type Err = BadKey;

    fn from_str(text: &str) -> Result<PluginKey, BadKey> {
        let bad = || BadKey(text.to_string());
        let (prefix, id) = text.split_once(':').ok_or_else(bad)?;
        match PluginFormat::from_prefix(prefix).ok_or_else(bad)? {
            PluginFormat::Vst3 => PluginKey::vst3(id),
            PluginFormat::Lv2 => PluginKey::lv2(id),
        }
    }
}

impl From<PluginKey> for String {
    fn from(key: PluginKey) -> String {
        key.to_string()
    }
}

impl TryFrom<String> for PluginKey {
    type Error = BadKey;

    fn try_from(text: String) -> Result<PluginKey, BadKey> {
        text.parse()
    }
}

/// Plugins with the same merge key are one row of the shelf. Only for showing: nothing is
/// loaded or saved by it. Two plugins merge only when the role, the tidied name and the tidied
/// vendor are all the same; a name that merely starts the same does not merge. When in doubt
/// this keeps two rows, as a wrong merge would hide a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MergeKey {
    pub role: PluginRole,
    pub name: String,
    pub vendor: String,
}

impl MergeKey {
    pub fn new(role: PluginRole, name: &str, vendor: &str) -> MergeKey {
        MergeKey {
            role,
            name: tidy_name(name),
            vendor: tidy_vendor(vendor),
        }
    }
}

const FORMAT_WORDS: [&str; 4] = ["vst3", "vst", "lv2", "clap"];
/// What a name may say in brackets at its end without being another plugin.
const TAIL_WORDS: [&str; 2] = ["stereo", "mono"];

fn is_format_word(word: &str) -> bool {
    FORMAT_WORDS.contains(&word.trim_matches(|c: char| !c.is_alphanumeric()))
}

/// Lower case, the format's name taken out wherever it stands as a word of its own, and a
/// "(Stereo)" or "(Mono)" at the end taken off.
fn tidy_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut words: Vec<&str> = lower
        .split_whitespace()
        .filter(|w| !is_format_word(w))
        .collect();
    while let Some(last) = words.last() {
        let inner = last
            .strip_prefix(['(', '['])
            .and_then(|w| w.strip_suffix([')', ']']));
        match inner {
            Some(inner) if TAIL_WORDS.contains(&inner) => words.pop(),
            _ => break,
        };
    }
    words.join(" ")
}

/// Lower case, and the format's name taken off the end ("LSP VST3" → "lsp").
fn tidy_vendor(vendor: &str) -> String {
    let lower = vendor.to_lowercase();
    let mut words: Vec<&str> = lower.split_whitespace().collect();
    while words.len() > 1 && words.last().is_some_and(|w| is_format_word(w)) {
        words.pop();
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use PluginRole::{Effect, Instrument};

    #[test]
    fn key_text_round_trips() {
        let vst3 = PluginKey::vst3("647370206b31732020202020756e7363").unwrap();
        assert_eq!(vst3.to_string(), "vst3:647370206B31732020202020756E7363");
        assert_eq!(vst3.to_string().parse::<PluginKey>().unwrap(), vst3);
        let bytes = [
            0x64, 0x73, 0x70, 0x20, 0x6b, 0x31, 0x73, 0x20, 0x20, 0x20, 0x20, 0x20, 0x75, 0x6e,
            0x73, 0x63,
        ];
        assert_eq!(PluginKey::vst3_bytes(bytes), vst3);

        let lv2: PluginKey = "lv2:urn:zamaudio:ZamComp".parse().unwrap();
        assert_eq!(lv2.format(), PluginFormat::Lv2);
        assert_eq!(lv2.id(), "urn:zamaudio:ZamComp");
        let json = "\"lv2:urn:zamaudio:ZamComp\"";
        assert_eq!(serde_json::to_string(&lv2).unwrap(), json);
        assert_eq!(serde_json::from_str::<PluginKey>(json).unwrap(), lv2);
    }

    #[test]
    fn bad_keys_are_refused() {
        for text in [
            "",
            "surge",
            "clap:org.surge-synth-team.surge-xt",
            "vst3:1234",
            "vst3:647370206B31732020202020756E73ZZ",
            "lv2:",
            "lv2:has space",
        ] {
            assert!(text.parse::<PluginKey>().is_err(), "{text}");
            assert!(
                serde_json::from_str::<PluginKey>(&format!("\"{text}\"")).is_err(),
                "{text}"
            );
        }
    }

    // The names and vendors of docs/history/research/plugin-formats.md 6, as installed on the PC they
    // were read from.
    #[test]
    fn the_same_plugin_in_two_formats_merges() {
        assert_eq!(
            MergeKey::new(Instrument, "Surge XT", "Surge Synth Team"),
            MergeKey::new(Instrument, "Surge XT", "Surge Synth Team")
        );
        assert_eq!(
            MergeKey::new(Effect, "ZamComp", "Damien Zammit"),
            MergeKey::new(Effect, "ZamComp", "Damien Zammit")
        );
        assert_eq!(
            MergeKey::new(Effect, "ZamComp LV2", "Damien Zammit"),
            MergeKey::new(Effect, "zamcomp (Stereo)", "DAMIEN ZAMMIT VST3")
        );
    }

    #[test]
    fn plugins_that_only_look_alike_do_not_merge() {
        // Another plugin whose name starts the same, and of another role.
        assert_ne!(
            MergeKey::new(Instrument, "Surge XT", "Surge Synth Team"),
            MergeKey::new(Effect, "Surge XT Effects", "Surge Synth Team")
        );
        assert_ne!(
            MergeKey::new(Effect, "Surge XT", "Surge Synth Team"),
            MergeKey::new(Effect, "Surge XT Effects", "Surge Synth Team")
        );
        // The same name and vendor, one an instrument and one an effect.
        assert_ne!(
            MergeKey::new(Instrument, "Surge XT", "Surge Synth Team"),
            MergeKey::new(Effect, "Surge XT", "Surge Synth Team")
        );
        // LSP names its plugins differently in each format: they stay two rows (20-5).
        let lv2 = MergeKey::new(Effect, "LSP Compressor Stereo", "LSP LV2");
        let vst3 = MergeKey::new(Effect, "Compressor Stereo", "LSP VST3");
        assert_eq!(lv2.vendor, vst3.vendor);
        assert_ne!(lv2, vst3);
        // Other LSP compressors must not fall together with it.
        for other in [
            "Sidechain Compressor Stereo",
            "GOTT Compressor Stereo",
            "Compressor Mono",
            "Compressor LeftRight",
        ] {
            assert_ne!(MergeKey::new(Effect, other, "LSP VST3"), vst3, "{other}");
        }
        // "Stereo" is taken off only in brackets at the end.
        assert_ne!(
            MergeKey::new(Effect, "Compressor Stereo", "LSP VST3"),
            MergeKey::new(Effect, "Compressor", "LSP VST3")
        );
        assert_ne!(
            MergeKey::new(Effect, "Compressor", "A"),
            MergeKey::new(Effect, "Compressor", "B")
        );
    }

    #[test]
    fn a_vendor_that_is_only_a_format_word_is_kept() {
        assert_eq!(MergeKey::new(Effect, "x", "LV2").vendor, "lv2");
    }
}
