// Stands in for src/state/app.svelte in component tests. Sets the state from outside and records the commands sent.
// Use: vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
import type { AppState, Command, Event, Telemetry } from "../src/bridge";
import { fakeState } from "../src/bridge/fake";

let state = $state.raw<AppState | null>(null);
let telemetry = $state.raw<Telemetry | null>(null);

export const current = {
  get state(): AppState | null {
    return state;
  },
  get rev(): number {
    return 0;
  },
  get failure(): string | null {
    return null;
  },
  get connected(): boolean {
    return true;
  },
  get telemetry(): Telemetry | null {
    return telemetry;
  },
};

export const sent: Command[] = [];
const listeners = new Set<(e: Event) => void>();

export function send(command: Command): Promise<number | null> {
  sent.push(command);
  return Promise.resolve(0);
}

/** What was sent without waiting (the keys of the PC keyboard). */
export const posted: Command[] = [];

export function post(command: Command): void {
  posted.push(command);
}

export function onEvent(listener: (e: Event) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function onTelemetry(): () => void {
  return () => {};
}

export async function start(): Promise<void> {}

// Everything below is used only by tests.

export function setState(next: AppState | null): void {
  state = next;
}

export function setTelemetry(next: Telemetry | null): void {
  telemetry = next;
}

export function emit(e: Event): void {
  for (const listener of listeners) listener(e);
}

export function reset(): void {
  state = null;
  telemetry = null;
  sent.length = 0;
  posted.length = 0;
  listeners.clear();
}

/** A state made from fakeState() with some parts changed. */
export function stateWith(edit: (s: AppState) => void): AppState {
  const s = fakeState();
  edit(s);
  return s;
}

/** Meter values. keys are the numbers of the held keys. */
export function telemetryWith(t: Partial<Omit<Telemetry, "keys">> & { keys?: number[] }): Telemetry {
  const keys = new Uint32Array(4);
  for (const n of t.keys ?? []) keys[n >>> 5]! |= 1 << (n & 31);
  return {
    seq: 1,
    sentUs: 0,
    lastVelocity: 0,
    sustain: 0,
    modulation: 0,
    running: true,
    masterClipped: false,
    pitchBend: 0,
    loadPermille: 0,
    masterPeak: [0, 0],
    position: null,
    returnPeaks: new Float32Array(4),
    trackPeaks: new Float32Array(2),
    mics: [],
    masterReductionDb: 0,
    spectrum: new Uint8Array(24),
    lufsShort: null,
    lufsIntegrated: null,
    truePeakDb: null,
    ...t,
    keys,
  };
}
