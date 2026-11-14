export const TPB = 480; // ticks per beat
export const TPBAR = TPB * 4;
export const BARS = 200;
export const LOW = 36; // C2
export const KEYS = 49; // C2..C6
export const ROW = 12; // size.note
export const NOTE_H = 11;
export const KEYS_W = 104; // size.col.keys
export const RULER_H = 20;
export const VEL_H = 72; // size.lane.vel

export const C = {
  g0: "#0A0A0A", g1: "#121212", g2: "#161616", g3: "#1A1A1A", g4: "#222222", g5: "#2C2C2C",
  g6: "#3A3A3A", g7: "#5E5E5E", g8: "#9A9A9A", g9: "#D6D6D6", g10: "#F2F2F2", accent: "#FFB000",
  ok: "#4CD964", hot: "#FFD60A",
};

export interface Notes {
  count: number;
  start: Float32Array; // ticks, sorted ascending
  len: Float32Array;
  pitch: Uint8Array;
  vel: Uint8Array;
  maxLen: number;
}

// Deterministic so every run draws the same thing.
export function makeNotes(count = 10000): Notes {
  let s = 12345;
  const rnd = () => ((s = (s * 1103515245 + 12345) >>> 0) / 4294967296);
  const start = new Float32Array(count), len = new Float32Array(count);
  const pitch = new Uint8Array(count), vel = new Uint8Array(count);
  const span = BARS * TPBAR;
  for (let i = 0; i < count; i++) {
    start[i] = Math.floor((i / count) * span / 120) * 120;
    len[i] = [120, 240, 480, 960, 1920][Math.floor(rnd() * 5)];
    pitch[i] = LOW + Math.floor(rnd() * KEYS);
    vel[i] = 40 + Math.floor(rnd() * 88);
  }
  return { count, start, len, pitch, vel, maxLen: 1920 };
}

export const isBlack = (p: number) => [1, 3, 6, 8, 10].includes(p % 12);
export const velShade = (v: number) => (v < 70 ? 0 : v < 90 ? 1 : 2); // gray.8 / gray.9 / gray.10

export interface View {
  x0: number; // first visible tick
  pxPerTick: number;
  topPitch: number; // pitch drawn in the first row
  playTick: number;
}

// First index whose start >= t.
export function lowerBound(a: Float32Array, t: number): number {
  let lo = 0, hi = a.length;
  while (lo < hi) { const m = (lo + hi) >> 1; if (a[m] < t) lo = m + 1; else hi = m; }
  return lo;
}

export interface Roll {
  resize(w: number, h: number, dpr: number): void;
  draw(v: View): void;
}
