// Shape math for the piano roll (rows, grid, note area, drag amounts, reading text). Pure functions that do not know the DOM.
// Positions are ticks in the song. x is "CSS px from the left edge of the note area", y is "CSS px from the top edge of the note area".

import { CONTROL_CC, CONTROL_PITCH_BEND, NO_FROM, type Grid, type IndexRange, type MusicalScale, type ScaleMode, type TakeNotes } from "../../bridge";
import type { ColorName } from "../../design/tokens";
import { isBlack } from "../../gl/keyboard";
import { barsMap } from "../../time/bars";
import { barBeatTick, TICKS_PER_QUARTER, tickText, ticksPerBar, ticksPerBeat, type Bars, type Meter } from "../time";
import { QUARTER_PX_STEPS } from "./view.svelte";

export const KEY_MIN = 0;
export const KEY_MAX = 127;
export const VELOCITY_MIN = 1;
export const VELOCITY_MAX = 127;
/** The velocity of notes written with the draw tool (not in the spec; a question for the lead). */
export const DRAW_VELOCITY = 100;

/** How the note area is viewed (CSS px). */
export interface RollGeom {
  scrollTick: number;
  /** CSS px per tick */
  ppt: number;
  /** The key of the top row */
  topKey: number;
  /** size.note: the height of one row */
  rowH: number;
  width: number;
  height: number;
  /** When drum rows are shown (spec 3 "Drum piano roll"). topKey is not used. */
  drum?: DrumGeom;
}

/** Drum rows: only the rows of fixed keys, from the top. A note is a square at the place it is hit. */
export interface DrumGeom {
  /** The row keys (from the top row). */
  keys: readonly number[];
  /** The top visible row (an index in keys). */
  top: number;
  /** The square width (CSS px). One grid cell − 2, not smaller than 6. */
  hitW: number;
  /** The table of row names (a built-in kit or an SF2 kit). */
  names?: "builtin" | "sf2";
}

export const tickToX = (g: RollGeom, tick: number) => (tick - g.scrollTick) * g.ppt;
export const xToTick = (g: RollGeom, x: number) => g.scrollTick + x / g.ppt;
/** The top edge of a key's row. NaN for keys not in the drum rows. */
export const keyToY = (g: RollGeom, key: number) => {
  if (!g.drum) return (g.topKey - key) * g.rowH;
  const row = g.drum.keys.indexOf(key);
  return row < 0 ? NaN : (row - g.drum.top) * g.rowH;
};
/** The key of the row at y. -1 outside the drum rows (below the bottom row). */
export const yToKey = (g: RollGeom, y: number) => {
  if (!g.drum) return g.topKey - Math.floor(y / g.rowH);
  return g.drum.keys[g.drum.top + Math.floor(y / g.rowH)] ?? -1;
};
/** The square width in ticks. */
const hitTicks = (g: RollGeom) => (g.drum ? g.drum.hitW / g.ppt : 0);
/** The number of visible rows (including the partial row at the bottom edge). */
export const rowsShown = (g: RollGeom) => Math.ceil(g.height / g.rowH);
/** The number of fully visible rows (the row count in spec 3 "Adding up the piano roll height"). */
export const wholeRows = (height: number, rowH: number) => Math.floor(height / rowH);

// ---- Notes of the shown take (read at their positions in the song) ----

export interface Shown {
  take: TakeNotes;
  /** The song position of the take's tick 0 (clip start + take_offset). */
  base: number;
  /** The longest note (to find what is visible). */
  maxLength: number;
  /** The clip's range [from, to) (in the song). Notes that start outside it (moved out by trimming the edge) are not shown. */
  from: number;
  to: number;
}

export function shownOf(take: TakeNotes, base: number, range: { from: number; to: number } = { from: -Infinity, to: Infinity }): Shown {
  let maxLength = 0;
  for (let i = 0; i < take.lengths.length; i++) maxLength = Math.max(maxLength, take.lengths[i]!);
  return { take, base, maxLength, from: range.from, to: range.to };
}

/** Whether to show note i (whether it starts inside the clip's range). */
export const noteShown = (n: Shown, i: number) => {
  const s = n.base + n.take.starts[i]!;
  return s >= n.from && s < n.to;
};

export const noteStart = (n: Shown, i: number) => n.base + n.take.starts[i]!;
export const noteEnd = (n: Shown, i: number) => n.base + n.take.starts[i]! + n.take.lengths[i]!;
export const noteCount = (n: Shown | null) => n?.take.starts.length ?? 0;

/** The first index of the notes that may touch ticks [from, to) (binary search, since they are in start order). */
export function firstNoteFrom(n: Shown, from: number): number {
  const target = from - n.base - n.maxLength;
  const s = n.take.starts;
  let lo = 0;
  let hi = s.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (s[mid]! < target) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Passes the indexes of notes touching ticks [from, to) to each, in order. Notes that start outside the clip's range are not passed. */
export function eachNoteIn(n: Shown, from: number, to: number, each: (i: number) => void): void {
  const s = n.take.starts;
  const l = n.take.lengths;
  for (let i = firstNoteFrom(n, from); i < s.length; i++) {
    const start = n.base + s[i]!;
    if (start >= to || start >= n.to) break;
    if (start + l[i]! > from && start >= n.from) each(i);
  }
}

export interface Hit {
  note: number;
  /** The 6px at the edge (where the length is changed). */
  edge: boolean;
  /** The left edge (where the start is moved; docs/history/design-notes.ja.md B-69). Only when hitNote is called with startEdge. */
  start?: true;
}

/**
 * The edge width (CSS px): normally edgePx. When both sides are checked and the note is so short that the left and right edgePx overlap, each side gets 1/3 of the width.
 * When only the right side is checked, up to half the width.
 */
export function edgeWidth(noteW: number, edgePx: number, both: boolean): number {
  if (!both) return Math.min(edgePx, noteW / 2);
  return 2 * edgePx > noteW ? noteW / 3 : edgePx;
}

/** The note under (x, y). If notes overlap, the one drawn later (the larger index). With startEdge, the left edge is checked too. For drums, only the square is checked (no edges). */
export function hitNote(n: Shown | null, g: RollGeom, x: number, y: number, edgePx: number, startEdge = false): Hit | null {
  if (!n || y < 0 || y >= g.height) return null;
  const key = yToKey(g, y);
  const tick = xToTick(g, x);
  let found: Hit | null = null;
  if (g.drum) {
    const w = g.drum.hitW;
    eachNoteIn(n, tick - hitTicks(g), tick + 1e-9, (i) => {
      if (n.take.keys[i] !== key) return;
      const x0 = tickToX(g, noteStart(n, i));
      if (x >= x0 && x < x0 + w + 1) found = { note: i, edge: false };
    });
    return found;
  }
  eachNoteIn(n, tick, tick + 1e-9, (i) => {
    if (n.take.keys[i] !== key) return;
    const x0 = tickToX(g, noteStart(n, i));
    const x1 = tickToX(g, noteEnd(n, i));
    if (x < x0 || x >= x1) return;
    const w = edgeWidth(x1 - x0, edgePx, startEdge);
    if (x >= x1 - w) found = { note: i, edge: true };
    else if (startEdge && x < x0 + w) found = { note: i, edge: true, start: true };
    else found = { note: i, edge: false };
  });
  return found;
}

/** Notes touched by the box (CSS px, in any direction). */
export function notesInBox(n: Shown | null, g: RollGeom, ax: number, ay: number, bx: number, by: number): number[] {
  if (!n) return [];
  const t0 = xToTick(g, Math.min(ax, bx));
  const t1 = xToTick(g, Math.max(ax, bx));
  const out: number[] = [];
  if (g.drum) {
    const d = g.drum;
    const top = d.top + Math.floor(Math.max(0, Math.min(ay, by)) / g.rowH);
    const bottom = d.top + Math.floor(Math.max(ay, by) / g.rowH);
    eachNoteIn(n, t0 - hitTicks(g), t1, (i) => {
      const row = d.keys.indexOf(n.take.keys[i]!);
      if (row >= top && row <= bottom && noteStart(n, i) >= t0 - hitTicks(g)) out.push(i);
    });
    return out;
  }
  const high = yToKey(g, Math.min(ay, by));
  const low = yToKey(g, Math.max(ay, by));
  eachNoteIn(n, t0, t1, (i) => {
    const k = n.take.keys[i]!;
    if (k >= low && k <= high) out.push(i);
  });
  return out;
}

// ---- Selection ----

export function indexesOf(ranges: readonly IndexRange[]): number[] {
  const out: number[] = [];
  for (const r of ranges) for (let i = r.start; i < r.end; i++) out.push(i);
  return out;
}

export function selectionFlags(count: number, ranges: readonly IndexRange[]): Uint8Array {
  const flags = new Uint8Array(count);
  for (const r of ranges) flags.fill(1, Math.min(r.start, count), Math.min(r.end, count));
  return flags;
}

// ---- Grid and scale ----

/** The ticks of one grid step. division 0 is one bar. Triplets are 2/3. */
export function gridTicks(grid: Grid, meter: Meter): number {
  const plain = grid.division === 0 ? ticksPerBar(meter) : (TICKS_PER_QUARTER * 4) / grid.division;
  return Math.max(1, Math.round(grid.triplet ? (plain * 2) / 3 : plain));
}

export const snapTick = (tick: number, step: number) => Math.round(tick / step) * step;

/**
 * Snaps to the grid (in songs with time signature changes, counted from the start of the time signature span; gridTick in time/bars.ts).
 * With floor, the start of the grid step at the pressed place. If not given, an even grid from 0 in steps of step.
 */
export type Snapper = (tick: number, floor: boolean) => number;
const snapWith = (grid: Snapper | undefined, tick: number, step: number) => (grid ? grid(tick, false) : snapTick(tick, step));
export const floorTick = (tick: number, step: number) => Math.floor(tick / step) * step;

/** The list of grids (the owner's decision). 0 is one bar. */
export const GRID_DIVISIONS = [0, 2, 4, 8, 16, 32] as const;

/** The same table as `ScaleMode::steps` in crates/meloet-model/src/roll.rs (held to scaleSteps.table.json by ui/tests). */
export const SCALE_STEPS = {
  major: [0, 2, 4, 5, 7, 9, 11],
  minor: [0, 2, 3, 5, 7, 8, 10],
  harmonicMinor: [0, 2, 3, 5, 7, 8, 11],
  melodicMinor: [0, 2, 3, 5, 7, 9, 11],
  majorPentatonic: [0, 2, 4, 7, 9],
  minorPentatonic: [0, 3, 5, 7, 10],
  blues: [0, 3, 5, 6, 7, 10],
  dorian: [0, 2, 3, 5, 7, 9, 10],
  mixolydian: [0, 2, 4, 5, 7, 9, 10],
} as const satisfies Record<ScaleMode, readonly number[]>;
export const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;

export function inScale(key: number, s: Pick<MusicalScale, "root" | "mode">): boolean {
  return (SCALE_STEPS[s.mode] as readonly number[]).includes((((key - s.root) % 12) + 12) % 12);
}

/** With scale lock on, rows outside the scale cannot be written. */
export const writable = (key: number, s: MusicalScale) => !s.lock || inScale(key, s);

/** C4 = 60 (the keyboard column in mock A-PianoRoll). */
export function noteName(key: number): string {
  return `${NOTE_NAMES[((key % 12) + 12) % 12]}${Math.floor(key / 12) - 1}`;
}

/** Row background: white keys row.white, black keys row.black, rows outside the scale row.out while the scale is locked. */
export function rowColor(key: number, s: MusicalScale): ColorName {
  if (s.lock && !inScale(key, s)) return "color.row.out";
  return isBlack(key) ? "color.row.black" : "color.row.white";
}

/** How much of the accent a note face has (spec 2 "Note"): up to 69 a half, 70 to 89 three quarters, 90 and up all of it. */
export function velocityStrength(v: number): number {
  return v < 70 ? 0.5 : v < 90 ? 0.75 : 1;
}

// ---- How it looks when opened ----

/**
 * The key of the top row when opened. Leave 4 rows above the highest note, and if that does not leave 3 rows below the lowest note,
 * show the low side (A-PianoRoll is 41 rows with E5, F-PR150 is 18 rows with D4; both have notes C3 to C5).
 * With no notes, C4 in the middle.
 */
export function fitTopKey(range: { low: number; high: number } | null, rows: number): number {
  const r = Math.max(1, rows);
  const top = range ? Math.min(range.high + 4, range.low - 3 + r - 1) : 60 + Math.floor(r / 2);
  return clampTopKey(top, r);
}

export function clampTopKey(top: number, rows: number): number {
  return Math.min(KEY_MAX, Math.max(Math.min(KEY_MAX, Math.max(0, rows - 1)), Math.round(top)));
}

export function keyRangeOf(n: Shown | null): { low: number; high: number } | null {
  if (!n) return null;
  let low = KEY_MAX + 1;
  let high = KEY_MIN - 1;
  for (let i = 0; i < n.take.keys.length; i++) {
    if (!noteShown(n, i)) continue;
    low = Math.min(low, n.take.keys[i]!);
    high = Math.max(high, n.take.keys[i]!);
  }
  return low <= high ? { low, high } : null;
}

/** What a clip opened shows at first: the loop when it is on and overlaps the clip, else the whole clip. */
export function openSpan(clip: { start: number; end: number }, loop: { start: number; end: number } | null): { start: number; end: number } {
  return loop && loop.start < clip.end && clip.start < loop.end ? { start: loop.start, end: loop.end } : { start: clip.start, end: clip.end };
}

/**
 * The horizontal zoom when opened: the largest step at which the clip fits in the note area (the mock is 40 per beat at 1280, 80 at 1920).
 * width is CSS px with the scale applied, scale is the scale.
 */
export function fitQuarterPx(clipTicks: number, width: number, scale: number): number {
  const quarters = clipTicks / TICKS_PER_QUARTER;
  let best: number = QUARTER_PX_STEPS[0];
  for (const step of QUARTER_PX_STEPS) if (quarters * step * (scale / 100) <= width) best = step;
  return best;
}

// ---- Drag amounts ----

/** The amount to move so the start of the grabbed note snaps to the grid. If step is null (Shift), it does not snap. It does not go before the start of the song or outside the keys. */
export function moveDelta(
  n: Shown,
  notes: readonly number[],
  grab: number,
  dxTicks: number,
  dKeys: number,
  step: number | null,
  grid?: Snapper,
): { ticks: number; keys: number } {
  const start = noteStart(n, grab);
  const target = start + dxTicks;
  let ticks = (step ? snapWith(grid, target, step) : Math.round(target)) - start;
  let low = KEY_MAX;
  let high = KEY_MIN;
  let first = Infinity;
  for (const i of notes) {
    low = Math.min(low, n.take.keys[i]!);
    high = Math.max(high, n.take.keys[i]!);
    first = Math.min(first, noteStart(n, i));
  }
  ticks = Math.max(ticks, -first);
  const keys = Math.min(KEY_MAX - high, Math.max(KEY_MIN - low, dKeys));
  return { ticks: ticks === 0 ? 0 : ticks, keys: keys === 0 ? 0 : keys };
}

/**
 * The key change when moving the grabbed note rows rows (down is +) in drum rows. As far as all moved notes move by the same number of rows and stay in the kit's
 * keys (if not, it goes toward 0). All notes move by the same key change, so they can only move where the key changes between rows match.
 */
export function drumMoveKeys(d: Pick<DrumGeom, "keys">, keys: ArrayLike<number>, notes: readonly number[], grab: number, rows: number): number {
  const rowOf = (k: number) => d.keys.indexOf(k);
  const grabRow = rowOf(keys[grab]!);
  if (grabRow < 0) return 0;
  for (let r = rows; r !== 0; r -= Math.sign(r)) {
    const target = grabRow + r;
    if (target < 0 || target >= d.keys.length) continue;
    const dk = d.keys[target]! - keys[grab]!;
    if (notes.every((i) => rowOf(keys[i]!) >= 0 && rowOf(keys[i]! + dk) === rowOf(keys[i]!) + r)) return dk;
  }
  return 0;
}

/** The amount to change the length so the end of the grabbed note snaps to the grid. No note gets shorter than 1 tick (when snapping, the grabbed note does not get shorter than one grid step). */
export function resizeDelta(n: Shown, notes: readonly number[], grab: number, dxTicks: number, step: number | null, grid?: Snapper): number {
  const end = noteEnd(n, grab);
  const target = end + dxTicks;
  let ticks = (step ? snapWith(grid, target, step) : Math.round(target)) - end;
  const grabLength = n.take.lengths[grab]!;
  if (step) ticks = Math.max(ticks, Math.min(step, grabLength) - grabLength);
  let shortest = Infinity;
  for (const i of notes) shortest = Math.min(shortest, n.take.lengths[i]!);
  ticks = Math.max(ticks, 1 - shortest);
  return ticks === 0 ? 0 : ticks;
}

/**
 * The amount to move the start so the start of the grabbed note snaps to the grid (the end does not move; + makes it shorter; docs/history/design-notes.ja.md B-69).
 * No note gets shorter than 1 tick (when snapping, the grabbed note does not get shorter than one grid step), and none goes before the start of the song.
 */
export function resizeStartDelta(n: Shown, notes: readonly number[], grab: number, dxTicks: number, step: number | null, grid?: Snapper): number {
  const start = noteStart(n, grab);
  const target = start + dxTicks;
  let ticks = (step ? snapWith(grid, target, step) : Math.round(target)) - start;
  const grabLength = n.take.lengths[grab]!;
  if (step) ticks = Math.min(ticks, grabLength - Math.min(step, grabLength));
  let shortest = Infinity;
  let first = Infinity;
  for (const i of notes) {
    shortest = Math.min(shortest, n.take.lengths[i]!);
    first = Math.min(first, noteStart(n, i));
  }
  ticks = Math.min(ticks, shortest - 1);
  ticks = Math.max(ticks, -first);
  return ticks === 0 ? 0 : ticks;
}

/** The start of the note at the place pressed with the draw tool. When snapping, the start of the pressed grid step. */
export function drawStart(tick: number, step: number | null, grid?: Snapper): number {
  return Math.max(0, step ? (grid ? grid(tick, true) : floorTick(tick, step)) : Math.round(tick));
}

/** The length when the draw tool is held and moved right. At least one grid step (1 tick when not snapping). */
export function drawLength(start: number, tick: number, step: number, snap: boolean, grid?: Snapper): number {
  return snap ? Math.max(step, snapWith(grid, tick, step) - start) : Math.max(1, Math.round(tick) - start);
}

/** The upward movement (CSS px) in the velocity lane as a change in value. full is the height of the bar at 127 (CSS px). */
export function velocityDelta(dyUp: number, full: number): number {
  const d = Math.round((dyUp * VELOCITY_MAX) / full);
  return d === 0 ? 0 : d;
}

export const clampVelocity = (v: number) => Math.min(VELOCITY_MAX, Math.max(VELOCITY_MIN, v));

/** Mod D: the length of the selected notes' range, rounded up to the grid. */
export function duplicateTicks(n: Shown, notes: readonly number[], step: number): number {
  let first = Infinity;
  let last = -Infinity;
  for (const i of notes) {
    first = Math.min(first, noteStart(n, i));
    last = Math.max(last, noteEnd(n, i));
  }
  if (!(last > first)) return step;
  return Math.max(step, Math.ceil((last - first) / step) * step);
}

/** In a quantized take, the original index of each note whose position, length or key differs from the original note (-1 if the same). */
export function movedFrom(take: TakeNotes, origin: TakeNotes, i: number): number {
  const j = take.froms[i]!;
  if (j === NO_FROM || j >= origin.starts.length) return -1;
  const same = origin.starts[j] === take.starts[i] && origin.lengths[j] === take.lengths[i] && origin.keys[j] === take.keys[i];
  return same ? -1 : j;
}

// ---- Text ----

/** 「3.1.0000」 (bar . beat . tick). */
export function positionText(tick: number, bars: Bars): string {
  const b = barBeatTick(tick, bars);
  return `${b.bar}.${b.beat}.${tickText(b.tick)}`;
}

/** 「1.0960」 (beat . tick). */
export function lengthText(ticks: number, meter: Meter): string {
  const perBeat = ticksPerBeat(meter);
  const t = Math.max(0, Math.round(Math.abs(ticks)));
  const beats = Math.floor(t / perBeat);
  return `${beats}.${tickText(t % perBeat)}`;
}

/** 「+2」「−2」. 0 is 「+0」. */
export const signed = (v: number) => `${v < 0 ? "−" : "+"}${Math.abs(v)}`;
/** 「+0.0960」「−1.0000」。 */
export const signedLength = (ticks: number, meter: Meter) => `${ticks < 0 ? "−" : "+"}${lengthText(ticks, meter)}`;

/** 「1–4」: the bars of the clip (including the last bar). */
export function barsText(start: number, end: number, bars: Bars): string {
  const map = barsMap(bars);
  const first = map.meterAt(start).bar + 1;
  const e = map.meterAt(end);
  const last = Math.max(first, e.barTick === Math.max(0, Math.floor(end)) ? e.bar : e.bar + 1);
  return `${first}–${last}`;
}

// ---- CC tab counts ----

export interface CcCounts {
  /** How many times sustain was pressed (how many times CC 64 went from below 64 to 64 or more) */
  sustain: number;
  pitchBend: number;
  modulation: number;
  /** CC numbers other than 1 and 64 */
  others: number[];
  /** The number of control points */
  total: number;
}

export function ccCounts(take: TakeNotes | null): CcCounts {
  const out: CcCounts = { sustain: 0, pitchBend: 0, modulation: 0, others: [], total: 0 };
  if (!take) return out;
  let down = false;
  const others = new Set<number>();
  for (let i = 0; i < take.controlTicks.length; i++) {
    out.total += 1;
    const kind = take.controlKinds[i];
    const number = take.controlNumbers[i]!;
    if (kind === CONTROL_PITCH_BEND) out.pitchBend += 1;
    else if (kind === CONTROL_CC && number === 64) {
      const on = take.controlValues[i]! >= 64;
      if (on && !down) out.sustain += 1;
      down = on;
    } else if (kind === CONTROL_CC && number === 1) out.modulation += 1;
    else if (kind === CONTROL_CC) others.add(number);
  }
  out.others = [...others].sort((a, b) => a - b);
  return out;
}
