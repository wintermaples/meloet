// Out-of-range values in number fields (docs/history/design-notes.ja.md B-68). If it reads as a number, set it to the nearer end; if not, null (the "broken" border).
// When clamped, also returns the contents of the ClampValue to send (field, value, range). Pure functions with no DOM.

import type { ValueField } from "../bridge";
import { tempoText } from "./format";
import { barAt, barsMap, perBeatOf, tickOfPosition, tickRangeText } from "../time/bars";
import { METER_DENS, METER_NUM_MAX, positionText, TEMPO_MAX, TEMPO_MIN, type Bars, type Meter } from "./time";

export interface ClampNote {
  field: ValueField;
  /** The value set (written as shown on screen). */
  value: string;
  range: string | null;
}

export interface Read<T> {
  value: T;
  /** Clamped because it was out of range. null if in range. */
  clamp: ClampNote | null;
}

/** Normalizes typed text so that full-width digits and symbols and the U+2212 minus can be read too. */
const tidy = (s: string) => s.normalize("NFKC").trim().replace(/−/g, "-");

const NUMBER = /^(-?)(\d*)(?:\.(\d*))?$/;

/** Rounds a decimal to digits places, as an integer times 10^digits (it works on the text, so 120.555 also becomes 12056). null if unreadable. */
export function readScaled(s: string, digits: number): number | null {
  const m = tidy(s).match(NUMBER);
  if (!m || (m[2] === "" && (m[3] ?? "") === "")) return null;
  const whole = Number(m[2] || "0");
  const frac = (m[3] ?? "").padEnd(digits + 1, "0");
  let scaled = whole * 10 ** digits + Number(frac.slice(0, digits) || "0");
  if (Number(frac[digits]) >= 5) scaled += 1;
  return m[1] === "-" ? -scaled : scaled;
}

const clampTo = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Tempo (30.00 to 300.00, up to 2 decimal places). */
export function readTempo(s: string): Read<number> | null {
  const centi = readScaled(s, 2);
  if (centi === null) return null;
  const kept = clampTo(centi, TEMPO_MIN * 100, TEMPO_MAX * 100);
  const value = kept / 100;
  return { value, clamp: kept === centi ? null : { field: "tempo", value: tempoText(value), range: null } };
}

/** Time signature bottom: the nearest of 2, 4, 8, 16. The larger one on a tie. */
export function nearestDen(n: number): number {
  let best = METER_DENS[0]!;
  for (const d of METER_DENS) if (Math.abs(d - n) <= Math.abs(best - n)) best = d;
  return best;
}

/** Time signature (「4/4」). Top 1 to 12, bottom nearestDen. When both are clamped, only the top is mentioned. */
export function readMeter(s: string): Read<Meter> | null {
  const m = tidy(s).match(/^([^/]*)\/([^/]*)$/);
  if (!m) return null;
  const numRaw = readScaled(m[1]!, 0);
  const denRaw = readScaled(m[2]!, 0);
  if (numRaw === null || denRaw === null) return null;
  const num = clampTo(numRaw, 1, METER_NUM_MAX);
  const den = nearestDen(denRaw);
  const clamp: ClampNote | null =
    num !== numRaw
      ? { field: "meterNum", value: String(num), range: null }
      : den !== denRaw
        ? { field: "meterDen", value: String(den), range: null }
        : null;
  return { value: { num, den }, clamp };
}

/** A count of bars typed into a box (1 to max); the range comes with the note. */
export function readBars(s: string, max: number): Read<number> | null {
  const raw = readScaled(s, 0);
  if (raw === null || !/^-?\d+$/.test(tidy(s))) return null;
  const value = clampTo(raw, 1, max);
  return { value, clamp: value === raw ? null : { field: "bars", value: String(value), range: `1〜${max}` } };
}

/** Velocity (1 to 127). */
export function readVelocity(s: string): Read<number> | null {
  const raw = readScaled(s, 0);
  if (raw === null) return null;
  const value = clampTo(raw, 1, 127);
  return { value, clamp: value === raw ? null : { field: "velocity", value: String(value), range: null } };
}

/**
 * The playback position (「5」「5.2」「5.2.0960」; spaces around the dividers are fine) as ticks in the song.
 * Clamps the bar to 1 and up, the beat to 1 to the top of the time signature, and the tick to 0 to (ticks per beat − 1). When two or more are clamped, only the largest unit is mentioned.
 */
export function readPosition(s: string, bars: Bars): Read<number> | null {
  const parts = tidy(s).split(/\s*\.\s*/);
  if (parts.length > 3) return null;
  const nums = parts.map((p) => (/^-?\d+$/.test(p) ? Number(p) : null));
  if (nums.some((n) => n === null)) return null;
  const [barRaw, beatRaw = 1, tickRaw = 0] = nums as number[];
  const map = barsMap(bars);
  const bar = Math.max(1, barRaw!);
  const at = barAt(map, bar);
  const num = at.num;
  const perBeat = perBeatOf(at);
  const beat = clampTo(beatRaw, 1, num);
  const tick = clampTo(tickRaw, 0, perBeat - 1);
  const value = tickOfPosition(map, bar, beat, tick);
  const shown = positionText(value, map);
  const clamp: ClampNote | null =
    bar !== barRaw
      ? { field: "bar", value: shown, range: null }
      : beat !== beatRaw
        ? { field: "beat", value: shown, range: `1〜${num}` }
        : tick !== tickRaw
          ? { field: "tick", value: shown, range: tickRangeText(perBeat) }
          : null;
  return { value, clamp };
}
