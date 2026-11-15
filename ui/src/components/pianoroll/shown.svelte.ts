// ピアノロールが見せているクリップと、そのテイクの中身（ノート）。ピアノロールの部品（ノートの場所、ベロシティの帯、
// ノートの一覧、選んだノート、鍵盤の列）はみな、ここを読む。テイクは id でためてある物を `takeNotes` で取る（docs/architecture.md 3-1 の 8）。
//
// 使いかた: ピアノロールの帯の部品が、初めに 1 回 `followShownTake()` を呼ぶ（部品の中なので $effect が使える）。

import { takeNotes, type ClipView, type TakeNotes, type TrackView } from "../../bridge";
import { current } from "../../state/app.svelte";

// $state.raw: テイクの中身は大きな型つき配列なので、深い proxy にしない。
let clip = $state.raw<ClipView | null>(null);
let track = $state.raw<TrackView | null>(null);
let take = $state.raw<TakeNotes | null>(null);
let origin = $state.raw<TakeNotes | null>(null);

export const shown = {
  /** `arrange.pianoRollClip` のクリップ。なければ null（クリップのない曲）。 */
  get clip(): ClipView | null {
    return clip;
  },
  /** そのクリップのトラック。 */
  get track(): TrackView | null {
    return track;
  },
  /** 見せているテイクの中身。取りにいっている間は、前の物のまま（ちらつかないように）。id は `take.id`。 */
  get take(): TakeNotes | null {
    return take;
  },
  /** クオンタイズしたテイクの、もとのテイクの中身（そろえる前の位置の枠を描くため）。なければ null。 */
  get origin(): TakeNotes | null {
    return origin;
  },
};

function findClip(id: number | null): { clip: ClipView; track: TrackView } | null {
  const s = current.state;
  if (!s || id === null) return null;
  for (const track of s.song.tracks) {
    const clip = track.clips.find((c) => c.id === id);
    if (clip) return { clip, track };
  }
  return null;
}

/** 部品の初めに 1 回呼ぶ。状態が変わるたびに `shown` を合わせる。 */
export function followShownTake(): void {
  let asked = 0;
  $effect(() => {
    const found = findClip(current.state?.arrange.pianoRollClip ?? null);
    clip = found?.clip ?? null;
    track = found?.track ?? null;
    const id = found?.clip.takeId ?? null;
    const ticket = ++asked;
    if (id === null) {
      take = null;
      origin = null;
      return;
    }
    // つながる前は取りにいけない（橋が投げる）。つながったら、この $effect がもう一度走る。
    if (take?.id === id || !current.connected) return;
    void takeNotes(id).then(async (got) => {
      if (ticket !== asked) return;
      const from = got.origin !== 0 ? await takeNotes(got.origin) : null;
      if (ticket !== asked) return;
      take = got;
      origin = from;
    });
  });
}
