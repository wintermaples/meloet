import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TIP_DELAY_MS, hideTip, term, tip, tipView, tipVisible } from "../src/lib/tip.svelte";

const enter = (el: HTMLElement) => el.dispatchEvent(new PointerEvent("pointerenter"));
const move = (el: HTMLElement) => el.dispatchEvent(new PointerEvent("pointermove"));
const leave = (el: HTMLElement) => el.dispatchEvent(new PointerEvent("pointerleave"));

describe("term tip", () => {
  let el: HTMLElement;
  beforeEach(() => {
    vi.useFakeTimers();
    el = document.createElement("span");
    document.body.append(el);
  });
  afterEach(() => {
    hideTip();
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  it("shows after resting 0.6 s, hides on leaving", () => {
    const a = term(el, "ループ") as { destroy(): void };
    enter(el);
    vi.advanceTimersByTime(TIP_DELAY_MS - 1);
    expect(tipVisible()).toBe(false);
    vi.advanceTimersByTime(1);
    expect(tipView.value).toMatchObject({ term: "ループ", body: "決めた範囲をくり返し再生する。" });
    leave(el);
    expect(tipVisible()).toBe(false);
    a.destroy();
  });

  it("not shown while moving (0.6 s after stopping)", () => {
    term(el, "ループ");
    enter(el);
    vi.advanceTimersByTime(400);
    move(el);
    vi.advanceTimersByTime(400);
    expect(tipVisible()).toBe(false);
    vi.advanceTimersByTime(200);
    expect(tipVisible()).toBe(true);
  });

  it("can be hidden from outside (Esc)", () => {
    term(el, "テンポ");
    enter(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(true);
    hideTip();
    expect(tipVisible()).toBe(false);
  });

  it("only one on the screen", () => {
    const other = document.createElement("span");
    document.body.append(other);
    term(el, "テンポ");
    term(other, "拍子");
    enter(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    leave(el);
    enter(other);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value?.term).toBe("拍子");
  });

  it("can show any text. Empty shows nothing", () => {
    const a = tip(el, "Arturia KeyLab Essential 61 mk3 MIDI 1") as { update(t: string | null): void };
    enter(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value).toMatchObject({ term: null, body: "Arturia KeyLab Essential 61 mk3 MIDI 1" });
    leave(el);
    a.update(null);
    enter(el);
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipVisible()).toBe(false);
  });

  // vitest resolves svelte to the server build, so it cannot be mounted for now (vite.config.ts needs resolve.conditions: ["browser"]).
  it.todo("Tip.svelte: the form is「用語 — 説明」, and the term is strong");
});
