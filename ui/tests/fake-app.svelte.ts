// 部品のテスト用の src/state/app.svelte の代わり。状態を外から入れ、送ったコマンドを記録する。
// 使いかた: vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
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

export function onEvent(listener: (e: Event) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function onTelemetry(): () => void {
  return () => {};
}

export async function start(): Promise<void> {}

// ここから下はテストだけが使う。

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
  listeners.clear();
}

/** fakeState() を元に、一部を書きかえた状態。 */
export function stateWith(edit: (s: AppState) => void): AppState {
  const s = fakeState();
  edit(s);
  return s;
}

/** 計器の値。keys は押している鍵の番号。 */
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
    ...t,
    keys,
  };
}
