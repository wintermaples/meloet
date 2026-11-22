//! The kinds the shelf sorts plugins under, from what each plugin says about itself (spec 3,
//! 「何百個のプラグイン」 14-A). The order of the arms is the order of the shelf's headings, and
//! a plugin that names several kinds goes under the first one in that order.

use serde::{Deserialize, Serialize};

/// 音源 or エフェクト. Decided from the plugin's own description, never from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginRole {
    Instrument,
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectCategory {
    Eq,
    Dynamics,
    Reverb,
    Delay,
    Modulation,
    Distortion,
    Filter,
    Pitch,
    Spatial,
    Analysis,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstrumentCategory {
    Synth,
    Sampler,
    Drum,
    /// 「ピアノ ・ 鍵盤」.
    Keys,
    Other,
}

impl EffectCategory {
    pub const ALL: [EffectCategory; 11] = [
        EffectCategory::Eq,
        EffectCategory::Dynamics,
        EffectCategory::Reverb,
        EffectCategory::Delay,
        EffectCategory::Modulation,
        EffectCategory::Distortion,
        EffectCategory::Filter,
        EffectCategory::Pitch,
        EffectCategory::Spatial,
        EffectCategory::Analysis,
        EffectCategory::Other,
    ];
}

impl InstrumentCategory {
    pub const ALL: [InstrumentCategory; 5] = [
        InstrumentCategory::Synth,
        InstrumentCategory::Sampler,
        InstrumentCategory::Drum,
        InstrumentCategory::Keys,
        InstrumentCategory::Other,
    ];
}

/// The role of a plugin and its kind within that role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "role", content = "category", rename_all = "camelCase")]
pub enum PluginKind {
    Instrument(InstrumentCategory),
    Effect(EffectCategory),
}

impl PluginKind {
    pub fn role(self) -> PluginRole {
        match self {
            PluginKind::Instrument(_) => PluginRole::Instrument,
            PluginKind::Effect(_) => PluginRole::Effect,
        }
    }

    /// From the classes an LV2 plugin is `a` member of: full URIs
    /// (`http://lv2plug.in/ns/lv2core#CompressorPlugin`), `lv2:CompressorPlugin` or the bare
    /// name. The classes above them need not be given. LV2 has no word for the kind of an
    /// instrument, so every LV2 instrument is `Other`.
    pub fn from_lv2<S: AsRef<str>>(classes: &[S]) -> PluginKind {
        let is = |name: &str| {
            classes
                .iter()
                .any(|c| lv2_is_a(lv2_local(c.as_ref()), name))
        };
        if is("InstrumentPlugin") {
            return PluginKind::Instrument(InstrumentCategory::Other);
        }
        let category = LV2_EFFECTS
            .iter()
            .find(|(names, _)| names.iter().any(|n| is(n)))
            .map_or(EffectCategory::Other, |(_, c)| *c);
        PluginKind::Effect(category)
    }

    /// From the sub-categories of a VST3 Audio Module Class, as one string ("Fx|Dynamics") or
    /// already split ("Fx", "Dynamics", "Stereo" in `moduleinfo.json`).
    pub fn from_vst3<S: AsRef<str>>(sub_categories: &[S]) -> PluginKind {
        let words: Vec<&str> = sub_categories
            .iter()
            .flat_map(|s| s.as_ref().split('|'))
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .collect();
        let has = |word: &str| words.contains(&word);
        // "Fx|Instrument" is an effect that may also be put where an instrument goes.
        if has("Instrument") && !has("Fx") {
            let category = VST3_INSTRUMENTS
                .iter()
                .find(|(word, _)| has(word))
                .map_or(InstrumentCategory::Other, |(_, c)| *c);
            return PluginKind::Instrument(category);
        }
        let category = VST3_EFFECTS
            .iter()
            .find(|(word, _)| has(word))
            .map_or(EffectCategory::Other, |(_, c)| *c);
        PluginKind::Effect(category)
    }
}

/// Spec 3, in the order of the headings. EQ stands before Filter and Reverb before Delay, so a
/// class finds its own heading before that of the class above it.
const LV2_EFFECTS: [(&[&str], EffectCategory); 10] = [
    (
        &["EQPlugin", "ParaEQPlugin", "MultiEQPlugin"],
        EffectCategory::Eq,
    ),
    (
        &[
            "DynamicsPlugin",
            "CompressorPlugin",
            "LimiterPlugin",
            "GatePlugin",
            "ExpanderPlugin",
        ],
        EffectCategory::Dynamics,
    ),
    (&["ReverbPlugin"], EffectCategory::Reverb),
    (&["DelayPlugin"], EffectCategory::Delay),
    (
        &[
            "ModulatorPlugin",
            "ChorusPlugin",
            "FlangerPlugin",
            "PhaserPlugin",
        ],
        EffectCategory::Modulation,
    ),
    (
        &["DistortionPlugin", "WaveshaperPlugin"],
        EffectCategory::Distortion,
    ),
    (
        &[
            "FilterPlugin",
            "LowpassPlugin",
            "HighpassPlugin",
            "BandpassPlugin",
            "CombPlugin",
            "AllpassPlugin",
        ],
        EffectCategory::Filter,
    ),
    (&["PitchPlugin"], EffectCategory::Pitch),
    (&["SpatialPlugin"], EffectCategory::Spatial),
    (&["AnalyserPlugin"], EffectCategory::Analysis),
];

/// `rdfs:subClassOf` of every class of LV2 core 1.18 (`lv2core.ttl`), `lv2:Plugin` left out.
const LV2_PARENTS: [(&str, &[&str]); 31] = [
    ("InstrumentPlugin", &["GeneratorPlugin"]),
    ("OscillatorPlugin", &["GeneratorPlugin"]),
    ("ConstantPlugin", &["GeneratorPlugin"]),
    ("ConverterPlugin", &["UtilityPlugin"]),
    ("AnalyserPlugin", &["UtilityPlugin"]),
    ("MixerPlugin", &["UtilityPlugin"]),
    ("FunctionPlugin", &["UtilityPlugin"]),
    ("ReverbPlugin", &["SimulatorPlugin", "DelayPlugin"]),
    ("PhaserPlugin", &["ModulatorPlugin"]),
    ("FlangerPlugin", &["ModulatorPlugin"]),
    ("ChorusPlugin", &["ModulatorPlugin"]),
    ("LowpassPlugin", &["FilterPlugin"]),
    ("BandpassPlugin", &["FilterPlugin"]),
    ("HighpassPlugin", &["FilterPlugin"]),
    ("CombPlugin", &["FilterPlugin"]),
    ("AllpassPlugin", &["FilterPlugin"]),
    ("EQPlugin", &["FilterPlugin"]),
    ("ParaEQPlugin", &["EQPlugin"]),
    ("MultiEQPlugin", &["EQPlugin"]),
    ("PitchPlugin", &["SpectralPlugin"]),
    ("AmplifierPlugin", &["DynamicsPlugin"]),
    ("EnvelopePlugin", &["DynamicsPlugin"]),
    ("WaveshaperPlugin", &["DistortionPlugin"]),
    ("CompressorPlugin", &["DynamicsPlugin"]),
    ("ExpanderPlugin", &["DynamicsPlugin"]),
    ("LimiterPlugin", &["DynamicsPlugin"]),
    ("GatePlugin", &["DynamicsPlugin"]),
    ("GeneratorPlugin", &[]),
    ("UtilityPlugin", &[]),
    ("SimulatorPlugin", &[]),
    ("SpectralPlugin", &[]),
];

fn lv2_local(class: &str) -> &str {
    class
        .strip_prefix("http://lv2plug.in/ns/lv2core#")
        .or_else(|| class.strip_prefix("lv2:"))
        .unwrap_or(class)
}

/// `class` is `ancestor` or below it. The table has no loops and is at most three deep.
fn lv2_is_a(class: &str, ancestor: &str) -> bool {
    class == ancestor
        || LV2_PARENTS
            .iter()
            .filter(|(name, _)| *name == class)
            .flat_map(|(_, parents)| parents.iter())
            .any(|parent| lv2_is_a(parent, ancestor))
}

const VST3_EFFECTS: [(&str, EffectCategory); 10] = [
    ("EQ", EffectCategory::Eq),
    ("Dynamics", EffectCategory::Dynamics),
    ("Reverb", EffectCategory::Reverb),
    ("Delay", EffectCategory::Delay),
    ("Modulation", EffectCategory::Modulation),
    ("Distortion", EffectCategory::Distortion),
    ("Filter", EffectCategory::Filter),
    ("Pitch Shift", EffectCategory::Pitch),
    ("Spatial", EffectCategory::Spatial),
    ("Analyzer", EffectCategory::Analysis),
];

const VST3_INSTRUMENTS: [(&str, InstrumentCategory); 4] = [
    ("Synth", InstrumentCategory::Synth),
    ("Sampler", InstrumentCategory::Sampler),
    ("Drum", InstrumentCategory::Drum),
    ("Piano", InstrumentCategory::Keys),
];

#[cfg(test)]
mod tests {
    use super::*;
    use EffectCategory as E;
    use InstrumentCategory as I;

    fn lv2(classes: &[&str]) -> PluginKind {
        PluginKind::from_lv2(classes)
    }

    fn vst3(sub_categories: &str) -> PluginKind {
        PluginKind::from_vst3(&[sub_categories])
    }

    #[test]
    fn the_order_is_the_order_of_the_shelf() {
        assert!(E::ALL.windows(2).all(|w| w[0] < w[1]));
        assert!(I::ALL.windows(2).all(|w| w[0] < w[1]));
    }

    // docs/history/research/plugin-formats.md 7: Surge XT and ZamComp as installed.
    #[test]
    fn real_plugins() {
        assert_eq!(
            lv2(&["lv2:InstrumentPlugin"]),
            PluginKind::Instrument(I::Other)
        );
        assert_eq!(vst3("Instrument|Synth"), PluginKind::Instrument(I::Synth));
        let zam = [
            "http://lv2plug.in/ns/lv2core#CompressorPlugin",
            "http://lv2plug.in/ns/lv2core#Plugin",
        ];
        assert_eq!(PluginKind::from_lv2(&zam), PluginKind::Effect(E::Dynamics));
        assert_eq!(vst3("Fx|Dynamics"), PluginKind::Effect(E::Dynamics));
        // lsp-plugins.vst3's moduleinfo.json gives the words already split.
        assert_eq!(
            PluginKind::from_vst3(&["Fx", "Dynamics", "Stereo"]),
            PluginKind::Effect(E::Dynamics)
        );
        assert_eq!(
            PluginKind::from_vst3(&[
                "Instrument",
                "Instrument",
                "Sampler",
                "Instrument",
                "Drum",
                "Stereo"
            ]),
            PluginKind::Instrument(I::Sampler)
        );
        assert_eq!(
            PluginKind::from_vst3(&["Fx", "Tools", "Mono"]),
            PluginKind::Effect(E::Other)
        );
        assert_eq!(
            PluginKind::from_vst3(&["Fx", "Analyzer"]),
            PluginKind::Effect(E::Analysis)
        );
    }

    #[test]
    fn lv2_table() {
        let rows: [(&str, E); 27] = [
            ("EQPlugin", E::Eq),
            ("ParaEQPlugin", E::Eq),
            ("MultiEQPlugin", E::Eq),
            ("DynamicsPlugin", E::Dynamics),
            ("CompressorPlugin", E::Dynamics),
            ("LimiterPlugin", E::Dynamics),
            ("GatePlugin", E::Dynamics),
            ("ExpanderPlugin", E::Dynamics),
            ("ReverbPlugin", E::Reverb),
            ("DelayPlugin", E::Delay),
            ("ModulatorPlugin", E::Modulation),
            ("ChorusPlugin", E::Modulation),
            ("FlangerPlugin", E::Modulation),
            ("PhaserPlugin", E::Modulation),
            ("DistortionPlugin", E::Distortion),
            ("WaveshaperPlugin", E::Distortion),
            ("FilterPlugin", E::Filter),
            ("LowpassPlugin", E::Filter),
            ("HighpassPlugin", E::Filter),
            ("BandpassPlugin", E::Filter),
            ("CombPlugin", E::Filter),
            ("AllpassPlugin", E::Filter),
            ("PitchPlugin", E::Pitch),
            ("SpatialPlugin", E::Spatial),
            ("AnalyserPlugin", E::Analysis),
            ("UtilityPlugin", E::Other),
            ("SimulatorPlugin", E::Other),
        ];
        for (class, category) in rows {
            assert_eq!(lv2(&[class]), PluginKind::Effect(category), "{class}");
        }
        // Below Dynamics without a row of their own in the spec: under the class above them.
        assert_eq!(lv2(&["AmplifierPlugin"]), PluginKind::Effect(E::Dynamics));
        assert_eq!(lv2(&["EnvelopePlugin"]), PluginKind::Effect(E::Dynamics));
        // Nothing said, only `lv2:Plugin`, or a class this table does not know.
        assert_eq!(lv2(&[]), PluginKind::Effect(E::Other));
        assert_eq!(lv2(&["lv2:Plugin"]), PluginKind::Effect(E::Other));
        assert_eq!(
            lv2(&["http://example.org/ns#Thing"]),
            PluginKind::Effect(E::Other)
        );
    }

    #[test]
    fn several_kinds_go_under_the_first_heading() {
        assert_eq!(
            lv2(&["DelayPlugin", "ReverbPlugin"]),
            PluginKind::Effect(E::Reverb)
        );
        assert_eq!(
            lv2(&["FilterPlugin", "EQPlugin"]),
            PluginKind::Effect(E::Eq)
        );
        assert_eq!(
            lv2(&["AnalyserPlugin", "CompressorPlugin"]),
            PluginKind::Effect(E::Dynamics)
        );
        assert_eq!(vst3("Fx|Delay|Reverb"), PluginKind::Effect(E::Reverb));
        assert_eq!(
            vst3("Instrument|Synth|Sampler"),
            PluginKind::Instrument(I::Synth)
        );
        assert_eq!(
            vst3("Instrument|Piano|Drum"),
            PluginKind::Instrument(I::Drum)
        );
    }

    #[test]
    fn vst3_table() {
        let rows = [
            ("Fx|EQ", E::Eq),
            ("Fx|Dynamics", E::Dynamics),
            ("Fx|Reverb", E::Reverb),
            ("Fx|Delay", E::Delay),
            ("Fx|Modulation", E::Modulation),
            ("Fx|Distortion", E::Distortion),
            ("Fx|Filter", E::Filter),
            ("Fx|Pitch Shift", E::Pitch),
            ("Fx|Spatial", E::Spatial),
            ("Fx|Analyzer", E::Analysis),
            ("Fx|Mastering", E::Other),
            ("Fx", E::Other),
            ("", E::Other),
            ("Fx|Instrument", E::Other),
        ];
        for (text, category) in rows {
            assert_eq!(vst3(text), PluginKind::Effect(category), "{text}");
        }
        let rows = [
            ("Instrument|Synth", I::Synth),
            ("Instrument|Sampler", I::Sampler),
            ("Instrument|Drum", I::Drum),
            ("Instrument|Piano", I::Keys),
            ("Instrument|External", I::Other),
            ("Instrument", I::Other),
        ];
        for (text, category) in rows {
            assert_eq!(vst3(text), PluginKind::Instrument(category), "{text}");
        }
    }

    #[test]
    fn kind_json_shape() {
        let json = serde_json::to_string(&PluginKind::Effect(E::Reverb)).unwrap();
        assert_eq!(json, r#"{"role":"effect","category":"reverb"}"#);
        let json = serde_json::to_string(&PluginKind::Instrument(I::Keys)).unwrap();
        assert_eq!(json, r#"{"role":"instrument","category":"keys"}"#);
    }
}
