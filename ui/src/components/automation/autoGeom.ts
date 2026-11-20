// アレンジのオートメーションの帯の形（仕様書 2 章「オートメーションの点」、3 章 Y16、モック R10-AutoEdit ・ F6-AutoRec）。DOM を知らない。
// トラックの行のすぐ下に、ひらいた帯（1 本 size.lane）と「▸ パン ▸ プラグインのパラメータ」の行（size.row）が入る。
// 帯の中の座標は、帯の左上からの CSS px。x は arrangeGeom と同じ「曲の頭からの px − 横のスクロール」。

import type { LanePoints, LaneRef, LaneView, TrackView } from "../../bridge";
import type { ColorName } from "../../design/tokens";
import type { GlRect } from "../../gl/slot";
import { nearestBeatTick } from "../../time/bars";
import type { TempoMap } from "../../time/tempoMap";
import { tickToX, xToTick, type View } from "../arrangeGeom";
import { hasLaneRef } from "./laneValue";

// ---- トラックの行と帯の並び ----

export interface RowDims {
  /** size.track */
  trackH: number;
  /** size.lane: ひらいた帯 1 本 */
  laneH: number;
  /** size.row: 「▸ パン ▸ プラグインのパラメータ」の行 */
  rowH: number;
}

export interface OpenLane {
  track: TrackView;
  lane: LaneView;
  top: number;
}

export interface AddRow {
  track: TrackView;
  top: number;
}

export interface ArrangeRows {
  /** トラックの行の上（トラックの順）。 */
  tops: number[];
  /** トラックのかたまり（行と、その下の帯と行）の下。 */
  bottoms: number[];
  lanes: OpenLane[];
  rows: AddRow[];
  height: number;
}

/**
 * 「▸ パン ▸ プラグインのパラメータ」の行を出すトラック: 帯のあるトラックだけ（モック F6-AutoRec の 2〜5 には行がない）。
 * 帯がまだないトラックの音量 ・ パンの帯を足す所は、仕様書にもモックにもない（design-notes B に戻す）。
 */
export const showsAddRow = (track: TrackView) => track.lanes.length > 0;

/** 帯は足した順。たたんだ帯は、行の中の「▸ （何を）」になる。 */
export function arrangeRows(tracks: readonly TrackView[], folded: readonly LaneRef[], d: RowDims): ArrangeRows {
  const out: ArrangeRows = { tops: [], bottoms: [], lanes: [], rows: [], height: 0 };
  let y = 0;
  for (const track of tracks) {
    out.tops.push(y);
    y += d.trackH;
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
  out.height = y;
  return out;
}

// ---- 帯の中: 値と高さ ----

export interface LaneDims {
  /** 帯の高さ（size.lane） */
  h: number;
  /** 上下のあき（0 と 1 の点が帯からはみ出さない） */
  pad: number;
  /** line.hair */
  hair: number;
  /** 点の大きさ（part.cc.point、6×6） */
  point: number;
}

const span = (d: LaneDims) => Math.max(1, d.h - 2 * d.pad - d.hair);
/** 位置（0〜1）→ y。1 が上。 */
export const valueToY = (d: LaneDims, value: number) => Math.round((d.pad + (1 - Math.min(1, Math.max(0, value))) * span(d)) / d.hair) * d.hair;
/** y → 位置（0〜1 に入れる）。 */
export const yToValue = (d: LaneDims, y: number) => Math.min(1, Math.max(0, 1 - (y - d.pad) / span(d)));
/** 上下に dy px 動かしたときの位置の差。 */
export const dyToValue = (d: LaneDims, dy: number) => -dy / span(d);

export interface Pt {
  tick: number;
  value: number;
}

export function pointList(p: Pick<LanePoints, "ticks" | "values">): Pt[] {
  const out: Pt[] = [];
  for (let i = 0; i < p.ticks.length; i++) out.push({ tick: p.ticks[i]!, value: p.values[i]! });
  return out;
}

// ---- ドラッグ（画面だけが先に動き、離して意図を 1 つ） ----

export interface PointDrag {
  /** 動かす点（帯の中の番号、ティックの順）。 */
  points: number[];
  /** つかんだ点。 */
  grab: number;
  downX: number;
  downY: number;
  moved: boolean;
  dTick: number;
  dValue: number;
}

/** 横は拍の頭に吸いつき（Shift ならティック）、上下は位置。いちばん前の点が曲の頭で止まる。 */
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

/** ドラッグ中の点の並び。動かした点がほかの点と同じティックに来たら、そこの点と入れかわる。moved は動かしている点の印。 */
export function draggedPoints(pts: readonly Pt[], drag: PointDrag | null): { pts: Pt[]; moving: boolean[] } {
  if (!drag?.moved) return { pts: [...pts], moving: pts.map((_, i) => drag?.points.includes(i) ?? false) };
  const chosen = new Set(drag.points);
  const moved = drag.points.map((i) => ({ tick: pts[i]!.tick + drag.dTick, value: Math.min(1, Math.max(0, pts[i]!.value + drag.dValue)) }));
  const taken = new Set(moved.map((p) => p.tick));
  const kept = pts.filter((p, i) => !chosen.has(i) && !taken.has(p.tick));
  const all = [...kept.map((p) => ({ p, moving: false })), ...moved.map((p) => ({ p, moving: true }))].sort((a, b) => a.p.tick - b.p.tick);
  return { pts: all.map((a) => a.p), moving: all.map((a) => a.moving) };
}

// ---- 絵（arrangeLanes の 1 枚に、ほかの slot と同じく四角で） ----

export interface LaneLook {
  /** 休み、仮の休み: 線が gray.7。 */
  resting: boolean;
  /** 「● 書く」がオン: 点は accent の枠だけ（書く前）。 */
  armed: boolean;
  /** えらんだ点（帯の中の番号。ドラッグ中は、動かしている点）。 */
  selected: (i: number) => boolean;
  hovered: number;
  /** ドラッグ中の、点のもとの所（accent の枠）。 */
  from: readonly Pt[];
  /** 点がないときの平らな線（手で置いた値）。分からなければ null。 */
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
  // 斜めの線は、高さの同じ所をまとめた hair の四角の列で。
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

/** 線（gray.9 の 1px、休みは gray.7。階段の帯は階段の形）と点（6×6、ふつう gray.10、えらんだ点 accent、乗った点に accent の 1px の枠）。 */
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

// ---- 押した所 ----

/** (x, y) の点（点の四角のまわり grab px まで）。いちばん近い点の番号、なければ -1。 */
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

/** 囲んだ四角（どちらの角からでも）の中の点の番号（ティックの順）。 */
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

/** 空いた所を押して足す点のティック（拍の頭。Shift ならティック）。 */
export function addTick(v: View, map: TempoMap, x: number, shift: boolean): number {
  const at = Math.max(0, xToTick(v, x));
  return shift ? Math.round(at) : nearestBeatTick(map, at);
}

/** 番号の範囲（[start, end) の列）を番号の Set に。 */
export function indexSet(ranges: readonly { start: number; end: number }[]): Set<number> {
  const out = new Set<number>();
  for (const r of ranges) for (let i = r.start; i < r.end; i++) out.add(i);
  return out;
}

/** 読み値の位置の差「+0. 1. 000」（小節 . 拍 . ティック。もとの所の拍子で数える）。 */
export function deltaText(ticks: number, perBar: number, perBeat: number, displayPerBeat: number): string {
  const a = Math.abs(Math.round(ticks));
  const bars = Math.floor(a / perBar);
  const beats = Math.floor((a % perBar) / perBeat);
  const rest = Math.floor(((a % perBeat) * displayPerBeat) / perBeat);
  return `${ticks < 0 ? "−" : "+"}${bars}. ${beats}. ${String(rest).padStart(3, "0")}`;
}
