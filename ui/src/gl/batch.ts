import type { Rgb } from "./color";
import { WAVE_TEX_MAX_PEAKS, type WaveSource } from "./wave";

/** Floats per rectangle: 6 corners of (x, y, r, g, b). */
export const FLOATS_PER_RECT = 30;
/** Floats per hatch: at, x0, y0, x1, y1, pitch, shift, r, g, b (`at` as for a wave). */
export const FLOATS_PER_HATCH = 10;

function corner(d: Float32Array, i: number, x: number, y: number, r: number, g: number, b: number): number {
  d[i] = x;
  d[i + 1] = y;
  d[i + 2] = r;
  d[i + 3] = g;
  d[i + 4] = b;
  return i + 5;
}

/**
 * A wave: a rectangle the wave shader fills from the peaks of `source` (renderer.ts). Places are device pixels
 * of the picture; they may have a fraction, but for the rectangle itself.
 */
export interface WaveQuad {
  /** The part to paint. */
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  source: WaveSource;
  /** The peak at the left edge of the pixel `mapX` is `base + frac` (a whole number and the rest); a pixel to the right is `step` peaks on. */
  mapX: number;
  base: number;
  frac: number;
  step: number;
  /** Only the peaks `first .. end` are drawn (the part of the file the take has). */
  first: number;
  end: number;
  /** Full scale reaches from `top` to `bottom`; silence is in the middle. */
  top: number;
  bottom: number;
  /** The clip's volume as a factor. */
  gain: number;
  /** The clip starts at `fadeX0` and fades in over `fadeIn` pixels; it fades out over `fadeOut` pixels up to `fadeX1`. */
  fadeX0: number;
  fadeIn: number;
  fadeX1: number;
  fadeOut: number;
  /** A bar `bar` wide every `pitch` pixels, counted from `barsX`; a bar is never lower than `minH`. Whole pixels. */
  barsX: number;
  pitch: number;
  bar: number;
  minH: number;
  color: Rgb;
  /** The line of silence, or none. */
  mid: Rgb | null;
  /** The lines of the fades, or none. */
  line: Rgb | null;
}

export interface PlacedWave extends WaveQuad {
  /** Floats of the rectangles drawn before it. */
  at: number;
  /** `source.stamp` when the wave shows the last peak the source has (it may change, and more may follow); else -1. */
  fresh: number;
}

const WAVE_NUMBERS = ["at", "x0", "y0", "x1", "y1", "mapX", "base", "frac", "step", "first", "end", "top", "bottom", "gain", "fadeX0", "fadeIn", "fadeX1", "fadeOut", "barsX", "pitch", "bar", "minH", "fresh"] as const;
const sameRgb = (a: Rgb | null, b: Rgb | null) => a === b || (a !== null && b !== null && a[0] === b[0] && a[1] === b[1] && a[2] === b[2]);

/**
 * The rectangles of one frame, in device pixels with the origin at the top left of the canvas.
 * Every edge is a whole pixel, so nothing is smeared across two pixels.
 */
export class Batch {
  data = new Float32Array(FLOATS_PER_RECT * 256);
  /** Floats in use. */
  length = 0;
  /** In the order they are drawn, each after the rectangles put before it. */
  waves: PlacedWave[] = [];
  /** Hatches (FLOATS_PER_HATCH each), drawn like waves after the rectangles put before them. */
  hatchData = new Float32Array(FLOATS_PER_HATCH * 32);
  /** Floats of hatchData in use. */
  hatchLength = 0;
  private cx0 = 0;
  private cy0 = 0;
  private cx1 = 0;
  private cy1 = 0;

  reset(): void {
    this.length = 0;
    this.hatchLength = 0;
    if (this.waves.length > 0) this.waves = [];
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

  /** Cut to the clip box, as a rectangle is. */
  wave(q: WaveQuad): void {
    const x0 = Math.max(Math.round(q.x0), this.cx0);
    const y0 = Math.max(Math.round(q.y0), this.cy0);
    const x1 = Math.min(Math.round(q.x1), this.cx1);
    const y1 = Math.min(Math.round(q.y1), this.cy1);
    if (x1 <= x0 || y1 <= y0) return;
    const peaks = q.source.peaks;
    // No further than the right edge reads (its bar reaches a pitch on): a wave that ends before the last peak of a
    // growing file is the same picture when the file has grown.
    const reach = Math.ceil(q.base + q.frac + (x1 - q.mapX + q.pitch) * q.step) + 1;
    const end = Math.min(q.end, peaks.count, WAVE_TEX_MAX_PEAKS, reach);
    const fresh = end >= peaks.count - 1 ? q.source.stamp : -1;
    this.waves.push({ ...q, x0, y0, x1, y1, end, at: this.length, fresh });
  }

  /**
   * Lines one pixel wide rising to the right ("/"), `pitch` pixels apart along a pixel row: inside the rectangle, the
   * pixels (x, y) where x + y − ox − oy is a multiple of pitch. (ox, oy) ties the pattern to what it lies on, so it
   * moves with it. Cut to the clip box, as a rectangle is.
   */
  hatch(x0: number, y0: number, x1: number, y1: number, color: Rgb, pitch: number, ox: number, oy: number): void {
    x0 = Math.max(Math.round(x0), this.cx0);
    y0 = Math.max(Math.round(y0), this.cy0);
    x1 = Math.min(Math.round(x1), this.cx1);
    y1 = Math.min(Math.round(y1), this.cy1);
    if (x1 <= x0 || y1 <= y0) return;
    const p = Math.max(2, Math.round(pitch));
    // The shader tests (x + y + shift) % p with whole numbers that are never below 0.
    const shift = (((-(Math.round(ox) + Math.round(oy))) % p) + p) % p;
    if (this.hatchLength + FLOATS_PER_HATCH > this.hatchData.length) {
      const bigger = new Float32Array(this.hatchData.length * 2);
      bigger.set(this.hatchData);
      this.hatchData = bigger;
    }
    const d = this.hatchData;
    const i = this.hatchLength;
    d[i] = this.length;
    d[i + 1] = x0;
    d[i + 2] = y0;
    d[i + 3] = x1;
    d[i + 4] = y1;
    d[i + 5] = p;
    d[i + 6] = shift;
    d[i + 7] = color[0];
    d[i + 8] = color[1];
    d[i + 9] = color[2];
    this.hatchLength = i + FLOATS_PER_HATCH;
  }

  get rectCount(): number {
    return this.length / FLOATS_PER_RECT;
  }

  sameAs(other: Batch): boolean {
    if (this.length !== other.length || this.waves.length !== other.waves.length || this.hatchLength !== other.hatchLength) return false;
    for (let i = 0; i < this.hatchLength; i++) if (this.hatchData[i] !== other.hatchData[i]) return false;
    for (let i = 0; i < this.waves.length; i++) {
      const a = this.waves[i]!;
      const b = other.waves[i]!;
      if (a.source.id !== b.source.id || !sameRgb(a.color, b.color) || !sameRgb(a.mid, b.mid) || !sameRgb(a.line, b.line)) return false;
      for (const k of WAVE_NUMBERS) if (a[k] !== b[k]) return false;
    }
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

  /** For tests: [at, x0, y0, x1, y1, pitch, shift, r, g, b] per hatch. */
  hatches(): number[][] {
    const out: number[][] = [];
    for (let i = 0; i < this.hatchLength; i += FLOATS_PER_HATCH) out.push(Array.from(this.hatchData.subarray(i, i + FLOATS_PER_HATCH)));
    return out;
  }
}
