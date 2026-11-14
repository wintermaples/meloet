// The real Transport. This file and nothing outside ui/src/bridge imports @tauri-apps/api.

import { Channel, invoke } from "@tauri-apps/api/core";
import type { Transport } from "./session";
import type { HelloReply, SendReply, ServerMessage, Snapshot } from "./types";

export const tauriTransport: Transport = {
  hello: (protocolVersion) => invoke<HelloReply>("hello", { protocolVersion }),
  subscribe(onMessage, onTelemetry) {
    const messages = new Channel<ServerMessage>(onMessage);
    const telemetry = new Channel<ArrayBuffer>(onTelemetry);
    return invoke<void>("subscribe", { messages, telemetry });
  },
  snapshot: () => invoke<Snapshot>("snapshot"),
  send: (command) => invoke<SendReply>("send", { command }),
  clock: () => invoke<number>("clock"),
};
