// ピアノロールの形の計算（行、方眼、ノートの場所、ドラッグの量、読み値の文）。DOM を知らない純粋な関数。
// 位置は曲の中のティック。x は「ノートの場所の左はしからの CSS px」、y は「ノートの場所の上はしからの CSS px」。

import { CONTROL_CC, CONTROL_PITCH_BEND, NO_FROM, type Grid, type IndexRange, type MusicalScale, type TakeNotes } from "../../bridge";
import type { ColorName } from "../../design/tokens";
import { isBlack } from "../../gl/keyboard";
import { barBeatTick, DISPLAY_TICKS_PER_BEAT, TICKS_PER_QUARTER, ticksPerBar, ticksPerBeat, type Meter } from "../time";
import { QUARTER_PX_STEPS } from "./view.svelte";

export const KEY_MIN = 0;
export const KEY_MAX = 127;
export const VELOCITY_MIN = 1;
export const VELOCITY_MAX = 127;
/** 書く道具で書くノートのベロシティ（仕様書にない。棟梁に聞く所）。 */
export const DRAW_VELOCITY = 100;

/** ノートの場所の見かた（CSS px）。 */
export interface RollGeom {
  scrollTick: number;
  /** 1 ティックの CSS px */
  ppt: number;
  /** いちばん上の行の鍵 */
  topKey: number;
  /** size.note: 1 行の高さ */
  rowH: number;
  width: number;
  height: number;
}

export const tickToX = (g: RollGeom, tick: number) => (tick - g.scrollTick) * g.ppt;
export const xToTick = (g: RollGeom, x: number) => g.scrollTick + x / g.ppt;
export const keyToY = (g: RollGeom, key: number) => (g.topKey - key) * g.rowH;
export const yToKey = (g: RollGeom, y: number) => g.topKey - Math.floor(y / g.rowH);
/** 見えている行の数（下のはしの半端な行もふくむ）。 */
export const rowsShown = (g: RollGeom) => Math.ceil(g.height / g.rowH);
/** 全部が見えている行の数（仕様書 3 章「縦の足し算」の行の数）。 */
export const wholeRows = (height: number, rowH: number) => Math.floor(height / rowH);

// ---- 見せているテイクのノート（曲の中の位置で読む） ----

export interface Shown {
  take: TakeNotes;
  /** テイクのティック 0 の、曲の中の位置（クリップの始まり ＋ take_offset）。 */
  base: number;
  /** いちばん長いノート（見えている所をさがすため）。 */
  maxLength: number;
}

export function shownOf(take: TakeNotes, base: number): Shown {
  let maxLength = 0;
  for (let i = 0; i < take.lengths.length; i++) maxLength = Math.max(maxLength, take.lengths[i]!);
  return { take, base, maxLength };
}

export const noteStart = (n: Shown, i: number) => n.base + n.take.starts[i]!;
export const noteEnd = (n: Shown, i: number) => n.base + n.take.starts[i]! + n.take.lengths[i]!;
export const noteCount = (n: Shown | null) => n?.take.starts.length ?? 0;

/** ティック [from, to) にかかるかもしれないノートの、最初の番号（始まりの順にならんでいるので二分探索）。 */
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

/** ティック [from, to) にかかるノートの番号を、順に each に渡す。 */
export function eachNoteIn(n: Shown, from: number, to: number, each: (i: number) => void): void {
  const s = n.take.starts;
  const l = n.take.lengths;
  for (let i = firstNoteFrom(n, from); i < s.length; i++) {
    const start = n.base + s[i]!;
    if (start >= to) break;
    if (start + l[i]! > from) each(i);
  }
}

export interface Hit {
  note: number;
  /** 右はしの 6px（長さを変える所） */
  edge: boolean;
}

/** (x, y) の下のノート。重なっていたら、あとに描く物（番号の大きい物）。 */
export function hitNote(n: Shown | null, g: RollGeom, x: number, y: number, edgePx: number): Hit | null {
  if (!n || y < 0 || y >= g.height) return null;
  const key = yToKey(g, y);
  const tick = xToTick(g, x);
  let found: Hit | null = null;
  eachNoteIn(n, tick, tick + 1e-9, (i) => {
    if (n.take.keys[i] !== key) return;
    const x0 = tickToX(g, noteStart(n, i));
    const x1 = tickToX(g, noteEnd(n, i));
    if (x < x0 || x >= x1) return;
    found = { note: i, edge: x >= x1 - Math.min(edgePx, (x1 - x0) / 2) };
  });
  return found;
}

/** 囲んだ枠（CSS px、どの向きでも）にかかるノート。 */
export function notesInBox(n: Shown | null, g: RollGeom, ax: number, ay: number, bx: number, by: number): number[] {
  if (!n) return [];
  const t0 = xToTick(g, Math.min(ax, bx));
  const t1 = xToTick(g, Math.max(ax, bx));
  const high = yToKey(g, Math.min(ay, by));
  const low = yToKey(g, Math.max(ay, by));
  const out: number[] = [];
  eachNoteIn(n, t0, t1, (i) => {
    const k = n.take.keys[i]!;
    if (k >= low && k <= high) out.push(i);
  });
  return out;
}

// ---- 選択 ----

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

// ---- グリッドと音階 ----

/** グリッド 1 つのティック。division 0 は 1 小節。三連符は 2/3。 */
export function gridTicks(grid: Grid, meter: Meter): number {
  const plain = grid.division === 0 ? ticksPerBar(meter) : (TICKS_PER_QUARTER * 4) / grid.division;
  return Math.max(1, Math.round(grid.triplet ? (plain * 2) / 3 : plain));
}

export const snapTick = (tick: number, step: number) => Math.round(tick / step) * step;
export const floorTick = (tick: number, step: number) => Math.floor(tick / step) * step;

/** グリッドの一覧（ヤチヨさんの決め 2026-11-15）。0 は 1 小節。 */
export const GRID_DIVISIONS = [0, 2, 4, 8, 16, 32] as const;

const SCALE_STEPS = { major: [0, 2, 4, 5, 7, 9, 11], minor: [0, 2, 3, 5, 7, 8, 10] } as const;
export const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;

export function inScale(key: number, s: Pick<MusicalScale, "root" | "mode">): boolean {
  return (SCALE_STEPS[s.mode] as readonly number[]).includes((((key - s.root) % 12) + 12) % 12);
}

/** スケール固定がオンなら、音階の外の行には書けない。 */
export const writable = (key: number, s: MusicalScale) => !s.lock || inScale(key, s);

/** C4 = 60（モック A-PianoRoll の鍵盤の列）。 */
export function noteName(key: number): string {
  return `${NOTE_NAMES[((key % 12) + 12) % 12]}${Math.floor(key / 12) - 1}`;
}

/** 行の地: 白鍵 gray.2、黒鍵 gray.1、書けない行（スケール固定の音階の外）gray.0。 */
export function rowColor(key: number, s: MusicalScale): ColorName {
  if (!writable(key, s)) return "color.gray.0";
  return isBlack(key) ? "color.gray.1" : "color.gray.2";
}

/** ノートの面（仕様書 2 章「ノート」）: 〜69 gray.8、70〜89 gray.9、90〜 gray.10。 */
export function velocityColor(v: number): ColorName {
  return v < 70 ? "color.gray.8" : v < 90 ? "color.gray.9" : "color.gray.10";
}

// ---- ひらいたときの見せかた ----

/**
 * ひらいたときの、いちばん上の行の鍵。いちばん高いノートの上に 4 行あけ、それでいちばん低いノートの下に 3 行が
 * 残らないなら低いほうを見せる（A-PianoRoll は 41 行で E5、F-PR150 は 18 行で D4。どちらも C3〜C5 のノート）。
 * ノートがなければ C4 をまん中に。
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
  if (!n || n.take.keys.length === 0) return null;
  let low = KEY_MAX;
  let high = KEY_MIN;
  for (const k of n.take.keys) {
    low = Math.min(low, k);
    high = Math.max(high, k);
  }
  return { low, high };
}

/**
 * ひらいたときの横の拡大: クリップがノートの場所に入る、いちばん大きい段（モックは 1280 で 1 拍 40、1920 で 80）。
 * width は拡大率をかけた CSS px、scale は拡大率。
 */
export function fitQuarterPx(clipTicks: number, width: number, scale: number): number {
  const quarters = clipTicks / TICKS_PER_QUARTER;
  let best: number = QUARTER_PX_STEPS[0];
  for (const step of QUARTER_PX_STEPS) if (quarters * step * (scale / 100) <= width) best = step;
  return best;
}

// ---- ドラッグの量 ----

/** つかんだノートの始まりがグリッドに吸いつくように動かす量。step が null（Shift）なら吸いつかない。曲の頭より前と、鍵の外へは出ない。 */
export function moveDelta(
  n: Shown,
  notes: readonly number[],
  grab: number,
  dxTicks: number,
  dKeys: number,
  step: number | null,
): { ticks: number; keys: number } {
  const start = noteStart(n, grab);
  const target = start + dxTicks;
  let ticks = (step ? snapTick(target, step) : Math.round(target)) - start;
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

/** つかんだノートの終わりがグリッドに吸いつくように、長さを変える量。どのノートも 1 ティックより短くならない（吸いつくときは、つかんだノートはグリッド 1 つより短くならない）。 */
export function resizeDelta(n: Shown, notes: readonly number[], grab: number, dxTicks: number, step: number | null): number {
  const end = noteEnd(n, grab);
  const target = end + dxTicks;
  let ticks = (step ? snapTick(target, step) : Math.round(target)) - end;
  const grabLength = n.take.lengths[grab]!;
  if (step) ticks = Math.max(ticks, Math.min(step, grabLength) - grabLength);
  let shortest = Infinity;
  for (const i of notes) shortest = Math.min(shortest, n.take.lengths[i]!);
  ticks = Math.max(ticks, 1 - shortest);
  return ticks === 0 ? 0 : ticks;
}

/** 書く道具で押した所のノートの始まり。吸いつくときは、押したグリッドの頭。 */
export function drawStart(tick: number, step: number | null): number {
  return Math.max(0, step ? floorTick(tick, step) : Math.round(tick));
}

/** 書く道具で押したまま右へ動かしたときの長さ。いちばん短くてグリッド 1 つ（吸いつかないときは 1 ティック）。 */
export function drawLength(start: number, tick: number, step: number, snap: boolean): number {
  return snap ? Math.max(step, snapTick(tick, step) - start) : Math.max(1, Math.round(tick) - start);
}

/** ベロシティの帯で、上へ動かした量（CSS px）を値の差に。full は 127 の棒の高さ（CSS px）。 */
export function velocityDelta(dyUp: number, full: number): number {
  const d = Math.round((dyUp * VELOCITY_MAX) / full);
  return d === 0 ? 0 : d;
}

export const clampVelocity = (v: number) => Math.min(VELOCITY_MAX, Math.max(VELOCITY_MIN, v));

/** Mod D: 選んだノートの範囲の長さを、グリッドに切り上げた分。 */
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

/** 鳴っている音: tick にかかっているノートの鍵を、4 語のビットに入れる。変わったら true。 */
export function soundingKeys(n: Shown | null, tick: number, out: Uint32Array): boolean {
  const before = [out[0], out[1], out[2], out[3]];
  out.fill(0);
  if (n) {
    eachNoteIn(n, tick, tick + 1e-9, (i) => {
      const k = n.take.keys[i]!;
      out[k >>> 5]! |= 1 << (k & 31);
    });
  }
  return out[0] !== before[0] || out[1] !== before[1] || out[2] !== before[2] || out[3] !== before[3];
}

/** クオンタイズしたテイクで、もとのノートと位置か長さか鍵がちがうノートの、もとの番号（ちがわなければ -1）。 */
export function movedFrom(take: TakeNotes, origin: TakeNotes, i: number): number {
  const j = take.froms[i]!;
  if (j === NO_FROM || j >= origin.starts.length) return -1;
  const same = origin.starts[j] === take.starts[i] && origin.lengths[j] === take.lengths[i] && origin.keys[j] === take.keys[i];
  return same ? -1 : j;
}

// ---- 文 ----

/** 「3.1.000」（小節 . 拍 . ティック）。 */
export function positionText(tick: number, meter: Meter): string {
  const b = barBeatTick(tick, meter);
  return `${b.bar}.${b.beat}.${String(b.tick).padStart(3, "0")}`;
}

/** 「1.240」（拍 . ティック）。 */
export function lengthText(ticks: number, meter: Meter): string {
  const perBeat = ticksPerBeat(meter);
  const t = Math.max(0, Math.round(Math.abs(ticks)));
  const beats = Math.floor(t / perBeat);
  const rest = Math.floor(((t % perBeat) * DISPLAY_TICKS_PER_BEAT) / perBeat);
  return `${beats}.${String(rest).padStart(3, "0")}`;
}

/** 「+2」「−2」。0 は「+0」。 */
export const signed = (v: number) => `${v < 0 ? "−" : "+"}${Math.abs(v)}`;
/** 「+0.240」「−1.000」。 */
export const signedLength = (ticks: number, meter: Meter) => `${ticks < 0 ? "−" : "+"}${lengthText(ticks, meter)}`;

/** 「1–4」: クリップの小節（終わりの小節をふくむ）。 */
export function barsText(start: number, end: number, meter: Meter): string {
  const perBar = ticksPerBar(meter);
  const first = Math.floor(start / perBar) + 1;
  const last = Math.max(first, Math.ceil(end / perBar));
  return `${first}–${last}`;
}

/** 「1/16」。0 は null（「1 小節」の文を使う）。 */
export const gridFraction = (division: number) => (division === 0 ? null : `1/${division}`);

// ---- CC の札の数 ----

export interface CcCounts {
  /** サステインを踏んだ回数（CC 64 が 64 より下から 64 以上になった回数） */
  sustain: number;
  pitchBend: number;
  modulation: number;
  /** 1 と 64 のほかの CC の番号 */
  others: number[];
  /** コントロールの点の数 */
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
