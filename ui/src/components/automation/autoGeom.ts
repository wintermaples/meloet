// The shape of automation lanes in the arrange (spec 2 "Automation point", spec 3 (Y16), mock R10-AutoEdit, F6-AutoRec). Does not know the DOM.
// Just below a track's row come the open lanes (size.lane each) and the 「＋ 帯を足す ▾」 row (size.row).
// Coordinates inside a lane are CSS px from the lane's top left. x is the same as arrangeGeom: "px from the song start − horizontal scroll".

import type { LanePoints, LaneRef, LaneView, TrackView } from "../../bridge";
import type { ColorName } from "../../design/tokens";
import type { GlRect } from "../../gl/slot";
import { nearestBeatTick } from "../../time/bars";
import type { TempoMap } from "../../time/tempoMap";
import { tickToX, xToTick, type View } from "../arrangeGeom";
import { tickText } from "../time";
import { hasLaneRef, type LaneOwner } from "./laneValue";

// ---- Track rows and the order of lanes ----

export interface RowDims {
  /** size.track */
  trackH: number;
  /** size.lane: one open lane */
  laneH: number;
  /** size.row: the 「＋ 帯を足す ▾」 row */
  rowH: number;
  /** The row whose vocal is being edited (ArrangeView.tallTrack), and its height (part.track.vocal). */
  tall?: { track: number | null; h: number };
}

export interface OpenLane {
  /** The track, or the master standing in as one (`masterOwner`). */
  track: LaneOwner;
  lane: LaneView;
  top: number;
}

export interface AddRow {
  track: TrackView;
  top: number;
}

export interface ArrangeRows {
  /** The top of each track row (in track order). */
  tops: number[];
  /** The bottom of each track block (the row, and the lanes and row below it). */
  bottoms: number[];
  /** The height of each track row (only the row whose vocal is being edited is taller). */
  heights: number[];
  lanes: OpenLane[];
  rows: AddRow[];
  /** The row 「マスター」 at the foot, while the master has a lane (round 12 M5): its lane open (size.lane), or folded (size.row). */
  master: { top: number; height: number; folded: boolean } | null;
  /** The height of the tracks' rows, without the master's. */
  tracksHeight: number;
  /** The top of the band 「いいとこ取り」, when one is open (round 12, A8). */
  bandTop: number | null;
  height: number;
}

/**
 * Tracks that show the 「＋ 帯を足す ▾」 row: only tracks that have lanes (mock F7-Answers92 and F6-AutoRec 2 to 5 have no row).
 * Where to add a gain or pan lane to a track with no lanes yet is in neither the spec nor the mock (to be sent back in docs/history/design-notes.ja.md B).
 */
export const showsAddRow = (track: TrackView) => track.lanes.length > 0;

/**
 * Lanes in the order they were added. Folded lanes are not shown (they are opened again from the 「＋ 帯を足す ▾」 list).
 * `band`: the band 「いいとこ取り」 that is open, right under its track's row and above the track's lanes.
 */
export function arrangeRows(
  tracks: readonly TrackView[],
  folded: readonly LaneRef[],
  d: RowDims,
  master: LaneOwner | null = null,
  band: { track: number; height: number } | null = null,
): ArrangeRows {
  const out: ArrangeRows = { tops: [], bottoms: [], heights: [], lanes: [], rows: [], master: null, tracksHeight: 0, bandTop: null, height: 0 };
  let y = 0;
  for (const track of tracks) {
    const h = d.tall && d.tall.track === track.id ? d.tall.h : d.trackH;
    out.tops.push(y);
    out.heights.push(h);
    y += h;
    if (band && band.track === track.id) {
      out.bandTop = y;
      y += band.height;
    }
    for (const lane of track.lanes) {
      if (hasLaneRef(folded, track.id, lane.target)) continue;
      out.lanes.push({ track, lane, top: y });
      y += d.laneH;
    }
    if (showsAddRow(track)) {
      out.rows.push({ track, top: y });
      y += d.rowH;
    }
    out.bottoms.push(y);
  }
  out.tracksHeight = y;
  const lane = master?.lanes[0];
  if (master && lane) {
    const shut = hasLaneRef(folded, master.id, lane.target);
    out.master = { top: y, height: shut ? d.rowH : d.laneH, folded: shut };
    if (!shut) out.lanes.push({ track: master, lane, top: y });
    y += out.master.height;
  }
  out.height = y;
  return out;
}

// ---- Inside a lane: values and height ----

export interface LaneDims {
  /** The lane height (size.lane) */
  h: number;
  /** The space above and below (so points at 0 and 1 stay inside the lane) */
  pad: number;
  /** line.hair */
  hair: number;
  /** The point size (part.cc.point, 6×6) */
  point: number;
  /** The range of positions shown from the bottom to the top of the lane (laneRange). 0 to 1 if not given. */
  range?: LaneRange;
}

/** The range of positions (0 to 1) the lane shows vertically. lo is the bottom edge, hi the top edge. */
export interface LaneRange {
  lo: number;
  hi: number;
}

export const FULL_RANGE: LaneRange = { lo: 0, hi: 1 };

/**
 * The lane's vertical range (spec 3 (92-6, 21-B)): from the lowest point to the highest, with 10 % room above and below. If there is one point, or all have the same value,
 * 10 % of the parameter's width around that value. On/off and mute (stepped lanes) are always bottom to top. Also bottom to top when there are no points.
 * What goes outside 0 to 1 is shifted to the other side.
 */
export function laneRange(pts: readonly Pt[], step: boolean): LaneRange {
  if (step || pts.length === 0) return FULL_RANGE;
  let min = Infinity;
  let max = -Infinity;
  for (const p of pts) {
    min = Math.min(min, p.value);
    max = Math.max(max, p.value);
  }
  const room = max > min ? (max - min) * 0.1 : 0.05;
  const width = Math.min(1, max - min + 2 * room);
  const lo = Math.min(1 - width, Math.max(0, min - room));
  return { lo, hi: lo + width };
}

/** The positions for the top and bottom range text (「上 −3.0 dB」「下 −9.0 dB」, mock Opt7-LaneScale): the highest and lowest points. If there is one point or all are the same, the edges of the range. */
export function rangeEnds(pts: readonly Pt[], range: LaneRange): LaneRange {
  const values = pts.map((p) => p.value);
  const lo = Math.min(...values);
  const hi = Math.max(...values);
  return hi > lo ? { lo, hi } : range;
}

const span = (d: LaneDims) => Math.max(1, d.h - 2 * d.pad - d.hair);
const widthOf = (d: LaneDims) => {
  const r = d.range ?? FULL_RANGE;
  return Math.max(1e-9, r.hi - r.lo);
};
const clamp01 = (v: number) => Math.min(1, Math.max(0, v));
/** Position (0 to 1) → y. The top of the range is the top of the lane. Outside the range, it stops at the lane's edge. */
export const valueToY = (d: LaneDims, value: number) => {
  const t = clamp01((clamp01(value) - (d.range?.lo ?? 0)) / widthOf(d));
  return Math.round((d.pad + (1 - t) * span(d)) / d.hair) * d.hair;
};
/** y → position (clamped to 0 to 1). */
export const yToValue = (d: LaneDims, y: number) => clamp01((d.range?.lo ?? 0) + (1 - (y - d.pad) / span(d)) * widthOf(d));
/** The change in position when moved dy px up or down. */
export const dyToValue = (d: LaneDims, dy: number) => (-dy / span(d)) * widthOf(d);

export interface Pt {
  tick: number;
  value: number;
}

export function pointList(p: Pick<LanePoints, "ticks" | "values">): Pt[] {
  const out: Pt[] = [];
  for (let i = 0; i < p.ticks.length; i++) out.push({ tick: p.ticks[i]!, value: p.values[i]! });
  return out;
}

// ---- Dragging (only the screen moves first; one intent on release) ----

export interface PointDrag {
  /** The points to move (indexes in the lane, in tick order). */
  points: number[];
  /** The point that was grabbed. */
  grab: number;
  downX: number;
  downY: number;
  moved: boolean;
  dTick: number;
  dValue: number;
}

/** Sideways it snaps to the start of beats (ticks with Shift); up and down is position. The first point stops at the start of the song. */
export function dragPoints(drag: PointDrag, pts: readonly Pt[], v: View, d: LaneDims, map: TempoMap, x: number, y: number, still: number, shift: boolean): PointDrag {
  const moved = drag.moved || Math.hypot(x - drag.downX, y - drag.downY) >= still;
  if (!moved) return drag;
  const grab = pts[drag.grab]!;
  const at = grab.tick + (x - drag.downX) / v.pxPerTick;
  const first = Math.min(...drag.points.map((i) => pts[i]!.tick));
  const to = Math.max(0, shift ? Math.round(at) : nearestBeatTick(map, at));
  const dTick = Math.max(to - grab.tick, -first) || 0;
  const dValue = dyToValue(d, y - drag.downY);
  if (drag.moved && dTick === drag.dTick && dValue === drag.dValue) return drag;
  return { ...drag, moved, dTick, dValue };
}

/** The points while dragging. If a moved point lands on the same tick as another point, it takes that point's place. moved marks the points being moved. */
export function draggedPoints(pts: readonly Pt[], drag: PointDrag | null): { pts: Pt[]; moving: boolean[] } {
  if (!drag?.moved) return { pts: [...pts], moving: pts.map((_, i) => drag?.points.includes(i) ?? false) };
  const chosen = new Set(drag.points);
  const moved = drag.points.map((i) => ({ tick: pts[i]!.tick + drag.dTick, value: Math.min(1, Math.max(0, pts[i]!.value + drag.dValue)) }));
  const taken = new Set(moved.map((p) => p.tick));
  const kept = pts.filter((p, i) => !chosen.has(i) && !taken.has(p.tick));
  const all = [...kept.map((p) => ({ p, moving: false })), ...moved.map((p) => ({ p, moving: true }))].sort((a, b) => a.p.tick - b.p.tick);
  return { pts: all.map((a) => a.p), moving: all.map((a) => a.moving) };
}

// ---- Drawing (on one canvas of arrangeLanes, as rectangles like the other slots) ----

export interface LaneLook {
  /** Resting, or resting for now: the line is gray.7. */
  resting: boolean;
  /** 「● 書く」 is on: points are only an accent border (before writing). */
  armed: boolean;
  /** The selected points (indexes in the lane; while dragging, the points being moved). */
  selected: (i: number) => boolean;
  hovered: number;
  /** While dragging, the original place of the points (accent border). */
  from: readonly Pt[];
  /** The flat line when there are no points (the value set by hand). Null if not known. */
  flat: number | null;
}

function lineRun(out: GlRect[], xa: number, ya: number, xb: number, yb: number, d: LaneDims, width: number, color: ColorName): void {
  const h = d.hair;
  const from = Math.max(xa, -h);
  const to = Math.min(xb, width + h);
  if (to <= from) return;
  if (ya === yb) {
    out.push({ x: from, y: ya, w: to - from + h, h, color });
    return;
  }
  // A slanted line as a row of hair rectangles, joining spans of the same height.
  const at = (x: number) => Math.round((ya + ((yb - ya) * (x - xa)) / (xb - xa)) / h) * h;
  let runX = from;
  let runY = at(from);
  for (let x = from + h; x < to; x += h) {
    const y = at(x);
    if (y === runY) continue;
    out.push({ x: runX, y: runY, w: x - runX, h, color });
    out.push({ x, y: Math.min(runY, y), w: h, h: Math.abs(y - runY) + h, color });
    runX = x;
    runY = y;
  }
  out.push({ x: runX, y: runY, w: to - runX + h, h, color });
}

/** The line (1px gray.9, gray.7 when resting; a staircase for stepped lanes) and points (6×6; normally gray.10, selected points accent, a hovered point with a 1px accent border). */
export function laneRects(v: View, pts: readonly Pt[], step: boolean, d: LaneDims, look: LaneLook): GlRect[] {
  const out: GlRect[] = [];
  const color: ColorName = look.resting ? "color.gray.7" : "color.gray.9";
  if (pts.length === 0) {
    if (look.flat !== null) lineRun(out, -d.hair, valueToY(d, look.flat), v.width, valueToY(d, look.flat), d, v.width, color);
    return out;
  }
  const xs = pts.map((p) => Math.round(tickToX(v, p.tick) / d.hair) * d.hair);
  const ys = pts.map((p) => valueToY(d, p.value));
  lineRun(out, -d.hair, ys[0]!, xs[0]!, ys[0]!, d, v.width, color);
  for (let i = 0; i + 1 < pts.length; i++) {
    const x = xs[i]!;
    const nx = xs[i + 1]!;
    if (nx < -d.hair || x > v.width + d.hair) continue;
    if (step) {
      lineRun(out, x, ys[i]!, nx, ys[i]!, d, v.width, color);
      if (nx >= -d.hair && nx <= v.width + d.hair) out.push({ x: nx, y: Math.min(ys[i]!, ys[i + 1]!), w: d.hair, h: Math.abs(ys[i + 1]! - ys[i]!) + d.hair, color });
    } else {
      lineRun(out, x, ys[i]!, nx, ys[i + 1]!, d, v.width, color);
    }
  }
  const last = pts.length - 1;
  lineRun(out, xs[last]!, ys[last]!, v.width, ys[last]!, d, v.width, color);

  const half = d.point / 2;
  const frame = (x: number, y: number, size: number) => {
    const x0 = x - size / 2;
    const y0 = y - size / 2;
    out.push(
      { x: x0, y: y0, w: size, h: d.hair, color: "color.accent" },
      { x: x0, y: y0 + size - d.hair, w: size, h: d.hair, color: "color.accent" },
      { x: x0, y: y0, w: d.hair, h: size, color: "color.accent" },
      { x: x0 + size - d.hair, y: y0, w: d.hair, h: size, color: "color.accent" },
    );
  };
  for (const p of look.from) {
    const x = tickToX(v, p.tick);
    if (x >= -d.point && x <= v.width + d.point) frame(x, valueToY(d, p.value), d.point);
  }
  pts.forEach((_, i) => {
    const x = xs[i]!;
    if (x < -d.point || x > v.width + d.point) return;
    const y = ys[i]!;
    const on = look.selected(i);
    if (look.armed && !on) frame(x, y, d.point);
    else out.push({ x: x - half, y: y - half, w: d.point, h: d.point, color: on ? "color.accent" : "color.gray.10" });
    if (i === look.hovered && !on) frame(x, y, d.point + 2 * d.hair);
  });
  return out;
}

// ---- Where it was pressed ----

/** The point at (x, y) (up to grab px around the point's square). The nearest point's index, or -1. */
export function pointAt(v: View, pts: readonly Pt[], d: LaneDims, x: number, y: number, grab: number): number {
  let best = -1;
  let bestD = Infinity;
  pts.forEach((p, i) => {
    const dx = Math.abs(tickToX(v, p.tick) - x);
    const dy = Math.abs(valueToY(d, p.value) - y);
    if (dx <= grab && dy <= grab && dx + dy < bestD) {
      best = i;
      bestD = dx + dy;
    }
  });
  return best;
}

/** The indexes of the points inside the box (dragged from either corner), in tick order. */
export function pointsInBox(v: View, pts: readonly Pt[], d: LaneDims, ax: number, ay: number, bx: number, by: number): number[] {
  const x0 = Math.min(ax, bx);
  const x1 = Math.max(ax, bx);
  const y0 = Math.min(ay, by);
  const y1 = Math.max(ay, by);
  const out: number[] = [];
  pts.forEach((p, i) => {
    const x = tickToX(v, p.tick);
    const y = valueToY(d, p.value);
    if (x >= x0 && x <= x1 && y >= y0 && y <= y1) out.push(i);
  });
  return out;
}

/** The tick of a point added by pressing an empty place (the start of a beat; a tick with Shift). */
export function addTick(v: View, map: TempoMap, x: number, shift: boolean): number {
  const at = Math.max(0, xToTick(v, x));
  return shift ? Math.round(at) : nearestBeatTick(map, at);
}

/** A range of indexes (a list of [start, end)) to a Set of indexes. */
export function indexSet(ranges: readonly { start: number; end: number }[]): Set<number> {
  const out = new Set<number>();
  for (const r of ranges) for (let i = r.start; i < r.end; i++) out.add(i);
  return out;
}

/** The position change for the reading, 「+0. 1. 0000」 (bar . beat . tick, counted in the time signature at the original place). */
export function deltaText(ticks: number, perBar: number, perBeat: number): string {
  const a = Math.abs(Math.round(ticks));
  const bars = Math.floor(a / perBar);
  const beats = Math.floor((a % perBar) / perBeat);
  return `${ticks < 0 ? "−" : "+"}${bars}. ${beats}. ${tickText(a % perBeat)}`;
}
