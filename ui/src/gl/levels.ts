import type { Telemetry } from "../bridge";
import type { BarLevel } from "./paint";
import { CLIP_HOLD_MS } from "./dims";
import { dbToLength, fall, peakToDb } from "./level";

/**
 * The levels on the screen. Channel 0 and 1 are the master's left and right; 2 + 2i and 3 + 2i
 * are track i (in the order of the telemetry).
 */
export class Levels {
  private target = new Float64Array(2).fill(-Infinity);
  private shown = new Float64Array(2).fill(-Infinity);
  private clipUntil = new Float64Array(2).fill(-Infinity);
  private lastMs = 0;

  private resize(channels: number): void {
    if (channels === this.target.length) return;
    const grow = (old: Float64Array) => {
      const next = new Float64Array(channels).fill(-Infinity);
      next.set(old.subarray(0, Math.min(old.length, channels)));
      return next;
    };
    this.target = grow(this.target);
    this.shown = grow(this.shown);
    this.clipUntil = grow(this.clipUntil);
  }

  onTelemetry(t: Telemetry, nowMs: number): void {
    this.advance(nowMs);
    this.resize(2 + t.trackPeaks.length);
    for (let c = 0; c < this.target.length; c++) {
      const peak = c < 2 ? t.masterPeak[c]! : t.trackPeaks[c - 2]!;
      const db = peakToDb(peak);
      this.target[c] = db;
      if (db > this.shown[c]!) this.shown[c] = db;
      if (peak > 1 || (c < 2 && t.masterClipped)) this.clipUntil[c] = nowMs + CLIP_HOLD_MS;
    }
  }

  /** Lets the shown levels fall up to `nowMs`. Returns whether any is still falling. */
  advance(nowMs: number): boolean {
    const dt = Math.max(0, nowMs - this.lastMs);
    this.lastMs = nowMs;
    let falling = false;
    for (let c = 0; c < this.shown.length; c++) {
      const next = fall(this.shown[c]!, this.target[c]!, dt);
      this.shown[c] = next;
      if (next > this.target[c]!) falling = true;
    }
    return falling;
  }

  bar(channel: number, nowMs: number): BarLevel {
    if (channel >= this.shown.length) return { length: 0, clipped: false };
    return { length: dbToLength(this.shown[channel]!), clipped: nowMs < this.clipUntil[channel]! };
  }

  /** When the next clip mark goes out, or null when none is lit. */
  nextClipEnd(nowMs: number): number | null {
    let end: number | null = null;
    for (const until of this.clipUntil) {
      if (until > nowMs && (end === null || until < end)) end = until;
    }
    return end;
  }
}
