// What each kind of slot looks like, as rectangles. No WebGL and no DOM here.

import type { Batch } from "./batch";
import { rgb } from "./color";
import { ARRANGE_GRID, KEYBOARD, METER, METER_HOT_FROM, OVERVIEW, VELOCITY } from "./dims";
import { keyboardLayout } from "./keyboard";

/** A slot's box in device pixels (whole numbers). `hair` is line.hair: always one device pixel. */
export interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}
export const HAIR = 1;

export function paintKeyboard(
  out: Batch,
  box: Box,
  size: "full" | "slim",
  firstNote: number,
  lastNote: number,
  isDown: (note: number) => boolean,
): void {
  const { whiteCount, keys } = keyboardLayout(firstNote, lastNote);
  if (whiteCount === 0) return;
  const { x0, y0, x1, y1 } = box;
  // Lines 0..whiteCount: the left edge, then the right edge of each white key.
  const cell = (x1 - x0 - HAIR) / whiteCount;
  const lineX = (i: number) => x0 + Math.round(i * cell);

  out.rect(x0, y0, x1, y1, rgb(KEYBOARD.frame));
  for (const key of keys) {
    if (key.black) continue;
    const fill = isDown(key.note) ? KEYBOARD.down : KEYBOARD.whiteFill;
    out.rect(lineX(key.index) + HAIR, y0 + HAIR, lineX(key.index + 1), y1 - HAIR, rgb(fill));
  }
  if (size === "slim") return;

  // Whole and odd, so the key sits evenly on its one-pixel line.
  const half = Math.max(1, Math.round(((cell * KEYBOARD.black.w) / KEYBOARD.white.w - HAIR) / 2));
  const bottom = y0 + Math.round(((y1 - y0) * KEYBOARD.black.h) / KEYBOARD.white.h);
  for (const key of keys) {
    if (!key.black) continue;
    const left = lineX(key.index) - half;
    const right = lineX(key.index) + HAIR + half;
    out.rect(left, y0, right, bottom, rgb(KEYBOARD.blackFrame));
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

/** Two bars, left above right. `gap` and `mark` (line.mark) are device pixels. */
export function paintMeter(out: Batch, box: Box, gap: number, mark: number, left: BarLevel, right: BarLevel): void {
  const { x0, y0, x1, y1 } = box;
  const h = Math.max(1, Math.round((y1 - y0 - gap) / 2));
  const hotFrom = x0 + Math.round((x1 - x0) * METER_HOT_FROM);
  const bar = (top: number, level: BarLevel) => {
    out.rect(x0, top, x1, top + h, rgb(METER.ground));
    const end = x0 + Math.round((x1 - x0) * Math.min(Math.max(level.length, 0), 1));
    out.rect(x0, top, Math.min(end, hotFrom), top + h, rgb(METER.ok));
    out.rect(hotFrom, top, end, top + h, rgb(METER.hot));
    if (level.clipped) out.rect(x1 - mark, top, x1, top + h, rgb(METER.clip));
  };
  bar(y0, left);
  bar(y1 - h, right);
}

/** Bar lines. `firstBarX` and `barPx` are device pixels and may have a fraction. */
export function paintArrangeGrid(out: Batch, box: Box, firstBarX: number, barPx: number): void {
  if (!(barPx >= 1)) return;
  const { x0, y0, x1, y1 } = box;
  const first = Math.max(0, Math.ceil(-firstBarX / barPx));
  for (let k = first; ; k++) {
    const x = x0 + Math.round(firstBarX + k * barPx);
    if (x >= x1) break;
    out.rect(x, y0, x + HAIR, y1, rgb(ARRANGE_GRID.line));
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
