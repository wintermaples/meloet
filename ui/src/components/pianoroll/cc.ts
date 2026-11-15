// CC の帯（仕様書 3 章の帯の表と「第 3 回のための決まり」の CC の帯、モック R3-Lanes）。DOM を知らない。
// 点は、テイクのコントロールの番号（tick の順）で呼ぶ。帯ごとに、その種類の点だけを拾う。
// 書く道具で押すと点がふえる、選ぶ道具で動かす・囲む、消す道具で押す・なぞると消す。離したときに意図を 1 つだけ送る。
// y は帯の上はしからの CSS px、x はノートの場所と同じ横の座標。

import { CONTROL_CC, CONTROL_PITCH_BEND, type Command, type ControlKind, type IndexRange, type TakeNotes } from "../../bridge";
import { t } from "../../text/ja";
import { pad3, signed4 } from "../meterFormat";
import type { Meter } from "../time";
import { indexesOf, positionText, snapTick, tickToX, xToTick, type RollGeom, type Shown } from "./geom";
import type { Mods } from "./drag";
import type { LaneTag, Tool } from "./view.svelte";

export const LANE_ORDER: readonly LaneTag[] = ["sustain", "pitchBend", "modulation", "other"];
export const SUSTAIN_CC = 64;
export const MODULATION_CC = 1;
export const CC_MAX = 127;
export const PITCH_BEND_MIN = -8192;
export const PITCH_BEND_MAX = 8191;
/** 帯がひらいても、ノートの場所はこの行の数を下まわらない（仕様書 3 章の帯の表）。 */
export const MIN_NOTE_ROWS = 10;

/** その帯が見せるコントロール。「そのほかの CC」で番号をまだえらんでいなければ null。 */
export function laneControl(tag: LaneTag, other: number | null): ControlKind | null {
  switch (tag) {
    case "sustain":
      return { kind: "cc", number: SUSTAIN_CC };
    case "pitchBend":
      return { kind: "pitchBend" };
    case "modulation":
      return { kind: "cc", number: MODULATION_CC };
    case "other":
      return other === null ? null : { kind: "cc", number: other };
  }
}

/** オン ・ オフの CC（MIDI の 64〜69: サステイン、ポルタメント、ソステヌート、ソフト、レガート、ホールド 2）。0 か 127 にだけ止まる。 */
export const isOnOff = (c: ControlKind) => c.kind === "cc" && c.number >= 64 && c.number <= 69;

export function valueRange(c: ControlKind): { min: number; max: number } {
  return c.kind === "pitchBend" ? { min: PITCH_BEND_MIN, max: PITCH_BEND_MAX } : { min: 0, max: CC_MAX };
}

export const snapOnOff = (v: number) => (v >= CC_MAX / 2 ? CC_MAX : 0);

/** 範囲に止め、オン ・ オフの CC は 0 か 127 に吸いつける。 */
export function settle(c: ControlKind, v: number): number {
  const r = valueRange(c);
  const inRange = Math.min(r.max, Math.max(r.min, Math.round(v)));
  return isOnOff(c) ? snapOnOff(inRange) : inRange;
}

// ---- 値と高さ ----

/** 帯の高さ（CSS px）と、上下のあき（127 と 0、ピッチベンドの両はしの点の中心まで）。 */
export interface LaneBox {
  height: number;
  pad: number;
}

/** 値 → 帯の上はしからの y（CSS px）。ピッチベンドは 0 がまん中。 */
export function valueToY(c: ControlKind, b: LaneBox, v: number): number {
  if (c.kind === "pitchBend") {
    const mid = b.height / 2;
    return mid - (v / -PITCH_BEND_MIN) * (mid - b.pad);
  }
  return b.height - b.pad - (v / CC_MAX) * (b.height - 2 * b.pad);
}

/** y → 値（丸めない、範囲に止めない）。 */
export function yToRawValue(c: ControlKind, b: LaneBox, y: number): number {
  if (c.kind === "pitchBend") {
    const mid = b.height / 2;
    return ((mid - y) / Math.max(1, mid - b.pad)) * -PITCH_BEND_MIN;
  }
  return ((b.height - b.pad - y) / Math.max(1, b.height - 2 * b.pad)) * CC_MAX;
}

export const yToValue = (c: ControlKind, b: LaneBox, y: number) => settle(c, yToRawValue(c, b, y));

// ---- 帯の点 ----

const laneMemo = new WeakMap<TakeNotes, Map<string, Uint32Array>>();
const laneKey = (c: ControlKind) => (c.kind === "cc" ? `cc${c.number}` : "pb");

/** テイクのコントロールのうち、c の種類の物の番号（tick の順）。 */
export function lanePoints(take: TakeNotes, c: ControlKind): Uint32Array {
  let byLane = laneMemo.get(take);
  if (!byLane) laneMemo.set(take, (byLane = new Map()));
  const key = laneKey(c);
  const had = byLane.get(key);
  if (had) return had;
  const kind = c.kind === "pitchBend" ? CONTROL_PITCH_BEND : CONTROL_CC;
  const number = c.kind === "cc" ? c.number : 0;
  const out: number[] = [];
  for (let i = 0; i < take.controlTicks.length; i++) {
    if (take.controlKinds[i] === kind && (kind === CONTROL_PITCH_BEND || take.controlNumbers[i] === number)) out.push(i);
  }
  const points = Uint32Array.from(out);
  byLane.set(key, points);
  return points;
}

export const pointTick = (n: Shown, i: number) => n.base + n.take.controlTicks[i]!;

/** ticks の順の points のうち、tick が from 以上の最初の位置（二分探索）。 */
export function firstPointFrom(n: Shown, points: Uint32Array, from: number): number {
  let lo = 0;
  let hi = points.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (pointTick(n, points[mid]!) < from) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** (x, y) にいちばん近い点（縦も横も reach 以内）。なければ -1。 */
export function hitPoint(n: Shown | null, c: ControlKind, g: RollGeom, b: LaneBox, x: number, y: number, reach: number): number {
  if (!n) return -1;
  const points = lanePoints(n.take, c);
  let best = -1;
  let bestD = Infinity;
  for (let k = firstPointFrom(n, points, xToTick(g, x - reach)); k < points.length; k++) {
    const i = points[k]!;
    const px = tickToX(g, pointTick(n, i));
    if (px > x + reach) break;
    const py = valueToY(c, b, n.take.controlValues[i]!);
    if (Math.abs(py - y) > reach) continue;
    const d = Math.abs(px - x) + Math.abs(py - y);
    if (d < bestD) {
      bestD = d;
      best = i;
    }
  }
  return best;
}

/** 囲んだ枠（どの向きでも）にかかる点。 */
export function pointsInBox(n: Shown | null, c: ControlKind, g: RollGeom, b: LaneBox, ax: number, ay: number, bx: number, by: number): number[] {
  if (!n) return [];
  const points = lanePoints(n.take, c);
  const t1 = xToTick(g, Math.max(ax, bx));
  const y0 = Math.min(ay, by);
  const y1 = Math.max(ay, by);
  const out: number[] = [];
  for (let k = firstPointFrom(n, points, xToTick(g, Math.min(ax, bx))); k < points.length; k++) {
    const i = points[k]!;
    if (pointTick(n, i) > t1) break;
    const y = valueToY(c, b, n.take.controlValues[i]!);
    if (y >= y0 && y <= y1) out.push(i);
  }
  return out;
}

// ---- 帯の高さ ----

/**
 * ひらいている帯（ひらいた順）のうち、残す物。ノートの場所が minRows 行を下まわるあいだ、古いほうからたたむ。
 * いちばん新しい帯は残す（押したのに、ひらかないことがないように）。notesH はいまのノートの場所の高さ。
 */
export function fitLanes(open: readonly LaneTag[], notesH: number, laneH: number, rowH: number, minRows = MIN_NOTE_ROWS): LaneTag[] {
  const keep = [...open];
  let h = notesH;
  while (keep.length > 1 && Math.floor(h / rowH) < minRows) {
    keep.shift();
    h += laneH;
  }
  return keep;
}

// ---- 文 ----

/** 「+0512」（ピッチベンド）、「+012」（CC）。 */
export function deltaText(c: ControlKind, d: number): string {
  return c.kind === "pitchBend" ? signed4(d) : `${d < 0 ? "−" : "+"}${pad3(Math.abs(d))}`;
}

/** 「+0512」（ピッチベンド）、「064」（CC）。入力計と同じ書きかた。 */
export function valueText(c: ControlKind, v: number): string {
  return c.kind === "pitchBend" ? signed4(v) : pad3(v);
}

/** オン ・ オフの CC の値: 「踏む（127）」「OFF（0）」。 */
export function switchText(v: number): string {
  return t("pr.readSwitch", { state: v >= CC_MAX / 2 ? t("pr.switchOn") : t("meter.off"), value: String(v) });
}

// ---- マウス ----

interface Press {
  x: number;
  y: number;
  moved: boolean;
}

export type LaneDrag =
  | { kind: "move"; press: Press; points: number[]; grab: number; ticks: number; value: number; selectOnClick: boolean }
  | { kind: "add"; press: Press; tick: number; value: number }
  | { kind: "box"; press: Press; x: number; y: number; add: boolean; hits: number[] }
  | { kind: "erase"; press: Press; x: number; y: number; hits: Set<number> };

export interface LaneInput {
  shown(): Shown | null;
  control(): ControlKind | null;
  geom(): RollGeom;
  box(): LaneBox;
  /** グリッド 1 つのティック */
  step(): number;
  meter(): Meter;
  tool(): Tool;
  /** pianoRoll.selectedControls（見せているテイクの物でなければ空） */
  selected(): readonly IndexRange[];
  /** 点をつかめる近さと、押しただけとみなす動き（どちらも space.1） */
  reachPx(): number;
  stillPx(): number;
  send(command: Command): void;
}

export class LaneMouse {
  drag: LaneDrag | null = null;
  /** 消す道具を乗せている点（-1 はなし） */
  eraseHover = -1;

  constructor(private readonly input: LaneInput) {}

  private hit(x: number, y: number): number {
    const i = this.input;
    const c = i.control();
    return c ? hitPoint(i.shown(), c, i.geom(), i.box(), x, y, i.reachPx()) : -1;
  }

  /** この帯の点のうち、選んでいる物。 */
  private selectedHere(): Set<number> {
    const n = this.input.shown();
    const c = this.input.control();
    if (!n || !c) return new Set();
    const mine = new Set(lanePoints(n.take, c));
    return new Set(indexesOf(this.input.selected()).filter((p) => mine.has(p)));
  }

  /** 点に乗っているか。 */
  over(x: number, y: number): boolean {
    return this.hit(x, y) >= 0;
  }

  /** 何も押していないときの、消す道具で乗せている点。変わったら true。 */
  hover(x: number, y: number): boolean {
    const erase = this.input.tool() === "erase" ? this.hit(x, y) : -1;
    const changed = erase !== this.eraseHover;
    this.eraseHover = erase;
    return changed;
  }

  down(x: number, y: number, mods: Mods): void {
    const i = this.input;
    const n = i.shown();
    const c = i.control();
    this.eraseHover = -1;
    if (!c) return;
    const press: Press = { x, y, moved: false };
    const hit = this.hit(x, y);
    if (i.tool() === "erase") {
      this.drag = { kind: "erase", press, x, y, hits: new Set(hit >= 0 ? [hit] : []) };
      return;
    }
    if (hit >= 0 && n) {
      const take = n.take.id;
      if (mods.mod) {
        i.send({ type: "selectControls", take, points: [hit], mode: "toggle" });
        return;
      }
      const selected = this.selectedHere();
      let points: number[];
      let selectOnClick = false;
      if (selected.has(hit)) {
        points = [...selected];
        selectOnClick = !mods.shift;
      } else {
        i.send({ type: "selectControls", take, points: [hit], mode: mods.shift ? "add" : "replace" });
        points = mods.shift ? [...selected, hit] : [hit];
      }
      this.drag = { kind: "move", press, points, grab: hit, ticks: 0, value: 0, selectOnClick };
      return;
    }
    if (i.tool() === "draw") {
      if (!n) return;
      this.drag = { kind: "add", press, tick: this.snapped(x, mods), value: yToValue(c, i.box(), y) };
      return;
    }
    this.drag = { kind: "box", press, x, y, add: mods.shift, hits: [] };
  }

  private snapped(x: number, mods: Mods): number {
    const tick = xToTick(this.input.geom(), x);
    return Math.max(0, mods.shift ? Math.round(tick) : snapTick(tick, this.input.step()));
  }

  /** 押したまま動かした。見た目が変わったら true。 */
  move(x: number, y: number, mods: Mods): boolean {
    const d = this.drag;
    const i = this.input;
    const n = i.shown();
    const c = i.control();
    if (!d || !c) return false;
    if (!d.press.moved) {
      if (Math.hypot(x - d.press.x, y - d.press.y) < i.stillPx() && d.kind !== "erase") return false;
      d.press.moved = true;
    }
    const g = i.geom();
    const b = i.box();
    switch (d.kind) {
      case "move": {
        if (!n) return false;
        const { ticks, value } = moveAmount(n, c, b, d, x - d.press.x, y, mods.shift ? null : i.step(), g.ppt);
        if (ticks === d.ticks && value === d.value) return false;
        d.ticks = ticks;
        d.value = value;
        return true;
      }
      case "add": {
        const tick = this.snapped(x, mods);
        const value = yToValue(c, b, y);
        if (tick === d.tick && value === d.value) return false;
        d.tick = tick;
        d.value = value;
        return true;
      }
      case "box": {
        d.x = x;
        d.y = y;
        d.hits = pointsInBox(n, c, g, b, d.press.x, d.press.y, x, y);
        return true;
      }
      case "erase": {
        const before = d.hits.size;
        // 速く動かしても間の点を飛ばさないように、つかめる近さずつ見る。
        const reach = i.reachPx();
        const steps = Math.max(1, Math.ceil(Math.hypot(x - d.x, y - d.y) / reach));
        for (let s = 1; s <= steps; s++) {
          const hit = hitPoint(n, c, g, b, d.x + ((x - d.x) * s) / steps, d.y + ((y - d.y) * s) / steps, reach);
          if (hit >= 0) d.hits.add(hit);
        }
        d.x = x;
        d.y = y;
        return d.hits.size !== before;
      }
    }
  }

  /** 離した。送った意図を返す。 */
  up(): Command | null {
    const d = this.drag;
    this.drag = null;
    const c = this.input.control();
    if (!d || !c) return null;
    const command = finishLane(d, this.input.shown()?.take.id ?? null, c);
    if (command) this.input.send(command);
    return command;
  }

  /** Esc: ドラッグをやめて、動かす前に戻す。やめたら true。 */
  cancel(): boolean {
    if (!this.drag) return false;
    this.drag = null;
    return true;
  }

  /** 「その場の読み値」と状態行に出す文。 */
  reading(): string | null {
    const d = this.drag;
    const n = this.input.shown();
    const c = this.input.control();
    if (!d || !c) return null;
    const meter = this.input.meter();
    switch (d.kind) {
      case "move": {
        if (!d.press.moved || !n) return null;
        if (isOnOff(c)) return switchText(settle(c, n.take.controlValues[d.grab]! + d.value));
        return t("pr.readControl", { delta: deltaText(c, d.value), position: positionText(pointTick(n, d.grab) + d.ticks, meter) });
      }
      case "add":
        return isOnOff(c) ? switchText(d.value) : t("pr.readControl", { delta: valueText(c, d.value), position: positionText(d.tick, meter) });
      case "box":
        return d.press.moved ? t("pr.points", { count: d.hits.length }) : null;
      case "erase":
        return null;
    }
  }
}

/**
 * つかんだ点がグリッドに吸いつくように横へ動かす量と、縦の値の差。曲の頭より前へは出ない。
 * 値は、動かす点がどれも範囲に入るように止める。オン ・ オフの CC は、つかんだ点が 0 か 127 に吸いつく差。
 */
export function moveAmount(
  n: Shown,
  c: ControlKind,
  b: LaneBox,
  d: { points: readonly number[]; grab: number; press: { y: number } },
  dx: number,
  y: number,
  step: number | null,
  ppt: number,
): { ticks: number; value: number } {
  const start = pointTick(n, d.grab);
  const target = start + dx / ppt;
  let ticks = (step ? snapTick(target, step) : Math.round(target)) - start;
  let first = Infinity;
  let low = Infinity;
  let high = -Infinity;
  for (const p of d.points) {
    first = Math.min(first, pointTick(n, p));
    low = Math.min(low, n.take.controlValues[p]!);
    high = Math.max(high, n.take.controlValues[p]!);
  }
  ticks = Math.max(ticks, -first);
  const grabbed = n.take.controlValues[d.grab]!;
  const raw = yToRawValue(c, b, y) - yToRawValue(c, b, d.press.y);
  let value: number;
  if (isOnOff(c)) value = snapOnOff(Math.min(CC_MAX, Math.max(0, grabbed + raw))) - grabbed;
  else {
    const r = valueRange(c);
    value = Math.min(r.max - high, Math.max(r.min - low, Math.round(raw)));
  }
  return { ticks: ticks === 0 ? 0 : ticks, value: value === 0 ? 0 : value };
}

/** 離したときに送る意図。 */
export function finishLane(d: LaneDrag, take: number | null, c: ControlKind): Command | null {
  if (take === null) return null;
  switch (d.kind) {
    case "move":
      if (!d.press.moved || (d.ticks === 0 && d.value === 0)) {
        return !d.press.moved && d.selectOnClick ? { type: "selectControls", take, points: [d.grab], mode: "replace" } : null;
      }
      return { type: "moveControls", take, points: [...d.points].sort((a, b) => a - b), ticks: d.ticks, value: d.value };
    case "add":
      return { type: "addControl", take, tick: d.tick, control: c, value: d.value };
    case "box":
      if (!d.press.moved || (d.hits.length === 0 && !d.add)) return d.add ? null : { type: "selectControls", take, points: [], mode: "replace" };
      if (d.hits.length === 0) return null;
      return { type: "selectControls", take, points: d.hits, mode: d.add ? "add" : "replace" };
    case "erase":
      return d.hits.size > 0 ? { type: "deleteControls", take, points: [...d.hits].sort((a, b) => a - b) } : null;
  }
}

/** 動かしている間の点の値と位置（ほかの点はそのまま）。 */
export function draggedPoint(n: Shown, c: ControlKind, d: LaneDrag | null, moving: Set<number> | null, i: number): { tick: number; value: number } {
  const tick = pointTick(n, i);
  const value = n.take.controlValues[i]!;
  if (!d || d.kind !== "move" || !moving || !moving.has(i)) return { tick, value };
  return { tick: tick + d.ticks, value: settle(c, value + d.value) };
}
