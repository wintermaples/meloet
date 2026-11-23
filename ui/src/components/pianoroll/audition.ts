// Plays sound from the piano roll (docs/history/design-notes.ja.md B-62). Does not know the DOM.
// Rust plays it (audition). The screen times how long, and sends an empty audition to stop.
// The keys being played light up in the keyboard column (Rust does not send audition keys back as Event::Note, so the screen lights them itself).

import type { AppState, Command } from "../../bridge";
import { TICKS_PER_QUARTER } from "../../time/ticks";
import { DRAW_VELOCITY } from "./geom";

/** Chords play at most 16 notes. */
export const MAX_CHORD = 16;
/** How long to play when moved with ↑ ↓ (ms). */
export const NUDGE_MS = 300;

/** The strength for the keyboard column and the draw tool: the fixed velocity value when it is on, otherwise 100 (B-66). */
export function fixedOr100(state: Pick<AppState, "keyboards">): number {
  const f = state.keyboards.velocityFixed;
  return f.on ? f.value : DRAW_VELOCITY;
}

/** Whether to play when a note is pressed, moved, or moved with ↑ ↓. Only when stopped (not while playing, recording or in count-in). */
export const notesMayAudition = (state: Pick<AppState, "transport">) => state.transport.state === "stopped";

/** How many ms ticks (quarter note = TICKS_PER_QUARTER) last at tempo bpm. */
export function ticksToMs(ticks: number, bpm: number): number {
  return bpm > 0 ? (ticks / TICKS_PER_QUARTER) * (60000 / bpm) : 0;
}

/** The chord of the moved notes: add delta to the keys, and take those inside 0 to 127, without repeats, lowest first, up to 16. */
export function chordOf(keys: Iterable<number>, delta: number): number[] {
  const out = new Set<number>();
  for (const k of keys) {
    const moved = k + delta;
    if (moved >= 0 && moved <= 127) out.add(moved);
  }
  return [...out].sort((a, b) => a - b).slice(0, MAX_CHORD);
}

export interface Timer {
  set(run: () => void, ms: number): unknown;
  clear(id: unknown): void;
}

const realTimer: Timer = {
  set: (run, ms) => setTimeout(run, ms),
  clear: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
};

/** The sound the screen is playing now. play replaces it; stop or a timeout stops it. */
export class Auditioner {
  private timer: unknown = null;
  /** The keys playing now (lit up). */
  keys: number[] = [];

  constructor(
    private readonly send: (command: Command) => void,
    private readonly onKeys: (keys: readonly number[]) => void = () => {},
    private readonly clock: Timer = realTimer,
  ) {}

  /** Plays keys (Rust stops the previous sound). If ms is given, stops after that time. */
  play(keys: readonly number[], velocity: number, ms: number | null = null): void {
    this.clearTimer();
    if (keys.length === 0) {
      this.stop();
      return;
    }
    this.send({ type: "audition", keys: [...keys], velocity });
    this.keys = [...keys];
    this.onKeys(this.keys);
    if (ms !== null) this.timer = this.clock.set(() => this.stop(), Math.max(0, ms));
  }

  /** Stops if playing. */
  stop(): void {
    this.clearTimer();
    if (this.keys.length === 0) return;
    this.keys = [];
    this.send({ type: "audition", keys: [], velocity: 0 });
    this.onKeys(this.keys);
  }

  private clearTimer(): void {
    if (this.timer !== null) this.clock.clear(this.timer);
    this.timer = null;
  }
}

type MoveNotes = Extract<Command, { type: "moveNotes" }>;

// ---- When moved with ↑ ↓ (the key handler calls pianoRollCommand; the piano roll part listens here) ----

let nudgeListeners: Array<(command: MoveNotes) => void> = [];

/** Called when an intent to move the selected notes with ↑ ↓ is made. Remove with the returned function. */
export function onNudgeKeys(listener: (command: MoveNotes) => void): () => void {
  nudgeListeners.push(listener);
  return () => {
    nudgeListeners = nudgeListeners.filter((l) => l !== listener);
  };
}

export function nudgedKeys(command: MoveNotes): void {
  for (const l of nudgeListeners) l(command);
}
