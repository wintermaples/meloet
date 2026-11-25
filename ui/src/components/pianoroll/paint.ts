// WebGL pictures of the piano roll (lists of rectangles). Does not know the DOM. Coordinates are screen pixels of the region's canvas (box is the slot's box).
// Notes follow spec 2 "Note": height 11, a 2px gray.10 at the left end, an accent face as strong as the velocity with an accent.ink frame, gray.10 when selected,
// the position before aligning only as a gray.7 border (accent for selected notes), rows that cannot be written row.out.
// Rows, keys and the grid lines have colors of their own (spec 1, user:2026-11-23). Bar lines are BAR_LINE wide, the other lines HAIR.
// The original position while dragging is an accent border. Both borders are drawn after the notes so notes do not hide them (docs/history/design-notes.ja.md B-64).

import type { ControlKind, LiveNote, MusicalScale } from "../../bridge";
import type { ColorName } from "../../design/tokens";
import type { Batch } from "../../gl/batch";
import { rgb, type Rgb } from "../../gl/color";
import { isBlack } from "../../gl/keyboard";
import { BAR_LINE, HAIR, type Box } from "../../gl/paint";
import { barsMap, eachGridTick, eachLine } from "../../time/bars";
import type { TempoMap } from "../../time/tempoMap";
import type { Meter } from "../time";
import { draggedPoint, firstPointFrom, isOnOff, lanePoints, pointTick, valueToY, type LaneBox, type LaneDrag } from "./cc";
import type { Drag } from "./drag";
import { draggedVelocity } from "./drag";
import { eachNoteIn, keyToY, movedFrom, noteEnd, noteStart, rowColor, rowsShown, velocityStrength, type RollGeom, type Shown } from "./geom";
import { paintGhosts, type GhostPicture } from "./ghosts";

/** Sizes (CSS px, with the scale applied). */
export interface RollSizes {
  noteH: number;
  edge: number;
  velBar: number;
  velCap: number;
  velFull: number;
  velMid: number;
  endMark: number;
  /** The difference between a drum square and the row height (part.drum.hit-gap). */
  drumGap: number;
}

/** What the large pictures of the note area and the velocity lane use. Call the slot's update when it changes. */
export interface RollPicture {
  g: RollGeom;
  sizes: RollSizes;
  scale: MusicalScale;
  meter: Meter;
  step: number;
  /** Bar lines in songs with time signature changes (if not given, bars of meter in a row). */
  map?: TempoMap;
  /** The grid is one bar (the length of that bar; it changes with time signature changes). */
  barStep?: boolean;
  /** The shown take. Null while recording when what is being recorded is shown. */
  shown: Shown | null;
  /** The original take of a quantized take (read with the same base). */
  origin: Shown | null;
  /** Marks of the selected notes (indexes in shown). */
  selected: Uint8Array | null;
  /** Notes being recorded (positions in the song). Those whose length is null (held) are only bars here. */
  live: readonly LiveNote[] | null;
  drag: Drag | null;
  eraseHover: number;
  /** Notes of other tracks (「ほかのトラックも見る」, ghosts.ts). Not drawn on drum rows. */
  ghosts?: GhostPicture | null;
}

const FRAME: ColorName = "color.gray.7";

/** The grid lines by kind: 0 bar, 1 beat, 2 other. */
const GRID = ["color.line.bar", "color.line.beat", "color.line.grid"] as const satisfies readonly ColorName[];
/** The width of the grid lines by kind (device pixels): the left pixel is on the line's place, the rest to its right. */
const GRID_W = [BAR_LINE, HAIR, HAIR] as const;

/** When lines get closer than this, use the next coarser lines (CSS px). */
const LINE_MIN_PX = 4;

function lineStep(step: number, perBeat: number, perBar: number, ppt: number): number {
  if (step * ppt >= LINE_MIN_PX) return step;
  if (perBeat * ppt >= LINE_MIN_PX) return perBeat;
  return perBar * Math.max(1, Math.ceil(LINE_MIN_PX / (perBar * ppt)));
}

/** Grid lines counted again from the start of each bar (so the start of a bar is a line even in songs with time signature changes). kind is 0 bar, 1 beat, 2 other. */
function eachGridLine(
  p: { meter: Meter; step: number; map?: TempoMap; barStep?: boolean },
  ppt: number,
  from: number,
  to: number,
  each: (tick: number, kind: 0 | 1 | 2) => void,
): void {
  const map = p.map ?? barsMap(p.meter);
  // If wide enough to see grid lines, the same lines as note snapping (counted from the start of the time signature span). Otherwise beat or bar lines.
  if (!p.barStep && p.step * ppt >= LINE_MIN_PX) eachGridTick(map, from, to, p.step, each);
  else eachLine(map, from, to, (perBeat, perBar) => lineStep(p.barStep ? perBar : p.step, perBeat, perBar, ppt), each);
}

/**
 * Note marks: 0 normal, 1 being moved (accent border at the original place), 2 end being moved (a line at the original end), 3 inside a box (accent),
 * 4 about to be removed (gray.7), 5 start being moved (a line at the original start).
 */
export function noteMarks(count: number, drag: Drag | null, eraseHover: number): Uint8Array | null {
  if (!drag && eraseHover < 0) return null;
  const m = new Uint8Array(count);
  if (eraseHover >= 0 && eraseHover < count) m[eraseHover] = 4;
  if (!drag) return m;
  switch (drag.kind) {
    case "move":
      if (drag.press.moved && !drag.copy) for (const i of drag.notes) m[i] = 1;
      break;
    case "resize":
      if (drag.press.moved) for (const i of drag.notes) m[i] = drag.edge === "start" ? 5 : 2;
      break;
    case "box":
      for (const i of drag.hits) m[i] = 3;
      break;
    case "erase":
      for (const i of drag.hits) m[i] = 4;
      break;
    default:
  }
  return m;
}

function frame(out: Batch, x0: number, y0: number, x1: number, y1: number, color: ColorName): void {
  const c = rgb(color);
  out.rect(x0, y0, x1, y0 + HAIR, c);
  out.rect(x0, y1 - HAIR, x1, y1, c);
  out.rect(x0, y0, x0 + HAIR, y1, c);
  out.rect(x1 - HAIR, y0, x1, y1, c);
}

/**
 * How a note or a drum square is painted (spec 2 "Note"): a velocity is an ordinary note (accent, as strong as the velocity,
 * over gray.1, with an accent.ink frame), "selected" is gray.10 with an accent.ink frame, a color name is a flat face.
 */
export type NoteLook = number | "selected" | ColorName;

function lookFill(look: NoteLook): Rgb {
  if (look === "selected") return rgb("color.gray.10");
  if (typeof look !== "number") return rgb(look);
  const t = velocityStrength(look);
  const a = rgb("color.accent");
  const b = rgb("color.gray.1");
  return [b[0] + (a[0] - b[0]) * t, b[1] + (a[1] - b[1]) * t, b[2] + (a[2] - b[2]) * t];
}

/** A face with, for ordinary and selected notes, a line.hair accent.ink frame inside its edge. */
function face(out: Batch, x0: number, y0: number, x1: number, y1: number, look: NoteLook): void {
  out.rect(x0, y0, x1, y1, lookFill(look));
  if (typeof look === "number" || look === "selected") frame(out, x0, y0, x1, y1, "color.accent.ink");
}

/** One note. x0 and x1 are the positions of the start and end lines (pixels). Drawn leaving the line's 1px free. */
export function paintNote(out: Batch, x0: number, x1: number, y: number, h: number, edge: number, look: NoteLook): void {
  const a = Math.round(x0) + HAIR;
  const b = Math.max(a + HAIR, Math.round(x1) - HAIR);
  const e = Math.min(b, a + edge);
  face(out, a, y, b, y + h, look);
  out.rect(a, y, e, y + h, rgb("color.gray.10"));
}

/** A drum square (spec 2 "Drum hit square"): from 1px right of the start line, width hitW, height row − gap in the middle of the row. */
function drumSquare(g: RollGeom, sizes: RollSizes, x0: number, rowTop: number, dpr: number): { x0: number; y0: number; x1: number; y1: number } {
  const rowH = g.rowH * dpr;
  const h = Math.max(HAIR, Math.round(rowH - sizes.drumGap * dpr));
  const y0 = Math.round(rowTop + (rowH - h) / 2);
  const a = Math.round(x0) + HAIR;
  return { x0: a, y0, x1: a + Math.max(HAIR, Math.round((g.drum?.hitW ?? 0) * dpr)), y1: y0 + h };
}

/** The drum note area: row backgrounds (row.white and row.black in turns, with gray.1 lines between), grid lines, hit squares. */
function paintDrumRoll(out: Batch, box: Box, p: RollPicture, marks: Uint8Array | null, dpr: number): void {
  const { g, sizes } = p;
  const d = g.drum!;
  const rowH = g.rowH * dpr;
  const ppt = g.ppt * dpr;
  const X = (tick: number) => box.x0 + (tick - g.scrollTick) * ppt;
  const rowTop = (key: number) => Math.round(box.y0 + (d.keys.indexOf(key) - d.top) * rowH);

  out.rect(box.x0, box.y0, box.x1, box.y1, rgb("color.gray.1"));
  const rows = Math.min(d.keys.length - d.top, rowsShown(g) + 1);
  for (let r = 0; r < rows; r++) {
    const y = box.y0 + r * rowH;
    out.rect(box.x0, y, box.x1, y + rowH - HAIR, rgb((d.top + r) % 2 === 0 ? "color.row.white" : "color.row.black"));
  }
  const bottom = box.y0 + rows * rowH;
  gridLines(out, box, p, X, Math.min(box.y1, bottom));

  const hitTicks = d.hitW / g.ppt;
  const from = g.scrollTick - hitTicks;
  const to = g.scrollTick + g.width / g.ppt;
  const square = (start: number, key: number) => drumSquare(g, sizes, X(start), rowTop(key), dpr);
  const n = p.shown;
  const sel = p.selected;
  const moving: number[] = [];
  const shownKey = (key: number) => d.keys.includes(key);
  if (n) {
    eachNoteIn(n, from, to, (i) => {
      const key = n.take.keys[i]!;
      if (!shownKey(key)) return;
      const mark = marks ? marks[i]! : 0;
      if (mark === 1) {
        moving.push(i);
        return;
      }
      const q = square(noteStart(n, i), key);
      if (q.y0 > box.y1 || q.y1 < box.y0) return;
      const look: NoteLook = mark === 4 ? FRAME : mark === 3 || (sel && sel[i]) ? "selected" : n.take.velocities[i]!;
      face(out, q.x0, q.y0, q.x1, q.y1, look);
    });
  }
  if (n && p.origin) {
    const o = p.origin;
    eachNoteIn(n, from - o.maxLength - n.maxLength, to + o.maxLength, (i) => {
      const j = movedFrom(n.take, o.take, i);
      if (j < 0 || !shownKey(o.take.keys[j]!)) return;
      const q = square(noteStart(o, j), o.take.keys[j]!);
      if (q.x1 < box.x0 || q.x0 > box.x1) return;
      frame(out, q.x0, q.y0, q.x1, q.y1, sel && sel[i] ? "color.accent.ink" : FRAME);
    });
  }
  if (n) {
    for (const i of moving) {
      const q = square(noteStart(n, i), n.take.keys[i]!);
      frame(out, q.x0, q.y0, q.x1, q.y1, "color.accent.ink");
    }
  }
  if (p.live) {
    for (const l of p.live) {
      if (l.length === null || !shownKey(l.key)) continue;
      const q = square(l.start, l.key);
      if (q.x1 < box.x0 || q.x0 > box.x1) continue;
      face(out, q.x0, q.y0, q.x1, q.y1, l.velocity);
    }
  }
}

/** Vertical grid lines (bar line.bar, beat line.beat, other line.grid), from the top edge to bottom. */
function gridLines(out: Batch, box: Box, p: RollPicture, X: (tick: number) => number, bottom: number): void {
  const { g } = p;
  const from = g.scrollTick;
  const to = g.scrollTick + g.width / g.ppt;
  eachGridLine(p, g.ppt, from, to, (tick, kind) => {
    const x = Math.round(X(tick));
    out.rect(x, box.y0, x + GRID_W[kind], bottom, rgb(GRID[kind]));
  });
}

/** The note area: row backgrounds, grid lines, notes of other tracks, notes, the border of the position before aligning, the border of the original drag position. */
export function paintRoll(out: Batch, box: Box, p: RollPicture, marks: Uint8Array | null, dpr: number): void {
  if (p.g.drum) {
    paintDrumRoll(out, box, p, marks, dpr);
    return;
  }
  const { g, sizes } = p;
  const rowH = g.rowH * dpr;
  const ppt = g.ppt * dpr;
  const X = (tick: number) => box.x0 + (tick - g.scrollTick) * ppt;
  const Y = (key: number) => box.y0 + (g.topKey - key) * rowH;
  const noteH = Math.round(sizes.noteH * dpr);
  const edge = Math.round(sizes.edge * dpr);

  out.rect(box.x0, box.y0, box.x1, box.y1, rgb("color.gray.1"));
  const rows = rowsShown(g) + 1;
  for (let r = 0; r < rows; r++) {
    const key = g.topKey - r;
    if (key < 0) break;
    const y = Y(key);
    out.rect(box.x0, y, box.x1, y + rowH - HAIR, rgb(rowColor(key, p.scale)));
  }

  const from = g.scrollTick;
  const to = g.scrollTick + g.width / g.ppt;
  gridLines(out, box, p, X, box.y1);
  if (p.ghosts) paintGhosts(out, box, g, p.ghosts, noteH, dpr);

  const rowTop = (key: number) => Math.round(Y(key));
  const noteFrame = (x0: number, x1: number, y: number, color: ColorName) =>
    frame(out, Math.round(x0) + HAIR, y, Math.max(Math.round(x0) + 2 * HAIR, Math.round(x1) - HAIR), y + noteH, color);
  const n = p.shown;
  const sel = p.selected;
  /** Notes moved by dragging (an accent border is drawn later at the original place). */
  const moving: number[] = [];

  if (n) {
    eachNoteIn(n, from, to, (i) => {
      const mark = marks ? marks[i]! : 0;
      const key = n.take.keys[i]!;
      const y = rowTop(key);
      if (y > box.y1 || y + rowH < box.y0) return;
      const x0 = X(noteStart(n, i));
      const x1 = X(noteEnd(n, i));
      if (mark === 1) {
        moving.push(i);
        return;
      }
      if (mark === 2 || mark === 5) {
        const h = Math.round(sizes.endMark * dpr);
        const top = Math.round(y + rowH / 2 - h / 2);
        const x = Math.round(mark === 2 ? x1 : x0);
        out.rect(x, top, x + HAIR, top + h, rgb(FRAME));
        return;
      }
      const look: NoteLook = mark === 4 ? FRAME : mark === 3 || (sel && sel[i]) ? "selected" : n.take.velocities[i]!;
      paintNote(out, x0, x1, y, noteH, edge, look);
    });
  }

  if (n && p.origin) {
    const o = p.origin;
    eachNoteIn(n, from - o.maxLength - n.maxLength, to + o.maxLength, (i) => {
      const j = movedFrom(n.take, o.take, i);
      if (j < 0) return;
      const x0 = X(noteStart(o, j));
      const x1 = X(noteEnd(o, j));
      if (x1 < box.x0 || x0 > box.x1) return;
      noteFrame(x0, x1, rowTop(o.take.keys[j]!), sel && sel[i] ? "color.accent.ink" : FRAME);
    });
  }

  if (n) for (const i of moving) noteFrame(X(noteStart(n, i)), X(noteEnd(n, i)), rowTop(n.take.keys[i]!), "color.accent.ink");

  if (p.live) {
    for (const l of p.live) {
      if (l.length === null) continue;
      const x0 = X(l.start);
      const x1 = X(l.start + l.length);
      if (x1 < box.x0 || x0 > box.x1) continue;
      paintNote(out, x0, x1, rowTop(l.key), noteH, edge, l.velocity);
    }
  }
}

/** The velocity lane: the middle line, and a bar per note (with a cap line at the top). box is below the lane's top line. */
export function paintVelocityLane(out: Batch, box: Box, p: RollPicture, dpr: number): void {
  const { g, sizes } = p;
  const ppt = g.ppt * dpr;
  const X = (tick: number) => box.x0 + (tick - g.scrollTick) * ppt;
  const mid = box.y0 + Math.round(sizes.velMid * dpr);
  out.rect(box.x0, mid, box.x1, mid + HAIR, rgb("color.gray.4"));
  const barW = Math.round(sizes.velBar * dpr);
  const capW = Math.round(sizes.velCap * dpr);
  const capOff = Math.floor((capW - barW) / 2);
  const full = sizes.velFull * dpr;
  const from = g.scrollTick - 1;
  const to = g.scrollTick + (g.width + sizes.velCap) / g.ppt;
  const bar = (tick: number, v: number, color: ColorName) => {
    const x = Math.round(X(tick)) + HAIR;
    const h = Math.max(HAIR, Math.round((v * full) / 127));
    const top = box.y1 - h;
    out.rect(x, top, x + barW, box.y1, rgb(color));
    out.rect(x - capOff, top, x - capOff + capW, top + HAIR, rgb(color));
  };
  const cap = (tick: number, v: number) => {
    const x = Math.round(X(tick)) + HAIR;
    const top = box.y1 - Math.max(HAIR, Math.round((v * full) / 127));
    out.rect(x - capOff, top, x - capOff + capW, top + HAIR, rgb(FRAME));
  };

  // On drums, notes on keys the kit has no row for are not shown (spec 3), so they get no bar either.
  const onRow = (key: number) => !g.drum || g.drum.keys.includes(key);
  const n = p.shown;
  if (n) {
    const d = p.drag?.kind === "velocity" ? p.drag : null;
    const dragged = d ? new Set(d.notes) : null;
    const sel = p.selected;
    eachNoteIn(n, from, to, (i) => {
      const s = noteStart(n, i);
      if (s < from || s >= to || !onRow(n.take.keys[i]!)) return;
      const v = n.take.velocities[i]!;
      const color: ColorName = sel && sel[i] ? "color.gray.10" : "color.accent.ink";
      if (d && dragged!.has(i)) {
        bar(s, draggedVelocity(v, d.delta), color);
        cap(s, v);
      } else {
        bar(s, v, color);
      }
    });
  }
  if (p.live) {
    for (const l of p.live) {
      if (l.start < from || l.start >= to || !onRow(l.key)) continue;
      bar(l.start, l.velocity, l.length === null ? "color.rec" : "color.accent.ink");
    }
  }
}

/** The key part of the keyboard column (right of the note name field). White keys key.white, black keys key.black, lit keys accent with an accent.ink mark on the left, a gray.1 line below. */
export function paintRollKeys(
  out: Batch,
  box: Box,
  g: RollGeom,
  lit: (key: number) => boolean,
  dpr: number,
  mark: { width: number; color: ColorName } = { width: 2, color: "color.accent.ink" },
): void {
  const rowH = g.rowH * dpr;
  out.rect(box.x0, box.y0, box.x1, box.y1, rgb("color.gray.1"));
  if (g.drum) {
    // The background of drum row names (mock R7-Drum: gray.5, a gray.1 line below). The names are HTML.
    // Sounding rows and played rows get a mark-width mark on the left (spec 3 (85), mock R8-DrumPlay).
    const rows = Math.min(g.drum.keys.length - g.drum.top, rowsShown(g) + 1);
    const markW = Math.max(HAIR, Math.round(mark.width * dpr));
    for (let r = 0; r < rows; r++) {
      const y = box.y0 + r * rowH;
      out.rect(box.x0, y, box.x1, y + rowH - HAIR, rgb("color.gray.5"));
      if (lit(g.drum.keys[g.drum.top + r]!)) out.rect(box.x0, y, box.x0 + markW, y + rowH - HAIR, rgb(mark.color));
    }
    return;
  }
  const rows = rowsShown(g) + 1;
  const markW = Math.max(HAIR, Math.round(mark.width * dpr));
  for (let r = 0; r < rows; r++) {
    const key = g.topKey - r;
    if (key < 0) break;
    const y = box.y0 + r * rowH;
    const color: ColorName = lit(key) ? "color.accent" : isBlack(key) ? "color.key.black" : "color.key.white";
    out.rect(box.x0, y, box.x1, y + rowH - HAIR, rgb(color));
    if (lit(key)) out.rect(box.x0, y, box.x0 + markW, y + rowH - HAIR, rgb(mark.color));
  }
}

/** The content of the moving surface. Notes are placed with the surface box's top left as (left, top) (CSS px of the note area). */
export interface LivePicture {
  g: RollGeom;
  sizes: RollSizes;
  /** The surface box's top left before moving (CSS px of the note area) */
  left: number;
  top: number;
  /** How far the surface box is moved vertically (CSS px), and the height of the note area. Notes do not go into the lane below. */
  ty: number;
  areaH: number;
  /** outline: only a line.hair frame in `face` where the note would be (the chord shadow). */
  notes: readonly { start: number; end: number; key: number; face: NoteLook; outline?: boolean }[];
}

export function paintLive(out: Batch, box: Box, p: LivePicture, dpr: number): void {
  const { g, sizes } = p;
  const noteH = Math.round(sizes.noteH * dpr);
  const edge = Math.round(sizes.edge * dpr);
  const ox = box.x0 - p.left * dpr;
  const oy = box.y0 - p.top * dpr;
  const areaTop = oy - p.ty * dpr;
  out.clip(box.x0, Math.max(box.y0, Math.round(areaTop)), box.x1, Math.min(box.y1, Math.round(areaTop + p.areaH * dpr)));
  for (const n of p.notes) {
    const top = keyToY(g, n.key);
    if (Number.isNaN(top)) continue;
    const x0 = ox + (n.start - g.scrollTick) * g.ppt * dpr;
    const x1 = ox + (n.end - g.scrollTick) * g.ppt * dpr;
    const y = Math.round(oy + top * dpr);
    if (g.drum) {
      const q = drumSquare(g, sizes, x0, y, dpr);
      if (n.outline) frame(out, q.x0, q.y0, q.x1, q.y1, "color.accent.ink");
      else face(out, q.x0, q.y0, q.x1, q.y1, n.face);
    } else if (n.outline) {
      const a = Math.round(x0) + HAIR;
      frame(out, a, y, Math.max(a + 2 * HAIR, Math.round(x1) - HAIR), y + noteH, "color.accent.ink");
    } else {
      paintNote(out, x0, x1, y, noteH, edge, n.face);
    }
  }
}

// ---- CC lanes (mock R3-Lanes) ----

/** What the picture of one CC lane uses. */
export interface LanePicture {
  g: RollGeom;
  meter: Meter;
  step: number;
  map?: TempoMap;
  barStep?: boolean;
  /** null is a lane whose 「そのほかの CC」 number is not chosen yet. */
  control: ControlKind | null;
  shown: Shown | null;
  lane: LaneBox;
  /** part.cc.point (CSS px) */
  point: number;
  /** Marks of the selected points (indexes among the take's controls). */
  selected: Uint8Array | null;
  /** This lane's drag. */
  drag: LaneDrag | null;
  eraseHover: number;
}

const LINE: ColorName = "color.gray.9";

/** The lines joining the points of one row of points (pixels). step: on/off CCs are steps (the value is kept until the next point); others go from point to point. */
function laneLine(out: Batch, xs: readonly number[], ys: readonly number[], x0: number, x1: number, restY: number, step: boolean): void {
  const c = rgb(LINE);
  const flat = (a: number, b: number, y: number) => out.rect(Math.min(a, b), y, Math.max(a, b) + HAIR, y + HAIR, c);
  const upright = (x: number, a: number, b: number) => out.rect(x, Math.min(a, b), x + HAIR, Math.max(a, b) + HAIR, c);
  let px = x0;
  let py = restY;
  for (let k = 0; k < xs.length; k++) {
    const x = xs[k]!;
    const y = ys[k]!;
    if (k === 0 || step) {
      flat(px, x, py);
      upright(x, py, y);
    } else {
      sloped(out, px, py, x, y, c);
    }
    px = x;
    py = y;
  }
  flat(px, x1, py);
}

/** A slanted line as a row of 1-pixel rectangles, joining spans of the same height. */
function sloped(out: Batch, xa: number, ya: number, xb: number, yb: number, c: ReturnType<typeof rgb>): void {
  if (xb <= xa) {
    out.rect(xa, Math.min(ya, yb), xa + HAIR, Math.max(ya, yb) + HAIR, c);
    return;
  }
  const at = (x: number) => Math.round(ya + ((yb - ya) * (x - xa)) / (xb - xa));
  let runX = xa;
  let runY = at(xa);
  for (let x = xa + 1; x <= xb; x++) {
    const y = at(x);
    if (y === runY) continue;
    // To the next height, join vertically so there is no gap.
    out.rect(runX, runY, x, runY + HAIR, c);
    out.rect(x, Math.min(runY, y), x + HAIR, Math.max(runY, y) + HAIR, c);
    runX = x;
    runY = y;
  }
  out.rect(runX, runY, xb + HAIR, runY + HAIR, c);
}

function laneSquare(out: Batch, cx: number, cy: number, size: number, color: ColorName, hollow: boolean): void {
  const x0 = Math.round(cx - size / 2);
  const y0 = Math.round(cy - size / 2);
  if (hollow) frame(out, x0, y0, x0 + size, y0 + size, color);
  else out.rect(x0, y0, x0 + size, y0 + size, rgb(color));
}

/** Lane background and lines: gray.1 background, the grid lines of the note area, and for pitch bend a gray.5 line at 0. */
function laneGround(out: Batch, box: Box, p: LanePicture, dpr: number): void {
  const { g } = p;
  out.rect(box.x0, box.y0, box.x1, box.y1, rgb("color.gray.1"));
  const to = g.scrollTick + g.width / g.ppt;
  eachGridLine(p, g.ppt, g.scrollTick, to, (tick, kind) => {
    const x = Math.round(box.x0 + (tick - g.scrollTick) * g.ppt * dpr);
    out.rect(x, box.y0, x + GRID_W[kind], box.y1, rgb(GRID[kind]));
  });
  if (p.control?.kind === "pitchBend") {
    const y = Math.round(box.y0 + valueToY(p.control, p.lane, 0) * dpr);
    out.rect(box.x0, y, box.x1, y + HAIR, rgb("color.gray.5"));
  }
}

/** Lines and points. Points in moving are drawn moved by the drag amount. */
function laneDots(out: Batch, box: Box, p: LanePicture, dpr: number, moving: Set<number> | null, face: (i: number) => ColorName): void {
  const n = p.shown;
  const c = p.control;
  if (!n || !c) return;
  const { g } = p;
  const all = lanePoints(n.take, c);
  const X = (tick: number) => Math.round(box.x0 + (tick - g.scrollTick) * g.ppt * dpr);
  const Y = (v: number) => Math.round(box.y0 + valueToY(c, p.lane, v) * dpr);
  // The visible points, and one before and after (so the line joins from beyond the edges).
  const from = g.scrollTick;
  const to = g.scrollTick + g.width / g.ppt;
  let list: { tick: number; value: number; i: number }[];
  if (moving && p.drag?.kind === "move") {
    const sorted = Array.from(all, (i) => ({ ...draggedPoint(n, c, p.drag, moving, i), i })).sort((a, b) => a.tick - b.tick);
    let a = 0;
    while (a < sorted.length && sorted[a]!.tick < from) a++;
    let b = a;
    while (b < sorted.length && sorted[b]!.tick <= to) b++;
    list = sorted.slice(Math.max(0, a - 1), Math.min(sorted.length, b + 1));
  } else {
    const k0 = Math.max(0, firstPointFrom(n, all, from) - 1);
    list = [];
    for (let k = k0; k < all.length; k++) {
      const i = all[k]!;
      const tick = pointTick(n, i);
      list.push({ tick, value: n.take.controlValues[i]!, i });
      if (tick > to) break;
    }
  }
  const before = list.length > 0 && list[0]!.tick < from;
  const xs = list.map((q) => X(q.tick));
  const ys = list.map((q) => Y(q.value));
  const restY = before ? ys[0]! : Y(0);
  laneLine(out, xs, ys, box.x0, box.x1, restY, isOnOff(c));
  const size = Math.round(p.point * dpr);
  for (let k = 0; k < list.length; k++) laneSquare(out, xs[k]!, ys[k]!, size, face(list[k]!.i), false);
}

/** The large picture of a CC lane. While points are moved, points and lines are left to the moving surface, and only a gray.7 border is drawn at the original place. */
export function paintLane(out: Batch, box: Box, p: LanePicture, dpr: number): void {
  laneGround(out, box, p, dpr);
  const n = p.shown;
  const c = p.control;
  if (!n || !c) return;
  const d = p.drag;
  if (d?.kind === "move" && d.press.moved) {
    const size = Math.round(p.point * dpr);
    for (const i of d.points) {
      const x = box.x0 + (pointTick(n, i) - p.g.scrollTick) * p.g.ppt * dpr;
      laneSquare(out, x, box.y0 + valueToY(c, p.lane, n.take.controlValues[i]!) * dpr, size, FRAME, true);
    }
    return;
  }
  const hits = d?.kind === "box" ? new Set(d.hits) : d?.kind === "erase" ? d.hits : null;
  const erasing = d?.kind === "erase";
  const sel = p.selected;
  laneDots(out, box, p, dpr, null, (i) => {
    if (i === p.eraseHover || (erasing && hits!.has(i))) return FRAME;
    if ((hits && !erasing && hits.has(i)) || (sel && sel[i])) return "color.accent.ink";
    return "color.gray.10";
  });
}

/** The moving surface of a CC lane: lines and points while moving, and the point before the draw tool places it (accent border). box is one lane. */
export function paintLaneLive(out: Batch, box: Box, p: LanePicture, dpr: number): void {
  const n = p.shown;
  const c = p.control;
  const d = p.drag;
  if (!c || !d) return;
  if (d.kind === "add") {
    const x = box.x0 + (d.tick - p.g.scrollTick) * p.g.ppt * dpr;
    laneSquare(out, x, box.y0 + valueToY(c, p.lane, d.value) * dpr, Math.round(p.point * dpr), "color.accent.ink", true);
    return;
  }
  if (d.kind !== "move" || !d.press.moved || !n) return;
  const moving = new Set(d.points);
  const sel = p.selected;
  laneDots(out, box, p, dpr, moving, (i) => (moving.has(i) || (sel && sel[i]) ? "color.accent.ink" : "color.gray.10"));
}
