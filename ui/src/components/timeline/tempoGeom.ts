// The shape of the arrange's 「テンポ ・ 拍子の変更」 band (spec 3 (Y17, 18-C), mock F6-Tempo, Opt6-Tempo). Does not know the DOM.
// Coordinates of the lower (tempo) row are CSS px from the row's top left. x is the same as arrangeGeom: "px from the song start − horizontal scroll".

import type { TimelineRef, TimelineView } from "../../bridge";
import type { ColorName } from "../../design/tokens";
import type { GlRect } from "../../gl/slot";
import { t } from "../../text";
import { nearestBarLine, nearestBeatTick } from "../../time/bars";
import type { TempoMap } from "../../time/tempoMap";
import { tickToX, xToTick, type View } from "../arrangeGeom";
import { tempoText } from "../format";
import { TEMPO_MAX, TEMPO_MIN } from "../time";

/** A tempo point, including the start of the song (tick 0). */
export interface TempoPt {
  tick: number;
  bpm: number;
  ramp: boolean;
}

/** The point at the start of the song and the change points (in tick order). */
export function tempoPoints(t: Pick<TimelineView, "tempoBpm" | "headRamp" | "tempoPoints">): TempoPt[] {
  return [{ tick: 0, bpm: t.tempoBpm, ramp: t.headRamp }, ...t.tempoPoints.map((p) => ({ tick: p.tick, bpm: p.tempoBpm, ramp: p.ramp }))];
}

/** Time signature flags (including bar 0 at the start of the song). */
export interface Flag {
  bar: number;
  tick: number;
  num: number;
  den: number;
}

export function flags(t: Pick<TimelineView, "meterNum" | "meterDen" | "meterChanges">, map: TempoMap): Flag[] {
  return [
    { bar: 0, tick: 0, num: t.meterNum, den: t.meterDen },
    ...t.meterChanges.map((c) => ({ bar: c.bar, tick: map.tickOfBar(c.bar), num: c.meterNum, den: c.meterDen })),
  ];
}

/** The number of changes: the number of places (ticks) where something changes. A tempo point and a time signature flag at the same place count as 1 (docs/history/design-notes.ja.md B-92). */
export function changeCount(t: Pick<TimelineView, "tempoPoints" | "meterChanges">, map: TempoMap): number {
  const at = new Set<number>(t.tempoPoints.map((p) => p.tick));
  for (const c of t.meterChanges) at.add(map.tickOfBar(c.bar));
  at.delete(0);
  return at.size;
}

/** Tempo to height: the highest tempo is pad from the top, the lowest is pad from the bottom. If all are the same, the middle. */
export interface TempoScale {
  hi: number;
  lo: number;
  /** px per 1.00 (upward). fallback if all tempos are the same. */
  pxPerBpm: number;
  height: number;
  pad: number;
}

export function tempoScale(points: readonly TempoPt[], height: number, pad: number, fallback: number): TempoScale {
  let hi = -Infinity;
  let lo = Infinity;
  for (const p of points) {
    hi = Math.max(hi, p.bpm);
    lo = Math.min(lo, p.bpm);
  }
  const span = hi - lo;
  return { hi, lo, pxPerBpm: span > 0 ? (height - 2 * pad) / span : fallback, height, pad };
}

export function bpmToY(s: TempoScale, bpm: number): number {
  return s.hi > s.lo ? s.pad + (s.hi - bpm) * s.pxPerBpm : s.height / 2 - (bpm - s.hi) * s.pxPerBpm;
}

/** Rounds to 0.01 and clamps to 30.00 to 300.00. */
export function tidyBpm(bpm: number): number {
  return Math.min(TEMPO_MAX, Math.max(TEMPO_MIN, Math.round(bpm * 100) / 100));
}

/** The tempo on the line at that place (the value for a new point). */
export const bpmAt = (map: TempoMap, tick: number) => tidyBpm(map.tempoAt(tick));

export interface BandDims {
  /** line.hair */
  hair: number;
  /** The size of a point (6×6) */
  point: number;
}

/** A point mark. */
export interface PointLook {
  selected: boolean;
  hovered: boolean;
}

const LINE: ColorName = "color.gray.9";

/** A slanted line as a row of hair rectangles, joining spans of the same height (nothing is drawn outside the visible width). */
function slope(out: GlRect[], xa: number, ya: number, xb: number, yb: number, d: BandDims, width: number): void {
  const h = d.hair;
  const from = Math.max(xa, -h);
  const to = Math.min(xb, width + h);
  if (to <= from) return;
  const at = (x: number) => Math.round((ya + ((yb - ya) * (x - xa)) / (xb - xa)) / h) * h;
  let runX = from;
  let runY = at(from);
  for (let x = from + h; x < to; x += h) {
    const y = at(x);
    if (y === runY) continue;
    out.push({ x: runX, y: runY, w: x - runX, h, color: LINE });
    out.push({ x, y: Math.min(runY, y), w: h, h: Math.abs(y - runY) + h, color: LINE });
    runX = x;
    runY = y;
  }
  out.push({ x: runX, y: runY, w: to - runX + h, h, color: LINE });
}

/** A horizontal line (only inside the visible width). */
function flat(out: GlRect[], x0: number, x1: number, y: number, d: BandDims, width: number): void {
  const a = Math.max(x0, -d.hair);
  const b = Math.min(x1, width + d.hair);
  if (b > a) out.push({ x: a, y, w: b - a + d.hair, h: d.hair, color: LINE });
}

const clampY = (y: number, s: TempoScale, d: BandDims) => Math.min(Math.max(y, 0), s.height - d.hair);

/**
 * The tempo staircase line, the slanted ramp lines, and points (6×6; normally gray.10, a selected point accent, a hovered point with a 1px accent border).
 * from: while dragging, the original place of the point (accent border).
 */
export function tempoRects(
  v: View,
  points: readonly TempoPt[],
  s: TempoScale,
  d: BandDims,
  look: (p: TempoPt, i: number) => PointLook,
  from: { tick: number; bpm: number } | null,
): GlRect[] {
  const out: GlRect[] = [];
  const ys = points.map((p) => clampY(Math.round(bpmToY(s, p.bpm) / d.hair) * d.hair, s, d));
  for (let i = 0; i < points.length; i++) {
    const p = points[i]!;
    const next = points[i + 1];
    const x = tickToX(v, p.tick);
    const y = ys[i]!;
    if (!next) {
      flat(out, x, v.width, y, d, v.width);
      break;
    }
    const nx = tickToX(v, next.tick);
    const ny = ys[i + 1]!;
    if (p.ramp && ny !== y) {
      slope(out, x, y, nx, ny, d, v.width);
    } else {
      flat(out, x, nx, y, d, v.width);
      if (nx >= -d.hair && nx <= v.width + d.hair) out.push({ x: nx, y: Math.min(y, ny), w: d.hair, h: Math.abs(ny - y) + d.hair, color: LINE });
    }
  }
  const half = d.point / 2;
  const square = (x: number, y: number, color: ColorName) => out.push({ x: x - half, y: y - half, w: d.point, h: d.point, color });
  const frame = (x: number, y: number) => {
    const x0 = x - half - d.hair;
    const y0 = y - half - d.hair;
    const w = d.point + 2 * d.hair;
    out.push(
      { x: x0, y: y0, w, h: d.hair, color: "color.accent" },
      { x: x0, y: y0 + w - d.hair, w, h: d.hair, color: "color.accent" },
      { x: x0, y: y0, w: d.hair, h: w, color: "color.accent" },
      { x: x0 + w - d.hair, y: y0, w: d.hair, h: w, color: "color.accent" },
    );
  };
  if (from) {
    const x = tickToX(v, from.tick);
    if (x >= -d.point && x <= v.width + d.point) frame(x, clampY(bpmToY(s, from.bpm), s, d));
  }
  points.forEach((p, i) => {
    const x = tickToX(v, p.tick);
    if (x < -d.point || x > v.width + d.point) return;
    const l = look(p, i);
    square(x, ys[i]!, l.selected ? "color.accent" : "color.gray.10");
    if (l.hovered && !l.selected) frame(x, ys[i]!);
  });
  return out;
}

/** Where to put a point's value text (below right of the point) and the ramp text (above the middle of the slanted line). */
export interface TempoLabel {
  tick: number;
  x: number;
  y: number;
  bpm: number;
}

export function tempoLabels(v: View, points: readonly TempoPt[], s: TempoScale, gap: number, d: BandDims): TempoLabel[] {
  return points
    .map((p) => ({ tick: p.tick, x: tickToX(v, p.tick) + gap, y: clampY(bpmToY(s, p.bpm), s, d), bpm: p.bpm }))
    .filter((l) => l.x < v.width && l.x > -v.width);
}

/** Ramp spans (those touching the visible part). x is the middle of the line. */
export function rampSpans(v: View, points: readonly TempoPt[]): { tick: number; x0: number; x1: number }[] {
  const out: { tick: number; x0: number; x1: number }[] = [];
  for (let i = 0; i + 1 < points.length; i++) {
    const p = points[i]!;
    const q = points[i + 1]!;
    if (!p.ramp || p.bpm === q.bpm) continue;
    const x0 = tickToX(v, p.tick);
    const x1 = tickToX(v, q.tick);
    if (x1 < 0 || x0 > v.width) continue;
    out.push({ tick: p.tick, x0, x1 });
  }
  return out;
}

/** The point at (x, y) (up to grab px around the point's square). The nearest point's index, or -1. */
export function pointAt(v: View, points: readonly TempoPt[], s: TempoScale, x: number, y: number, grab: number): number {
  let best = -1;
  let bestD = Infinity;
  points.forEach((p, i) => {
    const dx = Math.abs(tickToX(v, p.tick) - x);
    const dy = Math.abs(bpmToY(s, p.bpm) - y);
    if (dx <= grab && dy <= grab && dx + dy < bestD) {
      best = i;
      bestD = dx + dy;
    }
  });
  return best;
}

/** The flag at x (from its left edge line up to w px to the right, where the flag text is). -1 if none. */
export function flagAt(v: View, fs: readonly Flag[], x: number, widths: readonly number[]): number {
  for (let i = fs.length - 1; i >= 0; i--) {
    const fx = tickToX(v, fs[i]!.tick);
    if (x >= fx && x < fx + (widths[i] ?? 0)) return i;
  }
  return -1;
}

// ---- Dragging (only the screen moves first; one intent on release) ----

export interface PointDrag {
  /** The tick of the original point (the point's name). */
  tick: number;
  fromBpm: number;
  toTick: number;
  bpm: number;
  downX: number;
  downY: number;
  moved: boolean;
}

/** Sideways it snaps to the start of beats (ticks with Shift); up and down is tempo. The point at the start of the song does not move sideways. */
export function dragPoint(d: PointDrag, v: View, s: TempoScale, map: TempoMap, x: number, y: number, still: number, shift: boolean): PointDrag {
  const moved = d.moved || Math.hypot(x - d.downX, y - d.downY) >= still;
  if (!moved) return d;
  const at = d.tick + (x - d.downX) / v.pxPerTick;
  const toTick = d.tick === 0 ? 0 : Math.max(1, shift ? Math.round(at) : nearestBeatTick(map, at));
  const bpm = tidyBpm(d.fromBpm - (y - d.downY) / s.pxPerBpm);
  return { ...d, moved, toTick, bpm };
}

/** The points while dragging (if the moved point lands on the same place as another point, it takes that point's place). */
export function draggedPoints(points: readonly TempoPt[], d: PointDrag | null): TempoPt[] {
  if (!d || !d.moved) return [...points];
  const moving = points.find((p) => p.tick === d.tick);
  if (!moving) return [...points];
  return [...points.filter((p) => p.tick !== d.tick && p.tick !== d.toTick), { ...moving, tick: d.toTick, bpm: d.bpm }].sort((a, b) => a.tick - b.tick);
}

export interface FlagDrag {
  bar: number;
  toBar: number;
  downX: number;
  moved: boolean;
}

/** A flag snaps to the start of bars. The flag at the start of the song does not move. */
export function dragFlag(d: FlagDrag, v: View, map: TempoMap, x: number, still: number): FlagDrag {
  const moved = d.moved || Math.abs(x - d.downX) >= still;
  if (!moved || d.bar === 0) return moved === d.moved ? d : { ...d, moved };
  const at = map.tickOfBar(d.bar) + (x - d.downX) / v.pxPerTick;
  return { ...d, moved, toBar: Math.max(1, nearestBarLine(map, at) - 1) };
}

/** What a double press adds: in the lower row, a tempo point at the start of that beat; in the upper row, a time signature flag at the start of that bar. */
export function addAt(v: View, map: TempoMap, x: number, row: "tempo" | "meter"): { tick: number; bpm: number } | { bar: number; num: number; den: number } {
  const at = Math.max(0, xToTick(v, x));
  if (row === "tempo") {
    const tick = nearestBeatTick(map, at);
    return { tick, bpm: bpmAt(map, tick) };
  }
  const m = map.meterAt(at);
  return { bar: m.bar, num: m.num, den: m.den };
}

export const sameRef = (a: TimelineRef | null, b: TimelineRef | null): boolean =>
  a !== null &&
  b !== null &&
  a.kind === b.kind &&
  (a.kind === "tempoPoint" ? a.tick === (b as { tick: number }).tick : a.kind === "meterChange" ? a.bar === (b as { bar: number }).bar : a.id === (b as { id: number }).id);

/** The two lines of the open band's heading (「テンポ 96.00 ほか 3」「拍子 4/4 ほか 2」. Without changes, no 「ほか」). */
export function summaries(tl: Pick<TimelineView, "tempoBpm" | "meterNum" | "meterDen" | "tempoPoints" | "meterChanges">): { tempo: string; meter: string } {
  const tempo = tempoText(tl.tempoBpm);
  const signature = `${tl.meterNum}/${tl.meterDen}`;
  return {
    tempo: tl.tempoPoints.length > 0 ? t("timeline.tempoSummary", { tempo, count: tl.tempoPoints.length }) : t("timeline.tempoOnly", { tempo }),
    meter: tl.meterChanges.length > 0 ? t("timeline.meterSummary", { signature, count: tl.meterChanges.length }) : t("timeline.meterOnly", { signature }),
  };
}
