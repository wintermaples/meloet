// The text of the chosen clip panel (spec 3 (93-4), mock F7-AudioEdit). Pure functions that do not know the DOM.
// Position 「7. 1. 0000」, length 「2. 0. 0000」 (bar . beat . tick, all from 0), fade 「0.0480」 (beat . tick; B-94-28).

import type { StretchView } from "../../bridge";
import { t } from "../../text";
import { signed } from "../pianoroll/geom";
import { parseNoteLength, parseNotePosition } from "../pianoroll/noteFields";
import { noteLengthText } from "../format";
import { tickText, ticksPerBar, ticksPerBeat, type Bars, type Meter } from "../time";

/** 「2. 0. 0000」: the clip length. */
export function clipLengthLabel(ticks: number, meter: Meter): string {
  const perBar = ticksPerBar(meter);
  const perBeat = ticksPerBeat(meter);
  const t = Math.max(0, Math.round(ticks));
  return `${Math.floor(t / perBar)}. ${Math.floor((t % perBar) / perBeat)}. ${tickText(t % perBeat)}`;
}

/** 「0.0480」: the fade length. */
export const fadeLengthText = (ticks: number, meter: Meter) => noteLengthText(ticks, meter);

/** Removes spaces from the typed text and makes full-width characters normal (so 「7. 1. 0000」 is read too). */
const tidy = (s: string) => s.normalize("NFKC").replace(/\s+/g, "");

/** Position 「7. 1. 0000」 (also 「7.1」, 「7」) to the tick in the song. null if it cannot be read. */
export function parseClipPosition(s: string, bars: Bars): number | null {
  return parseNotePosition(tidy(s), bars);
}

/**
 * Length 「2. 0. 0000」 (also 「2.2」, 「2」; bar . beat . tick) to ticks. With `beats`, beat . tick (fades).
 * null if it cannot be read. The range is not checked (Rust brings it inside).
 */
export function parseClipLength(s: string, meter: Meter, beats = false): number | null {
  const text = tidy(s);
  if (beats) return parseNoteLength(text, meter);
  const m = text.match(/^(\d{1,4})(?:\.(\d{1,2})(?:\.(\d{1,4}))?)?$/);
  if (!m) return null;
  const beat = m[2] === undefined ? 0 : Number(m[2]);
  const tick = m[3] === undefined ? 0 : Number(m[3]);
  if (beat >= meter.num || tick >= ticksPerBeat(meter)) return null;
  return Number(m[1]) * ticksPerBar(meter) + beat * ticksPerBeat(meter) + tick;
}

// ---- Stretch and pitch (the marks of spec 2 "Audio clip"; spec 3 "Stretch", "Pitch") ----

/** The number of steps for a drag of `dy` px up (1 for each `stepPx`; minus going down). */
export const dragSteps = (dy: number, stepPx: number) => (stepPx > 0 ? Math.trunc(dy / stepPx) : 0);

/** Typed semitones or cents (「+2」「−1」「3」). Decimals go to the nearest whole number. null if it cannot be read. The range is not checked. */
export function parseWhole(s: string): number | null {
  const text = tidy(s).replace(/−/g, "-");
  if (!/^[+-]?(\d+(\.\d*)?|\.\d+)$/.test(text)) return null;
  return Math.round(Number(text));
}

/** Typed 「この音のテンポ」 (「120」「96.5」). null if it cannot be read. Rust does the range and rounding. */
export function parseSourceBpm(s: string): number | null {
  const text = tidy(s);
  if (!/^(\d+(\.\d*)?|\.\d+)$/.test(text)) return null;
  const bpm = Number(text);
  return bpm > 0 ? bpm : null;
}

/** The number of the 「×1.25」 mark (2 decimals). null when it does not follow the tempo or comes to 1.00 (B-94-55). */
export function ratioMarkValue(ratio: number | null): string | null {
  if (ratio === null || !Number.isFinite(ratio)) return null;
  const text = ratio.toFixed(2);
  return text === "1.00" ? null : text;
}

/** The cents text (minus is U+2212; no +). */
export const centsText = (v: number) => (v < 0 ? `−${-v}` : String(v));

/**
 * The marks to the right of the clip name: 「×1.25」, and 「+2」「−1 ・ +30」 (semitones, cents, both with a sign; spec 3 "How the mark
 * reads"). Empty if there are none. No pitch mark when both semitones and cents are 0.
 */
export function clipMarks(stretch: StretchView): readonly string[] {
  if (stretch.ratio === null && stretch.semitones === 0 && stretch.cents === 0) return NO_MARKS;
  const out: string[] = [];
  const ratio = ratioMarkValue(stretch.ratio);
  if (ratio !== null) out.push(t("clip.ratioMark", { ratio }));
  if (stretch.cents !== 0) out.push(pitchReading(stretch.semitones, stretch.cents));
  else if (stretch.semitones !== 0) out.push(signed(stretch.semitones));
  return out.length > 0 ? out : NO_MARKS;
}

/** 「+2 ・ +30」「−1 ・ +30」「+0 ・ −20」: semitones and cents, both with a sign (spec 3 "How the mark reads"; the reading of spec 6 "Moving the pitch"). */
export const pitchReading = (semitones: number, cents: number) => t("clip.pitchMarkCents", { semitones: signed(semitones), cents: signed(cents) });

export const NO_MARKS: readonly string[] = [];
