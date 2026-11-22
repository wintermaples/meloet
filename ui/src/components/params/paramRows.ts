// パラメータの欄の行（仕様書 2 章「パラメータの行」、3 章のリバーブ ・ ディレイ、architecture.md 19-3）。DOM を知らない純粋な関数。

import {
  EFFECT_PARAMS,
  type EffectKind,
  type EffectParam,
} from "../../bridge/types";
import { t, type TextKey } from "../../text/ja";

const MINUS = "−";

/** 小数のけた（仕様書 3 章の書きかた「2.40 s」「20 ms」「−6.0 dB」「35 %」「6000 Hz」と、19-3 の表の範囲の書きかた）。 */
export function decimals(p: EffectParam): number {
  switch (p.unit) {
    case "s":
      return 2;
    case "ms":
      return p.min > 0 && p.min < 1 ? 1 : 0; // おさえるまで 0.1〜100.0 ms
    case "hz":
      return p.max <= 10 ? 2 : 0; // ゆれの速さ 0.05〜5.00 Hz
    case "db":
    case "oct":
    case "ratio":
      return 1;
    default:
      return 0;
  }
}

/** 見せるけたに丸めた値（ドラッグで送る値もこの細かさ）。 */
export function roundTo(p: EffectParam, value: number): number {
  if (p.shape !== "continuous") return Math.round(value);
  const f = 10 ** decimals(p);
  return Math.round(Math.min(p.max, Math.max(p.min, value)) * f) / f;
}

/** 値の文（「2.40 s」「−6.0 dB」「+3.0 dB」「1/8」「オン」）。 */
export function valueText(
  effect: EffectKind,
  p: EffectParam,
  value: number,
): string {
  if (p.shape === "switch") return t(value >= 0.5 ? "param.on" : "param.off");
  if (p.shape === "steps") {
    const id =
      p.steps[Math.max(0, Math.min(p.steps.length - 1, Math.round(value)))];
    return id === undefined ? "" : t(`param.${effect}.step.${id}` as TextKey);
  }
  const s = valueNumberText(effect, p, value);
  return p.unit === "none" || p.shape !== "continuous"
    ? s
    : t(`param.value.${p.unit}` as TextKey, { value: s });
}

/** 値の数の所だけ（単位なし）。オン ・ オフと段階は、valueText と同じ文。 */
export function valueNumberText(effect: EffectKind, p: EffectParam, value: number): string {
  if (p.shape !== "continuous") return valueText(effect, p, value);
  let s = roundTo(p, value).toFixed(decimals(p));
  if (/^-0(\.0*)?$/.test(s)) s = s.slice(1);
  // 「−18.0〜+18.0 dB」のように、マイナスもある範囲だけ、プラスに「+」をつける。
  if (p.min < 0 && Number(s) > 0) s = `+${s}`;
  return s.replace("-", MINUS);
}

/** 打ちこむ枠にはじめに入れる数（単位なし、マイナスは打てる「-」）。 */
export function typedText(p: EffectParam, value: number): string {
  let s = roundTo(p, value).toFixed(p.shape === "continuous" ? decimals(p) : 0);
  if (/^-0(\.0*)?$/.test(s)) s = s.slice(1);
  return s;
}

/** 打ちこむ枠のうしろにそえる単位（「dB」「: 1」）。 */
export function unitText(p: EffectParam): string {
  if (p.shape !== "continuous" || p.unit === "none") return "";
  return t(`param.value.${p.unit}` as TextKey, { value: "" }).trim();
}

/** 棒を対数で動かすか（音の高さはいつも。時間は何十倍にも広がる物）。 */
function logScale(p: EffectParam): boolean {
  if (p.min <= 0) return false;
  return p.unit === "hz" || ((p.unit === "ms" || p.unit === "s") && p.max / p.min >= 50);
}

/** 値 → 棒の左からの割合（0〜1）。 */
export function fractionOf(p: EffectParam, value: number): number {
  const v = Math.min(p.max, Math.max(p.min, value));
  const f = logScale(p)
    ? Math.log(v / p.min) / Math.log(p.max / p.min)
    : (v - p.min) / (p.max - p.min);
  return Number.isFinite(f) ? Math.min(1, Math.max(0, f)) : 0;
}

/** 棒の割合 → 値（見せるけたに丸める）。 */
export function valueAt(p: EffectParam, fraction: number): number {
  const f = Math.min(1, Math.max(0, fraction));
  const v = logScale(p)
    ? p.min * (p.max / p.min) ** f
    : p.min + f * (p.max - p.min);
  return roundTo(p, v);
}

/** ← → の 1 回ぶん。Shift で、見せるけたの 1 つぶん。 */
export function keyStep(
  p: EffectParam,
  value: number,
  dir: 1 | -1,
  fine: boolean,
): number {
  if (p.shape !== "continuous")
    return Math.min(p.max, Math.max(p.min, Math.round(value) + dir));
  if (fine) return roundTo(p, value + dir / 10 ** decimals(p));
  return valueAt(p, fractionOf(p, value) + dir / 20);
}

export interface ParamRow {
  /** 見せている行のパラメータの番号（EFFECT_PARAMS の中の場所）。 */
  param: number;
  /** この行が受けもつ番号（ディレイの「間隔」は 2 つ）。えらんだ行の印に使う。 */
  covers: number[];
  name: string;
}

export interface ParamGroup {
  /** 見出し（EQ の「低い音」など）。null は見出しなし。 */
  group: string | null;
  name: string | null;
  rows: ParamRow[];
}

const DELAY_STEP = "intervalStep";
const DELAY_MS = "intervalMs";
const DELAY_SYNC = "sync";

/** パラメータの名前。 */
export function paramName(effect: EffectKind, param: number): string {
  const p = EFFECT_PARAMS[effect][param];
  return p ? t(`param.${effect}.${p.id}` as TextKey) : "";
}

/**
 * 欄に出す行。ディレイの「間隔」は 1 行で、「テンポに合わせる」がオンなら段階、オフなら ms（docs/history/design-notes.ja.md A-14）。
 * 見出しのある物はグループにまとめる（EFFECT_PARAMS の順のまま）。
 */
export function allRows(
  effect: EffectKind,
  values: readonly number[],
): ParamGroup[] {
  const table = EFFECT_PARAMS[effect];
  const syncAt =
    effect === "delay" ? table.findIndex((p) => p.id === DELAY_SYNC) : -1;
  const synced =
    syncAt < 0 || (values[syncAt] ?? table[syncAt]!.default) >= 0.5;
  const stepAt = table.findIndex((p) => p.id === DELAY_STEP);
  const msAt = table.findIndex((p) => p.id === DELAY_MS);
  const groups: ParamGroup[] = [];
  table.forEach((p, i) => {
    let row: ParamRow;
    if (effect === "delay" && (i === stepAt || i === msAt)) {
      if (i === msAt) return;
      row = {
        param: synced ? stepAt : msAt,
        covers: [stepAt, msAt],
        name: paramName(effect, i),
      };
    } else row = { param: i, covers: [i], name: paramName(effect, i) };
    const last = groups.at(-1);
    if (last && last.group === p.group) last.rows.push(row);
    else
      groups.push({
        group: p.group,
        name:
          p.group === null
            ? null
            : t(`param.${effect}.group.${p.group}` as TextKey),
        rows: [row],
      });
  });
  return groups;
}

/** さがす枠で残る行（名前にふくまれる物。英字の大小は見ない）。行の残らない見出しは出さない。 */
export function searchRows(
  groups: readonly ParamGroup[],
  query: string,
): ParamGroup[] {
  const q = query.trim().toLowerCase();
  if (q === "") return groups.map((g) => ({ ...g, rows: [...g.rows] }));
  return groups.flatMap((g) => {
    const rows = g.rows.filter((r) => r.name.toLowerCase().includes(q));
    return rows.length === 0 ? [] : [{ ...g, rows }];
  });
}

export const rowCount = (groups: readonly ParamGroup[]) =>
  groups.reduce((n, g) => n + g.rows.length, 0);

/** ↑ ↓ でえらぶ次の行のパラメータ（見えている行の中。はしで止まる）。 */
export function stepRow(
  rows: readonly ParamRow[],
  selected: number | null,
  delta: 1 | -1,
): number | null {
  if (rows.length === 0) return null;
  const at =
    selected === null ? -1 : rows.findIndex((r) => r.covers.includes(selected));
  if (at < 0) return (delta > 0 ? rows[0]! : rows.at(-1)!).param;
  return rows[Math.max(0, Math.min(rows.length - 1, at + delta))]!.param;
}
