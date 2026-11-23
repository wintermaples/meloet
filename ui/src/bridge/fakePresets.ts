// Round 12 M1 for the fake: 「プリセット ▾」 of an effect that comes with meloet, as Rust's `play/presets.rs` and
// `crates/meloet-core/src/presets.rs` (the fake keeps 「自分で残したもの」 for as long as the page lives, not in a file).
import { EFFECT_KINDS, EFFECT_PARAMS, FACTORY_PRESETS, PRESET_NAME_MAX, PRESETS_PER_EFFECT } from "./types";
import type { AppState, Command, EffectKind, PresetRef, PresetsView, SelectedInsert, StatusMessage } from "./types";

/** The values of each 「自分で残したもの」, by `${effect}:${name}`. */
const userValues = new Map<string, number[]>();

/** For scenes: a preset of the PC with these values. */
export function fakeUserPreset(state: AppState, effect: EffectKind, name: string, values: number[]): void {
  userValues.set(`${effect}:${name}`, [...values]);
  const list = state.presets.user.find((u) => u.effect === effect);
  if (list) list.names.push(name);
  else state.presets.user.push({ effect, names: [name] });
}

/** The parameter a factory preset leaves as the insert has it (`meloet_model::presets::left_as_is`). */
function leftAsIs(effect: EffectKind): number {
  return effect === "reverb" || effect === "delay" ? EFFECT_PARAMS[effect].findIndex((p) => p.id === "mix") : -1;
}

/** As Rust's `crate::presets::name_of`: trimmed, at most PRESET_NAME_MAX characters, null when empty. */
export function presetName(text: string): string | null {
  const name = [...text.trim()].slice(0, PRESET_NAME_MAX).join("").trimEnd();
  return name === "" ? null : name;
}

/** What the preset sets: [parameter, value]. */
function valuesOf(effect: EffectKind, preset: PresetRef): [number, number][] | null {
  if (preset.kind === "factory") {
    const found = FACTORY_PRESETS[effect].find((p) => p.id === preset.id);
    const left = leftAsIs(effect);
    return found ? found.values.flatMap((v, i) => (i === left ? [] : [[i, v] as [number, number]])) : null;
  }
  const values = userValues.get(`${effect}:${preset.name}`);
  return values ? values.map((v, i) => [i, v] as [number, number]) : null;
}

const same = (a: number, b: number) => Math.abs(a - b) <= 1e-6 * Math.max(1, Math.abs(a), Math.abs(b));

/** The preset whose values the insert has: the factory ones first. */
export function presetOf(effect: EffectKind, values: readonly number[], presets: PresetsView): PresetRef | null {
  const has = (set: [number, number][] | null) => set !== null && set.length > 0 && set.every(([i, v]) => values[i] !== undefined && same(values[i]!, v));
  for (const p of FACTORY_PRESETS[effect]) if (has(valuesOf(effect, { kind: "factory", id: p.id }))) return { kind: "factory", id: p.id };
  const names = presets.user.find((u) => u.effect === effect)?.names ?? [];
  for (const name of names) if (has(valuesOf(effect, { kind: "user", name }))) return { kind: "user", name };
  return null;
}

/**
 * The three intents of M1 on the selected insert (`sel`, changed in place). The fake has no lanes on effect parameters
 * that move, so nothing is kept back. null: not one of them.
 */
export function reducePresets(
  presetsNow: PresetsView,
  sel: SelectedInsert | null,
  busy: boolean,
  command: Command,
): { presets: PresetsView; notice: StatusMessage | null } | null {
  const presets = structuredClone(presetsNow);
  const sameInsert = (insert: SelectedInsert["insert"]) => sel !== null && JSON.stringify(sel.insert) === JSON.stringify(insert);
  let notice: StatusMessage | null = null;
  switch (command.type) {
    case "applyPreset": {
      if (busy || !sel || !sameInsert(command.insert)) break;
      for (const [i, v] of valuesOf(sel.effect, command.preset) ?? []) sel.values[i] = v;
      break;
    }
    case "savePreset": {
      const name = presetName(command.name);
      if (!name || !sel || !sameInsert(command.insert)) break;
      let list = presets.user.find((u) => u.effect === sel.effect);
      if (list?.names.includes(name)) notice = { id: "presetOverwritten", name };
      else if ((list?.names.length ?? 0) >= PRESETS_PER_EFFECT) {
        notice = { id: "tooManyPresets", effect: sel.effect };
        break;
      } else {
        if (!list) {
          list = { effect: sel.effect, names: [] };
          presets.user.push(list);
          presets.user.sort((a, b) => EFFECT_KINDS.indexOf(a.effect) - EFFECT_KINDS.indexOf(b.effect));
        }
        list.names.push(name);
      }
      userValues.set(`${sel.effect}:${name}`, [...sel.values]);
      break;
    }
    case "deletePreset": {
      const list = presets.user.find((u) => u.effect === command.effect);
      if (!list?.names.includes(command.name)) break;
      list.names = list.names.filter((n) => n !== command.name);
      userValues.delete(`${command.effect}:${command.name}`);
      presets.user = presets.user.filter((u) => u.names.length > 0);
      break;
    }
    default:
      return null;
  }
  return { presets, notice };
}
