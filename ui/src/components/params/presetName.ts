// The name of a preset of a built-in effect (round 12 M1): one that comes with meloet by `preset.<effect>.<id>`, one of
// the PC by the name it was kept under.
import type { EffectKind, PresetRef } from "../../bridge";
import { hasText, t } from "../../text";

export function presetName(effect: EffectKind, preset: PresetRef): string {
  if (preset.kind === "user") return preset.name;
  const key = `preset.${effect}.${preset.id}`;
  return hasText(key) ? t(key) : preset.id;
}
