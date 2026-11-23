// Text cut with "…" anywhere on the page shows its whole text in the term tip box (owner 2026-11-23).
// jsdom has no layout, so the tests set scrollWidth / clientWidth and the text-overflow by hand.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { selectedKeysFull, selectedKeysText } from "../src/components/pianoroll/drums";
import { TIP_DELAY_MS, findCutText, findCutThrough, hideTip, term, tip, tipView, tipVisible, watchCutText } from "../src/lib/tip.svelte";

const LONG = "Arturia KeyLab Essential 61 mk3 MIDI 1";

function box(text: string, opts: { ellipsis?: boolean; cut?: boolean } = {}): HTMLElement {
  const el = document.createElement("span");
  el.textContent = text;
  if (opts.ellipsis ?? true) el.style.textOverflow = "ellipsis";
  Object.defineProperty(el, "clientWidth", { configurable: true, get: () => 100 });
  Object.defineProperty(el, "scrollWidth", { configurable: true, get: () => (opts.cut ?? true ? 180 : 100) });
  document.body.append(el);
  return el;
}

const move = (el: Element, buttons = 0) => el.dispatchEvent(new PointerEvent("pointermove", { bubbles: true, buttons }));
const enter = (el: Element) => el.dispatchEvent(new PointerEvent("pointerenter"));
const leave = (el: Element) => el.dispatchEvent(new PointerEvent("pointerleave"));

describe("the whole text cut with「…」(one listener per page)", () => {
  let off: () => void;
  beforeEach(() => {
    vi.useFakeTimers();
    off = watchCutText();
  });
  afterEach(() => {
    off();
    hideTip();
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  it("only when cut, resting 0.6 s shows the whole text", () => {
    const el = box(LONG);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS - 1);
    expect(tipVisible()).toBe(false);
    vi.advanceTimersByTime(1);
    expect(tipView.value).toMatchObject({ term: null, body: LONG });
  });

  it("over an inner child, shows the cut text of the nearest ancestor", () => {
    const el = box("");
    const inner = document.createElement("b");
    inner.textContent = LONG;
    el.append(inner);
    move(inner);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value?.body).toBe(LONG);
  });

  it("not shown when not cut. Not shown for elements without ellipsis", () => {
    move(box(LONG, { cut: false }));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
    move(box(LONG, { ellipsis: false }));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });

  it("checks only once, when resting for 0.6 s (does not read layout while moving)", () => {
    const el = box(LONG);
    const reads = vi.fn(() => 180);
    Object.defineProperty(el, "scrollWidth", { configurable: true, get: reads });
    for (let i = 0; i < 10; i++) {
      move(el);
      vi.advanceTimersByTime(16);
    }
    expect(reads).not.toHaveBeenCalled();
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(reads).toHaveBeenCalledTimes(1);
  });

  it("an element with its own term tip shows only that", () => {
    const el = box("ループ");
    term(el, "ループ");
    enter(el);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value).toMatchObject({ term: "ループ" });
    // Moving on inside keeps the term tip; the page-wide one does not take its place.
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value).toMatchObject({ term: "ループ" });
  });

  it("the use:tip text comes before the cut text", () => {
    const el = box("C4 E4 G4 B4 …");
    tip(el, "C4 E4 G4 B4 D5");
    enter(el);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value?.body).toBe("C4 E4 G4 B4 D5");
  });

  it("once shown, moving inside the element keeps it. Leaving hides it", () => {
    const el = box(LONG);
    const other = box("short", { cut: false });
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    move(el);
    expect(tipView.value?.body).toBe(LONG);
    move(other);
    expect(tipVisible()).toBe(false);
  });

  it("leaving the page hides it", () => {
    const el = box(LONG);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    el.dispatchEvent(new PointerEvent("pointerout", { bubbles: true, relatedTarget: null }));
    expect(tipVisible()).toBe(false);
  });

  it("not shown while pressing. Pressing hides it", () => {
    const el = box(LONG);
    move(el, 1);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(true);
    el.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, buttons: 1 }));
    expect(tipVisible()).toBe(false);
    move(el, 1);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });

  it("Esc (hideTip) hides it", () => {
    const el = box(LONG);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    hideTip();
    expect(tipVisible()).toBe(false);
  });

  it("after removing the listener, it no longer shows", () => {
    off();
    move(box(LONG));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });

  it("a path with the head cut off (data-cut of cutHead) shows the whole path", () => {
    const field = document.createElement("span");
    field.setAttribute("data-cut", "");
    field.innerHTML = '<span class="path"><span data-cut-text>/home/me/songs/very/long/path/song.wav</span></span>';
    document.body.append(field);
    expect(findCutText(field.querySelector("[data-cut-text]"))?.text).toBe("/home/me/songs/very/long/path/song.wav");
  });
});

describe("text that lets the pointer through (clip names with pointer-events: none)", () => {
  let off: () => void;
  beforeEach(() => {
    vi.useFakeTimers();
    off = watchCutText();
  });
  afterEach(() => {
    off();
    hideTip();
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  const rect = (left: number, top: number, w: number, h: number) =>
    ({ left, top, right: left + w, bottom: top + h, width: w, height: h, x: left, y: top, toJSON: () => ({}) }) as DOMRect;

  /** A lanes box (the root) with the clip names as marked children; the pointer's target is the box itself. */
  function lanes(names: { text: string; at: DOMRect; cut: boolean }[]): { root: HTMLElement; reads: ReturnType<typeof vi.fn>[] } {
    const root = document.createElement("div");
    root.setAttribute("data-tip-through-root", "");
    const reads = names.map((n) => {
      const el = document.createElement("span");
      el.setAttribute("data-tip-through", "");
      el.textContent = n.text;
      const read = vi.fn(() => n.at);
      el.getBoundingClientRect = read;
      Object.defineProperty(el, "clientWidth", { configurable: true, get: () => 40 });
      Object.defineProperty(el, "scrollWidth", { configurable: true, get: () => (n.cut ? 90 : 40) });
      root.append(el);
      return read;
    });
    document.body.append(root);
    return { root, reads };
  }
  const moveAt = (el: Element, x: number, y: number, buttons = 0) =>
    el.dispatchEvent(new PointerEvent("pointermove", { bubbles: true, clientX: x, clientY: y, buttons }));

  it("resting over marked cut text shows the whole text. Leaving hides it", () => {
    const { root } = lanes([
      { text: "Aメロ（ピアノのとても長い名前）", at: rect(10, 10, 40, 16), cut: true },
      { text: "Bメロ", at: rect(100, 10, 40, 16), cut: false },
    ]);
    moveAt(root, 20, 15);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value).toMatchObject({ term: null, body: "Aメロ（ピアノのとても長い名前）", left: 10, top: 10, bottom: 26 });
    moveAt(root, 30, 20);
    expect(tipVisible()).toBe(true);
    moveAt(root, 70, 20);
    expect(tipVisible()).toBe(false);
  });

  it("not shown when not cut. If the upper mark is not cut, the lower mark is not checked", () => {
    const { root } = lanes([
      { text: "下の長い名前", at: rect(10, 10, 40, 16), cut: true },
      { text: "上", at: rect(10, 10, 40, 16), cut: false },
    ]);
    moveAt(root, 20, 15);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });

  it("not shown when over no mark", () => {
    const { root } = lanes([{ text: "長い名前", at: rect(10, 10, 40, 16), cut: true }]);
    moveAt(root, 200, 200);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });

  it("marked items outside the container are not checked", () => {
    const { root } = lanes([{ text: "長い名前", at: rect(10, 10, 40, 16), cut: true }]);
    const outside = document.createElement("span");
    outside.setAttribute("data-tip-through", "");
    outside.textContent = "外の長い名前";
    const outsideRead = vi.fn(() => rect(10, 10, 40, 16));
    outside.getBoundingClientRect = outsideRead;
    document.body.append(outside);
    moveAt(root, 20, 15);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value?.body).toBe("長い名前");
    expect(outsideRead).not.toHaveBeenCalled();
    // No root around the start: nothing is searched.
    expect(findCutThrough(outside, 20, 15)).toBeNull();
    expect(outsideRead).not.toHaveBeenCalled();
  });

  it("move does not read layout (once after resting 0.6 s)", () => {
    const { root, reads } = lanes([{ text: "長い名前", at: rect(10, 10, 40, 16), cut: true }]);
    for (let i = 0; i < 10; i++) {
      moveAt(root, 20 + i, 15);
      vi.advanceTimersByTime(16);
    }
    expect(reads[0]).not.toHaveBeenCalled();
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(true);
    const n = reads[0]!.mock.calls.length;
    // Moving on inside the shown name compares with the box kept at show time.
    moveAt(root, 25, 18);
    moveAt(root, 26, 18);
    expect(tipVisible()).toBe(true);
    expect(reads[0]!.mock.calls.length).toBe(n);
  });

  it("not shown while pressing (dragging clips or the play line)", () => {
    const { root } = lanes([{ text: "長い名前", at: rect(10, 10, 40, 16), cut: true }]);
    root.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, buttons: 1, clientX: 20, clientY: 15 }));
    moveAt(root, 20, 15, 1);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });
});

describe("pitches of selected notes (「…」for 5 or more)", () => {
  it("passes all names to tip only when cut with「…」", () => {
    const keys = [60, 62, 64, 65, 67, 67];
    expect(selectedKeysText(keys, false)).toMatch(/…$/);
    const full = selectedKeysFull(keys, false)!;
    expect(full).not.toMatch(/…/);
    expect(full.startsWith(selectedKeysText(keys, false).replace(/ …$/, ""))).toBe(true);
    expect(selectedKeysFull([60, 62], false)).toBeNull();
  });
});
