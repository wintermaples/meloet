//! 「プリセット ▾」 of an effect that comes with meloet (round 12, M1): using a preset is one
//! step on the song; keeping and removing one changes only the PC's `presets.json`.

use meloet_model::effects::EffectKind as ModelEffect;
use meloet_model::{StepKind, StripId, presets};
use meloet_protocol::{EffectKind, InsertRef, PresetRef, PresetsView, StatusMessage};

use super::Play;
use super::mixer::{effect_kind, model_effect, strip_id};
use crate::presets::{Kept, UserPresets, name_of};
use crate::state::Effect;

/// Values that are the same once written to a file and read back.
fn same_value(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
}

impl Play {
    pub fn set_user_presets(&mut self, presets: UserPresets) {
        self.user_presets = presets;
    }

    pub fn presets_view(&self) -> PresetsView {
        self.user_presets.view()
    }

    /// The built-in effect in the slot, with its strip and slot.
    fn builtin_at(&self, insert: InsertRef) -> Option<(StripId, usize, ModelEffect)> {
        let (strip, slot) = (strip_id(insert.strip), usize::from(insert.slot));
        let kind = self.doc().insert(strip, slot)?.kind()?;
        Some((strip, slot, kind))
    }

    /// What `preset` sets on an insert of `kind`: a position in the table and its value.
    fn preset_values(&self, kind: ModelEffect, preset: &PresetRef) -> Option<Vec<(u32, f32)>> {
        match preset {
            PresetRef::Factory { id } => {
                let left = presets::left_as_is(kind);
                let values = presets::find(kind, id)?.values_of(kind);
                Some(
                    (0u32..)
                        .zip(values)
                        .filter(|&(at, _)| Some(at) != left)
                        .collect(),
                )
            }
            PresetRef::User { name } => Some(self.user_presets.find(kind, name)?.values.clone()),
        }
    }

    /// The preset whose values the insert has: the factory ones first.
    pub(super) fn preset_of(&self, kind: ModelEffect, values: &[f32]) -> Option<PresetRef> {
        let has = |set: &[(u32, f32)]| {
            !set.is_empty()
                && (set.iter()).all(|&(at, v)| {
                    (values.get(at as usize)).is_some_and(|&now| same_value(now, v))
                })
        };
        let factory = (presets::factory(kind).iter()).find(|p| {
            has(&self
                .preset_values(kind, &factory_ref(p.id))
                .unwrap_or_default())
        });
        if let Some(p) = factory {
            return Some(factory_ref(p.id));
        }
        (self.user_presets.of(kind).iter())
            .find(|p| has(&p.values))
            .map(|p| PresetRef::User {
                name: p.name.clone(),
            })
    }

    /// `ApplyPreset`: one step 「プリセットを使う」. A parameter a lane moves stays; then what
    /// the status line says of them.
    pub fn apply_preset(
        &mut self,
        insert: InsertRef,
        preset: &PresetRef,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let (strip, slot, kind) = self.builtin_at(insert)?;
        let mut values = self.preset_values(kind, preset)?;
        let before = values.len();
        values.retain(|&(param, value)| {
            let lane = self.param_lane(strip, slot, kind, param, value);
            !lane.is_some_and(|(key, _)| self.lane_at(key).is_some_and(|l| l.moves()))
        });
        let kept = (before - values.len()) as u32;
        if let Ok(Some(mut edit)) = self.effect_params_edit(strip, slot, kind, &values) {
            edit.step = StepKind::ApplyPreset { strip, slot, kind };
            self.apply(edit, effects);
        }
        (kept > 0).then_some(StatusMessage::PresetLanesKept { count: kept })
    }

    /// `SavePreset`: the insert's values are kept under `name` with the PC.
    pub fn save_preset(
        &mut self,
        insert: InsertRef,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let name = name_of(name)?;
        let (strip, slot, kind) = self.builtin_at(insert)?;
        let values = (0u32..)
            .zip(self.doc().insert(strip, slot)?.values().iter().copied())
            .collect();
        let kept = self.user_presets.keep(kind, name.clone(), values);
        if kept == Kept::TooMany {
            return Some(StatusMessage::TooManyPresets {
                effect: effect_kind(kind),
            });
        }
        effects.push(Effect::SavePresets(Box::new(self.user_presets.clone())));
        (kept == Kept::Overwritten).then_some(StatusMessage::PresetOverwritten { name })
    }

    /// `DeletePreset`.
    pub fn delete_preset(&mut self, effect: EffectKind, name: &str, effects: &mut Vec<Effect>) {
        if self.user_presets.remove(model_effect(effect), name) {
            effects.push(Effect::SavePresets(Box::new(self.user_presets.clone())));
        }
    }
}

fn factory_ref(id: &str) -> PresetRef {
    PresetRef::Factory { id: id.to_owned() }
}

#[cfg(test)]
mod tests;
