// The undo history band (spec 2, spec 3 (Y19), spec 4, mocks F-History / R10-History):
// the row store (bridge/historyRows.ts), row form, choose / jump, keys, and the number of rows and time in a 5000-step scene.
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import App from "../src/App.svelte";
import { HISTORY_BLOCK, HistoryRowStore } from "../src/bridge/historyRows";
import type { HistoryRow, HistoryView } from "../src/bridge/types";
import { historyUi } from "../src/components/history/historyUi.svelte";
import { current } from "../src/state/app.svelte";

vi.mock("../src/gl/Surface.svelte", () => ({ default: () => {} }));

const settle = async () => {
  for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

const made = (index: number, step = index): HistoryRow => ({ index, step, name: { kind: "moveNotes" }, what: { kind: "notes", count: 1 }, place: { kind: "pianoRoll" } });
const view = (total: number, current: number, newest: number): HistoryView => ({ total, redo: total - current, current, newest });

/** A fetcher whose answers are returned by hand. */
function fetcher() {
  const calls: { from: number; to: number; answer: (rows: HistoryRow[]) => Promise<void> }[] = [];
  const fetch = (from: number, to: number) =>
    new Promise<HistoryRow[]>((resolve) => {
      calls.push({ from, to, answer: async (rows) => (resolve(rows), await Promise.resolve(), await Promise.resolve()) });
    });
  return { calls, fetch };
}

describe("row store (HistoryRowStore)", () => {
  it("fetches only the visible part, in blocks of 64 rows, and does not fetch arrived rows again", async () => {
    const f = fetcher();
    const book = new HistoryRowStore(f.fetch);
    book.follow(view(5000, 5000, 5000));
    book.want(4990, 5010);
    expect(f.calls.map((c) => [c.from, c.to])).toEqual([[4929, 4992], [4993, 5000]]);
    book.want(4995, 5000);
    expect(f.calls).toHaveLength(2);
    await f.calls[1]!.answer([4993, 4994, 4995, 4996, 4997, 4998, 4999, 5000].map((i) => made(i)));
    expect(book.row(5000)?.index).toBe(5000);
    book.want(4995, 5000);
    expect(f.calls).toHaveLength(2);
    expect(HISTORY_BLOCK).toBe(64);
  });

  it("undo, redo and jump do not drop rows", async () => {
    const f = fetcher();
    const book = new HistoryRowStore(f.fetch);
    book.follow(view(10, 10, 10));
    book.want(1, 10);
    await f.calls[0]!.answer(Array.from({ length: 10 }, (_, i) => made(i + 1)));
    book.follow(view(10, 7, 10));
    book.follow(view(10, 2, 10));
    book.follow(view(10, 9, 10));
    expect(book.held()).toBe(10);
  });

  it("a new step after 3 undos: drops the rows above the current one, fetches only the last row again", async () => {
    const f = fetcher();
    const book = new HistoryRowStore(f.fetch);
    book.follow(view(10, 10, 10));
    book.want(1, 10);
    await f.calls[0]!.answer(Array.from({ length: 10 }, (_, i) => made(i + 1)));
    book.follow(view(10, 7, 10));
    book.follow(view(8, 8, 11));
    expect(book.held()).toBe(7);
    expect(book.row(8)).toBeUndefined();
    book.want(1, 8);
    expect(f.calls.map((c) => [c.from, c.to])).toEqual([[1, 10], [1, 8]]);
    await f.calls[1]!.answer(Array.from({ length: 7 }, (_, i) => made(i + 1)).concat(made(8, 11)));
    expect(book.row(8)?.step).toBe(11);
  });

  it("even if two new steps come at once, drops above the old current", async () => {
    const f = fetcher();
    const book = new HistoryRowStore(f.fetch);
    book.follow(view(10, 5, 10));
    book.want(1, 10);
    await f.calls[0]!.answer(Array.from({ length: 10 }, (_, i) => made(i + 1)));
    book.follow(view(7, 7, 12));
    expect([book.held(), book.row(6)]).toEqual([5, undefined]);
  });

  it("rows that changed while fetching are not taken from the late answer", async () => {
    const f = fetcher();
    const book = new HistoryRowStore(f.fetch);
    book.follow(view(10, 7, 10));
    book.want(1, 10);
    book.follow(view(8, 8, 11));
    // An old answer (before the new step). Row 8 is an old step.
    await f.calls[0]!.answer(Array.from({ length: 10 }, (_, i) => made(i + 1)));
    expect(book.held()).toBe(7);
    expect(book.row(8)).toBeUndefined();
  });

  it("the newest row is taken only when it is the same step as newest (the answer came before the state)", async () => {
    const f = fetcher();
    const book = new HistoryRowStore(f.fetch);
    book.follow(view(5, 5, 5));
    book.want(1, 5);
    await f.calls[0]!.answer([made(1), made(2), made(3), made(4), made(5, 6)]);
    expect([book.held(), book.row(5)]).toEqual([4, undefined]);
  });

  it("when the song changes (clear), rows and answers on the way are dropped", async () => {
    const f = fetcher();
    const book = new HistoryRowStore(f.fetch);
    let calls = 0;
    book.listen(() => calls++);
    book.follow(view(3, 3, 3));
    book.want(1, 3);
    book.clear(view(3, 3, 3));
    await f.calls[0]!.answer([made(1), made(2), made(3)]);
    expect(book.held()).toBe(0);
    expect(calls).toBe(1);
  });
});

/** jsdom does not measure sizes. Only the row box gets the height of 8 band rows. */
function giveRowsHeight(px: number): () => void {
  const was = Object.getOwnPropertyDescriptor(Element.prototype, "clientHeight")!;
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return this.dataset.test === "history-rows" ? px : was.get!.call(this);
    },
  });
  return () => delete (HTMLElement.prototype as { clientHeight?: number }).clientHeight;
}

const text = (el: Element | null | undefined) => el?.textContent?.replace(/\s+/g, " ").trim() ?? "";
const rows = () => [...document.querySelectorAll<HTMLElement>("[data-test=history-list] .row")];
const rowOf = (index: number) => document.querySelector<HTMLElement>(`[data-test=history-list] .row[data-index="${index}"]`);
const list = () => document.querySelector<HTMLElement>("[data-test=history-list]")!;
const key = (k: string, target: Element = document.activeElement ?? document.body) =>
  target.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));

function openScene(scene: string) {
  let app: ReturnType<typeof mount>;
  let restore: () => void;
  beforeAll(async () => {
    restore = giveRowsHeight(192);
    history.replaceState(null, "", `/?scene=${scene}&scale=100`);
    app = mount(App, { target: document.body });
    await settle();
    await settle();
  });
  afterAll(() => {
    unmount(app);
    restore();
    history.replaceState(null, "", "/");
    historyUi.selected = null;
  });
}

describe("R10-History: 16 steps, 3 undone", () => {
  openScene("historyUndone3");
  afterEach(() => (document.activeElement as HTMLElement | null)?.blur());

  it("heading row: step count, current step, undo / redo", () => {
    expect(text(document.querySelector("[data-test=history-head]"))).toBe("16 手 ・ いまは 13 手目。上の 3 手は取り消してあり、やりなおせます");
    expect(text(document.querySelector("[data-test=history-undo]"))).toBe("取り消す Ctrl Z");
    expect(text(document.querySelector("[data-test=history-redo]"))).toBe("やりなおす Ctrl Shift Z");
  });

  it("newer steps on top. 8 rows, and 「… より前は、下へスクロール」 at the bottom", () => {
    expect(rows().map((r) => Number(r.dataset.index))).toEqual([16, 15, 14, 13, 12, 11, 10, 9, 8]);
    expect(text(rowOf(16))).toBe("16 「ノートを動かす」・ 3 ノート ピアノロール やりなおせます");
    expect(rowOf(16)!.classList.contains("undone")).toBe(true);
    expect(text(rowOf(13))).toBe("13 「音量を変える」・ 4 パッド ・ −10.5 → −12.0 dB ミキサー いまここ");
    expect(rowOf(13)!.classList.contains("now")).toBe(true);
    expect(text(rowOf(12))).toBe("12 「エフェクトをとばす」・ MVerb 4 パッド");
    expect(text(document.querySelector("[data-test=history-more]"))).toBe("… 8 手目より前は、下へスクロール");
  });

  it("pressing a row chooses it (background gray.4 and 「ここまで戻す Enter」). The focus border only when chosen by keys", async () => {
    rowOf(11)!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
    list().focus();
    await settle();
    expect(rowOf(11)!.classList.contains("lit")).toBe(true);
    expect(rowOf(11)!.classList.contains("ring")).toBe(false);
    expect(text(rowOf(11))).toBe("11 「マーカーを足す」・ サビ アレンジ ここまで戻す Enter");
  });

  it("↑ ↓ choose (border gray.10), Home goes to the current step", async () => {
    list().focus();
    key("ArrowUp");
    await settle();
    expect(historyUi.selected).toBe(12);
    expect(rowOf(12)!.classList.contains("ring")).toBe(true);
    key("ArrowDown");
    key("ArrowDown");
    await settle();
    expect(historyUi.selected).toBe(10);
    key("Home");
    await settle();
    expect(historyUi.selected).toBe(13);
    // The current step does not show 「ここまで戻す」.
    expect(rowOf(13)!.querySelector("[data-test=history-jump]")).toBeNull();
  });

  it("Enter goes back to the chosen step (HistoryJump). Above the current one, it redoes", async () => {
    list().focus();
    key("ArrowDown");
    key("ArrowDown");
    key("Enter");
    await settle();
    expect(current.state!.history).toMatchObject({ total: 16, current: 11, redo: 5 });
    expect(current.state!.status.message).toMatchObject({ id: "historyJumped", index: 11, steps: 2, forward: false });
    expect(text(rowOf(11))).toContain("いまここ");
    expect(text(rowOf(12))).toContain("やりなおせます");
    rowOf(15)!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
    await settle();
    (rowOf(15)!.querySelector("[data-test=history-jump]") as HTMLButtonElement).click();
    await settle();
    expect(current.state!.history).toMatchObject({ current: 15, redo: 1 });
    expect(current.state!.status.message).toMatchObject({ id: "historyJumped", index: 15, forward: true });
  });

  it("↑ ↓ without focus on the band do not belong to the band", async () => {
    const was = historyUi.selected;
    key("ArrowUp", document.body);
    await settle();
    expect(historyUi.selected).toBe(was);
  });

  it("Esc folds the band", async () => {
    list().focus();
    key("Escape");
    await settle();
    expect(current.state!.workspace.layout.lower.some((l) => l.band === "history")).toBe(false);
  });
});

describe("a song with 5000 steps", () => {
  openScene("history5000");

  it("opening makes only the visible rows. Fetches 1 block", () => {
    expect(rows().length).toBeLessThanOrEqual(10);
    expect(rowOf(5000)).not.toBeNull();
    expect(text(rowOf(5000))).toContain("いまここ");
    expect(text(document.querySelector("[data-test=history-more]"))).toBe("… 4992 手目より前は、下へスクロール");
  });

  it("fetches and draws the rows at the scrolled place, jumps 500 steps (prints the screen-only time)", async () => {
    const viewport = document.querySelector<HTMLElement>("[data-test=history-rows]")!;
    const times: number[] = [];
    for (let i = 1; i <= 20; i++) {
      const t0 = performance.now();
      viewport.scrollTop = i * 240 * 10;
      viewport.dispatchEvent(new Event("scroll"));
      await settle();
      times.push(performance.now() - t0);
      const want = 5000 - Math.floor(viewport.scrollTop / 24);
      expect(text(rowOf(want)), String(want)).toContain("「ノートを動かす」");
      expect(rows().length).toBeLessThanOrEqual(10);
    }
    viewport.scrollTop = 0;
    viewport.dispatchEvent(new Event("scroll"));
    await settle();
    list().focus();
    const t0 = performance.now();
    for (let i = 0; i < 500; i++) key("ArrowDown");
    await settle();
    const stepMs = performance.now() - t0;
    expect(historyUi.selected).toBe(4500);
    expect(rowOf(4500)!.classList.contains("ring")).toBe(true);
    const t1 = performance.now();
    key("Enter");
    await settle();
    const jumpMs = performance.now() - t1;
    expect(current.state!.history.current).toBe(4500);
    expect(text(rowOf(4500))).toContain("いまここ");
    // jsdom time (not the same as real WebKit; a rough guide).
    console.log(
      `history5000 (jsdom): scroll 20 steps median ${times.sort((a, b) => a - b)[10]!.toFixed(1)} ms, max ${times[19]!.toFixed(1)} ms; ` +
        `500 × ↓ ${stepMs.toFixed(1)} ms; jump 500 ${jumpMs.toFixed(1)} ms`,
    );
  });
});
