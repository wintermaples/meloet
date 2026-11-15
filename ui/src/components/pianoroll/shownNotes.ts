// 見せているテイクを、曲の中の位置で読む形（geom.ts の Shown）にする。同じテイクと位置なら作りなおさない。

import type { ClipView, TakeNotes } from "../../bridge";
import { shownOf, type Shown } from "./geom";

const memo = new WeakMap<TakeNotes, Shown>();

function made(take: TakeNotes, base: number): Shown {
  const had = memo.get(take);
  if (had && had.base === base) return had;
  const next = shownOf(take, base);
  memo.set(take, next);
  return next;
}

/** 新しいテイクを取りにいっている間は、前のテイクのまま（shown.svelte.ts と同じく、ちらつかないように）。 */
export function shownFor(take: TakeNotes | null, clip: ClipView | null): Shown | null {
  if (!take || !clip) return null;
  return made(take, clip.start + clip.takeOffset);
}

/** もとのテイク（クオンタイズしたテイクの、そろえる前）。見せているテイクと同じ位置で読む。 */
export function originFor(origin: TakeNotes | null, shown: Shown | null): Shown | null {
  return origin && shown ? made(origin, shown.base) : null;
}
