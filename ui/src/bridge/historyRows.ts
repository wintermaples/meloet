// The rows of the history band, fetched where the band looks and kept (docs/architecture.md 21-12, 21-15).
// Rows at or below `current` never change. A new step comes in after `current` and drops what was above it, so
// when `newest` changes the rows above the last `current` are let go; undo, redo and a jump move only `current`.

import type { HistoryRow, HistoryView } from "./types";

/** Rows are fetched in blocks of this many, so scrolling asks for a few blocks and not one row at a time. */
export const HISTORY_BLOCK = 64;

interface Asked {
  block: number;
  /** The lowest row let go of since it was asked for: rows from here on in the answer may be old. */
  cut: number;
}

export class HistoryRowStore {
  private readonly rows = new Map<number, HistoryRow>();
  private readonly asked = new Map<number, Asked>();
  private readonly listeners = new Set<() => void>();
  private view: HistoryView = { total: 0, redo: 0, current: 0, newest: 0 };
  /** Goes up on `clear`: answers asked for before it are of another song. */
  private generation = 0;
  /** How many fetches were sent (for the tests and the measurements). */
  fetches = 0;

  constructor(private readonly fetch: (from: number, to: number) => Promise<HistoryRow[]>) {}

  /** The section `history` that came; lets go of the rows it says may have changed. */
  follow(next: HistoryView): void {
    const before = this.view;
    this.view = next;
    let from = next.total + 1;
    if (next.newest !== before.newest) from = Math.min(from, before.current + 1, next.total);
    this.cut(from);
  }

  /** Lets go of every row, also those on the way: another song, or a snapshot taken again. */
  clear(view?: HistoryView): void {
    this.generation += 1;
    this.rows.clear();
    this.asked.clear();
    if (view) this.view = view;
    this.changed();
  }

  /** Row `index` (from 1) if it is here. */
  row(index: number): HistoryRow | undefined {
    return this.rows.get(index);
  }

  /** Fetches the rows `from..=to` that are not here or on the way. */
  want(from: number, to: number): void {
    const lo = Math.max(1, from);
    const hi = Math.min(to, this.view.total);
    if (hi < lo) return;
    for (let block = Math.floor((lo - 1) / HISTORY_BLOCK); block <= Math.floor((hi - 1) / HISTORY_BLOCK); block++) {
      if (this.asked.has(block)) continue;
      const first = block * HISTORY_BLOCK + 1;
      const last = Math.min(first + HISTORY_BLOCK - 1, this.view.total);
      let missing = false;
      for (let i = Math.max(first, lo); i <= Math.min(last, hi) && !missing; i++) missing = !this.rows.has(i);
      if (missing) this.ask(block, first, last);
    }
  }

  /** Called whenever rows come or go. Returns the function that stops it. */
  listen(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** How many rows are kept. */
  held(): number {
    return this.rows.size;
  }

  private ask(block: number, first: number, last: number): void {
    const asked: Asked = { block, cut: Infinity };
    const generation = this.generation;
    this.asked.set(block, asked);
    this.fetches += 1;
    this.fetch(first, last)
      .then((rows) => {
        if (generation !== this.generation) return;
        const { total, newest } = this.view;
        let took = false;
        for (const row of rows) {
          if (row.index >= asked.cut || row.index > total || (row.index === total && row.step !== newest)) continue;
          this.rows.set(row.index, row);
          took = true;
        }
        if (took) this.changed();
      })
      .catch(() => {})
      .finally(() => {
        if (this.asked.get(block) === asked) this.asked.delete(block);
      });
  }

  private cut(from: number): void {
    let dropped = false;
    for (const index of this.rows.keys()) {
      if (index < from) continue;
      this.rows.delete(index);
      dropped = true;
    }
    for (const [block, asked] of this.asked) {
      asked.cut = Math.min(asked.cut, from);
      // Its answer is no use from `from` on; a new ask for the block may go out.
      if ((block + 1) * HISTORY_BLOCK >= from) this.asked.delete(block);
    }
    if (dropped) this.changed();
  }

  private changed(): void {
    for (const listener of this.listeners) listener();
  }
}
