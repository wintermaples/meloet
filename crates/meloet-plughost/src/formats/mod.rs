//! The boundary every plugin format sits behind, in two halves by thread
//! (docs/report.md 4-5):
//!
//! - [`Processor`] lives on the audio thread of this process (`crate::rt`).
//! - [`Controller`] lives on the main thread (`crate::control`), with the plugin's window.
//!
//! Nothing of a format (COM pointers, lilv types) shows outside its module. Parameter values
//! are 0..=1 here; a format maps them to the plugin's own ranges.
//!
//! How a parameter change travels: from meloet's audio thread it arrives in the block's
//! events. From the main thread (`Controller::set_param`, asked for over the socket),
//! `crate::control` also hands it to the audio thread, which puts it into the next block's
//! events. So a `Processor` learns of every change through [`ProcessBlock::events`] and of
//! nothing else, and a `Controller` never has to reach its `Processor` itself.

pub mod fake;
pub mod lv2;
pub mod vst3;

use std::path::Path;

use meloet_plugin::ipc::{FromPlugin, ToPlugin, Transport};
use meloet_plugin::wire::{LoadedPlugin, ParamInfo};
use meloet_plugin::{PluginFormat, PluginKey};

/// What a plugin is made ready for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Activation {
    pub sample_rate: f64,
    /// No block is longer than this.
    pub max_frames: usize,
}

/// What a processor tells meloet during a block. Holds a fixed number of events; more are
/// dropped.
// The fake plugins say nothing; the formats will.
#[allow(dead_code)]
pub struct EventSink {
    events: Vec<FromPlugin>,
}

#[allow(dead_code)]
impl EventSink {
    pub fn with_capacity(capacity: usize) -> EventSink {
        EventSink {
            events: Vec::with_capacity(capacity),
        }
    }

    /// False when it is full.
    pub fn push(&mut self, event: FromPlugin) -> bool {
        let fits = self.events.len() < self.events.capacity();
        if fits {
            self.events.push(event);
        }
        fits
    }

    pub fn events(&self) -> &[FromPlugin] {
        &self.events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// One block. All four slices have the block's length, 1..=`Activation::max_frames`.
#[allow(dead_code)]
pub struct ProcessBlock<'a> {
    /// Left and right. Silence for an instrument.
    pub inputs: [&'a [f32]; 2],
    /// Left and right, to be overwritten completely.
    pub outputs: [&'a mut [f32]; 2],
    /// In the order they were sent; each one's frame is inside the block.
    pub events: &'a [ToPlugin],
    pub transport: &'a Transport,
    pub sink: &'a mut EventSink,
}

/// The half of a plugin on the audio thread. `process` and `reset` must not allocate, lock or
/// block. It is dropped on the main thread.
pub trait Processor: Send {
    fn process(&mut self, block: &mut ProcessBlock<'_>);
    /// Silence at once; parameters stay.
    fn reset(&mut self);
    /// The plugin's delay in frames, as of the last `process`. `crate::rt` tells meloet when
    /// it changes.
    fn latency(&self) -> u32 {
        0
    }
}

/// The half of a plugin on the main thread.
// The windows are not opened yet.
#[allow(dead_code)]
pub trait Controller {
    fn params(&mut self) -> Vec<ParamInfo>;
    /// The value and the text the plugin shows for it; `None` for a parameter it does not have.
    fn param(&mut self, id: u32) -> Option<(f64, String)>;
    fn set_param(&mut self, id: u32, value: f64);
    fn save_state(&mut self) -> Result<Vec<u8>, String>;
    fn load_state(&mut self, state: &[u8]) -> Result<(), String>;
    /// Puts the plugin's window into the X11 window `parent`; the size it wants.
    fn open_editor(&mut self, parent: u32) -> Result<(u32, u32), String>;
    fn close_editor(&mut self);
    /// Called about 60 times a second on the main thread while a window is open.
    fn idle(&mut self);
}

/// A plugin a bundle says it holds, before it is read. `id` is what its `PluginKey::id` will
/// be and what `--skip` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    pub name: Option<String>,
}

pub struct Loaded {
    pub plugin: LoadedPlugin,
    pub controller: Box<dyn Controller>,
    pub processor: Box<dyn Processor>,
}

/// Loads the plugin and makes it ready to run. On the main thread.
pub fn load(key: &PluginKey, bundle: &Path, activation: Activation) -> Result<Loaded, String> {
    if let Some(loaded) = fake::load(key, activation) {
        return loaded;
    }
    match key.format() {
        PluginFormat::Vst3 => vst3::load(key, bundle, activation),
        PluginFormat::Lv2 => lv2::load(key, bundle, activation),
    }
}
