// 音源とエフェクトの名前と見分けかた（最初から入っているもの ・ SF2 の音色 ・ プラグイン）。DOM を知らない純粋な関数。

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

/** 同じ音源か。プラグインは鍵（形式ごとの鍵）で比べる。 */
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

/** プラグインの行が、その鍵の形式を持っているか（使う形式でなくても）。 */
export function rowHasKey(row: PluginRowView, key: string): boolean {
  return row.key === key || row.formats.some((f) => f.key === key);
}

export function pluginRowOf(library: LibraryView, key: string): PluginRowView | null {
  return library.plugins.find((r) => rowHasKey(r, key)) ?? null;
}

/** トラックの音源の名前（最初から入っている物は日本語、SF2 とプラグインは持っている名前のまま）。 */
export function soundViewName(sound: SoundView): string {
  if (sound.kind === "builtin") return t(`instrument.${sound.instrument}`);
  return sound.name;
}

/** 棚の行の音源の名前。棚にない物は鍵のまま。 */
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

/** インサートの枠の名前。 */
export function insertName(insert: InsertView): string {
  return insert.effect.kind === "builtin" ? t(`effect.${insert.effect.effect}`) : insert.effect.name;
}

/** 状態行の音源の名前（SF2 は「Clean Guitar ・ FluidR3_GM.sf2」）。 */
export function soundNameText(sound: SoundName): string {
  if (sound.kind === "builtin") return t(`instrument.${sound.instrument}`);
  if (sound.kind === "sf2") return t("sound.sf2Name", { name: sound.name, file: sound.file });
  return sound.name;
}

/** 「サイン波で代わり」に乗ったときの文（仕様書 3 章 85 ・ 89-9）。sound はトラックの音源（曲が持っている形のまま）。 */
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

/** 「＋1」に乗ったときの文の形式の並び（モック F5-PluginAudition: LV2、VST3 の順）。 */
export function formatsTip(row: PluginRowView): string {
  const order: PluginFormat[] = ["lv2", "vst3"];
  const names = order.filter((f) => row.formats.some((x) => x.format === f)).map(formatName);
  return t("shelf.formatsTip", { formats: names.join(" ・ ") });
}

/** SF2 のドラムのキットの鍵の名前（仕様書 3 章の表）。表にない鍵は null。 */
export function gmDrumName(key: number): string | null {
  return key >= 35 && key <= 81 ? t(`gmDrum.${key}` as TextKey) : null;
}
