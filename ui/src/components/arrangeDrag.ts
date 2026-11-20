// アレンジのクリップのマウス（仕様書 3 章「第 3 回のための決まり」の「アレンジのクリップ」、4 章、モック R3-Arrange）。DOM を知らない。
// 押す → 動かす → 離す。動かしている間は見た目だけを変え、離したときに MoveClips か TrimClip を 1 つだけ送る（docs/architecture.md 17 章）。
// 動かすときは、選んであるクリップぜんぶをいっしょに（docs/design-notes.md B-71）。はしをつめる ・ のばすのは、つかんだ 1 つだけ。
// 座標は arrangeGeom.ts と同じ（x は見えている所の左はしから、y はトラックの行の並びの上から、CSS px）。

import type { ClipEdge, ClipView, Command, TrackView } from "../bridge";
import { t } from "../text/ja";
import { blockAt, clipBox, trackRowAt, type LaneDims, type View } from "./arrangeGeom";
import { positionText, signed, snapTick } from "./pianoroll/geom";
import { barFloat, barsMap } from "../time/bars";
import { tickText, ticksPerBar, ticksPerBeat, type Bars, type Meter } from "./time";

export interface ClipHit {
  clip: ClipView;
  row: number;
  /** 左右のはし（part.clip.edge）。はしでなければ null。 */
  edge: ClipEdge | null;
}

/** (x, y) の下のクリップ。はしの幅は、せまいクリップでもまん中が残るように、幅の 1/3 まで。 */
export function clipHit(v: View, d: LaneDims, tracks: readonly TrackView[], x: number, y: number, edgePx: number): ClipHit | null {
  const row = trackRowAt(d, y);
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

/** いっしょに動かすクリップの 1 つ。 */
export interface GroupClip {
  clip: ClipView;
  row: number;
}

export type ClipDrag =
  /**
   * clip ・ row: つかんだクリップ。group: いっしょに動かすクリップ（つかんだクリップもふくむ。トラックの順、始まりの順）。
   * ticks: もとの始まりから動かす量。toRow: つかんだクリップを置くトラックの行。ok: ぜんぶトラックの中で、ほかと重ならないか。
   */
  | { kind: "move"; press: Press; clip: ClipView; row: number; group: GroupClip[]; copy: boolean; ticks: number; toRow: number; ok: boolean }
  /** start ・ end: いまのはしの位置（曲の中のティック）。 */
  | { kind: "trim"; press: Press; clip: ClipView; row: number; edge: ClipEdge; start: number; end: number };

export interface DragInput {
  view: View;
  dims: LaneDims;
  tracks: readonly TrackView[];
  /** グリッド 1 つのティック（アレンジはふだん 1 小節）。Shift のときは null。 */
  step: number | null;
  /** グリッドに吸いつける（拍子の変更で小節の長さが変わる曲。なければ step のきざみ）。 */
  snap?: (tick: number) => number;
  /** 押しただけとみなす動き（space.1） */
  stillPx: number;
}

export interface DragMods {
  shift: boolean;
  alt: boolean;
}

/** ids のクリップを、トラックの順、始まりの順に。ないクリップは落とす。 */
export function groupOf(tracks: readonly TrackView[], ids: Iterable<number>): GroupClip[] {
  const want = new Set(ids);
  const out: GroupClip[] = [];
  tracks.forEach((track, row) => {
    for (const clip of track.clips) if (want.has(clip.id)) out.push({ clip, row });
  });
  return out;
}

/** group: いっしょに動かすクリップ（つかんだクリップが入っていなければ、つかんだクリップだけ）。はしをつかんだときは使わない。 */
export function startClipDrag(hit: ClipHit, x: number, y: number, mods: DragMods, group: readonly GroupClip[] = []): ClipDrag {
  const press = { x, y, moved: false };
  const { clip, row } = hit;
  if (hit.edge) return { kind: "trim", press, clip, row, edge: hit.edge, start: clip.start, end: clip.end };
  const members = group.some((g) => g.clip.id === clip.id) ? [...group] : [{ clip, row }];
  return { kind: "move", press, clip, row, group: members, copy: mods.alt, ticks: 0, toRow: row, ok: true };
}

/** [start, end) を行 row に置けるか。without: 数に入れないクリップ（動かしている物）。端が触れるだけは重ならない。 */
export function placeable(tracks: readonly TrackView[], row: number, start: number, end: number, without: number | null): boolean {
  const track = tracks[row];
  if (!track || start < 0) return false;
  return !track.clips.some((c) => c.id !== without && c.start < end && start < c.end);
}

/**
 * group を ticks ・ rows だけ動かして置けるか。どれか 1 つでもトラックの外に出る、曲の頭より前に出る、ほかのクリップと重なるなら false。
 * 複製（copy）なら、もとのクリップもじゃまになる。
 */
export function groupPlaceable(tracks: readonly TrackView[], group: readonly GroupClip[], ticks: number, rows: number, copy: boolean): boolean {
  const moving = copy ? null : new Set(group.map((g) => g.clip.id));
  return group.every(({ clip, row }) => {
    const track = tracks[row + rows];
    const start = clip.start + ticks;
    const end = clip.end + ticks;
    if (!track || start < 0) return false;
    return !track.clips.some((c) => !moving?.has(c.id) && c.start < end && start < c.end);
  });
}

/** 囲んだ四角（どちらの角からでも）にかかるクリップ。トラックの順、始まりの順。 */
export function clipsInBox(v: View, d: LaneDims, tracks: readonly TrackView[], ax: number, ay: number, bx: number, by: number): number[] {
  const x0 = Math.min(ax, bx);
  const x1 = Math.max(ax, bx);
  const y0 = Math.min(ay, by);
  const y1 = Math.max(ay, by);
  const out: number[] = [];
  tracks.forEach((track, row) => {
    for (const c of track.clips) {
      const b = clipBox(v, d, row, c.start, c.end);
      if (b.x0 <= x1 && x0 < b.x1 && b.y0 <= y1 && y0 < b.y1) out.push(c.id);
    }
  });
  return out;
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

const sameMove = (a: Extract<ClipDrag, { kind: "move" }>, b: Omit<typeof a, "kind" | "press" | "clip" | "row" | "group">) =>
  a.ticks === b.ticks && a.toRow === b.toRow && a.copy === b.copy && a.ok === b.ok;

/** 押したまま動かした。見た目が変わったら新しい物を、変わらなければ同じ物を返す。 */
export function dragClipTo(d: ClipDrag, i: DragInput, x: number, y: number, mods: DragMods): ClipDrag {
  let press = d.press;
  if (!press.moved) {
    if (Math.hypot(x - press.x, y - press.y) < i.stillPx) return d;
    press = { ...press, moved: true };
  }
  const dx = (x - press.x) / i.view.pxPerTick;
  const snap = (tick: number) => (i.step ? (i.snap ? i.snap(tick) : snapTick(tick, i.step)) : Math.round(tick));
  const c = d.clip;
  if (d.kind === "move") {
    // つかんだクリップをグリッドに吸いつけ、ほかは同じ間をあけてついてくる。いちばん前のクリップが曲の頭で止まる。
    const earliest = Math.min(...d.group.map((g) => g.clip.start));
    const ticks = Math.max(snap(c.start + dx) - c.start, -earliest) || 0;
    const rows = blockAt(i.dims, y) - blockAt(i.dims, press.y);
    const toRow = Math.min(i.tracks.length - 1, Math.max(0, d.row + rows));
    const copy = mods.alt;
    const next = {
      ticks,
      toRow,
      copy,
      ok: groupPlaceable(i.tracks, d.group, ticks, toRow - d.row, copy),
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
    return { type: "moveClips", clips: d.group.map((g) => g.clip.id), ticks: d.ticks, tracks: d.toRow - d.row, copy: d.copy };
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

/** 「2.2.0000」: クリップの長さを 小節 . 拍 . ティック（どれも 0 から）。 */
export function clipLengthText(ticks: number, meter: Meter): string {
  const perBar = ticksPerBar(meter);
  const perBeat = ticksPerBeat(meter);
  const t = Math.max(0, Math.round(ticks));
  const bars = Math.floor(t / perBar);
  const beats = Math.floor((t % perBar) / perBeat);
  return `${bars}.${beats}.${tickText(t % perBeat)}`;
}

/** 「その場の読み値」と状態行に出す文。出さないときは null。長さは、クリップの始まりの所の拍子で数える。 */
export function clipDragReading(d: ClipDrag, bars: Bars): string | null {
  if (!d.press.moved) return null;
  const map = barsMap(bars);
  if (d.kind === "trim") return t("arrange.readTrim", { length: clipLengthText(d.end - d.start, map.meterAt(d.start)) });
  const position = positionText(d.clip.start + d.ticks, map);
  if (d.copy) return t("arrange.readCopy", { position });
  const bars1 = barFloat(map, d.clip.start + d.ticks) - barFloat(map, d.clip.start);
  return t("arrange.readMove", { position, delta: barsDeltaText(bars1, 1) });
}
