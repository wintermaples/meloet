// Typing in the note list and the 「選んだノート」 panel (spec 3, "Note list"). Pure functions that do not know the DOM.
// Reading and writing field text, the allowed ranges, the step for ↑ ↓, and the intents to send (SetNotesField, RejectValue, ClampValue).
// If it reads as a number but is out of range, it is set to the nearest end and decided (docs/history/design-notes.ja.md B-68). Only what cannot be read is "broken".

import type { ClipView, Command, NoteField, TakeNotes, ValueField } from "../../bridge";
import { noteLengthText, notePositionText, rangeText } from "../format";
import { noteName } from "../meterFormat";
import { barAt, barsMap, meterAtOf, perBeatOf, tickOfPosition, tickRangeText } from "../../time/bars";
import { ticksPerBeat, type Bars, type Meter } from "../time";
import { drumKeyOf, keyLabel, type Drums } from "./drums";

const kitOf = (d: Drums | boolean): Drums => (d === true ? "builtin" : d);

export type FieldName = "position" | "key" | "length" | "velocity";
/** Order from left to right (the order of the list's fields, the Tab order). */
export const FIELD_ORDER: readonly FieldName[] = ["position", "key", "length", "velocity"];
/** Fields of the drum note list (no 「長さ」 column; spec 3). */
export const DRUM_FIELD_ORDER: readonly FieldName[] = ["position", "key", "velocity"];

const NOTE_FIELDS: Record<FieldName, NoteField> = { position: "start", key: "key", length: "length", velocity: "velocity" };
const VALUE_FIELDS: Record<FieldName, ValueField> = { position: "position", key: "key", length: "length", velocity: "velocity" };

type Notes = Pick<TakeNotes, "starts" | "lengths" | "keys" | "velocities">;
type Placed = Pick<ClipView, "start" | "end" | "takeOffset">;

/** Normalizes typed text so full-width digits and signs can be read too. */
const tidy = (s: string) => s.normalize("NFKC").trim();

/** Position 「3.1.0000」 (also 「3.1」「3」) to ticks in the song. Null if it cannot be read. The range is not checked. */
export function parseNotePosition(s: string, bars: Bars): number | null {
  const m = tidy(s).match(/^(\d{1,4})(?:\.(\d{1,2})(?:\.(\d{1,4}))?)?$/);
  if (!m) return null;
  const bar = Number(m[1]);
  const beat = m[2] === undefined ? 1 : Number(m[2]);
  const tick = m[3] === undefined ? 0 : Number(m[3]);
  const map = barsMap(bars);
  if (bar < 1 || beat < 1) return null;
  const at = barAt(map, bar);
  if (beat > at.num || tick >= perBeatOf(at)) return null;
  return tickOfPosition(map, bar, beat, tick);
}

/** Length 「1.0960」 (beat . tick; also 「2」) to ticks. Null if it cannot be read. The range is not checked. */
export function parseNoteLength(s: string, meter: Meter): number | null {
  const m = tidy(s).match(/^(\d{1,4})(?:\.(\d{1,4}))?$/);
  if (!m) return null;
  const tick = m[2] === undefined ? 0 : Number(m[2]);
  if (tick >= ticksPerBeat(meter)) return null;
  return Number(m[1]) * ticksPerBeat(meter) + tick;
}

const PITCH: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };

/** Note name 「D#4」 (C4 = 60; also 「Eb4」「d#4」) to a key number. Null if it cannot be read or is outside 0 to 127. */
export function parseKeyName(s: string): number | null {
  const m = tidy(s).match(/^([A-Ga-g])([#b]?)(-?\d{1,2})$/);
  if (!m) return null;
  const n = (Number(m[3]) + 1) * 12 + PITCH[m[1]!.toUpperCase()]! + (m[2] === "#" ? 1 : m[2] === "b" ? -1 : 0);
  return n >= 0 && n <= 127 ? n : null;
}

// ---- Also reads out-of-range values (B-68). Returns what was rounded (bar, beat, tick) ----

/** How to say what was rounded (field and range of ClampValue). */
export interface ClampedUnit {
  field: ValueField;
  range: string | null;
}

export interface LooseValue {
  value: number;
  /** If any of bar, beat or tick was put into range, the largest unit. */
  unit: ClampedUnit | null;
}

/** Normalizes with NFKC, and the math − to - too. */
const tidyMinus = (s: string) => tidy(s).replaceAll("\u2212", "-");
const clampTo = (v: number, low: number, high: number) => Math.min(high, Math.max(low, v));

/** Also reads positions 「3.5.0000」「0.1.0000」「3.1.9999」, and puts the bar from 1, the beat in 1 to the time signature's top, and the tick in 0 to one beat's ticks − 1. */
export function readNotePositionLoose(s: string, bars: Bars): LooseValue | null {
  const m = tidyMinus(s).match(/^(-?\d{1,4})(?:\.(\d{1,2})(?:\.(\d{1,4}))?)?$/);
  if (!m) return null;
  const rawBar = Number(m[1]);
  const rawBeat = m[2] === undefined ? 1 : Number(m[2]);
  const rawTick = m[3] === undefined ? 0 : Number(m[3]);
  const bar = Math.max(1, rawBar);
  const map = barsMap(bars);
  const at = barAt(map, bar);
  const num = at.num;
  const perBeat = perBeatOf(at);
  const beat = clampTo(rawBeat, 1, num);
  const tick = clampTo(rawTick, 0, perBeat - 1);
  let unit: ClampedUnit | null = null;
  if (bar !== rawBar) unit = { field: "bar", range: null };
  else if (beat !== rawBeat) unit = { field: "beat", range: rangeText("1", String(num)) };
  else if (tick !== rawTick) unit = { field: "tick", range: tickRangeText(perBeat) };
  return { value: tickOfPosition(map, bar, beat, tick), unit };
}

/** Also reads a length 「1.9999」, and puts the tick in 0 to one beat's ticks − 1. */
export function readNoteLengthLoose(s: string, meter: Meter): LooseValue | null {
  const m = tidy(s).match(/^(\d{1,4})(?:\.(\d{1,4}))?$/);
  if (!m) return null;
  const perBeat = ticksPerBeat(meter);
  const rawTick = m[2] === undefined ? 0 : Number(m[2]);
  const tick = Math.min(rawTick, perBeat - 1);
  const unit: ClampedUnit | null = tick !== rawTick ? { field: "tick", range: tickRangeText(perBeat) } : null;
  return { value: Number(m[1]) * perBeat + tick, unit };
}

/** Note names to key numbers, also outside 0 to 127 (「C10」「C-2」). */
export function readKeyNameLoose(s: string): LooseValue | null {
  const m = tidyMinus(s).match(/^([A-Ga-g])([#b]?)(-?\d{1,2})$/);
  if (!m) return null;
  const octave = Number(m[3]);
  return { value: (octave + 1) * 12 + PITCH[m[1]!.toUpperCase()]! + (m[2] === "#" ? 1 : m[2] === "b" ? -1 : 0), unit: null };
}

/** Velocity. Also reads negative numbers and decimals; decimals are rounded. */
export function readVelocityLoose(s: string): LooseValue | null {
  const m = tidyMinus(s).match(/^(-?)(\d{1,6}(?:\.\d+)?|\.\d+)$/);
  if (!m) return null;
  const v = Math.round(Number(m[2]));
  return { value: m[1] ? -v : v, unit: null };
}

/** Velocity (integer). Null if it cannot be read. The range is not checked. */
export function parseVelocity(s: string): number | null {
  const m = tidy(s).match(/^\d{1,3}$/);
  return m ? Number(m[0]) : null;
}

/** The rules for one input box (note fields and CC list fields). */
export interface FieldSpec {
  /** The field the status line names (field of RejectValue and ClampValue). */
  field: ValueField;
  /** The text put in when the box opens. */
  initial: string;
  /** 「もとの {kept} のまま」 when an unusable value is typed. Null when mixed (the text shown in the field is used). */
  kept: string | null;
  /** The current value. Null when mixed (any value decided would change something). */
  current: number | null;
  low: number;
  high: number;
  /** One step of ↑ ↓. */
  unit: number;
  /** How much Shift ↑ ↓ moves (times unit). 12 semitones for the note field, 10 for the others. */
  shift: number;
  /** The range text for the status line (position, length, note). Null for velocity, since it is screen text (value.velocityRange). */
  range: string | null;
  format: (v: number) => string;
  read: (s: string) => number | null;
  /** Also reads out-of-range values (B-68). Null if it cannot be read. */
  loose: (s: string) => LooseValue | null;
}

export interface NoteFieldSpec extends FieldSpec {
  name: FieldName;
}

/** The value to decide, and how to say what was rounded if it was (the content of ClampValue). */
export interface FieldRead {
  value: number;
  clamped: { field: ValueField; value: string; range: string | null } | null;
}

/**
 * Typed text to the value to decide. Null if it cannot be read as a number ("broken"). If out of range, the nearest end, and returns what was rounded.
 * When two or more were rounded, only the largest unit (clip range → bar → beat → tick).
 */
export function readField(spec: FieldSpec, s: string): FieldRead | null {
  const r = spec.loose(s);
  if (!r) return null;
  const value = clampTo(r.value, spec.low, spec.high);
  if (value !== r.value) return { value, clamped: { field: spec.field, value: spec.format(value), range: spec.range } };
  return { value, clamped: r.unit ? { field: r.unit.field, value: spec.format(value), range: r.unit.range } : null };
}

/** The intent that makes the status line say it was rounded (sent after the deciding intent). */
export function clampCommand(clamped: NonNullable<FieldRead["clamped"]>): Command {
  return { type: "clampValue", field: clamped.field, value: clamped.value, range: clamped.range };
}

/** A value in range. Null if it cannot be read or is out of range. */
export function parseField(spec: FieldSpec, s: string): number | null {
  const v = spec.read(s);
  return v !== null && v >= spec.low && v <= spec.high ? v : null;
}

/** The value moved by delta (1 or spec.shift; negative for down) with ↑ ↓. It does not go out of range. */
export function stepField(spec: FieldSpec, v: number, delta: number): number {
  return Math.min(spec.high, Math.max(spec.low, v + delta * spec.unit));
}

/**
 * Typing rules for field name of notes (indexes in the take, one or more). For two or more, as SetNotesField means:
 * position moves the earliest note and note moves the lowest note, keeping the others' distances. Length and velocity set all to the same value.
 * The value put in the box: for position the earliest position, for note the lowest note, for length the shared value (if mixed, the earliest note's length),
 * for velocity the shared value (if mixed, the smallest value).
 * With drums, notes are shown as drum piece names, and both piece names and note names are read.
 */
export function noteFieldSpec(
  name: FieldName,
  take: Notes,
  clip: Placed,
  notes: readonly number[],
  bars: Bars,
  drums: Drums | boolean = false,
): NoteFieldSpec | null {
  const picked = notes.filter((i) => i < take.starts.length);
  if (picked.length === 0) return null;
  const at = clip.start + clip.takeOffset;
  const starts = picked.map((i) => at + take.starts[i]!);
  const first = picked[starts.indexOf(Math.min(...starts))]!;
  // Length and the tick scale are counted in the time signature at the earliest note.
  const meter = meterAtOf(bars, Math.min(...starts));
  // The lowest for ↑ ↓ and for length is 1 tick (the lowest digit shown).
  const unit = 1;
  switch (name) {
    case "position": {
      const firstStart = Math.min(...starts);
      const lastEnd = Math.max(...picked.map((i) => at + take.starts[i]! + take.lengths[i]!));
      const low = clip.start;
      const high = Math.max(low, clip.end - (lastEnd - firstStart));
      const format = (v: number) => notePositionText(v, bars);
      return {
        name,
        field: VALUE_FIELDS[name],
        shift: 10,
        initial: format(firstStart),
        kept: format(firstStart),
        current: firstStart,
        low,
        high,
        unit,
        range: rangeText(format(low), format(high)),
        format,
        read: (s) => parseNotePosition(s, bars),
        loose: (s) => readNotePositionLoose(s, bars),
      };
    }
    case "key": {
      const keys = picked.map((i) => take.keys[i]!);
      const lowest = Math.min(...keys);
      const high = 127 - (Math.max(...keys) - lowest);
      const format = (k: number) => keyLabel(k, drums);
      return {
        name,
        field: VALUE_FIELDS[name],
        shift: 12,
        initial: format(lowest),
        kept: format(lowest),
        current: lowest,
        low: 0,
        high,
        unit: 1,
        range: rangeText(noteName(0), noteName(high)),
        format,
        read: drums ? (t) => drumKeyOf(t, kitOf(drums)) ?? parseKeyName(t) : parseKeyName,
        loose: drums
          ? (t) => {
              const k = drumKeyOf(t, kitOf(drums));
              return k === null ? readKeyNameLoose(t) : { value: k, unit: null };
            }
          : readKeyNameLoose,
      };
    }
    case "length": {
      const lengths = picked.map((i) => take.lengths[i]!);
      const same = lengths.every((l) => l === lengths[0]);
      const high = Math.max(unit, clip.end - Math.max(...starts));
      const format = (v: number) => noteLengthText(v, meter);
      const initial = format(same ? lengths[0]! : take.lengths[first]!);
      return {
        name,
        field: VALUE_FIELDS[name],
        shift: 10,
        initial,
        kept: same ? initial : null,
        current: same ? lengths[0]! : null,
        low: unit,
        high,
        unit,
        range: rangeText(format(unit), format(high)),
        format,
        read: (s) => parseNoteLength(s, meter),
        loose: (s) => readNoteLengthLoose(s, meter),
      };
    }
    case "velocity": {
      const velocities = picked.map((i) => take.velocities[i]!);
      const low = Math.min(...velocities);
      const same = velocities.every((v) => v === low);
      return {
        name,
        field: VALUE_FIELDS[name],
        shift: 10,
        initial: String(low),
        kept: same ? String(low) : null,
        current: same ? low : null,
        low: 1,
        high: 127,
        unit: 1,
        range: null,
        format: String,
        read: parseVelocity,
        loose: readVelocityLoose,
      };
    }
  }
}

/** The intent for the decided value. Null if the value did not change (no undo step is made). */
export function commitCommand(spec: NoteFieldSpec, value: number, take: number, notes: readonly number[]): Command | null {
  if (spec.current !== null && value === spec.current) return null;
  return { type: "setNotesField", take, notes: [...notes], field: NOTE_FIELDS[spec.name], value };
}

/** Enter (Tab) was pressed with an unusable value. shown is the text seen in the field (「ばらばら」「77 – 83」 when mixed). */
export function rejectCommand(spec: FieldSpec, shown: string): Command {
  return { type: "rejectValue", field: spec.field, kept: spec.kept ?? shown, range: spec.range };
}

/** The field to open next with Tab (Shift Tab if back). row is ±1 if it leaves the current row. order is the field order (the drum list has no length). */
export function nextField(name: FieldName, back: boolean, order: readonly FieldName[] = FIELD_ORDER): { name: FieldName; row: -1 | 0 | 1 } {
  const i = order.indexOf(name) + (back ? -1 : 1);
  if (i < 0) return { name: order[order.length - 1]!, row: -1 };
  if (i >= order.length) return { name: order[0]!, row: 1 };
  return { name: order[i]!, row: 0 };
}
