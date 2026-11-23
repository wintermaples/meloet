// The sounds playing now: the notes of the shown clip that touch the position heard now. Pure functions that do not know the DOM.
// Used by 「鳴っている音 C3 E3 G3 C4」 in the status line (mock F-PRPlay) and by the lit keys of the piano roll's keyboard column (spec 3, the piano roll while playing).

import type { ClipView } from "../../bridge";

export interface SoundingTake {
  id: number;
  starts: ArrayLike<number>;
  lengths: ArrayLike<number>;
  keys: ArrayLike<number>;
}

/**
 * The keys sounding at `tick` (a song position), lowest first, without repeats. A note sounds in [start, start + length), and
 * not outside the clip (outside [clip.start, clip.end)). If the take is not the clip's take (while fetching), nothing sounds.
 */
export function soundingKeys(
  take: SoundingTake | null,
  clip: Pick<ClipView, "start" | "end" | "takeOffset" | "takeId"> | null,
  tick: number,
): number[] {
  if (!take || !clip || take.id !== clip.takeId || tick < clip.start || tick >= clip.end) return [];
  const local = tick - (clip.start + clip.takeOffset);
  const on = new Set<number>();
  // Notes are in start order, so nothing after one that starts later than now is checked.
  for (let i = 0; i < take.starts.length; i++) {
    const start = take.starts[i]!;
    if (start > local) break;
    // Notes moved before the clip's start by trimming the edge do not sound.
    if (start < -clip.takeOffset) continue;
    if (local < start + take.lengths[i]!) on.add(take.keys[i]!);
  }
  return [...on].sort((a, b) => a - b);
}

/** Whether two lists of keys are the same (so the text is rewritten only when it changes). */
export function sameKeys(a: readonly number[], b: readonly number[]): boolean {
  return a.length === b.length && a.every((k, i) => k === b[i]);
}
