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

use std::cell::{RefCell, UnsafeCell};
use std::collections::HashMap;
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

use livi::event::{LV2AtomEventBuilder, LV2AtomSequence};
use livi::lilv;
use livi::{EmptyPortConnections, Features, FeaturesBuilder, PortIndex, PortType};
use meloet_plugin::ipc::{FromPlugin, MAX_LATENCY_FRAMES, ToPlugin};
use meloet_plugin::wire::{LoadedPlugin, ParamInfo, ParamShape};
use meloet_plugin::{
    EditorKind, InstrumentCategory, PluginInfo, PluginKey, PluginKind, PluginRole,
};

use super::{
    Activation, Candidate, Controller, GATE_WAIT, Gate, Loaded, ProcessBlock, Processor, Said,
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

/// Bytes an atom port holds when the plugin does not ask for more.
const ATOM_BYTES: usize = 16 * 1024;
/// `patch:writable` parameters that change in one block; a block with more loses the rest.
const BLOCK_SETS: usize = 256;
const PATCH_BIT: u32 = 1 << 31;

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

#[derive(Clone, Copy, PartialEq)]
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

fn group_name(world: &World, subject: &lilv::node::Node) -> Option<String> {
    let group = world.get(subject, GROUP)?;
    world
        .text(&group, &format!("{LV2}name"))
        .or_else(|| world.text(&group, &format!("{RDFS}label")))
        .filter(|name| !name.is_empty())
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
    for port in plugin.ports_with_type(PortType::ControlInput) {
        let Some(lilv_port) = raw.port_by_index(port.index.0) else {
            continue;
        };
        if has(&lilv_port, NOT_ON_GUI) {
            continue;
        }
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
        params.push(Param {
            id: port.index.0 as u32,
            target: Target::Port(port.index),
            name: port.name.clone(),
            group: group_name(world, &subject),
            unit: unit_symbol(world, &subject),
            min,
            max,
            default: f64::from(port.default_value),
            shape,
            // `lv2:enabled` and `lv2:freeWheeling` are the host's switches, not a person's.
            automatable: !designated(&lilv_port, "enabled")
                && !designated(&lilv_port, "freeWheeling"),
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
            group: group_name(world, &node),
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
    }
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
    let editor = match raw.uis() {
        None => EditorKind::None,
        Some(uis) => {
            let x11 = lv2_world.uri(&format!("{UI}X11UI"));
            if uis.iter().any(|ui| ui.is_a(&x11)) {
                EditorKind::X11
            } else if uis.count() > 0 {
                EditorKind::Unsupported
            } else {
                EditorKind::None
            }
        }
    };
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
    let (_shared, mut processor) = instantiate(&lv2_world, &plugin, activation)?;
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
    // Not ended: the scan's process goes when the bundle is read, and ending a plugin can
    // take longer than reading it (LSP).
    std::mem::forget((_shared, processor));
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
    /// The next block asks the plugin for all its `patch` properties.
    ask_all: AtomicBool,
    latency: AtomicU32,
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
    }
}

fn instantiate(
    world: &World,
    plugin: &livi::Plugin,
    activation: Activation,
) -> Result<(Arc<Shared>, Lv2Processor), String> {
    let features = world.features(activation.max_frames)?;
    let layout = layout(world, plugin, &features);
    // SAFETY: running a plugin's code is what this process is for; livi's own conditions (one
    // user of the instance at a time, buffers that outlive each `run`) are kept by the gate
    // and by `Lv2Processor::process`.
    let instance = unsafe { plugin.instantiate(features.clone(), activation.sample_rate) }
        .map_err(|e| format!("instantiate: {e:?}"))?;
    let counts = instance.port_counts();
    let frames = activation.max_frames;
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
        ask_all: AtomicBool::new(layout.patch_in.is_some()),
        latency: AtomicU32::new(0),
        features: features.clone(),
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
    };
    Ok((shared, processor))
}

pub fn load(key: &PluginKey, bundle: &Path, activation: Activation) -> Result<Loaded, String> {
    let lv2_world = world(bundle)?;
    let plugin = lv2_world
        .world
        .plugin_by_uri(key.id())
        .ok_or_else(|| format!("no plugin {} in the bundle", key.id()))?;
    let (shared, processor) = instantiate(&lv2_world, &plugin, activation)?;
    let plugin = LoadedPlugin {
        latency: 0,
        audio_inputs: processor.main_in.len() as u32,
        audio_outputs: processor.main_out.len() as u32,
        // The windows are not opened yet.
        editor: false,
    };
    Ok(Loaded {
        plugin,
        controller: Box::new(Lv2Controller {
            world: lv2_world,
            shared,
            told_latency: 0,
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

struct Lv2Controller {
    /// Keeps the bundle's description for the state calls.
    world: Rc<World>,
    shared: Arc<Shared>,
    told_latency: u32,
}

impl Controller for Lv2Controller {
    fn params(&mut self) -> Vec<ParamInfo> {
        self.shared
            .params
            .iter()
            .map(|param| ParamInfo {
                id: param.id,
                name: param.name.clone(),
                group: param.group.clone(),
                default: param.normal(param.default),
                shape: param.shape,
                automatable: param.automatable,
            })
            .collect()
    }

    fn param(&mut self, id: u32) -> Option<(f64, String)> {
        let &at = self.shared.by_id.get(&id)?;
        let value = self.shared.value(at);
        Some((value, self.shared.params[at].text(value)))
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
        let kept: Vec<(u32, f64)> = (shared.params.iter().enumerate())
            .filter(|(_, param)| matches!(param.target, Target::Patch { .. }))
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
            if let Target::Port(index) = param.target
                && let Some(plain) = instance.control_input(index)
            {
                self.shared.set_value(at, param.normal(f64::from(plain)));
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

    fn open_editor(&mut self, _parent: u32) -> Result<(u32, u32), String> {
        Err("no windows yet".into())
    }

    fn close_editor(&mut self) {}

    fn idle(&mut self) {}

    fn poll(&mut self, said: &mut Vec<Said>) {
        let latency = self.shared.latency.load(Relaxed);
        if latency != self.told_latency {
            self.told_latency = latency;
            said.push(Said::Latency(latency));
        }
    }
}

const STATE_URI: &str = "urn:meloet:state";
/// A state is: this, the number of `patch` parameters kept, for each its id (u32) and value
/// (f64, 0..=1), little endian, then the state as lilv writes it (Turtle).
const STATE_MAGIC: &[u8; 4] = b"ML2S";
