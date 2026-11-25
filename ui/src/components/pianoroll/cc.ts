// CC lanes (the band table in spec 3 and the CC lanes in "Rules for round 3 (piano roll and clips)", mock R3-Lanes). Does not know the DOM.
// A point is named by its index among the take's controls (in tick order). Each lane picks only the points of its kind.
// Pressing with the draw tool adds a point; the select tool moves or boxes; the erase tool removes by pressing or tracing. Only one intent is sent on release.
// y is CSS px from the top edge of the lane; x is the same horizontal coordinate as the note area.

import { CONTROL_CC, CONTROL_PITCH_BEND, type Command, type ControlKind, type IndexRange, type TakeNotes } from "../../bridge";
import { t } from "../../text";
import { pad3, signed4 } from "../meterFormat";
import type { Bars } from "../time";
import { indexesOf, positionText, snapTick, tickToX, xToTick, type RollGeom, type Shown, type Snapper } from "./geom";
import type { Mods } from "./drag";
import type { LaneTag, Tool } from "./view.svelte";

export const LANE_ORDER: readonly LaneTag[] = ["sustain", "pitchBend", "modulation", "other"];
export const SUSTAIN_CC = 64;
export const MODULATION_CC = 1;
export const CC_MAX = 127;
export const PITCH_BEND_MIN = -8192;
export const PITCH_BEND_MAX = 8191;
/** Even when lanes open, the note area does not go below this many rows (the band table in spec 3). */
export const MIN_NOTE_ROWS = 10;

/** The control the lane shows. Null if the number for 「そのほかの CC」 is not chosen yet. */
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

/** On/off CCs (MIDI 64 to 69: sustain, portamento, sostenuto, soft, legato, hold 2). They only stop at 0 or 127. */
export const isOnOff = (c: ControlKind) => c.kind === "cc" && c.number >= 64 && c.number <= 69;

export function valueRange(c: ControlKind): { min: number; max: number } {
  return c.kind === "pitchBend" ? { min: PITCH_BEND_MIN, max: PITCH_BEND_MAX } : { min: 0, max: CC_MAX };
}

export const snapOnOff = (v: number) => (v >= CC_MAX / 2 ? CC_MAX : 0);

/** Clamps to the range, and snaps on/off CCs to 0 or 127. */
export function settle(c: ControlKind, v: number): number {
  const r = valueRange(c);
  const inRange = Math.min(r.max, Math.max(r.min, Math.round(v)));
  return isOnOff(c) ? snapOnOff(inRange) : inRange;
}

// ---- Values and height ----

/** The lane height (CSS px) and the space above and below (to the centers of the points at 127 and 0, and at both ends of pitch bend). */
export interface LaneBox {
  height: number;
  pad: number;
}

/** Value → y from the top edge of the lane (CSS px). For pitch bend, 0 is the middle. */
export function valueToY(c: ControlKind, b: LaneBox, v: number): number {
  if (c.kind === "pitchBend") {
    const mid = b.height / 2;
    return mid - (v / -PITCH_BEND_MIN) * (mid - b.pad);
  }
  return b.height - b.pad - (v / CC_MAX) * (b.height - 2 * b.pad);
}

/** y → value (not rounded, not clamped). */
export function yToRawValue(c: ControlKind, b: LaneBox, y: number): number {
  if (c.kind === "pitchBend") {
    const mid = b.height / 2;
    return ((mid - y) / Math.max(1, mid - b.pad)) * -PITCH_BEND_MIN;
  }
  return ((b.height - b.pad - y) / Math.max(1, b.height - 2 * b.pad)) * CC_MAX;
}

export const yToValue = (c: ControlKind, b: LaneBox, y: number) => settle(c, yToRawValue(c, b, y));

// ---- Lane points ----

const laneMemo = new WeakMap<TakeNotes, Map<string, Uint32Array>>();
const laneKey = (c: ControlKind) => (c.kind === "cc" ? `cc${c.number}` : "pb");

/** The indexes of the take's controls of kind c (in tick order). */
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

/** The first index in points (in tick order) whose tick is from or more (binary search). */
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

/** The point nearest to (x, y) (within reach both vertically and horizontally). -1 if none. */
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

/** Points touched by the box (in any direction). */
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

// ---- Lane heights ----

/**
 * Which of the open lanes (in the order opened) to keep. While the note area is below minRows rows, the oldest are folded first.
 * The newest lane is kept (so pressing it never fails to open it). notesH is the current height of the note area.
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

// ---- Text ----

/** 「+0512」 (pitch bend), 「+012」 (CC). */
export function deltaText(c: ControlKind, d: number): string {
  return c.kind === "pitchBend" ? signed4(d) : `${d < 0 ? "−" : "+"}${pad3(Math.abs(d))}`;
}

/** 「+0512」 (pitch bend), 「064」 (CC). Written the same way as the input meter. */
export function valueText(c: ControlKind, v: number): string {
  return c.kind === "pitchBend" ? signed4(v) : pad3(v);
}

/** The value of an on/off CC: 「踏む（127）」「OFF（0）」. */
export function switchText(v: number): string {
  return t("pr.readSwitch", { state: v >= CC_MAX / 2 ? t("pr.switchOn") : t("meter.off"), value: String(v) });
}

// ---- Mouse ----

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
  /** The ticks of one grid step */
  step(): number;
  /** Snap to the grid the lane draws (counted from the start of the time signature span; from 0 in steps of step if not given). */
  grid?(): Snapper;
  /** The time signatures the readings count bars and beats in (the tempo map, or one time signature). */
  meter(): Bars;
  tool(): Tool;
  /** pianoRoll.selectedControls (empty if not from the shown take) */
  selected(): readonly IndexRange[];
  /** How near a point must be to grab it, and the movement still treated as a press (both space.1) */
  reachPx(): number;
  stillPx(): number;
  send(command: Command): void;
  /** While recording or counting in: Rust takes no edit, so a press only selects (no drag that edits starts). */
  busy?(): boolean;
}

export class LaneMouse {
  drag: LaneDrag | null = null;
  /** The point the erase tool is over (-1 for none) */
  eraseHover = -1;
  /** The take shown at the press: the indexes in the drag belong to it. */
  private pressTake: number | null = null;

  constructor(private readonly input: LaneInput) {}

  private hit(x: number, y: number): number {
    const i = this.input;
    const c = i.control();
    return c ? hitPoint(i.shown(), c, i.geom(), i.box(), x, y, i.reachPx()) : -1;
  }

  /** The selected points of this lane. */
  private selectedHere(): Set<number> {
    const n = this.input.shown();
    const c = this.input.control();
    if (!n || !c) return new Set();
    const mine = new Set(lanePoints(n.take, c));
    return new Set(indexesOf(this.input.selected()).filter((p) => mine.has(p)));
  }

  /** Whether it is over a point. */
  over(x: number, y: number): boolean {
    return this.hit(x, y) >= 0;
  }

  /** The point the erase tool is over when nothing is pressed. True if it changed. */
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
    const busy = i.busy?.() ?? false;
    this.eraseHover = -1;
    if (!c) return;
    this.pressTake = n?.take.id ?? null;
    const press: Press = { x, y, moved: false };
    const hit = this.hit(x, y);
    if (i.tool() === "erase") {
      if (busy) return;
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
      if (busy) return;
      this.drag = { kind: "move", press, points, grab: hit, ticks: 0, value: 0, selectOnClick };
      return;
    }
    if (i.tool() === "draw") {
      if (!n || busy) return;
      this.drag = { kind: "add", press, tick: this.snapped(x, mods), value: yToValue(c, i.box(), y) };
      return;
    }
    this.drag = { kind: "box", press, x, y, add: mods.shift, hits: [] };
  }

  private snapped(x: number, mods: Mods): number {
    const tick = xToTick(this.input.geom(), x);
    const grid = this.input.grid?.();
    return Math.max(0, mods.shift ? Math.round(tick) : grid ? grid(tick, false) : snapTick(tick, this.input.step()));
  }

  /** Moved while held. True if the look changed. */
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
        const { ticks, value } = moveAmount(n, c, b, d, x - d.press.x, y, mods.shift ? null : i.step(), g.ppt, i.grid?.());
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
        // Check one grab distance at a time, so points in between are not skipped when moving fast.
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

  /** Released. Returns the intent that was sent. */
  up(): Command | null {
    const d = this.drag;
    this.drag = null;
    const c = this.input.control();
    if (!d || !c) return null;
    const command = finishLane(d, this.pressTake, c);
    if (command) this.input.send(command);
    return command;
  }

  /** Esc: cancels the drag and goes back to before moving. True if cancelled. */
  cancel(): boolean {
    if (!this.drag) return false;
    this.drag = null;
    return true;
  }

  /** Cancels the drag when the shown take is no longer the one of the press (its indexes would point at other points). True if cancelled. */
  dropIfTakeChanged(): boolean {
    return this.drag !== null && (this.input.shown()?.take.id ?? null) !== this.pressTake && this.cancel();
  }

  /** The text for the 「その場の読み値」 and the status line. */
  reading(): string | null {
    const d = this.drag;
    const n = this.input.shown();
    const c = this.input.control();
    if (!d || !c) return null;
    const bars = this.input.meter();
    switch (d.kind) {
      case "move": {
        if (!d.press.moved || !n) return null;
        if (isOnOff(c)) return switchText(settle(c, n.take.controlValues[d.grab]! + d.value));
        return t("pr.readControl", { delta: deltaText(c, d.value), position: positionText(pointTick(n, d.grab) + d.ticks, bars) });
      }
      case "add":
        return isOnOff(c) ? switchText(d.value) : t("pr.readControl", { delta: valueText(c, d.value), position: positionText(d.tick, bars) });
      case "box":
        return d.press.moved ? t("pr.points", { count: d.hits.length }) : null;
      case "erase":
        return null;
    }
  }
}

/**
 * How far to move sideways so the grabbed point snaps to the grid, and the change in value. It does not go before the start of the song.
 * The value is clamped so all moved points stay in range. For on/off CCs, the change that snaps the grabbed point to 0 or 127.
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
  grid?: Snapper,
): { ticks: number; value: number } {
  const start = pointTick(n, d.grab);
  const target = start + dx / ppt;
  let ticks = (step ? (grid ? grid(target, false) : snapTick(target, step)) : Math.round(target)) - start;
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

/** The intent sent on release. */
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

/** The values and positions of points while moving (the other points stay as they are). */
export function draggedPoint(n: Shown, c: ControlKind, d: LaneDrag | null, moving: Set<number> | null, i: number): { tick: number; value: number } {
  const tick = pointTick(n, i);
  const value = n.take.controlValues[i]!;
  if (!d || d.kind !== "move" || !moving || !moving.has(i)) return { tick, value };
  return { tick: tick + d.ticks, value: settle(c, value + d.value) };
}
