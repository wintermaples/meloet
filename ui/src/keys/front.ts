// Which dialog has the keys. While one is open, the parts behind it (the output chooser, the shelf, the history band) do not take keys,
// and plain Enter and Esc go to that dialog (keys/attach.ts).
import type { AppState, Command, LatencyMeasure } from "../bridge";
import { about } from "../components/about/about.svelte";
import { exportedOpen } from "../components/export/exported";
import { keyList } from "../components/keylist/keyList.svelte";

export type FrontDialog = "about" | "keyList" | "scanStopped" | "song" | "export" | "exported" | "latency" | "settings";

/** The dialog that has the keys, in the Esc order (the screen's own dialogs first, then the order core's Back closes them). null when none is open. */
export function dialogInFront(state: AppState): FrontDialog | null {
  const ws = state.workspace;
  if (about.open) return "about";
  if (keyList.open) return "keyList";
  if (state.plugins.stoppedOpen) return "scanStopped";
  if (ws.songDialog !== null) return "song";
  if (state.export.dialog !== null) return "export";
  if (exportedOpen(state) !== null) return "exported";
  if (ws.latencyDialog !== null) return "latency";
  if (ws.settings !== null) return "settings";
  return null;
}

/** 「やめる」 of the latency dialog (Esc): while measuring, stops measuring; otherwise closes the dialog. */
export function latencyCancel(measure: LatencyMeasure | null): Command {
  return measure?.state === "measuring" ? { type: "stopLatencyMeasure" } : { type: "closeLatencyMeasure" };
}
