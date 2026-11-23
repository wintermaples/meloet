// Which clip's band 「いいとこ取り」 is open (round 12, A8). It belongs to the screen only: one band at a time, opened
// from the clip's right-click menu, closed with the ▾ of its heading or when the clip is gone or has one take left.

import type { AppState, AudioTakeView, ClipView, TrackView } from "../../bridge";
import { compable, rowTakes } from "./comp";

export const compBand = $state({ clip: null as number | null });

export function openComp(clip: number): void {
  compBand.clip = clip;
}

export function closeComp(): void {
  compBand.clip = null;
}

export interface OpenComp {
  track: TrackView;
  clip: ClipView;
  /** The takes that have a row, the newest first. */
  takes: AudioTakeView[];
}

/** The band that is open, when its clip is still one it can be open for. */
export function openCompOf(state: AppState, clip: number | null = compBand.clip): OpenComp | null {
  if (clip === null) return null;
  for (const track of state.song.tracks) {
    const found = track.clips.find((c) => c.id === clip);
    if (found) return compable(found) ? { track, clip: found, takes: rowTakes(found.audio!.takes) } : null;
  }
  return null;
}
