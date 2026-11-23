//! Parameters of insert effects and plugins: reading them in pages, the names of the values of
//! choice parameters, setting one or several, and the list of effects.
//!
//! A plugin's parameter is set as the parameters panel sets a typed value: the plugin takes
//! it, then its state is asked for and becomes the step. The core ends each set with what
//! happened (`AiAsk::ParamsSet`), so the answer says for sure whether the song holds it.

use std::time::{Duration, Instant};

use meloet_core::ai::{AiAnswer, AiAsk, AiParam, AiParams, AiParamsSet, AiSetEnd, AiSong};
use meloet_model::effects::{EffectKind, ParamShape, ParamSpec, Unit};
use meloet_model::{AutoTarget, Insert, PluginPlace, StripId};
use meloet_protocol::{Command, InsertRef, PluginKindView};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::mixer::{BusArg, screen_effect, slot_of, strip_of, strip_ref, unit_name};
use super::{Tools, WAIT_FOR, closing, done, failed, refusal_text};

/// Parameters a page of `get_plugin_params` gives when no `limit` is given, and at most.
const PAGE: usize = 50;
const MOST_PAGE: usize = 500;
/// Rows a page of `list_effects` gives when no `limit` is given, and at most.
const EFFECTS_PAGE: usize = 50;
const MOST_EFFECTS_PAGE: usize = 500;
/// Parameters `choices: true` reads the names of in one call.
const MOST_SCANNED: usize = 20;
/// Values a plugin is asked for the texts of, to find the names of a parameter's values.
const PROBES: u32 = 1001;
const MOST_PROBES: u32 = 4001;
/// More texts than this along the range: the text follows the value, it is no list.
const MOST_CHOICES: u32 = 64;
const MOST_MOST_CHOICES: u32 = 256;
/// Parameters one `set_params` takes.
const MOST_SET: usize = 64;
/// How long a plugin's list or texts are waited for.
const LIST_WAIT: Duration = Duration::from_secs(5);
/// A value that differs from its default by more than this is "changed": half a percent of
/// the range, so that a plugin that keeps its choices just inside a step (Surge XT: 0.005
/// for the first) does not count as changed.
const CHANGED: f64 = 0.005;

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ParamsArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// Only parameters whose name holds this text (any case).
    #[serde(default)]
    #[schemars(length(min = 1, max = 128))]
    pub name: Option<String>,
    /// Only parameters of this group (any case; "" for those in no group).
    #[serde(default)]
    #[schemars(length(max = 128))]
    pub group: Option<String>,
    /// Only parameters whose value is not their default (by more than half a percent, or
    /// another step).
    #[serde(default)]
    pub changed_only: bool,
    /// Only these parameters, by id.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub ids: Option<Vec<u32>>,
    /// Also the parameters a plugin has only to take a keyboard's knobs ("MIDI CC 0|1").
    #[serde(default)]
    pub midi_cc: bool,
    /// Each parameter as id, name, value and text only.
    #[serde(default)]
    pub brief: bool,
    /// The group names with how many parameters (after the filters) each has, instead of
    /// the parameters.
    #[serde(default)]
    pub groups_only: bool,
    /// Also the names of the values of every listed parameter that has no steps of its own
    /// (as get_param_choices finds them). At most 20 parameters a call.
    #[serde(default)]
    pub choices: bool,
    /// Parameters in this answer (default 50, at most 500).
    #[serde(default)]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
    /// How many to pass over first: the `next_offset` of the answer before.
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ChoicesArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// A plugin's parameter by its id or its exact name; a built-in effect's by name or
    /// index.
    pub param: Value,
    /// How many values from 0 to 1 the plugin is asked for its text of (default 1001, at
    /// most 4001). Values narrower than 1 / (probes - 1) can be missed.
    #[serde(default)]
    #[schemars(range(min = 2, max = 4001))]
    pub probes: Option<u32>,
    /// More different texts than this (default 64, at most 256) means the text follows the
    /// value and is no list of names.
    #[serde(default)]
    #[schemars(range(min = 2, max = 256))]
    pub most: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ParamArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// A built-in effect's parameter: its name or index from get_mixer. A plugin's: its id
    /// or its exact name from get_plugin_params.
    pub param: Value,
    /// A built-in effect's parameter in its own unit (as get_mixer shows); a plugin's from 0
    /// to 1. Give this or `text`.
    #[serde(default)]
    pub value: Option<f64>,
    /// The name of one of the parameter's values, as get_param_choices lists them (a
    /// built-in effect's step or "on" / "off"; a built-in value with its unit, "2 kHz", too).
    /// Give this or `value`.
    #[serde(default)]
    #[schemars(length(min = 1, max = 128))]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct OneParam {
    /// As `param` of set_param.
    pub param: Value,
    /// As `value` of set_param. Give this or `text`.
    #[serde(default)]
    pub value: Option<f64>,
    /// As `text` of set_param. Give this or `value`.
    #[serde(default)]
    #[schemars(length(min = 1, max = 128))]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SetParamsArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// 1 to 64 parameters of that one effect or plugin.
    #[schemars(length(min = 1, max = 64))]
    pub params: Vec<OneParam>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct EffectsArgs {
    /// Only effects whose name or vendor holds this text (any case).
    #[serde(default)]
    #[schemars(length(min = 1, max = 128))]
    pub name: Option<String>,
    /// Only plugins of this kind: eq, dynamics, reverb, delay, modulation, distortion,
    /// filter, pitch, spatial, analysis, other.
    #[serde(default)]
    #[schemars(length(min = 1, max = 32))]
    pub kind: Option<String>,
    /// Only plugins that come in this format: "vst3" or "lv2".
    #[serde(default)]
    #[schemars(length(min = 1, max = 8))]
    pub format: Option<String>,
    /// One effect only, by a plugin's key or a built-in effect's name.
    #[serde(default)]
    #[schemars(length(min = 1, max = 1024))]
    pub effect: Option<String>,
    /// Each plugin as key, name, vendor and kind only, and the built-in effects without
    /// their parameters.
    #[serde(default)]
    pub brief: bool,
    /// Plugins in this answer (default 50, at most 500).
    #[serde(default)]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
    /// How many plugins to pass over first: the `next_offset` of the answer before.
    #[serde(default)]
    pub offset: Option<u32>,
}

fn param_specs(kind: EffectKind) -> Vec<Value> {
    (kind.params().iter().enumerate())
        .map(|(i, p)| {
            let steps = match p.shape {
                ParamShape::Steps(names) => Some(names.to_vec()),
                _ => None,
            };
            json!({
                "index": i, "name": p.name, "unit": unit_name(p.unit),
                "min": p.min, "max": p.max, "default": p.default,
                "switch": matches!(p.shape, ParamShape::Switch), "steps": steps,
            })
        })
        .collect()
}

/// Where a plugin is: an insert slot, or the instrument of a track.
fn place_of(
    read: &AiSong,
    track: Option<u32>,
    bus: Option<BusArg>,
    slot: Option<u32>,
) -> Result<PluginPlace, CallToolResult> {
    let strip = strip_of(read, track, bus)?;
    match (slot, strip) {
        (Some(slot), strip) => Ok(PluginPlace::Insert(strip, slot_of(slot)?)),
        (None, StripId::Track(id)) => Ok(PluginPlace::Instrument(id)),
        (None, _) => Err(failed("give the insert `slot` of the plugin")),
    }
}

/// What sits at a place: a built-in effect, or a plugin.
enum At<'a> {
    Builtin(StripId, usize, &'a Insert, EffectKind),
    Plugin(PluginPlace, PluginInfo),
}

struct PluginInfo {
    key: String,
    name: String,
    lv2: bool,
}

impl PluginInfo {
    fn value(&self) -> Value {
        let format = if self.lv2 { "lv2" } else { "vst3" };
        json!({ "key": self.key, "name": self.name, "format": format })
    }
}

fn at<'a>(read: &'a AiSong, place: PluginPlace) -> Result<At<'a>, CallToolResult> {
    let doc = &read.song.doc;
    if let PluginPlace::Insert(strip, slot) = place {
        let insert = doc
            .insert(strip, slot)
            .ok_or_else(|| failed(refusal_text(&meloet_core::ai::AiRefusal::SlotEmpty)))?;
        if let Some(kind) = insert.kind() {
            return Ok(At::Builtin(strip, slot, insert, kind));
        }
    }
    let plugin = doc
        .plugin(place)
        .ok_or_else(|| failed(refusal_text(&meloet_core::ai::AiRefusal::NoSuchPlugin)))?;
    Ok(At::Plugin(
        place,
        PluginInfo {
            key: plugin.key.to_string(),
            name: plugin.name.to_string(),
            lv2: plugin.key.starts_with("lv2:"),
        },
    ))
}

/// A value as it is written out: six decimals are plenty for 0..1.
fn short(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

fn kind_of(p: &AiParam) -> &'static str {
    match (p.steps, p.labels.is_empty()) {
        (0, _) => "smooth",
        (2, true) => "switch",
        _ => "steps",
    }
}

/// The values of a parameter with steps of its own, each with its name.
fn labelled(p: &AiParam) -> Option<Vec<(f64, String)>> {
    if p.steps < 2 || p.labels.len() != p.steps as usize {
        return None;
    }
    let last = f64::from(p.steps - 1);
    Some(
        (p.labels.iter().enumerate())
            .map(|(i, l)| (i as f64 / last, l.clone()))
            .collect(),
    )
}

fn starts_with_number(text: &str) -> bool {
    let t = text
        .trim_start()
        .trim_start_matches(['-', '+', '−'])
        .trim_start();
    t.starts_with(|c: char| c.is_ascii_digit())
        || t.starts_with('.')
        || t.get(..3)
            .is_some_and(|inf| inf.eq_ignore_ascii_case("inf"))
        || t.starts_with('∞')
}

/// The plugin marks no steps, but its text now is a name.
fn likely_choice(p: &AiParam) -> bool {
    p.steps == 0
        && (p.text.as_deref()).is_some_and(|t| !t.trim().is_empty() && !starts_with_number(t))
}

fn changed_from_default(p: &AiParam) -> bool {
    let Some(value) = p.value else {
        return false;
    };
    let near = match p.steps {
        0 => CHANGED,
        steps => 0.5 / f64::from(steps - 1),
    };
    (value - p.default).abs() > near
}

fn param_value(p: &AiParam, brief: bool) -> Value {
    if brief {
        return json!({ "id": p.id, "name": p.name, "value": p.value.map(short), "text": p.text });
    }
    let mut v = json!({
        "id": p.id, "name": p.name, "group": p.group, "value": p.value.map(short),
        "default": short(p.default), "text": p.text, "steps": p.steps, "labels": p.labels,
        "automatable": p.automatable, "kind": kind_of(p),
    });
    if let Some(choices) = labelled(p) {
        v["choices"] = choices_value(&choices);
    } else if likely_choice(p) {
        v["likely_choice"] = json!(true);
    }
    v
}

fn choices_value(choices: &[(f64, String)]) -> Value {
    (choices.iter())
        .map(|(value, text)| json!({ "value": short(*value), "text": text }))
        .collect()
}

/// The values `probes` from 0 to 1.
fn probe_values(probes: u32) -> Vec<f64> {
    let last = f64::from(probes.max(2) - 1);
    (0..probes.max(2)).map(|i| f64::from(i) / last).collect()
}

/// What the texts at evenly spread values say of a parameter.
#[derive(Debug, PartialEq)]
enum Scanned {
    /// Where the text changes: a value inside each stretch of one text (its middle; 0 and 1
    /// for the first and the last), and the text.
    List(Vec<(f64, String)>),
    /// The text follows the value: this many different texts.
    Smooth(usize),
    /// The plugin gives no texts.
    NoTexts,
}

fn scan(values: &[f64], texts: &[Option<String>], most: usize) -> Scanned {
    if texts.iter().all(Option::is_none) {
        return Scanned::NoTexts;
    }
    // Each stretch: its first value, its last value and its text.
    let mut runs: Vec<(f64, f64, &str)> = Vec::new();
    for (&value, text) in values.iter().zip(texts) {
        let text = text.as_deref().unwrap_or("");
        match runs.last_mut() {
            Some(run) if run.2 == text => run.1 = value,
            _ => runs.push((value, value, text)),
        }
    }
    if runs.len() > most {
        return Scanned::Smooth(runs.len());
    }
    let last = runs.len() - 1;
    let mut list: Vec<(f64, String)> = Vec::new();
    for (i, &(low, high, text)) in runs.iter().enumerate() {
        let value = match i {
            0 => low,
            i if i == last => high,
            _ => (low + high) / 2.0,
        };
        if !list.iter().any(|(_, known)| known == text) {
            list.push((value, text.to_owned()));
        }
    }
    Scanned::List(list)
}

/// A parameter of a plugin by its id or its exact name (any case when only one has it).
fn find_param<'a>(list: &'a [AiParam], param: &Value) -> Result<&'a AiParam, String> {
    match param {
        Value::Number(n) => {
            let id = n.as_u64().and_then(|n| u32::try_from(n).ok());
            (id.and_then(|id| list.iter().find(|p| p.id == id))).ok_or_else(|| {
                format!("the plugin has no parameter {n}; get_plugin_params lists them")
            })
        }
        Value::String(name) => {
            let name = name.trim();
            if let Some(p) = list.iter().find(|p| p.name == name) {
                return Ok(p);
            }
            let same: Vec<&AiParam> = (list.iter())
                .filter(|p| p.name.eq_ignore_ascii_case(name))
                .collect();
            if let [one] = same[..] {
                return Ok(one);
            }
            let lower = name.to_lowercase();
            let near: Vec<&str> = (list.iter())
                .filter(|p| p.name.to_lowercase().contains(&lower))
                .take(10)
                .map(|p| p.name.as_str())
                .collect();
            Err(format!(
                "the plugin has no parameter named \"{name}\"{}",
                if near.is_empty() {
                    "; get_plugin_params with `name` finds them".to_owned()
                } else {
                    format!("; ones that hold it: {near:?}")
                }
            ))
        }
        _ => Err("`param` is a parameter's id (a number) or its name".into()),
    }
}

/// A parameter of a built-in effect by its name or index.
fn builtin_param(kind: EffectKind, param: &Value) -> Result<(u32, &'static ParamSpec), String> {
    let index = match param {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(name) => kind.param_index(name.trim()),
        _ => None,
    };
    index
        .and_then(|i| Some((i, kind.param(i)?)))
        .ok_or_else(|| {
            format!(
                "{} has no such parameter; get_mixer lists them",
                kind.name()
            )
        })
}

fn unit_text(unit: Unit) -> &'static str {
    match unit {
        Unit::Hz => " Hz",
        Unit::Db => " dB",
        Unit::Ms => " ms",
        Unit::Seconds => " s",
        Unit::Percent => " %",
        Unit::Octaves => " oct",
        Unit::Ratio => " : 1",
        Unit::None => "",
    }
}

/// A built-in effect's value as words: its step, on or off, or the number with its unit.
fn builtin_text(spec: &ParamSpec, value: f32) -> String {
    match spec.shape {
        ParamShape::Steps(names) => (names.get(value.round().max(0.0) as usize))
            .map_or_else(|| format!("{value}"), |n| (*n).to_owned()),
        ParamShape::Switch => if value >= 0.5 { "on" } else { "off" }.to_owned(),
        ParamShape::Continuous => {
            let number = format!("{:.2}", value);
            let number = number.trim_end_matches('0').trim_end_matches('.');
            format!("{number}{}", unit_text(spec.unit))
        }
    }
}

/// The names of a built-in effect's values, if they have names.
fn builtin_choices(spec: &ParamSpec) -> Option<Vec<(f64, String)>> {
    match spec.shape {
        ParamShape::Steps(names) => Some(
            (names.iter().enumerate())
                .map(|(i, n)| (i as f64, (*n).to_owned()))
                .collect(),
        ),
        ParamShape::Switch => Some(vec![(0.0, "off".into()), (1.0, "on".into())]),
        ParamShape::Continuous => None,
    }
}

/// A built-in effect's value from `value` or `text`.
fn builtin_value(spec: &ParamSpec, value: Option<f64>, text: Option<&str>) -> Result<f32, String> {
    match (value, text) {
        (Some(v), None) if v.is_finite() => Ok(v as f32),
        (Some(_), None) => Err("the value must be a number".into()),
        (None, Some(text)) => {
            let text = text.trim();
            if let Some(choices) = builtin_choices(spec) {
                let named = (choices.iter()).find(|(_, n)| n.eq_ignore_ascii_case(text));
                let truth = match text.to_lowercase().as_str() {
                    "true" | "yes" => Some(1.0),
                    "false" | "no" => Some(0.0),
                    _ => None,
                };
                return match (named, spec.shape) {
                    (Some((v, _)), _) => Ok(*v as f32),
                    (None, ParamShape::Switch) if truth.is_some() => Ok(truth.unwrap_or(0.0)),
                    _ => Err(format!(
                        "\"{text}\" is none of {:?}",
                        choices.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>()
                    )),
                };
            }
            spec.read(text).ok_or_else(|| {
                format!("\"{text}\" is not a value of this parameter (a number in its unit)")
            })
        }
        _ => Err("give `value` or `text`, one of them".into()),
    }
}

/// The page of a list: the first and how many.
fn page(offset: Option<u32>, limit: Option<u32>, default: usize, most: usize) -> (usize, usize) {
    let limit = limit.map_or(default, |l| (l as usize).clamp(1, most));
    (offset.unwrap_or(0) as usize, limit)
}

fn paged(total: usize, offset: usize, count: usize) -> Value {
    let next = offset + count;
    json!({
        "total": total, "offset": offset, "count": count,
        "next_offset": (next < total).then_some(next),
    })
}

/// A plugin's answer of a set, against what it had before.
struct SetRow {
    id: u32,
    name: String,
    asked: f64,
    before: (Option<f64>, Option<String>),
    now: (Option<f64>, Option<String>),
}

impl SetRow {
    /// The plugin shows something else now: another text, or another value when it shows
    /// no text.
    fn moved(&self) -> bool {
        match (&self.before.1, &self.now.1) {
            (Some(before), Some(now)) => before != now,
            _ => match (self.before.0, self.now.0) {
                (Some(before), Some(now)) => (before - now).abs() > 1e-9,
                _ => true,
            },
        }
    }

    fn value(&self) -> Value {
        json!({
            "id": self.id, "name": self.name, "asked": short(self.asked),
            "value": self.now.0.map(short), "text": self.now.1,
            "before": { "value": self.before.0.map(short), "text": self.before.1 },
            "changed": self.moved(),
        })
    }
}

#[tool_router(router = params_router, vis = "pub(super)")]
impl Tools {
    /// The effects an insert slot can take: meloet's own six with their parameters (on the
    /// first page) and this PC's effect plugins by key, 50 a page (`limit`, `offset`; the
    /// answer has `total` and `next_offset`). Narrow it with `name`, `kind`, `format`, or
    /// `effect` for one; `brief` gives less of each.
    #[tool(annotations(title = "List effects", read_only_hint = true, open_world_hint = false))]
    async fn list_effects(
        &self,
        Parameters(args): Parameters<EffectsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let lower = |t: &Option<String>| t.as_ref().map(|t| t.trim().to_lowercase());
        let (name, kind, format) = (lower(&args.name), lower(&args.kind), lower(&args.format));
        let effect = args.effect.as_deref().map(str::trim);
        let named = |text: &str| {
            name.as_ref()
                .is_none_or(|n| text.to_lowercase().contains(n))
        };
        let builtin: Vec<Value> = (EffectKind::ALL.iter())
            .filter(|_| kind.is_none() && format.is_none())
            .filter(|k| effect.is_none_or(|e| e == k.name()) && named(k.name()))
            .map(|k| match args.brief {
                true => json!({ "effect": k.name() }),
                false => json!({ "effect": k.name(), "params": param_specs(*k) }),
            })
            .collect();
        let plugins: Vec<Value> = (read.library.plugins.iter())
            .filter(|p| matches!(p.kind, PluginKindView::Effect(_)))
            .filter(|p| effect.is_none_or(|e| p.key == e || p.formats.iter().any(|f| f.key == e)))
            .filter(|p| named(&p.name) || named(&p.vendor))
            .filter(|p| {
                let category = serde_json::to_value(p.kind).unwrap_or_default();
                kind.as_ref()
                    .is_none_or(|k| category["category"] == json!(k))
            })
            .filter(|p| {
                format.as_ref().is_none_or(|f| {
                    (p.formats.iter())
                        .any(|pf| serde_json::to_value(pf.format).unwrap_or_default() == json!(f))
                })
            })
            .map(|p| match args.brief {
                true => {
                    let kind = serde_json::to_value(p.kind).unwrap_or_default();
                    json!({ "key": p.key, "name": p.name, "vendor": p.vendor,
                        "kind": kind["category"] })
                }
                false => json!({ "key": p.key, "name": p.name, "vendor": p.vendor,
                    "kind": p.kind, "formats": p.formats }),
            })
            .collect();
        let (offset, limit) = page(args.offset, args.limit, EFFECTS_PAGE, MOST_EFFECTS_PAGE);
        let total = plugins.len();
        let shown: Vec<Value> = plugins.into_iter().skip(offset).take(limit).collect();
        let mut value = paged(total, offset, shown.len());
        value["builtin"] = json!(if offset == 0 { builtin } else { Vec::new() });
        value["plugins"] = json!(shown);
        Ok(done(value.to_string(), value))
    }

    /// A plugin's parameters (an insert, or the track's instrument without `slot`): id, name,
    /// group, value 0 to 1, default, text, kind and the `choices` of one with steps. 50 a
    /// page (`limit`, `offset`; `total`, `next_offset`): narrow with `name`, `group`, `ids`,
    /// `changed_only`; `groups_only` and `brief` give less. `likely_choice`: names for its
    /// values come with `choices: true`.
    #[tool(annotations(
        title = "Get plugin parameters",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn get_plugin_params(
        &self,
        Parameters(args): Parameters<ParamsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let place = check!(place_of(&read, args.track, args.bus, args.slot));
        let At::Plugin(place, info) = check!(at(&read, place)) else {
            return Ok(failed(
                "that slot holds a built-in effect; get_mixer shows its parameters",
            ));
        };
        let list = check!(self.plugin_list(place).await);
        let name = args.name.as_ref().map(|n| n.trim().to_lowercase());
        let group = args.group.as_ref().map(|g| g.trim().to_lowercase());
        let kept: Vec<&AiParam> = (list.iter())
            .filter(|p| args.midi_cc || !p.midi_cc)
            .filter(|p| {
                name.as_ref()
                    .is_none_or(|n| p.name.to_lowercase().contains(n))
            })
            .filter(|p| {
                group
                    .as_ref()
                    .is_none_or(|g| p.group.as_deref().unwrap_or("").to_lowercase() == *g)
            })
            .filter(|p| args.ids.as_ref().is_none_or(|ids| ids.contains(&p.id)))
            .filter(|p| !args.changed_only || changed_from_default(p))
            .collect();
        if args.groups_only {
            let mut groups: Vec<(Option<&str>, usize)> = Vec::new();
            for p in &kept {
                match groups.iter_mut().find(|(g, _)| *g == p.group.as_deref()) {
                    Some((_, count)) => *count += 1,
                    None => groups.push((p.group.as_deref(), 1)),
                }
            }
            let groups: Vec<Value> = (groups.iter())
                .map(|(name, count)| json!({ "name": name, "count": count }))
                .collect();
            let value = json!({ "plugin": info.value(), "total": kept.len(), "groups": groups });
            return Ok(done(value.to_string(), value));
        }
        let (offset, limit) = page(args.offset, args.limit, PAGE, MOST_PAGE);
        let shown: Vec<&AiParam> = kept.iter().skip(offset).take(limit).copied().collect();
        let mut params: Vec<Value> = shown.iter().map(|p| param_value(p, args.brief)).collect();
        if args.choices {
            let wanted: Vec<usize> = (shown.iter().enumerate())
                .filter(|(_, p)| labelled(p).is_none())
                .map(|(i, _)| i)
                .collect();
            if wanted.len() > MOST_SCANNED {
                return Ok(failed(format!(
                    "`choices: true` reads at most {MOST_SCANNED} parameters at a time and this \
                     page has {} without steps; narrow it with `name`, `group`, `ids` or `limit`",
                    wanted.len()
                )));
            }
            for i in wanted {
                let (list, note) =
                    check!(self.choices_of(place, shown[i], &info, None, None).await);
                params[i]["choices"] = list.as_deref().map_or(Value::Null, choices_value);
                if let Some(note) = note {
                    params[i]["choices_note"] = json!(note);
                }
            }
        }
        let mut value = paged(kept.len(), offset, params.len());
        value["plugin"] = info.value();
        value["params"] = json!(params);
        Ok(done(value.to_string(), value))
    }

    /// The names of a choice parameter's values, each with the value to set for it: a
    /// built-in effect's or a plugin's (by id or exact name). For a plugin parameter without
    /// steps of its own, the plugin is asked for its text at many values (nothing is set)
    /// and the list is made where the text changes.
    #[tool(annotations(
        title = "Get parameter choices",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn get_param_choices(
        &self,
        Parameters(args): Parameters<ChoicesArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let place = check!(place_of(&read, args.track, args.bus, args.slot));
        match check!(at(&read, place)) {
            At::Builtin(_, _, _, kind) => {
                let (index, spec) = check!(builtin_param(kind, &args.param).map_err(failed));
                let value = match builtin_choices(spec) {
                    Some(list) => json!({ "index": index, "name": spec.name, "list": true,
                        "choices": choices_value(&list), "source": "steps" }),
                    None => json!({ "index": index, "name": spec.name, "list": false,
                        "note": format!("a smooth parameter from {} to {}{}",
                            spec.min, spec.max, unit_text(spec.unit)) }),
                };
                Ok(done(value.to_string(), value))
            }
            At::Plugin(place, info) => {
                let list = check!(self.plugin_list(place).await);
                let p = check!(find_param(&list, &args.param).map_err(failed));
                let (choices, note) = check!(
                    self.choices_of(place, p, &info, args.probes, args.most)
                        .await
                );
                let source = match (labelled(p).is_some(), &choices) {
                    (true, _) => "steps",
                    (false, Some(_)) => "scan",
                    (false, None) => "none",
                };
                let mut value = json!({
                    "id": p.id, "name": p.name, "plugin": info.value(),
                    "list": choices.is_some(), "source": source,
                    "choices": choices.as_deref().map(choices_value),
                });
                if let Some(note) = note {
                    value["note"] = json!(note);
                }
                Ok(done(value.to_string(), value))
            }
        }
    }

    /// Sets a parameter of an insert effect (built-in: name or index, in its unit) or a
    /// plugin (id or exact name, 0 to 1; without `slot`, the track's instrument) as one undo
    /// step; `text` sets a value by its name. Answers the value and text afterwards and
    /// `changed`; errors say when the song does not hold it.
    #[tool(annotations(
        title = "Set effect parameter",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_param(
        &self,
        Parameters(args): Parameters<ParamArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let place = check!(place_of(&read, args.track, args.bus, args.slot));
        let one = OneParam {
            param: args.param,
            value: args.value,
            text: args.text,
        };
        match check!(at(&read, place)) {
            At::Builtin(strip, slot, insert, kind) => {
                let (index, spec) = check!(builtin_param(kind, &one.param).map_err(failed));
                let value =
                    check!(builtin_value(spec, one.value, one.text.as_deref()).map_err(failed));
                let lane = match strip {
                    StripId::Track(t) => (read.song.doc.track(t))
                        .and_then(|t| {
                            t.lane(AutoTarget::Insert {
                                insert: insert.id,
                                param: index,
                            })
                        })
                        .is_some(),
                    _ => false,
                };
                if lane {
                    // As the screen does: the value rests the lane, in the same step.
                    let before = [(index, value, insert.value(index))];
                    let insert = InsertRef {
                        strip: strip_ref(strip),
                        slot: slot as u8,
                    };
                    let command = Command::SetParam {
                        insert,
                        effect: screen_effect(kind),
                        param: index,
                        value,
                        commit: true,
                    };
                    let result = self.screen(command).await;
                    let (Some(said), false) =
                        (&result.structured_content, result.is_error == Some(true))
                    else {
                        return Ok(result);
                    };
                    let mut answer = check!(self.builtin_answer(strip, slot, &before, true).await);
                    for (k, v) in said.as_object().into_iter().flatten() {
                        answer[k] = v.clone();
                    }
                    let answer = flatten(answer);
                    return Ok(done(
                        format!("Done: set the effect parameter.\n{answer}"),
                        answer,
                    ));
                }
                Ok(self
                    .set_builtin(strip, slot, insert, vec![(index, value)], true)
                    .await)
            }
            At::Plugin(place, info) => Ok(self.set_plugin(place, &info, vec![one], true).await),
        }
    }

    /// Sets several parameters of one insert effect or plugin as one undo step, each as
    /// set_param takes it (`param` with `value` or `text`). All are checked first: if one
    /// cannot be set (unknown, out of range, moved by an automation lane), nothing is set.
    /// The answer gives each one's value and text afterwards and whether it changed.
    #[tool(annotations(
        title = "Set parameters",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_params(
        &self,
        Parameters(args): Parameters<SetParamsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if args.params.is_empty() || args.params.len() > MOST_SET {
            return Ok(failed(format!("give 1 to {MOST_SET} parameters")));
        }
        let read = check!(self.read().await);
        let place = check!(place_of(&read, args.track, args.bus, args.slot));
        match check!(at(&read, place)) {
            At::Builtin(strip, slot, insert, kind) => {
                let mut values = Vec::new();
                let mut wrong = Vec::new();
                for (i, one) in args.params.iter().enumerate() {
                    let found = builtin_param(kind, &one.param).and_then(|(index, spec)| {
                        Ok((index, builtin_value(spec, one.value, one.text.as_deref())?))
                    });
                    match found {
                        Ok((index, _)) if values.iter().any(|(at, _)| *at == index) => {
                            wrong.push(format!("params[{i}]: the same parameter twice"));
                        }
                        Ok(found) => values.push(found),
                        Err(why) => wrong.push(format!("params[{i}]: {why}")),
                    }
                }
                if !wrong.is_empty() {
                    return Ok(failed(format!("Nothing was set: {}.", wrong.join("; "))));
                }
                Ok(self.set_builtin(strip, slot, insert, values, false).await)
            }
            At::Plugin(place, info) => Ok(self.set_plugin(place, &info, args.params, false).await),
        }
    }
}

/// The one row of an answer made for several, at its top.
fn flatten(mut answer: Value) -> Value {
    if let Some(row) = answer["params"]
        .as_array()
        .and_then(|rows| rows.first())
        .cloned()
        && let (Some(map), Some(row)) = (answer.as_object_mut(), row.as_object())
    {
        map.remove("params");
        for (k, v) in row {
            map.entry(k.clone()).or_insert(v.clone());
        }
    }
    answer
}

impl Tools {
    /// An ask answered with [`AiAnswer::Params`]; asked again while the plugin has not
    /// answered, for `within` at most.
    async fn params_answer(
        &self,
        ask: AiAsk,
        within: Duration,
    ) -> Result<AiParams, CallToolResult> {
        let until = Instant::now() + within;
        loop {
            match self.answer(ask.clone()).await? {
                AiAnswer::Params(AiParams::Waiting) if Instant::now() < until => {}
                AiAnswer::Params(AiParams::Waiting) => {
                    return Err(failed(format!(
                        "the plugin did not answer within {} s; try again",
                        within.as_secs()
                    )));
                }
                AiAnswer::Params(params) => return Ok(params),
                AiAnswer::Refused(r) => return Err(failed(refusal_text(&r))),
                _ => return Err(closing()),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn plugin_list(&self, place: PluginPlace) -> Result<Vec<AiParam>, CallToolResult> {
        match self
            .params_answer(AiAsk::PluginParams { place }, LIST_WAIT)
            .await?
        {
            AiParams::Listed(list) => Ok(list),
            _ => Err(closing()),
        }
    }

    /// The names of a plugin parameter's values: its steps, or what its texts say. `None`
    /// with a note when they are no list.
    async fn choices_of(
        &self,
        place: PluginPlace,
        p: &AiParam,
        info: &PluginInfo,
        probes: Option<u32>,
        most: Option<u32>,
    ) -> Result<(Option<Vec<(f64, String)>>, Option<String>), CallToolResult> {
        if let Some(list) = labelled(p) {
            return Ok((Some(list), None));
        }
        let probes = match p.steps {
            2 => 2,
            _ => probes.unwrap_or(PROBES).clamp(2, MOST_PROBES),
        };
        let most = most.unwrap_or(MOST_CHOICES).clamp(2, MOST_MOST_CHOICES) as usize;
        let values = probe_values(probes);
        let ask = AiAsk::ParamTexts {
            place,
            param: p.id,
            values: values.clone(),
        };
        let AiParams::Texts(texts) = self.params_answer(ask, LIST_WAIT).await? else {
            return Err(closing());
        };
        let lv2 = || {
            if info.lv2 {
                " LV2 plugins have no texts of their own for values: meloet names only the \
                 values of their scale points, and lists those as the labels of a parameter \
                 with steps."
            } else {
                ""
            }
        };
        Ok(match scan(&values, &texts, most) {
            Scanned::List(list) if list.len() >= 2 || p.steps == 2 => (Some(list), None),
            Scanned::List(_) => (
                None,
                Some(format!(
                    "its text is the same over the whole range.{}",
                    lv2()
                )),
            ),
            Scanned::Smooth(n) => {
                let end = |t: Option<&Option<String>>| t.cloned().flatten().unwrap_or_default();
                let note = format!(
                    "no list: {n} different texts in {probes} values, from \"{}\" to \"{}\"; \
                     set it with `value`.{}",
                    end(texts.first()),
                    end(texts.last()),
                    lv2()
                );
                (None, Some(note))
            }
            Scanned::NoTexts => (
                None,
                Some(format!("the plugin gives no texts for it.{}", lv2())),
            ),
        })
    }

    /// A plugin parameter's value from `value` (0..1) or `text` (a name of one of its
    /// values).
    async fn plugin_value(
        &self,
        place: PluginPlace,
        p: &AiParam,
        info: &PluginInfo,
        one: &OneParam,
    ) -> Result<f64, String> {
        match (one.value, one.text.as_deref()) {
            (Some(v), None) if (0.0..=1.0).contains(&v) => Ok(v),
            (Some(_), None) => Err("a plugin's parameter is 0 to 1".into()),
            (None, Some(text)) => {
                let (choices, note) = (self.choices_of(place, p, info, None, None).await)
                    .map_err(|e| tool_text(&e))?;
                let Some(choices) = choices else {
                    return Err(format!(
                        "\"{}\" has no names for its values ({}); give `value` (0 to 1)",
                        p.name,
                        note.unwrap_or_default().trim_end_matches('.')
                    ));
                };
                let text = text.trim();
                (choices.iter())
                    .find(|(_, name)| name.trim().eq_ignore_ascii_case(text))
                    .map(|(value, _)| *value)
                    .ok_or_else(|| {
                        let names: Vec<&str> = choices.iter().map(|(_, n)| n.as_str()).collect();
                        format!(
                            "\"{}\" has no value named \"{text}\"; its values are {names:?}",
                            p.name
                        )
                    })
            }
            _ => Err("give `value` or `text`, one of them".into()),
        }
    }

    /// Sets parameters of a plugin as one step and says what came of it.
    async fn set_plugin(
        &self,
        place: PluginPlace,
        info: &PluginInfo,
        wanted: Vec<OneParam>,
        single: bool,
    ) -> CallToolResult {
        let list = match self.plugin_list(place).await {
            Ok(list) => list,
            Err(result) => return result,
        };
        let mut rows: Vec<SetRow> = Vec::new();
        let mut wrong = Vec::new();
        for (i, one) in wanted.iter().enumerate() {
            let at = if single {
                String::new()
            } else {
                format!("params[{i}]: ")
            };
            let p = match find_param(&list, &one.param) {
                Ok(p) => p,
                Err(why) => {
                    wrong.push(format!("{at}{why}"));
                    continue;
                }
            };
            if rows.iter().any(|r| r.id == p.id) {
                wrong.push(format!("{at}\"{}\" twice", p.name));
                continue;
            }
            match self.plugin_value(place, p, info, one).await {
                Ok(asked) => rows.push(SetRow {
                    id: p.id,
                    name: p.name.clone(),
                    asked,
                    before: (p.value, p.text.clone()),
                    now: (None, None),
                }),
                Err(why) => wrong.push(format!("{at}{why}")),
            }
        }
        if !wrong.is_empty() {
            return failed(format!("Nothing was set: {}.", wrong.join("; ")));
        }
        let values = rows.iter().map(|r| (r.id, r.asked)).collect();
        let what = if single {
            "set the plugin parameter"
        } else {
            "set the plugin parameters"
        };
        let d = match self.ask(AiAsk::SetPluginParams { place, values }).await {
            Ok(d) => d,
            Err(result) => return result,
        };
        let Some(ticket) = d.ticket else {
            return closing();
        };
        let set = match self
            .params_answer(AiAsk::ParamsSet { ticket }, WAIT_FOR)
            .await
        {
            Ok(AiParams::Set(set)) => set,
            Ok(_) => return closing(),
            Err(_) => {
                return failed(format!(
                    "Could not {what} as a step: the plugin was told, but it did not give its \
                     state within {} s, so the song does not hold the values yet. If its step \
                     comes later it shows in the undo history (song_overview).",
                    WAIT_FOR.as_secs()
                ));
            }
        };
        set_answer(what, info, rows, set, single)
    }

    /// Sets parameters of a built-in effect as one step and says what came of it.
    async fn set_builtin(
        &self,
        strip: StripId,
        slot: usize,
        insert: &Insert,
        values: Vec<(u32, f32)>,
        single: bool,
    ) -> CallToolResult {
        let before: Vec<(u32, f32, Option<f32>)> = (values.iter())
            .map(|&(i, v)| (i, v, insert.value(i)))
            .collect();
        let ask = AiAsk::SetEffectParams {
            strip,
            slot,
            values,
        };
        let d = match self.ask(ask).await {
            Ok(d) => d,
            Err(result) => return result,
        };
        let changed = d.step.is_some();
        let mut answer = match self.builtin_answer(strip, slot, &before, changed).await {
            Ok(answer) => answer,
            Err(result) => return result,
        };
        answer["step"] = json!(d.step);
        answer["undo_steps"] = json!(d.undo_count);
        if single {
            // The fields set_param always had.
            for k in ["clip", "take", "track"] {
                answer[k] = Value::Null;
            }
            answer = flatten(answer);
        }
        let what = if changed {
            "Done: set the effect parameters."
        } else {
            "Nothing changed: every parameter had that value already; no step was made."
        };
        done(format!("{what}\n{answer}"), answer)
    }

    /// The values of a built-in effect's parameters after a set, against what they were.
    async fn builtin_answer(
        &self,
        strip: StripId,
        slot: usize,
        before: &[(u32, f32, Option<f32>)],
        changed: bool,
    ) -> Result<Value, CallToolResult> {
        let read = self.read().await?;
        let insert = (read.song.doc.insert(strip, slot)).ok_or_else(closing)?;
        let kind = insert.kind().ok_or_else(closing)?;
        let rows: Vec<Value> = (before.iter())
            .filter_map(|&(index, asked, was)| {
                let spec = kind.param(index)?;
                let now = insert.value(index)?;
                Some(json!({
                    "index": index, "param": spec.name, "asked": asked,
                    "value": now, "text": builtin_text(spec, now),
                    "before": was.map(|was| json!({ "value": was, "text": builtin_text(spec, was) })),
                    "changed": was != Some(now),
                    "unit": unit_name(spec.unit),
                }))
            })
            .collect();
        Ok(json!({
            "effect": kind.name(), "changed": changed, "in_song": true,
            "undo_steps": read.undo_count, "params": rows,
        }))
    }
}

/// The text of a tool's answer.
fn tool_text(result: &CallToolResult) -> String {
    (result.content.first())
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// What a tool says of a set of plugin parameters that ended.
fn set_answer(
    what: &str,
    info: &PluginInfo,
    mut rows: Vec<SetRow>,
    set: AiParamsSet,
    single: bool,
) -> CallToolResult {
    for row in &mut rows {
        if let Some(now) = set.params.iter().find(|p| p.id == row.id) {
            row.now = (now.value, now.text.clone());
        }
    }
    let moved: Vec<&str> = rows
        .iter()
        .filter(|r| r.moved())
        .map(|r| r.name.as_str())
        .collect();
    let unchanged: Vec<&str> = rows
        .iter()
        .filter(|r| !r.moved())
        .map(|r| r.name.as_str())
        .collect();
    let (step, changed, in_song) = match &set.end {
        AiSetEnd::Step(row) => (json!(row), true, true),
        AiSetEnd::Same => (Value::Null, false, moved.is_empty()),
        _ => (Value::Null, false, false),
    };
    let mut answer = json!({
        "plugin": info.value(),
        "changed": changed,
        "in_song": in_song,
        "step": step,
        "undo_steps": set.undo_count,
        "params": rows.iter().map(SetRow::value).collect::<Vec<_>>(),
        "unchanged": unchanged,
    });
    if single {
        answer = flatten(answer);
        if let Some(map) = answer.as_object_mut() {
            map.remove("unchanged");
        }
    }
    let why = match &set.end {
        AiSetEnd::Step(_) => return done(format!("Done: {what}.\n{answer}"), answer),
        AiSetEnd::Same if moved.is_empty() => {
            let said = "Nothing changed: the plugin had those values already; no step was made.";
            return done(format!("{said}\n{answer}"), answer);
        }
        AiSetEnd::Same => format!(
            "the plugin took them ({} shows another value now) but its saved state did not \
             change, so the song does not hold them: undo and saving miss them. The plugin may \
             not keep {} in its state, or meloet's sound is not running (no output), so the \
             plugin has not taken them in yet",
            moved.join(", "),
            if moved.len() == 1 {
                "this parameter"
            } else {
                "these parameters"
            },
        ),
        AiSetEnd::NoState => "the plugin did not give its state (it stopped or does not \
             answer), so the song does not hold the values; the plugin may sound with them \
             until it is loaded again"
            .into(),
        AiSetEnd::Overtaken => "the song's own state was put back into the plugin first (an \
             undo, a redo or a step from the history), so the values are gone again and the \
             song is as it was"
            .into(),
        AiSetEnd::Gone => "the plugin is not at that place any more".into(),
    };
    failed(format!("Could not {what} as a step: {why}.\n{answer}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(names: &[(usize, &str)]) -> (Vec<f64>, Vec<Option<String>>) {
        let total: usize = names.iter().map(|(n, _)| n).sum();
        let values = probe_values(total as u32);
        let texts = (names.iter())
            .flat_map(|(n, t)| std::iter::repeat_n(Some((*t).to_owned()), *n))
            .collect();
        (values, texts)
    }

    #[test]
    fn a_scan_lists_the_names_where_the_text_changes() {
        let (values, texts) =
            texts(&[(50, "Classic"), (90, "Sine"), (90, "Wavetable"), (51, "FM")]);
        let Scanned::List(list) = scan(&values, &texts, 64) else {
            panic!("a list")
        };
        let names: Vec<&str> = list.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, ["Classic", "Sine", "Wavetable", "FM"]);
        // The first and the last at the ends; the others inside their stretch.
        assert_eq!(list[0].0, 0.0);
        assert_eq!(list[3].0, 1.0);
        for (i, (value, _)) in list.iter().enumerate() {
            let at = (value * (values.len() - 1) as f64).round() as usize;
            assert_eq!(texts[at].as_deref(), Some(names[i]), "{value}");
        }
    }

    #[test]
    fn a_text_that_follows_the_value_is_no_list() {
        let values = probe_values(1001);
        let texts: Vec<Option<String>> = values.iter().map(|v| Some(format!("{v:.2}"))).collect();
        assert_eq!(scan(&values, &texts, 64), Scanned::Smooth(101));
        let none = vec![None; values.len()];
        assert_eq!(scan(&values, &none, 64), Scanned::NoTexts);
    }

    #[test]
    fn numbers_and_names() {
        for number in ["0.00", "-12.5 dB", "+3", "−inf dB", ".5", "4.000000  ", "∞"] {
            assert!(starts_with_number(number), "{number}");
        }
        for name in ["Classic", "LP 12 dB", "Off", "Key Split"] {
            assert!(!starts_with_number(name), "{name}");
        }
    }

    #[test]
    fn built_in_values_as_words_and_back() {
        let delay = EffectKind::Delay;
        let (step, spec) = builtin_param(delay, &json!("intervalStep")).unwrap();
        assert_eq!(step, 0);
        assert_eq!(builtin_text(spec, 2.0), "eighth");
        assert_eq!(builtin_value(spec, None, Some("Sixteenth")), Ok(4.0));
        assert!(builtin_value(spec, None, Some("whole")).is_err());
        let (_, sync) = builtin_param(delay, &json!(2)).unwrap();
        assert_eq!(builtin_text(sync, 1.0), "on");
        assert_eq!(builtin_value(sync, None, Some("off")), Ok(0.0));
        let (_, high) = builtin_param(delay, &json!("highCut")).unwrap();
        assert_eq!(builtin_text(high, 5000.0), "5000 Hz");
        assert_eq!(builtin_value(high, None, Some("2 kHz")), Ok(2000.0));
        assert!(builtin_value(high, Some(1.0), Some("2 kHz")).is_err());
        let (_, mix) = builtin_param(EffectKind::Reverb, &json!("decay")).unwrap();
        assert_eq!(builtin_text(mix, 2.4), "2.4 s");
    }
}
