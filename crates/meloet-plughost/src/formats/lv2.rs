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

mod ui;

use std::cell::{Cell, RefCell, UnsafeCell};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

use livi::event::{LV2AtomEventBuilder, LV2AtomSequence};
use livi::lilv;
use livi::{EmptyPortConnections, Features, FeaturesBuilder, PortIndex, PortType};
use meloet_plugin::ipc::{FromPlugin, MAX_LATENCY_FRAMES, ToPlugin};
use meloet_plugin::wire::{LoadedPlugin, ParamInfo, ParamNow, ParamShape};
use meloet_plugin::{
    EditorKind, InstrumentCategory, PluginInfo, PluginKey, PluginKind, PluginRole,
};

use super::{
    Activation, Candidate, Controller, GATE_WAIT, Gate, Loaded, Opened, ProcessBlock, Processor,
    Read, Said, leading_number,
};
use crate::rtcheck;

const LV2: &str = "http://lv2plug.in/ns/lv2core#";
const ATOM: &str = "http://lv2plug.in/ns/ext/atom#";
const PATCH: &str = "http://lv2plug.in/ns/ext/patch#";
const UI: &str = "http://lv2plug.in/ns/extensions/ui#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const MIDI_EVENT: &str = "http://lv2plug.in/ns/ext/midi#MidiEvent";
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
    /// Among the atom inputs: where MIDI goes, and where `patch` messages go.
    midi_in: Option<usize>,
    patch_in: Option<usize>,
    atom_in_bytes: Vec<usize>,
    atom_out_bytes: Vec<usize>,
    latency_port: Option<PortIndex>,
    /// Among the audio inputs and among the audio outputs: the one or two that carry the
    /// plugin's main sound (left, right).
    main_in: Vec<usize>,
    main_out: Vec<usize>,
    /// The port numbers of the atom inputs, of the atom outputs and of the control outputs,
    /// each in the order livi has them.
    atom_in_ports: Vec<u32>,
    atom_out_ports: Vec<u32>,
    control_out_ports: Vec<u32>,
    /// Every port's number by its symbol.
    symbols: HashMap<CString, u32>,
}

/// Which of the audio ports of one direction are the main ones: not a side chain, in the same
/// group as the first such port, and two at most.
fn main_ports(world: &World, plugin: &livi::Plugin, direction: PortType) -> Vec<usize> {
    let raw = plugin.raw();
    let side_chain = world.uri(&format!("{LV2}isSideChain"));
    let group_of = |port: &lilv::port::Port| world.get(&port.node(), GROUP);
    let mut first_group = None;
    let mut main = Vec::new();
    for (at, port) in plugin.ports_with_type(direction).enumerate() {
        let Some(port) = raw.port_by_index(port.index.0) else {
            continue;
        };
        let group = group_of(&port);
        let aside = port.has_property(&side_chain)
            || group
                .as_ref()
                .is_some_and(|g| world.get(g, SIDE_CHAIN_OF).is_some());
        if aside {
            continue;
        }
        let group = group.and_then(|g| g.as_uri().map(str::to_owned));
        let first = first_group.get_or_insert_with(|| group.clone());
        if *first == group && main.len() < 2 {
            main.push(at);
        }
    }
    main
}

fn layout(world: &World, plugin: &livi::Plugin, features: &Features) -> Layout {
    let raw = plugin.raw();
    let has = |port: &lilv::port::Port, property: &str| port.has_property(&world.uri(property));
    let designated = |port: &lilv::port::Port, what: &str| {
        port.get(&world.uri(&format!("{LV2}designation")))
            .is_some_and(|d| d.as_uri() == Some(&format!("{LV2}{what}")))
    };
    let mut params = Vec::new();
    let mut groups = HashMap::new();
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
        let hosts_own = designated(&lilv_port, "enabled") || designated(&lilv_port, "freeWheeling");
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
    let (mut midi_in, mut patch_in) = (None, None);
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
    }
    let atom_out_bytes = plugin
        .ports_with_type(PortType::AtomSequenceOutput)
        .map(|port| {
            raw.port_by_index(port.index.0)
                .map_or(ATOM_BYTES, |p| bytes(&p))
        })
        .collect();
    Layout {
        params,
        midi_in,
        patch_in,
        atom_in_bytes,
        atom_out_bytes,
        latency_port: raw.latency_port_index().map(PortIndex),
        main_in: main_ports(world, plugin, PortType::AudioInput),
        main_out: main_ports(world, plugin, PortType::AudioOutput),
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
    let classes: Vec<String> = raw
        .value(&lv2_world.uri(RDF_TYPE))
        .iter()
        .filter_map(|class| class.as_uri().map(str::to_owned))
        .collect();
    let counts = *plugin.port_counts();
    let midi = lv2_world.uri(MIDI_EVENT);
    let hears_midi = plugin
        .ports_with_type(PortType::AtomSequenceInput)
        .filter_map(|port| raw.port_by_index(port.index.0))
        .any(|port| port.supports_event(&midi));
    let mut kind = PluginKind::from_lv2(&classes);
    if kind.role() == PluginRole::Effect
        && hears_midi
        && counts.audio_inputs == 0
        && counts.audio_outputs > 0
    {
        kind = PluginKind::Instrument(InstrumentCategory::Other);
    }
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
    };
    let (_shared, mut processor, _link) = instantiate(&lv2_world, &plugin, activation)?;
    info.audio_inputs = processor.main_in.len() as u32;
    info.audio_outputs = processor.main_out.len() as u32;
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
    // SAFETY: running a plugin's code is what this process is for; livi's own conditions (one
    // user of the instance at a time, buffers that outlive each `run`) are kept by the gate
    // and by `Lv2Processor::process`.
    let instance = unsafe { plugin.instantiate(features.clone(), activation.sample_rate) }
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
        latency_port: layout.latency_port,
        main_in: layout.main_in,
        main_out: layout.main_out,
        audio_inputs: counts.audio_inputs,
        cv_inputs: counts.cv_inputs,
        max_frames: frames,
        silence: vec![0.0; frames].into_boxed_slice(),
        mono: vec![0.0; frames].into_boxed_slice(),
        out: vec![0.0; frames * counts.audio_outputs].into_boxed_slice(),
        cv_out: vec![0.0; frames * counts.cv_outputs].into_boxed_slice(),
        sets: Vec::with_capacity(BLOCK_SETS),
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
    let (editor, window) = window(&lv2_world, plugin.raw());
    let plugin = LoadedPlugin {
        latency: 0,
        audio_inputs: processor.main_in.len() as u32,
        audio_outputs: processor.main_out.len() as u32,
        editor,
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
    latency_port: Option<PortIndex>,
    main_in: Vec<usize>,
    main_out: Vec<usize>,
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
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.outputs[0].len();
        let shared = Arc::clone(&self.shared);
        let shared = &*shared;
        if frames > self.max_frames || !shared.gate.enter() {
            // The main thread is taking or putting back the state: this block goes round the
            // plugin.
            for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
                output.copy_from_slice(input);
            }
            return;
        }
        // SAFETY: inside the gate nothing else touches the instance.
        let instance = unsafe { &mut *shared.instance.get() };
        for sequence in &mut self.atom_in {
            sequence.clear();
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
        for (at, event) in block.events.iter().enumerate() {
            let ToPlugin::Param { id, .. } = *event else {
                continue;
            };
            let Some(&param) = shared.by_id.get(&id) else {
                continue;
            };
            if !matches!(shared.params[param].target, Target::Patch { .. }) {
                continue;
            }
            match self.sets.iter().position(|(p, _)| *p == param) {
                Some(set) => self.sets[set].1 = at,
                None if self.sets.len() < self.sets.capacity() => self.sets.push((param, at)),
                None => {}
            }
        }
        for (at, event) in block.events.iter().enumerate() {
            match *event {
                ToPlugin::Midi { frame, data } => self.midi(frame, &data),
                ToPlugin::Param { frame, id, value } => {
                    let Some(&param) = shared.by_id.get(&id) else {
                        continue;
                    };
                    let plain = shared.params[param].plain(value);
                    match shared.params[param].target {
                        Target::Port(index) => {
                            instance.set_control_input(index, plain as f32);
                            shared.set_value(param, value);
                        }
                        Target::Patch { urid, range } => {
                            if self.sets.contains(&(param, at)) {
                                self.set(frame, urid, range, plain);
                                shared.set_value(param, value);
                            }
                        }
                    }
                }
            }
        }

        let [in_left, in_right] = block.inputs;
        let main_in = (self.main_in.first().copied(), self.main_in.get(1).copied());
        if main_in.1.is_none() {
            for (mono, (l, r)) in self.mono.iter_mut().zip(in_left.iter().zip(in_right)) {
                *mono = (l + r) * 0.5;
            }
        }
        let (mono, silence) = (&self.mono[..frames], &self.silence[..frames]);
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
            .map(|channel| &mut channel[..frames]);
        let cv_outputs = self
            .cv_out
            .chunks_exact_mut(self.max_frames)
            .map(|channel| &mut channel[..frames]);
        let ports = EmptyPortConnections::new()
            .with_audio_inputs(inputs)
            .with_audio_outputs(outputs)
            .with_atom_sequence_inputs(self.atom_in.iter())
            .with_atom_sequence_outputs(self.atom_out.iter_mut())
            .with_cv_inputs(cv_inputs)
            .with_cv_outputs(cv_outputs);
        // SAFETY: every buffer lives until `run` is over and is `frames` long or longer; the
        // atom buffers were made with this instance's own features.
        let ran = rtcheck::plugin(|| unsafe { instance.run(frames, ports) }).is_ok();

        let channel = |at: usize| &self.out[at * self.max_frames..][..frames];
        match (ran, self.main_out.first(), self.main_out.get(1)) {
            (true, Some(&left), right) => {
                block.outputs[0].copy_from_slice(channel(left));
                block.outputs[1].copy_from_slice(channel(*right.unwrap_or(&left)));
            }
            _ => {
                for (output, input) in block.outputs.iter_mut().zip(block.inputs) {
                    output.copy_from_slice(input);
                }
            }
        }
        if ran {
            self.hear(block.sink);
            if shared.ui_open.load(Relaxed) {
                self.tell_window(instance);
            }
            if let Some(frames) = self.latency_port.and_then(|p| instance.control_output(p))
                && frames.is_finite()
                && frames >= 0.0
            {
                let frames = (frames.round() as u32).min(MAX_LATENCY_FRAMES);
                shared.latency.store(frames, Relaxed);
            }
        }
        shared.gate.leave();
    }

    fn reset(&mut self) {
        self.quiet = true;
    }

    fn latency(&self) -> u32 {
        self.shared.latency.load(Relaxed)
    }
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
        (self.said.borrow_mut()).push(Said::ParamChanged(self.shared.params[at].id, normal));
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
        link.said.borrow_mut().push(if grabbed {
            Said::ParamBegin(param.id)
        } else {
            Said::ParamEnd(param.id)
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
}

impl Lv2Controller {
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
                    midi_cc: false,
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
        let _claim = (self.shared.gate.claim(GATE_WAIT)).ok_or("the plugin is busy")?;
        // SAFETY: inside the gate nothing else touches the instance.
        let instance = unsafe { &*self.shared.instance.get() };
        let state = instance.state().map_err(|e| format!("state: {e:?}"))?;
        let text = state
            .to_string(STATE_URI)
            .map_err(|e| format!("state: {e:?}"))?;
        let shared = &self.shared;
        // Only what is known: a value nobody knows is not made known by being saved.
        let kept: Vec<(u32, f64)> = (shared.params.iter().enumerate())
            .filter(|(at, param)| {
                matches!(param.target, Target::Patch { .. }) && shared.known[*at].load(Relaxed)
            })
            .map(|(at, param)| (param.id, shared.value(at)))
            .collect();
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
        let _claim = (self.shared.gate.claim(GATE_WAIT)).ok_or("the plugin is busy")?;
        // SAFETY: inside the gate nothing else touches the instance.
        let instance = unsafe { &mut *self.shared.instance.get() };
        instance
            .restore_state(&state)
            .map_err(|e| format!("state: {e:?}"))?;
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
        Ok(())
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
    }
}

const STATE_URI: &str = "urn:meloet:state";
/// A state is: this, the number of `patch` parameters kept, for each its id (u32) and value
/// (f64, 0..=1), little endian, then the state as lilv writes it (Turtle).
const STATE_MAGIC: &[u8; 4] = b"ML2S";
