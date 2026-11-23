//! A plugin's state in a form an AI can read, for the plugins meloet knows how to read:
//! Surge XT, whose patch is XML (`crate::surge`). `get_plugin_state` gives it beside the
//! bytes, and `set_plugin_state_text` puts an edited text back.
//!
//! The bytes are the plugin host's container (`crates/meloet-plughost/src/formats/`):
//! - VST3 (`vst3.rs`, `save_state`): `MV3S`, the length of the component's state (u64,
//!   little endian), the component's state (`IComponent::getState`), then the controller's
//!   (`IEditController::getState`) to the end.
//! - LV2 (`lv2.rs`, `save_state`): `ML2S`, a count (u32, little endian), that many pairs of a
//!   parameter id (u32) and its value 0..=1 (f64), then the state as lilv writes it, Turtle
//!   text in UTF-8 to the end.

use meloet_model::PluginRef;

use crate::surge;

/// A state as text.
#[derive(Debug, Clone, PartialEq)]
pub struct Readable {
    /// What the text is, such as "xml".
    pub kind: &'static str,
    pub text: String,
}

/// The state of `plugin` as text, when meloet knows how to read that plugin's state.
pub fn readable(plugin: &PluginRef, bytes: &[u8]) -> Option<Readable> {
    let format = surge::format_of(plugin).ok()?;
    let text = surge::xml_of(format, bytes).ok()?;
    Some(Readable { kind: "xml", text })
}

/// Whether meloet reads the state of `plugin` as text.
pub fn knows(plugin: &PluginRef) -> bool {
    surge::is_surge(plugin)
}

/// The state `now` of `plugin` with `text` (as [`readable`] gives it, edited) in place of
/// what the text stands for; the rest of the state as it is.
pub fn with_text(plugin: &PluginRef, now: &[u8], text: &str) -> Result<Vec<u8>, String> {
    let format = surge::format_of(plugin).map_err(|_| {
        format!(
            "meloet reads the state of {} in no text form (only Surge XT's); use \
             set_plugin_state",
            plugin.name
        )
    })?;
    surge::with_xml(format, now, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_surge_xt_is_read_as_text() {
        let sine = PluginRef::new("lv2:urn:meloet:fake:sine", "Sine", "meloet", "");
        assert_eq!(readable(&sine, &0.5f64.to_le_bytes()), None);
        assert!(!knows(&sine));
        let refused = with_text(&sine, &0.5f64.to_le_bytes(), "<patch/>").unwrap_err();
        assert!(refused.contains("only Surge XT"), "{refused}");
    }
}
