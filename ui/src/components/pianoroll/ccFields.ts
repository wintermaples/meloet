// ひらいた「CC の一覧」（仕様書 3 章「第 5 版で足した決まり」、モック R5-Lists）の行と欄の打ちこみ。DOM を知らない。
// 行は種類ごとの見出しと、その種類のすべての点（tick の順）。打ちこめるのは位置と値。決めると MoveControls を 1 つ。

import { CONTROL_PITCH_BEND, type ClipView, type Command, type ControlKind, type TakeNotes } from "../../bridge";
import { t } from "../../text/ja";
import { notePositionText, rangeText } from "../format";
import { signed4 } from "../meterFormat";
import type { Bars } from "../time";
import { isOnOff, MODULATION_CC, settle, SUSTAIN_CC, switchText, valueRange, valueText } from "./cc";
import { readNotePositionLoose, type FieldSpec, type LooseValue } from "./noteFields";

export type CcFieldName = "position" | "value";
/** 左から右への順（Tab の順）。 */
export const CC_FIELD_ORDER: readonly CcFieldName[] = ["position", "value"];

type Controls = Pick<TakeNotes, "controlTicks" | "controlKinds" | "controlNumbers" | "controlValues">;
type Placed = Pick<ClipView, "start" | "end" | "takeOffset">;

export function controlOf(take: Controls, i: number): ControlKind {
  return take.controlKinds[i] === CONTROL_PITCH_BEND ? { kind: "pitchBend" } : { kind: "cc", number: take.controlNumbers[i]! };
}

/** 種類の名前（CC の帯と同じ: サステイン、モジュレーション、ピッチベンド、「CC 7」）。 */
export function controlName(c: ControlKind): string {
  if (c.kind === "pitchBend") return t("pr.pitchBend");
  if (c.number === SUSTAIN_CC) return t("pr.sustain");
  if (c.number === MODULATION_CC) return t("pr.modulation");
  return t("pr.ccNumber", { number: String(c.number) });
}

/** 値（帯の読み値と同じ: 「踏む（127）」「OFF（0）」「+2048」「064」）。 */
export function controlValueText(c: ControlKind, v: number): string {
  return isOnOff(c) ? switchText(v) : valueText(c, v);
}

export interface CcRow {
  /** テイクの中の番号。 */
  index: number;
  position: string;
  value: string;
}

export function ccRow(take: Controls, clip: Placed, i: number, meter: Bars): CcRow {
  const c = controlOf(take, i);
  return {
    index: i,
    position: notePositionText(clip.start + clip.takeOffset + take.controlTicks[i]!, meter),
    value: controlValueText(c, take.controlValues[i]!),
  };
}

/** 種類ごとのまとまり。points はテイクの中の番号（tick の順）。 */
export interface CcGroup {
  key: string;
  name: string;
  points: number[];
}

/** 種類を見分けるキー（たたんだ状態を覚えるのに使う）。 */
export function ccGroupKey(c: ControlKind): string {
  return c.kind === "pitchBend" ? "pitchBend" : `cc${c.number}`;
}

/** CC の札と同じ順: サステイン、ピッチベンド、モジュレーション、そのほかの CC は番号の順。 */
function groupRank(c: ControlKind): number {
  if (c.kind === "pitchBend") return 1;
  if (c.number === SUSTAIN_CC) return 0;
  if (c.number === MODULATION_CC) return 2;
  return 3 + c.number;
}

/** テイクにある種類だけのまとまり。 */
export function ccGroups(take: Controls): CcGroup[] {
  const byKey = new Map<string, CcGroup & { rank: number }>();
  for (let i = 0; i < take.controlTicks.length; i++) {
    const c = controlOf(take, i);
    const key = ccGroupKey(c);
    let g = byKey.get(key);
    if (!g) byKey.set(key, (g = { key, name: controlName(c), points: [], rank: groupRank(c) }));
    g.points.push(i);
  }
  return [...byKey.values()].sort((a, b) => a.rank - b.rank).map(({ key, name, points }) => ({ key, name, points }));
}

/**
 * 一覧の行のならび。0 以上はテイクの中の番号（点の行）、負は見出し（-1 - まとまりの番号）。たたんだ種類の点は入れない。
 * lineOf は、点の番号 → 何行目か（たたんでいれば -1）。
 */
export function ccLines(groups: readonly CcGroup[], folded: readonly string[], count: number): { lines: Int32Array; lineOf: Int32Array } {
  let n = groups.length;
  for (const g of groups) if (!folded.includes(g.key)) n += g.points.length;
  const lines = new Int32Array(n);
  const lineOf = new Int32Array(count).fill(-1);
  let at = 0;
  groups.forEach((g, gi) => {
    lines[at++] = -1 - gi;
    if (folded.includes(g.key)) return;
    for (const i of g.points) {
      lineOf[i] = at;
      lines[at++] = i;
    }
  });
  return { lines, lineOf };
}

/** line の行から dir の向きに、次の点の行のテイクの中の番号（見出しはとばす）。なければ null。 */
export function neighborPoint(lines: Int32Array, line: number, dir: -1 | 1): number | null {
  for (let l = line + dir; l >= 0 && l < lines.length; l += dir) if (lines[l]! >= 0) return lines[l]!;
  return null;
}

/** 値。符号（+ − -）と小数も読み、小数は四捨五入。 */
export function readControlValueLoose(s: string): LooseValue | null {
  const m = s
    .normalize("NFKC")
    .trim()
    .replaceAll("−", "-")
    .match(/^([+-]?)(\d{1,6}(?:\.\d+)?|\.\d+)$/);
  if (!m) return null;
  const v = Math.round(Number(m[2]));
  return { value: m[1] === "-" ? -v : v, unit: null };
}

/**
 * 点 i の欄 name の打ちこみの決まり。位置はクリップの中（もう外にある点は、その位置より外へは出さない。Rust の MoveControls と同じ）。
 * 値は CC 0〜127、ピッチベンド −8192〜+8191。オン ・ オフの CC は、決めるときに 0 か 127 に吸いつける（`commitControl`）。
 */
export function controlFieldSpec(name: CcFieldName, take: Controls, clip: Placed, i: number, meter: Bars): FieldSpec | null {
  if (i < 0 || i >= take.controlTicks.length) return null;
  if (name === "position") {
    const tick = clip.start + clip.takeOffset + take.controlTicks[i]!;
    const low = Math.min(clip.start, tick);
    const high = Math.max(clip.end - 1, tick);
    const format = (v: number) => notePositionText(v, meter);
    return {
      field: "position",
      initial: format(tick),
      kept: format(tick),
      current: tick,
      low,
      high,
      unit: 1,
      shift: 10,
      range: rangeText(format(low), format(high)),
      format,
      read: (s) => {
        const r = readNotePositionLoose(s, meter);
        return r && r.unit === null ? r.value : null;
      },
      loose: (s) => readNotePositionLoose(s, meter),
    };
  }
  const c = controlOf(take, i);
  const v = take.controlValues[i]!;
  const r = valueRange(c);
  const format = c.kind === "pitchBend" ? signed4 : String;
  const onOff = isOnOff(c);
  return {
    field: "controlValue",
    initial: format(v),
    kept: format(v),
    current: v,
    low: r.min,
    high: r.max,
    // オン ・ オフの CC は ↑ で 127、↓ で 0。
    unit: onOff ? r.max : 1,
    shift: 10,
    range: rangeText(format(r.min), format(r.max)),
    format,
    read: (s) => readControlValueLoose(s)?.value ?? null,
    loose: readControlValueLoose,
  };
}

/** 決めた値の意図（MoveControls 1 つ）。変わらなければ null。 */
export function commitControl(name: CcFieldName, spec: FieldSpec, value: number, take: Controls & { id: number }, i: number): Command | null {
  const v = name === "value" ? settle(controlOf(take, i), value) : value;
  if (v === spec.current) return null;
  const delta = v - spec.current!;
  return name === "position"
    ? { type: "moveControls", take: take.id, points: [i], ticks: delta, value: 0 }
    : { type: "moveControls", take: take.id, points: [i], ticks: 0, value: delta };
}

/** Tab（back なら Shift Tab）で次にひらく欄。いまの行の外へ出るなら row が ±1。 */
export function nextCcField(name: CcFieldName, back: boolean): { name: CcFieldName; row: -1 | 0 | 1 } {
  const i = CC_FIELD_ORDER.indexOf(name) + (back ? -1 : 1);
  if (i < 0) return { name: CC_FIELD_ORDER[CC_FIELD_ORDER.length - 1]!, row: -1 };
  if (i >= CC_FIELD_ORDER.length) return { name: CC_FIELD_ORDER[0]!, row: 1 };
  return { name: CC_FIELD_ORDER[i]!, row: 0 };
}
