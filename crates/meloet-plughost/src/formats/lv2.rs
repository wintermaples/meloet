//! LV2 behind [`Processor`] and [`Controller`]: livi runs the plugin, and what livi does not
//! do is here (parameters sent as `patch:Set`, reading what the plugin sends back, the delay
//! port, which bundle is read).
//!
//! A bundle's description (its Turtle files) is read once for the process and kept: reading
//! it can take a second and a hundred megabytes (LSP). Only the bundle asked for is read,
//! never every bundle of the PC.
//!
//! The instance is shared by the two halves, and LV2 does not allow its state to be taken or
//! put back while `run` runs. A [`Gate`] keeps the two apart: the audio thread never waits at
//! it, a block that meets the main thread inside goes round the plugin.
//!
//! A plugin need not answer `patch:Get` (Surge XT 1.3.4 does not know the word), and then the
//! value of a `patch` parameter is only what meloet last sent or the plugin last said by
//! itself. So those values travel with the state ([`STATE_MAGIC`]): a plugin that gets a state
//! back shows the values it was saved with, and one that does answer corrects them.
//!
//! Parameter ids: a control port's is its index. A `patch:writable` parameter has no number
//! of its own, so its id is made from its URI ([`patch_id`]), with the top bit set.
//!
//! The plugin's window ([`ui`]) is on the main thread and talks to the instance on the audio
//! thread through the same doors as meloet: what it writes to a control port or sends as
//! `patch:Set` becomes a parameter change of the next block; any other message it sends, and
//! everything the instance sends back, crosses in two rings of bytes. The audio thread fills
//! its ring only while a window is open.

mod midnam;
mod presets;
mod ui;

use std::cell::{Cell, RefCell, UnsafeCell};
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

use livi::event::{LV2AtomEventBuilder, LV2AtomSequence};
use livi::lilv;
use livi::{EmptyPortConnections, Features, FeaturesBuilder, PortIndex, PortType};
use meloet_plugin::ipc::{FromPlugin, MAX_LATENCY_FRAMES, ToPlugin, Transport};
use meloet_plugin::wire::{KeyName, LoadedPlugin, ParamInfo, ParamNow, ParamShape, PresetInfo};
use meloet_plugin::{
    EditorKind, InstrumentCategory, PluginInfo, PluginKey, PluginKind, PluginRole,
};

use super::outputs::{Outputs, Ports, SetShape, Side};
use super::{
    Activation, Candidate, Controller, GATE_WAIT, Gate, Loaded, Opened, ProcessBlock, Processor,
    Read, Said, leading_number,
};
use crate::rtcheck;

const LV2: &str = "http://lv2plug.in/ns/lv2core#";
const ATOM: &str = "http://lv2plug.in/ns/ext/atom#";
const PATCH: &str = "http://lv2plug.in/ns/ext/patch#";
const TIME: &str = "http://lv2plug.in/ns/ext/time#";
const UI: &str = "http://lv2plug.in/ns/extensions/ui#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const MIDI_EVENT: &str = "http://lv2plug.in/ns/ext/midi#MidiEvent";
const PG: &str = "http://lv2plug.in/ns/ext/port-groups#";
const GROUP: &str = "http://lv2plug.in/ns/ext/port-groups#group";
const MINIMUM_SIZE: &str = "http://lv2plug.in/ns/ext/resize-port#minimumSize";
const NOT_ON_GUI: &str = "http://lv2plug.in/ns/ext/port-props#notOnGUI";
const UNIT: &str = "http://lv2plug.in/ns/extensions/units#unit";
const UNIT_SYMBOL: &str = "http://lv2plug.in/ns/extensions/units#symbol";
const SIDE_CHAIN_OF: &str = "http://lv2plug.in/ns/ext/port-groups#sideChainOf";
const SUB_GROUP_OF: &str = "http://lv2plug.in/ns/ext/port-groups#subGroupOf";
/// How far up a tree of groups its top is looked for.
const MOST_GROUPS: usize = 16;

/// Bytes an atom port holds when the plugin does not ask for more.
const ATOM_BYTES: usize = 16 * 1024;
/// `patch:writable` parameters that change in one block; a block with more loses the rest.
const BLOCK_SETS: usize = 256;
/// A block whose control inputs have automation points in it is run in pieces cut at the
/// points, none shorter than this (docs/architecture.md 21-7).
const SHORTEST_PIECE: usize = 32;
/// Control inputs that have points in one block, at most; more keep their last value.
const MOVING_PORTS: usize = 64;
const BUF_SIZE: &str = "http://lv2plug.in/ns/ext/buf-size#";
const PATCH_BIT: u32 = 1 << 31;
/// The largest message a window sends its plugin in one piece; larger ones are dropped.
const UI_ATOM_BYTES: usize = 2048;
/// The rings between a window and its plugin.
const FROM_UI_BYTES: usize = 64 * 1024;
const TO_UI_BYTES: usize = 256 * 1024;
/// A parameter with more steps than this has no list of their texts.
const MOST_LABELS: u32 = 128;

struct World {
    world: livi::World,
    /// By the longest block: what an instance is handed when it is made.
    features: RefCell<HashMap<usize, Arc<Features>>>,
    /// Bundles of presets read into `world` besides its own ([`presets`]).
    preset_bundles: RefCell<HashSet<PathBuf>>,
}

thread_local! {
    /// The bundles this process has read. Main thread only; never emptied.
    static WORLDS: RefCell<HashMap<PathBuf, Rc<World>>> = RefCell::new(HashMap::new());
}

/// `file:///usr/lib/lv2/Surge%20XT.lv2/`, as lilv wants a bundle named.
fn bundle_uri(bundle: &Path) -> Result<String, String> {
    let bundle = bundle.canonicalize().map_err(|e| e.to_string())?;
    let mut uri = String::from("file://");
    for &byte in bundle.as_os_str().as_encoded_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    if !uri.ends_with('/') {
        uri.push('/');
    }
    Ok(uri)
}

fn world(bundle: &Path) -> Result<Rc<World>, String> {
    if let Some(world) = WORLDS.with_borrow(|w| w.get(bundle).cloned()) {
        return Ok(world);
    }
    if !bundle.join("manifest.ttl").is_file() {
        return Err("no manifest.ttl".into());
    }
    let world = Rc::new(World {
        world: livi::World::with_load_bundle(&bundle_uri(bundle)?),
        features: RefCell::new(HashMap::new()),
        preset_bundles: RefCell::new(HashSet::new()),
    });
    WORLDS.with_borrow_mut(|w| w.insert(bundle.to_path_buf(), world.clone()));
    Ok(world)
}

impl World {
    fn features(&self, max_frames: usize) -> Result<Arc<Features>, String> {
        if let Some(features) = self.features.borrow().get(&max_frames) {
            return Ok(features.clone());
        }
        let features = self
            .world
            .try_build_features(FeaturesBuilder {
                min_block_length: 1,
                max_block_length: max_frames,
            })
            .map_err(|e| format!("features: {e:?}"))?;
        self.features
            .borrow_mut()
            .insert(max_frames, features.clone());
        Ok(features)
    }

    fn uri(&self, uri: &str) -> lilv::node::Node {
        self.world.raw().new_uri(uri)
    }

    fn get(&self, subject: &lilv::node::Node, predicate: &str) -> Option<lilv::node::Node> {
        self.world
            .raw()
            .get(Some(subject), Some(&self.uri(predicate)), None)
    }

    fn text(&self, subject: &lilv::node::Node, predicate: &str) -> Option<String> {
        self.get(subject, predicate)?.as_str().map(str::to_owned)
    }

    fn number(&self, subject: &lilv::node::Node, predicate: &str) -> Option<f64> {
        number(&self.get(subject, predicate)?)
    }
}

fn number(node: &lilv::node::Node) -> Option<f64> {
    node.as_float()
        .map(f64::from)
        .or_else(|| node.as_int().map(f64::from))
        .or_else(|| node.as_bool().map(|b| if b { 1.0 } else { 0.0 }))
        .filter(|x| x.is_finite())
}

/// The plugins of the bundle that meloet can run; `Err` when the bundle cannot be read.
pub fn list(bundle: &Path) -> Result<Vec<Candidate>, String> {
    let world = world(bundle)?;
    let runnable: Vec<Candidate> = world
        .world
        .iter_plugins()
        .map(|plugin| Candidate {
            id: plugin.uri(),
            name: Some(plugin.name()),
        })
        .collect();
    // livi leaves out what it cannot run (a feature it does not have, a kind of port it does
    // not know). Those are not broken; they are only named in the log.
    for plugin in world.world.raw().plugins().iter() {
        if let Some(uri) = plugin.uri().as_uri()
            && !runnable.iter().any(|c| c.id == uri)
        {
            eprintln!("meloet-plughost: {uri} needs something meloet does not have; left out");
        }
    }
    Ok(runnable)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Range {
    Float,
    Double,
    Int,
    Bool,
}

enum Target {
    /// A control port.
    Port(PortIndex),
    /// A `patch:writable` parameter: its URID and the atom type of its value.
    Patch { urid: u32, range: Range },
}

struct Param {
    id: u32,
    target: Target,
    name: String,
    group: Option<String>,
    unit: String,
    min: f64,
    max: f64,
    default: f64,
    shape: ParamShape,
    automatable: bool,
    /// Not for a person: the plugin says so (`notOnGUI`), or it is the host's own switch
    /// (`lv2:enabled`, `lv2:freeWheeling`). The plugin's window may still write it.
    hidden: bool,
    /// Values with names of their own.
    points: Vec<(f64, String)>,
}

impl Param {
    fn normal(&self, plain: f64) -> f64 {
        if self.max > self.min && plain.is_finite() {
            ((plain - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn plain(&self, normal: f64) -> f64 {
        let plain = self.min + (self.max - self.min) * normal.clamp(0.0, 1.0);
        match self.shape {
            ParamShape::Continuous => plain,
            _ => plain.round(),
        }
    }

    fn text(&self, normal: f64) -> String {
        let plain = self.plain(normal);
        if let Some((_, name)) = self.points.iter().find(|(at, _)| (at - plain).abs() < 1e-6) {
            return name.clone();
        }
        let mut text = match self.shape {
            ParamShape::Continuous => format!("{plain:.2}"),
            ParamShape::Toggle => return if plain >= 0.5 { "On" } else { "Off" }.to_string(),
            ParamShape::Stepped { .. } => format!("{plain:.0}"),
        };
        if !self.unit.is_empty() {
            text.push(' ');
            text.push_str(&self.unit);
        }
        text
    }

    /// The text of every step, for a parameter with few enough.
    fn labels(&self) -> Vec<String> {
        match self.shape {
            ParamShape::Stepped { steps } if steps <= MOST_LABELS => (0..steps)
                .map(|step| self.text(f64::from(step) / f64::from(steps - 1)))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The value a typed text stands for: the name of one of its values, on and off for a
    /// switch, or a number in the parameter's own unit.
    fn read(&self, text: &str) -> Option<Read> {
        let text = text.trim();
        let exact = |plain: f64| Read {
            value: self.normal(plain),
            clamped: Some(false),
        };
        if let Some((plain, _)) =
            (self.points.iter()).find(|(_, name)| name.eq_ignore_ascii_case(text))
        {
            return Some(exact(*plain));
        }
        if self.shape == ParamShape::Toggle {
            if text.eq_ignore_ascii_case("on") {
                return Some(exact(self.max));
            }
            if text.eq_ignore_ascii_case("off") {
                return Some(exact(self.min));
            }
        }
        let plain = leading_number(text)?;
        if plain.is_nan() {
            return None;
        }
        let slack = (self.max - self.min) * 1e-9;
        Some(Read {
            value: if plain == f64::INFINITY {
                1.0
            } else {
                self.normal(plain)
            },
            clamped: Some(plain < self.min - slack || plain > self.max + slack),
        })
    }
}

fn stepped(min: f64, max: f64) -> ParamShape {
    match (max - min).round() {
        span if span < 1.0 => ParamShape::Continuous,
        span if span < 2.0 => ParamShape::Toggle,
        span => ParamShape::Stepped {
            steps: span.min(65_535.0) as u32 + 1,
        },
    }
}

/// FNV-1a of the URI, top bit set. Two parameters that land on one number are told apart by
/// the caller.
fn patch_id(uri: &str) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for byte in uri.bytes() {
        hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
    }
    hash | PATCH_BIT
}

fn unit_symbol(world: &World, subject: &lilv::node::Node) -> String {
    let Some(unit) = world.get(subject, UNIT) else {
        return String::new();
    };
    if let Some(symbol) = world.text(&unit, UNIT_SYMBOL) {
        return symbol;
    }
    let known = [
        ("db", "dB"),
        ("hz", "Hz"),
        ("khz", "kHz"),
        ("ms", "ms"),
        ("s", "s"),
        ("pc", "%"),
        ("bpm", "BPM"),
        ("cent", "ct"),
        ("semitone12TET", "st"),
        ("degree", "°"),
    ];
    let name = unit
        .as_uri()
        .and_then(|uri| uri.strip_prefix("http://lv2plug.in/ns/extensions/units#"));
    known
        .iter()
        .find(|(unit, _)| Some(*unit) == name)
        .map_or(String::new(), |(_, symbol)| symbol.to_string())
}

/// The name of the group a port or a parameter is shown under: of a tree of groups, the one
/// right below the tree's root. What sits in the root itself is in no group (Surge XT has one
/// root, "Root", with fourteen groups in it).
fn group_name(
    world: &World,
    subject: &lilv::node::Node,
    names: &mut HashMap<String, Option<String>>,
) -> Option<String> {
    let mut group = world.get(subject, GROUP)?;
    let uri = group.as_uri()?.to_string();
    if let Some(name) = names.get(&uri) {
        return name.clone();
    }
    let mut below_root = None;
    for _ in 0..MOST_GROUPS {
        let Some(parent) = world.get(&group, SUB_GROUP_OF) else {
            break;
        };
        below_root = Some(group);
        group = parent;
    }
    let is_root = |group: &lilv::node::Node| {
        let children = (world.world.raw()).find_nodes(None, &world.uri(SUB_GROUP_OF), Some(group));
        children.count() > 0
    };
    let named = match below_root {
        Some(group) => Some(group),
        None if is_root(&group) => None,
        None => Some(group),
    };
    let name = named.and_then(|group| {
        world
            .text(&group, &format!("{LV2}name"))
            .or_else(|| world.text(&group, &format!("{RDFS}label")))
            .filter(|name| !name.is_empty())
    });
    names.insert(uri, name.clone());
    name
}

/// What the plugin's description says of its ports and parameters.
struct Layout {
    params: Vec<Param>,
    /// Among the atom inputs: where MIDI goes, where `patch` messages go, and where the
    /// song's position (`time:Position`) goes.
    midi_in: Option<usize>,
    patch_in: Option<usize>,
    time_in: Option<usize>,
    /// The control inputs that are the host's to fill with the tempo (`lv2:designation
    /// time:beatsPerMinute`) and with whether the song moves (`time:speed`).
    tempo_ports: Vec<PortIndex>,
    speed_ports: Vec<PortIndex>,
    /// The control inputs that are the host's to say with that the blocks come faster than
    /// time passes (`lv2:designation lv2:freeWheeling`).
    freewheel_ports: Vec<PortIndex>,
    atom_in_bytes: Vec<usize>,
    atom_out_bytes: Vec<usize>,
    latency_port: Option<PortIndex>,
    /// Among the audio inputs: the one or two that take the plugin's main sound (left,
    /// right).
    main_in: Vec<usize>,
    /// Among the audio outputs: the ones that are heard ([`heard_outputs`]).
    heard_out: Vec<(usize, Side)>,
    /// The audio outputs in sets ([`output_sets`]).
    output_sets: Vec<SetShape>,
    instrument: bool,
    /// The port numbers of the atom inputs, of the atom outputs and of the control outputs,
    /// each in the order livi has them.
    atom_in_ports: Vec<u32>,
    atom_out_ports: Vec<u32>,
    control_out_ports: Vec<u32>,
    /// Every port's number by its symbol.
    symbols: HashMap<CString, u32>,
}

/// What the description says of one audio port, as much as decides whether it is heard and
/// where.
#[derive(Debug, Clone, Default)]
struct AudioPort {
    group: Option<String>,
    /// The group's own `lv2:name` (or `rdfs:label`).
    group_name: Option<String>,
    /// Also a port the description does not have: never heard.
    side_chain: bool,
    /// `lv2:connectionOptional`.
    optional: bool,
    /// `lv2:designation pg:left` or `pg:right`.
    designated: Option<Side>,
    name: String,
    symbol: String,
}

fn audio_ports(world: &World, plugin: &livi::Plugin, direction: PortType) -> Vec<AudioPort> {
    let raw = plugin.raw();
    let side_chain = world.uri(&format!("{LV2}isSideChain"));
    let optional = world.uri(&format!("{LV2}connectionOptional"));
    let designation = world.uri(&format!("{LV2}designation"));
    let (left, right) = (format!("{PG}left"), format!("{PG}right"));
    (plugin.ports_with_type(direction))
        .map(|port| {
            let Some(lilv_port) = raw.port_by_index(port.index.0) else {
                return AudioPort {
                    side_chain: true,
                    ..AudioPort::default()
                };
            };
            let group = world.get(&lilv_port.node(), GROUP);
            let side_chain = lilv_port.has_property(&side_chain)
                || (group.as_ref()).is_some_and(|g| world.get(g, SIDE_CHAIN_OF).is_some());
            let designated = lilv_port.get(&designation).and_then(|d| match d.as_uri() {
                Some(uri) if uri == left => Some(Side::Left),
                Some(uri) if uri == right => Some(Side::Right),
                _ => None,
            });
            let group_name = group.as_ref().and_then(|group| {
                (world.text(group, &format!("{LV2}name")))
                    .or_else(|| world.text(group, &format!("{RDFS}label")))
                    .filter(|name| !name.is_empty())
            });
            AudioPort {
                group: group.and_then(|g| g.as_uri().map(str::to_owned)),
                group_name,
                side_chain,
                optional: lilv_port.has_property(&optional),
                designated,
                name: port.name,
                symbol: port.symbol,
            }
        })
        .collect()
}

/// The ports in the group of the first port that is not a side chain ("no group" is a group
/// too), in port order: where the plugin's main sound is.
fn main_group(ports: &[AudioPort]) -> Vec<usize> {
    let Some(first) = ports.iter().find(|port| !port.side_chain) else {
        return Vec::new();
    };
    (ports.iter().enumerate())
        .filter(|(_, port)| !port.side_chain && port.group == first.group)
        .map(|(at, _)| at)
        .collect()
}

/// Which audio outputs are heard, in port order, and where each goes.
///
/// The main group ([`main_group`]), of an effect its first two only. An instrument whose
/// first output is in no group names no main sound (DrumGizmo, avldrums' "Multi" kits): every
/// output that is not a side chain is heard, except those of a group that are
/// `lv2:connectionOptional`.
///
/// One output goes to both sides, two to left and right. Of more, each goes by its
/// designation, else by the last word of its name ("Cymbals L", "out_right"), else to both.
fn heard_outputs(ports: &[AudioPort], instrument: bool) -> Vec<(usize, Side)> {
    let no_main = (ports.iter().find(|port| !port.side_chain)).is_some_and(|p| p.group.is_none());
    let heard: Vec<usize> = if instrument && no_main {
        (ports.iter().enumerate())
            .filter(|(_, port)| !port.side_chain && (port.group.is_none() || !port.optional))
            .map(|(at, _)| at)
            .collect()
    } else {
        let mut main = main_group(ports);
        if !instrument {
            main.truncate(2);
        }
        main
    };
    match heard[..] {
        [only] => vec![(only, Side::Both)],
        [left, right] => vec![(left, Side::Left), (right, Side::Right)],
        _ => (heard.iter()).map(|&at| (at, side(&ports[at]))).collect(),
    }
}

fn side(port: &AudioPort) -> Side {
    if let Some(side) = port.designated {
        return side;
    }
    let last = (port.name)
        .rsplit(|c: char| !c.is_alphanumeric())
        .find(|word| !word.is_empty());
    let is = |word: &str, names: [&str; 2]| names.iter().any(|n| word.eq_ignore_ascii_case(n));
    match last {
        Some(word) if is(word, ["l", "left"]) => Side::Left,
        Some(word) if is(word, ["r", "right"]) => Side::Right,
        _ => Side::Both,
    }
}

/// The audio outputs in sets, in port order (docs/architecture.md 20-15). Every output that is
/// not a side chain is in one set.
///
/// The two outputs of a group are one stereo set (left: the one designated `pg:left`, else
/// the first); so are the two outputs of a plugin that has only those, in no group. Every
/// other output is a set of its own. A set is heard by default as `heard` ([`heard_outputs`])
/// says; a pair that would not be heard so (one of the two heard, or the two on other sides
/// than left and right) is two sets of one.
fn output_sets(ports: &[AudioPort], heard: &[(usize, Side)]) -> Vec<SetShape> {
    let ours: Vec<usize> = (ports.iter().enumerate())
        .filter(|(_, port)| !port.side_chain)
        .map(|(at, _)| at)
        .collect();
    let heard_at = |at: usize| heard.iter().find(|(p, _)| *p == at).map(|&(_, side)| side);
    let pair = |a: usize, b: usize| {
        let b_is_left = ports[b].designated == Some(Side::Left);
        let (left, right) = if b_is_left && ports[a].designated != Some(Side::Left) {
            (b, a)
        } else {
            (a, b)
        };
        match (heard_at(left), heard_at(right)) {
            (None, None) | (Some(Side::Left), Some(Side::Right)) => Some((left, right)),
            _ => None,
        }
    };
    let mut pairs = Vec::new();
    match ours[..] {
        [a, b] if ports[a].group.is_none() && ports[b].group.is_none() => pairs.extend(pair(a, b)),
        _ => {
            let mut groups: Vec<&str> = Vec::new();
            for &at in &ours {
                if let Some(group) = ports[at].group.as_deref()
                    && !groups.contains(&group)
                {
                    groups.push(group);
                }
            }
            for group in groups {
                let members: Vec<usize> = (ours.iter().copied())
                    .filter(|&at| ports[at].group.as_deref() == Some(group))
                    .collect();
                if let [a, b] = members[..] {
                    pairs.extend(pair(a, b));
                }
            }
        }
    }
    let mut sets = Vec::new();
    for &(left, right) in &pairs {
        sets.push(SetShape {
            id: ports[left].symbol.clone(),
            name: (ports[left].group_name.clone()).unwrap_or_else(|| ports[left].name.clone()),
            ports: Ports::Stereo { left, right },
            on: heard_at(left).is_some(),
            pan: 0.0,
        });
    }
    for &at in &ours {
        if pairs.iter().any(|&(l, r)| at == l || at == r) {
            continue;
        }
        sets.push(SetShape {
            id: ports[at].symbol.clone(),
            name: ports[at].name.clone(),
            ports: Ports::Mono(at),
            on: heard_at(at).is_some(),
            pan: heard_at(at).unwrap_or_else(|| side(&ports[at])).pan(),
        });
    }
    sets.sort_by_key(|set| set.ports.first());
    sets
}

fn layout(world: &World, plugin: &livi::Plugin, features: &Features) -> Layout {
    let raw = plugin.raw();
    let has = |port: &lilv::port::Port, property: &str| port.has_property(&world.uri(property));
    let designated = |port: &lilv::port::Port, what: &str| {
        port.get(&world.uri(&format!("{LV2}designation")))
            .is_some_and(|d| d.as_uri() == Some(&format!("{LV2}{what}")))
    };
    let timed = |port: &lilv::port::Port, what: &str| {
        port.get(&world.uri(&format!("{LV2}designation")))
            .is_some_and(|d| d.as_uri() == Some(&format!("{TIME}{what}")))
    };
    let mut params = Vec::new();
    let mut groups = HashMap::new();
    let (mut tempo_ports, mut speed_ports) = (Vec::new(), Vec::new());
    let mut freewheel_ports = Vec::new();
    for port in plugin.ports_with_type(PortType::ControlInput) {
        let Some(lilv_port) = raw.port_by_index(port.index.0) else {
            continue;
        };
        let (min, max) = (
            f64::from(port.min_value.unwrap_or(0.0)),
            f64::from(port.max_value.unwrap_or(1.0)),
        );
        let (min, max) = if max > min { (min, max) } else { (0.0, 1.0) };
        let whole = has(&lilv_port, &format!("{LV2}integer"))
            || has(&lilv_port, &format!("{LV2}enumeration"));
        let shape = if has(&lilv_port, &format!("{LV2}toggled")) {
            ParamShape::Toggle
        } else if whole {
            stepped(min, max)
        } else {
            ParamShape::Continuous
        };
        let points = lilv_port
            .scale_points()
            .iter()
            .filter_map(|point| {
                let label = point.label().as_str()?.to_string();
                Some((number(&point.value())?, label))
            })
            .collect();
        let subject = lilv_port.node();
        let (tempo, speed) = (
            timed(&lilv_port, "beatsPerMinute"),
            timed(&lilv_port, "speed"),
        );
        if tempo {
            tempo_ports.push(port.index);
        }
        if speed {
            speed_ports.push(port.index);
        }
        let freewheel = designated(&lilv_port, "freeWheeling");
        if freewheel {
            freewheel_ports.push(port.index);
        }
        let hosts_own = designated(&lilv_port, "enabled") || freewheel || tempo || speed;
        params.push(Param {
            id: port.index.0 as u32,
            target: Target::Port(port.index),
            name: port.name.clone(),
            group: group_name(world, &subject, &mut groups),
            unit: unit_symbol(world, &subject),
            min,
            max,
            default: f64::from(port.default_value),
            shape,
            automatable: !hosts_own,
            hidden: hosts_own || has(&lilv_port, NOT_ON_GUI),
            points,
        });
    }

    let writable = world.world.raw().find_nodes(
        Some(&raw.uri()),
        &world.uri(&format!("{PATCH}writable")),
        None,
    );
    let mut patches: Vec<(String, lilv::node::Node)> = writable
        .iter()
        .filter_map(|node| Some((node.as_uri()?.to_string(), node)))
        .collect();
    // In the order of their URIs, so that two that land on one id are told apart the same way
    // every time.
    patches.sort_by(|a, b| a.0.cmp(&b.0));
    for (uri, node) in patches {
        let range = world.get(&node, &format!("{RDFS}range"));
        let range = match range.as_ref().and_then(|r| r.as_uri()) {
            Some(r) if r == format!("{ATOM}Float") => Range::Float,
            Some(r) if r == format!("{ATOM}Double") => Range::Double,
            Some(r) if r == format!("{ATOM}Int") => Range::Int,
            Some(r) if r == format!("{ATOM}Bool") => Range::Bool,
            _ => continue,
        };
        let Ok(c_uri) = std::ffi::CString::new(uri.as_str()) else {
            continue;
        };
        let (min, max) = (
            world.number(&node, &format!("{LV2}minimum")).unwrap_or(0.0),
            world.number(&node, &format!("{LV2}maximum")).unwrap_or(1.0),
        );
        let (min, max) = if max > min { (min, max) } else { (0.0, 1.0) };
        let mut id = patch_id(&uri);
        while params.iter().any(|p: &Param| p.id == id) {
            id = id.wrapping_add(1) | PATCH_BIT;
        }
        params.push(Param {
            id,
            target: Target::Patch {
                urid: features.urid(&c_uri),
                range,
            },
            name: world.text(&node, &format!("{RDFS}label")).unwrap_or(uri),
            group: group_name(world, &node, &mut groups),
            unit: unit_symbol(world, &node),
            min,
            max,
            default: world.number(&node, &format!("{LV2}default")).unwrap_or(min),
            shape: match range {
                Range::Bool => ParamShape::Toggle,
                Range::Int => stepped(min, max),
                Range::Float | Range::Double => ParamShape::Continuous,
            },
            automatable: true,
            hidden: false,
            points: Vec::new(),
        });
    }

    let (midi, message) = (world.uri(MIDI_EVENT), world.uri(&format!("{PATCH}Message")));
    let position = world.uri(&format!("{TIME}Position"));
    let bytes = |port: &lilv::port::Port| {
        let asked = port
            .get(&world.uri(MINIMUM_SIZE))
            .and_then(|n| n.as_int())
            .map_or(0, |n| n.max(0) as usize);
        asked.max(ATOM_BYTES)
    };
    let numbers = |kind: PortType| -> Vec<u32> {
        (plugin.ports_with_type(kind))
            .map(|port| port.index.0 as u32)
            .collect()
    };
    let symbols = (plugin.ports())
        .filter_map(|port| {
            let symbol = raw.port_by_index(port.index.0)?.symbol()?;
            Some((CString::new(symbol.as_str()?).ok()?, port.index.0 as u32))
        })
        .collect();
    let (mut midi_in, mut patch_in, mut time_in) = (None, None, None);
    let mut atom_in_bytes = Vec::new();
    for (at, port) in plugin
        .ports_with_type(PortType::AtomSequenceInput)
        .enumerate()
    {
        let Some(port) = raw.port_by_index(port.index.0) else {
            atom_in_bytes.push(ATOM_BYTES);
            continue;
        };
        atom_in_bytes.push(bytes(&port));
        let control = designated(&port, "control");
        if port.supports_event(&midi) && (midi_in.is_none() || control) {
            midi_in = Some(at);
        }
        if port.supports_event(&message) && (patch_in.is_none() || control) {
            patch_in = Some(at);
        }
        if port.supports_event(&position) && (time_in.is_none() || control) {
            time_in = Some(at);
        }
    }
    let atom_out_bytes = plugin
        .ports_with_type(PortType::AtomSequenceOutput)
        .map(|port| {
            raw.port_by_index(port.index.0)
                .map_or(ATOM_BYTES, |p| bytes(&p))
        })
        .collect();
    let mut main_in = main_group(&audio_ports(world, plugin, PortType::AudioInput));
    main_in.truncate(2);
    let instrument = plugin_kind(world, plugin).role() == PluginRole::Instrument;
    let outputs = audio_ports(world, plugin, PortType::AudioOutput);
    let heard_out = heard_outputs(&outputs, instrument);
    Layout {
        params,
        midi_in,
        patch_in,
        time_in,
        tempo_ports,
        speed_ports,
        freewheel_ports,
        atom_in_bytes,
        atom_out_bytes,
        latency_port: raw.latency_port_index().map(PortIndex),
        main_in,
        output_sets: output_sets(&outputs, &heard_out),
        heard_out,
        instrument,
        atom_in_ports: numbers(PortType::AtomSequenceInput),
        atom_out_ports: numbers(PortType::AtomSequenceOutput),
        control_out_ports: numbers(PortType::ControlOutput),
        symbols,
    }
}

/// `/usr/lib/lv2/Surge XT.lv2/x.so` from the `file:` URI lilv names a file by.
fn file_path(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file://")?;
    let mut bytes = Vec::with_capacity(path.len());
    let mut rest = path.as_bytes();
    while let Some((&byte, after)) = rest.split_first() {
        let hex = (byte == b'%')
            .then(|| after.get(..2))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match hex {
            Some(byte) => {
                bytes.push(byte);
                rest = &after[2..];
            }
            None => {
                bytes.push(byte);
                rest = after;
            }
        }
    }
    Some(std::ffi::OsString::from_vec(bytes).into())
}

/// The files of the window meloet opens for a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UiFiles {
    uri: String,
    bundle: PathBuf,
    binary: PathBuf,
}

/// Whether the plugin has a window, and the one meloet can open if it has: an `ui:X11UI`
/// that needs nothing meloet does not hand it.
fn window(world: &World, raw: &lilv::plugin::Plugin) -> (EditorKind, Option<UiFiles>) {
    let Some(uis) = raw.uis() else {
        return (EditorKind::None, None);
    };
    let x11 = world.uri(&format!("{UI}X11UI"));
    let required = world.uri(&format!("{LV2}requiredFeature"));
    let mut any = false;
    for ui in uis.iter() {
        any = true;
        if !ui.is_a(&x11) {
            continue;
        }
        let subject = ui.uri();
        let needs = world
            .world
            .raw()
            .find_nodes(Some(&subject), &required, None);
        let has_all = (needs.iter())
            .all(|feature| (feature.as_uri()).is_some_and(|uri| ui::FEATURES.contains(&uri)));
        let path = |node: Option<lilv::node::Node>| file_path(node?.as_uri()?);
        if let (true, Some(uri), Some(bundle), Some(binary)) = (
            has_all,
            subject.as_uri(),
            path(ui.bundle_uri()),
            path(ui.binary_uri()),
        ) && binary.is_file()
        {
            let files = UiFiles {
                uri: uri.to_string(),
                bundle,
                binary,
            };
            return (EditorKind::X11, Some(files));
        }
    }
    let kind = if any {
        EditorKind::Unsupported
    } else {
        EditorKind::None
    };
    (kind, None)
}

/// By the plugin's classes; an effect that hears MIDI and has audio outputs but no audio
/// inputs is an instrument.
fn plugin_kind(world: &World, plugin: &livi::Plugin) -> PluginKind {
    let raw = plugin.raw();
    let classes: Vec<String> = raw
        .value(&world.uri(RDF_TYPE))
        .iter()
        .filter_map(|class| class.as_uri().map(str::to_owned))
        .collect();
    let counts = *plugin.port_counts();
    let midi = world.uri(MIDI_EVENT);
    let hears_midi = plugin
        .ports_with_type(PortType::AtomSequenceInput)
        .filter_map(|port| raw.port_by_index(port.index.0))
        .any(|port| port.supports_event(&midi));
    let kind = PluginKind::from_lv2(&classes);
    if kind.role() == PluginRole::Effect
        && hears_midi
        && counts.audio_inputs == 0
        && counts.audio_outputs > 0
    {
        return PluginKind::Instrument(InstrumentCategory::Other);
    }
    kind
}

/// Reads one plugin of [`list`]; `Err` when that plugin cannot be read. The plugin is made
/// and run for one block, so that one that cannot even do that is found here and not in a
/// song.
pub fn probe(bundle: &Path, id: &str) -> Result<PluginInfo, String> {
    let lv2_world = world(bundle)?;
    let plugin = lv2_world
        .world
        .plugin_by_uri(id)
        .ok_or_else(|| format!("no plugin {id} in the bundle"))?;
    let raw = plugin.raw();
    let kind = plugin_kind(&lv2_world, &plugin);
    let (editor, _) = window(&lv2_world, raw);
    let whole = |predicate: &str| {
        raw.value(&lv2_world.uri(&format!("{LV2}{predicate}")))
            .iter()
            .find_map(|n| n.as_int())
    };
    let version = match (whole("minorVersion"), whole("microVersion")) {
        (None, None) => String::new(),
        (minor, micro) => format!("{}.{}", minor.unwrap_or(0), micro.unwrap_or(0)),
    };
    let mut info = PluginInfo {
        key: PluginKey::lv2(id).map_err(|e| e.to_string())?,
        name: plugin.name(),
        vendor: raw
            .author_name()
            .and_then(|name| name.as_str().map(str::to_owned))
            .unwrap_or_default(),
        version,
        kind,
        editor,
        audio_inputs: 0,
        audio_outputs: 0,
    };

    let activation = Activation {
        sample_rate: 48_000.0,
        max_frames: 256,
        offline: false,
    };
    let (_shared, mut processor, _link) = instantiate(&lv2_world, &plugin, activation)?;
    info.audio_inputs = processor.main_in.len() as u32;
    info.audio_outputs = processor.channels_out();
    let silence = [0.0f32; 256];
    let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
    let mut sink = super::EventSink::with_capacity(16);
    processor.process(&mut ProcessBlock {
        inputs: [&silence, &silence],
        outputs: [&mut left, &mut right],
        events: &[],
        transport: &Default::default(),
        sink: &mut sink,
    });
    Ok(info)
}

/// The body of a `time:Position` object: id and type, then eight properties of 24 bytes.
const POSITION_BYTES: usize = 8 + 8 * 24;

/// The song's position for LV2's `time:Position`. `beat` and `beats_per_minute` count quarter
/// notes whatever the meter, as the hosts and plugins found do (docs/architecture.md 22-2);
/// only `bar_beat` counts notes of the meter's lower number (`beatUnit`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct SongTime {
    frame: i64,
    speed: f32,
    bar: i64,
    bar_beat: f32,
    beat: f64,
    beat_unit: i32,
    beats_per_bar: f32,
    beats_per_minute: f32,
}

impl SongTime {
    fn of(transport: &Transport) -> SongTime {
        let number = |x: f64| if x.is_finite() { x } else { 0.0 };
        let (num, den) = match (transport.meter_num, transport.meter_den) {
            (0, _) | (_, 0) => (4, 4),
            meter => meter,
        };
        // Notes of `beatUnit` to a quarter note.
        let per_quarter = f64::from(den) / 4.0;
        let position = number(transport.position_quarters);
        let quarter_bpm = match number(transport.tempo_bpm) {
            bpm if bpm > 0.0 => bpm,
            _ => 120.0,
        };
        SongTime {
            frame: transport.position_frames,
            speed: if transport.flags & Transport::PLAYING != 0 {
                1.0
            } else {
                0.0
            },
            bar: transport.bar.min(i64::MAX as u64) as i64,
            bar_beat: ((position - number(transport.bar_start_quarters)) * per_quarter) as f32,
            beat: position,
            beat_unit: i32::from(den),
            beats_per_bar: f32::from(num),
            beats_per_minute: quarter_bpm as f32,
        }
    }

    fn object(&self, urids: &Urids) -> [u8; POSITION_BYTES] {
        let time = &urids.time;
        let mut body = [0u8; POSITION_BYTES];
        body[4..8].copy_from_slice(&time.position.to_ne_bytes());
        let mut at = 8;
        // A property: key, context, size and type of the value, the value in 8 bytes.
        let mut property = |key: u32, kind: u32, size: u32, value: [u8; 8]| {
            for (i, word) in [key, 0, size, kind].into_iter().enumerate() {
                body[at + i * 4..at + i * 4 + 4].copy_from_slice(&word.to_ne_bytes());
            }
            body[at + 16..at + 24].copy_from_slice(&value);
            at += 24;
        };
        let float = |x: f32| {
            let mut value = [0u8; 8];
            value[..4].copy_from_slice(&x.to_ne_bytes());
            value
        };
        let mut int = [0u8; 8];
        int[..4].copy_from_slice(&self.beat_unit.to_ne_bytes());
        property(time.frame, urids.long, 8, self.frame.to_ne_bytes());
        property(time.speed, urids.float, 4, float(self.speed));
        property(time.bar, urids.long, 8, self.bar.to_ne_bytes());
        property(time.bar_beat, urids.float, 4, float(self.bar_beat));
        property(time.beat, urids.double, 8, self.beat.to_ne_bytes());
        property(time.beat_unit, urids.int, 4, int);
        property(
            time.beats_per_bar,
            urids.float,
            4,
            float(self.beats_per_bar),
        );
        property(
            time.beats_per_minute,
            urids.float,
            4,
            float(self.beats_per_minute),
        );
        body
    }
}

struct Urids {
    midi: u32,
    object: u32,
    blank: u32,
    urid: u32,
    float: u32,
    double: u32,
    int: u32,
    long: u32,
    bool: u32,
    get: u32,
    set: u32,
    put: u32,
    property: u32,
    value: u32,
    body: u32,
    time: TimeUrids,
}

/// `time:Position` and its fields.
struct TimeUrids {
    position: u32,
    frame: u32,
    speed: u32,
    bar: u32,
    bar_beat: u32,
    beat: u32,
    beat_unit: u32,
    beats_per_bar: u32,
    beats_per_minute: u32,
}

impl Urids {
    fn new(features: &Features) -> Urids {
        let atom = |name: &CStr| features.urid(name);
        Urids {
            midi: features.midi_urid(),
            object: atom(c"http://lv2plug.in/ns/ext/atom#Object"),
            blank: atom(c"http://lv2plug.in/ns/ext/atom#Blank"),
            urid: atom(c"http://lv2plug.in/ns/ext/atom#URID"),
            float: atom(c"http://lv2plug.in/ns/ext/atom#Float"),
            double: atom(c"http://lv2plug.in/ns/ext/atom#Double"),
            int: atom(c"http://lv2plug.in/ns/ext/atom#Int"),
            long: atom(c"http://lv2plug.in/ns/ext/atom#Long"),
            bool: atom(c"http://lv2plug.in/ns/ext/atom#Bool"),
            get: atom(c"http://lv2plug.in/ns/ext/patch#Get"),
            set: atom(c"http://lv2plug.in/ns/ext/patch#Set"),
            put: atom(c"http://lv2plug.in/ns/ext/patch#Put"),
            property: atom(c"http://lv2plug.in/ns/ext/patch#property"),
            value: atom(c"http://lv2plug.in/ns/ext/patch#value"),
            body: atom(c"http://lv2plug.in/ns/ext/patch#body"),
            time: TimeUrids {
                position: atom(c"http://lv2plug.in/ns/ext/time#Position"),
                frame: atom(c"http://lv2plug.in/ns/ext/time#frame"),
                speed: atom(c"http://lv2plug.in/ns/ext/time#speed"),
                bar: atom(c"http://lv2plug.in/ns/ext/time#bar"),
                bar_beat: atom(c"http://lv2plug.in/ns/ext/time#barBeat"),
                beat: atom(c"http://lv2plug.in/ns/ext/time#beat"),
                beat_unit: atom(c"http://lv2plug.in/ns/ext/time#beatUnit"),
                beats_per_bar: atom(c"http://lv2plug.in/ns/ext/time#beatsPerBar"),
                beats_per_minute: atom(c"http://lv2plug.in/ns/ext/time#beatsPerMinute"),
            },
        }
    }

    /// The number in an atom's body, whatever numeric type it has.
    fn number(&self, kind: u32, body: &[u8]) -> Option<f64> {
        let four = || body.first_chunk::<4>().copied();
        let eight = || body.first_chunk::<8>().copied();
        let value = match kind {
            k if k == self.float => f64::from(f32::from_ne_bytes(four()?)),
            k if k == self.double => f64::from_ne_bytes(eight()?),
            k if k == self.int || k == self.bool => f64::from(i32::from_ne_bytes(four()?)),
            k if k == self.long => i64::from_ne_bytes(eight()?) as f64,
            _ => return None,
        };
        value.is_finite().then_some(value)
    }
}

/// The properties of an atom object's body (after its id and type): key, type and body of
/// each value. Stops at the first one that does not fit in `body`.
fn properties(body: &[u8]) -> impl Iterator<Item = (u32, u32, &[u8])> {
    let mut rest = body.get(8..).unwrap_or(&[]);
    std::iter::from_fn(move || {
        let word = |at: usize| Some(u32::from_ne_bytes(*rest.get(at..)?.first_chunk::<4>()?));
        let (key, size, kind) = (word(0)?, word(8)? as usize, word(12)?);
        let value = rest.get(16..16usize.checked_add(size)?)?;
        let next = (16 + size).next_multiple_of(8);
        rest = rest.get(next..).unwrap_or(&[]);
        Some((key, kind, value))
    })
}

/// The type of an atom object, from its body.
fn object_type(body: &[u8]) -> Option<u32> {
    Some(u32::from_ne_bytes(*body.get(4..)?.first_chunk::<4>()?))
}

/// What the two halves share.
struct Shared {
    gate: Gate,
    /// Only inside the gate.
    instance: UnsafeCell<livi::Instance>,
    /// The instance was given its feature, and it is dropped after the instance.
    midnam_update: Box<midnam::Update>,
    features: Arc<Features>,
    params: Vec<Param>,
    by_id: HashMap<u32, usize>,
    by_urid: HashMap<u32, usize>,
    /// Each parameter's value, 0..=1, as last set or as the plugin last said (f64 bits).
    values: Box<[AtomicU64]>,
    /// Whether that value is the plugin's: a control port's always is, a `patch` parameter's
    /// once it was set or the plugin spoke of it.
    known: Box<[AtomicBool]>,
    /// The next block asks the plugin for all its `patch` properties.
    ask_all: AtomicBool,
    latency: AtomicU32,
    /// A window is open: the audio thread keeps `control_out` and the ring to the window
    /// filled.
    ui_open: AtomicBool,
    /// Every control output: its port number and its value as of the last block (f32 bits).
    control_out: Box<[(u32, AtomicU32)]>,
    /// The plugin's `LV2_Handle` and its `extension_data`, for its window.
    handle: *mut c_void,
    extension_data: Option<ui::ExtensionData>,
    sample_rate: f64,
}

// SAFETY: `instance` is reached only inside the gate, by one thread at a time; everything else
// is read-only after `instantiate` or atomic.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    fn value(&self, at: usize) -> f64 {
        f64::from_bits(self.values[at].load(Relaxed))
    }

    fn set_value(&self, at: usize, normal: f64) {
        self.values[at].store(normal.to_bits(), Relaxed);
        self.known[at].store(true, Relaxed);
    }

    /// The plugin's MIDNAM document ([`midnam::document`]).
    ///
    /// # Safety
    /// Nothing else may be in the plugin: inside the gate, or before the audio thread has it.
    /// Or the plugin asked for it to be read with midnam's update ([`midnam::Update`]): the
    /// extension's own host (Ardour) reads it then on its window thread while `run` goes on,
    /// so a plugin that asks is made to be read so.
    unsafe fn midnam(&self) -> Option<String> {
        let interface = (self.extension_data?)(midnam::INTERFACE.as_ptr());
        // SAFETY: the plugin's own pointer for the interface and its own handle; the caller
        // keeps everything else out of the plugin.
        unsafe { midnam::document(interface, self.handle) }
    }

    fn now(&self, at: usize) -> ParamNow {
        let param = &self.params[at];
        let value = self.known[at].load(Relaxed).then(|| self.value(at));
        ParamNow {
            id: param.id,
            value,
            text: value.map(|value| param.text(value)),
        }
    }
}

fn instantiate(
    world: &World,
    plugin: &livi::Plugin,
    activation: Activation,
) -> Result<(Arc<Shared>, Lv2Processor, Box<Link>), String> {
    let features = world.features(activation.max_frames)?;
    let layout = layout(world, plugin, &features);
    let midnam_update = midnam::Update::new();
    // SAFETY: running a plugin's code is what this process is for; livi's own conditions (one
    // user of the instance at a time, buffers that outlive each `run`) are kept by the gate
    // and by `Lv2Processor::process`. The midnam feature lives in `Shared`, after the
    // instance.
    let instance = unsafe {
        plugin.instantiate_with_features(
            features.clone(),
            activation.sample_rate,
            &[midnam_update.feature()],
        )
    }
    .map_err(|e| format!("instantiate: {e:?}"))?;
    let counts = instance.port_counts();
    let frames = activation.max_frames;
    let handle = instance.raw().instance().handle().cast::<c_void>();
    let extension_data =
        (instance.raw().instance().descriptor()).and_then(|descriptor| descriptor.extension_data);
    let known = (layout.params.iter())
        .map(|param| AtomicBool::new(matches!(param.target, Target::Port(_))))
        .collect();
    let control_out = (layout.control_out_ports.iter())
        .map(|&port| (port, AtomicU32::new(0)))
        .collect();
    let (to_dsp, from_ui) = rtrb::RingBuffer::new(FROM_UI_BYTES);
    let (to_ui, from_dsp) = rtrb::RingBuffer::new(TO_UI_BYTES);
    let values: Box<[AtomicU64]> = layout
        .params
        .iter()
        .map(|param| {
            let plain = match param.target {
                Target::Port(index) => instance
                    .control_input(index)
                    .map_or(param.default, f64::from),
                Target::Patch { .. } => param.default,
            };
            AtomicU64::new(param.normal(plain).to_bits())
        })
        .collect();
    let by_id = (layout.params.iter().enumerate())
        .map(|(at, param)| (param.id, at))
        .collect();
    let by_urid = (layout.params.iter().enumerate())
        .filter_map(|(at, param)| match param.target {
            Target::Patch { urid, .. } => Some((urid, at)),
            Target::Port(_) => None,
        })
        .collect();
    let shared = Arc::new(Shared {
        gate: Gate::default(),
        instance: UnsafeCell::new(instance),
        midnam_update,
        params: layout.params,
        by_id,
        by_urid,
        values,
        known,
        ask_all: AtomicBool::new(layout.patch_in.is_some()),
        latency: AtomicU32::new(0),
        ui_open: AtomicBool::new(false),
        control_out,
        handle,
        extension_data,
        sample_rate: activation.sample_rate,
        features: features.clone(),
    });
    let link = Box::new(Link {
        shared: shared.clone(),
        urids: Urids::new(&features),
        event_transfer: features.urid(c"http://lv2plug.in/ns/ext/atom#eventTransfer"),
        symbols: layout.symbols,
        to_dsp: RefCell::new(to_dsp),
        from_dsp: RefCell::new(from_dsp),
        said: RefCell::new(Vec::new()),
        asked: Cell::new(None),
        shown: shared.values.iter().map(|_| Cell::new(0)).collect(),
        out_shown: shared.control_out.iter().map(|_| Cell::new(0)).collect(),
    });
    let sequence = |bytes: &usize| LV2AtomSequence::new(&features, *bytes);
    let processor = Lv2Processor {
        shared: shared.clone(),
        urids: Urids::new(&features),
        atom_in: layout.atom_in_bytes.iter().map(sequence).collect(),
        atom_out: layout.atom_out_bytes.iter().map(sequence).collect(),
        midi_in: layout.midi_in,
        patch_in: layout.patch_in,
        time_in: layout.time_in,
        tempo_ports: layout.tempo_ports,
        speed_ports: layout.speed_ports,
        freewheel_ports: layout.freewheel_ports,
        offline: activation.offline,
        latency_port: layout.latency_port,
        main_in: layout.main_in,
        outputs: Outputs::new(
            layout.output_sets,
            layout.heard_out,
            layout.instrument,
            activation.sample_rate,
        ),
        audio_inputs: counts.audio_inputs,
        cv_inputs: counts.cv_inputs,
        max_frames: frames,
        silence: vec![0.0; frames].into_boxed_slice(),
        mono: vec![0.0; frames].into_boxed_slice(),
        out: vec![0.0; frames * counts.audio_outputs].into_boxed_slice(),
        cv_out: vec![0.0; frames * counts.cv_outputs].into_boxed_slice(),
        sets: Vec::with_capacity(BLOCK_SETS),
        whole_blocks: [
            "coarseBlockLength",
            "fixedBlockLength",
            "powerOf2BlockLength",
        ]
        .iter()
        .any(|f| {
            plugin
                .raw()
                .has_feature(&world.uri(&format!("{BUF_SIZE}{f}")))
        }),
        moving: Vec::with_capacity(MOVING_PORTS),
        cuts: Vec::with_capacity(activation.max_frames / SHORTEST_PIECE + 2),
        quiet: false,
        atom_in_ports: layout.atom_in_ports,
        atom_out_ports: layout.atom_out_ports,
        from_ui,
        to_ui,
        from_ui_atom: vec![0; UI_ATOM_BYTES + 8].into_boxed_slice(),
    };
    Ok((shared, processor, link))
}

pub fn load(key: &PluginKey, bundle: &Path, activation: Activation) -> Result<Loaded, String> {
    load_as(key, bundle, activation, false)
}

/// The plugin's description names `lv2:isLive`, as a feature it wants or one it can use:
/// its sound depends on time passing as it plays.
fn says_live(world: &World, plugin: &lilv::plugin::Plugin) -> bool {
    plugin.has_feature(&world.uri(&format!("{LV2}isLive")))
}

/// As [`load`], and as if the plugin's window were open and had just sent the plugin a few
/// messages: the audio thread then does everything it does for a window.
#[cfg(test)]
pub fn load_with_window(
    key: &PluginKey,
    bundle: &Path,
    activation: Activation,
) -> Result<Loaded, String> {
    load_as(key, bundle, activation, true)
}

fn load_as(
    key: &PluginKey,
    bundle: &Path,
    activation: Activation,
    as_with_window: bool,
) -> Result<Loaded, String> {
    let lv2_world = world(bundle)?;
    let plugin = lv2_world
        .world
        .plugin_by_uri(key.id())
        .ok_or_else(|| format!("no plugin {} in the bundle", key.id()))?;
    let (shared, processor, link) = instantiate(&lv2_world, &plugin, activation)?;
    if as_with_window {
        shared.ui_open.store(true, Relaxed);
        if let Some(&port) = processor.atom_in_ports.first() {
            let mut atom = Vec::new();
            atom.extend_from_slice(&4u32.to_ne_bytes());
            atom.extend_from_slice(&link.urids.float.to_ne_bytes());
            atom.extend_from_slice(&1.0f32.to_ne_bytes());
            for _ in 0..8 {
                link.wrote_atom(port, &atom);
            }
        }
    }
    // SAFETY: the processor is not on the audio thread yet.
    let key_names = unsafe { shared.midnam() }.map(|document| midnam::key_names(&document));
    let (editor, window) = window(&lv2_world, plugin.raw());
    let plugin = LoadedPlugin {
        latency: 0,
        audio_inputs: processor.main_in.len() as u32,
        audio_outputs: processor.channels_out(),
        editor,
        live: says_live(&lv2_world, plugin.raw()),
        outputs: processor.outputs.list(),
        // Read from the controller once it is made (`crate::control`).
        key_names: None,
    };
    Ok(Loaded {
        plugin,
        controller: Box::new(Lv2Controller {
            ui: None,
            link,
            world: lv2_world,
            shared,
            told_latency: 0,
            plugin_uri: key.id().to_string(),
            window,
            key_names,
            renamed: false,
        }),
        processor: Box::new(processor),
    })
}

struct Lv2Processor {
    shared: Arc<Shared>,
    urids: Urids,
    atom_in: Vec<LV2AtomSequence>,
    atom_out: Vec<LV2AtomSequence>,
    midi_in: Option<usize>,
    patch_in: Option<usize>,
    time_in: Option<usize>,
    tempo_ports: Vec<PortIndex>,
    speed_ports: Vec<PortIndex>,
    freewheel_ports: Vec<PortIndex>,
    /// For an export: `freewheel_ports` are held at 1, and what the plugin gives its worker
    /// is done before the next piece of sound is asked for.
    offline: bool,
    latency_port: Option<PortIndex>,
    main_in: Vec<usize>,
    outputs: Outputs,
    audio_inputs: usize,
    cv_inputs: usize,
    max_frames: usize,
    silence: Box<[f32]>,
    /// The input of a plugin with one main audio input.
    mono: Box<[f32]>,
    /// Every audio output of the plugin, `max_frames` each.
    out: Box<[f32]>,
    cv_out: Box<[f32]>,
    /// The `patch:writable` parameters of the block being made: the parameter, and the event
    /// of the block that sets it last.
    sets: Vec<(usize, usize)>,
    /// The plugin wants blocks of its own length (`bufsz:coarseBlockLength` and the like):
    /// its block is never cut.
    whole_blocks: bool,
    /// The parameters of control inputs that have points in the block being made.
    moving: Vec<usize>,
    /// Where the pieces of the block being made start.
    cuts: Vec<usize>,
    /// Send "all sound off" with the next block.
    quiet: bool,
    atom_in_ports: Vec<u32>,
    atom_out_ports: Vec<u32>,
    /// Messages of the plugin's window, each as: port (u32), length (u32), the atom.
    from_ui: rtrb::Consumer<u8>,
    /// What the plugin sends out while a window is open, in the same form.
    to_ui: rtrb::Producer<u8>,
    /// One message of the window on its way into a port.
    from_ui_atom: Box<[u8]>,
}

impl Lv2Processor {
    /// The channels of the main output as meloet takes it: 1, one output heard on both
    /// sides; 2, a left and a right, folded from more outputs when more are heard.
    fn channels_out(&self) -> u32 {
        self.outputs.heard().len().min(2) as u32
    }

    fn midi(&mut self, frame: u32, data: &[u8; 3]) {
        if let Some(port) = self.midi_in {
            let _ =
                self.atom_in[port].push_midi_event::<3>(i64::from(frame), self.urids.midi, data);
        }
    }

    /// `patch:Set` of one property to one value.
    fn set(&mut self, frame: u32, urid: u32, range: Range, plain: f64) {
        let Some(port) = self.patch_in else {
            return;
        };
        let urids = &self.urids;
        let mut body = [0u8; 56];
        let mut at = 0;
        let mut word = |x: u32| {
            body[at..at + 4].copy_from_slice(&x.to_ne_bytes());
            at += 4;
        };
        // The object: id, type. Then two properties: key, context, size and type of the
        // value, the value, padded to 8 bytes.
        word(0);
        word(urids.set);
        word(urids.property);
        word(0);
        word(4);
        word(urids.urid);
        word(urid);
        word(0);
        word(urids.value);
        word(0);
        let len = match range {
            Range::Float => {
                word(4);
                word(urids.float);
                word((plain as f32).to_bits());
                at
            }
            Range::Int => {
                word(4);
                word(urids.int);
                word(plain.round() as i32 as u32);
                at
            }
            Range::Bool => {
                word(4);
                word(urids.bool);
                word(u32::from(plain >= 0.5));
                at
            }
            Range::Double => {
                word(8);
                word(urids.double);
                let bits = plain.to_bits().to_ne_bytes();
                body[at..at + 8].copy_from_slice(&bits);
                at + 8
            }
        };
        if let Ok(event) =
            LV2AtomEventBuilder::<56>::new(i64::from(frame), urids.object, &body[..len])
        {
            let _ = self.atom_in[port].push_event(&event);
        }
    }

    /// Where the song is at the first frame of the block: one `time:Position` at time 0 for a
    /// plugin with a port for it (a JUCE plugin forgets the position at every block and knows
    /// only what the block brings), and the tempo and speed for control ports that are the
    /// host's to fill.
    fn tell_time(&mut self, instance: &mut livi::Instance, transport: &Transport) {
        let time = SongTime::of(transport);

        for &port in &self.tempo_ports {
            instance.set_control_input(port, time.beats_per_minute);
        }
        for &port in &self.speed_ports {
            instance.set_control_input(port, time.speed);
        }
        if let Some(port) = self.time_in {
            let body = time.object(&self.urids);
            let event = LV2AtomEventBuilder::<POSITION_BYTES>::new_full(0, self.urids.object, body);
            let _ = self.atom_in[port].push_event(&event);
        }
    }

    /// An empty `patch:Get`: the plugin answers with every property it has.
    fn ask_all(&mut self) {
        let Some(port) = self.patch_in else {
            return;
        };
        let mut body = [0u8; 8];
        body[4..].copy_from_slice(&self.urids.get.to_ne_bytes());
        let event = LV2AtomEventBuilder::<8>::new_full(0, self.urids.object, body);
        let _ = self.atom_in[port].push_event(&event);
    }

    /// Puts the messages the plugin's window sent since the last block into their ports.
    fn hear_window(&mut self) {
        let word = |bytes: &[u8]| u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        while self.from_ui.slots() >= 8 {
            let mut head = [0u8; 8];
            if self.from_ui.pop_entire_slice(&mut head).is_err() {
                return;
            }
            let (port, len) = (word(&head), word(&head[4..]) as usize);
            let fits = (8..=self.from_ui_atom.len()).contains(&len);
            if !fits || (self.from_ui.pop_entire_slice(&mut self.from_ui_atom[..len])).is_err() {
                // The ring is out of step with its writer; what is in it is of no use.
                while self.from_ui.pop().is_ok() {}
                return;
            }
            let atom = &self.from_ui_atom[..len];
            let (size, kind) = (word(atom) as usize, word(&atom[4..]));
            let (Some(at), Some(body)) = (
                self.atom_in_ports.iter().position(|p| *p == port),
                atom.get(8..8 + size),
            ) else {
                continue;
            };
            if let Ok(event) = LV2AtomEventBuilder::<UI_ATOM_BYTES>::new(0, kind, body) {
                let _ = self.atom_in[at].push_event(&event);
            }
        }
    }

    /// Copies what the plugin's window shows of this block: the control outputs, and every
    /// message the plugin sent.
    fn tell_window(&mut self, instance: &livi::Instance) {
        for (port, value) in self.shared.control_out.iter() {
            if let Some(now) = instance.control_output(PortIndex(*port as usize)) {
                value.store(now.to_bits(), Relaxed);
            }
        }
        for (sequence, port) in self.atom_out.iter().zip(&self.atom_out_ports) {
            for event in sequence.iter() {
                let size = event.data.len() as u32;
                // A full ring loses the message: the window draws a little late.
                let Ok(chunk) = self.to_ui.write_chunk_uninit(16 + event.data.len()) else {
                    return;
                };
                let head = [*port, size + 8, size, event.event.body.type_];
                chunk.fill_from_iter(
                    (head.into_iter().flat_map(u32::to_ne_bytes)).chain(event.data.iter().copied()),
                );
            }
        }
    }

    /// One property the plugin says the value of.
    fn heard(shared: &Shared, urid: u32, plain: f64, sink: &mut super::EventSink) {
        if let Some(&at) = shared.by_urid.get(&urid) {
            let param = &shared.params[at];
            let normal = param.normal(plain);
            shared.set_value(at, normal);
            sink.push(FromPlugin::Param {
                id: param.id,
                value: normal,
            });
        }
    }

    /// Reads the `patch:Set` and `patch:Put` messages the plugin wrote in this block.
    fn hear(&self, sink: &mut super::EventSink) {
        let urids = &self.urids;
        for sequence in &self.atom_out {
            for event in sequence.iter() {
                let kind = event.event.body.type_;
                if kind != urids.object && kind != urids.blank {
                    continue;
                }
                let message = object_type(event.data);
                if message == Some(urids.set) {
                    let (mut property, mut value) = (None, None);
                    for (key, kind, body) in properties(event.data) {
                        if key == urids.property && kind == urids.urid {
                            property = body.first_chunk::<4>().map(|b| u32::from_ne_bytes(*b));
                        } else if key == urids.value {
                            value = urids.number(kind, body);
                        }
                    }
                    if let (Some(property), Some(value)) = (property, value) {
                        Self::heard(&self.shared, property, value, sink);
                    }
                } else if message == Some(urids.put) {
                    for (key, kind, body) in properties(event.data) {
                        if key != urids.body || (kind != urids.object && kind != urids.blank) {
                            continue;
                        }
                        for (property, kind, value) in properties(body) {
                            if let Some(value) = urids.number(kind, value) {
                                Self::heard(&self.shared, property, value, sink);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Processor for Lv2Processor {
    fn process(&mut self, block: &mut ProcessBlock<'_>) -> bool {
        let frames = block.outputs[0].len();
        let shared = Arc::clone(&self.shared);
        let shared = &*shared;
        if frames > self.max_frames || !shared.gate.enter() {
            // The main thread is taking or putting back the state: this block goes round the
            // plugin.
            for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
                output.copy_from_slice(input);
            }
            self.outputs.missed(frames);
            return false;
        }
        // SAFETY: inside the gate nothing else touches the instance.
        let instance = unsafe { &mut *shared.instance.get() };
        for sequence in &mut self.atom_in {
            sequence.clear();
        }
        self.tell_time(instance, block.transport);
        if self.offline {
            for &port in &self.freewheel_ports {
                instance.set_control_input(port, 1.0);
            }
        }
        if shared.ask_all.swap(false, Relaxed) {
            self.ask_all();
        }
        self.hear_window();
        if std::mem::take(&mut self.quiet) {
            // All sound off, all notes off.
            self.midi(0, &[0xb0, 120, 0]);
            self.midi(0, &[0xb0, 123, 0]);
        }
        // Of the changes to one `patch` parameter in this block, only the last is sent.
        self.sets.clear();
        self.moving.clear();
        for (at, event) in block.events.iter().enumerate() {
            let ToPlugin::Param { id, .. } = *event else {
                continue;
            };
            let Some(&param) = shared.by_id.get(&id) else {
                continue;
            };
            match shared.params[param].target {
                Target::Patch { .. } => match self.sets.iter().position(|(p, _)| *p == param) {
                    Some(set) => self.sets[set].1 = at,
                    None if self.sets.len() < self.sets.capacity() => self.sets.push((param, at)),
                    None => {}
                },
                Target::Port(_) => {
                    if !self.moving.contains(&param) && self.moving.len() < self.moving.capacity() {
                        self.moving.push(param);
                    }
                }
            }
        }
        // Cut where a control input's line bends, pieces of `SHORTEST_PIECE` at least. Of
        // points closer together than that, the last: a jump is a point with the old value
        // and one with the new a frame later.
        self.cuts.clear();
        self.cuts.push(0);
        if !self.whole_blocks {
            let moving = &self.moving;
            let bends = || {
                (block.events.iter())
                    .filter_map(|e| match *e {
                        ToPlugin::Param { frame, id, .. } => Some((frame as usize, id)),
                        ToPlugin::Midi { .. } => None,
                    })
                    .filter(|&(frame, id)| {
                        frame + SHORTEST_PIECE <= frames
                            && shared.by_id.get(&id).is_some_and(|p| moving.contains(p))
                    })
                    .map(|(frame, _)| frame)
            };
            loop {
                let from = *self.cuts.last().unwrap_or(&0) + SHORTEST_PIECE;
                let Some(first) = bends().filter(|&f| f >= from).min() else {
                    break;
                };
                let cut = (bends())
                    .filter(|&f| f >= first && f < first + SHORTEST_PIECE)
                    .max()
                    .unwrap_or(first);
                if self.cuts.len() == self.cuts.capacity() {
                    break;
                }
                self.cuts.push(cut);
            }
        }

        let [in_left, in_right] = block.inputs;
        let main_in = (self.main_in.first().copied(), self.main_in.get(1).copied());
        if main_in.1.is_none() {
            for (mono, (l, r)) in self.mono.iter_mut().zip(in_left.iter().zip(in_right)) {
                *mono = (l + r) * 0.5;
            }
        }
        let mut ran = true;
        for piece in 0..self.cuts.len() {
            let from = self.cuts[piece];
            let to = self.cuts.get(piece + 1).copied().unwrap_or(frames);
            if piece > 0 {
                for sequence in &mut self.atom_in {
                    sequence.clear();
                }
                let later = later(block.transport, from, shared.sample_rate);
                self.tell_time(instance, &later);
            }
            let inside = |frame: u32| {
                let frame = frame as usize;
                frame >= from && (frame < to || to == frames)
            };
            for (at, event) in block.events.iter().enumerate() {
                match *event {
                    ToPlugin::Midi { frame, data } if inside(frame) => {
                        self.midi(frame - from as u32, &data);
                    }
                    ToPlugin::Param { frame, id, value } if inside(frame) => {
                        let Some(&param) = shared.by_id.get(&id) else {
                            continue;
                        };
                        if let Target::Patch { urid, range } = shared.params[param].target
                            && self.sets.contains(&(param, at))
                        {
                            let plain = shared.params[param].plain(value);
                            self.set(frame - from as u32, urid, range, plain);
                            shared.set_value(param, value);
                        }
                    }
                    _ => {}
                }
            }
            // A control input with points: the value of its line at the middle of the piece.
            let middle = (from + to - 1) / 2;
            for &param in &self.moving {
                let Target::Port(index) = shared.params[param].target else {
                    continue;
                };
                let value = port_line(
                    block.events,
                    shared.params[param].id,
                    shared.value(param),
                    middle,
                );
                instance.set_control_input(index, shared.params[param].plain(value) as f32);
            }
            let (mono, silence) = (&self.mono[from..to], &self.silence[from..to]);
            let (in_left, in_right) = (&in_left[from..to], &in_right[from..to]);
            // Side chains and the inputs past the main two hear silence.
            let inputs = (0..self.audio_inputs).map(|channel| match main_in {
                (Some(only), None) if channel == only => mono,
                (Some(left), Some(_)) if channel == left => in_left,
                (_, Some(right)) if channel == right => in_right,
                _ => silence,
            });
            let cv_inputs = (0..self.cv_inputs).map(|_| silence);
            let outputs = self
                .out
                .chunks_exact_mut(self.max_frames)
                .map(|channel| &mut channel[from..to]);
            let cv_outputs = self
                .cv_out
                .chunks_exact_mut(self.max_frames)
                .map(|channel| &mut channel[from..to]);
            let ports = EmptyPortConnections::new()
                .with_audio_inputs(inputs)
                .with_audio_outputs(outputs)
                .with_atom_sequence_inputs(self.atom_in.iter())
                .with_atom_sequence_outputs(self.atom_out.iter_mut())
                .with_cv_inputs(cv_inputs)
                .with_cv_outputs(cv_outputs);
            // SAFETY: every buffer lives until `run` is over and is `to - from` long or
            // longer; the atom buffers were made with this instance's own features.
            let piece_ran = rtcheck::plugin(|| unsafe { instance.run(to - from, ports) }).is_ok();
            if self.offline {
                // The worker's own thread looks for work ten times a second: by the clock,
                // so the same export would not give the same sound twice. Here the work is
                // done at once, and the plugin has the answer with its next piece.
                rtcheck::plugin(|| shared.features.worker_manager().run_workers());
            }
            ran &= piece_ran;
            if piece_ran {
                self.hear(block.sink);
                if shared.ui_open.load(Relaxed) {
                    self.tell_window(instance);
                }
            }
        }
        // The control inputs end the block at their last point.
        for &param in &self.moving {
            let value = port_line(
                block.events,
                shared.params[param].id,
                shared.value(param),
                frames,
            );
            shared.set_value(param, value);
        }

        if ran && !self.outputs.heard().is_empty() {
            let [left, right] = &mut block.outputs;
            self.outputs.mix(&self.out, self.max_frames, left, right);
        } else {
            for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
                output.copy_from_slice(input);
            }
            self.outputs.missed(frames);
        }
        if ran
            && let Some(frames) = self.latency_port.and_then(|p| instance.control_output(p))
            && frames.is_finite()
            && frames >= 0.0
        {
            let frames = (frames.round() as u32).min(MAX_LATENCY_FRAMES);
            shared.latency.store(frames, Relaxed);
        }
        shared.gate.leave();
        true
    }

    fn reset(&mut self) {
        self.quiet = true;
        self.outputs.snap();
    }

    fn latency(&self) -> u32 {
        self.shared.latency.load(Relaxed)
    }

    fn outputs(&mut self) -> Option<&mut Outputs> {
        Some(&mut self.outputs)
    }
}

/// The value of a control input's line at `frame` of the block: straight from `before` (the
/// value it had, at frame −1) through the points of `id` in the block, flat after the last.
fn port_line(events: &[ToPlugin], id: u32, before: f64, frame: usize) -> f64 {
    let (mut from, mut value) = (-1.0f64, before);
    for event in events {
        let ToPlugin::Param {
            frame: at,
            id: of,
            value: to,
        } = *event
        else {
            continue;
        };
        if of != id {
            continue;
        }
        let at = f64::from(at);
        if at >= frame as f64 {
            if at <= from {
                return to;
            }
            return value + (to - value) * (frame as f64 - from) / (at - from);
        }
        (from, value) = (at, to);
    }
    value
}

/// The song `frames` frames after `transport`, for a later piece of a block. The engine cuts
/// its blocks at every beat, so no beat or bar starts inside one.
fn later(transport: &Transport, frames: usize, sample_rate: f64) -> Transport {
    let mut t = *transport;
    if t.flags & Transport::PLAYING != 0 && sample_rate > 0.0 {
        let seconds = frames as f64 / sample_rate;
        t.position_frames += frames as i64;
        t.position_quarters += seconds * t.tempo_bpm / 60.0;
    }
    t
}

/// What a plugin's window calls back into. It stays where it is for as long as the plugin:
/// the window keeps a pointer to it.
struct Link {
    shared: Arc<Shared>,
    urids: Urids,
    event_transfer: u32,
    symbols: HashMap<CString, u32>,
    to_dsp: RefCell<rtrb::Producer<u8>>,
    from_dsp: RefCell<rtrb::Consumer<u8>>,
    /// What the person did in the window since the main thread last asked.
    said: RefCell<Vec<Said>>,
    /// The size the window last asked for.
    asked: Cell<Option<(u32, u32)>>,
    /// What the window was last told of each parameter (as `Shared::values`) and of each
    /// control output (as `Shared::control_out`).
    shown: Box<[Cell<u64>]>,
    out_shown: Box<[Cell<u32>]>,
}

impl Link {
    /// The parameter a port of the plugin is, with its place.
    fn port_param(&self, port: u32) -> Option<(usize, &Param)> {
        let &at = self.shared.by_id.get(&port)?;
        let param = &self.shared.params[at];
        matches!(param.target, Target::Port(_)).then_some((at, param))
    }

    fn moved(&self, at: usize, normal: f64) {
        self.shared.set_value(at, normal);
        self.shown[at].set(normal.to_bits());
        let id = self.shared.params[at].id;
        (self.said.borrow_mut()).push(Said::ParamChanged(id, normal, super::now_us()));
    }

    /// The window wrote one atom to a port. A `patch:Set` of a parameter is a parameter
    /// change like any other; everything else goes to the plugin as it is.
    fn wrote_atom(&self, port: u32, atom: &[u8]) {
        let word = |at: usize| Some(u32::from_ne_bytes(*atom.get(at..)?.first_chunk::<4>()?));
        let (Some(size), Some(kind)) = (word(0), word(4)) else {
            return;
        };
        let Some(body) = atom.get(8..8 + size as usize) else {
            return;
        };
        let urids = &self.urids;
        if (kind == urids.object || kind == urids.blank) && object_type(body) == Some(urids.set) {
            let (mut property, mut value) = (None, None);
            for (key, kind, body) in properties(body) {
                if key == urids.property && kind == urids.urid {
                    property = body.first_chunk::<4>().map(|b| u32::from_ne_bytes(*b));
                } else if key == urids.value {
                    value = urids.number(kind, body);
                }
            }
            if let (Some(property), Some(plain)) = (property, value)
                && let Some(&at) = self.shared.by_urid.get(&property)
            {
                self.moved(at, self.shared.params[at].normal(plain));
                return;
            }
        }
        if body.len() > UI_ATOM_BYTES {
            return;
        }
        let mut frame = Vec::with_capacity(16 + body.len());
        frame.extend_from_slice(&port.to_ne_bytes());
        frame.extend_from_slice(&(size + 8).to_ne_bytes());
        frame.extend_from_slice(&atom[..8 + body.len()]);
        // A full ring loses the message.
        let _ = self.to_dsp.borrow_mut().push_entire_slice(&frame);
    }
}

/// `LV2UI_Write_Function`: `controller` is the plugin's [`Link`].
unsafe extern "C" fn ui_write(
    controller: *mut c_void,
    port: u32,
    size: u32,
    protocol: u32,
    buffer: *const c_void,
) {
    if controller.is_null() || buffer.is_null() {
        return;
    }
    // SAFETY: the handle given to the window is its plugin's `Link`, which outlives it; the
    // buffer has `size` bytes, as the window says.
    let (link, bytes) = unsafe {
        (
            &*controller.cast::<Link>(),
            std::slice::from_raw_parts(buffer.cast::<u8>(), size as usize),
        )
    };
    if protocol == 0 {
        if let (Ok(bytes), Some((at, param))) = (<[u8; 4]>::try_from(bytes), link.port_param(port))
        {
            let plain = f32::from_ne_bytes(bytes);
            if plain.is_finite() {
                link.moved(at, param.normal(f64::from(plain)));
            }
        }
    } else if protocol == link.event_transfer {
        link.wrote_atom(port, bytes);
    }
}

/// `LV2UI_Resize::ui_resize`.
unsafe extern "C" fn ui_resize(handle: *mut c_void, width: c_int, height: c_int) -> c_int {
    if handle.is_null() || width < 1 || height < 1 {
        return 1;
    }
    // SAFETY: as in `ui_write`.
    let link = unsafe { &*handle.cast::<Link>() };
    link.asked.set(Some((width as u32, height as u32)));
    0
}

/// `LV2UI_Touch::touch`.
unsafe extern "C" fn ui_touch(handle: *mut c_void, port: u32, grabbed: bool) {
    if handle.is_null() {
        return;
    }
    // SAFETY: as in `ui_write`.
    let link = unsafe { &*handle.cast::<Link>() };
    if let Some((_, param)) = link.port_param(port)
        && !param.hidden
    {
        let at = super::now_us();
        link.said.borrow_mut().push(if grabbed {
            Said::ParamBegin(param.id, at)
        } else {
            Said::ParamEnd(param.id, at)
        });
    }
}

/// `LV2UI_Port_Map::port_index`.
unsafe extern "C" fn ui_port_index(handle: *mut c_void, symbol: *const c_char) -> u32 {
    if handle.is_null() || symbol.is_null() {
        return ui::NO_PORT;
    }
    // SAFETY: as in `ui_write`; a symbol is a C string.
    let (link, symbol) = unsafe { (&*handle.cast::<Link>(), CStr::from_ptr(symbol)) };
    link.symbols.get(symbol).copied().unwrap_or(ui::NO_PORT)
}

struct Lv2Controller {
    // Before `link` and `shared`: the window calls into both until it is gone.
    ui: Option<ui::Ui>,
    link: Box<Link>,
    /// Keeps the bundle's description for the state calls.
    world: Rc<World>,
    shared: Arc<Shared>,
    told_latency: u32,
    plugin_uri: String,
    /// The window meloet can open, if the plugin has one.
    window: Option<UiFiles>,
    /// What its midnam names its keys, as of its load, the last state put back, or the last
    /// time it asked for them to be read again.
    key_names: Option<Vec<KeyName>>,
    /// `key_names` changed since the last [`Controller::poll`].
    renamed: bool,
}

impl Lv2Controller {
    /// The names of a document read now, told with the next poll when they are new.
    fn rename(&mut self, document: Option<String>) {
        let names = document.map(|document| midnam::key_names(&document));
        if names != self.key_names {
            self.key_names = names;
            self.renamed = true;
        }
    }

    /// Tells the window what changed in the plugin since it was last told.
    fn show(&self, ui: &ui::Ui) {
        let (shared, link) = (&self.shared, &self.link);
        for (at, param) in shared.params.iter().enumerate() {
            let Target::Port(index) = param.target else {
                continue;
            };
            let bits = shared.values[at].load(Relaxed);
            if link.shown[at].replace(bits) != bits {
                let plain = param.plain(f64::from_bits(bits)) as f32;
                ui.port_event(index.0 as u32, 0, &plain.to_ne_bytes());
            }
        }
        for ((port, value), shown) in shared.control_out.iter().zip(&link.out_shown) {
            let bits = value.load(Relaxed);
            if shown.replace(bits) != bits {
                ui.port_event(*port, 0, &bits.to_ne_bytes());
            }
        }
        let mut atom = Vec::new();
        loop {
            let port = {
                let mut from = link.from_dsp.borrow_mut();
                let mut head = [0u8; 8];
                if from.slots() < 8 || from.pop_entire_slice(&mut head).is_err() {
                    break;
                }
                let word = |at: usize| u32::from_ne_bytes([0, 1, 2, 3].map(|i| head[at + i]));
                atom.resize(word(4) as usize, 0);
                if from.pop_entire_slice(&mut atom).is_err() {
                    break;
                }
                word(0)
            };
            ui.port_event(port, link.event_transfer, &atom);
        }
    }
}

impl Controller for Lv2Controller {
    fn params(&mut self) -> Vec<ParamInfo> {
        let shared = &self.shared;
        (shared.params.iter().enumerate())
            .filter(|(_, param)| !param.hidden)
            .map(|(at, param)| {
                let now = shared.now(at);
                ParamInfo {
                    id: param.id,
                    name: param.name.clone(),
                    group: param.group.clone(),
                    default: param.normal(param.default),
                    shape: param.shape,
                    labels: param.labels(),
                    automatable: param.automatable,
                    midi_cc: super::named_midi_cc(&param.name),
                    value: now.value,
                    text: now.text,
                }
            })
            .collect()
    }

    fn param(&mut self, id: u32) -> Option<ParamNow> {
        let &at = self.shared.by_id.get(&id)?;
        Some(self.shared.now(at))
    }

    fn text(&mut self, id: u32, value: f64) -> Option<String> {
        let &at = self.shared.by_id.get(&id)?;
        Some(self.shared.params[at].text(value))
    }

    fn read_text(&mut self, id: u32, text: &str) -> Option<Read> {
        let &at = self.shared.by_id.get(&id)?;
        self.shared.params[at].read(text)
    }

    fn set_param(&mut self, id: u32, value: f64) {
        let Some(&at) = self.shared.by_id.get(&id) else {
            return;
        };
        self.shared.set_value(at, value);
        // A control port is set here too, so that a state taken before the next block has it.
        // A `patch` parameter can only be sent with a block.
        if let Target::Port(index) = self.shared.params[at].target
            && let Some(_claim) = self.shared.gate.claim(GATE_WAIT)
        {
            let plain = self.shared.params[at].plain(value);
            // SAFETY: inside the gate nothing else touches the instance.
            unsafe { &mut *self.shared.instance.get() }.set_control_input(index, plain as f32);
        }
    }

    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        let shared = &self.shared;
        let (state, kept) = {
            let _claim = (shared.gate.claim(GATE_WAIT)).ok_or("the plugin is busy")?;
            // SAFETY: inside the gate nothing else touches the instance.
            let instance = unsafe { &*shared.instance.get() };
            let state = instance.state().map_err(|e| format!("state: {e:?}"))?;
            // Only what is known: a value nobody knows is not made known by being saved.
            let kept: Vec<(u32, f64)> = (shared.params.iter().enumerate())
                .filter(|(at, param)| {
                    matches!(param.target, Target::Patch { .. }) && shared.known[*at].load(Relaxed)
                })
                .map(|(at, param)| (param.id, shared.value(at)))
                .collect();
            (state, kept)
        };
        let text = state
            .to_string(STATE_URI)
            .map_err(|e| format!("state: {e:?}"))?;
        let mut bytes = Vec::with_capacity(8 + kept.len() * 12 + text.len());
        bytes.extend_from_slice(STATE_MAGIC);
        bytes.extend_from_slice(&(kept.len() as u32).to_le_bytes());
        for (id, value) in kept {
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(text.as_bytes());
        Ok(bytes)
    }

    fn load_state(&mut self, state: &[u8]) -> Result<(), String> {
        let bad = || "not an LV2 state of meloet".to_string();
        let rest = state.strip_prefix(STATE_MAGIC).ok_or_else(bad)?;
        let (count, rest) = rest.split_first_chunk::<4>().ok_or_else(bad)?;
        let kept_len = (u32::from_le_bytes(*count) as usize)
            .checked_mul(12)
            .filter(|len| *len <= rest.len())
            .ok_or_else(bad)?;
        let (kept, text) = rest.split_at(kept_len);
        let text = std::str::from_utf8(text).map_err(|_| bad())?;
        let state = self
            .world
            .world
            .state_from_string(&self.shared.features, text)
            .map_err(|e| format!("state: {e:?}"))?;
        presets::applies(&state, &self.plugin_uri)
            .map_err(|_| "the state is not one of this plugin".to_string())?;
        let _claim = (self.shared.gate.claim(GATE_WAIT)).ok_or("the plugin is busy")?;
        // SAFETY: inside the gate nothing else touches the instance.
        let instance = unsafe { &mut *self.shared.instance.get() };
        instance
            .restore_state(&state)
            .map_err(|e| format!("state: {e:?}"))?;
        // SAFETY: inside the gate nothing else touches the instance.
        let document = unsafe { self.shared.midnam() };
        for (at, param) in self.shared.params.iter().enumerate() {
            match param.target {
                Target::Port(index) => {
                    if let Some(plain) = instance.control_input(index) {
                        self.shared.set_value(at, param.normal(f64::from(plain)));
                    }
                }
                // Whatever the state made it: not known until the state or the plugin says.
                Target::Patch { .. } => self.shared.known[at].store(false, Relaxed),
            }
        }
        for (id, value) in kept.as_chunks::<12>().0.iter().map(|one| {
            let (id, value) = one.split_at(4);
            (
                u32::from_le_bytes([id[0], id[1], id[2], id[3]]),
                f64::from_le_bytes(value.try_into().unwrap_or([0; 8])),
            )
        }) {
            if let Some(&at) = self.shared.by_id.get(&id)
                && matches!(self.shared.params[at].target, Target::Patch { .. })
                && value.is_finite()
            {
                self.shared.set_value(at, value.clamp(0.0, 1.0));
            }
        }
        // The `patch` parameters are whatever the state made them: ask, for the plugins that
        // answer.
        self.shared.ask_all.store(true, Relaxed);
        drop(_claim);
        self.rename(document);
        Ok(())
    }

    fn key_names(&mut self) -> Option<Vec<KeyName>> {
        self.key_names.clone()
    }

    fn presets(&mut self, bundles: &[PathBuf]) -> Result<Vec<PresetInfo>, String> {
        presets::list(&self.world, &self.plugin_uri, bundles)
    }

    fn preset_state(&mut self, uri: &str, bundles: &[PathBuf]) -> Result<Vec<u8>, String> {
        let features = &self.shared.features;
        presets::state(&self.world, features, &self.plugin_uri, uri, bundles)
    }

    fn save_preset(&mut self, dir: &Path, file: &str, label: &str) -> Result<PresetInfo, String> {
        presets::place_ok(dir, file)?;
        let state = {
            let _claim = (self.shared.gate.claim(GATE_WAIT)).ok_or("the plugin is busy")?;
            // SAFETY: inside the gate nothing else touches the instance.
            let instance = unsafe { &*self.shared.instance.get() };
            instance.state().map_err(|e| format!("state: {e:?}"))?
        };
        presets::save(&self.world, &self.plugin_uri, state, dir, file, label)
    }

    fn open_editor(&mut self, parent: u32) -> Result<Opened, String> {
        self.close_editor();
        let files = self.window.as_ref().ok_or("no window meloet can open")?;
        let (shared, link) = (&self.shared, &self.link);
        let features = &shared.features;
        let no_worker = lv2_raw::LV2Feature {
            uri: c"urn:meloet:none".as_ptr(),
            data: std::ptr::null_mut(),
        };
        let mut urid = features.iter_features(&no_worker).filter(|feature| {
            // SAFETY: a feature's URI is a C string.
            let uri = unsafe { CStr::from_ptr(feature.uri) }.to_bytes();
            uri == b"http://lv2plug.in/ns/ext/urid#map"
                || uri == b"http://lv2plug.in/ns/ext/urid#unmap"
        });
        let (Some(map), Some(unmap)) = (urid.next(), urid.next()) else {
            return Err("no URID map".into());
        };
        let plugin = ui::Plugin {
            uri: &self.plugin_uri,
            instance: shared.handle,
            extension_data: shared.extension_data,
            urid: [map, unmap],
            sample_rate: shared.sample_rate as f32,
            option_urids: [
                features.urid(c"http://lv2plug.in/ns/ext/parameters#sampleRate"),
                features.urid(c"http://lv2plug.in/ns/extensions/ui#updateRate"),
                features.urid(c"http://lv2plug.in/ns/ext/atom#Float"),
            ],
        };
        let callbacks = ui::Callbacks {
            handle: (&raw const **link).cast_mut().cast(),
            write: ui_write,
            resize: ui_resize,
            touch: ui_touch,
            port_index: ui_port_index,
        };
        link.asked.set(None);
        // What the plugin sent while no window was there is old.
        while link.from_dsp.borrow_mut().pop().is_ok() {}
        // SAFETY: the instance lives in `shared` and the callbacks' handle is `link`; both
        // are dropped after `ui`.
        let ui = unsafe {
            ui::Ui::open(
                &files.uri,
                &files.bundle,
                &files.binary,
                parent,
                &plugin,
                &callbacks,
            )
        }?;
        // The window starts from every port's value, whatever it was told the last time.
        for (at, shown) in link.shown.iter().enumerate() {
            shown.set(!shared.values[at].load(Relaxed));
        }
        for ((_, value), shown) in shared.control_out.iter().zip(&link.out_shown) {
            shown.set(!value.load(Relaxed));
        }
        shared.ui_open.store(true, Relaxed);
        self.show(&ui);
        self.ui = Some(ui);
        Ok(Opened {
            size: link.asked.take(),
            resizable: false,
        })
    }

    fn close_editor(&mut self) {
        self.shared.ui_open.store(false, Relaxed);
        self.ui = None;
        self.link.asked.set(None);
    }

    fn idle(&mut self) {
        if let Some(ui) = &self.ui {
            self.show(ui);
            if !ui.idle() {
                self.link.said.borrow_mut().push(Said::Closed);
            }
        }
    }

    fn poll(&mut self, said: &mut Vec<Said>) {
        let latency = self.shared.latency.load(Relaxed);
        if latency != self.told_latency {
            self.told_latency = latency;
            said.push(Said::Latency(latency));
        }
        said.append(&mut self.link.said.borrow_mut());
        if let Some((width, height)) = self.link.asked.take() {
            said.push(Said::Resized(width, height));
        }
        if self.shared.midnam_update.take() {
            // SAFETY: the plugin asked for it (`Shared::midnam`); the audio thread may be in
            // `run`, and nothing else is in the plugin.
            let document = unsafe { self.shared.midnam() };
            self.rename(document);
        }
        if std::mem::take(&mut self.renamed) {
            said.push(Said::KeyNames);
        }
    }
}

const STATE_URI: &str = "urn:meloet:state";
/// A state is: this, the number of `patch` parameters kept, for each its id (u32) and value
/// (f64, 0..=1), little endian, then the state as lilv writes it (Turtle).
const STATE_MAGIC: &[u8; 4] = b"ML2S";

#[cfg(test)]
mod time_tests {
    use super::*;

    fn urids() -> Urids {
        Urids {
            midi: 1,
            object: 2,
            blank: 3,
            urid: 4,
            float: 5,
            double: 6,
            int: 7,
            long: 8,
            bool: 9,
            get: 10,
            set: 11,
            put: 12,
            property: 13,
            value: 14,
            body: 15,
            time: TimeUrids {
                position: 20,
                frame: 21,
                speed: 22,
                bar: 23,
                bar_beat: 24,
                beat: 25,
                beat_unit: 26,
                beats_per_bar: 27,
                beats_per_minute: 28,
            },
        }
    }

    #[test]
    fn the_position_counts_quarter_notes_and_the_place_in_the_bar_the_meters_notes() {
        // 6/8 at 90 quarters a minute, two eighths into the fourth bar (bars from 0).
        let transport = Transport {
            flags: Transport::PLAYING,
            meter_num: 6,
            meter_den: 8,
            tempo_bpm: 90.0,
            position_quarters: 10.0,
            bar_start_quarters: 9.0,
            loop_start_quarters: 0.0,
            loop_end_quarters: 0.0,
            position_frames: 320_000,
            bar: 3,
        };
        let time = SongTime::of(&transport);
        let want = SongTime {
            frame: 320_000,
            speed: 1.0,
            bar: 3,
            bar_beat: 2.0,
            beat: 10.0,
            beat_unit: 8,
            beats_per_bar: 6.0,
            beats_per_minute: 90.0,
        };
        assert_eq!(time, want);

        // Stopped, and nothing told yet: 4/4 at 120, standing at the head.
        let nothing = SongTime::of(&Transport::default());
        assert_eq!((nothing.speed, nothing.beat_unit), (0.0, 4));
        assert_eq!(
            (nothing.beats_per_bar, nothing.beats_per_minute),
            (4.0, 120.0)
        );
    }

    #[test]
    fn the_position_object_has_every_field_with_its_type() {
        let urids = urids();
        let time = SongTime {
            frame: -7,
            speed: 1.0,
            bar: 3,
            bar_beat: 2.5,
            beat: 20.5,
            beat_unit: 8,
            beats_per_bar: 6.0,
            beats_per_minute: 180.0,
        };
        let body = time.object(&urids);
        assert_eq!(object_type(&body), Some(urids.time.position));
        let got: Vec<(u32, u32, Option<f64>, usize)> = properties(&body)
            .map(|(key, kind, value)| (key, kind, urids.number(kind, value), value.len()))
            .collect();
        assert_eq!(
            got,
            [
                (21, urids.long, Some(-7.0), 8),
                (22, urids.float, Some(1.0), 4),
                (23, urids.long, Some(3.0), 8),
                (24, urids.float, Some(2.5), 4),
                (25, urids.double, Some(20.5), 8),
                (26, urids.int, Some(8.0), 4),
                (27, urids.float, Some(6.0), 4),
                (28, urids.float, Some(180.0), 4),
            ]
        );
    }
}

#[cfg(test)]
mod piece_tests {
    use std::path::Path;

    use super::*;
    use crate::formats::{EventSink, ProcessBlock};

    fn param(frame: u32, value: f64) -> ToPlugin {
        ToPlugin::Param {
            frame,
            id: 0,
            value,
        }
    }

    #[test]
    fn a_control_input_follows_its_line_through_the_points() {
        let other = ToPlugin::Param {
            frame: 50,
            id: 7,
            value: 0.0,
        };
        let events = [param(99, 0.5), param(199, 1.0), other];
        assert_eq!(port_line(&events, 0, 0.0, 0), 0.005);
        assert_eq!(port_line(&events, 0, 0.0, 99), 0.5);
        assert_eq!(port_line(&events, 0, 0.0, 149), 0.75);
        assert_eq!(port_line(&events, 0, 0.0, 255), 1.0);
        assert_eq!(port_line(&events, 3, 0.25, 10), 0.25);
        // A jump: two points one frame apart.
        let jump = [param(127, 0.0), param(128, 1.0)];
        assert_eq!(port_line(&jump, 0, 0.0, 127), 0.0);
        assert_eq!(port_line(&jump, 0, 0.0, 128), 1.0);
    }

    /// `amp-swh` takes its gain at the head of each `run`: a jump inside the block is heard
    /// at its frame; a point closer than 32 frames to the last cut is not cut at.
    #[test]
    fn a_block_with_points_runs_in_pieces() {
        let bundle = Path::new("/usr/lib/lv2/amp-swh.lv2");
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return;
        }
        let key: PluginKey = "lv2:http://plugin.org.uk/swh-plugins/amp".parse().unwrap();
        let activation = Activation {
            sample_rate: 48_000.0,
            max_frames: 256,
            offline: false,
        };
        let mut loaded = load(&key, bundle, activation).unwrap();
        // −70 to +70 dB: 0.5 is 0 dB, 0.5 − 20/140 is −20 dB.
        let quiet = 0.5 - 20.0 / 140.0;
        let input = [0.5f32; 256];
        let mut run = |events: &[ToPlugin]| {
            let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
            let mut sink = EventSink::with_capacity(16);
            loaded.processor.process(&mut ProcessBlock {
                inputs: [&input, &input],
                outputs: [&mut left, &mut right],
                events,
                transport: &Transport::default(),
                sink: &mut sink,
            });
            left
        };
        run(&[param(0, quiet)]);
        let out = run(&[param(127, quiet), param(128, 0.5)]);
        assert!((out[127] - 0.05).abs() < 1e-3, "{}", out[127]);
        assert!((out[128] - 0.5).abs() < 1e-3, "{}", out[128]);
        // Points at 128 and 140 are closer than 32 frames: one cut, at the later. Each
        // piece has the value of the line at its middle frame (69: from 0 dB at frame −1
        // down to −20 dB at 128; 197: from −20 dB at 140 up to 0 dB at 255).
        let out = run(&[param(128, quiet), param(140, quiet), param(255, 0.5)]);
        let db = |out: f32| 20.0 * (out / 0.5).log10();
        assert!(
            (db(out[0]) - -20.0 * 70.0 / 129.0).abs() < 0.01,
            "{}",
            db(out[0])
        );
        assert!(
            (db(out[140]) - -20.0 * 58.0 / 115.0).abs() < 0.01,
            "{}",
            db(out[140])
        );
        assert!(out[139] == out[0] && out[255] == out[140]);
    }
    /// A bundle of descriptions only; nothing of it is ever loaded.
    #[test]
    fn a_plugin_that_names_is_live_is_told_as_live() {
        let bundle = std::env::temp_dir().join(format!("meloet-live-{}.lv2", std::process::id()));
        std::fs::create_dir_all(&bundle).unwrap();
        let manifest = "\
            @prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n\
            @prefix doap: <http://usefulinc.com/ns/doap#> .\n\
            <urn:meloet:test:can> a lv2:Plugin ; doap:name \"Can\" ; lv2:binary <x.so> ;\n\
                lv2:optionalFeature lv2:isLive .\n\
            <urn:meloet:test:must> a lv2:Plugin ; doap:name \"Must\" ; lv2:binary <x.so> ;\n\
                lv2:requiredFeature lv2:isLive .\n\
            <urn:meloet:test:not> a lv2:Plugin ; doap:name \"Not\" ; lv2:binary <x.so> ;\n\
                lv2:optionalFeature lv2:hardRTCapable .\n";
        std::fs::write(bundle.join("manifest.ttl"), manifest).unwrap();
        let lv2_world = world(&bundle).unwrap();
        let mut said = Vec::new();
        for plugin in lv2_world.world.raw().plugins().iter() {
            let uri = plugin.uri().as_uri().unwrap_or_default().to_string();
            said.push((uri, says_live(&lv2_world, &plugin)));
        }
        said.sort();
        let _ = std::fs::remove_dir_all(&bundle);
        assert_eq!(
            said,
            [
                ("urn:meloet:test:can".to_string(), true),
                ("urn:meloet:test:must".to_string(), true),
                ("urn:meloet:test:not".to_string(), false),
            ]
        );
    }

    #[test]
    #[ignore = "needs Surge XT (LV2)"]
    fn the_free_wheeling_port_is_held_at_one_for_an_export_only() {
        let bundle = Path::new("/usr/lib/lv2/Surge XT.lv2");
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return;
        }
        let lv2_world = world(bundle).unwrap();
        let plugin = (lv2_world.world)
            .plugin_by_uri("https://surge-synthesizer.github.io/lv2/surge-xt")
            .unwrap();
        for offline in [false, true] {
            let activation = Activation {
                sample_rate: 48_000.0,
                max_frames: 256,
                offline,
            };
            let (shared, mut processor, _link) =
                instantiate(&lv2_world, &plugin, activation).unwrap();
            assert_eq!(processor.freewheel_ports.len(), 1);
            let silence = [0.0f32; 256];
            let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
            let mut sink = EventSink::with_capacity(16);
            for _ in 0..3 {
                processor.process(&mut ProcessBlock {
                    inputs: [&silence, &silence],
                    outputs: [&mut left, &mut right],
                    events: &[],
                    transport: &Transport::default(),
                    sink: &mut sink,
                });
            }
            // SAFETY: nothing else has the instance; the processor is not running.
            let instance = unsafe { &*shared.instance.get() };
            let port = instance.control_input(processor.freewheel_ports[0]);
            assert_eq!(port, Some(if offline { 1.0 } else { 0.0 }), "{offline}");
        }
    }
}

#[cfg(test)]
mod output_tests {
    use std::path::Path;

    use super::Side::{Both, Left, Right};
    use super::*;
    use meloet_plugin::wire::{OutputSet, OutputSetting, OutputSettings};

    use crate::formats::outputs::{Capture, fold, heard_by_default};
    use crate::formats::{EventSink, ProcessBlock};

    fn port(name: &str) -> AudioPort {
        AudioPort {
            name: name.to_string(),
            symbol: name.to_lowercase().replace(' ', "_"),
            ..AudioPort::default()
        }
    }

    fn in_group(name: &str, group: &str, designated: Option<Side>) -> AudioPort {
        AudioPort {
            group: Some(format!("urn:test#{group}")),
            group_name: Some(group.to_string()),
            designated,
            ..port(name)
        }
    }

    fn optional(port: AudioPort) -> AudioPort {
        AudioPort {
            optional: true,
            ..port
        }
    }

    fn side_chain(port: AudioPort) -> AudioPort {
        AudioPort {
            side_chain: true,
            ..port
        }
    }

    #[test]
    fn every_output_of_a_kit_that_names_no_main_output_is_heard() {
        // DrumGizmo: sixteen outputs, in no group, with no designation.
        let drumgizmo: Vec<AudioPort> = (1..=16).map(|n| port(&format!("Out{n}"))).collect();
        let all: Vec<(usize, Side)> = (0..16).map(|at| (at, Both)).collect();
        assert_eq!(heard_outputs(&drumgizmo, true), all);

        // avldrums' Black Pearl Multi: five drums alone, then two groups of a pair.
        let mut multi = ["Kick", "Snare", "HiHat", "Tom", "Floor Tom"]
            .map(port)
            .to_vec();
        multi.extend([
            in_group("Cymbals L", "Cymbals", Some(Left)),
            in_group("Cymbals R", "Cymbals", Some(Right)),
            in_group("Percussions L", "Percussions", Some(Left)),
            in_group("Percussions R", "Percussions", Some(Right)),
        ]);
        assert_eq!(
            heard_outputs(&multi, true),
            [
                (0, Both),
                (1, Both),
                (2, Both),
                (3, Both),
                (4, Both),
                (5, Left),
                (6, Right),
                (7, Left),
                (8, Right),
            ]
        );
    }

    #[test]
    fn an_instrument_with_a_main_group_is_heard_from_that_group_only() {
        // Surge XT: groups 2 and 3 are copies of the scenes, and optional.
        let surge = [
            in_group("Audio Out 1", "output_group_1", Some(Left)),
            in_group("Audio Out 2", "output_group_1", Some(Right)),
            optional(in_group("Audio Out 3", "output_group_2", Some(Left))),
            optional(in_group("Audio Out 4", "output_group_2", Some(Right))),
            optional(in_group("Audio Out 5", "output_group_3", Some(Left))),
            optional(in_group("Audio Out 6", "output_group_3", Some(Right))),
        ];
        assert_eq!(heard_outputs(&surge, true), [(0, Left), (1, Right)]);

        // LSP Multisampler x12 DO: the stereo output first, then twelve direct outputs of a
        // group each, none optional.
        let mut lsp = vec![
            in_group("Output L", "stereo_out", Some(Left)),
            in_group("Output R", "stereo_out", Some(Right)),
        ];
        for n in 0..12 {
            let group = format!("direct_out_{n}");
            lsp.push(in_group(
                &format!("Direct output left {n}"),
                &group,
                Some(Left),
            ));
            lsp.push(in_group(
                &format!("Direct output right {n}"),
                &group,
                Some(Right),
            ));
        }
        assert_eq!(heard_outputs(&lsp, true), [(0, Left), (1, Right)]);

        // A main group of more than two is heard whole.
        let three = [
            in_group("Front L", "main", Some(Left)),
            in_group("Front R", "main", Some(Right)),
            in_group("Centre", "main", None),
            in_group("Aux L", "aux", Some(Left)),
        ];
        assert_eq!(
            heard_outputs(&three, true),
            [(0, Left), (1, Right), (2, Both)]
        );
    }

    #[test]
    fn one_or_two_outputs_are_heard_as_they_always_were() {
        for instrument in [true, false] {
            assert_eq!(heard_outputs(&[port("Out")], instrument), [(0, Both)]);
            let pair = [port("Out 1"), port("Out 2")];
            assert_eq!(heard_outputs(&pair, instrument), [(0, Left), (1, Right)]);
            // Neither names nor designations turn a pair round.
            let turned = [
                in_group("Right", "main", Some(Right)),
                in_group("Left", "main", Some(Left)),
            ];
            assert_eq!(heard_outputs(&turned, instrument), [(0, Left), (1, Right)]);
            assert_eq!(heard_outputs(&[], instrument), []);
        }
    }

    #[test]
    fn an_effect_is_heard_from_its_first_two_outputs_only() {
        let six: Vec<AudioPort> = (1..=6).map(|n| port(&format!("Out{n}"))).collect();
        assert_eq!(heard_outputs(&six, false), [(0, Left), (1, Right)]);
        let grouped = [
            in_group("A", "main", None),
            in_group("B", "main", None),
            in_group("C", "main", None),
        ];
        assert_eq!(heard_outputs(&grouped, false), [(0, Left), (1, Right)]);
    }

    #[test]
    fn a_side_chain_output_is_never_heard() {
        let ports = [
            side_chain(port("Key")),
            port("Out1"),
            side_chain(port("Key 2")),
            port("Out2"),
            port("Out3"),
        ];
        assert_eq!(
            heard_outputs(&ports, true),
            [(1, Both), (3, Both), (4, Both)]
        );
        assert_eq!(heard_outputs(&ports, false), [(1, Left), (3, Right)]);
        assert_eq!(heard_outputs(&[side_chain(port("Key"))], true), []);
    }

    #[test]
    fn an_instrument_with_no_main_output_leaves_out_only_optional_groups() {
        let ports = [
            port("Out1"),
            optional(port("Out2")),
            optional(in_group("Extra L", "extra", Some(Left))),
            in_group("Room L", "room", Some(Left)),
            in_group("Room R", "room", Some(Right)),
        ];
        assert_eq!(
            heard_outputs(&ports, true),
            [(0, Both), (1, Both), (3, Left), (4, Right)]
        );
    }

    #[test]
    fn past_two_outputs_the_last_word_of_a_name_says_the_side() {
        let names = [
            "Out L",
            "out_right",
            "OUT-r",
            "Left",
            "l",
            "Mid",
            "OutL",
            "Left 1",
            "",
        ];
        let ports = names.map(port);
        let sides: Vec<Side> = (heard_outputs(&ports, true).into_iter())
            .map(|(_, side)| side)
            .collect();
        assert_eq!(
            sides,
            [Left, Right, Right, Left, Left, Both, Both, Both, Both]
        );
        // A designation comes before the name.
        let ports = [
            AudioPort {
                designated: Some(Right),
                ..port("Out L")
            },
            port("Out L"),
            port("Out 3"),
        ];
        assert_eq!(
            heard_outputs(&ports, true),
            [(0, Right), (1, Left), (2, Both)]
        );
    }

    /// The sets of `ports` as (id, name, ports, on, pan), after checking that by default they
    /// are heard as stage 1 heard them.
    fn sets_of(ports: &[AudioPort], instrument: bool) -> Vec<(String, String, Ports, bool, f32)> {
        let heard = heard_outputs(ports, instrument);
        let sets = output_sets(ports, &heard);
        assert_eq!(heard_by_default(&sets), heard);
        let mut seen: Vec<usize> = Vec::new();
        for set in &sets {
            match set.ports {
                Ports::Mono(at) => seen.push(at),
                Ports::Stereo { left, right } => seen.extend([left, right]),
            }
        }
        seen.sort();
        let ours: Vec<usize> = (0..ports.len())
            .filter(|&at| !ports[at].side_chain)
            .collect();
        assert_eq!(
            seen, ours,
            "every output that is not a side chain in one set"
        );
        (sets.into_iter())
            .map(|set| (set.id, set.name, set.ports, set.on, set.pan))
            .collect()
    }

    fn stereo(left: usize, right: usize) -> Ports {
        Ports::Stereo { left, right }
    }

    fn mono_sets(ids: &[&str], on: bool, pan: f32) -> Vec<(String, String, Ports, bool, f32)> {
        (ids.iter().enumerate())
            .map(|(at, id)| {
                let name = id.to_string();
                (
                    id.to_lowercase().replace(' ', "_"),
                    name,
                    Ports::Mono(at),
                    on,
                    pan,
                )
            })
            .collect()
    }

    #[test]
    fn the_outputs_of_known_plugins_are_in_sets() {
        // DrumGizmo: sixteen sets of one, all heard in the middle.
        let names: Vec<String> = (1..=16).map(|n| format!("Out{n}")).collect();
        let drumgizmo: Vec<AudioPort> = names.iter().map(|name| port(name)).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        assert_eq!(sets_of(&drumgizmo, true), mono_sets(&names, true, 0.0));

        // avldrums' Black Pearl Multi: five drums alone and two pairs.
        let mut multi = ["Kick", "Snare", "HiHat", "Tom", "Floor Tom"]
            .map(port)
            .to_vec();
        multi.extend([
            in_group("Cymbals L", "Cymbals", Some(Left)),
            in_group("Cymbals R", "Cymbals", Some(Right)),
            in_group("Percussions L", "Percussions", Some(Left)),
            in_group("Percussions R", "Percussions", Some(Right)),
        ]);
        let mut want = mono_sets(&["Kick", "Snare", "HiHat", "Tom", "Floor Tom"], true, 0.0);
        want.push((
            "cymbals_l".into(),
            "Cymbals".into(),
            stereo(5, 6),
            true,
            0.0,
        ));
        want.push((
            "percussions_l".into(),
            "Percussions".into(),
            stereo(7, 8),
            true,
            0.0,
        ));
        assert_eq!(sets_of(&multi, true), want);

        // Surge XT: the main pair heard, the scenes' pairs not.
        let surge = [
            in_group("Audio Out 1", "Output", Some(Left)),
            in_group("Audio Out 2", "Output", Some(Right)),
            optional(in_group("Audio Out 3", "Scene A", Some(Left))),
            optional(in_group("Audio Out 4", "Scene A", Some(Right))),
            optional(in_group("Audio Out 5", "Scene B", Some(Left))),
            optional(in_group("Audio Out 6", "Scene B", Some(Right))),
        ];
        let pair = |id: &str, name: &str, left, on| {
            (
                id.to_string(),
                name.to_string(),
                stereo(left, left + 1),
                on,
                0.0,
            )
        };
        assert_eq!(
            sets_of(&surge, true),
            [
                pair("audio_out_1", "Output", 0, true),
                pair("audio_out_3", "Scene A", 2, false),
                pair("audio_out_5", "Scene B", 4, false),
            ]
        );
    }

    #[test]
    fn one_or_two_outputs_are_one_set() {
        for instrument in [true, false] {
            assert_eq!(
                sets_of(&[port("Out")], instrument),
                mono_sets(&["Out"], true, 0.0)
            );
            let pair = [port("Out 1"), port("Out 2")];
            assert_eq!(
                sets_of(&pair, instrument),
                [("out_1".into(), "Out 1".into(), stereo(0, 1), true, 0.0)]
            );
            let grouped = [
                in_group("Out L", "Main", Some(Left)),
                in_group("Out R", "Main", Some(Right)),
            ];
            assert_eq!(
                sets_of(&grouped, instrument),
                [("out_l".into(), "Main".into(), stereo(0, 1), true, 0.0)]
            );
            assert_eq!(sets_of(&[], instrument), []);
        }
    }

    #[test]
    fn a_pair_that_stage_1_does_not_hear_as_a_pair_is_two_sets_of_one() {
        // The two outputs turned round: stage 1 heard the first on the left.
        let turned = [
            in_group("Right", "Main", Some(Right)),
            in_group("Left", "Main", Some(Left)),
        ];
        let mut want = mono_sets(&["Right", "Left"], true, 0.0);
        (want[0].4, want[1].4) = (-1.0, 1.0);
        assert_eq!(sets_of(&turned, true), want);
        // One of the pair heard, the other not (optional in a group).
        let half = [
            port("Out1"),
            optional(in_group("Extra L", "Extra", Some(Left))),
            in_group("Extra R", "Extra", Some(Right)),
        ];
        let got = sets_of(&half, true);
        assert_eq!(got.len(), 3);
        assert_eq!(
            (got[1].2, got[1].3, got[1].4),
            (Ports::Mono(1), false, -1.0)
        );
        assert_eq!((got[2].2, got[2].3, got[2].4), (Ports::Mono(2), true, 1.0));
        // A pair heard on both sides each, past two outputs.
        let both = [
            port("Out1"),
            in_group("A", "Room", None),
            in_group("B", "Room", None),
        ];
        assert_eq!(
            sets_of(&both, true),
            mono_sets(&["Out1", "A", "B"], true, 0.0)
        );
        // Of an effect, a pair past the first two, not heard.
        let effect = [
            in_group("In L", "Main", Some(Left)),
            in_group("In R", "Main", Some(Right)),
            in_group("Send L", "Send", Some(Left)),
            in_group("Send R", "Send", Some(Right)),
        ];
        let got = sets_of(&effect, false);
        assert_eq!(
            got[0],
            ("in_l".into(), "Main".into(), stereo(0, 1), true, 0.0)
        );
        assert_eq!(
            got[1],
            ("send_l".into(), "Send".into(), stereo(2, 3), false, 0.0)
        );
    }

    #[test]
    fn outputs_past_a_pair_are_sets_of_one() {
        // An effect with six outputs and no groups: the first two heard, left and right.
        let six: Vec<AudioPort> = ["Out 1", "Out 2", "Out L", "Out R", "Out 5", "Out 6"]
            .map(port)
            .to_vec();
        let got = sets_of(&six, false);
        let placed: Vec<(bool, f32)> = got.iter().map(|set| (set.3, set.4)).collect();
        assert_eq!(
            placed,
            [
                (true, -1.0),
                (true, 1.0),
                (false, -1.0),
                (false, 1.0),
                (false, 0.0),
                (false, 0.0)
            ]
        );
        // A group of three: three sets.
        let three = [
            in_group("Front L", "main", Some(Left)),
            in_group("Front R", "main", Some(Right)),
            in_group("Centre", "main", None),
        ];
        let got = sets_of(&three, true);
        let placed: Vec<(Ports, f32)> = got.iter().map(|set| (set.2, set.4)).collect();
        assert_eq!(
            placed,
            [
                (Ports::Mono(0), -1.0),
                (Ports::Mono(1), 1.0),
                (Ports::Mono(2), 0.0)
            ]
        );
        // Side chains are in no set.
        let ports = [
            side_chain(port("Key")),
            port("Out1"),
            side_chain(port("Key 2")),
            port("Out2"),
        ];
        let got = sets_of(&ports, true);
        assert_eq!(
            got,
            [("out1".into(), "Out1".into(), stereo(1, 3), true, 0.0)]
        );
    }

    /// Each output eight frames long, all of it `value`.
    fn outputs(values: &[f32]) -> Vec<f32> {
        values.iter().flat_map(|&value| [value; 8]).collect()
    }

    #[test]
    fn outputs_past_the_second_are_in_the_stereo_pair() {
        let out = outputs(&[1.0, 2.0, 4.0, 8.0, 16.0]);
        let heard = [(0, Both), (1, Left), (2, Right), (3, Both), (4, Left)];
        let (mut left, mut right) = ([f32::NAN; 5], [f32::NAN; 5]);
        fold(&heard, &out, 8, &mut left, &mut right);
        assert_eq!(left, [1.0 + 2.0 + 8.0 + 16.0; 5]);
        assert_eq!(right, [1.0 + 4.0 + 8.0; 5]);
    }

    #[test]
    fn one_or_two_outputs_are_copied_bit_for_bit() {
        let a = [
            -0.0f32,
            1.0e-40,
            f32::from_bits(0x7fc0_1234),
            f32::INFINITY,
            0.25,
        ];
        let b = [
            1.0e-42f32,
            -0.0,
            -1.0,
            f32::from_bits(0xffc0_0001),
            f32::NEG_INFINITY,
        ];
        let mut out = vec![9.0f32; 16];
        out[..5].copy_from_slice(&a);
        out[8..13].copy_from_slice(&b);
        let bits = |x: &[f32]| x.iter().map(|x| x.to_bits()).collect::<Vec<u32>>();
        let (mut left, mut right) = ([0.0f32; 5], [0.0f32; 5]);
        fold(&[(0, Left), (1, Right)], &out, 8, &mut left, &mut right);
        assert_eq!((bits(&left), bits(&right)), (bits(&a), bits(&b)));
        fold(&[(1, Both)], &out, 8, &mut left, &mut right);
        assert_eq!((bits(&left), bits(&right)), (bits(&b), bits(&b)));
    }

    #[test]
    fn what_is_not_a_number_does_not_reach_the_pair() {
        let mut out = outputs(&[1.0, 2.0, 4.0]);
        out[8 + 1] = f32::NAN;
        out[16 + 3] = f32::INFINITY;
        let heard = [(0, Both), (1, Left), (2, Both)];
        let (mut left, mut right) = ([0.0f32; 5], [0.0f32; 5]);
        fold(&heard, &out, 8, &mut left, &mut right);
        assert_eq!(left, [7.0, 0.0, 7.0, 0.0, 7.0]);
        assert_eq!(right, [5.0, 5.0, 5.0, 0.0, 5.0]);
    }

    fn activation(offline: bool) -> Activation {
        Activation {
            sample_rate: 48_000.0,
            max_frames: 256,
            offline,
        }
    }

    /// The outputs a real plugin is heard from when it is loaded, and the channels it says
    /// it has; `None` when it is not installed.
    fn heard_of(bundle: &str, uri: &str) -> Option<(Vec<(usize, Side)>, u32)> {
        let bundle = Path::new(bundle);
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return None;
        }
        let lv2_world = world(bundle).unwrap();
        let plugin = lv2_world.world.plugin_by_uri(uri).unwrap();
        let (_shared, processor, _link) =
            instantiate(&lv2_world, &plugin, activation(false)).unwrap();
        Some((processor.outputs.heard().to_vec(), processor.channels_out()))
    }

    #[test]
    #[ignore = "needs DrumGizmo, avldrums, Surge XT and LSP (LV2)"]
    fn real_plugins_are_heard_from_the_outputs_their_descriptions_name() {
        if let Some(got) = heard_of("/usr/lib/lv2/drumgizmo.lv2", "http://drumgizmo.org/lv2") {
            let all: Vec<(usize, Side)> = (0..16).map(|at| (at, Both)).collect();
            assert_eq!(got, (all, 2));
        }
        if let Some((heard, channels)) = heard_of(
            "/usr/lib/lv2/avldrums.lv2",
            "http://gareus.org/oss/lv2/avldrums#BlackPearlMulti",
        ) {
            let sides: Vec<Side> = heard.iter().map(|&(_, side)| side).collect();
            assert_eq!(
                sides,
                [Both, Both, Both, Both, Both, Left, Right, Left, Right]
            );
            assert_eq!(channels, 2);
        }
        let pair = Some((vec![(0, Left), (1, Right)], 2));
        let surge = heard_of(
            "/usr/lib/lv2/Surge XT.lv2",
            "https://surge-synthesizer.github.io/lv2/surge-xt",
        );
        assert!(surge.is_none() || surge == pair, "{surge:?}");
        let lsp = heard_of(
            "/usr/lib/lv2/lsp-plugins.lv2",
            "http://lsp-plug.in/plugins/lv2/multisampler_x12_do",
        );
        assert!(lsp.is_none() || lsp == pair, "{lsp:?}");
    }

    /// Black Pearl Multi plays its cymbals out of outputs 6 and 7 and its percussion out of 8
    /// and 9, and loads its kit through its worker. Plays `keys` once, its output sets set as
    /// `settings` say: the energy of its first two outputs, the stereo pair (left and right in
    /// turn), and each set as captured.
    fn play_black_pearl(
        offline: bool,
        keys: &[u8],
        settings: &OutputSettings,
    ) -> Option<(f64, Vec<f32>, Capture)> {
        let bundle = Path::new("/usr/lib/lv2/avldrums.lv2");
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return None;
        }
        let lv2_world = world(bundle).unwrap();
        let plugin = (lv2_world.world)
            .plugin_by_uri("http://gareus.org/oss/lv2/avldrums#BlackPearlMulti")
            .unwrap();
        let (shared, mut processor, _link) =
            instantiate(&lv2_world, &plugin, activation(offline)).unwrap();
        processor.outputs.set(settings, false);
        let on: Vec<ToPlugin> = (keys.iter())
            .map(|&key| ToPlugin::Midi {
                frame: 0,
                data: [0x99, key, 100],
            })
            .collect();
        const WARM: usize = 8;
        const BLOCKS: usize = 200;
        let silence = [0.0f32; 256];
        let mut sink = EventSink::with_capacity(16);
        let (mut first_two, mut pair) = (0.0f64, Vec::new());
        let before = rtcheck::count();
        for block in 0..WARM + BLOCKS {
            if block == WARM {
                let capture = Capture::new(processor.outputs.ids(), 256 * BLOCKS as u64);
                processor.outputs.arm(capture.unwrap());
            }
            let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
            {
                let _audio = rtcheck::enter();
                processor.process(&mut ProcessBlock {
                    inputs: [&silence, &silence],
                    outputs: [&mut left, &mut right],
                    events: if block == WARM { &on } else { &[] },
                    transport: &Transport::default(),
                    sink: &mut sink,
                });
            }
            if !offline {
                // What the worker's own thread does ten times a second, here between blocks,
                // so that the kit is there by the same block in both ways.
                shared.features.worker_manager().run_workers();
            }
            if block >= WARM {
                first_two += energy(&processor.out[..512]);
                pair.extend(left.iter().zip(&right).flat_map(|(&l, &r)| [l, r]));
            }
        }
        assert_eq!(rtcheck::count(), before, "allocated on the audio thread");
        let capture = processor.outputs.take_capture().unwrap();
        Some((first_two, pair, capture))
    }

    fn energy(samples: &[f32]) -> f64 {
        (samples.iter()).map(|&x| f64::from(x) * f64::from(x)).sum()
    }

    #[test]
    #[ignore = "needs avldrums (LV2)"]
    fn every_output_of_a_kit_is_heard_alike_as_it_plays_and_in_an_export() {
        let defaults = OutputSettings::default();
        // Crash 1 alone: nothing of it comes out of the first two outputs (kick and snare).
        let Some((first_two, pair, _)) = play_black_pearl(false, &[49], &defaults) else {
            return;
        };
        eprintln!(
            "crash: first two outputs {first_two:e}, the pair {:e}",
            energy(&pair)
        );
        assert_eq!(first_two, 0.0);
        assert!(energy(&pair) > 1.0, "{}", energy(&pair));

        // Kick, snare, closed hat, crash and tambourine.
        let keys = [36, 38, 42, 49, 54];
        let (first_two, playing, _) = play_black_pearl(false, &keys, &defaults).unwrap();
        let (_, export, _) = play_black_pearl(true, &keys, &defaults).unwrap();
        eprintln!(
            "kit: first two outputs {first_two:e}, the pair {:e}",
            energy(&playing)
        );
        assert!(energy(&playing) > 2.0 * first_two);
        let bits = |x: &[f32]| x.iter().map(|x| x.to_bits()).collect::<Vec<u32>>();
        assert!(bits(&playing) == bits(&export), "playing and export differ");
    }

    fn setting(id: &str, gain_db: f32, pan: Option<f32>, on: Option<bool>) -> OutputSetting {
        OutputSetting {
            id: id.into(),
            gain_db,
            pan,
            on,
        }
    }

    fn settings(sets: Vec<OutputSetting>) -> OutputSettings {
        OutputSettings { rule: 1, sets }
    }

    /// Only the set `id` on, at 0 dB where the rule puts it.
    fn only(id: &str, ids: &[String]) -> OutputSettings {
        let off = |other: &String| setting(other, 0.0, None, Some(other == id));
        settings(ids.iter().map(off).collect())
    }

    /// The largest difference between `a` and `b + c`, frame by frame.
    fn off_by(a: &[f32], b: &[f32], c: &[f32]) -> f32 {
        assert_eq!((a.len(), b.len()), (c.len(), c.len()));
        (a.iter().zip(b).zip(c)).fold(0.0, |most, ((a, b), c)| most.max((a - (b + c)).abs()))
    }

    /// What the sets the default rule hears give together.
    fn heard_sets(sounds: &[(String, Vec<f32>)], on: &[OutputSet], but: Option<&str>) -> Vec<f32> {
        let mut sum = vec![0.0f32; sounds[0].1.len()];
        for ((id, sound), set) in sounds.iter().zip(on) {
            if set.on && Some(id.as_str()) != but {
                for (sum, x) in sum.iter_mut().zip(sound) {
                    *sum += x;
                }
            }
        }
        sum
    }

    /// The sets of a plugin by its description, as `LoadedPlugin::outputs` tells them.
    fn sets_of_plugin(bundle: &str, uri: &str) -> Option<Vec<OutputSet>> {
        let bundle = Path::new(bundle);
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return None;
        }
        let key: PluginKey = format!("lv2:{uri}").parse().unwrap();
        let loaded = load(&key, bundle, activation(false)).unwrap();
        let sets = loaded.plugin.outputs;
        eprintln!("{uri}:");
        for set in &sets {
            eprintln!(
                "  {:?} {:?} channels {} on {} pan {}",
                set.id, set.name, set.channels, set.on, set.pan
            );
        }
        Some(sets)
    }

    #[test]
    #[ignore = "needs DrumGizmo, avldrums, Surge XT and LSP (LV2)"]
    fn real_plugins_tell_their_output_sets() {
        if let Some(sets) = sets_of_plugin("/usr/lib/lv2/drumgizmo.lv2", "http://drumgizmo.org/lv2")
        {
            assert_eq!(sets.len(), 16);
            assert!(sets.iter().all(|s| s.channels == 1 && s.on && s.pan == 0.0));
        }
        if let Some(sets) = sets_of_plugin(
            "/usr/lib/lv2/avldrums.lv2",
            "http://gareus.org/oss/lv2/avldrums#BlackPearlMulti",
        ) {
            let ids: Vec<&str> = sets.iter().map(|s| s.id.as_str()).collect();
            assert_eq!(
                ids,
                ["out1", "out2", "out3", "out4", "out5", "out6", "out8"]
            );
            assert!(sets.iter().all(|s| s.on && s.pan == 0.0));
        }
        if let Some(sets) = sets_of_plugin(
            "/usr/lib/lv2/Surge XT.lv2",
            "https://surge-synthesizer.github.io/lv2/surge-xt",
        ) {
            let set = |id: &str, name: &str, on| OutputSet {
                id: id.into(),
                name: name.into(),
                channels: 2,
                on,
                pan: 0.0,
            };
            assert_eq!(
                sets,
                [
                    set("audio_out_1", "Output", true),
                    set("audio_out_3", "Scene A", false),
                    set("audio_out_5", "Scene B", false),
                ]
            );
        }
        if let Some(sets) = sets_of_plugin(
            "/usr/lib/lv2/lsp-plugins.lv2",
            "http://lsp-plug.in/plugins/lv2/multisampler_x12_do",
        ) {
            assert_eq!(sets.len(), 13);
            assert!(sets.iter().all(|s| s.channels == 2 && s.pan == 0.0));
            assert!(sets[0].on && sets[1..].iter().all(|s| !s.on));
        }
    }

    #[test]
    #[ignore = "needs avldrums (LV2)"]
    fn a_set_of_a_kit_switched_off_takes_out_its_own_sound_only() {
        let keys = [36, 38, 42, 49, 54];
        let Some((_, all_on, captured)) = play_black_pearl(false, &keys, &Default::default())
        else {
            return;
        };
        let sounds = captured.sounds();
        let key: PluginKey = "lv2:http://gareus.org/oss/lv2/avldrums#BlackPearlMulti"
            .parse()
            .unwrap();
        let loaded = load(
            &key,
            Path::new("/usr/lib/lv2/avldrums.lv2"),
            activation(false),
        );
        let sets = loaded.unwrap().plugin.outputs;
        // The pair as it always was is every set heard, each as captured.
        let none = vec![0.0f32; all_on.len()];
        assert!(off_by(&all_on, &heard_sets(&sounds, &sets, None), &none) < 1e-6);
        // The snare alone, and the cymbals (a pair).
        for id in ["out2", "out6"] {
            let off = settings(vec![setting(id, 0.0, None, Some(false))]);
            let (_, without, again) = play_black_pearl(false, &keys, &off).unwrap();
            assert_eq!(again.sounds(), sounds, "the plugin played otherwise");
            let own = &sounds.iter().find(|(set, _)| set == id).unwrap().1;
            let others = heard_sets(&sounds, &sets, Some(id));
            let most = off_by(&all_on, &without, own);
            eprintln!(
                "{id}: own sound {:e}, all on − one off − own: {most:e}, the others: {:e}",
                energy(own),
                off_by(&without, &others, &none)
            );
            assert!(energy(own) > 0.1, "{id}: {}", energy(own));
            assert!(most < 1e-6, "{id}: {most}");
            assert!(off_by(&without, &others, &none) < 1e-6, "{id}");
        }
    }

    #[test]
    #[ignore = "needs avldrums (LV2)"]
    fn a_set_heard_alone_is_its_capture_and_an_export_sounds_the_same() {
        let keys = [36, 38, 42, 49, 54];
        let Some((_, _, captured)) = play_black_pearl(false, &keys, &Default::default()) else {
            return;
        };
        let ids: Vec<String> = captured.sounds().into_iter().map(|(id, _)| id).collect();
        for id in ["out2", "out6", "out8"] {
            let (_, alone, captured) = play_black_pearl(false, &keys, &only(id, &ids)).unwrap();
            let own = &captured
                .sounds()
                .into_iter()
                .find(|(set, _)| set == id)
                .unwrap()
                .1;
            assert!(energy(own) > 0.0, "{id}");
            // Equal as numbers: the bits differ at most in the sign of a zero.
            assert!(alone == *own, "{id}");
        }
        let changed = settings(vec![
            setting("out1", -6.0, Some(0.5), None),
            setting("out6", 3.0, Some(-0.25), None),
            setting("out8", 0.0, None, Some(false)),
        ]);
        let (_, playing, _) = play_black_pearl(false, &keys, &changed).unwrap();
        let (_, export, _) = play_black_pearl(true, &keys, &changed).unwrap();
        let (_, before, _) = play_black_pearl(false, &keys, &Default::default()).unwrap();
        let bits = |x: &[f32]| x.iter().map(|x| x.to_bits()).collect::<Vec<u32>>();
        assert!(bits(&playing) == bits(&export), "playing and export differ");
        assert!(
            bits(&playing) != bits(&before),
            "the settings changed nothing"
        );
    }

    fn base64(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut text = String::new();
        for three in bytes.chunks(3) {
            let n = (u32::from(three[0]) << 16)
                | (u32::from(*three.get(1).unwrap_or(&0)) << 8)
                | u32::from(*three.get(2).unwrap_or(&0));
            for at in 0..4 {
                if at <= three.len() {
                    text.push(DIGITS[(n >> (18 - 6 * at) & 63) as usize] as char);
                } else {
                    text.push('=');
                }
            }
        }
        text
    }

    /// A kit of four channels, instrument `n` alone on channel `n` and played by key 35 + `n`,
    /// each a short tone of its own: its drumkit file and its MIDI map.
    fn tiny_kit(dir: &Path) -> (PathBuf, PathBuf) {
        std::fs::create_dir_all(dir).unwrap();
        let (mut channels, mut instruments, mut keys) =
            (String::new(), String::new(), String::new());
        for n in 1..=4 {
            let frames = 4800u32;
            let mut wav = Vec::new();
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&(36 + frames * 2).to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt ");
            for (value, bytes) in [
                (16u32, 4),
                (1, 2),
                (1, 2),
                (48_000, 4),
                (96_000, 4),
                (2, 2),
                (16, 2),
            ] {
                wav.extend_from_slice(&value.to_le_bytes()[..bytes]);
            }
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&(frames * 2).to_le_bytes());
            for i in 0..frames {
                let phase = std::f32::consts::TAU * 200.0 * n as f32 * i as f32 / 48_000.0;
                let sample = (phase.sin() * 16_000.0 * (1.0 - i as f32 / frames as f32)) as i16;
                wav.extend_from_slice(&sample.to_le_bytes());
            }
            std::fs::write(dir.join(format!("i{n}.wav")), wav).unwrap();
            std::fs::write(
                dir.join(format!("i{n}.xml")),
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                     <instrument version=\"2.0\" name=\"I{n}\"><samples>\
                     <sample name=\"s{n}\" power=\"1\">\
                     <audiofile channel=\"Ch{n}\" file=\"i{n}.wav\" filechannel=\"1\"/>\
                     </sample></samples></instrument>\n"
                ),
            )
            .unwrap();
            channels += &format!("<channel name=\"Ch{n}\"/>");
            instruments += &format!(
                "<instrument name=\"I{n}\" file=\"i{n}.xml\">\
                 <channelmap in=\"Ch{n}\" out=\"Ch{n}\" main=\"true\"/></instrument>"
            );
            keys += &format!("<map note=\"{}\" instr=\"I{n}\"/>", 35 + n);
        }
        let kit = dir.join("drumkit.xml");
        std::fs::write(
            &kit,
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <drumkit name=\"Tiny\" version=\"2.0\" samplerate=\"48000\">\
                 <channels>{channels}</channels><instruments>{instruments}</instruments>\
                 </drumkit>\n"
            ),
        )
        .unwrap();
        let map = dir.join("midimap.xml");
        std::fs::write(
            &map,
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<midimap>{keys}</midimap>\n"),
        )
        .unwrap();
        (kit, map)
    }

    /// One block of 256 frames with `events`: left and right.
    fn block256(processor: &mut dyn Processor, events: &[ToPlugin]) -> ([f32; 256], [f32; 256]) {
        let silence = [0.0f32; 256];
        let mut sink = EventSink::with_capacity(16);
        let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
        processor.process(&mut ProcessBlock {
            inputs: [&silence, &silence],
            outputs: [&mut left, &mut right],
            events,
            transport: &Transport::default(),
            sink: &mut sink,
        });
        (left, right)
    }

    /// DrumGizmo with the tiny kit of `dir` ([`tiny_kit`]), loaded and heard, and silent
    /// again; nothing of its playing is left to chance. `None` when it is not installed.
    fn drumgizmo(dir: &Path, offline: bool) -> Option<Loaded> {
        let bundle = Path::new("/usr/lib/lv2/drumgizmo.lv2");
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return None;
        }
        let (kit, map) = tiny_kit(dir);
        let key: PluginKey = "lv2:http://drumgizmo.org/lv2".parse().unwrap();
        let mut loaded = load(&key, bundle, activation(offline)).unwrap();
        let state = kit_state(&mut loaded, &kit, &map);
        loaded.controller.load_state(&state).unwrap();

        let third = [ToPlugin::Midi {
            frame: 0,
            data: [0x90, 38, 127],
        }];
        let mut loudest = 0.0f32;
        // A key on the third channel every 50 blocks, until the kit has loaded and it is
        // heard; ten seconds at most.
        for block in 0..2000 {
            let events: &[ToPlugin] = if block % 50 == 0 { &third } else { &[] };
            let (left, right) = block256(&mut *loaded.processor, events);
            loudest = (left.iter().chain(&right)).fold(loudest, |m, x| m.max(x.abs()));
            if loudest > 0.01 {
                eprintln!("heard at block {block}: {loudest}");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(loudest > 0.01, "{loudest}");
        // Each tone of the kit is a tenth of a second long.
        for _ in 0..40 {
            block256(&mut *loaded.processor, &[]);
        }
        Some(loaded)
    }

    /// A state of DrumGizmo that names the kit `kit` and its MIDI map `map`, made from the
    /// state of `loaded`.
    fn kit_state(loaded: &mut Loaded, kit: &Path, map: &Path) -> Vec<u8> {
        let saved = loaded.controller.save_state().unwrap();
        // No `patch` parameters: the Turtle starts right after the count.
        let text = std::str::from_utf8(&saved[8..]).unwrap();
        let (head, rest) = text.split_once("atom#config> \"\"\"").unwrap();
        let (_, tail) = rest.split_once("\"\"\"").unwrap();
        let config = format!(
            "<config version=\"1.0\">\n\
             <value name=\"drumkitfile\">{}</value>\n\
             <value name=\"midimapfile\">{}</value>\n\
             <value name=\"enable_velocity_modifier\">false</value>\n\
             <value name=\"enable_velocity_randomiser\">false</value>\n\
             <value name=\"enable_latency_modifier\">false</value>\n</config>",
            kit.display(),
            map.display()
        );
        let mut state = STATE_MAGIC.to_vec();
        state.extend_from_slice(&0u32.to_le_bytes());
        state.extend_from_slice(
            format!(
                "{head}atom#config> \"\"\"{}\"\"\"{tail}",
                base64(config.as_bytes())
            )
            .as_bytes(),
        );
        state
    }

    /// DrumGizmo names no keys while it has no kit. The kit named in a state is read on a
    /// thread of its own, after the state is put back; then the plugin says from `run` that
    /// its names changed (midnam's update), and they are read again: the instruments of the
    /// kit's MIDI map, on their keys.
    #[test]
    #[ignore = "needs DrumGizmo (LV2)"]
    fn drumgizmo_names_the_keys_of_its_kit_once_it_has_read_it() {
        let bundle = Path::new("/usr/lib/lv2/drumgizmo.lv2");
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return;
        }
        let dir = std::env::temp_dir().join(format!("meloet-kit-names-{}", std::process::id()));
        let (kit, map) = tiny_kit(&dir);
        let key: PluginKey = "lv2:http://drumgizmo.org/lv2".parse().unwrap();
        let mut loaded = load(&key, bundle, activation(false)).unwrap();
        assert_eq!(loaded.controller.key_names(), Some(Vec::new()));
        let state = kit_state(&mut loaded, &kit, &map);
        loaded.controller.load_state(&state).unwrap();
        let mut said = Vec::new();
        loaded.controller.poll(&mut said);
        assert_eq!(loaded.controller.key_names(), Some(Vec::new()));
        assert!(!said.contains(&Said::KeyNames), "{said:?}");
        let mut asked_at = None;
        for block in 0..2000 {
            block256(&mut *loaded.processor, &[]);
            said.clear();
            loaded.controller.poll(&mut said);
            if said.contains(&Said::KeyNames) {
                asked_at = Some(block);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = std::fs::remove_dir_all(&dir);
        eprintln!("named after block {asked_at:?}");
        assert!(
            asked_at.is_some(),
            "the plugin never said its names changed"
        );
        let names: Vec<(u8, String)> = (loaded.controller.key_names().unwrap().into_iter())
            .map(|n| (n.key, n.name))
            .collect();
        let wanted: Vec<(u8, String)> = (1..=4).map(|n| (35 + n, format!("I{n}"))).collect();
        assert_eq!(names, wanted);
    }

    /// DrumGizmo plays each channel of a kit out of its own output; the kit is named in the
    /// state, and loads on a thread of its own.
    #[test]
    #[ignore = "needs DrumGizmo (LV2)"]
    fn the_third_channel_of_a_drumgizmo_kit_is_heard() {
        let dir = std::env::temp_dir().join(format!("meloet-kit-{}", std::process::id()));
        let loaded = drumgizmo(&dir, false);
        let _ = std::fs::remove_dir_all(&dir);
        drop(loaded);
    }

    /// The four keys of the tiny kit at once, then silence: `BLOCKS` blocks, with each set
    /// captured. The pair (left and right in turn) and the capture.
    fn hit_tiny_kit(loaded: &mut Loaded, settings: &OutputSettings) -> (Vec<f32>, Capture) {
        const BLOCKS: usize = 30;
        let keys: Vec<ToPlugin> = (36..=39)
            .map(|key| ToPlugin::Midi {
                frame: 0,
                data: [0x90, key, 127],
            })
            .collect();
        let outputs = loaded.processor.outputs().unwrap();
        outputs.set(settings, false);
        let ids = outputs.ids();
        outputs.arm(Capture::new(ids, 256 * BLOCKS as u64).unwrap());
        let mut pair = Vec::new();
        for block in 0..BLOCKS {
            let events: &[ToPlugin] = if block == 0 { &keys } else { &[] };
            let (left, right) = block256(&mut *loaded.processor, events);
            pair.extend(left.iter().zip(&right).flat_map(|(&l, &r)| [l, r]));
        }
        let capture = loaded.processor.outputs().unwrap().take_capture().unwrap();
        (pair, capture)
    }

    #[test]
    #[ignore = "needs DrumGizmo (LV2)"]
    fn a_set_of_drumgizmo_switched_off_takes_out_its_own_sound_only() {
        let dir = std::env::temp_dir().join(format!("meloet-kit-sets-{}", std::process::id()));
        let Some(mut loaded) = drumgizmo(&dir, false) else {
            return;
        };
        let sets = loaded.plugin.outputs.clone();
        let ids: Vec<String> = sets.iter().map(|set| set.id.clone()).collect();
        let (all_on, captured) = hit_tiny_kit(&mut loaded, &OutputSettings::default());
        let sounds = captured.sounds();
        let none = vec![0.0f32; all_on.len()];
        assert!(off_by(&all_on, &heard_sets(&sounds, &sets, None), &none) < 1e-6);
        let off = settings(vec![setting("out3", 0.0, None, Some(false))]);
        let (without, again) = hit_tiny_kit(&mut loaded, &off);
        let alone = only("out2", &ids);
        let (second_alone, by_itself) = hit_tiny_kit(&mut loaded, &alone);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(again.sounds(), sounds, "the plugin played otherwise");
        let own = |sounds: &[(String, Vec<f32>)], id: &str| {
            (sounds.iter().find(|(set, _)| set == id).unwrap().1).clone()
        };
        let most = off_by(&all_on, &without, &own(&sounds, "out3"));
        let others = off_by(&without, &heard_sets(&sounds, &sets, Some("out3")), &none);
        eprintln!(
            "out3: own sound {:e}, all on − one off − own: {most:e}, the others: {others:e}",
            energy(&own(&sounds, "out3"))
        );
        assert!(energy(&own(&sounds, "out3")) > 1.0);
        assert!(most < 1e-6 && others < 1e-6);
        // A set heard alone at 0 dB where the rule puts it is what was captured of it.
        let second = own(&by_itself.sounds(), "out2");
        assert!(energy(&second) > 1.0);
        assert!(second_alone == second);
    }

    #[test]
    #[ignore = "needs DrumGizmo (LV2)"]
    fn drumgizmo_sounds_the_same_as_it_plays_and_in_an_export() {
        let changed = settings(vec![
            setting("out1", -6.0, Some(-0.5), None),
            setting("out2", 0.0, None, Some(false)),
            setting("out4", 4.0, Some(0.75), None),
        ]);
        let mut pairs = Vec::new();
        for offline in [false, true] {
            let dir =
                std::env::temp_dir().join(format!("meloet-kit-{offline}-{}", std::process::id()));
            let Some(mut loaded) = drumgizmo(&dir, offline) else {
                return;
            };
            pairs.push(hit_tiny_kit(&mut loaded, &changed).0);
            let _ = std::fs::remove_dir_all(&dir);
        }
        assert!(energy(&pairs[0]) > 1.0);
        let bits = |x: &[f32]| x.iter().map(|x| x.to_bits()).collect::<Vec<u32>>();
        assert!(
            bits(&pairs[0]) == bits(&pairs[1]),
            "playing and export differ"
        );
    }

    #[test]
    #[ignore = "needs Surge XT (LV2)"]
    fn a_scene_of_surge_xt_switched_on_is_heard_too() {
        let bundle = Path::new("/usr/lib/lv2/Surge XT.lv2");
        if !bundle.exists() {
            eprintln!("{bundle:?} is not installed; nothing tested");
            return;
        }
        let key: PluginKey = "lv2:https://surge-synthesizer.github.io/lv2/surge-xt"
            .parse()
            .unwrap();
        let mut loaded = load(&key, bundle, activation(false)).unwrap();
        let sets = loaded.plugin.outputs.clone();
        const BLOCKS: usize = 40;
        let outputs = loaded.processor.outputs().unwrap();
        outputs.set(
            &settings(vec![setting("audio_out_3", 0.0, None, Some(true))]),
            false,
        );
        outputs.arm(Capture::new(outputs.ids(), 256 * BLOCKS as u64).unwrap());
        let note = [ToPlugin::Midi {
            frame: 0,
            data: [0x90, 60, 100],
        }];
        let mut pair = Vec::new();
        for block in 0..BLOCKS {
            let events: &[ToPlugin] = if block == 0 { &note } else { &[] };
            let (left, right) = block256(&mut *loaded.processor, events);
            pair.extend(left.iter().zip(&right).flat_map(|(&l, &r)| [l, r]));
        }
        let sounds = (loaded.processor.outputs().unwrap().take_capture())
            .unwrap()
            .sounds();
        let main = &sounds[0].1;
        let scene = &sounds[1].1;
        eprintln!(
            "Surge XT: main {:e}, scene A {:e}, scene B {:e}, the pair {:e}",
            energy(main),
            energy(scene),
            energy(&sounds[2].1),
            energy(&pair)
        );
        assert_eq!(sets.len(), 3);
        assert!(energy(main) > 1.0 && energy(scene) > 1.0);
        // The pair is the main output and scene A.
        assert!(off_by(&pair, main, scene) < 1e-6);
        assert!(off_by(&pair, main, &vec![0.0; main.len()]) > 0.01);
    }

    /// Every LV2 plugin of this PC that meloet can run: by default its sets are heard as stage
    /// 1 heard its outputs, each output in one set.
    #[test]
    #[ignore = "reads every LV2 bundle of this PC"]
    fn the_sets_of_every_installed_plugin_are_heard_by_default_as_before() {
        let mut dirs = vec![
            PathBuf::from("/usr/lib/lv2"),
            PathBuf::from("/usr/local/lib/lv2"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(Path::new(&home).join(".lv2"));
        }
        let (mut plugins, mut with_sets, mut apart) = (0, 0, Vec::new());
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut bundles: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            bundles.sort();
            for bundle in bundles {
                if !bundle.join("manifest.ttl").is_file() {
                    continue;
                }
                let lv2_world = World {
                    world: livi::World::with_load_bundle(&bundle_uri(&bundle).unwrap()),
                    features: RefCell::new(HashMap::new()),
                    preset_bundles: RefCell::new(HashSet::new()),
                };
                for plugin in lv2_world.world.iter_plugins() {
                    let ports = audio_ports(&lv2_world, &plugin, PortType::AudioOutput);
                    let instrument =
                        plugin_kind(&lv2_world, &plugin).role() == PluginRole::Instrument;
                    let heard = heard_outputs(&ports, instrument);
                    let sets = output_sets(&ports, &heard);
                    assert_eq!(heard_by_default(&sets), heard, "{}", plugin.uri());
                    let mut seen: Vec<usize> = (sets.iter())
                        .flat_map(|set| match set.ports {
                            Ports::Mono(at) => vec![at],
                            Ports::Stereo { left, right } => vec![left, right],
                        })
                        .collect();
                    seen.sort();
                    let ours: Vec<usize> = (0..ports.len())
                        .filter(|&at| !ports[at].side_chain)
                        .collect();
                    assert_eq!(seen, ours, "{}", plugin.uri());
                    let mut ids: Vec<&str> = sets.iter().map(|set| set.id.as_str()).collect();
                    ids.sort();
                    ids.dedup();
                    assert_eq!(ids.len(), sets.len(), "{}", plugin.uri());
                    // Groups of two that became two sets of one.
                    let mut groups: Vec<&str> = ours
                        .iter()
                        .filter_map(|&at| ports[at].group.as_deref())
                        .collect();
                    groups.sort();
                    let twos = (groups.chunk_by(|a, b| a == b))
                        .filter(|group| group.len() == 2)
                        .count();
                    let paired = (sets.iter())
                        .filter(|set| {
                            matches!(set.ports, Ports::Stereo { left, .. } if ports[left].group.is_some())
                        })
                        .count();
                    if paired < twos {
                        apart.push(plugin.uri());
                    }
                    plugins += 1;
                    if sets.len() > 1 {
                        with_sets += 1;
                    }
                }
            }
        }
        eprintln!(
            "{plugins} plugins, {with_sets} of them with more than one set; \
             a pair as two sets of one: {apart:?}"
        );
        assert!(plugins > 0);
    }
}
