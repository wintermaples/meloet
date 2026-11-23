//! A plugin's state in a form an AI can read, for the plugins meloet knows how to read.
//! `get_plugin_state` gives it beside the bytes. None is known yet.
//!
//! The bytes are the plugin host's container (`crates/meloet-plughost/src/formats/`):
//! - VST3 (`vst3.rs`, `save_state`): `MV3S`, the length of the component's state (u64,
//!   little endian), the component's state (`IComponent::getState`), then the controller's
//!   (`IEditController::getState`) to the end.
//! - LV2 (`lv2.rs`, `save_state`): `ML2S`, a count (u32, little endian), that many pairs of a
//!   parameter id (u32) and its value 0..=1 (f64), then the state as lilv writes it, Turtle
//!   text in UTF-8 to the end.

use meloet_model::PluginRef;

/// A state as text.
#[derive(Debug, Clone, PartialEq)]
pub struct Readable {
    /// What the text is, such as "xml".
    pub kind: &'static str,
    pub text: String,
}

/// The state of `plugin` as text, when meloet knows how to read that plugin's state.
pub fn readable(_plugin: &PluginRef, _bytes: &[u8]) -> Option<Readable> {
    None
}
