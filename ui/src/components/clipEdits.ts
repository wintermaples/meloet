// When joining, cutting and pasting clips can be used (spec 3 "Rules added in round 10", 7 A-27). No DOM.
// The context menus, the menu bar and the keys share these rules; Rust decides what is joined and pasted, these only disable rows.

import { REPEAT_COUNT_MAX, type AppState, type ClipView, type TrackView } from "../bridge";

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

/**
 * Whether the clips kept can be pasted onto `track`: the first one goes there, and a track of the other kind takes none
 * (keys clips go on keyboard tracks, audio clips on vocal tracks). Rust decides the rest (rows past the ends, overlaps).
 */
export function pasteable(state: AppState, track: TrackView | undefined): boolean {
  const a = state.arrange;
  return a.clipboardClips > 0 && track !== undefined && (a.clipboardKind === null || a.clipboardKind === track.kind.kind);
}

/** Whether Mod V and 「編集 ▸ 貼りつけ」 can paste: they paste onto the selected track. */
export function pasteableOnSelected(state: AppState): boolean {
  return pasteable(state, state.song.tracks.find((t) => t.id === state.song.playTarget));
}

/** 「ここで切る」: the snapped place, when it is inside the clip (not at an end). */
export function splitTick(clip: ClipView, snapped: number): number | null {
  return clip.start < snapped && snapped < clip.end ? snapped : null;
}

/**
 * The clips Mod R and 「くり返す…」 repeat (round 12 A1): the chosen ones. null when none, while recording or counting in,
 * or while the arrange is only the overview (the box goes on the clips).
 */
export function repeatTarget(state: AppState): number[] | null {
  const ids = state.arrange.selectedClips;
  const busy = state.transport.state === "recording" || state.transport.state === "countIn";
  return ids.length === 0 || busy || state.workspace.layout.arrangeOverview ? null : [...ids];
}

/** The count typed in the box of 「くり返す…」: a whole number, put into 1..REPEAT_COUNT_MAX (B-68); null when it is not one. */
export function repeatCountOf(text: string): number | null {
  const s = text.trim().replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0));
  if (!/^[0-9]+$/.test(s)) return null;
  return Math.min(REPEAT_COUNT_MAX, Math.max(1, Number(s)));
}

/** The clip F2 renames: the one clicked last, else the first chosen. */
export function renameClipTarget(state: AppState): number | null {
  const a = state.arrange;
  if (a.selectedClips.length === 0) return null;
  return a.selectedClip !== null && a.selectedClips.includes(a.selectedClip) ? a.selectedClip : a.selectedClips[0]!;
}

/** While the pass going on overdubs (`RecordingView.overdub`), the name of the clip its notes go into; null for a plain recording. */
export function overdubClipName(tracks: readonly TrackView[], recording: { track: number; clip: number | null; overdub: boolean }): string | null {
  if (!recording.overdub || recording.clip === null) return null;
  return tracks.find((t) => t.id === recording.track)?.clips.find((c) => c.id === recording.clip)?.name ?? null;
}
