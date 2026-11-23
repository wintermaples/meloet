// Keys for tempo and time signature changes and markers (spec 4). Does not know the DOM.
// K: a marker at the play line (one letter, so it does not work while typing). Alt → / Alt ←: to the next / previous marker (these work).
// F2: rename the chosen marker (only when a marker is chosen; otherwise the track name).
// Delete: removes the chosen tempo point, time signature flag or marker (the start of the song cannot be removed). Only when keys go to the arrange
// and it was chosen after any clip (pick.ts). While recording and during count-in, none of them are sent (the core does not accept them either).
// The key handler (handleKeydown in ui/src/keys/attach.ts) calls this before the other key rules, except for Esc.

import type { Command, TimelineView } from "../../bridge";
import { t } from "../../text/ja";
import { lastPick } from "./pick";

export interface TimelineKey {
  key: string;
  code?: string;
  ctrl: boolean;
  meta: boolean;
  alt: boolean;
  shift: boolean;
}

export interface TimelineKeyContext {
  /** Typing text right now. */
  typing: boolean;
  /** Recording or in count-in. */
  busy: boolean;
  /** Keys go to the arrange (the piano roll is not open, or the arrange was pressed last). */
  toArrange: boolean;
}

/** The intent to send. Null if this is not a tempo, time signature or marker key (go on to the other rules). */
export function timelineKeyCommand(k: TimelineKey, c: TimelineKeyContext, timeline: TimelineView): Command | null {
  if (k.ctrl || k.meta || c.busy) return null;
  if (k.alt) {
    if (k.shift) return null;
    if (k.key === "ArrowRight") return { type: "stepMarker", forward: true };
    if (k.key === "ArrowLeft") return { type: "stepMarker", forward: false };
    return null;
  }
  if (k.shift) return null;
  const sel = timeline.selected;
  if (k.key === "F2") return sel?.kind === "marker" ? { type: "openMarkerName", id: sel.id } : null;
  if (c.typing) return null;
  if (k.key === "Delete") {
    // Taken here even when the start of the song is chosen (the core does not remove it). Not passed on to clips.
    if (!sel || !c.toArrange || lastPick.kind !== "timeline") return null;
    return { type: "deleteTimeline", target: sel };
  }
  const letter = /^[a-z]$/i.test(k.key) ? k.key.toLowerCase() : k.code === "KeyK" ? "k" : null;
  if (letter === "k") return { type: "addMarker", name: t("marker.newName", { count: timeline.markers.length + 1 }), tick: null };
  return null;
}
