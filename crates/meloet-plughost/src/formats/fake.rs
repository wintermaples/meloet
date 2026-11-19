//! Plugins that are not plugins, for tests of everything around a plugin. They exist only
//! while the environment variable [`ENV`] is set.
//!
//! To load (`lv2:` keys; the bundle is not looked at):
//! - `urn:meloet:fake:sine`: an instrument. A sine at the key held; parameter 0 is its level.
//! - `urn:meloet:fake:pass`: an effect. The input, untouched.
//! - `urn:meloet:fake:segv`: as `pass` until its first note, then it crashes inside `process`.
//! - `urn:meloet:fake:hang`: as `pass` until its first note, then `process` never returns.
//!
//! To scan (by the bundle's file name without its ending):
//! - `found`: two plugins. `empty`: none. `broken`: a bundle that cannot be read.
//! - `segv`: one plugin; reading it crashes. `hang`: one plugin; reading it never ends.
//! - `mixed`: `urn:meloet:fake:a`, `…:b` and `…:c`. Reading `b` crashes; `a` and `c` are read.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use meloet_plugin::ipc::ToPlugin;
use meloet_plugin::wire::{LoadedPlugin, ParamInfo, ParamShape};
use meloet_plugin::{
    EditorKind, EffectCategory, InstrumentCategory, PluginFormat, PluginInfo, PluginKey, PluginKind,
};

use super::{Activation, Candidate, Controller, Loaded, ProcessBlock, Processor};

pub const ENV: &str = "MELOET_PLUGHOST_FAKE";
const PREFIX: &str = "urn:meloet:fake:";
const LEVEL: u32 = 0;
const DEFAULT_LEVEL: f64 = 0.5;

fn enabled() -> bool {
    std::env::var_os(ENV).is_some_and(|v| !v.is_empty())
}

pub fn load(key: &PluginKey, activation: Activation) -> Option<Result<Loaded, String>> {
    if !enabled() || key.format() != PluginFormat::Lv2 {
        return None;
    }
    let name = key.id().strip_prefix(PREFIX)?;
    let level = Arc::new(AtomicU64::new(DEFAULT_LEVEL.to_bits()));
    let processor: Box<dyn Processor> = match name {
        "sine" => Box::new(Sine {
            sample_rate: activation.sample_rate,
            level: level.clone(),
            key: None,
            phase: 0.0,
        }),
        "pass" => Box::new(Pass { fault: None }),
        "segv" => Box::new(Pass {
            fault: Some(Fault::Crash),
        }),
        "hang" => Box::new(Pass {
            fault: Some(Fault::Hang),
        }),
        _ => return Some(Err(format!("no fake plugin {name}"))),
    };
    let instrument = name == "sine";
    Some(Ok(Loaded {
        plugin: LoadedPlugin {
            latency: 0,
            audio_inputs: if instrument { 0 } else { 2 },
            audio_outputs: 2,
            editor: false,
        },
        controller: Box::new(FakeController {
            level,
            has_level: instrument,
        }),
        processor,
    }))
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
        Fault::Hang => loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        },
    }
}

#[derive(Clone, Copy)]
enum Fault {
    Crash,
    Hang,
}

struct Pass {
    fault: Option<Fault>,
}

impl Processor for Pass {
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let note =
            |e: &ToPlugin| matches!(e, ToPlugin::Midi { data, .. } if data[0] & 0xf0 == 0x90);
        if let Some(fault) = self.fault
            && block.events.iter().any(note)
        {
            fail(fault);
        }
        for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
            output.copy_from_slice(input);
        }
    }

    fn reset(&mut self) {}
}

struct Sine {
    sample_rate: f64,
    level: Arc<AtomicU64>,
    key: Option<u8>,
    phase: f64,
}

impl Sine {
    fn take(&mut self, event: &ToPlugin) {
        match *event {
            ToPlugin::Midi { data, .. } => match (data[0] & 0xf0, data[1], data[2]) {
                (0x90, key, velocity) if velocity > 0 => self.key = Some(key),
                (0x90 | 0x80, key, _) if self.key == Some(key) => self.key = None,
                // All sound off, all notes off.
                (0xb0, 120 | 123, _) => self.key = None,
                _ => {}
            },
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
    }

    fn reset(&mut self) {
        self.key = None;
        self.phase = 0.0;
    }
}

struct FakeController {
    /// Shared with the processor, which stands in for a real plugin's own inner link.
    level: Arc<AtomicU64>,
    has_level: bool,
}

impl FakeController {
    fn level(&self) -> f64 {
        f64::from_bits(self.level.load(Relaxed))
    }
}

impl Controller for FakeController {
    fn params(&mut self) -> Vec<ParamInfo> {
        if !self.has_level {
            return Vec::new();
        }
        vec![ParamInfo {
            id: LEVEL,
            name: "Level".into(),
            group: None,
            default: DEFAULT_LEVEL,
            shape: ParamShape::Continuous,
            automatable: true,
        }]
    }

    fn param(&mut self, id: u32) -> Option<(f64, String)> {
        (self.has_level && id == LEVEL)
            .then(|| (self.level(), format!("{:.0} %", self.level() * 100.0)))
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if self.has_level && id == LEVEL && value.is_finite() {
            self.level.store(value.clamp(0.0, 1.0).to_bits(), Relaxed);
        }
    }

    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.level().to_le_bytes().to_vec())
    }

    fn load_state(&mut self, state: &[u8]) -> Result<(), String> {
        let bytes: [u8; 8] = state.try_into().map_err(|_| "not a fake plugin's state")?;
        let level = f64::from_le_bytes(bytes);
        if !(0.0..=1.0).contains(&level) {
            return Err("not a fake plugin's state".into());
        }
        self.level.store(level.to_bits(), Relaxed);
        Ok(())
    }

    fn open_editor(&mut self, _parent: u32) -> Result<(u32, u32), String> {
        Err("a fake plugin has no window".into())
    }

    fn close_editor(&mut self) {}

    fn idle(&mut self) {}
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
    Some(match stem {
        "found" => vec![("Sine", synth, None), ("Pass", effect, None)],
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
