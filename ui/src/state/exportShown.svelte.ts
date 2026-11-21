// 書き出しのダイアログと状態行の進みで見せる物（約束 26 の export）。たしかめの「数えています 38 %」と進みの % ・ 残りの時間だけが
// 変わったときは SHOW_EVERY_MS に 1 回まで見せかえる（audioWork.svelte.ts と同じ考え。docs/architecture.md 22-22・22-23）。
// ほかの所（形、範囲、たしかめや進みの状態）が変わった時はすぐ。

import type { ExportDialogView, ExportView } from "../bridge";
import { SHOW_EVERY_MS } from "./audioWork.svelte";
import { current } from "./app.svelte";

/** 数だけを除いた形。これが同じなら、数だけが変わった。 */
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

  /** いま見せる書き出しのようす。 */
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

  /** テストの間で、前の見せかえの時刻を持ちこさない。 */
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
