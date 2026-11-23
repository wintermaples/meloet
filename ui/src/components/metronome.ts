// The metronome window's numbers (round 12 R3): the volume bar and 「テンポを手拍子で」. Pure; knows no DOM.

import { CLICK_VOLUME_MAX_DB, CLICK_VOLUME_MIN_DB } from "../bridge";
import { t } from "../text";

/** The taps the tempo is worked out from, at most. */
export const TAP_KEEP = 8;
/** A tap this long after the one before starts again. */
export const TAP_RESET_MS = 2000;
/** The tempo range of the song (30.00 to 300.00). */
export const TAP_MIN_BPM = 30;
export const TAP_MAX_BPM = 300;

/** 「タップ」: the mean gap of the last taps as a tempo, in whole BPM. Screen only; nothing is sent per tap. */
export class TapTempo {
  private times: number[] = [];

  /** A tap at `ms` (performance.now()). Returns the tempo it gives now (null below two taps). */
  tap(ms: number): number | null {
    const last = this.times[this.times.length - 1];
    if (last !== undefined && (ms - last >= TAP_RESET_MS || ms < last)) this.times = [];
    this.times.push(ms);
    if (this.times.length > TAP_KEEP) this.times.shift();
    return this.bpm();
  }

  bpm(): number | null {
    const n = this.times.length;
    if (n < 2) return null;
    const gap = (this.times[n - 1]! - this.times[0]!) / (n - 1);
    if (!(gap > 0)) return null;
    return Math.min(TAP_MAX_BPM, Math.max(TAP_MIN_BPM, Math.round(60_000 / gap)));
  }

  clear(): void {
    this.times = [];
  }
}

const SPAN = CLICK_VOLUME_MAX_DB - CLICK_VOLUME_MIN_DB;

/** The volume (whole dB) at a place on the bar, 0 (left, −24) to 1 (right, +6). */
export function clickDbAt(frac: number): number {
  return Math.round(CLICK_VOLUME_MIN_DB + Math.min(1, Math.max(0, frac)) * SPAN);
}

/** Where the volume is on the bar, 0 to 1. */
export function clickFrac(db: number): number {
  return Math.min(1, Math.max(0, (db - CLICK_VOLUME_MIN_DB) / SPAN));
}

/** 「−12 dB」. */
export function clickDbText(db: number): string {
  const whole = Math.round(db);
  return t("common.db", { value: (whole === 0 ? 0 : whole).toString().replace("-", "−") });
}

/** Typed volume (「-12」「−12 dB」「+3」), as whole dB put into the range. undefined if it cannot be read. */
export function parseClickDb(text: string): number | undefined {
  const s = text.trim().replace(/[−ー]/g, "-").replace(/\s*dB$/i, "");
  if (!/^[+-]?(\d+(\.\d*)?|\.\d+)$/.test(s)) return undefined;
  return Math.min(CLICK_VOLUME_MAX_DB, Math.max(CLICK_VOLUME_MIN_DB, Math.round(Number(s))));
}
