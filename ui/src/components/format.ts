// How numbers are shown. Pure functions with no DOM.
import type { ScaleMode } from "../bridge";
import { findTerm, RANGE_MARK, rawText, t, textLanguage, type TextKey } from "../text";
import { barBeatTick, tickText, ticksPerBeat, type Bars, type Meter } from "./time";

/** The minus sign is 「−」 (U+2212), as in the spec. */
const MINUS = "−";

/** dB below this is 「−∞」. */
export const SILENT_DB = -90;

/** dB as a number with 1 decimal place ("0.0", "−4.8"). */
export function dbNumber(db: number): string {
  const s = (Math.round(db * 10) / 10).toFixed(1);
  return s === "-0.0" ? "0.0" : s.replace("-", MINUS);
}

/** A value where 1.0 = 0 dBFS, as text like 「−8.4 dB」. 「−∞ dB」 if too small. */
export function peakText(peak: number): string {
  const db = peakDb(peak);
  return db < SILENT_DB ? t("common.dbSilent") : t("common.db", { value: dbNumber(db) });
}

/** The form without 「dB」 (「−6.2」「−∞」; spec 3, the master in the narrow transport). */
export function peakNumberText(peak: number): string {
  const db = peakDb(peak);
  return db < SILENT_DB ? t("common.dbSilentBare") : t("common.dbBare", { value: dbNumber(db) });
}

function peakDb(peak: number): number {
  return peak > 0 ? 20 * Math.log10(peak) : -Infinity;
}

/** Tempo as 「96.00」. */
export function tempoText(bpm: number): string {
  return bpm.toFixed(2);
}

/** Latency in ms. Below 20 ms with 1 decimal place (「13.5」「13.0」), from 20 ms a whole number (「53」). The style of spec 6. */
export function msText(ms: number): string {
  const r = Math.round(ms * 10) / 10;
  return r < 20 ? r.toFixed(1) : String(Math.round(ms));
}

/** The largest value in the last windowMs. */
export class PeakHold {
  private samples: { at: number; value: number }[] = [];

  constructor(readonly windowMs = 1000) {}

  push(value: number, nowMs: number): void {
    this.samples.push({ at: nowMs, value });
    this.drop(nowMs);
  }

  max(nowMs: number): number {
    this.drop(nowMs);
    return this.samples.reduce((m, s) => Math.max(m, s.value), 0);
  }

  private drop(nowMs: number): void {
    const from = nowMs - this.windowMs;
    let i = 0;
    while (i < this.samples.length && this.samples[i]!.at < from) i++;
    if (i > 0) this.samples.splice(0, i);
  }
}

/** Unix ms in this PC's local time, as the values for the slots of 「11 月 15 日 22:41」 (minutes in 2 digits). */
export function dayAndTime(ms: number): { month: number; day: number; time: string } {
  const d = new Date(ms);
  return { month: d.getMonth() + 1, day: d.getDate(), time: `${d.getHours()}:${String(d.getMinutes()).padStart(2, "0")}` };
}

/**
 * Splits a text into plain parts and terms (parts with `word`, the Japanese word of the term), so that term tips can be
 * added. In English the term's English word is looked for, as a whole word in any case, also with a plural "s".
 */
export function termParts(text: string, word: string): { text: string; term: boolean }[] {
  const english = textLanguage() === "en" ? findTerm(word)?.term : undefined;
  if (english) {
    const escaped = english.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return text
      .split(new RegExp(`(\\b${escaped}(?:e?s)?\\b)`, "i"))
      .map((piece, i) => ({ text: piece, term: i % 2 === 1 }))
      .filter((p) => p.text !== "");
  }
  return text
    .split(word)
    .flatMap((piece, i) => (i === 0 ? [piece] : [word, piece]))
    .map((piece) => ({ text: piece, term: piece === word }))
    .filter((p) => p.text !== "");
}

export interface TextPart {
  text: string;
  /** The name of the slot of a filled-in value. null for the rest of the text. */
  hole: string | null;
}

/** Splits a text into plain parts and filled-in values (so that only the values get another font or color). */
export function textParts(key: TextKey, values: Readonly<Record<string, string | number>>): TextPart[] {
  const out: TextPart[] = [];
  const text = rawText(key, values);
  const re = /\{(\w+)\}/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > last) out.push({ text: text.slice(last, m.index), hole: null });
    const name = m[1]!;
    out.push(name in values ? { text: String(values[name]), hole: name } : { text: m[0], hole: null });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ text: text.slice(last), hole: null });
  return out;
}

// ---- Around the piano roll (note list, selected notes, status line) ----

const PITCH_CLASSES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;

/** A note name without the octave (「D#」). 0 is C. */
export function pitchClassName(keyOrRoot: number): string {
  return PITCH_CLASSES[((keyOrRoot % 12) + 12) % 12]!;
}

const SCALE_MODE_TEXT: Readonly<Record<ScaleMode, TextKey>> = {
  major: "scale.major",
  minor: "scale.minor",
  harmonicMinor: "scale.harmonicMinor",
  melodicMinor: "scale.melodicMinor",
  majorPentatonic: "scale.majorPentatonic",
  minorPentatonic: "scale.minorPentatonic",
  blues: "scale.blues",
  dorian: "scale.dorian",
  mixolydian: "scale.mixolydian",
};

/** The name of a kind of scale (「メジャー」「ブルース」). */
export const scaleModeName = (mode: ScaleMode) => t(SCALE_MODE_TEXT[mode]);

/** The scale name (「C メジャー」「A マイナー」; the owner's decision). */
export function scaleName(root: number, mode: ScaleMode): string {
  return t("scale.name", { root: pitchClassName(root), mode: scaleModeName(mode) });
}

/** The grid name (「1 小節」「1/16」; for triplets 「1/8 三連符」). division 0 is 1 bar. */
export function gridText(division: number, triplet: boolean): string {
  const base = division === 0 ? t("grid.bar") : t("grid.division", { division });
  return triplet ? `${base} ${t("grid.triplet")}` : base;
}

/** The usable range, 「1〜127」「C-1〜G9」 (the range style of spec 6 "Typed a value out of range"). */
export function rangeText(low: string, high: string): string {
  return `${low}${RANGE_MARK}${high}`;
}

/** Position in the note list (「3.1.0000」, the form of mock A-PianoRoll; ticks in 4 digits). Ticks in the song. */
export function notePositionText(tick: number, bars: Bars): string {
  const b = barBeatTick(tick, bars);
  return `${b.bar}.${b.beat}.${tickText(b.tick)}`;
}

/** Note length (「拍 . ティック」: 「4.0000」「0.1440」, the form of mock A-PianoRoll). */
export function noteLengthText(ticks: number, meter: Meter): string {
  const perBeat = ticksPerBeat(meter);
  const whole = Math.max(0, Math.floor(ticks));
  return `${Math.floor(whole / perBeat)}.${tickText(whole % perBeat)}`;
}
