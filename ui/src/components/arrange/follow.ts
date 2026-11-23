// Song tick → file frame for audio clips (the rules of `AudioClipView` in protocol 25, docs/architecture.md 22-19).
// Same formula and same answers as `FileClock` in crates/meloet-model/src/audio/follow.rs (matches follow.table.json;
// ui/tests/arrange.follow.test.ts). Knows neither the DOM nor Tauri.
//
// Tick 0 of the take (`clip.start + clip.takeOffset`) is `firstFrame`. After that, two ways:
//  - Not following (`sourceTempo` is null): the file plays at its own speed. Time is counted in seconds of the song map.
//  - Following: the audio sticks to song ticks. A song tick is a take tick as it is,
//    and the map of the audio's own tempo decides the file seconds. The song tempo does not appear in the formula.

import type { ClipView, SourceTempoView } from "../../bridge/types";
import { TempoMap } from "../../time/tempoMap";

/** The map of the audio's own tempo: ticks from tick 0 of the take → file seconds. No time signature. */
export function sourceMapOf(source: SourceTempoView): TempoMap {
  return new TempoMap({ tempoBpm: source.tempoBpm, headRamp: source.headRamp, tempoPoints: source.tempoPoints, meterNum: 4, meterDen: 4, meterChanges: [] });
}

const sourceMaps = new WeakMap<SourceTempoView, TempoMap>();

/**
 * Builds `sourceMapOf` only once for the same `SourceTempoView`. The same object comes until the song section arrives, so it is not built every frame.
 * null (not following) stays null.
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

/** Only what the formula needs (from `ClipView` and its `audio`). */
export interface FollowClip {
  start: number;
  takeOffset: number;
  firstFrame: number;
  sampleRate: number;
}

export const followClipOf = (clip: ClipView): FollowClip | null =>
  clip.audio === null ? null : { start: clip.start, takeOffset: clip.takeOffset, firstFrame: clip.audio.firstFrame, sampleRate: clip.audio.sampleRate };

/**
 * The file frame (fractional) heard at song tick `tick`. `source` is the map of the audio's own tempo for a following clip
 * (`sourceMapOf(clip.audio.sourceTempo)`, built once per clip and kept), or null when not following.
 * Silent before 0 and after the end of the take.
 */
export function clipFrameAt(song: TempoMap, source: TempoMap | null, clip: FollowClip, tick: number): number {
  const zero = clip.start + clip.takeOffset;
  const seconds = source === null ? song.secondsAt(tick) - song.secondsAt(zero) : source.secondsAt(tick - zero);
  return clip.firstFrame + seconds * clip.sampleRate;
}

/** Song tempo ÷ the audio's own tempo (at that point; 「×1.25」). null when not following. */
export function stretchRatioAt(song: TempoMap, source: TempoMap | null, clip: FollowClip, tick: number): number | null {
  return source === null ? null : song.tempoAt(tick) / source.tempoAt(tick - (clip.start + clip.takeOffset));
}
