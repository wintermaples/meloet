// The mouse on arrange clips (spec 3 "Rules for round 3", the arrange clip part; spec 4; mock R3-Arrange). Does not know the DOM.
// Press → move → release. While moving, only the look changes; on release, exactly one MoveClips or TrimClip is sent (architecture 17).
// Moving takes all selected clips together (docs/history/design-notes.ja.md B-71). Trimming or extending an edge is only for the one grabbed clip.
// Coordinates are the same as arrangeGeom.ts (x from the left edge of the visible part, y from the top of the track rows, CSS px).

import type { ClipEdge, ClipView, Command, TrackView } from "../bridge";
import { t } from "../text/ja";
import { blockAt, clipBox, trackRowAt, type LaneDims, type View } from "./arrangeGeom";
import { positionText, signed, snapTick } from "./pianoroll/geom";
import { barFloat, barsMap } from "../time/bars";
import { tickText, ticksPerBar, ticksPerBeat, type Bars, type Meter } from "./time";

export interface ClipHit {
  clip: ClipView;
  row: number;
  /** The left or right edge (part.clip.edge). null when not on an edge. */
  edge: ClipEdge | null;
}

/** The clip under (x, y). The edge width is at most 1/3 of the clip width, so a narrow clip keeps a middle. */
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
  /** true once it moved more than space.1 (until then, it was only a press) */
  moved: boolean;
}

/** One of the clips that move together. */
export interface GroupClip {
  clip: ClipView;
  row: number;
}

export type ClipDrag =
  /**
   * clip, row: the grabbed clip. group: the clips that move together (including the grabbed clip; in track order, then start order).
   * ticks: how far to move from the original start. toRow: the track row for the grabbed clip. ok: all stay inside tracks and overlap nothing.
   */
  | { kind: "move"; press: Press; clip: ClipView; row: number; group: GroupClip[]; copy: boolean; ticks: number; toRow: number; ok: boolean }
  /** start, end: the current edge positions (ticks in the song). */
  | { kind: "trim"; press: Press; clip: ClipView; row: number; edge: ClipEdge; start: number; end: number };

export interface DragInput {
  view: View;
  dims: LaneDims;
  tracks: readonly TrackView[];
  /** Ticks of one grid step (the arrange usually uses 1 bar). null with Shift. */
  step: number | null;
  /** Snap to the grid (for songs where time signature changes alter the bar length; otherwise steps of step). */
  snap?: (tick: number) => number;
  /** Movement still taken as a plain press (space.1) */
  stillPx: number;
}

export interface DragMods {
  shift: boolean;
  alt: boolean;
}

/** The clips of ids, in track order, then start order. Missing clips are dropped. */
export function groupOf(tracks: readonly TrackView[], ids: Iterable<number>): GroupClip[] {
  const want = new Set(ids);
  const out: GroupClip[] = [];
  tracks.forEach((track, row) => {
    for (const clip of track.clips) if (want.has(clip.id)) out.push({ clip, row });
  });
  return out;
}

/** group: the clips that move together (only the grabbed clip if it is not in them). Not used when an edge is grabbed. */
export function startClipDrag(hit: ClipHit, x: number, y: number, mods: DragMods, group: readonly GroupClip[] = []): ClipDrag {
  const press = { x, y, moved: false };
  const { clip, row } = hit;
  if (hit.edge) return { kind: "trim", press, clip, row, edge: hit.edge, start: clip.start, end: clip.end };
  const members = group.some((g) => g.clip.id === clip.id) ? [...group] : [{ clip, row }];
  return { kind: "move", press, clip, row, group: members, copy: mods.alt, ticks: 0, toRow: row, ok: true };
}

/** Whether [start, end) can go on row. without: clips not counted (the moving ones). Ends that only touch do not overlap. */
export function placeable(tracks: readonly TrackView[], row: number, start: number, end: number, without: number | null): boolean {
  const track = tracks[row];
  if (!track || start < 0) return false;
  return !track.clips.some((c) => c.id !== without && c.start < end && start < c.end);
}

/**
 * Whether group can be placed after moving by ticks and rows. false if any one goes outside the tracks, before the song start, or overlaps another clip.
 * For a copy, the original clips are in the way too.
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

/** Clips touching the dragged rectangle (from either corner). In track order, then start order. */
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

/** On the same track, the end of the clip just before clip (0 if none) and the start of the clip just after (Infinity if none). */
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

/** Moved while pressed. Returns a new object if the look changed, the same one if not. */
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
    // The grabbed clip snaps to the grid; the others follow with the same gaps. The front clip stops at the song start.
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
  // When snapping, do not make it shorter than one grid step (or the original length, if that was shorter).
  const shortest = i.step ? Math.min(i.step, c.end - c.start) : 1;
  let start = c.start;
  let end = c.end;
  if (d.edge === "start") start = Math.min(Math.max(snap(c.start + dx), floor), c.end - shortest);
  else end = Math.max(Math.min(snap(c.end + dx), ceiling), c.start + shortest);
  if (press === d.press && start === d.start && end === d.end) return d;
  return { ...d, press, start, end };
}

/** The intent to send on release. null for a plain press, a place where it cannot go, or no change. */
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

/** The cursor shape (an OS shape; spec 3). */
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

/** 「+1」 (bars). If moved without snapping and it lands mid-bar, 「+1.25」 (up to 3 decimals). */
export function barsDeltaText(ticks: number, perBar: number): string {
  const bars = ticks / perBar;
  if (Number.isInteger(bars)) return signed(bars);
  return `${bars < 0 ? "−" : "+"}${Number(Math.abs(bars).toFixed(3))}`;
}

/** 「2.2.0000」: the clip length as bar . beat . tick (all from 0). */
export function clipLengthText(ticks: number, meter: Meter): string {
  const perBar = ticksPerBar(meter);
  const perBeat = ticksPerBeat(meter);
  const t = Math.max(0, Math.round(ticks));
  const bars = Math.floor(t / perBar);
  const beats = Math.floor((t % perBar) / perBeat);
  return `${bars}.${beats}.${tickText(t % perBeat)}`;
}

/** The text for the reading in place and the status line. null when not shown. The length counts by the time signature at the clip start. */
export function clipDragReading(d: ClipDrag, bars: Bars): string | null {
  if (!d.press.moved) return null;
  const map = barsMap(bars);
  if (d.kind === "trim") return t("arrange.readTrim", { length: clipLengthText(d.end - d.start, map.meterAt(d.start)) });
  const position = positionText(d.clip.start + d.ticks, map);
  if (d.copy) return t("arrange.readCopy", { position });
  const bars1 = barFloat(map, d.clip.start + d.ticks) - barFloat(map, d.clip.start);
  return t("arrange.readMove", { position, delta: barsDeltaText(bars1, 1) });
}
