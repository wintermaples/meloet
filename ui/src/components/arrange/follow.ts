// 歌のクリップの「曲のティック → ファイルのこま」（約束 25 の `AudioClipView` の決まり、docs/architecture.md 22-19）。
// crates/meloet-model/src/audio/follow.rs の `FileClock` と同じ式で、同じ答えを出す（follow.table.json と合う。
// ui/tests/arrange.follow.test.ts）。DOM も Tauri も知らない。
//
// テイクの 0 ティック（`clip.start + clip.takeOffset`）が `firstFrame`。そこから先は 2 通り:
//  - ついていかない（`sourceTempo` が null）: ファイルの速さのまま。間は、曲の地図の秒で数える。
//  - ついていく: 音は曲のティックに貼りつく。曲のティックがそのままテイクのティックで、
//    「この音のテンポ」の地図がファイルの秒を決める。曲のテンポは式に出てこない。

import type { ClipView, SourceTempoView } from "../../bridge/types";
import { TempoMap } from "../../time/tempoMap";

/** 「この音のテンポ」の地図: テイクの 0 ティックからのティック → ファイルの秒。拍子は使わない。 */
export function sourceMapOf(source: SourceTempoView): TempoMap {
  return new TempoMap({ tempoBpm: source.tempoBpm, headRamp: source.headRamp, tempoPoints: source.tempoPoints, meterNum: 4, meterDen: 4, meterChanges: [] });
}

const sourceMaps = new WeakMap<SourceTempoView, TempoMap>();

/**
 * `sourceMapOf` を、同じ `SourceTempoView` には 1 回だけ作る。曲の区画が届くまで同じ物が来るので、毎コマは作らない。
 * null（ついていかない）は null。
 */
export function cachedSourceMap(source: SourceTempoView | null): TempoMap | null {
  if (source === null) return null;
  let map = sourceMaps.get(source);
  if (!map) {
    map = sourceMapOf(source);
    sourceMaps.set(source, map);
  }
  return map;
}

/** 式に要る所だけ（`ClipView` と、その `audio` から）。 */
export interface FollowClip {
  start: number;
  takeOffset: number;
  firstFrame: number;
  sampleRate: number;
}

export const followClipOf = (clip: ClipView): FollowClip | null =>
  clip.audio === null ? null : { start: clip.start, takeOffset: clip.takeOffset, firstFrame: clip.audio.firstFrame, sampleRate: clip.audio.sampleRate };

/**
 * 曲のティック `tick` で鳴る、ファイルのこま（小数）。`source` は、ついていくクリップの「この音のテンポ」の地図
 * （`sourceMapOf(clip.audio.sourceTempo)`。クリップごとに 1 回作って持っておく）、ついていかなければ null。
 * 0 より前と、テイクの終わりより先は音なし。
 */
export function clipFrameAt(song: TempoMap, source: TempoMap | null, clip: FollowClip, tick: number): number {
  const zero = clip.start + clip.takeOffset;
  const seconds = source === null ? song.secondsAt(tick) - song.secondsAt(zero) : source.secondsAt(tick - zero);
  return clip.firstFrame + seconds * clip.sampleRate;
}

/** 曲のテンポ ÷ この音のテンポ（その場の数。「×1.25」）。ついていかなければ null。 */
export function stretchRatioAt(song: TempoMap, source: TempoMap | null, clip: FollowClip, tick: number): number | null {
  return source === null ? null : song.tempoAt(tick) / source.tempoAt(tick - (clip.start + clip.takeOffset));
}
