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

/// What plugins asked the main thread to watch and to call. Only VST3 has such a thing.
pub use vst3::runloop;

use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering::SeqCst};
use std::time::{Duration, Instant};

use meloet_plugin::ipc::{FromPlugin, ToPlugin, Transport};
use meloet_plugin::wire::{LoadedPlugin, ParamInfo, ParamNow};
use meloet_plugin::{PluginFormat, PluginKey};

/// What a plugin is made ready for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Activation {
    pub sample_rate: f64,
    /// No block is longer than this.
    pub max_frames: usize,
}

/// Keeps the audio thread and the main thread out of a plugin at the same time, for the calls
/// a format does not allow together with `run` (LV2 state, VST3 `setActive`). The audio thread
/// never waits here: while the main thread is inside, its block goes without the plugin.
#[derive(Default)]
pub struct Gate(AtomicU8);

const GATE_FREE: u8 = 0;
const GATE_AUDIO: u8 = 1;
const GATE_MAIN: u8 = 2;

impl Gate {
    /// On the audio thread. False while the main thread is inside; then the plugin must not
    /// be touched this block. After true, [`Gate::leave`].
    pub fn enter(&self) -> bool {
        self.0
            .compare_exchange(GATE_FREE, GATE_AUDIO, SeqCst, SeqCst)
            .is_ok()
    }

    pub fn leave(&self) {
        self.0.store(GATE_FREE, SeqCst);
    }

    /// On the main thread: waits for the audio thread to come out of the plugin, for `within`
    /// at most (it may be stuck inside a plugin that hangs).
    pub fn claim(&self, within: Duration) -> Option<Claim<'_>> {
        let until = Instant::now() + within;
        loop {
            if self
                .0
                .compare_exchange(GATE_FREE, GATE_MAIN, SeqCst, SeqCst)
                .is_ok()
            {
                return Some(Claim(self));
            }
            if Instant::now() > until {
                return None;
            }
            std::thread::sleep(Duration::from_micros(100));
        }
    }
}

pub struct Claim<'a>(&'a Gate);

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        self.0.0.store(GATE_FREE, SeqCst);
    }
}

/// How long the main thread waits at a [`Gate`].
pub const GATE_WAIT: Duration = Duration::from_secs(2);

/// What a plugin did by itself, as its controller tells it on the main thread
/// ([`Controller::poll`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Said {
    ParamBegin(u32),
    /// Goes to meloet and to the plugin's own processor.
    ParamChanged(u32, f64),
    ParamEnd(u32),
    Latency(u32),
    /// Any parameter may have another value now.
    ParamsChanged,
    /// The plugin wants its window to have this size.
    Resized(u32, u32),
    /// The plugin closed its window by itself.
    Closed,
}

/// A plugin's window, just put into the X11 window it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opened {
    /// The size the plugin wants; `None` when it does not say (then the size of the window
    /// it made inside is taken).
    pub size: Option<(u32, u32)>,
    /// The person may give the window another size ([`Controller::resize_editor`]).
    pub resizable: bool,
}

/// What a typed text stands for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Read {
    /// 0..=1.
    pub value: f64,
    /// Whether the text named a value outside the range; `None` when the format cannot
    /// tell.
    pub clamped: Option<bool>,
}

/// What a processor tells meloet during a block. Holds a fixed number of events; more are
/// dropped.
pub struct EventSink {
    events: Vec<FromPlugin>,
}

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
pub trait Controller {
    /// Every parameter a person can move, with its value now.
    fn params(&mut self) -> Vec<ParamInfo>;
    /// The value now and the text the plugin shows for it; `None` for a parameter it does
    /// not have.
    fn param(&mut self, id: u32) -> Option<ParamNow>;
    /// The text the plugin shows for a value, 0..=1.
    fn text(&mut self, id: u32, value: f64) -> Option<String>;
    /// The value a typed text stands for; `None` when it cannot be read.
    fn read_text(&mut self, id: u32, text: &str) -> Option<Read>;
    fn set_param(&mut self, id: u32, value: f64);
    fn save_state(&mut self) -> Result<Vec<u8>, String>;
    fn load_state(&mut self, state: &[u8]) -> Result<(), String>;
    /// Puts the plugin's window into the X11 window `parent`.
    fn open_editor(&mut self, parent: u32) -> Result<Opened, String>;
    /// Takes the plugin's window out again. Before `parent` goes.
    fn close_editor(&mut self);
    /// The person gave the window this size; what the plugin makes of it.
    fn resize_editor(&mut self, width: u32, height: u32) -> (u32, u32) {
        (width, height)
    }
    /// Called about 60 times a second on the main thread while a window is open.
    fn idle(&mut self);
    /// Called on the main thread many times a second and after every request: the plugin's
    /// housekeeping outside its window, and what it did by itself since the last call.
    fn poll(&mut self, said: &mut Vec<Said>);
}

/// The number a text starts with ("-12.5 dB", "+3", "−inf dB").
pub fn leading_number(text: &str) -> Option<f64> {
    let text = text.trim_start();
    let (sign, rest) = match text.chars().next()? {
        '-' | '−' => (-1.0, &text[text.chars().next()?.len_utf8()..]),
        '+' => (1.0, &text[1..]),
        _ => (1.0, text),
    };
    let rest = rest.trim_start();
    if rest
        .get(..3)
        .is_some_and(|inf| inf.eq_ignore_ascii_case("inf"))
        || rest.starts_with('∞')
    {
        return Some(sign * f64::INFINITY);
    }
    let end = rest
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map_or(rest.len(), |(at, _)| at);
    rest[..end].parse::<f64>().ok().map(|number| sign * number)
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
