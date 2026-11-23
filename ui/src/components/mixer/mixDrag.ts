// Intents sent by fader, pan and send drags (the "commit" comment on Command in the protocol).
// While moving, commit: false (only the sound changes) at most once a frame; on release, one commit: true (one step).
// Cancelled with Esc (cancelDrag): CancelMixPreview. Released without moving: nothing is sent.

import type { Command } from "../../bridge";
import { forgetHand, noteHand } from "../automation/handValue";

export class MixPreview<V> {
  private pending: { value: V } | null = null;
  private frame = 0;
  private moved = false;
  /** The last "change only the sound" intent (the hand position is kept for the line of a band being written; automation/handValue.ts). */
  private last: Command | null = null;

  constructor(
    private readonly make: (value: V, commit: boolean) => Command,
    private readonly send: (c: Command) => void,
  ) {}

  /** Moved. Next frame, send only the newest value. */
  move(value: V): void {
    this.moved = true;
    this.pending = { value };
    if (this.frame === 0) this.frame = requestAnimationFrame(() => this.flush());
  }

  /** Released. */
  commit(value: V): void {
    this.stop();
    if (this.moved) this.send(this.make(value, true));
  }

  /** Cancelled (Esc, or the pointer was taken). */
  cancel(): void {
    this.stop();
    if (this.moved) this.send({ type: "cancelMixPreview" });
  }

  private flush(): void {
    this.frame = 0;
    const p = this.pending;
    this.pending = null;
    if (!p) return;
    const c = this.make(p.value, false);
    this.last = c;
    noteHand(c);
    this.send(c);
  }

  private stop(): void {
    if (this.last) forgetHand(this.last);
    this.last = null;
    if (this.frame !== 0) cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.pending = null;
  }
}

/** From releasing the key to making it one step (spec 3 (86-7)). */
export const KEY_COMMIT_MS = 1000;

/**
 * Fader, pan and send keys (the "keys" comment on Command in the protocol). Each press sends commit: false; 1 second after the key is released,
 * or when focus leaves (flush), one commit: true with the newest value. A press in between starts the wait again.
 */
export class KeySteps<V> {
  private last: { value: V } | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly make: (value: V, commit: boolean) => Command,
    private readonly send: (c: Command) => void,
  ) {}

  step(value: V): void {
    this.clear();
    this.last = { value };
    this.send(this.make(value, false));
  }

  release(): void {
    if (!this.last) return;
    this.clear();
    this.timer = setTimeout(() => this.flush(), KEY_COMMIT_MS);
  }

  flush(): void {
    this.clear();
    const last = this.last;
    this.last = null;
    if (last) this.send(this.make(last.value, true));
  }

  private clear(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }
}
