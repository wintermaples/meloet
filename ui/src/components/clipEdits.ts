// When joining and cutting clips can be used (spec 3 "Rules added in round 10", 7 A-27). No DOM.
// The context menus, the menu bar and Mod J share these rules; Rust decides what is joined, these only disable rows.

import type { AppState, ClipView, TrackView } from "../bridge";

/** Two or more keyboard clips of one track, with no clip that is not chosen between them. */
export function joinable(tracks: readonly TrackView[], ids: readonly number[]): boolean {
  const picked = new Set(ids);
  if (picked.size < 2) return false;
  const track = tracks.find((t) => t.clips.some((c) => picked.has(c.id)));
  if (!track) return false;
  const clips = track.clips.filter((c) => picked.has(c.id));
  if (clips.length !== picked.size || clips.some((c) => c.audio !== null)) return false;
  const start = Math.min(...clips.map((c) => c.start));
  const end = Math.max(...clips.map((c) => c.end));
  return !track.clips.some((c) => !picked.has(c.id) && c.start < end && c.end > start);
}

/** The clips Mod J and 「編集 ▸ クリップをつなげる」 send; null when they cannot be joined. */
export function joinTarget(state: AppState): number[] | null {
  const ids = state.arrange.selectedClips;
  return joinable(state.song.tracks, ids) ? [...ids] : null;
}

/** 「ここで切る」: the snapped place, when it is inside the clip (not at an end). */
export function splitTick(clip: ClipView, snapped: number): number | null {
  return clip.start < snapped && snapped < clip.end ? snapped : null;
}

/** The clip F2 renames: the one clicked last, else the first chosen. */
export function renameClipTarget(state: AppState): number | null {
  const a = state.arrange;
  if (a.selectedClips.length === 0) return null;
  return a.selectedClip !== null && a.selectedClips.includes(a.selectedClip) ? a.selectedClip : a.selectedClips[0]!;
}

/** While overdubbing (「重ねる」 on), the name of the clip the keyboard recording goes into; null when it makes a new clip. */
export function overdubClipName(tracks: readonly TrackView[], recording: { track: number; clip: number | null }): string | null {
  if (recording.clip === null) return null;
  const track = tracks.find((t) => t.id === recording.track);
  if (!track || track.kind.kind !== "keys") return null;
  return track.clips.find((c) => c.id === recording.clip)?.name ?? null;
}
