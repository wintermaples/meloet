// Text cut with "…" anywhere on the page shows its whole text in the term tip box (owner 2026-11-23).
// jsdom has no layout, so the tests set scrollWidth / clientWidth and the text-overflow by hand.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { selectedKeysFull, selectedKeysText } from "../src/components/pianoroll/drums";
import { TIP_DELAY_MS, findCutText, hideTip, term, tip, tipView, tipVisible, watchCutText } from "../src/lib/tip.svelte";

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

describe("「…」で切れた文の全部（ページに 1 つの耳）", () => {
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

  it("切れているときだけ、0.6 秒止めると全部の文が出る", () => {
    const el = box(LONG);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS - 1);
    expect(tipVisible()).toBe(false);
    vi.advanceTimersByTime(1);
    expect(tipView.value).toMatchObject({ term: null, body: LONG });
  });

  it("中の子に乗っても、近い祖先の切れた文を出す", () => {
    const el = box("");
    const inner = document.createElement("b");
    inner.textContent = LONG;
    el.append(inner);
    move(inner);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value?.body).toBe(LONG);
  });

  it("切れていなければ出ない。ellipsis でない要素も出ない", () => {
    move(box(LONG, { cut: false }));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
    move(box(LONG, { ellipsis: false }));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });

  it("調べるのは止まって 0.6 秒たったとき 1 回だけ（動いているあいだはレイアウトを読まない）", () => {
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

  it("自分の用語の説明を持つ要素では、そちらだけが出る", () => {
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

  it("use:tip の文は、切れた文より先", () => {
    const el = box("C4 E4 G4 B4 …");
    tip(el, "C4 E4 G4 B4 D5");
    enter(el);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value?.body).toBe("C4 E4 G4 B4 D5");
  });

  it("出たあとは、要素の中を動いても消えない。離れたら消える", () => {
    const el = box(LONG);
    const other = box("short", { cut: false });
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    move(el);
    expect(tipView.value?.body).toBe(LONG);
    move(other);
    expect(tipVisible()).toBe(false);
  });

  it("ページの外に出たら消える", () => {
    const el = box(LONG);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    el.dispatchEvent(new PointerEvent("pointerout", { bubbles: true, relatedTarget: null }));
    expect(tipVisible()).toBe(false);
  });

  it("押している間は出ない。押すと消える", () => {
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

  it("Esc（hideTip）で消える", () => {
    const el = box(LONG);
    move(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    hideTip();
    expect(tipVisible()).toBe(false);
  });

  it("耳を外すと、もう出ない", () => {
    off();
    move(box(LONG));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });

  it("頭を省いた道（cutHead の data-cut）は、道の全部", () => {
    const field = document.createElement("span");
    field.setAttribute("data-cut", "");
    field.innerHTML = '<span class="path"><span data-cut-text>/home/me/songs/very/long/path/song.wav</span></span>';
    document.body.append(field);
    expect(findCutText(field.querySelector("[data-cut-text]"))?.text).toBe("/home/me/songs/very/long/path/song.wav");
  });
});

describe("選んだノートの音（5 つ以上は「…」）", () => {
  it("「…」で省いたときだけ、全部の名前を tip に渡す", () => {
    const keys = [60, 62, 64, 65, 67, 67];
    expect(selectedKeysText(keys, false)).toMatch(/…$/);
    const full = selectedKeysFull(keys, false)!;
    expect(full).not.toMatch(/…/);
    expect(full.startsWith(selectedKeysText(keys, false).replace(/ …$/, ""))).toBe(true);
    expect(selectedKeysFull([60, 62], false)).toBeNull();
  });
});
