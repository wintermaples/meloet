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
    /// For an export: the blocks come as fast as the plugin answers them, not as time
    /// passes (VST3 `kOffline`, LV2 `lv2:freeWheeling`).
    pub offline: bool,
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
    /// The last number of these three: when the plugin said it ([`now_us`]).
    ParamBegin(u32, u64),
    /// Goes to meloet and to the plugin's own processor.
    ParamChanged(u32, f64, u64),
    ParamEnd(u32, u64),
    /// The plugin loaded a program or a preset by itself (VST3 `kParamValuesChanged`,
    /// `kParamTitlesChanged`): a change the person made in its window.
    Preset,
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
    /// The person left the plugin's window: what they changed there may not all have been
    /// told.
    fn editor_left(&mut self) {}
    /// Called about 60 times a second on the main thread while a window is open.
    fn idle(&mut self);
    /// Called on the main thread many times a second and after every request: the plugin's
    /// housekeeping outside its window, and what it did by itself since the last call.
    fn poll(&mut self, said: &mut Vec<Said>);
}

/// CLOCK_MONOTONIC in microseconds: the clock of meloet's engine, so meloet can place what
/// the person did in a window in the song (docs/architecture.md 21-9).
pub fn now_us() -> u64 {
    meloet_plugin::ipc::futex::monotonic_ns() / 1000
}

/// A text as a person types it, with the digits, signs and point of the wide Japanese forms
/// ("３０", "－１２．５") made the plain ones plugins read.
pub fn half_width(text: &str) -> String {
    (text.chars())
        .map(|c| match c {
            '０'..='９' => char::from_digit(c as u32 - '０' as u32, 10).unwrap_or(c),
            '−' | '－' => '-',
            '＋' => '+',
            '．' => '.',
            c => c,
        })
        .collect()
}

/// A name as JUCE gives the parameters it makes only to take a keyboard's knobs:
/// "MIDI CC 0|0" to "MIDI CC 15|129" (channel, then controller).
pub fn named_midi_cc(name: &str) -> bool {
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    (name.trim().strip_prefix("MIDI CC "))
        .and_then(|rest| rest.split_once('|'))
        .is_some_and(|(channel, number)| digits(channel) && digits(number))
}

/// The number a text starts with.
pub fn leading_number(text: &str) -> Option<f64> {
    number_and_rest(text).map(|(number, _)| number)
}

/// The number a text starts with ("-12.5 dB", "+3", "−inf dB") and what follows it.
pub fn number_and_rest(text: &str) -> Option<(f64, &str)> {
    let text = text.trim_start();
    let (sign, rest) = match text.chars().next()? {
        '-' | '−' => (-1.0, &text[text.chars().next()?.len_utf8()..]),
        '+' => (1.0, &text[1..]),
        _ => (1.0, text),
    };
    let rest = rest.trim_start();
    let endless = match rest.get(..3) {
        Some(inf) if inf.eq_ignore_ascii_case("inf") => Some(&rest[3..]),
        _ => rest.strip_prefix('∞'),
    };
    if let Some(after) = endless {
        return Some((sign * f64::INFINITY, after));
    }
    let end = rest
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map_or(rest.len(), |(at, _)| at);
    let number = rest[..end].parse::<f64>().ok()?;
    Some((sign * number, &rest[end..]))
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

/// Whether a typed text names a value past the end of the range its value was put at, as far
/// as the numbers in the texts tell. For the formats that cannot say it themselves.
pub fn past_the_end(typed: &str, value: f64, low: Option<&str>, high: Option<&str>) -> bool {
    let number = |text: Option<&str>| text.and_then(leading_number);
    let (Some(typed), Some(low), Some(high)) = (leading_number(typed), number(low), number(high))
    else {
        return false;
    };
    // One end may be without bound ("-inf dB"); the other is still an end.
    let span = high - low;
    if span.is_nan() || span == 0.0 {
        return false;
    }
    let slack = if span.is_finite() {
        span.abs() * 1e-6
    } else {
        1e-9
    };
    if value >= 1.0 {
        (typed - high) * span.signum() > slack
    } else if value <= 0.0 {
        (low - typed) * span.signum() > slack
    } else {
        false
    }
}

/// What a typed text stands for, where the plugin's own reading cannot be relied on: JUCE's
/// VST3 plugins answer every text, with an end of the range (not always the near one) for
/// what they could not read or what lies outside. `answer` is the plugin's reading, `low`
/// and `high` the texts it shows at the ends, `shown` the text it shows for a value.
pub fn judge_typed(
    typed: &str,
    answer: Option<f64>,
    low: Option<&str>,
    high: Option<&str>,
    shown: impl FnOnce(f64) -> Option<String>,
) -> Option<Read> {
    let typed = typed.trim();
    let answer = answer.filter(|answer| answer.is_finite());
    let Some((number, unit)) = number_and_rest(typed) else {
        // Without a number, only a text the plugin itself shows for what it read.
        let value = answer?.clamp(0.0, 1.0);
        let same = shown(value).is_some_and(|text| text.trim().eq_ignore_ascii_case(typed));
        return same.then_some(Read {
            value,
            clamped: Some(false),
        });
    };
    // A number in the unit of the ends is past an end whatever the plugin made of it.
    if let (Some((low_number, low_unit)), Some((high_number, high_unit))) = (
        low.and_then(number_and_rest),
        high.and_then(number_and_rest),
    ) {
        let unit = unit.trim();
        let known = unit.is_empty()
            || [low_unit, high_unit]
                .iter()
                .any(|end| end.trim().eq_ignore_ascii_case(unit));
        let span = high_number - low_number;
        if known && !span.is_nan() && span != 0.0 {
            let slack = if span.is_finite() {
                span.abs() * 1e-6
            } else {
                1e-9
            };
            let past = |value| Read {
                value,
                clamped: Some(true),
            };
            if (number - high_number) * span.signum() > slack {
                return Some(past(1.0));
            }
            if (low_number - number) * span.signum() > slack {
                return Some(past(0.0));
            }
        }
    }
    let answer = answer?;
    let value = answer.clamp(0.0, 1.0);
    let outside = !(-1e-9..=1.0 + 1e-9).contains(&answer);
    Some(Read {
        value,
        clamped: Some(outside || past_the_end(typed, value, low, high)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_digits_are_made_plain() {
        assert_eq!(half_width("３０"), "30");
        assert_eq!(half_width("－１２．５ dB"), "-12.5 dB");
        assert_eq!(half_width("−３"), "-3");
        assert_eq!(half_width("＋７"), "+7");
        assert_eq!(half_width("Hall"), "Hall");
        assert_eq!(leading_number(&half_width("３０")), Some(30.0));
    }

    /// A plugin as Surge XT's VST3 is: it answers every text.
    #[test]
    fn typed_texts_a_plugin_answers_whatever_they_are() {
        let ends = (Some("-48.00 dB"), Some("0.00 dB"));
        let shown = |value: f64| Some(format!("{:.2} dB", -48.0 + 48.0 * value));
        let judge =
            |typed: &str, answer: f64| judge_typed(typed, Some(answer), ends.0, ends.1, shown);
        let read = |value, clamped| {
            Some(Read {
                value,
                clamped: Some(clamped),
            })
        };
        // Above the range, which the plugin put at the low end.
        assert_eq!(judge("3 dB", 0.0), read(1.0, true));
        assert_eq!(judge("99999999", 0.0), read(1.0, true));
        assert_eq!(judge("3db", 0.0), read(1.0, true));
        assert_eq!(judge("-99999999", 0.0), read(0.0, true));
        // Inside the range the plugin is believed.
        assert_eq!(judge("-6", 0.875), read(0.875, false));
        assert_eq!(judge("-6 dB", 0.875), read(0.875, false));
        assert_eq!(judge("0", 1.0), read(1.0, false));
        assert_eq!(judge("-48", 0.0), read(0.0, false));
        // No number: unreadable, whatever the plugin answered.
        assert_eq!(judge("???", 1.0), None);
        assert_eq!(judge("abc", 0.3), None);
        assert_eq!(judge("", 1.0), None);
        // A unit of another kind is the plugin's to read.
        assert_eq!(judge("2 kHz", 0.5), read(0.5, false));
        // The plugin could not read a number inside the range.
        assert_eq!(judge_typed("-6", None, ends.0, ends.1, shown), None);
        // The plugin could not read a number past the end: it is past the end all the same.
        assert_eq!(
            judge_typed("7", None, ends.0, ends.1, shown),
            read(1.0, true)
        );
    }

    #[test]
    fn typed_names_are_what_the_plugin_shows() {
        let shown = |value: f64| Some(if value >= 0.5 { "Hall" } else { "Room" }.to_string());
        let judge = |typed: &str, answer: f64| {
            judge_typed(typed, Some(answer), Some("Room"), Some("Hall"), shown)
        };
        assert_eq!(judge("hall", 1.0).map(|read| read.value), Some(1.0));
        assert_eq!(judge("Plate", 0.0), None);
        // A number where the ends show none: the plugin's answer, as it gave it.
        assert_eq!(judge("1", 1.0).map(|read| read.value), Some(1.0));
        assert_eq!(judge("5", 1.5).and_then(|read| read.clamped), Some(true));
        // An end without a bound.
        let ends = (Some("-inf dB"), Some("6.00 dB"));
        let shown = |_| Some("-inf dB".to_string());
        assert_eq!(
            judge_typed("-inf", Some(0.0), ends.0, ends.1, shown).map(|read| read.value),
            Some(0.0)
        );
        assert_eq!(
            judge_typed("12", Some(0.0), ends.0, ends.1, shown),
            Some(Read {
                value: 1.0,
                clamped: Some(true)
            })
        );
    }

    #[test]
    fn names_of_parameters_that_only_take_a_keyboards_knobs() {
        for name in ["MIDI CC 0|0", "MIDI CC 15|129", " MIDI CC 3|74 "] {
            assert!(named_midi_cc(name), "{name}");
        }
        for name in [
            "MIDI CC",
            "MIDI CC 3",
            "MIDI CC 3|",
            "MIDI CC |4",
            "MIDI CC a|4",
            "MIDI CC 3|4 Depth",
            "Midi CC 3|4",
            "CC 3|4",
            "Global Volume",
        ] {
            assert!(!named_midi_cc(name), "{name}");
        }
    }

    #[test]
    fn numbers_at_the_start_of_texts() {
        assert_eq!(leading_number("523.25 Hz"), Some(523.25));
        assert_eq!(leading_number(" -12.5 dB"), Some(-12.5));
        assert_eq!(leading_number("−3 st"), Some(-3.0));
        assert_eq!(leading_number("+7"), Some(7.0));
        assert_eq!(leading_number("-inf dB"), Some(f64::NEG_INFINITY));
        assert_eq!(leading_number("Hall"), None);
        assert_eq!(leading_number(""), None);
    }

    #[test]
    fn a_typed_value_past_the_end_is_seen_from_the_texts() {
        let ends = (Some("0.00 %"), Some("100.00 %"));
        assert!(past_the_end("150", 1.0, ends.0, ends.1));
        assert!(past_the_end("-5 %", 0.0, ends.0, ends.1));
        assert!(!past_the_end("100", 1.0, ends.0, ends.1));
        assert!(!past_the_end("0", 0.0, ends.0, ends.1));
        assert!(!past_the_end("50", 0.5, ends.0, ends.1));
        // A range that runs downward.
        assert!(!past_the_end("30", 1.0, Some("20 dB"), Some("-60 dB")));
        assert!(past_the_end("-70", 1.0, Some("20 dB"), Some("-60 dB")));
        assert!(past_the_end("30", 0.0, Some("20 dB"), Some("-60 dB")));
        // Texts without numbers say nothing.
        assert!(!past_the_end("Hall", 1.0, Some("Room"), Some("Hall")));
        assert!(!past_the_end("-80", 0.0, Some("-inf dB"), Some("0 dB")));
        assert!(past_the_end("30", 1.0, Some("-inf dB"), Some("20.00 dB")));
        assert!(!past_the_end("20", 1.0, Some("-inf dB"), Some("20.00 dB")));
    }
}
