// The only door between the screen and Rust. Everything outside ui/src/bridge imports from here.

import { createFakeTransport, fakeMeasureEnabled, fakeMeasureReport } from "./fake";
import { connectWith, type Connection, type Handlers, type Transport } from "./session";
import { tauriMeasureEnabled, tauriMeasureReport, tauriTransport } from "./tauri";
import type { Command } from "./types";

export type * from "./types";
export { PROTOCOL_VERSION } from "./types";
export { keyIsDown, type Telemetry } from "./telemetry";
export { ProtocolVersionError, type Connection, type Handlers } from "./session";

let connection: Connection | null = null;

function pickTransport(): Transport {
  return "__TAURI_INTERNALS__" in window ? tauriTransport : createFakeTransport();
}

/** hello → subscribe → snapshot. Rejects when the protocol versions differ. */
export async function connect(handlers: Handlers): Promise<Connection> {
  connection = await connectWith(pickTransport(), handlers);
  return connection;
}

function connected(): Connection {
  if (!connection) throw new Error("bridge: connect() has not finished");
  return connection;
}

/** Sends an intent. Resolves to the rev after it. */
export async function send(command: Command): Promise<number> {
  return connected().send(command);
}

/** `rustMonoUs ≈ performance.now() * 1000 + offset`. */
export async function clockOffsetUs(): Promise<number> {
  return connected().clockOffsetUs();
}

const inTauri = () => "__TAURI_INTERNALS__" in window;

/** Development only: whether to measure key → screen. Does not need connect(). */
export function measureEnabled(): Promise<boolean> {
  return inTauri() ? tauriMeasureEnabled() : fakeMeasureEnabled();
}

/** Development only: asks Rust to write a measurement file. */
export function measureReport(name: string, json: string): Promise<void> {
  return inTauri() ? tauriMeasureReport(name, json) : fakeMeasureReport(name, json);
}
