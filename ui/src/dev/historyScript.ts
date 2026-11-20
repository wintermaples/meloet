// Development only (MELOET_UI_SCRIPT with hist-* parts): times the history band with many steps. It makes steps,
// opens the band, scrolls its rows and jumps 500 steps, the way a person would, and writes how long each took until
// the rows were on the screen. The record has the shape of script.ts, so ui_script_summary.py reads it.
//   hist-make:N    sends N tempo changes (each one step), to have a long history in the real app
//   hist-open:N    opens the band N times (closed in between): until every row in view has its text
//   hist-scroll:N  scrolls the rows N times, 10 rows a step (turns at the ends): until the rows in view have text
//   hist-jump:N    jumps N times, 500 steps back, then forward (selects the row and presses Enter): until the row
//                  jumped to is 「いまここ」
// inputMs: from the input until the microtasks after it ran (what the screen does at once; for a jump, also the wait
// for the selected row to come).
// replyMs: until it was on the screen (fetching rows, and for a jump the core's answer).

import type { HistoryView } from "../bridge";
import { SCRIPT_REPORT, type Clock, type PartRecord, type ScriptPart, type ScriptRecord, type Step } from "./script";

export interface HistoryTarget {
  /** The page has a state and can send. */
  ready(): boolean;
  history(): HistoryView | null;
  bandOpen(): boolean;
  toggleBand(): void;
  /** Sends the `i`-th step of hist-make; resolves when the core has taken it. */
  make(i: number): Promise<void>;
  /** The box the rows scroll in. */
  viewport(): HTMLElement | null;
  /** Every row in view has its text (its row has come). */
  filled(): boolean;
  /** Selects row `index`, waits until its row has come, and presses Enter on the band. */
  jumpTo(index: number): Promise<void>;
  /** Row `index` is drawn as 「いまここ」. */
  isNow(index: number): boolean;
}

const READY_LIMIT_MS = 60000;
const REPLY_LIMIT_MS = 5000;
const SETTLE_MS = 1500;
const STEP_GAP_MS = 50;
const PART_GAP_MS = 1000;
const MARK_EVERY = 100;
const SCROLL_ROWS = 10;
const JUMP = 500;

export class HistoryScript {
  private stopped = false;
  readonly record: ScriptRecord;

  constructor(
    text: string,
    private readonly parts: ScriptPart[],
    private readonly target: HistoryTarget,
    private readonly clock: Clock,
    private readonly report: (name: string, json: string) => Promise<void>,
  ) {
    this.record = { script: text, done: false, error: null, parts: [] };
  }

  stop(): void {
    this.stopped = true;
  }

  async run(): Promise<void> {
    try {
      const limit = this.clock.now() + READY_LIMIT_MS;
      while (!this.target.ready()) {
        if (this.stopped) return;
        if (this.clock.now() > limit) throw new Error("the page has no state");
        await this.clock.wait(200);
      }
      await this.clock.wait(SETTLE_MS);
      for (const part of this.parts) {
        if (this.stopped) return;
        const rec: PartRecord = { kind: part.kind, count: part.count, startMs: this.clock.now(), endMs: 0, steps: [], frameGapsMs: [], marks: [] };
        this.record.parts.push(rec);
        let last: number | null = null;
        const stopFrames = this.clock.everyFrame((ms) => {
          if (last !== null) rec.frameGapsMs.push(Math.round((ms - last) * 100) / 100);
          last = ms;
        });
        try {
          for (let i = 0; i < part.count && !this.stopped; i++) {
            rec.steps.push(await this.step(part, i));
            if ((i + 1) % MARK_EVERY === 0 || i + 1 === part.count) {
              rec.marks.push({ done: i + 1, atMs: this.clock.now(), wallMs: this.clock.wall() });
              await this.send();
            }
          }
        } finally {
          stopFrames();
        }
        rec.endMs = this.clock.now();
        await this.clock.wait(PART_GAP_MS);
      }
      this.record.done = true;
    } catch (e) {
      this.record.error = String(e);
    }
    await this.send();
  }

  private async send(): Promise<void> {
    try {
      await this.report(SCRIPT_REPORT, JSON.stringify(this.record));
    } catch {
      // The next report carries it.
    }
  }

  private step(part: ScriptPart, i: number): Promise<Step> {
    switch (part.kind) {
      case "histMake":
        return this.make(i);
      case "histOpen":
        return this.open();
      case "histScroll":
        return this.scroll();
      case "histJump":
        return this.jump();
      default:
        throw new Error(`not a history part: ${part.kind}`);
    }
  }

  /** Waits frames until `done`; the time from `from`, or null when it did not come. */
  private async until(done: () => boolean, from: number): Promise<number | null> {
    while (this.clock.now() - from < REPLY_LIMIT_MS) {
      if (done()) return this.clock.now() - from;
      await this.clock.frame();
    }
    return null;
  }

  private async make(i: number): Promise<Step> {
    const atMs = this.clock.now();
    await this.target.make(i);
    const inputMs = this.clock.now() - atMs;
    return { atMs, inputMs, changed: true };
  }

  private async open(): Promise<Step> {
    if (this.target.bandOpen()) {
      this.target.toggleBand();
      await this.until(() => !this.target.bandOpen(), this.clock.now());
      await this.clock.wait(STEP_GAP_MS);
    }
    const atMs = this.clock.now();
    this.target.toggleBand();
    await this.clock.settle();
    const inputMs = this.clock.now() - atMs;
    const replyMs = await this.until(() => this.target.bandOpen() && this.target.viewport() !== null && this.target.filled(), atMs);
    await this.clock.wait(STEP_GAP_MS);
    return { atMs, inputMs, changed: replyMs !== null, replyMs };
  }

  private direction = 1;

  private async scroll(): Promise<Step> {
    const v = this.target.viewport();
    if (!v) throw new Error("the history band is not open");
    const rowH = this.rowHeight(v);
    const before = v.scrollTop;
    const atMs = this.clock.now();
    v.scrollTop = before + this.direction * SCROLL_ROWS * rowH;
    if (v.scrollTop === before) {
      this.direction = -this.direction;
      v.scrollTop = before + this.direction * SCROLL_ROWS * rowH;
    }
    v.dispatchEvent(new Event("scroll"));
    await this.clock.settle();
    const inputMs = this.clock.now() - atMs;
    const replyMs = await this.until(() => this.target.filled(), atMs);
    await this.clock.wait(STEP_GAP_MS);
    return { atMs, inputMs, changed: v.scrollTop !== before, replyMs };
  }

  private back = true;

  private async jump(): Promise<Step> {
    const h = this.target.history();
    if (!h || h.total === 0) throw new Error("no steps to jump through");
    if (this.back && h.current - JUMP < 1) this.back = false;
    if (!this.back && h.current + JUMP > h.total) this.back = true;
    const to = Math.max(1, Math.min(h.total, h.current + (this.back ? -JUMP : JUMP)));
    this.back = !this.back;
    const atMs = this.clock.now();
    await this.target.jumpTo(to);
    await this.clock.settle();
    const inputMs = this.clock.now() - atMs;
    const replyMs = await this.until(() => this.target.history()?.current === to && this.target.isNow(to), atMs);
    await this.clock.wait(STEP_GAP_MS);
    return { atMs, inputMs, changed: replyMs !== null, replyMs };
  }

  private rowHeight(v: HTMLElement): number {
    const row = v.querySelector<HTMLElement>(".row");
    if (!row) throw new Error("the history band shows no row");
    return row.getBoundingClientRect().height;
  }
}
