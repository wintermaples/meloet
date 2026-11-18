// What each kind of slot looks like, as rectangles. No WebGL and no DOM here.

import type { Batch } from "./batch";
import { rgb, type Rgb } from "./color";
import { ARRANGE_GRID, CLIP_MARK, KEY_FORMS, KEYBOARD, METER, METER_HOT_FROM, OVERVIEW, VELOCITY, type KeyForm } from "./dims";
import { keyboardLayout } from "./keyboard";
import type { GlRect } from "./slot";

/** A slot's box in device pixels of its canvas (whole numbers; it may reach past the canvas). `hair` is line.hair: always one device pixel. */
export interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}
export const HAIR = 1;

/** A small size at the app's scale (spec 1: multiply, round, .5 goes up), in device pixels. */
function smallPx(px: number, scale: number, dpr: number): number {
  return Math.max(1, Math.round(Math.round((px * scale) / 100) * dpr));
}

/** The black key of a form, in device pixels. `scale` is the app's scale in %. */
export function blackKeyPx(form: KeyForm, scale: number, dpr: number): { w: number; h: number } {
  const { w, h } = KEY_FORMS[form].black;
  return { w: smallPx(w, scale, dpr), h: smallPx(h, scale, dpr) };
}

/** The clip mark's width in device pixels. */
export function clipMarkPx(scale: number, dpr: number): number {
  return smallPx(CLIP_MARK, scale, dpr);
}

/** White keys share the box evenly; `black` is the size of a black key in device pixels. */
export function paintKeyboard(
  out: Batch,
  box: Box,
  black: { w: number; h: number },
  firstNote: number,
  lastNote: number,
  isDown: (note: number) => boolean,
  dim = false,
): void {
  const { whiteCount, keys } = keyboardLayout(firstNote, lastNote);
  if (whiteCount === 0) return;
  const { x0, y0, x1, y1 } = box;
  const lines = dim ? KEYBOARD.dim : KEYBOARD.lit;
  // Lines 0..whiteCount: the left edge, then the right edge of each white key.
  const cell = (x1 - x0 - HAIR) / whiteCount;
  const lineX = (i: number) => x0 + Math.round(i * cell);

  out.rect(x0, y0, x1, y1, rgb(lines.frame));
  for (const key of keys) {
    if (key.black) continue;
    const fill = isDown(key.note) ? KEYBOARD.down : KEYBOARD.whiteFill;
    out.rect(lineX(key.index) + HAIR, y0 + HAIR, lineX(key.index + 1), y1 - HAIR, rgb(fill));
  }

  // Centered on its one-pixel line; an even width has its extra pixel on the right.
  const before = Math.floor((black.w - HAIR) / 2);
  const bottom = y0 + black.h;
  for (const key of keys) {
    if (!key.black) continue;
    const left = lineX(key.index) - before;
    const right = left + black.w;
    out.rect(left, y0, right, bottom, rgb(lines.blackFrame));
    const fill = isDown(key.note) ? KEYBOARD.down : KEYBOARD.blackFill;
    out.rect(left + HAIR, y0 + HAIR, right - HAIR, bottom - HAIR, rgb(fill));
  }
}

export function paintVelocity(out: Batch, box: Box, velocity: number): void {
  const { x0, y0, x1, y1 } = box;
  out.rect(x0, y0, x1, y1, rgb(VELOCITY.ground));
  const part = Math.min(Math.max(velocity / VELOCITY.max, 0), 1);
  out.rect(x0, y0, x0 + (x1 - x0) * part, y1, rgb(VELOCITY.fill));
}

export interface BarLevel {
  /** 0..1 of the bar's length. */
  length: number;
  clipped: boolean;
}

/**
 * Two bars: left above right, each growing to the right; `vertical`: left beside right, each growing
 * upward. `gap` and `mark` (clipMarkPx) are device pixels.
 */
export function paintMeter(out: Batch, box: Box, gap: number, mark: number, left: BarLevel, right: BarLevel, vertical = false): void {
  const { x0, y0, x1, y1 } = box;
  const length = vertical ? y1 - y0 : x1 - x0;
  const thick = Math.max(1, Math.round(((vertical ? x1 - x0 : y1 - y0) - gap) / 2));
  const hotFrom = Math.round(length * METER_HOT_FROM);
  // `from`..`to` are measured along the bar from its 0 % end; `at` is the bar's first pixel across.
  const piece = (at: number, from: number, to: number, color: Rgb) => {
    if (vertical) out.rect(at, y1 - to, at + thick, y1 - from, color);
    else out.rect(x0 + from, at, x0 + to, at + thick, color);
  };
  const bar = (at: number, level: BarLevel) => {
    piece(at, 0, length, rgb(METER.ground));
    const end = Math.round(length * Math.min(Math.max(level.length, 0), 1));
    piece(at, 0, Math.min(end, hotFrom), rgb(METER.ok));
    piece(at, hotFrom, end, rgb(METER.hot));
    if (level.clipped) piece(at, length - mark, length, rgb(METER.clip));
  };
  bar(vertical ? x0 : y0, left);
  bar((vertical ? x1 : y1) - thick, right);
}

/** Bar lines, and `beats` lines to a bar when it is more than 1. `firstBarX` and `barPx` are device pixels and may have a fraction. */
export function paintArrangeGrid(out: Batch, box: Box, firstBarX: number, barPx: number, beats = 1): void {
  if (!(barPx >= 1)) return;
  const { x0, y0, x1, y1 } = box;
  const n = Math.max(1, Math.floor(beats));
  const step = barPx / n;
  const bar = rgb(n > 1 ? ARRANGE_GRID.bar : ARRANGE_GRID.line);
  const beat = rgb(ARRANGE_GRID.beat);
  const first = Math.max(0, Math.ceil(-firstBarX / step));
  for (let k = first; ; k++) {
    const x = x0 + Math.round(firstBarX + k * step);
    if (x >= x1) break;
    out.rect(x, y0, x + HAIR, y1, k % n === 0 ? bar : beat);
  }
}

/** `rects` are CSS px from the box's top left; `dpr` turns them into device pixels. */
export function paintRects(out: Batch, box: Box, rects: readonly GlRect[], dpr: number): void {
  for (const r of rects) {
    out.rect(box.x0 + r.x * dpr, box.y0 + r.y * dpr, box.x0 + (r.x + r.w) * dpr, box.y0 + (r.y + r.h) * dpr, rgb(r.color));
  }
}

/** The frame of the visible range. `inset` is OVERVIEW.inset in device pixels. */
export function paintOverview(out: Batch, box: Box, inset: number): void {
  const { x0, x1 } = box;
  const y0 = box.y0 + inset;
  const y1 = box.y1 - inset;
  const c = rgb(OVERVIEW.frame);
  out.rect(x0, y0, x1, y0 + HAIR, c);
  out.rect(x0, y1 - HAIR, x1, y1, c);
  out.rect(x0, y0, x0 + HAIR, y1, c);
  out.rect(x1 - HAIR, y0, x1, y1, c);
}
