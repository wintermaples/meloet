// アレンジのクリップのマウス（仕様書 3 章「第 3 回のための決まり」の「アレンジのクリップ」、4 章、モック R3-Arrange）。DOM を知らない。
// 押す → 動かす → 離す。動かしている間は見た目だけを変え、離したときに MoveClip か TrimClip を 1 つだけ送る（docs/architecture.md 17 章）。
// 座標は arrangeGeom.ts と同じ（x は見えている所の左はしから、y はトラックの行の並びの上から、CSS px）。

import type { ClipEdge, ClipView, Command, TrackView } from "../bridge";
import { t } from "../text/ja";
import { clipBox, type LaneDims, type View } from "./arrangeGeom";
import { positionText, signed, snapTick } from "./pianoroll/geom";
import { DISPLAY_TICKS_PER_BEAT, ticksPerBar, ticksPerBeat, type Meter } from "./time";

export interface ClipHit {
  clip: ClipView;
  row: number;
  /** 左右のはし（part.clip.edge）。はしでなければ null。 */
  edge: ClipEdge | null;
}

/** (x, y) の下のクリップ。はしの幅は、せまいクリップでもまん中が残るように、幅の 1/3 まで。 */
export function clipHit(v: View, d: LaneDims, tracks: readonly TrackView[], x: number, y: number, edgePx: number): ClipHit | null {
  const row = Math.floor(y / d.trackH);
  const track = tracks[row];
  if (!track) return null;
  for (const clip of track.clips) {
    const b = clipBox(v, d, row, clip.start, clip.end);
    if (x < b.x0 || x >= b.x1 || y < b.y0 || y >= b.y1) continue;
    const e = Math.min(edgePx, (b.x1 - b.x0) / 3);
    const edge: ClipEdge | null = x < b.x0 + e ? "start" : x >= b.x1 - e ? "end" : null;
    return { clip, row, edge };
  }
  return null;
}

interface Press {
  x: number;
  y: number;
  /** space.1 より動いたら true（それまでは、ただ押しただけ） */
  moved: boolean;
}

export type ClipDrag =
  /** ticks: もとの始まりから動かす量。toRow: 置くトラックの行。ok: そこに置けるか（ほかのクリップと重ならないか）。 */
  | { kind: "move"; press: Press; clip: ClipView; row: number; copy: boolean; ticks: number; toRow: number; ok: boolean }
  /** start ・ end: いまのはしの位置（曲の中のティック）。 */
  | { kind: "trim"; press: Press; clip: ClipView; row: number; edge: ClipEdge; start: number; end: number };

export interface DragInput {
  view: View;
  dims: LaneDims;
  tracks: readonly TrackView[];
  /** グリッド 1 つのティック（アレンジはふだん 1 小節）。Shift のときは null。 */
  step: number | null;
  /** 押しただけとみなす動き（space.1） */
  stillPx: number;
}

export interface DragMods {
  shift: boolean;
  alt: boolean;
}

export function startClipDrag(hit: ClipHit, x: number, y: number, mods: DragMods): ClipDrag {
  const press = { x, y, moved: false };
  const { clip, row } = hit;
  if (hit.edge) return { kind: "trim", press, clip, row, edge: hit.edge, start: clip.start, end: clip.end };
  return { kind: "move", press, clip, row, copy: mods.alt, ticks: 0, toRow: row, ok: true };
}

/** [start, end) を行 row に置けるか。without: 数に入れないクリップ（動かしている物）。端が触れるだけは重ならない。 */
export function placeable(tracks: readonly TrackView[], row: number, start: number, end: number, without: number | null): boolean {
  const track = tracks[row];
  if (!track || start < 0) return false;
  return !track.clips.some((c) => c.id !== without && c.start < end && start < c.end);
}

/** 同じトラックの、clip のすぐ前のクリップの終わり（なければ 0）と、すぐ後のクリップの始まり（なければ Infinity）。 */
export function neighbours(track: TrackView, clip: ClipView): { floor: number; ceiling: number } {
  let floor = 0;
  let ceiling = Infinity;
  for (const c of track.clips) {
    if (c.id === clip.id) continue;
    if (c.end <= clip.start) floor = Math.max(floor, c.end);
    if (c.start >= clip.end) ceiling = Math.min(ceiling, c.start);
  }
  return { floor, ceiling };
}

const sameMove = (a: Extract<ClipDrag, { kind: "move" }>, b: Omit<typeof a, "kind" | "press" | "clip" | "row">) =>
  a.ticks === b.ticks && a.toRow === b.toRow && a.copy === b.copy && a.ok === b.ok;

/** 押したまま動かした。見た目が変わったら新しい物を、変わらなければ同じ物を返す。 */
export function dragClipTo(d: ClipDrag, i: DragInput, x: number, y: number, mods: DragMods): ClipDrag {
  let press = d.press;
  if (!press.moved) {
    if (Math.hypot(x - press.x, y - press.y) < i.stillPx) return d;
    press = { ...press, moved: true };
  }
  const dx = (x - press.x) / i.view.pxPerTick;
  const snap = (tick: number) => (i.step ? snapTick(tick, i.step) : Math.round(tick));
  const c = d.clip;
  if (d.kind === "move") {
    const start = Math.max(0, snap(c.start + dx));
    const rows = Math.floor(y / i.dims.trackH) - Math.floor(press.y / i.dims.trackH);
    const toRow = Math.min(i.tracks.length - 1, Math.max(0, d.row + rows));
    const copy = mods.alt;
    const next = {
      ticks: start - c.start,
      toRow,
      copy,
      ok: placeable(i.tracks, toRow, start, start + (c.end - c.start), copy ? null : c.id),
    };
    if (press === d.press && sameMove(d, next)) return d;
    return { ...d, press, ...next };
  }
  const track = i.tracks[d.row];
  if (!track) return d;
  const { floor, ceiling } = neighbours(track, c);
  // 吸いつくときは、グリッド 1 つ（もとがそれより短ければ、もとの長さ）より短くしない。
  const shortest = i.step ? Math.min(i.step, c.end - c.start) : 1;
  let start = c.start;
  let end = c.end;
  if (d.edge === "start") start = Math.min(Math.max(snap(c.start + dx), floor), c.end - shortest);
  else end = Math.max(Math.min(snap(c.end + dx), ceiling), c.start + shortest);
  if (press === d.press && start === d.start && end === d.end) return d;
  return { ...d, press, start, end };
}

/** 離したときに送る意図。押しただけ、置けない所、何も変わらないときは null。 */
export function clipDragCommand(d: ClipDrag, tracks: readonly TrackView[]): Command | null {
  if (!d.press.moved) return null;
  if (d.kind === "move") {
    const track = tracks[d.toRow];
    if (!d.ok || !track || (d.ticks === 0 && d.toRow === d.row && !d.copy)) return null;
    return { type: "moveClip", clip: d.clip.id, ticks: d.ticks, track: track.number, copy: d.copy };
  }
  const ticks = d.edge === "start" ? d.start - d.clip.start : d.end - d.clip.end;
  return ticks === 0 ? null : { type: "trimClip", clip: d.clip.id, edge: d.edge, ticks };
}

/** カーソルの形（OS の形。仕様書 3 章）。 */
export type ArrangeCursor = "default" | "move" | "copy" | "ew-resize" | "not-allowed";

export function hoverCursor(hit: ClipHit | null, alt: boolean): ArrangeCursor {
  if (!hit) return "default";
  if (hit.edge) return "ew-resize";
  return alt ? "copy" : "move";
}

export function clipDragCursor(d: ClipDrag): ArrangeCursor {
  if (d.kind === "trim") return "ew-resize";
  if (d.press.moved && !d.ok) return "not-allowed";
  return d.copy ? "copy" : "move";
}

/** 「+1」（小節）。グリッドに吸いつかずに動かして小節の途中なら「+1.25」（小数 3 けたまで）。 */
export function barsDeltaText(ticks: number, perBar: number): string {
  const bars = ticks / perBar;
  if (Number.isInteger(bars)) return signed(bars);
  return `${bars < 0 ? "−" : "+"}${Number(Math.abs(bars).toFixed(3))}`;
}

/** 「2.2.000」: クリップの長さを 小節 . 拍 . ティック（どれも 0 から）。 */
export function clipLengthText(ticks: number, meter: Meter): string {
  const perBar = ticksPerBar(meter);
  const perBeat = ticksPerBeat(meter);
  const t = Math.max(0, Math.round(ticks));
  const bars = Math.floor(t / perBar);
  const beats = Math.floor((t % perBar) / perBeat);
  const rest = Math.floor(((t % perBeat) * DISPLAY_TICKS_PER_BEAT) / perBeat);
  return `${bars}.${beats}.${String(rest).padStart(3, "0")}`;
}

/** 「その場の読み値」と状態行に出す文。出さないときは null。 */
export function clipDragReading(d: ClipDrag, meter: Meter): string | null {
  if (!d.press.moved) return null;
  if (d.kind === "trim") return t("arrange.readTrim", { length: clipLengthText(d.end - d.start, meter) });
  const position = positionText(d.clip.start + d.ticks, meter);
  if (d.copy) return t("arrange.readCopy", { position });
  return t("arrange.readMove", { position, delta: barsDeltaText(d.ticks, ticksPerBar(meter)) });
}
