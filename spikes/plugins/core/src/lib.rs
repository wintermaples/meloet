//! The format-free boundary: what the rest of the DAW sees of a plugin, whether it is VST3 or LV2.
use std::path::PathBuf;

pub mod x11win;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Format {
    Vst3,
    Lv2,
}

/// 音源 or エフェクト. Decided from the plugin's own metadata, never from its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Instrument,
    Effect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UiKind {
    /// VST3 IPlugView with kPlatformTypeX11EmbedWindowID, or LV2 ui:X11UI. Needs an X11 parent window.
    X11Embed,
    Gtk3,
    Qt5,
    Qt6,
    /// LV2 kx external UI or ui:showInterface: the plugin opens its own top-level window.
    External,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PluginInfo {
    pub format: Format,
    /// VST3: class id as hex. LV2: plugin URI.
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub kind: Kind,
    pub path: PathBuf,
    pub uis: Vec<UiKind>,
    pub audio_ins: u32,
    pub audio_outs: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ParamInfo {
    pub id: u32,
    pub name: String,
    /// Always 0..1 at this boundary. LV2 ranges are mapped by the LV2 side.
    pub default: f64,
    pub automatable: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct MidiEvent {
    pub frame: u32,
    pub data: [u8; 3],
}

pub type Result<T> = std::result::Result<T, String>;

/// Main-thread side. In the real engine `process` moves to an audio-thread handle;
/// the spike renders offline on one thread so every check is deterministic.
pub trait Plugin {
    fn info(&self) -> &PluginInfo;
    fn activate(&mut self, sample_rate: f64, max_block: usize) -> Result<()>;
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], midi: &[MidiEvent]);
    fn params(&mut self) -> Vec<ParamInfo>;
    fn get_param(&mut self, id: u32) -> f64;
    fn set_param(&mut self, id: u32, value: f64);
    fn save_state(&mut self) -> Result<Vec<u8>>;
    fn load_state(&mut self, data: &[u8]) -> Result<()>;
    /// Attach the plugin GUI to an X11 window. Returns the size the GUI wants.
    fn open_editor(&mut self, parent: u32) -> Result<(u32, u32)>;
    fn close_editor(&mut self);
    /// Called ~60 times a second on the GUI thread while an editor is open.
    fn editor_idle(&mut self);
}
