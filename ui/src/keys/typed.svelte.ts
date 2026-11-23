// R2 「キーで弾く」: while it is on, the PC keyboard plays notes. Keys by their place (KeyboardEvent.code), so the
// layout and the input method's letters do not matter: white keys A S D F G H J K L ; are C D E F G A B C D E, black
// keys W E are C# D#, T Y U are F# G# A#, O P are C# D# above. Z and X lower and raise the octave of A (C0 to C7, C3 at
// first; kept while meloet runs). Letters and digits without a modifier are not shortcuts meanwhile (spec R2).
// Does not know Tauri: the keys go out through `post`, which does not wait for Rust.

import type { Command } from "../bridge";

/** Semitones of each key above the C of the octave. */
export const TYPED_NOTES: Readonly<Record<string, number>> = {
  KeyA: 0,
  KeyW: 1,
  KeyS: 2,
  KeyE: 3,
  KeyD: 4,
  KeyF: 5,
  KeyT: 6,
  KeyG: 7,
  KeyY: 8,
  KeyH: 9,
  KeyU: 10,
  KeyJ: 11,
  KeyK: 12,
  KeyO: 13,
  KeyL: 14,
  KeyP: 15,
  Semicolon: 16,
};
export const OCTAVE_LOWEST = 0;
export const OCTAVE_HIGHEST = 7;
const OCTAVE_FIRST = 3;
const OCTAVE_KEYS: Readonly<Record<string, -1 | 1>> = { KeyZ: -1, KeyX: 1 };
/** Letters and digits: not shortcuts while 「キーで弾く」 is on, even those that play nothing. */
const NOT_SHORTCUTS = /^(Key[A-Z]|Digit[0-9]|Numpad[0-9])$/;
/** An event time further than this from performance.now() is not taken (it would not be on the same clock). */
const EVENT_TIME_SLACK_MS = 1000;

export interface TypedDeps {
  /** Sends without waiting for the rev. */
  post(command: Command): void;
  /** `rustMonoUs ≈ performance.now() * 1000 + offset` (the bridge's clockOffsetUs). */
  clockOffsetUs(): Promise<number>;
  now?: () => number;
}

/** The octave of the A key (shown as 「オクターブ C3」). */
export const typedKeys = $state({ octave: OCTAVE_FIRST });

/** The keys held, by code, with the note each one sounds. */
const held = new Map<string, number>();
let offsetUs: number | null = null;
let offsetAsked = false;

/** The note a key plays at an octave, or null. */
export function typedNote(code: string, octave: number): number | null {
  const step = TYPED_NOTES[code];
  return step === undefined ? null : (octave + 1) * 12 + step;
}

/** The note of the A key now (for 「オクターブ C3」). */
export function octaveNote(): number {
  return (typedKeys.octave + 1) * 12;
}

/**
 * The time of a key on Rust's clock (Command.typedKey.tUs). 0 while the clock offset is not known yet, or when the
 * event's time is not on performance.now()'s clock: Rust takes 0 as now.
 */
function rustUs(eventMs: number, deps: TypedDeps): number {
  if (offsetUs === null) {
    if (!offsetAsked) {
      offsetAsked = true;
      deps.clockOffsetUs().then(
        (offset) => (offsetUs = offset),
        () => (offsetAsked = false),
      );
    }
    return 0;
  }
  const now = (deps.now ?? (() => performance.now()))();
  const ms = Math.abs(eventMs - now) <= EVENT_TIME_SLACK_MS ? eventMs : now;
  return Math.max(0, Math.round(ms * 1000 + offsetUs));
}

/**
 * A keydown while 「キーで弾く」 is on and no text is typed. true when the key is the PC keyboard's (a note, Z, X, or another
 * letter or digit): it is not a shortcut then. Keys with a modifier are not the PC keyboard's.
 */
export function typedKeyDown(ev: KeyboardEvent, deps: TypedDeps): boolean {
  if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.shiftKey) return false;
  const note = typedNote(ev.code, typedKeys.octave);
  const step = OCTAVE_KEYS[ev.code];
  if (note === null && step === undefined && !NOT_SHORTCUTS.test(ev.code)) return false;
  // A key the input method takes: neither a note nor a shortcut.
  if (ev.key === "Process") return true;
  ev.preventDefault();
  if (ev.repeat) return true;
  if (step !== undefined) {
    const octave = Math.min(OCTAVE_HIGHEST, Math.max(OCTAVE_LOWEST, typedKeys.octave + step));
    if (octave !== typedKeys.octave) {
      letGoTyped(deps, ev.timeStamp);
      typedKeys.octave = octave;
    }
  } else if (note !== null && !held.has(ev.code)) {
    held.set(ev.code, note);
    deps.post({ type: "typedKey", note, down: true, tUs: rustUs(ev.timeStamp, deps) });
  }
  return true;
}

/** A keyup: lets go of the note its key holds, whatever else is going on. */
export function typedKeyUp(ev: KeyboardEvent, deps: TypedDeps): void {
  const note = held.get(ev.code);
  if (note === undefined) return;
  held.delete(ev.code);
  deps.post({ type: "typedKey", note, down: false, tUs: rustUs(ev.timeStamp, deps) });
}

/** Lets go of every key held: the window lost the keys, the octave changes, 「キーで弾く」 is being turned off. */
export function letGoTyped(deps: TypedDeps, atMs?: number): void {
  const at = atMs ?? (deps.now ?? (() => performance.now()))();
  for (const note of held.values()) deps.post({ type: "typedKey", note, down: false, tUs: rustUs(at, deps) });
  held.clear();
}

/** The switch (Mod K, the button, the menu, Esc). Off lets go of the keys held first: Rust may take it late. */
export function switchTypedKeys(on: boolean, deps: TypedDeps & { send(command: Command): void }): void {
  if (!on) letGoTyped(deps);
  deps.send({ type: "typedKeys", on });
}

/** 「キーで弾く」 is off: Rust let go of its keys already. */
export function forgetTyped(): void {
  held.clear();
}

/** The notes held (for tests). */
export function typedHeld(): number[] {
  return [...held.values()];
}

/** Back to how it is at start (for tests). */
export function resetTypedKeys(): void {
  held.clear();
  typedKeys.octave = OCTAVE_FIRST;
  offsetUs = null;
  offsetAsked = false;
}
