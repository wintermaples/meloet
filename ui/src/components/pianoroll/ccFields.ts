// ひらいた「CC の一覧」（仕様書 3 章「第 5 版で足した決まり」、モック R5-Lists）の行と欄の打ちこみ。DOM を知らない。
// 行はテイクのすべての点（テイクの中の番号 = tick の順）。打ちこめるのは位置と値だけ。決めると MoveControls を 1 つ。

import { CONTROL_PITCH_BEND, type ClipView, type Command, type ControlKind, type TakeNotes } from "../../bridge";
import { t } from "../../text/ja";
import { notePositionText, rangeText } from "../format";
import { signed4 } from "../meterFormat";
import type { Meter } from "../time";
import { isOnOff, MODULATION_CC, settle, SUSTAIN_CC, switchText, valueRange, valueText } from "./cc";
import { displayTickStep, readNotePositionLoose, type FieldSpec, type LooseValue } from "./noteFields";

export type CcFieldName = "position" | "value";
/** 左から右への順（Tab の順）。種類の列は打ちこまない。 */
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
  /** テイクの中の番号（一覧の何行目かと同じ）。 */
  index: number;
  position: string;
  kind: string;
  value: string;
}

export function ccRow(take: Controls, clip: Placed, i: number, meter: Meter): CcRow {
  const c = controlOf(take, i);
  return {
    index: i,
    position: notePositionText(clip.start + clip.takeOffset + take.controlTicks[i]!, meter),
    kind: controlName(c),
    value: controlValueText(c, take.controlValues[i]!),
  };
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
export function controlFieldSpec(name: CcFieldName, take: Controls, clip: Placed, i: number, meter: Meter): FieldSpec | null {
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
      unit: displayTickStep(meter),
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
