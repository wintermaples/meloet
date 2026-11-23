//! Surge XT's patch XML: its parameters, their modulation routings, and edits that change
//! only the bytes they mean (docs/research/2026-11-23-plugin-presets-state.md Q4).
//!
//! `<patch revision=…>` holds `<parameters>`, one element a parameter named by Surge's own
//! name (`a_pitch`, `a_lfo0_rate`, `fx1_type` …) with `type` (0 a whole number, 2 a float),
//! `value` in the parameter's own units, and attributes such as `deactivated`. A routing is a
//! `<modrouting source depth muted source_index [source_scene]/>` inside the element of the
//! parameter it moves; Surge reads routings only on float parameters. Each edit parses the
//! text again, so edits can follow one another.

use std::borrow::Cow;

use super::sources;
use super::xml::{self, Element};

/// Routings one parameter may hold, and parameter elements an edit may name.
const MOST_ROUTINGS: usize = 64;

pub struct Patch<'t> {
    pub text: &'t str,
    root: Element,
    params: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    pub revision: Option<i64>,
    pub name: Option<String>,
    pub category: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Routing {
    pub target: String,
    pub source: i64,
    pub depth: f64,
    pub muted: bool,
    pub source_index: i64,
    pub source_scene: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    /// `type`: 0 a whole number, 2 a float.
    pub kind: Option<i64>,
    pub value: Option<String>,
    /// The other attributes, as written.
    pub attrs: Vec<(String, String)>,
    pub routings: Vec<Routing>,
}

impl Param {
    pub fn number(&self) -> Option<f64> {
        self.value.as_deref()?.trim().parse().ok()
    }

    pub fn flag(&self, name: &str) -> Option<bool> {
        let (_, v) = self.attrs.iter().find(|(n, _)| n == name)?;
        Some(v.trim() == "1")
    }
}

/// Whose scene a parameter is: 0 for scene A (`a_…`), 1 for scene B, `None` for the patch
/// as a whole (whose routings say the scene of their source).
pub fn scene_of(name: &str) -> Option<i64> {
    match name.get(..2) {
        Some("a_") => Some(0),
        Some("b_") => Some(1),
        _ => None,
    }
}

fn number<T: std::str::FromStr>(el: &Element, text: &str, name: &str) -> Option<T> {
    el.value(text, name)?.trim().parse().ok()
}

impl<'t> Patch<'t> {
    pub fn parse(text: &'t str) -> Result<Patch<'t>, String> {
        let root =
            xml::parse(text).map_err(|e| format!("the patch is not well-formed XML: {e}"))?;
        if root.name(text) != "patch" {
            return Err("the XML is not a Surge XT patch: its root is not <patch>".into());
        }
        let mut found =
            (root.children.iter().enumerate()).filter(|(_, c)| c.name(text) == "parameters");
        let (Some((params, _)), None) = (found.next(), found.next()) else {
            return Err("the patch has no <parameters> (or more than one)".into());
        };
        Ok(Patch { text, root, params })
    }

    fn params_el(&self) -> &Element {
        &self.root.children[self.params]
    }

    pub fn meta(&self) -> Meta {
        let text = self.text;
        let meta = self.root.child(text, "meta");
        let attr = |name| meta.and_then(|m| m.value(text, name)).map(Cow::into_owned);
        Meta {
            revision: number(&self.root, text, "revision"),
            name: attr("name"),
            category: attr("category"),
            author: attr("author"),
        }
    }

    fn element(&self, name: &str) -> Option<&Element> {
        self.params_el().child(self.text, name)
    }

    fn read(&self, el: &Element) -> Param {
        let text = self.text;
        let name = el.name(text).to_owned();
        let attrs = (el.attrs.iter())
            .map(|a| (&text[a.name.clone()], xml::unescape(&text[a.value.clone()])))
            .filter(|(n, _)| *n != "type" && *n != "value")
            .map(|(n, v)| (n.to_owned(), v.into_owned()))
            .collect();
        let routings = (el.children.iter())
            .filter(|c| c.name(text) == "modrouting")
            .map(|r| Routing {
                target: name.clone(),
                source: number(r, text, "source").unwrap_or(-1),
                depth: number(r, text, "depth").unwrap_or(f64::NAN),
                muted: number::<i64>(r, text, "muted").unwrap_or(0) != 0,
                source_index: number(r, text, "source_index").unwrap_or(0),
                source_scene: number(r, text, "source_scene"),
            })
            .collect();
        Param {
            kind: number(el, text, "type"),
            value: el.value(text, "value").map(Cow::into_owned),
            attrs,
            routings,
            name,
        }
    }

    pub fn params(&self) -> Vec<Param> {
        (self.params_el().children.iter())
            .map(|el| self.read(el))
            .collect()
    }

    pub fn param(&self, name: &str) -> Option<Param> {
        self.element(name).map(|el| self.read(el))
    }

    pub fn routings(&self) -> Vec<Routing> {
        (self.params_el().children.iter())
            .flat_map(|el| self.read(el).routings)
            .collect()
    }

    /// The parameter element `name`, or why not, with names near it.
    fn wanted(&self, name: &str) -> Result<&Element, String> {
        self.element(name).ok_or_else(|| {
            let names: Vec<&str> = (self.params_el().children.iter())
                .map(|c| c.name(self.text))
                .collect();
            format!(
                "the patch has no parameter \"{name}\"; near: {}",
                near(name, &names).join(", ")
            )
        })
    }
}

/// Up to 8 names nearest to `name` (those holding it first, then by edit distance).
pub fn near<'a>(name: &str, names: &[&'a str]) -> Vec<&'a str> {
    let lower = name.to_lowercase();
    let mut scored: Vec<(usize, &str)> = (names.iter())
        .map(|n| {
            let score = match n.to_lowercase().contains(&lower) {
                true => n.len().saturating_sub(lower.len()) / 8,
                false => 100 + distance(&lower, &n.to_lowercase()),
            };
            (score, *n)
        })
        .collect();
    scored.sort();
    scored.into_iter().take(8).map(|(_, n)| n).collect()
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut last = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = (last + usize::from(ca != *cb))
                .min(row[j] + 1)
                .min(here + 1);
            last = here;
        }
    }
    row[b.len()]
}

/// As Surge XT 1.3 writes a depth: the shortest text of its 32-bit float.
fn depth_text(depth: f64) -> String {
    format!("{}", depth as f32)
}

/// The attribute is there with the same number, as Surge XT keeps it (32-bit floats): an
/// edit to it would change nothing the plugin keeps.
fn same_number(el: &Element, text: &str, name: &str, value: f64) -> bool {
    number::<f64>(el, text, name).is_some_and(|old| old as f32 == value as f32)
}

fn same_flag(el: &Element, text: &str, name: &str, on: bool) -> bool {
    el.value(text, name)
        .is_some_and(|v| v.trim() == if on { "1" } else { "0" })
}

/// What Surge XT allows when a routing is made in its window (it does not check a patch it
/// reads): voice sources move only parameters of a scene, no LFO moves itself, only
/// scene-level sources move a scene LFO, the two envelopes do not move envelopes, and
/// Keytrack does not move a scene's pitch. Whether a parameter can be moved at all is not
/// known here: the plugin's state after the change shows it.
fn can_route(source: u32, target: &str) -> Result<(), String> {
    let Some(s) = sources::by_id(source) else {
        return Err(format!("there is no modulation source {source}"));
    };
    let why = if scene_of(target).is_none() && !s.scene_level() {
        Some(
            "a parameter of the patch as a whole takes only sources with one value for the \
              whole scene (macros, scene LFOs, wheels and pedals, channel aftertouch, pitch \
              bend, lowest/highest/latest key)",
        )
    } else if let Some((lfo, _)) = lfo_part(target)
        && sources::lfo_of(source) == Some(lfo as u32)
    {
        Some("an LFO cannot move its own parameters")
    } else if lfo_part(target).is_some_and(|(lfo, _)| lfo >= 6) && !s.scene_level() {
        Some("a scene LFO takes only sources with one value for the whole scene")
    } else if (target.get(2..)).is_some_and(|t| t.starts_with("env")) && matches!(source, 15 | 16) {
        Some("the envelopes cannot move envelope parameters")
    } else if (target == "a_pitch" || target == "b_pitch") && source == 2 {
        Some("Keytrack cannot move a scene's pitch")
    } else {
        None
    };
    match why {
        Some(why) => Err(format!(
            "Surge XT does not allow {} on {target}: {why}",
            s.name
        )),
        None => Ok(()),
    }
}

/// Adds the routing from `source` to `target` with `depth`, or sets the depth (and `muted`,
/// when given) of the one there. `scene`: the scene of the source (0 A, 1 B) for a target of
/// the patch as a whole; not written for a target of a scene.
pub fn set_routing(
    text: &str,
    target: &str,
    source: u32,
    depth: f64,
    muted: Option<bool>,
    scene: Option<i64>,
) -> Result<String, String> {
    if !depth.is_finite() {
        return Err("the depth must be a number".into());
    }
    can_route(source, target)?;
    let patch = Patch::parse(text)?;
    let el = patch.wanted(target)?;
    if number::<i64>(el, text, "type") != Some(2) {
        return Err(format!(
            "\"{target}\" is not a float parameter (type=\"2\"); Surge XT reads routings only \
             on those"
        ));
    }
    let global = scene_of(target).is_none();
    let scene = if global {
        Some(scene.unwrap_or(0))
    } else {
        None
    };
    let same = |r: &&Element| {
        r.name(text) == "modrouting"
            && number::<i64>(r, text, "source") == Some(i64::from(source))
            && number::<i64>(r, text, "source_index").unwrap_or(0) == 0
            && (!global
                || number::<i64>(r, text, "source_scene").unwrap_or(0) == scene.unwrap_or(0))
    };
    let mut edits = Vec::new();
    if let Some(r) = el.children.iter().find(same) {
        if !same_number(r, text, "depth", depth) {
            edits.push(attr_edit(r, text, "depth", &depth_text(depth)));
        }
        if let Some(muted) = muted
            && !same_flag(r, text, "muted", muted)
        {
            edits.push(attr_edit(r, text, "muted", if muted { "1" } else { "0" }));
        }
    } else {
        let count = (el.children.iter())
            .filter(|c| c.name(text) == "modrouting")
            .count();
        if count >= MOST_ROUTINGS {
            return Err(format!("\"{target}\" has {count} routings already"));
        }
        let mut new = format!(
            "<modrouting source=\"{source}\" depth=\"{}\" muted=\"{}\" source_index=\"0\"",
            depth_text(depth),
            u8::from(muted.unwrap_or(false))
        );
        if let Some(scene) = scene {
            new.push_str(&format!(" source_scene=\"{scene}\""));
        }
        new.push_str(" />");
        edits.push(match &el.end {
            Some(end) => (end.start..end.start, new),
            None => {
                let name = el.name(text);
                (el.attr_place()..el.start.end, format!(">{new}</{name}>"))
            }
        });
    }
    Ok(xml::apply(text, edits))
}

/// The edit that gives `el` the attribute `name` with `value` (escaped here).
fn attr_edit(
    el: &Element,
    text: &str,
    name: &str,
    value: &str,
) -> (std::ops::Range<usize>, String) {
    let value = xml::escape(value);
    match el.attr(text, name) {
        Some(a) => (a.value.clone(), value),
        None => {
            let at = el.attr_place();
            (at..at, format!(" {name}=\"{value}\""))
        }
    }
}

/// Takes away the routings of `target` from `source` (every routing of it when `None`).
/// Gives the text and how many went.
pub fn remove_routings(
    text: &str,
    target: &str,
    source: Option<u32>,
) -> Result<(String, usize), String> {
    let patch = Patch::parse(text)?;
    let el = patch.wanted(target)?;
    let edits: Vec<_> = (el.children.iter())
        .filter(|r| r.name(text) == "modrouting")
        .filter(|r| source.is_none_or(|s| number::<i64>(r, text, "source") == Some(i64::from(s))))
        .map(|r| (r.span(), String::new()))
        .collect();
    let gone = edits.len();
    Ok((xml::apply(text, edits), gone))
}

/// A change of one parameter element: its value, and attributes set to 0 or 1.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParamEdit {
    pub name: String,
    pub value: Option<f64>,
    pub flags: Vec<(&'static str, bool)>,
}

/// The attributes an edit may set.
pub const FLAGS: [&str; 4] = ["deactivated", "temposync", "extend_range", "absolute"];

pub fn set_param(text: &str, edit: &ParamEdit) -> Result<String, String> {
    let patch = Patch::parse(text)?;
    let name = edit.name.as_str();
    let el = patch.wanted(name)?;
    let kind: Option<i64> = number(el, text, "type");
    let mut edits = Vec::new();
    if let Some(value) = edit.value {
        if !value.is_finite() {
            return Err(format!("{name}: the value must be a number"));
        }
        let written = match kind {
            Some(0) if value.fract() != 0.0 || value.abs() > 1e9 => {
                return Err(format!("{name} takes a whole number (type=\"0\")"));
            }
            Some(0) => format!("{}", value as i64),
            Some(2) => format!("{value:.14}"),
            _ => return Err(format!("{name} has no type Surge XT reads a value of")),
        };
        if let Some((low, high)) = range_of(name)
            && !(low..=high).contains(&value)
        {
            return Err(format!(
                "{name} is {low} to {high} (in the patch's own units)"
            ));
        }
        if !same_number(el, text, "value", value) {
            edits.push(attr_edit(el, text, "value", &written));
        }
    }
    for (at, (flag, on)) in edit.flags.iter().enumerate() {
        if !FLAGS.contains(flag) {
            return Err(format!("{flag} is not an attribute this edit sets"));
        }
        if edit.flags[..at].iter().any(|(f, _)| f == flag) {
            return Err(format!("{name}: {flag} is given twice"));
        }
        if !same_flag(el, text, flag, *on) {
            edits.push(attr_edit(el, text, flag, if *on { "1" } else { "0" }));
        }
    }
    Ok(xml::apply(text, edits))
}

/// The patch named `name` in its `<meta>` (as Surge XT names a patch it saves).
pub fn set_name(text: &str, name: &str) -> Result<String, String> {
    let patch = Patch::parse(text)?;
    let Some(meta) = patch.root.child(text, "meta") else {
        return Err("the patch has no <meta>".into());
    };
    Ok(xml::apply(text, vec![attr_edit(meta, text, "name", name)]))
}

/// What a parameter of an LFO is: (LFO index 0..=11, field).
fn lfo_part(name: &str) -> Option<(usize, &str)> {
    scene_of(name)?;
    let rest = name[2..].strip_prefix("lfo")?;
    let (index, field) = rest.split_once('_')?;
    let index: usize = index.parse().ok()?;
    (index < 12).then_some((index, field))
}

/// What the value of a parameter means where meloet knows it: rates are 2^value Hz and an
/// LFO's envelope times 2^value seconds (Surge XT's `ATwoToTheBx` display).
pub fn unit_of(name: &str) -> Option<&'static str> {
    match lfo_part(name)?.1 {
        "rate" => Some("Hz"),
        "delay" | "attack" | "hold" | "decay" | "release" => Some("s"),
        _ => None,
    }
}

/// The range a value of a known kind may take, in the patch's own units.
pub fn range_of(name: &str) -> Option<(f64, f64)> {
    match unit_of(name)? {
        "Hz" => Some((-7.0, 9.0)),
        _ => Some((-8.0, 5.0)),
    }
}

/// The value in hertz or seconds (`None` for a tempo-synced rate: that is a note length).
pub fn natural(name: &str, value: f64, temposync: bool) -> Option<(f64, &'static str)> {
    let unit = unit_of(name)?;
    if unit == "Hz" && temposync {
        return None;
    }
    Some((value.exp2(), unit))
}

/// The patch's own value of a time or rate given in seconds or hertz.
pub fn from_natural(name: &str, natural: f64, unit: &str) -> Result<f64, String> {
    match unit_of(name) {
        Some(u) if u == unit && natural > 0.0 && natural.is_finite() => Ok(natural.log2()),
        Some(u) if u == unit => Err(format!("{name}: give more than 0 {unit}")),
        _ => Err(format!(
            "{name} is not given in {unit}; give `value` in the patch's own units"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\
        <patch revision=\"24\"><meta name=\"Init &amp; Saw\" category=\"Templates\" \
        author=\"x\"><tags /></meta><parameters><volume type=\"2\" value=\"0.0\" />\
        <a_pitch type=\"2\" value=\"0.00000000000000\" extend_range=\"0\" />\
        <a_osc1_type type=\"0\" value=\"0\" />\
        <a_lfo0_rate type=\"2\" value=\"0.00000000000000\" deactivated=\"0\" />\
        <a_lfo0_delay type=\"2\" value=\"-8.00000000000000\" />\
        <a_vca_level type=\"2\" value=\"0.0\"><modrouting source=\"1\" depth=\"-9.770000\" \
        muted=\"0\" source_index=\"0\" /></a_vca_level></parameters><unknown x=\"1\"/></patch>";

    #[test]
    fn it_reads_parameters_and_routings() {
        let patch = Patch::parse(PATCH).unwrap();
        let meta = patch.meta();
        assert_eq!(meta.revision, Some(24));
        assert_eq!(meta.name.as_deref(), Some("Init & Saw"));
        assert_eq!(patch.params().len(), 6);
        let pitch = patch.param("a_pitch").unwrap();
        assert_eq!(pitch.kind, Some(2));
        assert_eq!(pitch.number(), Some(0.0));
        assert_eq!(pitch.flag("extend_range"), Some(false));
        let routings = patch.routings();
        assert_eq!(routings.len(), 1);
        assert_eq!(routings[0].target, "a_vca_level");
        assert_eq!(routings[0].depth, -9.77);
        assert!(Patch::parse("<patch/>").is_err());
        assert!(Patch::parse("<other><parameters/></other>").is_err());
    }

    #[test]
    fn a_routing_is_added_changed_and_taken_away_by_its_bytes_alone() {
        let added = set_routing(PATCH, "a_pitch", 17, 0.5, None, None).unwrap();
        let expected = PATCH.replace(
            "extend_range=\"0\" />",
            "extend_range=\"0\"><modrouting source=\"17\" depth=\"0.5\" muted=\"0\" \
             source_index=\"0\" /></a_pitch>",
        );
        assert_eq!(added, expected);
        // What Surge XT keeps already (as a 32-bit float) is no change.
        assert_eq!(
            set_routing(&added, "a_pitch", 17, 0.500000001, Some(false), None).unwrap(),
            added
        );
        let rate = ParamEdit {
            name: "a_lfo0_rate".into(),
            value: Some(0.0),
            flags: vec![("deactivated", false)],
        };
        assert_eq!(set_param(PATCH, &rate).unwrap(), PATCH);
        let changed = set_routing(&added, "a_pitch", 17, -0.25, Some(true), None).unwrap();
        assert_eq!(
            changed,
            expected.replace("depth=\"0.5\" muted=\"0\"", "depth=\"-0.25\" muted=\"1\"")
        );
        // Another source beside it, in an element that has an end tag.
        let two = set_routing(&changed, "a_pitch", 6, 1.0, None, None).unwrap();
        let patch = Patch::parse(&two).unwrap();
        let sources: Vec<i64> = patch
            .param("a_pitch")
            .unwrap()
            .routings
            .iter()
            .map(|r| r.source)
            .collect();
        assert_eq!(sources, [17, 6]);
        let (one, gone) = remove_routings(&two, "a_pitch", Some(17)).unwrap();
        assert_eq!(gone, 1);
        let (none, gone) = remove_routings(&one, "a_pitch", None).unwrap();
        assert_eq!(gone, 1);
        assert_eq!(
            none,
            PATCH.replace("extend_range=\"0\" />", "extend_range=\"0\"></a_pitch>")
        );
        // The patch as a whole: the routing says its source's scene.
        let global = set_routing(PATCH, "volume", 23, 0.1, None, Some(1)).unwrap();
        assert!(global.contains("<volume type=\"2\" value=\"0.0\"><modrouting source=\"23\" depth=\"0.1\" muted=\"0\" source_index=\"0\" source_scene=\"1\" /></volume>"), "{global}");
    }

    #[test]
    fn routings_go_only_on_float_parameters_that_are_there() {
        let not_float = set_routing(PATCH, "a_osc1_type", 17, 1.0, None, None).unwrap_err();
        assert!(not_float.contains("not a float"), "{not_float}");
        let missing = set_routing(PATCH, "a_ptch", 17, 1.0, None, None).unwrap_err();
        assert!(missing.contains("near: a_pitch"), "{missing}");
        assert!(set_routing(PATCH, "a_pitch", 17, f64::NAN, None, None).is_err());
        for (target, source, says) in [
            ("volume", 17, "whole scene"),
            ("a_lfo0_rate", 17, "its own"),
            ("a_pitch", 2, "Keytrack"),
            ("a_pitch", 41, "no modulation source"),
        ] {
            let why = set_routing(PATCH, target, source, 1.0, None, None).unwrap_err();
            assert!(why.contains(says), "{target} {source}: {why}");
        }
        assert!(set_routing(PATCH, "a_lfo0_rate", 18, 1.0, None, None).is_ok());
        assert!(remove_routings(PATCH, "nope", None).is_err());
    }

    #[test]
    fn parameters_get_values_and_attributes() {
        let edit = ParamEdit {
            name: "a_lfo0_delay".into(),
            value: Some(-1.0),
            flags: vec![("deactivated", false), ("temposync", true)],
        };
        let out = set_param(PATCH, &edit).unwrap();
        assert!(out.contains("<a_lfo0_delay type=\"2\" value=\"-1.00000000000000\" deactivated=\"0\" temposync=\"1\" />"), "{out}");
        let rate = ParamEdit {
            name: "a_lfo0_rate".into(),
            value: Some(10.0),
            ..Default::default()
        };
        assert!(set_param(PATCH, &rate).unwrap_err().contains("-7 to 9"));
        let whole = ParamEdit {
            name: "a_osc1_type".into(),
            value: Some(2.5),
            ..Default::default()
        };
        assert!(set_param(PATCH, &whole).is_err());
        let whole = ParamEdit {
            name: "a_osc1_type".into(),
            value: Some(2.0),
            flags: vec![("deactivated", true)],
        };
        assert!(
            set_param(PATCH, &whole)
                .unwrap()
                .contains("<a_osc1_type type=\"0\" value=\"2\" deactivated=\"1\" />")
        );
    }

    #[test]
    fn units() {
        assert_eq!(natural("a_lfo0_rate", 2.0, false), Some((4.0, "Hz")));
        assert_eq!(natural("a_lfo0_rate", 2.0, true), None);
        assert_eq!(natural("b_lfo11_delay", -1.0, false), Some((0.5, "s")));
        assert_eq!(natural("a_lfo0_sustain", 1.0, false), None);
        assert_eq!(natural("a_lfo12_rate", 1.0, false), None);
        assert_eq!(from_natural("a_lfo0_delay", 0.5, "s"), Ok(-1.0));
        assert!(from_natural("a_lfo0_delay", 0.5, "Hz").is_err());
        assert!(from_natural("a_pitch", 0.5, "s").is_err());
        assert_eq!(
            near("lfo0_rat", &["a_lfo0_rate", "b_lfo0_rate", "volume"])[..2],
            ["a_lfo0_rate", "b_lfo0_rate"]
        );
    }
}
