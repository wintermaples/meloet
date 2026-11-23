// 「音のクリップにする」 (round 12, A7) on the arrange: the row of a keys clip's right-click menu.

import type { TrackView } from "../../bridge";

/** The keys clips among `ids` are all on one track (at least one): Rust takes only those, and nothing for two tracks. */
export function bounceable(tracks: readonly TrackView[], ids: readonly number[]): boolean {
  const picked = new Set(ids);
  return tracks.filter((t) => t.clips.some((c) => c.audio === null && picked.has(c.id))).length === 1;
}
