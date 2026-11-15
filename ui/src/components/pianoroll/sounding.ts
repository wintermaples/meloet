// いま鳴っている音: 見せているクリップのノートのうち、耳に届いている位置にかかっている物。DOM を知らない純粋な関数。
// 状態行の「鳴っている音 C3 E3 G3 C4」（モック F-PRPlay）と、ピアノロールの鍵盤の列の光り（仕様書 3 章「再生中のピアノロール」）が使う。

import type { ClipView } from "../../bridge";

export interface SoundingTake {
  id: number;
  starts: ArrayLike<number>;
  lengths: ArrayLike<number>;
  keys: ArrayLike<number>;
}

/**
 * `tick`（曲の中の位置）で鳴っている鍵を、低い順に重なりなく。ノートは [はじまり, はじまり + 長さ) で鳴り、
 * クリップの外（[clip.start, clip.end) の外）では鳴らない。テイクがクリップのテイクでなければ（取りにいっている間）、何も鳴らない。
 */
export function soundingKeys(
  take: SoundingTake | null,
  clip: Pick<ClipView, "start" | "end" | "takeOffset" | "takeId"> | null,
  tick: number,
): number[] {
  if (!take || !clip || take.id !== clip.takeId || tick < clip.start || tick >= clip.end) return [];
  const local = tick - (clip.start + clip.takeOffset);
  const on = new Set<number>();
  // ノートははじまりの順なので、いまより後に始まる物から先は見ない。
  for (let i = 0; i < take.starts.length; i++) {
    const start = take.starts[i]!;
    if (start > local) break;
    if (local < start + take.lengths[i]!) on.add(take.keys[i]!);
  }
  return [...on].sort((a, b) => a - b);
}

/** 2 つの鍵の並びが同じか（文を書きかえるのは、変わったときだけにするため）。 */
export function sameKeys(a: readonly number[], b: readonly number[]): boolean {
  return a.length === b.length && a.every((k, i) => k === b[i]);
}
