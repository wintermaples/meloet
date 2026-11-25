// Rows of the params panel (spec 2 "Parameter row", the reverb and delay of spec 3, architecture 19-3). Pure functions that do not know the DOM.

import {
  EFFECT_PARAMS,
  type EffectKind,
  type EffectParam,
} from "../../bridge/types";
import { t, type TextKey } from "../../text";

const MINUS = "−";

/** Decimal places (as spec 3 writes them, 「2.40 s」「20 ms」「−6.0 dB」「35 %」「6000 Hz」, and as the ranges in the table of 19-3). */
export function decimals(p: EffectParam): number {
  switch (p.unit) {
    case "s":
      return 2;
    case "ms":
      return p.min > 0 && p.min < 1 ? 1 : 0; // attack 0.1 to 100.0 ms
    case "hz":
      return p.max <= 10 ? 2 : 0; // modulation rate 0.05 to 5.00 Hz
    case "db":
    case "oct":
    case "ratio":
      return 1;
    default:
      return 0;
  }
}

/** The value rounded to the shown places (values sent while dragging are this fine too). */
export function roundTo(p: EffectParam, value: number): number {
  if (p.shape !== "continuous") return Math.round(value);
  const f = 10 ** decimals(p);
  return Math.round(Math.min(p.max, Math.max(p.min, value)) * f) / f;
}

/** The value text (「2.40 s」「−6.0 dB」「+3.0 dB」「1/8」「オン」). */
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

/** Only the number of the value (no unit). On/off and steps give the same text as valueText. */
export function valueNumberText(effect: EffectKind, p: EffectParam, value: number): string {
  if (p.shape !== "continuous") return valueText(effect, p, value);
  let s = roundTo(p, value).toFixed(decimals(p));
  if (/^-0(\.0*)?$/.test(s)) s = s.slice(1);
  // Only for ranges that also go below zero, like 「−18.0〜+18.0 dB」, plus values get a 「+」.
  if (p.min < 0 && Number(s) > 0) s = `+${s}`;
  return s.replace("-", MINUS);
}

/** The number first put in the typing box (no unit; minus is a typeable 「-」). */
export function typedText(p: EffectParam, value: number): string {
  let s = roundTo(p, value).toFixed(p.shape === "continuous" ? decimals(p) : 0);
  if (/^-0(\.0*)?$/.test(s)) s = s.slice(1);
  return s;
}

/** The unit put after the typing box (「dB」「: 1」). */
export function unitText(p: EffectParam): string {
  if (p.shape !== "continuous" || p.unit === "none") return "";
  return t(`param.value.${p.unit}` as TextKey, { value: "" }).trim();
}

/** Whether the bar moves on a log scale (always for frequency; for times whose range spans many tens of times). */
function logScale(p: EffectParam): boolean {
  if (p.min <= 0) return false;
  return p.unit === "hz" || ((p.unit === "ms" || p.unit === "s") && p.max / p.min >= 50);
}

/** Value → share of the bar from the left (0 to 1). */
export function fractionOf(p: EffectParam, value: number): number {
  const v = Math.min(p.max, Math.max(p.min, value));
  const f = logScale(p)
    ? Math.log(v / p.min) / Math.log(p.max / p.min)
    : (v - p.min) / (p.max - p.min);
  return Number.isFinite(f) ? Math.min(1, Math.max(0, f)) : 0;
}

/** Share of the bar → value (rounded to the shown places). */
export function valueAt(p: EffectParam, fraction: number): number {
  const f = Math.min(1, Math.max(0, fraction));
  const v = logScale(p)
    ? p.min * (p.max / p.min) ** f
    : p.min + f * (p.max - p.min);
  return roundTo(p, v);
}

/** One press of ← →. With Shift, one unit of the last shown place. */
export function keyStep(
  p: EffectParam,
  value: number,
  dir: 1 | -1,
  fine: boolean,
): number {
  if (p.shape !== "continuous")
    return Math.min(p.max, Math.max(p.min, Math.round(value) + dir));
  const unit = roundTo(p, value + dir / 10 ** decimals(p));
  if (fine) return unit;
  const next = valueAt(p, fractionOf(p, value) + dir / 20);
  // At the bottom of a log scale one twentieth of the bar can be less than one shown place.
  return next !== roundTo(p, value) ? next : unit;
}

export interface ParamRow {
  /** The parameter number of the shown row (its place in EFFECT_PARAMS). */
  param: number;
  /** The numbers this row covers (two for the delay 「間隔」). Used to mark the chosen row. */
  covers: number[];
  name: string;
}

export interface ParamGroup {
  /** The heading (such as 「低い音」 of the EQ). null for no heading. */
  group: string | null;
  name: string | null;
  rows: ParamRow[];
}

const DELAY_STEP = "intervalStep";
const DELAY_MS = "intervalMs";
const DELAY_SYNC = "sync";

/** The parameter name. */
export function paramName(effect: EffectKind, param: number): string {
  const p = EFFECT_PARAMS[effect][param];
  return p ? t(`param.${effect}.${p.id}` as TextKey) : "";
}

/**
 * The rows shown in the panel. The delay 「間隔」 is one row: steps when 「テンポに合わせる」 is on, ms when off
 * (docs/history/design-notes.ja.md A-14). Those with a heading are put in groups (in the order of EFFECT_PARAMS).
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

/** The rows left by the search box (names that contain it; case does not matter). Headings with no rows left are not shown. */
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

/** The parameter of the next row chosen with ↑ ↓ (among the rows in view; stops at the ends). */
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
