//! The presets that come with meloet for the built-in effects (round 12, M1:
//! 「はじめから入っているもの」). A preset names the values it sets; a parameter it does not
//! name takes its default. The screen gets the table as `FACTORY_PRESETS`
//! (`ui/src/bridge/generated/presets.ts`) and names a preset `preset.<effect>.<id>`.

use crate::effects::{EffectKind, delay, reverb};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FactoryPreset {
    /// camelCase, unique within the effect.
    pub id: &'static str,
    /// A parameter's name (`ParamSpec::name`) and its value in the parameter's own unit.
    pub values: &'static [(&'static str, f32)],
}

impl FactoryPreset {
    /// Every parameter of `kind`, in the order of its table.
    pub fn values_of(&self, kind: EffectKind) -> Vec<f32> {
        (kind.params().iter())
            .map(|p| {
                (self.values.iter())
                    .find(|(name, _)| *name == p.name)
                    .map_or(p.default, |&(_, v)| p.clamp(v))
            })
            .collect()
    }
}

/// The presets of `kind`, in the order of the menu.
pub fn factory(kind: EffectKind) -> &'static [FactoryPreset] {
    match kind {
        EffectKind::Eq4 => &EQ4,
        EffectKind::Compressor => &COMPRESSOR,
        EffectKind::Reverb => &REVERB,
        EffectKind::Delay => &DELAY,
        EffectKind::Chorus => &CHORUS,
        EffectKind::Limiter => &LIMITER,
        EffectKind::Gate => &GATE,
        EffectKind::DeEsser => &DE_ESSER,
        EffectKind::Saturation => &SATURATION,
    }
}

/// The parameter a factory preset of `kind` leaves as the insert has it: the mix of a reverb
/// or a delay, which depends on the strip the insert is on (`Insert::new_on`).
pub fn left_as_is(kind: EffectKind) -> Option<u32> {
    match kind {
        EffectKind::Reverb => Some(reverb::MIX),
        EffectKind::Delay => Some(delay::MIX),
        _ => None,
    }
}

const fn preset(id: &'static str, values: &'static [(&'static str, f32)]) -> FactoryPreset {
    FactoryPreset { id, values }
}

const EQ4: [FactoryPreset; 4] = [
    preset(
        "vocalClear",
        &[
            ("lowFreq", 120.0),
            ("lowGain", -3.0),
            ("lowMidFreq", 300.0),
            ("lowMidGain", -2.0),
            ("lowMidWidth", 1.5),
            ("highMidFreq", 3000.0),
            ("highMidGain", 3.0),
            ("highMidWidth", 1.5),
            ("highFreq", 10000.0),
            ("highGain", 2.0),
        ],
    ),
    preset(
        "unmuddy",
        &[
            ("lowFreq", 100.0),
            ("lowGain", 0.0),
            ("lowMidFreq", 350.0),
            ("lowMidGain", -4.0),
            ("lowMidWidth", 1.2),
            ("highMidFreq", 3000.0),
            ("highMidGain", 0.0),
            ("highMidWidth", 1.0),
            ("highFreq", 8000.0),
            ("highGain", 0.0),
        ],
    ),
    preset(
        "cleanLows",
        &[
            ("lowFreq", 150.0),
            ("lowGain", -6.0),
            ("lowMidFreq", 250.0),
            ("lowMidGain", -2.0),
            ("lowMidWidth", 1.0),
            ("highMidFreq", 3000.0),
            ("highMidGain", 0.0),
            ("highMidWidth", 1.0),
            ("highFreq", 8000.0),
            ("highGain", 0.0),
        ],
    ),
    preset(
        "fatBass",
        &[
            ("lowFreq", 80.0),
            ("lowGain", 4.0),
            ("lowMidFreq", 700.0),
            ("lowMidGain", -2.0),
            ("lowMidWidth", 1.0),
            ("highMidFreq", 2000.0),
            ("highMidGain", 2.0),
            ("highMidWidth", 1.5),
            ("highFreq", 8000.0),
            ("highGain", 0.0),
        ],
    ),
];

const COMPRESSOR: [FactoryPreset; 4] = [
    preset(
        "vocalEven",
        &[
            ("threshold", -18.0),
            ("ratio", 3.0),
            ("attack", 5.0),
            ("release", 80.0),
            ("makeup", 4.0),
        ],
    ),
    preset(
        "drumPunch",
        &[
            ("threshold", -15.0),
            ("ratio", 4.0),
            ("attack", 20.0),
            ("release", 60.0),
            ("makeup", 3.0),
        ],
    ),
    preset(
        "bassSolid",
        &[
            ("threshold", -20.0),
            ("ratio", 5.0),
            ("attack", 10.0),
            ("release", 120.0),
            ("makeup", 4.0),
        ],
    ),
    preset(
        "gentleGlue",
        &[
            ("threshold", -12.0),
            ("ratio", 2.0),
            ("attack", 30.0),
            ("release", 200.0),
            ("makeup", 2.0),
        ],
    ),
];

const REVERB: [FactoryPreset; 3] = [
    preset(
        "smallRoom",
        &[
            ("decay", 0.6),
            ("size", 25.0),
            ("preDelay", 5.0),
            ("highCut", 8000.0),
            ("lowCut", 150.0),
            ("width", 80.0),
        ],
    ),
    preset(
        "hall",
        &[
            ("decay", 3.2),
            ("size", 85.0),
            ("preDelay", 30.0),
            ("highCut", 6000.0),
            ("lowCut", 120.0),
            ("width", 100.0),
        ],
    ),
    preset(
        "behindVocal",
        &[
            ("decay", 1.8),
            ("size", 55.0),
            ("preDelay", 60.0),
            ("highCut", 5000.0),
            ("lowCut", 250.0),
            ("width", 100.0),
        ],
    ),
];

const DELAY: [FactoryPreset; 3] = [
    preset(
        "echo",
        &[
            ("intervalStep", 0.0),
            ("intervalMs", 250.0),
            ("sync", 1.0),
            ("feedback", 40.0),
            ("highCut", 5000.0),
            ("lowCut", 200.0),
            ("pingPong", 0.0),
        ],
    ),
    preset(
        "slapback",
        &[
            ("intervalStep", 2.0),
            ("intervalMs", 100.0),
            ("sync", 0.0),
            ("feedback", 10.0),
            ("highCut", 7000.0),
            ("lowCut", 150.0),
            ("pingPong", 0.0),
        ],
    ),
    preset(
        "pingPong",
        &[
            ("intervalStep", 1.0),
            ("intervalMs", 250.0),
            ("sync", 1.0),
            ("feedback", 45.0),
            ("highCut", 6000.0),
            ("lowCut", 250.0),
            ("pingPong", 1.0),
        ],
    ),
];

const CHORUS: [FactoryPreset; 2] = [
    preset(
        "subtleWide",
        &[
            ("rate", 0.5),
            ("depth", 25.0),
            ("width", 100.0),
            ("mix", 35.0),
        ],
    ),
    preset(
        "deepSwirl",
        &[
            ("rate", 1.2),
            ("depth", 80.0),
            ("width", 100.0),
            ("mix", 50.0),
        ],
    ),
];

const LIMITER: [FactoryPreset; 2] = [
    preset(
        "finish",
        &[("ceiling", -1.0), ("gain", 3.0), ("release", 80.0)],
    ),
    preset(
        "gentle",
        &[("ceiling", -1.0), ("gain", 0.0), ("release", 200.0)],
    ),
];

const GATE: [FactoryPreset; 2] = [
    preset(
        "noiseOff",
        &[("threshold", -50.0), ("range", -60.0), ("release", 250.0)],
    ),
    preset(
        "shortTail",
        &[("threshold", -30.0), ("range", -24.0), ("release", 80.0)],
    ),
];

const DE_ESSER: [FactoryPreset; 2] = [
    preset(
        "softS",
        &[
            ("frequency", 6500.0),
            ("threshold", -32.0),
            ("reduction", 6.0),
        ],
    ),
    preset(
        "firmS",
        &[
            ("frequency", 5500.0),
            ("threshold", -38.0),
            ("reduction", 12.0),
        ],
    ),
];

const SATURATION: [FactoryPreset; 2] = [
    preset(
        "warm",
        &[
            ("drive", 4.0),
            ("tone", 9000.0),
            ("mix", 100.0),
            ("output", 0.0),
        ],
    ),
    preset(
        "rough",
        &[
            ("drive", 20.0),
            ("tone", 6000.0),
            ("mix", 70.0),
            ("output", 0.0),
        ],
    ),
];

pub fn find(kind: EffectKind, id: &str) -> Option<&'static FactoryPreset> {
    factory(kind).iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_names_parameters_of_its_effect_within_their_range() {
        for kind in EffectKind::ALL {
            let presets = factory(kind);
            for (i, preset) in presets.iter().enumerate() {
                assert!(
                    presets[..i].iter().all(|p| p.id != preset.id),
                    "{} {}: one id once",
                    kind.name(),
                    preset.id
                );
                for &(name, value) in preset.values {
                    let spec = (kind.param_index(name).and_then(|at| kind.param(at)))
                        .unwrap_or_else(|| panic!("{} {}: no {name}", kind.name(), preset.id));
                    assert_eq!(
                        spec.clamp(value),
                        value,
                        "{} {} {name}",
                        kind.name(),
                        preset.id
                    );
                }
                assert_eq!(preset.values_of(kind).len(), kind.params().len());
                assert_eq!(find(kind, preset.id), Some(preset));
            }
        }
    }

    #[test]
    fn the_spec_counts_and_every_parameter_named_but_the_one_left_as_is() {
        let counts = [4, 4, 3, 3, 2, 2, 2, 2, 2];
        for (kind, count) in EffectKind::ALL.into_iter().zip(counts) {
            assert_eq!(factory(kind).len(), count, "{}", kind.name());
            for preset in factory(kind) {
                for (at, spec) in kind.params().iter().enumerate() {
                    let named = preset.values.iter().any(|(name, _)| *name == spec.name);
                    assert_eq!(
                        named,
                        left_as_is(kind) != Some(at as u32),
                        "{} {} {}",
                        kind.name(),
                        preset.id,
                        spec.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_parameter_not_named_takes_its_default() {
        let preset = FactoryPreset {
            id: "test",
            values: &[("ratio", 3.0), ("makeup", 99.0)],
        };
        let defaults: Vec<f32> = (EffectKind::Compressor.params().iter())
            .map(|p| p.default)
            .collect();
        assert_eq!(
            preset.values_of(EffectKind::Compressor),
            [defaults[0], 3.0, defaults[2], defaults[3], 24.0]
        );
    }
}
