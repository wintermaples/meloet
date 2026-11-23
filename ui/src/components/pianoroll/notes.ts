// Calculations for the note list and 「選んだノート」. Pure functions that do not know the DOM.
// A note is named by "take id + its index in that take (in start order, from 0)" (docs/architecture.md 3-1, item 8).

import type { ClipView, IndexRange, TakeNotes } from "../../bridge";
import { noteLengthText, notePositionText } from "../format";
import { noteName } from "../meterFormat";
import { meterAtOf } from "../../time/bars";
import type { Bars } from "../time";

type Notes = Pick<TakeNotes, "starts" | "lengths" | "keys" | "velocities">;
type Placed = Pick<ClipView, "start" | "end" | "takeOffset">;

/** The indexes of notes visible in the clip (starting inside the clip's range; the same as what sounds, and as the clip picture in the arrange). */
export function clipNoteIndexes(clip: Placed, take: { starts: ArrayLike<number>; lengths: ArrayLike<number> }): Uint32Array {
  const at = clip.start + clip.takeOffset;
  const out = new Uint32Array(take.starts.length);
  let n = 0;
  for (let i = 0; i < take.starts.length; i++) {
    const start = at + take.starts[i]!;
    if (start >= clip.end || start < clip.start) continue;
    out[n++] = i;
  }
  return out.subarray(0, n);
}

/** Whether index i is inside the selected ranges (in start order, not overlapping). */
export function inRanges(ranges: readonly IndexRange[], i: number): boolean {
  let lo = 0;
  let hi = ranges.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    const r = ranges[mid]!;
    if (i < r.start) hi = mid;
    else if (i >= r.end) lo = mid + 1;
    else return true;
  }
  return false;
}

export interface SelectedSummary {
  count: number;
  /** The earliest position (「3.1.000」). Adding 「〜」 for two or more is up to the side that shows it. */
  position: string;
  /** Note names in take order (「D4 E4 F4」; drum piece names for drums). When there are many, up to maxKeys (the side that shows it cuts what does not fit). */
  keys: string;
  /** 「0.360」 if all the same, null if not (「ばらばら」). */
  length: string | null;
  /** The smallest and largest velocity. */
  velocityLow: number;
  velocityHigh: number;
}

/**
 * A summary of the selected notes (the right panel 「選んだノート」 in mock A-PianoRoll). Null if nothing is selected.
 * Positions are in the song (the take's 0 is the clip's start + takeOffset).
 */
export function selectedSummary(
  take: Notes,
  clip: Placed,
  ranges: readonly IndexRange[],
  meter: Bars,
  maxKeys = 64,
  label: (key: number) => string = noteName,
): SelectedSummary | null {
  const n = take.starts.length;
  let count = 0;
  let first = Infinity;
  let length: number | null | undefined;
  let low = Infinity;
  let high = -Infinity;
  const names: string[] = [];
  for (const r of ranges) {
    for (let i = r.start; i < Math.min(r.end, n); i++) {
      count++;
      first = Math.min(first, take.starts[i]!);
      const len = take.lengths[i]!;
      length = length === undefined || length === len ? len : null;
      low = Math.min(low, take.velocities[i]!);
      high = Math.max(high, take.velocities[i]!);
      if (names.length < maxKeys) names.push(label(take.keys[i]!));
    }
  }
  if (count === 0) return null;
  return {
    count,
    position: notePositionText(clip.start + clip.takeOffset + first, meter),
    keys: names.join(" "),
    length: length === null || length === undefined ? null : noteLengthText(length, meterAtOf(meter, clip.start + clip.takeOffset + first)),
    velocityLow: low,
    velocityHigh: high,
  };
}
