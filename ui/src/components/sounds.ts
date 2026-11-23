// Names of instruments and effects and how to tell them apart (built-in, SF2 preset, plugin). Pure functions that do not know the DOM.

import type {
  EffectRef,
  InsertView,
  LibraryView,
  PluginFormat,
  PluginRowView,
  SoundName,
  SoundRef,
  SoundView,
  StandIn,
} from "../bridge";
import { t, type TextKey } from "../text/ja";

/** Whether two instruments are the same. Plugins compare by key (one key per format). */
export function sameSound(a: SoundRef | null | undefined, b: SoundRef | null | undefined): boolean {
  if (!a || !b || a.kind !== b.kind) return false;
  if (a.kind === "builtin") return a.instrument === (b as typeof a).instrument;
  if (a.kind === "plugin") return a.key === (b as typeof a).key;
  const c = b as typeof a;
  return a.file === c.file && a.bank === c.bank && a.program === c.program;
}

export function sameEffect(a: EffectRef | null | undefined, b: EffectRef | null | undefined): boolean {
  if (!a || !b || a.kind !== b.kind) return false;
  return a.kind === "builtin" ? a.effect === (b as typeof a).effect : a.key === (b as typeof a).key;
}

/** Whether a plugin row has the format of the key (even if it is not the format in use). */
export function rowHasKey(row: PluginRowView, key: string): boolean {
  return row.key === key || row.formats.some((f) => f.key === key);
}

export function pluginRowOf(library: LibraryView, key: string): PluginRowView | null {
  return library.plugins.find((r) => rowHasKey(r, key)) ?? null;
}

/** The instrument name of a track (built-in ones in Japanese; SF2 and plugins keep their own names). */
export function soundViewName(sound: SoundView): string {
  if (sound.kind === "builtin") return t(`instrument.${sound.instrument}`);
  return sound.name;
}

/** The instrument name of a shelf row. Things not in the shelf show the key. */
export function soundRefName(sound: SoundRef, library: LibraryView): string {
  if (sound.kind === "builtin") return t(`instrument.${sound.instrument}`);
  if (sound.kind === "plugin") return pluginRowOf(library, sound.key)?.name ?? sound.key;
  const preset = library.sf2.find((f) => f.file === sound.file)?.presets.find((p) => p.bank === sound.bank && p.program === sound.program);
  return preset?.name ?? sound.file;
}

export function effectRefName(effect: EffectRef, library: LibraryView): string {
  if (effect.kind === "builtin") return t(`effect.${effect.effect}`);
  return pluginRowOf(library, effect.key)?.name ?? effect.key;
}

/** The name in an insert slot. */
export function insertName(insert: InsertView): string {
  return insert.effect.kind === "builtin" ? t(`effect.${insert.effect.effect}`) : insert.effect.name;
}

/** The instrument name in the status line (SF2: 「Clean Guitar ・ FluidR3_GM.sf2」). */
export function soundNameText(sound: SoundName): string {
  if (sound.kind === "builtin") return t(`instrument.${sound.instrument}`);
  if (sound.kind === "sf2") return t("sound.sf2Name", { name: sound.name, file: sound.file });
  return sound.name;
}

/** The text on hover over 「サイン波で代わり」 (spec 3 (85, 89-9)). sound is the track's instrument (as the song keeps it). */
export function standInTip(standIn: StandIn, sound: SoundView): string {
  switch (standIn) {
    case "packMissing":
      return t("track.standInPackTip");
    case "sf2Missing":
      return sound.kind === "sf2" ? t("track.standInSf2Tip", { file: sound.file }) : t("track.standInPackTip");
    case "pluginMissing":
      return t("track.standInPluginTip", { name: soundViewName(sound) });
    case "pluginUnreadable":
      return t("track.standInUnreadableTip", { name: soundViewName(sound) });
  }
}

export const formatName = (f: PluginFormat) => t(`plugin.format.${f}`);

/** The order of formats in the text on hover over 「＋1」 (mock F5-PluginAudition: LV2, then VST3). */
export function formatsTip(row: PluginRowView): string {
  const order: PluginFormat[] = ["lv2", "vst3"];
  const names = order.filter((f) => row.formats.some((x) => x.format === f)).map(formatName);
  return t("shelf.formatsTip", { formats: names.join(" ・ ") });
}

/** The key names of an SF2 drum kit (the table in spec 3). null for keys not in the table. */
export function gmDrumName(key: number): string | null {
  return key >= 35 && key <= 81 ? t(`gmDrum.${key}` as TextKey) : null;
}
