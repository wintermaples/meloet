// Turns the shown take into the form read at song positions (Shown in geom.ts). Not made again for the same take and position.

import type { ClipView, TakeNotes } from "../../bridge";
import { shownOf, type Shown } from "./geom";

const memo = new WeakMap<TakeNotes, Shown>();

function made(take: TakeNotes, base: number, from: number, to: number): Shown {
  const had = memo.get(take);
  if (had && had.base === base && had.from === from && had.to === to) return had;
  const next = shownOf(take, base, { from, to });
  memo.set(take, next);
  return next;
}

/** While a new take is being fetched, the previous take stays (the same as shown.svelte.ts, so it does not flicker). */
export function shownFor(take: TakeNotes | null, clip: ClipView | null): Shown | null {
  if (!take || !clip) return null;
  return made(take, clip.start + clip.takeOffset, clip.start, clip.end);
}

/** The original take (of a quantized take, before aligning). Read with the same position and range as the shown take. */
export function originFor(origin: TakeNotes | null, shown: Shown | null): Shown | null {
  return origin && shown ? made(origin, shown.base, shown.from, shown.to) : null;
}
