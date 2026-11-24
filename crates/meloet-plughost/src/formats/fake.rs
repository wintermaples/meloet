//! Plugins that are not plugins, for tests of everything around a plugin. They exist only
//! while the environment variable [`ENV`] is set.
//!
//! To load (`lv2:` keys; the bundle is not looked at):
//! - `urn:meloet:fake:sine`: an instrument. A sine at the key held; parameter 0 is its level.
//! - `urn:meloet:fake:pass`: an effect. The input, untouched.
//! - `urn:meloet:fake:gain`: an effect. The input, as loud as its parameter 0 says.
//! - `urn:meloet:fake:segv`: as `pass` until its first note, then it crashes inside `process`.
//! - `urn:meloet:fake:hang`: as `pass` until its first note, then `process` never returns.
//! - `urn:meloet:fake:window`: as `sine`, with a window meloet opens (an empty one) and two
//!   more parameters in a group, a list of three steps and a switch.
//! - `urn:meloet:fake:knob`: as `window`; someone in its window takes hold of the level as
//!   soon as it is open, turns it up to 90 % in forty moves and lets it go.
//! - `urn:meloet:fake:slowwindow`: as `window`; its window takes 0.4 s to open.
//! - `urn:meloet:fake:choices`: as `sine`, with no window and the two parameters of `window`
//!   and a third in their group: `Type`, of no steps, whose texts are four names. Its state
//!   holds the level, the step and the type, not the switch. Its presets are [`PRESETS`] and
//!   the preset files in the bundles it is given; a preset it saves is such a file.
//! - `urn:meloet:fake:notyet`: as `pass`, with a window of a kind meloet does not open.
//! - `urn:meloet:fake:slow`: as `pass`; every block takes it 30 ms, longer than the block is.
//! - `urn:meloet:fake:mode`: an effect. 1 in every sample when it was made ready for an
//!   export (`Activation::offline`), 0 when not.
//! - `urn:meloet:fake:live`: as `pass`; it says it cannot run faster than it plays.
//! - `urn:meloet:fake:drums`: as `sine`; it names two keys ([`DRUM_NAMES`]).
//! - `urn:meloet:fake:kit`: as `sine`; it names none.
//! - `urn:meloet:fake:slowkit`: as `sine`; it names no keys (an empty list) until
//!   [`KIT_READ`] after it was made, then the keys of `drums`, as DrumGizmo names them once
//!   its kit is read. It says so by itself ([`Said::KeyNames`]).
//! - `urn:meloet:fake:late`: as `sine`; a state put into it is taken in at the end of its next
//!   block, after that block's parameter changes, as Surge XT takes one while its blocks run.
//!   Before its first block it takes one at once.
//! - `urn:meloet:fake:outs`: an instrument with five audio outputs in four output sets
//!   ([`OUTS`]), and no parameters. While any key is held each output gives its own constant
//!   in every sample, else 0. By default the pair is left 0.34375, right 0.15625.
//!
//! To scan (by the bundle's file name without its ending):
//! - `found`: two plugins. `empty`: none. `broken`: a bundle that cannot be read.
//! - `choices`: the instrument `choices`.
//! - `segv`: one plugin; reading it crashes. `hang`: one plugin; reading it never ends.
//! - `mixed`: `urn:meloet:fake:a`, `…:b` and `…:c`. Reading `b` crashes; `a` and `c` are read.
//! - `drums`: `drums` and `kit`, both drum instruments.
//! - `slowkit`: the drum instrument `slowkit`.
//! - `outs`: the instrument `outs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{
    AtomicBool, AtomicU64, Ordering::Acquire, Ordering::Relaxed, Ordering::Release,
};

use meloet_plugin::ipc::ToPlugin;
use meloet_plugin::wire::{KeyName, LoadedPlugin, ParamInfo, ParamNow, ParamShape, PresetInfo};
use meloet_plugin::{
    EditorKind, EffectCategory, InstrumentCategory, PluginFormat, PluginInfo, PluginKey, PluginKind,
};

use super::outputs::{Outputs, Ports, SetShape, Side};
use super::{
    Activation, Candidate, Controller, Loaded, Opened, ProcessBlock, Processor, Read, Said,
    leading_number,
};

pub const ENV: &str = "MELOET_PLUGHOST_FAKE";
const PREFIX: &str = "urn:meloet:fake:";
const LEVEL: u32 = 0;
const DEFAULT_LEVEL: f64 = 0.5;
/// The two parameters only the fakes with a window have. They change no sound.
const MODE: u32 = 1;
const LOUD: u32 = 2;
const MODES: [&str; 3] = ["Soft", "Hard", "Odd"];
/// The parameter only `choices` has, and its names: each a quarter of the range.
const TYPE: u32 = 3;
const TYPES: [&str; 4] = ["Saw", "Square", "Sine", "Noise"];
const WINDOW_SIZE: (u32, u32) = (320, 200);
/// The moves of the person in the window of `knob`, two with every idle call.
const KNOB_MOVES: u32 = 40;
const KNOB_END: f64 = 0.9;
/// The keys `drums` names.
pub const DRUM_NAMES: [(u8, &str); 2] = [(36, "Fake Kick"), (38, "Fake Snare")];
/// How long `slowkit` takes to read its kit.
pub const KIT_READ: std::time::Duration = std::time::Duration::from_millis(300);
/// The presets of `choices`: URI, label, and its state (level, step, type).
pub const PRESETS: [(&str, &str, [f64; 3]); 2] = [
    ("urn:meloet:fake:preset:quiet", "Quiet", [0.125, 0.0, 0.0]),
    ("urn:meloet:fake:preset:loud", "Loud", [0.875, 1.0, 0.625]),
];
pub const PRESET_BANK: &str = "Fake Bank";
/// A preset of `choices` written into a bundle is a file of this ending holding its state;
/// the bundle's manifest says it applies to `choices`.
const PRESET_ENDING: &str = "fakepreset";
/// The output sets of `outs`, in port order: id, name, the constants of its outputs (of a
/// stereo set left, then right), heard by default, and its pan by default.
///
/// - `a`: outputs 0 and 1, 1/16 and 1/8; heard, left and right.
/// - `b`: output 2, 1/32; heard in the middle (on both sides).
/// - `c`: output 3, 1/4; heard on the left only.
/// - `d`: output 4, 1/2; not heard by default, in the middle when switched on.
pub const OUTS: [(&str, &str, &[f32], bool, f32); 4] = [
    ("a", "Fake Pair", &[0.0625, 0.125], true, 0.0),
    ("b", "Fake Middle", &[0.03125], true, 0.0),
    ("c", "Fake Left", &[0.25], true, -1.0),
    ("d", "Fake Spare", &[0.5], false, 0.0),
];
/// The outputs of `outs` heard by default and where, as [`OUTS`] says.
const OUTS_HEARD: [(usize, Side); 4] = [
    (0, Side::Left),
    (1, Side::Right),
    (2, Side::Both),
    (3, Side::Left),
];

fn enabled() -> bool {
    std::env::var_os(ENV).is_some_and(|v| !v.is_empty())
}

pub fn load(key: &PluginKey, activation: Activation) -> Option<Result<Loaded, String>> {
    if !enabled() || key.format() != PluginFormat::Lv2 {
        return None;
    }
    Some(make(key.id().strip_prefix(PREFIX)?, activation))
}

/// The fake plugin of that name, whatever the environment says.
pub fn make(name: &str, activation: Activation) -> Result<Loaded, String> {
    let level = Arc::new(AtomicU64::new(DEFAULT_LEVEL.to_bits()));
    let late = (name == "late").then(|| Arc::new(Late::default()));
    let mut outputs = Vec::new();
    let processor: Box<dyn Processor> = match name {
        "outs" => {
            let outs = Outs::new(activation);
            outputs = outs.outputs.list();
            Box::new(outs)
        }
        "sine" | "window" | "knob" | "slowwindow" | "drums" | "kit" | "slowkit" | "choices"
        | "late" => Box::new(Sine {
            sample_rate: activation.sample_rate,
            level: level.clone(),
            key: None,
            phase: 0.0,
            late: late.clone(),
        }),
        "pass" | "notyet" | "live" => Box::new(Pass { fault: None }),
        "slow" => Box::new(Pass {
            fault: Some(Fault::Slow),
        }),
        "mode" => Box::new(Mode(activation.offline)),
        "gain" => Box::new(Gain {
            level: level.clone(),
        }),
        "segv" => Box::new(Pass {
            fault: Some(Fault::Crash),
        }),
        "hang" => Box::new(Pass {
            fault: Some(Fault::Hang),
        }),
        _ => return Err(format!("no fake plugin {name}")),
    };
    let window = matches!(name, "window" | "knob" | "slowwindow");
    let choices = name == "choices";
    let instrument =
        matches!(name, "sine" | "drums" | "kit" | "slowkit" | "late" | "outs") || window || choices;
    let editor = match name {
        _ if window => EditorKind::X11,
        "notyet" => EditorKind::Unsupported,
        _ => EditorKind::None,
    };
    Ok(Loaded {
        plugin: LoadedPlugin {
            latency: 0,
            audio_inputs: if instrument { 0 } else { 2 },
            audio_outputs: 2,
            editor,
            live: name == "live",
            outputs,
            key_names: None,
        },
        controller: Box::new(FakeController {
            level,
            has_level: (instrument && name != "outs") || name == "gain",
            window,
            choices,
            mode: 0.0,
            loud: 0.0,
            kind: 0.0,
            slow: name == "slowwindow",
            knob: (name == "knob").then_some(0),
            open: false,
            said: Vec::new(),
            names: match name {
                "drums" => Names::Drums,
                "slowkit" => Names::Later(std::time::Instant::now() + KIT_READ),
                _ => Names::None,
            },
            late,
        }),
        processor,
    })
}

/// Never returns: nothing may come back from a plugin that crashed or hung.
fn fail(fault: Fault) -> ! {
    match fault {
        Fault::Crash => {
            // SAFETY: none; the write to address 0 is the point. The process is first made
            // undumpable so that the test leaves no core file or crash report behind.
            unsafe {
                libc::prctl(libc::PR_SET_DUMPABLE, 0);
                std::ptr::null_mut::<u32>().write_volatile(1);
            }
            std::process::abort()
        }
        Fault::Hang | Fault::Slow => loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        },
    }
}

#[derive(Clone, Copy)]
enum Fault {
    Crash,
    Hang,
    Slow,
}

/// How long `slow` takes for a block.
const SLOW: std::time::Duration = std::time::Duration::from_millis(30);

struct Mode(bool);

impl Processor for Mode {
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        for output in &mut block.outputs {
            output.fill(if self.0 { 1.0 } else { 0.0 });
        }
    }

    fn reset(&mut self) {}
}

struct Pass {
    fault: Option<Fault>,
}

impl Processor for Pass {
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let note =
            |e: &ToPlugin| matches!(e, ToPlugin::Midi { data, .. } if data[0] & 0xf0 == 0x90);
        match self.fault {
            Some(Fault::Slow) => std::thread::sleep(SLOW),
            Some(fault) if block.events.iter().any(note) => fail(fault),
            _ => {}
        }
        for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
            output.copy_from_slice(input);
        }
    }

    fn reset(&mut self) {}
}

/// An effect with one parameter: how loud what goes through it comes out.
struct Gain {
    level: Arc<AtomicU64>,
}

impl Processor for Gain {
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        for event in block.events {
            if let ToPlugin::Param {
                id: LEVEL, value, ..
            } = *event
            {
                self.level.store(value.to_bits(), Relaxed);
            }
        }
        let level = f64::from_bits(self.level.load(Relaxed)) as f32;
        for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
            for (out, sample) in output.iter_mut().zip(input.iter()) {
                *out = sample * level;
            }
        }
    }

    fn reset(&mut self) {}
}

struct Sine {
    sample_rate: f64,
    level: Arc<AtomicU64>,
    key: Option<u8>,
    phase: f64,
    late: Option<Arc<Late>>,
}

/// The state `late` was given and has not taken in yet.
#[derive(Default)]
struct Late {
    ran: AtomicBool,
    waiting: AtomicBool,
    level: AtomicU64,
}

/// The key held after a MIDI message, when `key` was held before it.
fn held(key: Option<u8>, data: [u8; 3]) -> Option<u8> {
    match (data[0] & 0xf0, data[1], data[2]) {
        (0x90, key, velocity) if velocity > 0 => Some(key),
        (0x90 | 0x80, off, _) if key == Some(off) => None,
        // All sound off, all notes off.
        (0xb0, 120 | 123, _) => None,
        _ => key,
    }
}

impl Sine {
    fn take(&mut self, event: &ToPlugin) {
        match *event {
            ToPlugin::Midi { data, .. } => self.key = held(self.key, data),
            ToPlugin::Param {
                id: LEVEL, value, ..
            } => self.level.store(value.to_bits(), Relaxed),
            ToPlugin::Param { .. } => {}
        }
    }
}

impl Processor for Sine {
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let mut events = block.events.iter().peekable();
        for frame in 0..block.outputs[0].len() {
            while let Some(event) = events.next_if(|e| e.frame() as usize <= frame) {
                self.take(event);
            }
            let sample = match self.key {
                Some(key) => {
                    let hz = 440.0 * 2f64.powf((f64::from(key) - 69.0) / 12.0);
                    self.phase = (self.phase + hz / self.sample_rate).fract();
                    let level = f64::from_bits(self.level.load(Relaxed));
                    ((self.phase * std::f64::consts::TAU).sin() * level) as f32
                }
                None => 0.0,
            };
            block.outputs[0][frame] = sample;
            block.outputs[1][frame] = sample;
        }
        if let Some(late) = &self.late {
            late.ran.store(true, Relaxed);
            if late.waiting.swap(false, Acquire) {
                self.level.store(late.level.load(Relaxed), Relaxed);
            }
        }
    }

    fn reset(&mut self) {
        self.key = None;
        self.phase = 0.0;
    }
}

/// `outs`: each output its constant of [`OUTS`] while a key is held.
struct Outs {
    /// By output.
    levels: Vec<f32>,
    key: Option<u8>,
    max_frames: usize,
    /// Every output, `max_frames` each.
    out: Box<[f32]>,
    outputs: Outputs,
}

/// The constant of each output of `outs`, and its sets.
fn outs_sets() -> (Vec<f32>, Vec<SetShape>) {
    let mut levels = Vec::new();
    let mut sets = Vec::new();
    for (id, name, constants, on, pan) in OUTS {
        let at = levels.len();
        levels.extend_from_slice(constants);
        sets.push(SetShape {
            id: id.into(),
            name: name.into(),
            ports: match constants.len() {
                1 => Ports::Mono(at),
                _ => Ports::Stereo {
                    left: at,
                    right: at + 1,
                },
            },
            on,
            pan,
        });
    }
    (levels, sets)
}

impl Outs {
    fn new(activation: Activation) -> Outs {
        let (levels, sets) = outs_sets();
        let outputs = Outputs::new(sets, OUTS_HEARD.to_vec(), true, activation.sample_rate);
        Outs {
            out: vec![0.0; levels.len() * activation.max_frames].into_boxed_slice(),
            levels,
            key: None,
            max_frames: activation.max_frames,
            outputs,
        }
    }
}

impl Processor for Outs {
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.outputs[0].len();
        if frames > self.max_frames {
            for output in &mut block.outputs {
                output.fill(0.0);
            }
            self.outputs.missed(frames);
            return;
        }
        let mut events = block.events.iter().peekable();
        for frame in 0..frames {
            while let Some(event) = events.next_if(|e| e.frame() as usize <= frame) {
                if let ToPlugin::Midi { data, .. } = *event {
                    self.key = held(self.key, data);
                }
            }
            for (output, &level) in self.levels.iter().enumerate() {
                let sample = if self.key.is_some() { level } else { 0.0 };
                self.out[output * self.max_frames + frame] = sample;
            }
        }
        let [left, right] = &mut block.outputs;
        self.outputs.mix(&self.out, self.max_frames, left, right);
    }

    fn reset(&mut self) {
        self.key = None;
        self.outputs.snap();
    }

    fn outputs(&mut self) -> Option<&mut Outputs> {
        Some(&mut self.outputs)
    }
}

struct FakeController {
    /// Shared with the processor, which stands in for a real plugin's own inner link.
    level: Arc<AtomicU64>,
    has_level: bool,
    /// It has a window, and the parameters `MODE` and `LOUD`.
    window: bool,
    /// It has `MODE`, `LOUD` and `TYPE`, and a state that holds the first and the last.
    choices: bool,
    mode: f64,
    loud: f64,
    kind: f64,
    slow: bool,
    /// How many idle calls the person in the window of `knob` has been at it.
    knob: Option<u32>,
    open: bool,
    said: Vec<Said>,
    names: Names,
    late: Option<Arc<Late>>,
}

/// The keys a fake names.
#[derive(Clone, Copy, PartialEq)]
enum Names {
    None,
    Drums,
    /// None until then, the keys of `drums` after.
    Later(std::time::Instant),
}

impl FakeController {
    fn level(&self) -> f64 {
        f64::from_bits(self.level.load(Relaxed))
    }

    fn value(&self, id: u32) -> Option<f64> {
        match id {
            LEVEL if self.has_level => Some(self.level()),
            MODE if self.window || self.choices => Some(self.mode),
            LOUD if self.window || self.choices => Some(self.loud),
            TYPE if self.choices => Some(self.kind),
            _ => None,
        }
    }

    fn step(value: f64) -> usize {
        (value.clamp(0.0, 1.0) * (MODES.len() - 1) as f64).round() as usize
    }

    fn now(&self, id: u32) -> Option<ParamNow> {
        let value = self.value(id)?;
        Some(ParamNow {
            id,
            value: Some(value),
            text: Self::text_of(id, value),
        })
    }

    fn text_of(id: u32, value: f64) -> Option<String> {
        Some(match id {
            LEVEL => format!("{:.0} %", value * 100.0),
            MODE => MODES[Self::step(value)].to_string(),
            LOUD => if value >= 0.5 { "On" } else { "Off" }.to_string(),
            TYPE => TYPES[Self::kind_of(value)].to_string(),
            _ => return None,
        })
    }

    fn kind_of(value: f64) -> usize {
        ((value.clamp(0.0, 1.0) * TYPES.len() as f64) as usize).min(TYPES.len() - 1)
    }
}

impl Controller for FakeController {
    fn params(&mut self) -> Vec<ParamInfo> {
        let param = |id, name: &str, group: Option<&str>, default, shape, labels: &[&str]| {
            let now = self.now(id);
            ParamInfo {
                id,
                name: name.into(),
                group: group.map(str::to_owned),
                default,
                shape,
                labels: labels.iter().map(|label| label.to_string()).collect(),
                automatable: true,
                midi_cc: false,
                value: now.as_ref().and_then(|now| now.value),
                text: now.and_then(|now| now.text),
            }
        };
        let mut params = Vec::new();
        if self.has_level {
            let shape = ParamShape::Continuous;
            params.push(param(LEVEL, "Level", None, DEFAULT_LEVEL, shape, &[]));
        }
        if self.window || self.choices {
            let steps = ParamShape::Stepped {
                steps: MODES.len() as u32,
            };
            params.push(param(MODE, "Mode", Some("Shape"), 0.0, steps, &MODES));
            let toggle = ParamShape::Toggle;
            params.push(param(LOUD, "Loud", Some("Shape"), 0.0, toggle, &[]));
        }
        if self.choices {
            let shape = ParamShape::Continuous;
            params.push(param(TYPE, "Type", Some("Shape"), 0.0, shape, &[]));
        }
        params
    }

    fn param(&mut self, id: u32) -> Option<ParamNow> {
        self.now(id)
    }

    fn text(&mut self, id: u32, value: f64) -> Option<String> {
        self.value(id).and_then(|_| Self::text_of(id, value))
    }

    fn read_text(&mut self, id: u32, text: &str) -> Option<Read> {
        self.value(id)?;
        let text = text.trim();
        let exact = |value| Read {
            value,
            clamped: Some(false),
        };
        match id {
            LEVEL => {
                let percent = leading_number(text).filter(|n| !n.is_nan())?;
                Some(Read {
                    value: (percent / 100.0).clamp(0.0, 1.0),
                    clamped: Some(!(0.0..=100.0).contains(&percent)),
                })
            }
            MODE => (MODES.iter())
                .position(|mode| mode.eq_ignore_ascii_case(text))
                .map(|step| exact(step as f64 / (MODES.len() - 1) as f64)),
            TYPE => (TYPES.iter())
                .position(|kind| kind.eq_ignore_ascii_case(text))
                .map(|at| exact((at as f64 + 0.5) / TYPES.len() as f64)),
            _ if text.eq_ignore_ascii_case("on") => Some(exact(1.0)),
            _ if text.eq_ignore_ascii_case("off") => Some(exact(0.0)),
            _ => None,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if self.value(id).is_none() || !value.is_finite() {
            return;
        }
        let value = value.clamp(0.0, 1.0);
        match id {
            LEVEL => self.level.store(value.to_bits(), Relaxed),
            MODE => self.mode = Self::step(value) as f64 / (MODES.len() - 1) as f64,
            TYPE => self.kind = value,
            _ => self.loud = value.round(),
        }
    }

    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        let mut state = self.level().to_le_bytes().to_vec();
        if self.choices {
            state.extend(self.mode.to_le_bytes());
            state.extend(self.kind.to_le_bytes());
        }
        Ok(state)
    }

    fn load_state(&mut self, state: &[u8]) -> Result<(), String> {
        let bad = || "not a fake plugin's state".to_string();
        let values: Vec<f64> = (state.chunks(8))
            .map(|bytes| bytes.try_into().map(f64::from_le_bytes))
            .collect::<Result<_, _>>()
            .map_err(|_| bad())?;
        let level = match (self.choices, &values[..]) {
            (false, [level]) => *level,
            (true, [level, mode, kind]) => {
                (self.mode, self.kind) = (mode.clamp(0.0, 1.0), kind.clamp(0.0, 1.0));
                *level
            }
            _ => return Err(bad()),
        };
        if !(0.0..=1.0).contains(&level) {
            return Err(bad());
        }
        match &self.late {
            Some(late) if late.ran.load(Relaxed) => {
                late.level.store(level.to_bits(), Relaxed);
                late.waiting.store(true, Release);
            }
            _ => self.level.store(level.to_bits(), Relaxed),
        }
        Ok(())
    }

    fn key_names(&mut self) -> Option<Vec<KeyName>> {
        let name = |(key, name): (u8, &str)| KeyName {
            key,
            name: name.into(),
        };
        match self.names {
            Names::None => None,
            Names::Drums => Some(DRUM_NAMES.map(name).to_vec()),
            Names::Later(_) => Some(Vec::new()),
        }
    }

    fn presets(&mut self, bundles: &[PathBuf]) -> Result<Vec<PresetInfo>, String> {
        if !self.choices {
            return Err("this fake plugin has no presets".into());
        }
        let mut presets: Vec<PresetInfo> = (PRESETS.iter())
            .map(|(uri, label, _)| PresetInfo {
                uri: (*uri).into(),
                label: (*label).into(),
                bank: Some(PRESET_BANK.into()),
                file: None,
            })
            .collect();
        for bundle in bundles {
            let Ok(entries) = std::fs::read_dir(bundle) else {
                continue;
            };
            let mut files: Vec<PathBuf> = (entries.flatten().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == PRESET_ENDING))
                .collect();
            files.sort();
            presets.extend(files.into_iter().map(|file| PresetInfo {
                uri: format!("file://{}", file.display()),
                label: (file.file_stem().unwrap_or_default().to_string_lossy()).into(),
                bank: None,
                file: Some(file),
            }));
        }
        Ok(presets)
    }

    fn preset_state(&mut self, uri: &str, bundles: &[PathBuf]) -> Result<Vec<u8>, String> {
        let none = || format!("no preset {uri} applies to this plugin");
        let presets = self.presets(bundles)?;
        let preset = (presets.iter().find(|p| p.uri == uri)).ok_or_else(none)?;
        if let Some(file) = &preset.file {
            return std::fs::read(file).map_err(|e| e.to_string());
        }
        let (_, _, values) = (PRESETS.iter().find(|(u, ..)| *u == uri)).ok_or_else(none)?;
        Ok(values.iter().flat_map(|v| v.to_le_bytes()).collect())
    }

    fn save_preset(&mut self, dir: &Path, file: &str, label: &str) -> Result<PresetInfo, String> {
        if !self.choices {
            return Err("this fake plugin has no presets".into());
        }
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir
            .join(file)
            .with_file_name(format!("{label}.{PRESET_ENDING}"));
        std::fs::write(&path, self.save_state()?).map_err(|e| e.to_string())?;
        let uri = format!("file://{}", path.display());
        let manifest = format!(
            "<{uri}> a <http://lv2plug.in/ns/ext/presets#Preset> ;\n    \
             <http://lv2plug.in/ns/lv2core#appliesTo> <{PREFIX}choices> .\n"
        );
        std::fs::write(dir.join("manifest.ttl"), manifest).map_err(|e| e.to_string())?;
        Ok(PresetInfo {
            uri,
            label: label.into(),
            bank: None,
            file: Some(path),
        })
    }

    fn open_editor(&mut self, _parent: u32) -> Result<Opened, String> {
        if !self.window {
            return Err("this fake plugin has no window".into());
        }
        if self.slow {
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        self.open = true;
        if self.knob.is_some() {
            self.knob = Some(0);
        }
        Ok(Opened {
            size: Some(WINDOW_SIZE),
            resizable: false,
        })
    }

    fn close_editor(&mut self) {
        self.open = false;
    }

    fn idle(&mut self) {
        let (true, Some(turn)) = (self.open, self.knob) else {
            return;
        };
        let moves = KNOB_MOVES / 2;
        match turn {
            0 => self.said.push(Said::ParamBegin(LEVEL, super::now_us())),
            turn if turn <= moves => {
                for half in [1, 0] {
                    let done = f64::from(turn * 2 - half) / f64::from(KNOB_MOVES);
                    let level = DEFAULT_LEVEL + (KNOB_END - DEFAULT_LEVEL) * done;
                    self.level.store(level.to_bits(), Relaxed);
                    self.said
                        .push(Said::ParamChanged(LEVEL, level, super::now_us()));
                }
            }
            turn if turn == moves + 1 => self.said.push(Said::ParamEnd(LEVEL, super::now_us())),
            _ => return,
        }
        self.knob = Some(turn + 1);
    }

    fn poll(&mut self, said: &mut Vec<Said>) {
        said.append(&mut self.said);
        if let Names::Later(at) = self.names
            && std::time::Instant::now() >= at
        {
            self.names = Names::Drums;
            said.push(Said::KeyNames);
        }
    }
}

fn info(name: &str, kind: PluginKind) -> PluginInfo {
    PluginInfo {
        key: PluginKey::lv2(&format!("{PREFIX}{}", name.to_lowercase())).expect("a fixed key"),
        name: format!("Fake {name}"),
        vendor: "meloet".into(),
        version: "1".into(),
        kind,
        editor: EditorKind::None,
        audio_inputs: if kind.role() == meloet_plugin::PluginRole::Instrument {
            0
        } else {
            2
        },
        audio_outputs: 2,
    }
}

/// What a fake bundle holds: each plugin's name (its id is [`PREFIX`] and the name in lower
/// case), what it is, and what goes wrong when it is read.
fn contents(stem: &str) -> Option<Vec<(&'static str, PluginKind, Option<Fault>)>> {
    let synth = PluginKind::Instrument(InstrumentCategory::Synth);
    let effect = PluginKind::Effect(EffectCategory::Other);
    let drum = PluginKind::Instrument(InstrumentCategory::Drum);
    Some(match stem {
        "found" => vec![("Sine", synth, None), ("Pass", effect, None)],
        "drums" => vec![("Drums", drum, None), ("Kit", drum, None)],
        "slowkit" => vec![("SlowKit", drum, None)],
        "choices" => vec![("Choices", synth, None)],
        "outs" => vec![("Outs", synth, None)],
        "empty" => vec![],
        "segv" => vec![("Segv", effect, Some(Fault::Crash))],
        "hang" => vec![("Hang", effect, Some(Fault::Hang))],
        "mixed" => vec![
            ("A", effect, None),
            ("B", effect, Some(Fault::Crash)),
            ("C", synth, None),
        ],
        _ => return None,
    })
}

fn stem(bundle: &Path) -> Option<&str> {
    bundle.file_stem()?.to_str()
}

/// `None` when this is not a fake bundle.
pub fn list(bundle: &Path) -> Option<Result<Vec<Candidate>, String>> {
    if !enabled() {
        return None;
    }
    let stem = stem(bundle)?;
    if stem == "broken" {
        return Some(Err("a fake broken bundle".into()));
    }
    let candidates = contents(stem)?
        .into_iter()
        .map(|(name, _, _)| Candidate {
            id: format!("{PREFIX}{}", name.to_lowercase()),
            name: Some(format!("Fake {name}")),
        })
        .collect();
    Some(Ok(candidates))
}

/// `None` when this is not a fake bundle.
pub fn probe(bundle: &Path, id: &str) -> Option<Result<PluginInfo, String>> {
    if !enabled() {
        return None;
    }
    let stem = stem(bundle)?;
    if stem == "broken" {
        return Some(Err("a fake broken bundle".into()));
    }
    let found = contents(stem)?
        .into_iter()
        .find(|(name, _, _)| format!("{PREFIX}{}", name.to_lowercase()) == id);
    Some(match found {
        Some((_, _, Some(fault))) => fail(fault),
        Some((name, kind, None)) => Ok(info(name, kind)),
        None => Err(format!("no fake plugin {id} in {stem}")),
    })
}

#[cfg(test)]
mod tests {
    use meloet_plugin::ipc::Transport;

    use super::*;
    use crate::formats::EventSink;
    use crate::formats::outputs::heard_by_default;

    #[test]
    fn outs_is_heard_by_default_as_its_doc_says() {
        let (levels, sets) = outs_sets();
        assert_eq!(heard_by_default(&sets), OUTS_HEARD);
        assert_eq!(levels.len(), 5);
        let activation = Activation {
            sample_rate: 48_000.0,
            max_frames: 64,
            offline: false,
        };
        let mut loaded = make("outs", activation).unwrap();
        let ids: Vec<&str> = loaded
            .plugin
            .outputs
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "b", "c", "d"]);
        assert!(loaded.controller.params().is_empty());
        let silence = [0.0f32; 64];
        let mut sink = EventSink::with_capacity(4);
        let mut pair = |events: &[ToPlugin]| {
            let (mut left, mut right) = ([9.0f32; 64], [9.0f32; 64]);
            loaded.processor.process(&mut ProcessBlock {
                inputs: [&silence, &silence],
                outputs: [&mut left, &mut right],
                events,
                transport: &Transport::default(),
                sink: &mut sink,
            });
            (left[63], right[63])
        };
        assert_eq!(pair(&[]), (0.0, 0.0));
        let on = ToPlugin::Midi {
            frame: 0,
            data: [0x90, 60, 100],
        };
        assert_eq!(pair(&[on]), (0.34375, 0.15625));
        let off = ToPlugin::Midi {
            frame: 0,
            data: [0x80, 60, 0],
        };
        assert_eq!(pair(&[off]), (0.0, 0.0));
    }
}
