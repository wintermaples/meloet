// Time in the song (docs/architecture.md 11). Gives the same answers as crates/meloet-model/src/time.rs. Pure functions with no DOM.

import { positionAt, type Telemetry, type TransportView } from "../bridge";
import { barsMap, type Bars } from "../time/bars";
import { TICKS_PER_QUARTER } from "../time/ticks";

export type { Bars } from "../time/bars";
export { TICKS_PER_QUARTER } from "../time/ticks";

/** The usable tempo (the owner's decision). In steps of 0.01. */
export const TEMPO_MIN = 30;
export const TEMPO_MAX = 300;
/** The usable time signatures (the owner's decision). */
export const METER_NUM_MAX = 12;
export const METER_DENS: readonly number[] = [2, 4, 8, 16];

export interface Meter {
  num: number;
  den: number;
}

export function ticksPerBeat(meter: Meter): number {
  return (TICKS_PER_QUARTER * 4) / meter.den;
}

export function ticksPerBar(meter: Meter): number {
  return meter.num * ticksPerBeat(meter);
}

export interface BarBeatTick {
  bar: number;
  beat: number;
  tick: number;
}

/**
 * Bars and beats start at 1; the tick is the tick inside that beat (0 to 1919 in 4/4). Fractional ticks are cut off, and anything before 0 is read as 0.
 * bars is one time signature, or the tempo map (for songs with time signature changes).
 */
export function barBeatTick(tick: number, bars: Bars): BarBeatTick {
  return barsMap(bars).barBeatTick(tick, { bar: 0, beat: 0, tick: 0 });
}

/** The tick text for display. Always 4 digits with leading zeros (「0960」). */
export const tickText = (tick: number) => String(tick).padStart(4, "0");

/** 「7. 3. 0960」 (the form of mock F-ArrangeRec; ticks in 4 digits). */
export function positionText(tick: number, bars: Bars): string {
  const b = barBeatTick(tick, bars);
  return `${b.bar}. ${b.beat}. ${tickText(b.tick)}`;
}

/** 「00:00:16.562」 (mock F-ArrangeRec). Below a millisecond is cut off. */
export function clockText(seconds: number): string {
  const ms = Math.max(0, Math.floor(seconds * 1000 + 1e-6));
  const two = (n: number) => String(n).padStart(2, "0");
  const h = Math.floor(ms / 3_600_000);
  const m = Math.floor(ms / 60_000) % 60;
  const s = Math.floor(ms / 1000) % 60;
  return `${two(h)}:${two(m)}:${two(s)}.${String(ms % 1000).padStart(3, "0")}`;
}

/** Normalizes typed text so that full-width digits and symbols can be read too. */
const tidy = (s: string) => s.normalize("NFKC").trim();

/** Parses a typed tempo. null for values out of range or unreadable text. Up to 2 decimal places. */
export function parseTempo(s: string): number | null {
  const m = tidy(s).match(/^(\d{1,3})(?:\.(\d{0,2}))?$/);
  if (!m) return null;
  const centi = Number(m[1]) * 100 + Number((m[2] ?? "").padEnd(2, "0"));
  if (centi < TEMPO_MIN * 100 || centi > TEMPO_MAX * 100) return null;
  return centi / 100;
}

/** Parses a typed time signature (「4/4」). null for unusable values or unreadable text. */
export function parseMeter(s: string): Meter | null {
  const m = tidy(s).match(/^(\d{1,2})\s*\/\s*(\d{1,2})$/);
  if (!m) return null;
  const num = Number(m[1]);
  const den = Number(m[2]);
  return num >= 1 && num <= METER_NUM_MAX && METER_DENS.includes(den) ? { num, den } : null;
}

/**
 * For an unusable typed time signature, whether the top or the bottom is wrong. The bottom if the top is 1 to 12, otherwise the top
 * (also the top when there is no 「/」 or it cannot be read).
 */
export function meterFieldOf(s: string): "meterNum" | "meterDen" {
  const m = tidy(s).match(/^(\d{1,2})\s*\/(.*)$/);
  const num = m ? Number(m[1]) : NaN;
  return num >= 1 && num <= METER_NUM_MAX ? "meterDen" : "meterNum";
}

export function meterText(meter: Meter): string {
  return `${meter.num}/${meter.den}`;
}

/** The tempo after a ↑ ↓ step. Never goes out of range. */
export function stepTempo(bpm: number, delta: number): number {
  const centi = Math.round(bpm * 100) + delta * 100;
  return Math.min(TEMPO_MAX * 100, Math.max(TEMPO_MIN * 100, centi)) / 100;
}

/** The time signature after a ↑ ↓ step. Only the top number changes. */
export function stepMeter(meter: Meter, delta: number): Meter {
  return { num: Math.min(METER_NUM_MAX, Math.max(1, meter.num + delta)), den: meter.den };
}

/**
 * The position (ticks) shown by the transport numbers and the playhead line. When stopped, transport.position (it arrives at once in a diff);
 * during the count-in, where recording starts (mock F-Connect4); when running, the telemetry position moved forward to rustNowUs
 * (the telemetry position as it is if rustNowUs is null). transport.position if the telemetry has no position.
 */
export function shownTick(transport: TransportView, telemetry: Telemetry | null, rustNowUs: number | null): number {
  if (transport.state === "stopped") return transport.position;
  if (transport.state === "countIn") return transport.recording?.from ?? transport.position;
  const p = telemetry?.position;
  if (!p) return transport.position;
  return rustNowUs === null ? p.tick : (positionAt(telemetry, rustNowUs) ?? transport.position);
}
