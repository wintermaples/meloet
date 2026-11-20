// 小節と拍の線を、テンポの地図（tempoMap.ts）から読む。拍子で小節の長さが変わっても、目盛 ・ グリッド ・ 吸いつきが同じ線を使う。
// DOM も Tauri も知らない。60 Hz では使わない（スクロールや曲が変わったときだけ）。

import { TempoMap, type TimelineShape } from "./tempoMap";
import { TICKS_PER_QUARTER } from "./ticks";

const WHOLE = 4 * TICKS_PER_QUARTER;

/** 拍子だけ（曲の頭の拍子、テンポの変更なし）。 */
export interface MeterShape {
  num: number;
  den: number;
}

/** 小節の長さを知る物: テンポの地図か、拍子が 1 つだけの曲の拍子。 */
export type Bars = TempoMap | MeterShape;

export const perBeatOf = (m: MeterShape) => WHOLE / m.den;
export const perBarOf = (m: MeterShape) => (m.num * WHOLE) / m.den;

const maps = new WeakMap<object, TempoMap>();
/** 地図ごとの、拍子の区間の頭のティック（曲の頭の 0 から、順に）。 */
const sections = new WeakMap<TempoMap, number[]>();
/** `timeline` の区画の地図。同じ区画（差分で入れかわるまで同じ物）には同じ地図を返す。 */
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
 * ノートのグリッド（core のクオンタイズと同じ考え）: 拍子の区間の頭から step ずつ数え、次の拍子の旗をこえる段は、その旗の小節の頭。
 * floor なら押した所のグリッドの頭（書く道具）、そうでなければいちばん近い段（まん中なら後ろ）。0 より前は 0。
 * bar（グリッドが 1 小節）なら、その所の小節の頭。
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
/** 拍子が 1 つだけの地図（テンポは小節の線に関わらないので 120）。 */
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

/** tick の所の拍子（拍子 1 つならそれ）。長さ（拍 . ティック）を、その所の拍で数えるため。 */
export function meterAtOf(b: Bars, tick: number): MeterShape {
  if (!(b instanceof TempoMap)) return b;
  const m = b.meterAt(tick);
  return { num: m.num, den: m.den };
}

/** いちばん近い小節の頭（まん中なら後ろ）。0 より前は 0。 */
export function nearestBarTick(map: TempoMap, tick: number): number {
  if (!(tick > 0)) return 0;
  const m = map.meterAt(tick);
  const next = m.barTick + perBarOf(m);
  return tick - m.barTick < next - tick ? m.barTick : next;
}

/** いちばん近い小節の線の番号（1 から。「5–9」の 5 と 9）。 */
export function nearestBarLine(map: TempoMap, tick: number): number {
  return map.meterAt(nearestBarTick(map, tick)).bar + 1;
}

/** 小節の線の番号（1 から）の所のティック。 */
export const barLineTick = (map: TempoMap, line: number) => map.tickOfBar(Math.max(0, line - 1));

/** いちばん近い拍の頭（まん中なら後ろ）。0 より前は 0。core の吸いつきと同じ決まり。 */
export function nearestBeatTick(map: TempoMap, tick: number): number {
  return snapToStep(map, tick, 0);
}

/**
 * step ティックのグリッドに吸いつく。グリッドは小節の頭から数えなおす（半端な小節でも、次の小節の頭が線になる）。
 * step が 0 なら拍、小節より長ければ小節の頭。0 より前は 0。まん中なら後ろ。
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

/** gridTick と同じグリッドの線を、ティック [from, to) で順に渡す（拍子の区間の頭から数えなおす）。kind は eachLine と同じ。 */
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

/** 小節を 1 とした位置（4 小節目のまん中は 3.5）。目盛の「+1.25」のため。 */
export function barFloat(map: TempoMap, tick: number): number {
  const m = map.meterAt(Math.max(0, tick));
  return m.bar + (Math.max(0, tick) - m.barTick) / perBarOf(m);
}

/** 小節 bar（1 から）の頭のティックと、その小節の拍子。 */
export function barAt(map: TempoMap, bar: number): { head: number; num: number; den: number } {
  const head = map.tickOfBar(Math.max(0, bar - 1));
  const m = map.meterAt(head);
  return { head, num: m.num, den: m.den };
}

/** 「小節 . 拍 . ティック」（どれも範囲の中。ティックはその拍の中のティック）を曲の中のティックに。 */
export function tickOfPosition(map: TempoMap, bar: number, beat: number, tick: number): number {
  const b = barAt(map, bar);
  const perBeat = perBeatOf(b);
  return b.head + (beat - 1) * perBeat + Math.min(tick, perBeat - 1);
}

/** 打ちこむ「ティック」の範囲の文（「0〜1919」、6/8 は「0〜0959」）。 */
export const tickRangeText = (perBeat: number) => `0〜${String(perBeat - 1).padStart(4, "0")}`;

/** 線の種類: 0 小節の頭、1 拍の頭、2 それより細かい線。 */
export type LineKind = 0 | 1 | 2;

/**
 * ティック [from, to) の線を順に渡す。stepFor は、その小節の 1 拍と 1 小節のティックから、線のきざみを決める。
 * きざみが 1 小節以上なら、小節の番号（0 から）がきざみの小節の数で割りきれる小節の頭だけ。
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
