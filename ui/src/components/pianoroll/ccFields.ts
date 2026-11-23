// Rows of the open 「CC の一覧」 and typing in its fields (spec 3 "Rules added in version 5 (after round 3)", mock R5-Lists). Does not know the DOM.
// The rows are a heading for each kind and all points of that kind (in tick order). Position and value can be typed. Deciding sends one MoveControls.

import { CONTROL_PITCH_BEND, type ClipView, type Command, type ControlKind, type TakeNotes } from "../../bridge";
import { t } from "../../text/ja";
import { notePositionText, rangeText } from "../format";
import { signed4 } from "../meterFormat";
import type { Bars } from "../time";
import { isOnOff, MODULATION_CC, settle, SUSTAIN_CC, switchText, valueRange, valueText } from "./cc";
import { readNotePositionLoose, type FieldSpec, type LooseValue } from "./noteFields";

export type CcFieldName = "position" | "value";
/** Order from left to right (the Tab order). */
export const CC_FIELD_ORDER: readonly CcFieldName[] = ["position", "value"];

type Controls = Pick<TakeNotes, "controlTicks" | "controlKinds" | "controlNumbers" | "controlValues">;
type Placed = Pick<ClipView, "start" | "end" | "takeOffset">;

export function controlOf(take: Controls, i: number): ControlKind {
  return take.controlKinds[i] === CONTROL_PITCH_BEND ? { kind: "pitchBend" } : { kind: "cc", number: take.controlNumbers[i]! };
}

/** The kind name (the same as CC lanes: sustain, modulation, pitch bend, 「CC 7」). */
export function controlName(c: ControlKind): string {
  if (c.kind === "pitchBend") return t("pr.pitchBend");
  if (c.number === SUSTAIN_CC) return t("pr.sustain");
  if (c.number === MODULATION_CC) return t("pr.modulation");
  return t("pr.ccNumber", { number: String(c.number) });
}

/** The value (the same as the lane's reading: 「踏む（127）」「OFF（0）」「+2048」「064」). */
export function controlValueText(c: ControlKind, v: number): string {
  return isOnOff(c) ? switchText(v) : valueText(c, v);
}

export interface CcRow {
  /** The index in the take. */
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

/** A group per kind. points are indexes in the take (in tick order). */
export interface CcGroup {
  key: string;
  name: string;
  points: number[];
}

/** A key that tells kinds apart (used to remember the folded state). */
export function ccGroupKey(c: ControlKind): string {
  return c.kind === "pitchBend" ? "pitchBend" : `cc${c.number}`;
}

/** The same order as the CC tabs: sustain, pitch bend, modulation, then other CCs by number. */
function groupRank(c: ControlKind): number {
  if (c.kind === "pitchBend") return 1;
  if (c.number === SUSTAIN_CC) return 0;
  if (c.number === MODULATION_CC) return 2;
  return 3 + c.number;
}

/** Groups only for kinds the take has. */
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
 * The order of the list rows. 0 or more is an index in the take (a point row); negative is a heading (-1 - group index). Points of folded kinds are left out.
 * lineOf is point index → row number (-1 if folded).
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

/** From the row line in direction dir, the take index of the next point row (headings are skipped). Null if none. */
export function neighborPoint(lines: Int32Array, line: number, dir: -1 | 1): number | null {
  for (let l = line + dir; l >= 0 && l < lines.length; l += dir) if (lines[l]! >= 0) return lines[l]!;
  return null;
}

/** A value. Also reads signs (+ − -) and decimals; decimals are rounded. */
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
 * Typing rules for field name of point i. Position stays inside the clip (a point already outside does not go further out than its position; the same as MoveControls in Rust).
 * Value is 0 to 127 for CC, −8192 to +8191 for pitch bend. On/off CCs snap to 0 or 127 when decided (`commitControl`).
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
    // On/off CCs: ↑ gives 127, ↓ gives 0.
    unit: onOff ? r.max : 1,
    shift: 10,
    range: rangeText(format(r.min), format(r.max)),
    format,
    read: (s) => readControlValueLoose(s)?.value ?? null,
    loose: readControlValueLoose,
  };
}

/** The intent for the decided value (one MoveControls). Null if nothing changed. */
export function commitControl(name: CcFieldName, spec: FieldSpec, value: number, take: Controls & { id: number }, i: number): Command | null {
  const v = name === "value" ? settle(controlOf(take, i), value) : value;
  if (v === spec.current) return null;
  const delta = v - spec.current!;
  return name === "position"
    ? { type: "moveControls", take: take.id, points: [i], ticks: delta, value: 0 }
    : { type: "moveControls", take: take.id, points: [i], ticks: 0, value: delta };
}

/** The field to open next with Tab (Shift Tab if back). row is ±1 if it leaves the current row. */
export function nextCcField(name: CcFieldName, back: boolean): { name: CcFieldName; row: -1 | 0 | 1 } {
  const i = CC_FIELD_ORDER.indexOf(name) + (back ? -1 : 1);
  if (i < 0) return { name: CC_FIELD_ORDER[CC_FIELD_ORDER.length - 1]!, row: -1 };
  if (i >= CC_FIELD_ORDER.length) return { name: CC_FIELD_ORDER[0]!, row: 1 };
  return { name: CC_FIELD_ORDER[i]!, row: 0 };
}
