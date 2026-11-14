import { BARS, KEYS, LOW, ROW, RULER_H, TPBAR, VEL_H, type Roll, type View } from "./model";

export interface Stats { n: number; mean: number; p50: number; p95: number; p99: number; max: number }

export function stats(a: number[]): Stats {
  const s = [...a].sort((x, y) => x - y);
  const q = (p: number) => s[Math.min(s.length - 1, Math.floor(p * s.length))];
  const r = (x: number) => Math.round(x * 1000) / 1000;
  return { n: s.length, mean: r(s.reduce((x, y) => x + y, 0) / s.length), p50: r(q(0.5)), p95: r(q(0.95)), p99: r(q(0.99)), max: r(s[s.length - 1]) };
}

// Horizontal scroll at 2 bars/s while zoom swings between 1 bar and the whole song every 8 s.
export function viewAt(tSec: number, width: number, height: number): View {
  const k = (1 - Math.cos((2 * Math.PI * tSec) / 8)) / 2;
  const bars = Math.exp(Math.log(1) + k * Math.log(BARS));
  const visible = bars * TPBAR;
  const span = BARS * TPBAR - visible;
  const x0 = span > 0 ? ((tSec * 2 * TPBAR) % span) : 0;
  const rows = Math.floor((height - RULER_H - VEL_H) / ROW);
  return { x0, pxPerTick: width / visible, topPitch: LOW + KEYS - 1 - Math.max(0, Math.floor((KEYS - rows) / 2)), playTick: x0 + visible / 3 };
}

// displayMs is the real refresh period of the monitor. rAF may run slower than the display; that counts as a miss.
export interface RunResult { intervals: Stats; work: Stats; rafMedianMs: number; displayMs: number; frames: number; within1: number; over2: number }

export function run(opts: {
  roll: Roll; size: () => { w: number; h: number }; durMs: number; warmMs: number; displayMs: number;
  perFrame?: (tSec: number) => void;
}): Promise<RunResult> {
  return new Promise((resolve) => {
    const intervals: number[] = [], work: number[] = [];
    let t0 = -1, last = -1;
    const frame = (ts: number) => {
      if (t0 < 0) t0 = ts;
      const el = ts - t0;
      const a = performance.now();
      const { w, h } = opts.size();
      opts.roll.draw(viewAt(el / 1000, w, h));
      opts.perFrame?.(el / 1000);
      const b = performance.now();
      if (el > opts.warmMs) { intervals.push(ts - last); work.push(b - a); }
      last = ts;
      if (el < opts.warmMs + opts.durMs) requestAnimationFrame(frame);
      else {
        const iv = stats(intervals);
        const d = opts.displayMs;
        resolve({
          intervals: iv, work: stats(work), rafMedianMs: iv.p50, displayMs: d, frames: intervals.length,
          // 25 % slack for timer jitter; a frame that slips a whole vsync is far above it.
          within1: intervals.filter((x) => x <= d * 1.25).length / intervals.length,
          over2: intervals.filter((x) => x > d * 2.25).length,
        });
      }
    };
    requestAnimationFrame(frame);
  });
}

// Draw on an external clock instead of rAF. rAF still runs, passively, to count rendering updates.
export function runTicked(opts: {
  roll: Roll; size: () => { w: number; h: number }; durMs: number; warmMs: number; displayMs: number;
  perFrame?: (tSec: number) => void; start: (onTick: () => void) => void;
}): Promise<RunResult & { rafIntervals: Stats; rafWithin1: number }> {
  return new Promise((resolve) => {
    const intervals: number[] = [], work: number[] = [], raf: number[] = [];
    const t0 = performance.now();
    let last = -1, lastRaf = -1, done = false;
    const rafLoop = (ts: number) => {
      if (ts - t0 > opts.warmMs && lastRaf >= 0) raf.push(ts - lastRaf);
      lastRaf = ts;
      if (!done) requestAnimationFrame(rafLoop);
    };
    requestAnimationFrame(rafLoop);
    opts.start(() => {
      if (done) return;
      const now = performance.now();
      const el = now - t0;
      const { w, h } = opts.size();
      opts.roll.draw(viewAt(el / 1000, w, h));
      opts.perFrame?.(el / 1000);
      const b = performance.now();
      if (el > opts.warmMs && last >= 0) { intervals.push(now - last); work.push(b - now); }
      last = now;
      if (el > opts.warmMs + opts.durMs) {
        done = true;
        const d = opts.displayMs;
        const iv = stats(intervals);
        resolve({
          intervals: iv, work: stats(work), rafMedianMs: stats(raf).p50, displayMs: d, frames: intervals.length,
          within1: intervals.filter((x) => x <= d * 1.25).length / intervals.length,
          over2: intervals.filter((x) => x > d * 2.25).length,
          rafIntervals: stats(raf), rafWithin1: raf.filter((x) => x <= d * 1.25).length / raf.length,
        });
      }
    });
  });
}
