// The parameters of the plugin the parameters panel shows (`SelectedPlugin.table`). The names and
// groups are fetched once for a table (`plugin_params`); the values that change come on the
// messages Channel (`paramValues`), numbered with `seq` per table. The book fetches the table
// again when a number is skipped, the same way the live notes do.

import type { ParamRow, ParamTable, ParamValues } from "./types";

/** `changed`: the ids of the rows whose value changed, or null when the whole table is another one. */
export type ParamTableListener = (changed: readonly number[] | null) => void;

export class ParamTableBook {
  /** The table the state names. */
  private want: number | null = null;
  /** The table held; null until it has come, and when the state names none. */
  private held: ParamTable | null = null;
  private readonly rows = new Map<number, ParamRow>();
  private waiting: ParamValues[] = [];
  private fetching = false;
  private readonly listeners = new Set<ParamTableListener>();

  constructor(
    private readonly fetch: (table: number) => Promise<ParamTable | null>,
    private readonly onError: (error: unknown) => void = () => {},
  ) {}

  /** Call with `SelectedPlugin.table` (null when no plugin is shown) whenever the state changes. */
  show(table: number | null): void {
    if (table === this.want) return;
    this.want = table;
    this.waiting = [];
    if (this.held) {
      this.held = null;
      this.rows.clear();
      this.tell(null);
    }
    if (table !== null) this.sync().catch(this.onError);
  }

  receive(message: ParamValues): void {
    if (message.table !== this.want) return;
    if (this.fetching || !this.held) {
      this.waiting.push(message);
      // A fetch that failed is tried again on the next message.
      if (!this.fetching) this.sync().catch(this.onError);
      return;
    }
    if (message.seq <= this.held.seq) return;
    if (message.seq === this.held.seq + 1) {
      this.apply(message);
      this.tell(message.values.map((v) => v.id));
      return;
    }
    this.sync().catch(this.onError);
  }

  /**
   * The table of the plugin shown, or null while it is on its way. The same object until another
   * table comes; its rows' `value` and `text` are written in place as values come.
   */
  table(): ParamTable | null {
    return this.held;
  }

  row(id: number): ParamRow | undefined {
    return this.rows.get(id);
  }

  /** Called after every change. Returns the function that stops it. */
  listen(listener: ParamTableListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private apply(message: ParamValues): void {
    for (const v of message.values) {
      const row = this.rows.get(v.id);
      if (!row) continue;
      row.value = v.value;
      row.text = v.text;
    }
    this.held!.seq = message.seq;
  }

  private tell(changed: readonly number[] | null): void {
    for (const l of this.listeners) l(changed);
  }

  private async sync(): Promise<void> {
    if (this.fetching) return;
    this.fetching = true;
    try {
      for (;;) {
        const want = this.want;
        if (want === null) return;
        const got = await this.fetch(want);
        // The state named another table meanwhile: that one is fetched instead.
        if (this.want !== want) continue;
        // Rust holds another table by now; the state that names it is on its way.
        if (!got) return;
        this.held = got;
        this.rows.clear();
        for (const g of got.groups) for (const r of g.params) this.rows.set(r.id, r);
        for (const r of got.keyboardKnobs) this.rows.set(r.id, r);
        const queued = this.waiting;
        this.waiting = [];
        let gap = false;
        for (const m of queued) {
          if (m.table !== want || m.seq <= got.seq) continue;
          if (m.seq !== got.seq + 1) {
            gap = true;
            break;
          }
          this.apply(m);
        }
        if (!gap) break;
      }
    } finally {
      this.fetching = false;
    }
    this.tell(null);
  }
}
