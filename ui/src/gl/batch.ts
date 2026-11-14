import type { Rgb } from "./color";

/** Floats per rectangle: 6 corners of (x, y, r, g, b). */
export const FLOATS_PER_RECT = 30;

function corner(d: Float32Array, i: number, x: number, y: number, r: number, g: number, b: number): number {
  d[i] = x;
  d[i + 1] = y;
  d[i + 2] = r;
  d[i + 3] = g;
  d[i + 4] = b;
  return i + 5;
}

/**
 * The rectangles of one frame, in device pixels with the origin at the top left of the canvas.
 * Every edge is a whole pixel, so nothing is smeared across two pixels.
 */
export class Batch {
  data = new Float32Array(FLOATS_PER_RECT * 256);
  /** Floats in use. */
  length = 0;
  private cx0 = 0;
  private cy0 = 0;
  private cx1 = 0;
  private cy1 = 0;

  reset(): void {
    this.length = 0;
  }

  /** Later rectangles are cut to this box. */
  clip(x0: number, y0: number, x1: number, y1: number): void {
    this.cx0 = x0;
    this.cy0 = y0;
    this.cx1 = x1;
    this.cy1 = y1;
  }

  rect(x0: number, y0: number, x1: number, y1: number, color: Rgb): void {
    x0 = Math.max(Math.round(x0), this.cx0);
    y0 = Math.max(Math.round(y0), this.cy0);
    x1 = Math.min(Math.round(x1), this.cx1);
    y1 = Math.min(Math.round(y1), this.cy1);
    if (x1 <= x0 || y1 <= y0) return;
    if (this.length + FLOATS_PER_RECT > this.data.length) {
      const bigger = new Float32Array(this.data.length * 2);
      bigger.set(this.data);
      this.data = bigger;
    }
    const d = this.data;
    const r = color[0];
    const g = color[1];
    const b = color[2];
    let i = this.length;
    i = corner(d, i, x0, y0, r, g, b);
    i = corner(d, i, x1, y0, r, g, b);
    i = corner(d, i, x0, y1, r, g, b);
    i = corner(d, i, x0, y1, r, g, b);
    i = corner(d, i, x1, y0, r, g, b);
    i = corner(d, i, x1, y1, r, g, b);
    this.length = i;
  }

  get rectCount(): number {
    return this.length / FLOATS_PER_RECT;
  }

  sameAs(other: Batch): boolean {
    if (this.length !== other.length) return false;
    for (let i = 0; i < this.length; i++) if (this.data[i] !== other.data[i]) return false;
    return true;
  }

  /** For tests: [x0, y0, x1, y1, r, g, b] per rectangle. */
  rects(): number[][] {
    const out: number[][] = [];
    for (let i = 0; i < this.length; i += FLOATS_PER_RECT) {
      const d = this.data;
      out.push([d[i]!, d[i + 1]!, d[i + 25]!, d[i + 26]!, d[i + 2]!, d[i + 3]!, d[i + 4]!]);
    }
    return out;
  }
}
