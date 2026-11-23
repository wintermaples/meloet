//! Surge XT: its patch inside a plugin state, its patch files (.fxp) as presets, and its
//! modulation routing (docs/research/2026-11-23-plugin-presets-state.md).

mod chunk;
mod fxp;
mod juce64;
pub(crate) mod patch;
pub mod presets;
pub(crate) mod sources;
mod state;
mod xml;

use meloet_model::PluginRef;

pub use crate::presets::{Preset, PresetError};
pub(crate) use state::Format;

/// Surge XT's keys as this PC's scan gives them (Surge XT 1.3.4): the synth only, not
/// "Surge XT Effects".
const VST3_KEY: &str = "vst3:ABCDEF019182FAEB566D624153675854";
const LV2_URI: &str = "https://surge-synthesizer.github.io/lv2/surge-xt";

/// Whether the plugin is Surge XT (the synth, VST3 or LV2).
pub fn is_surge(plugin: &PluginRef) -> bool {
    let key = &*plugin.key;
    key == VST3_KEY || key.strip_prefix("lv2:") == Some(LV2_URI)
}

/// The format of a plugin of Surge XT, or why it is none.
pub(crate) fn format_of(plugin: &PluginRef) -> Result<Format, String> {
    match Format::of_key(&plugin.key) {
        Some(format) if is_surge(plugin) => Ok(format),
        _ => Err(format!(
            "{} is not Surge XT (VST3 or LV2); this works only on Surge XT",
            plugin.name
        )),
    }
}

/// The patch XML inside a state of Surge XT.
pub(crate) fn xml_of(format: Format, state: &[u8]) -> Result<String, String> {
    let bytes = state::chunk_of(format, state)?;
    Ok(chunk::parse(&bytes)?.text()?.to_owned())
}

/// The state with `xml` as its patch: `xml` must be a patch (well-formed XML whose root is
/// `<patch>` with `<parameters>`); the embedded wavetables and the rest of the state are
/// kept.
pub(crate) fn with_xml(format: Format, state: &[u8], xml: &str) -> Result<Vec<u8>, String> {
    patch::Patch::parse(xml)?;
    let bytes = state::chunk_of(format, state)?;
    let chunk = chunk::parse(&bytes)?;
    let new = chunk::with_xml(&chunk, xml)?;
    state::with_chunk(format, state, &new)
}

/// The id of a patch parameter (`a_lfo0_rate`) in the plugin's list of parameters: JUCE
/// makes the VST3 id from the parameter's own name (a 31-bit string hash); meloet's plugin
/// host makes an LV2 one from the URI `…/surge-xt:<name>` (an FNV-1a hash with the top
/// bit, `crates/meloet-plughost/src/formats/lv2.rs` `patch_id`; another id when two
/// collide, which this does not follow).
pub(crate) fn param_id(format: Format, name: &str) -> u32 {
    match format {
        Format::Vst3 => {
            let hash = (name.chars()).fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32));
            hash & 0x7fff_ffff
        }
        Format::Lv2 => {
            let uri = format!("{LV2_URI}:{name}");
            let hash = (uri.bytes()).fold(0x811c_9dc5u32, |h, b| {
                (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
            });
            hash | 1 << 31
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surge_xt_is_known_by_its_keys() {
        let plugin = |key: &str| PluginRef::new(key, "Surge XT", "Surge Synth Team", "");
        assert!(is_surge(&plugin(VST3_KEY)));
        assert!(is_surge(&plugin(&format!("lv2:{LV2_URI}"))));
        assert!(!is_surge(&plugin(&format!("lv2:{LV2_URI}-fx"))));
        assert!(!is_surge(&plugin("vst3:ABCDEF019182FAEB566D624153675855")));
        assert!(format_of(&plugin("lv2:urn:meloet:fake:sine")).is_err());
        assert_eq!(format_of(&plugin(VST3_KEY)), Ok(Format::Vst3));
    }

    /// Ids read from Surge XT 1.3.4's parameter lists on this PC.
    #[test]
    fn parameter_ids_are_those_of_the_plugin() {
        for (name, vst3, lv2) in [
            ("a_pitch", 867_473_890, 2_722_519_831),
            ("a_lfo0_rate", 1_235_992_486, 3_973_154_385),
            ("a_lfo0_delay", 1_795_727_229, 2_673_915_914),
            ("fx1_type", 145_909_210, 3_144_570_145),
        ] {
            assert_eq!(param_id(Format::Vst3, name), vst3, "{name}");
            assert_eq!(param_id(Format::Lv2, name), lv2, "{name}");
        }
    }
}
