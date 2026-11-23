// What the export dialog and the status line progress show (`export` of protocol 26). When only the check's 「数えています 38 %」,
// the progress percent and the time left change, it is shown again at most once every SHOW_EVERY_MS (the same idea as
// audioWork.svelte.ts; architecture 22-22, 22-23). When anything else (shape, range, state of the check or the run) changes, at once.

import type { ExportDialogView, ExportView } from "../bridge";
import { SHOW_EVERY_MS } from "./audioWork.svelte";
import { current } from "./app.svelte";

/** The shape without the numbers. If this is the same, only the numbers changed. */
function keyOf(view: ExportView | null): string {
  if (!view) return "";
  const dialog: ExportDialogView | null = view.dialog && {
    ...view.dialog,
    check: view.dialog.check.state === "counting" ? { ...view.dialog.check, percent: 0, peakDb: null } : view.dialog.check,
  };
  const run = view.run?.state === "running" ? { ...view.run, percent: 0, remainingS: null } : view.run;
  return JSON.stringify([dialog, run]);
}

export class ExportShown {
  #tick = $state(0);
  #seen: ExportView | null = null;
  #shown: ExportView | null = null;
  #key = "";
  #at = -Infinity;
  #timer: ReturnType<typeof setTimeout> | null = null;
  readonly #read: () => ExportView | null;
  readonly #now: () => number;

  constructor(read: () => ExportView | null, now: () => number = () => performance.now()) {
    this.#read = read;
    this.#now = now;
  }

  /** The export state to show now. */
  get current(): ExportView | null {
    void this.#tick;
    const view = this.#read();
    if (view !== this.#seen) {
      this.#seen = view;
      const key = keyOf(view);
      if (key !== this.#key || this.#now() - this.#at >= SHOW_EVERY_MS) this.#take(view, key);
      else if (JSON.stringify(view) !== JSON.stringify(this.#shown)) this.#later();
    }
    return this.#shown;
  }

  /** Do not carry the time of the last update from one test to the next. */
  forget(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#seen = null;
    this.#shown = null;
    this.#key = "";
    this.#at = -Infinity;
  }

  #take(view: ExportView | null, key: string): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#key = key;
    this.#at = this.#now();
    this.#shown = view;
  }

  #later(): void {
    if (this.#timer !== null) return;
    this.#timer = setTimeout(
      () => {
        this.#timer = null;
        const view = this.#read();
        if (view === this.#shown) return;
        this.#take(view, keyOf(view));
        this.#tick++;
      },
      Math.max(0, this.#at + SHOW_EVERY_MS - this.#now()),
    );
  }
}

export const exportShown = new ExportShown(() => current.state?.export ?? null);
