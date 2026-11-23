// Mod ＋ / Mod − (spec 4, horizontal zoom in / out) and where they go (the last one pressed).
import { afterEach, describe, expect, it } from "vitest";
import { fakeState } from "../src/bridge/fake";
import { arrangeView, DEFAULT_ARRANGE_BAR, DEFAULT_QUARTER_PX, prView } from "../src/components/pianoroll/view.svelte";
import { handleKeydown, zoomHorizontal } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";

const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });

afterEach(() => {
  arrangeView.barPx = DEFAULT_ARRANGE_BAR;
  prView.quarterPx = DEFAULT_QUARTER_PX;
  prView.focus = "arrange";
});

describe("Mod ＋ / Mod −", () => {
  it("Ctrl ＋ (US Shift ＝, JIS Shift ；, ＝ without Shift, keypad) zooms in, Ctrl − (keypad too) zooms out", () => {
    const zoomIn = { kind: "zoom", delta: 1 };
    const zoomOut = { kind: "zoom", delta: -1 };
    expect(keyAction(k("+", { ctrl: true, shift: true, code: "Equal" }), false, "other")).toEqual(zoomIn);
    expect(keyAction(k("+", { ctrl: true, shift: true, code: "Semicolon" }), false, "other")).toEqual(zoomIn);
    expect(keyAction(k("=", { ctrl: true, code: "Equal" }), false, "other")).toEqual(zoomIn);
    expect(keyAction(k("+", { ctrl: true, code: "NumpadAdd" }), false, "other")).toEqual(zoomIn);
    expect(keyAction(k("-", { ctrl: true, code: "Minus" }), false, "other")).toEqual(zoomOut);
    expect(keyAction(k("-", { ctrl: true, code: "NumpadSubtract" }), false, "other")).toEqual(zoomOut);
  });

  it("Mac uses Cmd. No Mod or with Alt does nothing. Works while typing (with Mod)", () => {
    expect(keyAction(k("+", { meta: true, shift: true }), false, "mac")).toEqual({ kind: "zoom", delta: 1 });
    expect(keyAction(k("+", { ctrl: true, shift: true }), false, "mac")).toBeNull();
    expect(keyAction(k("+", { shift: true }), false, "other")).toBeNull();
    expect(keyAction(k("-", { ctrl: true, alt: true }), false, "other")).toBeNull();
    expect(keyAction(k("-", { ctrl: true }), true, "other")).toEqual({ kind: "zoom", delta: -1 });
  });

  it("to the arrange if it was pressed last, to the piano roll if it is open there", () => {
    const state = fakeState();
    zoomHorizontal(1, state);
    expect(arrangeView.barPx).toBe(91);
    expect(prView.quarterPx).toBe(DEFAULT_QUARTER_PX);

    prView.focus = "pianoRoll";
    zoomHorizontal(1, state);
    expect(arrangeView.barPx).toBe(128);

    state.workspace.layout.lower = [{ band: "pianoRoll", height: 300 }];
    zoomHorizontal(-1, state);
    expect(arrangeView.barPx).toBe(128);
    expect(prView.quarterPx).toBe(57);
  });

  it("from key events: not passed to browser zoom, nothing sent to Rust", () => {
    const sent: unknown[] = [];
    const ev = new KeyboardEvent("keydown", { key: "-", code: "Minus", ctrlKey: true, cancelable: true });
    handleKeydown(ev, { state: () => fakeState(), send: (c) => sent.push(c), nav: { platform: "Linux x86_64" } });
    expect(ev.defaultPrevented).toBe(true);
    expect(sent).toEqual([]);
    expect(arrangeView.barPx).toBe(45);
  });
});
