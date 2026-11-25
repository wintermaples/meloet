// Shape math for the arrange (clip pictures, take tags, ruler drags, paging, the overview). Pure functions that do not know the DOM.
// Positions are CSS px. x is "px from the song start − horizontal scroll". y is px from the top of the track rows.

import type { ClipView, Command, LiveNote, TickSpan, TrackView } from "../bridge";
import type { ColorName } from "../design/tokens";
import type { GlRect } from "../gl/slot";
import { barLineTick, nearestBarLine } from "../time/bars";
import { TempoMap } from "../time/tempoMap";

/** Where bar lines go: ticks per bar (a song with one time signature), or the tempo map (a song with time signature changes). */
export type BarGrid = number | TempoMap;

/** The tick of a bar line number (from 1). */
export const lineTick = (g: BarGrid, line: number) => (g instanceof TempoMap ? barLineTick(g, line) : (line - 1) * g);
/** The nearest bar line number (from 1). */
const lineNear = (g: BarGrid, tick: number) => (g instanceof TempoMap ? nearestBarLine(g, tick) : Math.max(1, Math.round(tick / g) + 1));

/** Sizes at that scale (CSS px). */
export interface LaneDims {
  /** size.track: row height (including the bottom line) */
  trackH: number;
  /** From the top and bottom of the row to the clip */
  inset: number;
  /** line.hair */
  hair: number;
  /** line.mark: the top edge of a clip */
  mark: number;
  /** Line height of text.s: the name line */
  nameH: number;
  /** space.1: between the name line and the notes, and between the notes and the bottom border */
  pad: number;
  /** Height of the note picture (mock.arrange.note) */
  noteH: number;
  /** The tops of the track rows (an automation band under a row adds a gap; automation/autoGeom.ts). Without it, row number × trackH. */
  tops?: readonly number[];
  /** Track row heights (a row with audio being edited is 160; rowHeights in arrange/audioClipGeom.ts). Without it, every row is trackH. With it, tops are needed too. */
  heights?: readonly number[];
}

/** The top of track row row. */
export const rowTop = (d: LaneDims, row: number) => d.tops?.[row] ?? row * d.trackH;

/** The height of track row row (including the bottom line). */
export const rowHeight = (d: LaneDims, row: number) => d.heights?.[row] ?? d.trackH;

/** The track row under y (clip area only; -1 on a band or below the last row). */
export function trackRowAt(d: LaneDims, y: number): number {
  if (!d.tops) return y >= 0 ? Math.floor(y / d.trackH) : -1;
  for (let i = 0; i < d.tops.length; i++) if (y >= d.tops[i]! && y < d.tops[i]! + rowHeight(d, i)) return i;
  return -1;
}

/** Which track block (the row and the bands under it) y is in. Above the top is 0; below the bottom is the last row. */
export function blockAt(d: LaneDims, y: number): number {
  if (!d.tops) return Math.floor(y / d.trackH);
  let row = 0;
  for (let i = 0; i < d.tops.length; i++) if (y >= d.tops[i]!) row = i;
  return row;
}

export interface View {
  /** px per tick */
  pxPerTick: number;
  scrollX: number;
  /** Visible width */
  width: number;
}

export const tickToX = (v: View, tick: number) => tick * v.pxPerTick - v.scrollX;
export const xToTick = (v: View, x: number) => (x + v.scrollX) / v.pxPerTick;

/** A note in a clip picture. start is the tick in the song. */
export interface NoteShape {
  start: number;
  length: number;
  key: number;
}

export interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** The clip box of track row `row` (from 0) for ticks [start, end). */
export function clipBox(v: View, d: LaneDims, row: number, start: number, end: number): Box {
  return {
    x0: tickToX(v, start),
    x1: tickToX(v, end),
    y0: rowTop(d, row) + d.inset,
    y1: rowTop(d, row) + rowHeight(d, row) - d.hair - d.inset,
  };
}

export interface ClipColors {
  frame: ColorName;
  top: ColorName;
  note: ColorName;
}

export const CLIP_PLAIN = (track: ColorName): ClipColors => ({
  frame: "color.gray.6",
  top: track,
  note: "color.gray.8",
});
export const CLIP_SELECTED: ClipColors = {
  frame: "color.accent.ink",
  top: "color.accent.ink",
  note: "color.gray.8",
};
export const CLIP_RECORDING: ClipColors = {
  frame: "color.rec",
  top: "color.rec",
  note: "color.rec",
};

/** A clip on a muted track (spec 2 "Muted track"): only the inner lines are gray.7. The name and the color strip stay. */
export function clipColorsFor(colors: ClipColors, muted: boolean): ClipColors {
  return muted ? { ...colors, note: "color.gray.7" } : colors;
}

export function trackColor(track: Pick<TrackView, "color">): ColorName {
  return `color.track.${((track.color - 1) % 5) + 1}` as ColorName;
}

/** The key range (lowest and highest key). null without notes. */
export function keyRange(notes: Iterable<NoteShape>): { low: number; high: number } | null {
  let low = Infinity;
  let high = -Infinity;
  for (const n of notes) {
    low = Math.min(low, n.key);
    high = Math.max(high, n.key);
  }
  return low <= high ? { low, high } : null;
}

export interface ClipPaintOptions {
  /** Do not draw the left border (the rest of a recording clip whose first part was drawn already) */
  openLeft?: boolean;
  /** Do not draw the right border (another face draws the rest) */
  openRight?: boolean;
  /** The key range of the notes. Taken from `notes` if missing */
  range?: { low: number; high: number } | null;
  /** Cut the notes at this tick (held keys) */
  until?: number;
  /** Face color. gray.4 if missing (gray.2 for an audio clip whose sound file is not found) */
  ground?: ColorName;
}

/**
 * One clip rectangle: border, top edge, face, and a small picture of the notes (under the name line, the key range filling the height).
 * Nothing outside the visible width is drawn.
 */
export function paintClip(
  out: GlRect[],
  v: View,
  d: LaneDims,
  box: Box,
  colors: ClipColors,
  notes: readonly NoteShape[],
  o: ClipPaintOptions = {},
): void {
  const { y0, y1 } = box;
  const x0 = Math.max(box.x0, -d.hair);
  const x1 = Math.min(box.x1, v.width + d.hair);
  if (x1 <= x0 || y1 <= y0) return;
  const left = o.openLeft || box.x0 < x0 ? 0 : d.hair;
  const right = o.openRight || box.x1 > x1 ? 0 : d.hair;
  out.push({ x: x0, y: y0, w: x1 - x0, h: y1 - y0, color: colors.frame });
  out.push({ x: x0, y: y0, w: x1 - x0, h: d.mark, color: colors.top });
  out.push({
    x: x0 + left,
    y: y0 + d.mark,
    w: x1 - x0 - left - right,
    h: y1 - y0 - d.mark - d.hair,
    color: o.ground ?? "color.gray.4",
  });

  const range = o.range === undefined ? keyRange(notes) : o.range;
  if (!range) return;
  const top = y0 + d.mark + d.nameH + d.pad;
  const bottom = y1 - d.hair - d.pad - d.noteH;
  if (bottom < top) return;
  const span = range.high - range.low;
  const inL = box.x0 + (o.openLeft ? 0 : d.hair);
  const inR = box.x1 - (o.openRight ? 0 : d.hair);
  for (const n of notes) {
    const end = o.until === undefined ? n.start + n.length : Math.min(n.start + n.length, o.until);
    const nx0 = Math.max(tickToX(v, n.start), inL, x0);
    const nx1 = Math.min(Math.max(tickToX(v, end), tickToX(v, n.start) + d.hair), inR, x1);
    if (nx1 <= nx0) continue;
    const y = span === 0 ? (top + bottom) / 2 : top + ((range.high - n.key) / span) * (bottom - top);
    out.push({ x: nx0, y, w: nx1 - nx0, h: d.noteH, color: colors.note });
  }
}

/** Border only (spec 2 "Clip", moving: a gray.7 border at the original place). Edges outside the visible width are not drawn. */
export function paintFrame(out: GlRect[], v: View, d: LaneDims, box: Box, color: ColorName): void {
  const { y0, y1 } = box;
  const x0 = Math.max(box.x0, -d.hair);
  const x1 = Math.min(box.x1, v.width + d.hair);
  if (x1 <= x0 || y1 <= y0) return;
  const h = d.hair;
  out.push({ x: x0, y: y0, w: x1 - x0, h, color }, { x: x0, y: y1 - h, w: x1 - x0, h, color });
  if (box.x0 >= x0) out.push({ x: box.x0, y: y0, w: h, h: y1 - y0, color });
  if (box.x1 <= x1) out.push({ x: box.x1 - h, y: y0, w: h, h: y1 - y0, color });
}

/** A clip's place: its row and its ticks. */
export interface ClipPlace {
  row: number;
  start: number;
  end: number;
}

/**
 * The copies a repeat would make (round 12 A1): copy k of each clip `k × length` later, a gray.2 face with a gray.7 border, for k = 1..count;
 * then a line.mark accent bar at the right end of `handle` after the last copy (where the handle is held). Nothing outside the visible width.
 */
export function paintRepeats(out: GlRect[], v: View, d: LaneDims, clips: readonly ClipPlace[], length: number, count: number, handle: ClipPlace): void {
  for (let k = 1; k <= count; k++) {
    for (const c of clips) {
      const box = clipBox(v, d, c.row, c.start + k * length, c.end + k * length);
      const x0 = Math.max(box.x0, -d.hair);
      const x1 = Math.min(box.x1, v.width + d.hair);
      if (x1 <= x0) continue;
      out.push({ x: x0, y: box.y0, w: x1 - x0, h: box.y1 - box.y0, color: "color.gray.2" });
      paintFrame(out, v, d, box, "color.gray.7");
    }
  }
  const box = clipBox(v, d, handle.row, handle.start, handle.end + count * length);
  if (box.x1 > 0 && box.x1 - d.mark < v.width) out.push({ x: box.x1 - d.mark, y: box.y0, w: d.mark, h: box.y1 - box.y0, color: "color.accent.ink" });
}

/**
 * Take notes (positions from tick 0 of the take) to positions in the song. Only notes that sound (start inside the clip range [start, end)).
 * Notes trimmed outside are not drawn (paintClip cuts ends that stick out).
 */
export function takeShapes(
  clip: Pick<ClipView, "start" | "end" | "takeOffset">,
  take: {
    starts: ArrayLike<number>;
    lengths: ArrayLike<number>;
    keys: ArrayLike<number>;
  },
): NoteShape[] {
  const at = clip.start + clip.takeOffset;
  const out: NoteShape[] = [];
  for (let i = 0; i < take.starts.length; i++) {
    const start = at + take.starts[i]!;
    const length = take.lengths[i]!;
    if (start >= clip.end || start < clip.start) continue;
    out.push({ start, length, key: take.keys[i]! });
  }
  return out;
}

/** Recording notes to shapes. Held keys (length null) go to `now`. Notes that ended before `from` are dropped. */
export function liveShapes(notes: readonly LiveNote[], now: number, from = -Infinity): NoteShape[] {
  const out: NoteShape[] = [];
  for (const n of notes) {
    const length = n.length ?? Math.max(0, now - n.start);
    if (n.start + length < from) continue;
    out.push({ start: n.start, length, key: n.key });
  }
  return out;
}

/** The tag before the name: none, 「2/3」, or only a square when there is not enough width. */
export type TakeMark = "none" | "tag" | "square";

export function takeMark(clip: Pick<ClipView, "takes">, width: number, minWidth: number): TakeMark {
  if (clip.takes < 2) return "none";
  return width < minWidth ? "square" : "tag";
}

/** Page by one page: move in steps of `width` so that position `x` (px from the song start) is visible. */
export function followScroll(x: number, scrollX: number, width: number): number {
  if (!(width > 0)) return scrollX;
  if (x >= scrollX && x < scrollX + width) return scrollX;
  return Math.max(0, scrollX + Math.floor((x - scrollX) / width) * width);
}

/** Bar line numbers (from 1). The 5 and 9 of 「5–9」. */
export function barLineAt(v: View, x: number, perBar: BarGrid): number {
  return lineNear(perBar, xToTick(v, x));
}

export interface BarRange {
  from: number;
  to: number;
}

/** A loop range: bar line numbers (the 5 and 9 of 「5–9」) and the ticks where it is drawn (`start`, `end`). */
export interface LoopRange extends BarRange {
  start: number;
  end: number;
}

/**
 * The loop range (ticks) as it is: drawn at its ticks, numbered as core numbers it in the status line (`Play::bars`):
 * the bar that holds the start, and the bar line at the end or after it.
 */
export function loopBars(range: TickSpan | null, perBar: BarGrid): LoopRange | null {
  if (!range) return null;
  const start = Math.max(0, range.start);
  const end = Math.max(0, range.end);
  if (perBar instanceof TempoMap) {
    const s = perBar.meterAt(start);
    const e = perBar.meterAt(end);
    return { from: s.bar + 1, to: (e.barTick === end ? e.bar : e.bar + 1) + 1, start: range.start, end: range.end };
  }
  return { from: Math.floor(start / perBar) + 1, to: Math.ceil(end / perBar) + 1, start: range.start, end: range.end };
}

/** A range of bar lines (a ruler drag), drawn at those bar lines. */
export const barLoop = (r: BarRange, perBar: BarGrid): LoopRange => ({ ...r, start: lineTick(perBar, r.from), end: lineTick(perBar, r.to) });

/** A ruler drag. range: a new range from the pressed line. edge: move one end of an existing range (fixed is the end that stays). */
export type RulerDrag = { kind: "range"; anchor: number } | { kind: "edge"; fixed: number };

/**
 * If the press is within `grab` px of a range end (at its ticks when known), grab that end.
 * The end that stays is sent as its bar line number (`SetLoop` takes bar lines): the outward one of `loopBars`.
 */
export function startRulerDrag(v: View, x: number, perBar: BarGrid, loop: (BarRange & Partial<LoopRange>) | null, grab: number): RulerDrag {
  if (loop) {
    const lineX = (bar: number, tick: number | undefined) => tickToX(v, tick ?? lineTick(perBar, bar));
    const dFrom = Math.abs(x - lineX(loop.from, loop.start));
    const dTo = Math.abs(x - lineX(loop.to, loop.end));
    if (Math.min(dFrom, dTo) <= grab) return { kind: "edge", fixed: dFrom <= dTo ? loop.to : loop.from };
  }
  return { kind: "range", anchor: barLineAt(v, x, perBar) };
}

/** The range while dragging. At least 1 bar. */
export function dragRange(drag: RulerDrag, v: View, x: number, perBar: BarGrid): BarRange {
  const fixed = drag.kind === "range" ? drag.anchor : drag.fixed;
  const at = barLineAt(v, x, perBar);
  if (at === fixed)
    return at > 1 && x < tickToX(v, lineTick(perBar, fixed))
      ? { from: at - 1, to: fixed }
      : { from: fixed, to: fixed + 1 };
  return { from: Math.min(at, fixed), to: Math.max(at, fixed) };
}

/**
 * The step of lines that a ruler press snaps to (docs/history/design-notes.ja.md B-61, the lines visible on the ruler). When zoomed out and numbers are thinned,
 * every numbered bar; usually 1 bar. When zoomed in so that a beat is minGap px or more (the same as the gap between numbers), beats.
 */
export function rulerStep(barPx: number, perBar: number, perBeat: number, labelEvery: number, minGap: number): number {
  return beatsShown(barPx, perBar, perBeat, minGap) ? perBeat : labelEvery * perBar;
}

/** Whether it is zoomed in enough for the ruler to snap to beats. Only then are beat lines drawn in the clip area too (B-61). */
export function beatsShown(barPx: number, perBar: number, perBeat: number, minGap: number): boolean {
  return (barPx * perBeat) / perBar >= minGap;
}

/**
 * The command on ruler release. If not moved, go to the pressed place (snap to the near line in steps of step; if step is a function, snap with it.
 * With Shift, step is null and there is no snap).
 */
export function rulerCommand(
  drag: RulerDrag,
  v: View,
  downX: number,
  upX: number,
  perBar: BarGrid,
  still: number,
  step: number | ((tick: number) => number) | null,
): Command {
  if (Math.abs(upX - downX) < still) {
    const at = xToTick(v, downX);
    const snapped = typeof step === "function" ? step(at) : step ? Math.round(at / step) * step : Math.round(at);
    return { type: "locate", tick: Math.max(0, snapped) };
  }
  const r = dragRange(drag, v, upX, perBar);
  return { type: "setLoop", fromBar: r.from, toBar: r.to };
}

/** The clip at the pressed place (null if none). */
export function clipAt(v: View, d: LaneDims, tracks: readonly TrackView[], x: number, y: number): number | null {
  const row = trackRowAt(d, y);
  const track = tracks[row];
  if (!track) return null;
  for (const c of track.clips) {
    const b = clipBox(v, d, row, c.start, c.end);
    if (x >= b.x0 && x < b.x1 && y >= b.y0 && y < b.y1) return c.id;
  }
  return null;
}

/** The start of the current lap of a recording clip. */
export function lapStart(from: number, lap: number, loop: TickSpan | null): number {
  return lap > 0 && loop ? loop.start : from;
}

/**
 * How a recording clip is split. [lapFrom, commit) is on the big face (arrangeLanes); [tailFrom, now] is on the small face (recordingClip).
 * commit moves bar by bar. tailFrom starts 1 bar before commit, so there is no gap even in the one frame before the big face finishes drawing.
 */
export function recordingSplit(lapFrom: number, now: number, perBar: number): { commit: number; tailFrom: number } {
  const done = Math.max(0, Math.floor((now - lapFrom) / perBar));
  const commit = lapFrom + done * perBar;
  return { commit, tailFrom: Math.max(lapFrom, commit - perBar) };
}

/** The clip lines per track in the overview. Stretched vertically if taller than 40 (size.overview). */
export function overviewRects(
  v: View,
  tracks: readonly TrackView[],
  height: number,
  base: { overviewH: number; lineH: number; gap: number; hair: number },
  recording: { track: number; from: number; now: number } | null,
): GlRect[] {
  const n = tracks.length;
  if (n === 0 || !(height > 0)) return [];
  const stretch = Math.max(1, height / base.overviewH);
  let stride = (base.lineH + base.gap) * stretch;
  let lineH = base.lineH * stretch;
  if (stride * n > height) {
    stride = height / n;
    lineH = Math.max(base.hair, stride - base.gap);
  }
  const top = (height - (stride * n - (stride - lineH))) / 2;
  const out: GlRect[] = [];
  tracks.forEach((track, i) => {
    const y = top + i * stride;
    const line = (start: number, end: number, color: ColorName) => {
      const x0 = Math.max(tickToX(v, start), 0);
      const x1 = Math.min(tickToX(v, end) - base.gap, v.width);
      if (x1 > x0) out.push({ x: x0, y, w: x1 - x0, h: lineH, color });
    };
    for (const c of track.clips) line(c.start, c.end, trackColor(track));
    if (recording && recording.track === track.id && recording.now > recording.from) {
      line(recording.from, recording.now, "color.rec");
    }
  });
  return out;
}

/** The scroll (px) after a horizontal wheel move. Right up to `max` (or up to where it is, if paging already went past). */
export function wheelScroll(scrollX: number, dx: number, max: number): number {
  return Math.max(0, Math.min(scrollX + dx, Math.max(max, scrollX)));
}

/** The right end of horizontal scrolling: until the last thing in the song comes to the middle of the screen. */
export function scrollMax(
  tracks: readonly TrackView[],
  loop: TickSpan | null,
  position: number,
  v: Omit<View, "scrollX">,
): number {
  let end = Math.max(position, loop?.end ?? 0);
  for (const t of tracks) for (const c of t.clips) end = Math.max(end, c.end);
  return Math.max(0, end * v.pxPerTick - v.width / 2);
}

// ---- Horizontal zoom in and out, and the "visible range frame" of the overview (spec 3 "Rules for round 3") ----

/** Every how many bars the ruler shows a number (1, 2, 4, 8 …), so that numbers are not closer than minGap px. */
export function labelEvery(barPx: number, minGap: number): number {
  let every = 1;
  while (every * barPx < minGap && every < 1024) every *= 2;
  return every;
}

/** One step of Mod + wheel (WheelEvent px), so that one notch of a normal mouse is one step. */
export const WHEEL_ZOOM_STEP = 50;

/**
 * Mod + wheel to steps. Turning up (deltaY < 0) zooms in (+1). One event moves at most one step.
 * Line or page units (deltaMode 1, 2) are one step per event. Pixel units (fine touchpad moves) are one step each time WHEEL_ZOOM_STEP adds up.
 */
export function wheelZoomSteps(acc: number, deltaY: number, deltaMode: number): { steps: number; acc: number } {
  if (deltaY === 0) return { steps: 0, acc };
  if (deltaMode !== 0) return { steps: deltaY < 0 ? 1 : -1, acc: 0 };
  const next = Math.sign(acc) === Math.sign(deltaY) ? acc + deltaY : deltaY;
  if (Math.abs(next) < WHEEL_ZOOM_STEP) return { steps: 0, acc: next };
  return { steps: next < 0 ? 1 : -1, acc: 0 };
}

/**
 * The "visible range frame" of the overview (1px accent; inset at top and bottom). The range is ticks in the song.
 * Lines align with screen pixels. Edges outside the visible width are not drawn.
 */
export function rangeFrameRects(
  v: View,
  range: { start: number; end: number },
  height: number,
  o: { inset: number; hair: number; dpr: number },
): GlRect[] {
  const snap = (x: number) => Math.round(x * o.dpr) / o.dpr;
  const x0 = snap(tickToX(v, range.start));
  const x1 = snap(tickToX(v, range.end));
  const y0 = o.inset;
  const y1 = height - o.inset;
  const l = Math.max(x0, 0);
  const r = Math.min(x1, v.width);
  if (r <= l || y1 - y0 < 2 * o.hair) return [];
  const color: ColorName = "color.accent.ink";
  const out: GlRect[] = [
    { x: l, y: y0, w: r - l, h: o.hair, color },
    { x: l, y: y1 - o.hair, w: r - l, h: o.hair, color },
  ];
  if (x0 >= 0) out.push({ x: x0, y: y0, w: o.hair, h: y1 - y0, color });
  if (x1 <= v.width) out.push({ x: x1 - o.hair, y: y0, w: o.hair, h: y1 - y0, color });
  return out;
}

/**
 * If the frame [startPx, endPx) (px from the song start) does not fit in the overview, the scrollX after paging. If it fits, unchanged.
 * When the frame is wider than the overview, put its left edge at the overview's left edge. Otherwise, center it.
 */
export function revealRange(startPx: number, endPx: number, scrollX: number, width: number): number {
  if (!(width > 0)) return scrollX;
  if (startPx >= scrollX && endPx <= scrollX + width) return scrollX;
  const span = endPx - startPx;
  return Math.max(0, span >= width ? startPx : startPx - (width - span) / 2);
}
