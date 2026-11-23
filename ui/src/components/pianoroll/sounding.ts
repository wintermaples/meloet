// The sounds playing now: the notes of the shown clip that touch the position heard now. Pure functions that do not know the DOM.
// Used by 「鳴っている音 C3 E3 G3 C4」 in the status line (mock F-PRPlay), by the lit keys of the piano roll's keyboard column (spec 3, the piano roll while playing)
// and by the lit notes of the note area (round 12 E1, glow.ts).

import type { ClipView } from "../../bridge";

export interface SoundingTake {
  id: number;
  starts: ArrayLike<number>;
  lengths: ArrayLike<number>;
  keys: ArrayLike<number>;
}

type SoundingClip = Pick<ClipView, "start" | "end" | "takeOffset" | "takeId">;

/**
 * The keys sounding at `tick` (a song position), lowest first, without repeats. A note sounds in [start, start + length), and
 * not outside the clip (outside [clip.start, clip.end)). If the take is not the clip's take (while fetching), nothing sounds.
 */
export function soundingKeys(take: SoundingTake | null, clip: SoundingClip | null, tick: number): number[] {
  return keysOf(take, soundingNotes(take, clip, tick));
}

/**
 * The indexes of the notes sounding at `tick`, by the rules of `soundingKeys`, in index order. Written into `out` (emptied first)
 * when it is given, so a caller that asks every frame makes no new array.
 */
export function soundingNotes(take: SoundingTake | null, clip: SoundingClip | null, tick: number, out: number[] = []): number[] {
  out.length = 0;
  if (!take || !clip || take.id !== clip.takeId || tick < clip.start || tick >= clip.end) return out;
  const local = tick - (clip.start + clip.takeOffset);
  // Notes are in start order, so nothing after one that starts later than now is checked.
  for (let i = 0; i < take.starts.length; i++) {
    const start = take.starts[i]!;
    if (start > local) break;
    // Notes moved before the clip's start by trimming the edge do not sound.
    if (start < -clip.takeOffset) continue;
    if (local < start + take.lengths[i]!) out.push(i);
  }
  return out;
}

/** The keys of the notes `notes` of `take`, lowest first, without repeats. */
export function keysOf(take: SoundingTake | null, notes: readonly number[]): number[] {
  if (!take || notes.length === 0) return [];
  const on = new Set<number>();
  for (const i of notes) on.add(take.keys[i]!);
  return [...on].sort((a, b) => a - b);
}

/** Whether two lists of keys are the same (so the text is rewritten only when it changes). */
export function sameKeys(a: readonly number[], b: readonly number[]): boolean {
  return a.length === b.length && a.every((k, i) => k === b[i]);
}
