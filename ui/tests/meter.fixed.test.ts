// Fixed velocity on the input meter (docs/history/design-notes.ja.md B-66), and Esc while dragging a scroll bar (spec 4).
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

describe("fixed velocity (B-66)", () => {
  it("「固定」 right of the readout. Pressing sends setVelocityFixed (the current value, on/off). Has a term tip", () => {
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
    // Right of the readout (same row, after the readout).
    const value = q('[data-test="velocity"] .value')!;
    expect(value.compareDocumentPosition(toggle) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    toggle.click();
    expect(app.sent).toEqual([{ type: "setVelocityFixed", on: true, value: 100 }]);
  });

  it("not in the hiding order: a hidden copy (ruler) always measures the width, and the meterFit result has no 「固定」 column", () => {
    show(InputMeter, app.stateWith(withKeyboard));
    expect(q(".ruler .fixed")?.textContent).toBe(t("input.velocityFixed"));
    expect(q(".ruler [data-part]")).not.toBeNull();
    expect(q('.ruler .fixed')!.closest("[data-part]")).toBeNull();
  });

  it("while on: the toggle is on (accent), the readout is the fixed value, and pressing opens an input box. Out of range is rounded with clampValue", async () => {
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

  it("shown when slim too (the readout in the small text.s form)", () => {
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

  it("works without a keyboard (for the draw tool). When off the readout is hidden, when on it is the fixed value", () => {
    show(InputMeter, app.stateWith(() => {}));
    expect(q('[data-test="velocity-fixed"]')).not.toBeNull();
    expect(q('[data-test="velocity"] .value')).toBeNull();
    app.setState(app.stateWith((s) => (s.keyboards.velocityFixed = { on: true, value: 64 })));
    flushSync();
    expect(q('button[data-test="velocity-fixed-value"]')!.textContent?.trim()).toBe("64");
  });
});

describe("Esc while dragging a scroll bar (spec 4)", () => {
  it("horizontal bar: Esc while moving the thumb puts it back where it was before", () => {
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
