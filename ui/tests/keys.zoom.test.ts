// Mod ＋ ・ Mod −（仕様書 4 章「横に拡大 ・ 縮小」）と、その行き先（最後に押したほう）。
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

describe("Mod ＋ ・ Mod −", () => {
  it("Ctrl ＋（US の Shift ＝、JIS の Shift ；、Shift なしの ＝、テンキー）で拡大、Ctrl −（テンキーも）で縮小", () => {
    const zoomIn = { kind: "zoom", delta: 1 };
    const zoomOut = { kind: "zoom", delta: -1 };
    expect(keyAction(k("+", { ctrl: true, shift: true, code: "Equal" }), false, "other")).toEqual(zoomIn);
    expect(keyAction(k("+", { ctrl: true, shift: true, code: "Semicolon" }), false, "other")).toEqual(zoomIn);
    expect(keyAction(k("=", { ctrl: true, code: "Equal" }), false, "other")).toEqual(zoomIn);
    expect(keyAction(k("+", { ctrl: true, code: "NumpadAdd" }), false, "other")).toEqual(zoomIn);
    expect(keyAction(k("-", { ctrl: true, code: "Minus" }), false, "other")).toEqual(zoomOut);
    expect(keyAction(k("-", { ctrl: true, code: "NumpadSubtract" }), false, "other")).toEqual(zoomOut);
  });

  it("Mac は Cmd。Mod なしと Alt つきは何もしない。文字を打っている最中も効く（Mod つき）", () => {
    expect(keyAction(k("+", { meta: true, shift: true }), false, "mac")).toEqual({ kind: "zoom", delta: 1 });
    expect(keyAction(k("+", { ctrl: true, shift: true }), false, "mac")).toBeNull();
    expect(keyAction(k("+", { shift: true }), false, "other")).toBeNull();
    expect(keyAction(k("-", { ctrl: true, alt: true }), false, "other")).toBeNull();
    expect(keyAction(k("-", { ctrl: true }), true, "other")).toEqual({ kind: "zoom", delta: -1 });
  });

  it("最後に押したのがアレンジならアレンジ、ピアノロールでひらいていればピアノロール", () => {
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

  it("キーのできごとから: ブラウザの拡大に渡さず、Rust には何も送らない", () => {
    const sent: unknown[] = [];
    const ev = new KeyboardEvent("keydown", { key: "-", code: "Minus", ctrlKey: true, cancelable: true });
    handleKeydown(ev, { state: () => fakeState(), send: (c) => sent.push(c), nav: { platform: "Linux x86_64" } });
    expect(ev.defaultPrevented).toBe(true);
    expect(sent).toEqual([]);
    expect(arrangeView.barPx).toBe(45);
  });
});
