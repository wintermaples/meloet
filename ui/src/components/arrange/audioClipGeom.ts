// Geometry of audio clips (spec 2 "Audio clip", spec 3 (27-A, 29-A, 30-C), mock F7-AudioEdit / F7-AudioDrop / R11-Audio).
// The one place where the WebGL side (waveforms) and the HTML side (handles, gain line, readout, drop frame) read the same numbers.
// All in CSS px. x and y are from the top left of the arrange clip area (arrangeLanes).

import type { ClipView, DropMode, TrackView } from "../../bridge";
import { CLIP_GAIN_MAX_DB, CLIP_GAIN_MIN_DB } from "../../bridge/types";
import type { TempoMap } from "../../time/tempoMap";
import { clipBox, rowHeight, rowTop, tickToX, trackRowAt, xToTick, type Box, type LaneDims, type View } from "../arrangeGeom";
import type { GlRect } from "../../gl/slot";
import { dbToFrac, fracToDb } from "../mixer/scale";
import { fadeGain } from "./fade";
import { cachedSourceMap, clipFrameAt, followClipOf } from "./follow";

/** What the geometry needs from an audio clip (from `ClipView` and `AudioClipView`). */
export interface AudioClipShape {
  start: number;
  end: number;
  fadeIn: number;
  fadeOut: number;
}

export const audioShapeOf = (clip: ClipView): AudioClipShape | null =>
  clip.audio === null ? null : { start: clip.start, end: clip.end, fadeIn: clip.audio.fadeIn, fadeOut: clip.audio.fadeOut };

/** Where the waveform, fade line and gain line go: from below the name row to above the bottom border. */
export interface WaveArea {
  x0: number;
  x1: number;
  top: number;
  bottom: number;
  /** The waveform center line (gray.5 in a 160 row). */
  mid: number;
}

export function waveArea(d: LaneDims, box: Box): WaveArea {
  const top = Math.min(box.y0 + d.mark + d.nameH, box.y1 - d.hair);
  const bottom = box.y1 - d.hair;
  return { x0: box.x0 + d.hair, x1: box.x1 - d.hair, top, bottom, mid: (top + bottom) / 2 };
}

// ---- Gain line (1px accent. Drag up and down for −60 to +12 dB) ----
// The scale is the same as the mixer fader (spec 2 "Scale marks of meters and faders", spec 3 (39-B)): from the bottom up to 0 dB as in the table,
// and straight in dB from 0 dB (20 % from the top) to +12 at the top. The bottom is the −∞ position, but the value does not go below −60 (docs/history/design-notes.ja.md A-24, item 12).

const ZERO_FRAC = dbToFrac(0);

/** dB → length from the bottom, 0..1. */
function gainFrac(db: number): number {
  const v = clampGain(db);
  return v >= 0 ? ZERO_FRAC + (v / CLIP_GAIN_MAX_DB) * (1 - ZERO_FRAC) : dbToFrac(v);
}

/** dB → line y. */
export function gainLineY(area: WaveArea, db: number): number {
  return area.bottom - gainFrac(db) * (area.bottom - area.top);
}

/** Line y → dB (in range, steps of 0.1: as fine as the readout 「−3.0 dB」). */
export function gainAtY(area: WaveArea, y: number): number {
  const height = area.bottom - area.top;
  if (height <= 0) return 0;
  const frac = Math.min(1, Math.max(0, (area.bottom - y) / height));
  const db = frac >= ZERO_FRAC ? ((frac - ZERO_FRAC) / (1 - ZERO_FRAC)) * CLIP_GAIN_MAX_DB : (fracToDb(frac) ?? CLIP_GAIN_MIN_DB);
  return clampGain(Math.round(db * 10) / 10);
}

export const clampGain = (db: number) => Math.min(CLIP_GAIN_MAX_DB, Math.max(CLIP_GAIN_MIN_DB, db));

/** Clip gain (dB) as the factor applied to the waveform height. */
export const gainFactor = (db: number) => 10 ** (clampGain(db) / 20);

// ---- Fades (drag the 6×6 handles at the top left and right corners sideways. The line shape is in fade.ts) ----

export type FadeEdge = "fadeIn" | "fadeOut";

/** The x where the fade-in ends and the x where the fade-out starts. */
export function fadeXs(v: View, clip: AudioClipShape): { inX: number; outX: number } {
  return { inX: tickToX(v, clip.start + clip.fadeIn), outX: tickToX(v, clip.end - clip.fadeOut) };
}

/**
 * The handle box (`size` is part.clip.edge, 6). Centered on the fade edge x, at a height that crosses the line under the name row.
 * With no fade (0), it sits right at the clip corner.
 */
export function fadeHandleBox(v: View, d: LaneDims, box: Box, clip: AudioClipShape, edge: FadeEdge, size: number): Box {
  const area = waveArea(d, box);
  const xs = fadeXs(v, clip);
  const center = edge === "fadeIn" ? xs.inX : xs.outX;
  const x0 = Math.min(Math.max(center - size / 2, area.x0), Math.max(area.x0, area.x1 - size));
  const y0 = area.top - size / 3;
  return { x0, x1: x0 + size, y0, y1: y0 + size };
}

/** Adds the left and right handle rects (gray.10) to `out`. The WebGL side draws them. */
export function handleRects(out: GlRect[], v: View, d: LaneDims, box: Box, clip: AudioClipShape, size: number): void {
  for (const edge of ["fadeIn", "fadeOut"] as const) {
    const b = fadeHandleBox(v, d, box, clip, edge, size);
    out.push({ x: b.x0, y: b.y0, w: b.x1 - b.x0, h: b.y1 - b.y0, color: "color.gray.10" });
  }
}

/** The fade length (ticks) when the handle is dragged to x. From 0 up to (clip length − the other fade). */
export function fadeTicksAt(v: View, clip: AudioClipShape, edge: FadeEdge, x: number): number {
  const tick = xToTick(v, x);
  const other = edge === "fadeIn" ? clip.fadeOut : clip.fadeIn;
  const most = Math.max(0, clip.end - clip.start - other);
  const ticks = edge === "fadeIn" ? tick - clip.start : clip.end - tick;
  return Math.round(Math.min(most, Math.max(0, ticks)));
}

/** Level (0-1) → fade line y: 0 is the bottom, 1 is the top. */
export const fadeLineY = (area: WaveArea, gain: number) => area.bottom - gain * (area.bottom - area.top);

/**
 * Points of the fade line (left to right): one every `stepPx`, plus both ends. Empty with no fade.
 * Not cut to the visible width (the line is short).
 */
export function fadeLinePoints(v: View, area: WaveArea, clip: AudioClipShape, edge: FadeEdge, stepPx = 2): { x: number; y: number }[] {
  const length = edge === "fadeIn" ? clip.fadeIn : clip.fadeOut;
  if (length <= 0) return [];
  const { inX, outX } = fadeXs(v, clip);
  const from = edge === "fadeIn" ? tickToX(v, clip.start) : outX;
  const to = edge === "fadeIn" ? inX : tickToX(v, clip.end);
  const width = to - from;
  const count = Math.max(1, Math.ceil(width / stepPx));
  const points: { x: number; y: number }[] = [];
  for (let i = 0; i <= count; i++) {
    const part = i / count;
    points.push({ x: from + part * width, y: fadeLineY(area, fadeGain(edge === "fadeIn" ? part : 1 - part)) });
  }
  return points;
}

// ---- Waveform place (which part of the take lands where in the clip) ----

/**
 * The file frame (fractional) heard at song tick `tick`. Tick 0 of the take (`clip.start + clip.takeOffset`) is
 * `firstFrame`. With `sourceTempo` it follows the song tempo, without it the file plays at its own speed (`clipFrameAt` in follow.ts).
 */
export function fileFrameAt(map: TempoMap, clip: ClipView, tick: number): number {
  const follow = followClipOf(clip);
  return follow === null ? 0 : clipFrameAt(map, cachedSourceMap(clip.audio!.sourceTempo), follow, tick);
}

// ---- The row for editing audio (160) ----

/** Track row height: only the row being edited is `tall` (part.track.vocal, 160); others are `d.trackH`. */
export function rowHeights(d: LaneDims, tracks: readonly Pick<TrackView, "id">[], tallTrack: number | null, tall: number): number[] {
  return tracks.map((t) => (t.id === tallTrack ? tall : d.trackH));
}

// ---- While an audio file is being dragged in (29-A, 30-C) ----

/** Which side of the center box the pointer is on: the left half of the window is 「別々のトラックに」, the right half 「1 本にならべる」. */
export const dropModeAt = (pageX: number, windowWidth: number): DropMode => (pageX < windowWidth / 2 ? "separate" : "inRow");

export interface DropTarget {
  /** The vocal track where it is dropped (`TrackView.id`). null over a keyboard track or where there is no track (a new vocal track). */
  track: number | null;
  /** The row that shows the frame. For a new track, the one after the last row. */
  row: number;
  /** The tick snapped to a beat. */
  tick: number;
}

/** Where the audio file goes under the pointer. `snap` snaps to a beat (`nearestBeatTick`). */
export function dropTarget(v: View, d: LaneDims, tracks: readonly TrackView[], x: number, y: number, snap: (tick: number) => number): DropTarget {
  const tick = Math.max(0, snap(Math.max(0, xToTick(v, x))));
  const row = trackRowAt(d, y);
  const under = row >= 0 ? tracks[row] : undefined;
  if (under && under.kind.kind === "vocal") return { track: under.id, row, tick };
  return { track: null, row: tracks.length, tick };
}

/**
 * The box of the gray.7 frame where it goes. The length is not known yet, so it is `ticks` wide (one bar, for example). `rows` is the number of tracks.
 * A new track (`target.row === rows`) is just below the last row, at the normal row height.
 */
export function dropFrameBox(v: View, d: LaneDims, target: DropTarget, rows: number, ticks: number): Box {
  if (target.row < rows) return clipBox(v, d, target.row, target.tick, target.tick + ticks);
  const top = rows === 0 ? 0 : rowTop(d, rows - 1) + rowHeight(d, rows - 1);
  return { x0: tickToX(v, target.tick), x1: tickToX(v, target.tick + ticks), y0: top + d.inset, y1: top + d.trackH - d.hair - d.inset };
}
