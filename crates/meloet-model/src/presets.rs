//! The presets that come with meloet for the built-in effects (round 12, M1:
//! 「はじめから入っているもの」). A preset names the values it sets; a parameter it does not
//! name takes its default. The screen gets the table as `FACTORY_PRESETS`
//! (`ui/src/bridge/generated/presets.ts`) and names a preset `preset.<effect>.<id>`.

use crate::effects::EffectKind;

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
    // round12 stub: M1 (the table of the round 12 spec M1, two for each M6 effect too)
    match kind {
        EffectKind::Eq4
        | EffectKind::Compressor
        | EffectKind::Reverb
        | EffectKind::Delay
        | EffectKind::Chorus
        | EffectKind::Limiter
        | EffectKind::Gate
        | EffectKind::DeEsser
        | EffectKind::Saturation => &[],
    }
}

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
