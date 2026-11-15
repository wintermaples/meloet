// The screen's copy of the Rust state. Components read `current`; they never change the state
// themselves. To change something, they `send` an intent and wait for the patch.

import {
  connect,
  send as bridgeSend,
  type AppState,
  type Command,
  type Event,
  type Telemetry,
} from "../bridge";

let state = $state.raw<AppState | null>(null);
let rev = $state(0);
let failure = $state<string | null>(null);
let connected = $state(false);
let telemetry = $state.raw<Telemetry | null>(null);

export const current = {
  get state(): AppState | null {
    return state;
  },
  get rev(): number {
    return rev;
  },
  get failure(): string | null {
    return failure;
  },
  /** True once `send` works. The first state can arrive a moment before this. */
  get connected(): boolean {
    return connected;
  },
  /** The latest telemetry. Sent about 60 times a second while something moves. */
  get telemetry(): Telemetry | null {
    return telemetry;
  },
};

const eventListeners = new Set<(e: Event) => void>();
const telemetryListeners = new Set<(t: Telemetry) => void>();

/** Key on/off and pedal events, called the moment they arrive (before any re-render). */
export function onEvent(listener: (e: Event) => void): () => void {
  eventListeners.add(listener);
  return () => eventListeners.delete(listener);
}

/** Every telemetry message, called the moment it arrives. */
export function onTelemetry(listener: (t: Telemetry) => void): () => void {
  telemetryListeners.add(listener);
  return () => telemetryListeners.delete(listener);
}

export async function start(): Promise<void> {
  try {
    await connect({
      onState(next, nextRev) {
        state = next;
        rev = nextRev;
      },
      onEvent(e) {
        for (const listener of eventListeners) listener(e);
      },
      onTelemetry(t) {
        telemetry = t;
        for (const listener of telemetryListeners) listener(t);
      },
      onError(e) {
        failure = String(e);
      },
    });
    connected = true;
  } catch (e) {
    failure = String(e);
  }
}

/** Resolves to the rev after the command (null if it failed). The same rev as before means nothing changed. */
export function send(command: Command): Promise<number | null> {
  return bridgeSend(command).catch((e) => {
    failure = String(e);
    return null;
  });
}
