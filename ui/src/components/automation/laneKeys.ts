// Delete for automation points (spec 3 (Y16) 「えらんで Delete で消す」). Does not know the DOM.
// Only when keys go to the arrange and the last thing chosen by pressing was an automation point (timeline/pick.ts).
// Not sent while typing, or while recording or during count-in.

import type { AutomationView, Command } from "../../bridge";
import { lastPick } from "../timeline/pick";
import { indexSet } from "./autoGeom";

/** The intent to send. Null if this is not Delete for points (go on to the other rules). "none": taken, but no point is chosen, so nothing is done. */
export function laneDeleteCommand(
  k: { key: string; ctrl: boolean; meta: boolean; alt: boolean; shift: boolean },
  c: { typing: boolean; busy: boolean; toArrange: boolean },
  automation: AutomationView,
): Command | "none" | null {
  if (k.key !== "Delete" || k.ctrl || k.meta || k.alt || k.shift || c.typing || !c.toArrange || lastPick.kind !== "lane") return null;
  const sel = automation.selected;
  if (c.busy || !sel) return "none";
  return { type: "deleteAutoPoints", lane: sel.laneId, points: [...indexSet(sel.points)].sort((a, b) => a - b) };
}
