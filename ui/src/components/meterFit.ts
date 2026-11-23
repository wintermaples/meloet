// How the input meter fits across (spec 1 "Sizes" size.meter.name and size.meter.name.slim, "Rules for the scale",
// spec 3 "Keyboard names are also shown as the real names..."). All widths are CSS px at the current scale.
import { partAt, size, type Scale } from "../design/tokens";

/** The value name used for the width of the name field. The CSS variable is `--` plus the name with dots turned into hyphens. */
export type NameWidth =
  | "size.meter.name"
  | "part.meter.name.long"
  | "size.meter.name.slim"
  | "part.meter.name.slim-narrow";

/** When full, the name field widens if the name is longer than this many characters. */
export const LONG_NAME_CHARS = 8;

export interface MeterFitInput {
  meter: "full" | "slim";
  scale: Scale;
  /** Number of characters in the keyboard name. */
  nameChars: number;
  /** The usable width in the row. */
  available: number;
  /** Width of all parts and gaps, except the name, pitch bend and modulation. */
  rest: number;
  /** When slim: the width of the link mark and the name, not cut. */
  name: number;
  /** Widths of pitch bend and modulation (with the gap before). */
  pitchBend: number;
  modulation: number;
  /**
   * Only when full: the width saved by narrow keys in the keyboard picture, saved by leaving out the instrument name of the target,
   * saved by leaving out the 「弾くと鳴る先」 label, and the width of sustain (with the gap before).
   */
  narrowKeys?: number;
  instrument?: number;
  targetLabel?: number;
  sustain?: number;
  /** Width of each sentence when a hint is shown at the right end. rest includes all the sentences. */
  sentences?: readonly number[];
  /** Widths of 「キーで弾く」 and then of its octave, each with the gap before (R2). rest includes them. */
  typed?: readonly number[];
}

export interface MeterFit {
  nameWidth: NameWidth;
  pitchBend: boolean;
  modulation: boolean;
  /** Draw the keyboard picture with narrow keys (white key width 8). */
  narrowKeys: boolean;
  /** Show the instrument name in the target value (the track is always shown). */
  instrument: boolean;
  /** Show the 「弾くと鳴る先」 label. */
  targetLabel: boolean;
  sustain: boolean;
  /** How many hint sentences to show from the front. The first stays; if it does not fit, the screen cuts its end with 「…」. */
  sentences: number;
  /** How many of `typed` to show, from the front (R2): only in the room the rest leaves. */
  typed: number;
}

type Step = "modulation" | "pitchBend" | "narrowKeys" | "instrument" | "targetLabel" | "sentence" | "sustain";

/** The order of hiding when full (spec 1 "Rules for the scale"). sentence drops the 2nd sentence and later, one at a time from the end. */
const TARGET_ORDER: readonly Step[] = ["modulation", "pitchBend", "narrowKeys", "instrument", "targetLabel", "sustain"];
const MESSAGE_ORDER: readonly Step[] = ["modulation", "pitchBend", "narrowKeys", "sentence", "sustain"];

export function nameWidthPx(name: NameWidth, scale: Scale): number {
  const i = scale === 100 ? 0 : scale === 125 ? 1 : 2;
  return name === "size.meter.name" || name === "size.meter.name.slim" ? size[name][i] : partAt(name, scale);
}

/**
 * Which readings to show, and the width of the name field.
 * When slim: hide modulation, then pitch bend; if still not enough, lower the name limit from 176 to 128.
 * After that, hide the hint sentences from the 2nd on.
 * When full with 「弾くと鳴る先」 at the right end: modulation → pitch bend → narrow keys → instrument name →
 * the 「弾くと鳴る先」 label → sustain.
 * When full with a hint at the right end: modulation → pitch bend → narrow keys → hint sentences from the 2nd on → sustain.
 * A name or hint that still does not fit is cut with 「…」.
 * 「キーで弾く」 and its octave came after these rules: they are fitted as if they were not there, and shown only in the room
 * left (R2), so they never push out what the spec shows.
 */
export function meterFit(input: MeterFitInput): MeterFit {
  const { meter, scale, available, pitchBend, modulation } = input;
  const typedWidths = input.typed ?? [];
  const rest = typedWidths.reduce((r, w) => r - w, input.rest);
  const typedFits = (need: number) => {
    let shown = 0;
    for (const w of typedWidths) {
      if (need + w > available) break;
      need += w;
      shown += 1;
    }
    return shown;
  };
  const sentenceWidths = input.sentences ?? [];
  let sentences = sentenceWidths.length;
  const dropSentence = (need: number) => need - sentenceWidths[--sentences]!;
  if (meter === "full") {
    const first: NameWidth = input.nameChars > LONG_NAME_CHARS ? "part.meter.name.long" : "size.meter.name";
    const saved: Record<Exclude<Step, "sentence">, number> = {
      modulation,
      pitchBend,
      narrowKeys: input.narrowKeys ?? 0,
      instrument: input.instrument ?? 0,
      targetLabel: input.targetLabel ?? 0,
      sustain: input.sustain ?? 0,
    };
    const fit: MeterFit = {
      nameWidth: first,
      pitchBend: true,
      modulation: true,
      narrowKeys: false,
      instrument: true,
      targetLabel: true,
      sustain: true,
      sentences,
      typed: 0,
    };
    let need = rest + nameWidthPx(first, scale) + pitchBend + modulation;
    for (const step of input.sentences ? MESSAGE_ORDER : TARGET_ORDER) {
      if (step === "sentence") {
        while (need > available && sentences > 1) need = dropSentence(need);
        continue;
      }
      if (need <= available) break;
      if (step === "narrowKeys") fit.narrowKeys = true;
      else fit[step] = false;
      need -= saved[step];
    }
    fit.sentences = sentences;
    fit.typed = typedFits(need);
    return fit;
  }
  const width = (w: NameWidth, bend: boolean, mod: boolean) =>
    rest + Math.min(input.name, nameWidthPx(w, scale)) + (bend ? pitchBend : 0) + (mod ? modulation : 0);
  const slim = (nameWidth: NameWidth, pitchBend: boolean, modulation: boolean): MeterFit => {
    let need = width(nameWidth, pitchBend, modulation);
    while (need > available && sentences > 1) need = dropSentence(need);
    return {
      nameWidth,
      pitchBend,
      modulation,
      narrowKeys: false,
      instrument: true,
      targetLabel: true,
      sustain: true,
      sentences,
      typed: typedFits(need),
    };
  };
  const fits = (w: NameWidth, bend: boolean, mod: boolean) => width(w, bend, mod) <= available;
  const first: NameWidth = "size.meter.name.slim";
  if (fits(first, true, true)) return slim(first, true, true);
  if (fits(first, true, false)) return slim(first, true, false);
  if (fits(first, false, false)) return slim(first, false, false);
  return slim("part.meter.name.slim-narrow", false, false);
}

/** Splits a text after 「。」. */
export function sentences(text: string): string[] {
  return text.match(/[^。]+。?/g) ?? [text];
}

/** Number of characters (a surrogate pair counts as 1). */
export function charCount(text: string): number {
  return [...text].length;
}
