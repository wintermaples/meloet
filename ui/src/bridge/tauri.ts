// The real Transport. This file and nothing outside ui/src/bridge imports @tauri-apps/api.

import { Channel, invoke } from "@tauri-apps/api/core";
import type { Transport } from "./session";
import type { HelloReply, HistoryRow, LiveNotesSnapshot, ParamTable, SendReply, ServerMessage, Snapshot } from "./types";

export const tauriTransport: Transport = {
  hello: (protocolVersion) => invoke<HelloReply>("hello", { protocolVersion }),
  subscribe(onMessage, onTelemetry) {
    const messages = new Channel<ServerMessage>(onMessage);
    const telemetry = new Channel<ArrayBuffer>(onTelemetry);
    return invoke<void>("subscribe", { messages, telemetry });
  },
  snapshot: () => invoke<Snapshot>("snapshot"),
  send: (command) => invoke<SendReply>("send", { command }),
  post: (command) => invoke<void>("post", { command }),
  clock: () => invoke<number>("clock"),
  // A raw reply (tauri::ipc::Response) arrives as an ArrayBuffer.
  takeNotes: (id) => invoke<ArrayBuffer>("take_notes", { id }),
  lane: (laneId) => invoke<ArrayBuffer>("lane", { laneId }),
  peaks: (audio, level, first, count) => invoke<ArrayBuffer>("peaks", { audio, level, first, count }),
  historyRows: (from, to) => invoke<HistoryRow[]>("history_rows", { from, to }),
  liveNotes: () => invoke<LiveNotesSnapshot>("live_notes"),
  pluginParams: (table) => invoke<ParamTable | null>("plugin_params", { table }),
};

/** Development only: whether the key → screen measurement is on. */
export const tauriMeasureEnabled = () => invoke<boolean>("measure_enabled");
/** Development only: hands a measurement file to Rust, which writes it. */
export const tauriMeasureReport = (name: string, json: string) => invoke<void>("measure_report", { name, json });
/** Development only: the screen's input script (MELOET_UI_SCRIPT), or null. */
export const tauriUiScript = () => invoke<string | null>("ui_script");
