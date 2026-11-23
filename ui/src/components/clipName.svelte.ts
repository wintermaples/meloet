// The clip name box (mock P10-ClipName). Which clip is being renamed, and where the box is, belong to the screen only;
// deciding sends one renameClip. The name itself is in core.

import type { Command } from "../bridge";

export type ClipNamePlace = "arrange" | "params";

export const clipEdit = $state({ renaming: null as number | null, place: "arrange" as ClipNamePlace });

export function startClipRename(clip: number, place: ClipNamePlace = "arrange"): void {
  clipEdit.renaming = clip;
  clipEdit.place = place;
}

export function stopClipRename(): void {
  clipEdit.renaming = null;
}

/** An empty name keeps the old one; the same name is no step. */
export function renameCommand(clip: number, old: string, draft: string): Command | null {
  const name = draft.trim();
  return name === "" || name === old ? null : { type: "renameClip", clip, name };
}
