// Reads the bar and beat lines from the tempo map (tempoMap.ts). Even when time signatures change the bar length, the ruler, grid and snapping use the same lines.
// Knows neither the DOM nor Tauri. Not used at 60 Hz (only when scrolling or when the song changes).

import { RANGE_MARK } from "../text";
import { TempoMap, type TimelineShape } from "./tempoMap";
import { TICKS_PER_QUARTER } from "./ticks";

const WHOLE = 4 * TICKS_PER_QUARTER;

/** A time signature only (the one at the song start, no tempo changes). */
export interface MeterShape {
  num: number;
  den: number;
}

/** What knows the bar lengths: a tempo map, or the time signature of a song with only one. */
export type Bars = TempoMap | MeterShape;

export const perBeatOf = (m: MeterShape) => WHOLE / m.den;
export const perBarOf = (m: MeterShape) => (m.num * WHOLE) / m.den;

const maps = new WeakMap<object, TempoMap>();
/** Per map, the start ticks of the time signature sections (in order, from 0 at the song start). */
const sections = new WeakMap<TempoMap, number[]>();
/** The map of the `timeline` section. The same section (the same object until a diff replaces it) gets the same map. */
export function mapOf(timeline: TimelineShape): TempoMap {
  let map = maps.get(timeline);
  if (!map) {
    const m = new TempoMap(timeline);
    const bars = [...new Set(timeline.meterChanges.map((c) => c.bar).filter((b) => b > 0))].sort((a, b) => a - b);
    sections.set(m, [0, ...bars.map((b) => m.tickOfBar(b))]);
    maps.set(timeline, m);
    map = m;
  }
  return map;
}

/**
 * The note grid (the same idea as quantize in core): count by step from the start of the time signature section; a step past the next time signature flag is the start of that flag's bar.
 * With floor, the start of the grid cell that was pressed (the draw tool); otherwise the nearest step (the later one at the middle). Before 0 is 0.
 * With bar (a grid of one bar), the start of the bar there.
 */
export function gridTick(map: TempoMap, tick: number, step: number, floor = false, bar = false): number {
  const t = Math.max(0, tick);
  if (bar) return floor ? map.meterAt(t).barTick : nearestBarTick(map, t);
  const heads = sections.get(map) ?? [0];
  let i = 0;
  while (i + 1 < heads.length && heads[i + 1]! <= t) i++;
  const head = heads[i]!;
  const next = heads[i + 1];
  const at = head + (floor ? Math.floor((t - head) / step) : Math.round((t - head) / step)) * step;
  if (next === undefined) return at;
  if (at > next) return next;
  return !floor && next - t <= Math.abs(at - t) ? next : at;
}

const plain = new Map<string, TempoMap>();
/** A map with only one time signature (the tempo is 120, as it does not change the bar lines). */
function meterMap(m: MeterShape): TempoMap {
  const key = `${m.num}/${m.den}`;
  let map = plain.get(key);
  if (!map) {
    map = new TempoMap({ tempoBpm: 120, meterNum: m.num, meterDen: m.den, headRamp: false, tempoPoints: [], meterChanges: [] });
    plain.set(key, map);
  }
  return map;
}

export const barsMap = (b: Bars): TempoMap => (b instanceof TempoMap ? b : meterMap(b));

/** The time signature at tick (the only one, if there is one). Used to count a length (beat . tick) in the beats there. */
export function meterAtOf(b: Bars, tick: number): MeterShape {
  if (!(b instanceof TempoMap)) return b;
  const m = b.meterAt(tick);
  return { num: m.num, den: m.den };
}

/** The nearest bar start (the later one at the middle). Before 0 is 0. */
export function nearestBarTick(map: TempoMap, tick: number): number {
  if (!(tick > 0)) return 0;
  const m = map.meterAt(tick);
  const next = m.barTick + perBarOf(m);
  return tick - m.barTick < next - tick ? m.barTick : next;
}

/** The number of the nearest bar line (from 1; the 5 and 9 of 「5–9」). */
export function nearestBarLine(map: TempoMap, tick: number): number {
  return map.meterAt(nearestBarTick(map, tick)).bar + 1;
}

/** The tick at a bar line number (from 1). */
export const barLineTick = (map: TempoMap, line: number) => map.tickOfBar(Math.max(0, line - 1));

/** The nearest beat start (the later one at the middle). Before 0 is 0. The same rule as snapping in core. */
export function nearestBeatTick(map: TempoMap, tick: number): number {
  return snapToStep(map, tick, 0);
}

/**
 * Snaps to a grid of step ticks. The grid counts again from each bar start (even after a short bar, the next bar start is a line).
 * A step of 0 means beats; longer than a bar means bar starts. Before 0 is 0. The later one at the middle.
 */
export function snapToStep(map: TempoMap, tick: number, step: number): number {
  if (!(tick > 0)) return 0;
  const m = map.meterAt(tick);
  const bar = perBarOf(m);
  const s = step === 0 ? perBeatOf(m) : step;
  if (s >= bar) return nearestBarTick(map, tick);
  const end = m.barTick + bar;
  const at = Math.min(end, m.barTick + Math.round((tick - m.barTick) / s) * s);
  return end - tick <= Math.abs(at - tick) ? end : at;
}

/** Gives the lines of the same grid as gridTick in ticks [from, to), in order (counting again from each time signature section start). kind is as in eachLine. */
export function eachGridTick(map: TempoMap, from: number, to: number, step: number, each: (tick: number, kind: LineKind) => void): void {
  const heads = sections.get(map) ?? [0];
  let t = gridTick(map, from, step, true);
  if (t < from) t = Math.min(t + step, nextHead(heads, t));
  const m = { num: 0, den: 0, bar: 0, barTick: 0 };
  while (t < to) {
    map.meterAt(t, m);
    const inBar = t - m.barTick;
    each(t, inBar === 0 ? 0 : inBar % perBeatOf(m) === 0 ? 1 : 2);
    t = Math.min(t + step, nextHead(heads, t));
  }
}

const nextHead = (heads: readonly number[], t: number) => heads.find((h) => h > t) ?? Infinity;

/** The position in bars (the middle of bar 4 is 3.5). For the 「+1.25」 on the ruler. */
export function barFloat(map: TempoMap, tick: number): number {
  const m = map.meterAt(Math.max(0, tick));
  return m.bar + (Math.max(0, tick) - m.barTick) / perBarOf(m);
}

/** The start tick of bar (from 1) and the time signature of that bar. */
export function barAt(map: TempoMap, bar: number): { head: number; num: number; den: number } {
  const head = map.tickOfBar(Math.max(0, bar - 1));
  const m = map.meterAt(head);
  return { head, num: m.num, den: m.den };
}

/** 「小節 . 拍 . ティック」 (each in range; the tick is within its beat) to a tick in the song. */
export function tickOfPosition(map: TempoMap, bar: number, beat: number, tick: number): number {
  const b = barAt(map, bar);
  const perBeat = perBeatOf(b);
  return b.head + (beat - 1) * perBeat + Math.min(tick, perBeat - 1);
}

/** The range text for typing a 「ティック」 (「0〜1919」, or 「0〜0959」 in 6/8). */
export const tickRangeText = (perBeat: number) => `0${RANGE_MARK}${String(perBeat - 1).padStart(4, "0")}`;

/** Line kinds: 0 bar start, 1 beat start, 2 finer lines. */
export type LineKind = 0 | 1 | 2;

/**
 * Gives the lines in ticks [from, to), in order. stepFor picks the line step from the ticks of one beat and one bar of that bar.
 * When the step is one bar or more, only the starts of bars whose number (from 0) divides evenly by the step's bar count.
 */
export function eachLine(
  map: TempoMap,
  from: number,
  to: number,
  stepFor: (perBeat: number, perBar: number) => number,
  each: (tick: number, kind: LineKind, bar: number) => void,
): void {
  const start = map.meterAt(Math.max(0, from));
  let bar = start.bar;
  let head = start.barTick;
  while (head < to) {
    const m = map.meterAt(head);
    const perBeat = perBeatOf(m);
    const perBar = perBarOf(m);
    const s = Math.max(1, stepFor(perBeat, perBar));
    if (s >= perBar) {
      const every = Math.max(1, Math.round(s / perBar));
      if (bar % every === 0 && head >= from) each(head, 0, bar);
    } else {
      for (let k = 0; k * s < perBar; k++) {
        const tick = head + k * s;
        if (tick < from) continue;
        if (tick >= to) break;
        each(tick, k === 0 ? 0 : (k * s) % perBeat === 0 ? 1 : 2, bar);
      }
    }
    head += perBar;
    bar += 1;
  }
}
