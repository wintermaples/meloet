// 入力計のベロシティ固定（docs/design-notes.md B-66）と、棒のドラッグ中の Esc（仕様書 4 章）。
import { flushSync, mount, tick, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import InputMeter from "../src/components/InputMeter.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import HScrollbar from "../src/lib/HScrollbar.svelte";
import { tipView } from "../src/lib/tip.svelte";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});

function show(component: Component<any>, state: AppState | null, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}

const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const KEYBOARD = { id: "kb", name: "鍵盤 49", portName: "鍵盤 49 MIDI 1", address: "24:0" };
const withKeyboard = (s: AppState) => (s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false });

describe("ベロシティ固定（B-66）", () => {
  it("読み値の右に「固定」。押すと setVelocityFixed（いまの値のまま、オン ・ オフ）。用語の説明つき", () => {
    show(InputMeter, app.stateWith(withKeyboard));
    const toggle = q('[data-test="velocity-fixed"]')!;
    expect(toggle.textContent).toBe(t("input.velocityFixed"));
    expect(toggle.getAttribute("aria-pressed")).toBe("false");
    vi.useFakeTimers();
    toggle.dispatchEvent(new PointerEvent("pointerenter"));
    vi.advanceTimersByTime(600);
    flushSync();
    expect(tipView.value?.term).toBe("ベロシティ固定");
    toggle.dispatchEvent(new PointerEvent("pointerleave"));
    vi.useRealTimers();
    // 読み値の右（同じ行で、読み値よりあと）。
    const value = q('[data-test="velocity"] .value')!;
    expect(value.compareDocumentPosition(toggle) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    toggle.click();
    expect(app.sent).toEqual([{ type: "setVelocityFixed", on: true, value: 100 }]);
  });

  it("引っこめる順には入れない: 見えない写し（ruler）でいつも幅を数え、meterFit の結果には「固定」の欄がない", () => {
    show(InputMeter, app.stateWith(withKeyboard));
    expect(q(".ruler .fixed")?.textContent).toBe(t("input.velocityFixed"));
    expect(q(".ruler [data-part]")).not.toBeNull();
    expect(q('.ruler .fixed')!.closest("[data-part]")).toBeNull();
  });

  it("オンの間: 切りかえは on（accent）、読み値は固定の値で、押すと入力の枠。範囲の外は丸めて clampValue", async () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        withKeyboard(s);
        s.keyboards.velocityFixed = { on: true, value: 90 };
      }),
    );
    const toggle = q('[data-test="velocity-fixed"]')!;
    expect(toggle.classList.contains("on")).toBe(true);
    const shown = q<HTMLButtonElement>('button[data-test="velocity-fixed-value"]')!;
    expect(shown.textContent?.trim()).toBe("90");
    shown.click();
    flushSync();
    await tick();
    const input = q<HTMLInputElement>('input[data-test="velocity-fixed-value"]')!;
    input.value = "300";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    flushSync();
    expect(app.sent).toEqual([
      { type: "setVelocityFixed", on: true, value: 127 },
      { type: "clampValue", field: "velocity", value: "127", range: null },
    ]);
    toggle.click();
    expect(app.sent.at(-1)).toEqual({ type: "setVelocityFixed", on: false, value: 90 });
  });

  it("細いときも出す（読み値は text.s の小さい形）", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        withKeyboard(s);
        s.workspace.layout.meter = "slim";
        s.keyboards.velocityFixed = { on: true, value: 100 };
      }),
    );
    expect(q('[data-test="velocity-fixed"]')).not.toBeNull();
    expect(q('button[data-test="velocity-fixed-value"]')!.classList.contains("small")).toBe(true);
  });

  it("鍵盤がないときも使える（書く道具のため）。オフなら読み値は出さず、オンなら固定の値", () => {
    show(InputMeter, app.stateWith(() => {}));
    expect(q('[data-test="velocity-fixed"]')).not.toBeNull();
    expect(q('[data-test="velocity"] .value')).toBeNull();
    app.setState(app.stateWith((s) => (s.keyboards.velocityFixed = { on: true, value: 64 })));
    flushSync();
    expect(q('button[data-test="velocity-fixed-value"]')!.textContent?.trim()).toBe("64");
  });
});

describe("棒のドラッグ中の Esc（仕様書 4 章）", () => {
  it("横の棒: つまみを動かしている間の Esc で、動かす前の所へ戻す", () => {
    const moves: number[] = [];
    show(HScrollbar, null, { total: 2000, visible: 500, offset: 100, onscroll: (x: number) => moves.push(x) });
    const thumb = q('[data-testid="hscrollbar-thumb"]')!;
    thumb.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true, clientX: 50, button: 0 }));
    thumb.dispatchEvent(new MouseEvent("pointermove", { bubbles: true, clientX: 150 }));
    expect(moves.at(-1)).not.toBe(100);
    expect(cancelDrag()).toBe(true);
    expect(moves.at(-1)).toBe(100);
    expect(cancelDrag()).toBe(false);
  });
});
