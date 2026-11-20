// ノートの一覧と「選んだノート」の計算。DOM を知らない純粋な関数。
// ノートは「テイクの id ＋ そのテイクの中の番号（はじまりの順、0 から）」で指す（docs/architecture.md 3-1 の 8）。

import type { ClipView, IndexRange, TakeNotes } from "../../bridge";
import { noteLengthText, notePositionText } from "../format";
import { noteName } from "../meterFormat";
import { meterAtOf } from "../../time/bars";
import type { Bars } from "../time";

type Notes = Pick<TakeNotes, "starts" | "lengths" | "keys" | "velocities">;
type Placed = Pick<ClipView, "start" | "end" | "takeOffset">;

/** クリップの中に見えているノートの番号（始まりがクリップの範囲の中。鳴る物と同じ。アレンジのクリップの絵とも同じ）。 */
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

/** 番号 i が、選んだ範囲（はじまりの順、重ならない）に入っているか。 */
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
  /** いちばん早い位置（「3.1.000」）。2 つ以上なら「〜」をつけるのは見せる側。 */
  position: string;
  /** 音名をテイクの順に（「D4 E4 F4」。ドラムは楽器の名前）。多いときは maxKeys まで（欄に入りきらない分は見せる側が切る）。 */
  keys: string;
  /** そろっていれば「0.360」、ちがえば null（「ばらばら」）。 */
  length: string | null;
  /** いちばん小さい・大きいベロシティ。 */
  velocityLow: number;
  velocityHigh: number;
}

/**
 * 選んだノートのまとめ（モック A-PianoRoll の右の欄「選んだノート」）。何も選んでいなければ null。
 * 位置は曲の中（テイクの 0 がクリップの start + takeOffset）。
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
