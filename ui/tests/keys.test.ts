import { afterEach, describe, expect, it } from "vitest";
import { isTyping, keyAction, platformOf, type KeyInput } from "../src/keys/keys";

const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({
  key,
  ctrl: false,
  meta: false,
  alt: false,
  shift: false,
  ...mods,
});

describe("keyAction", () => {
  it("one-letter keys open and fold bands (A goes to the arrange)", () => {
    expect(keyAction(k("a"), false, "other")).toEqual({ kind: "arrange" });
    expect(keyAction(k("E"), false, "other")).toEqual({ kind: "toggleBand", band: "pianoRoll" });
    expect(keyAction(k("m"), false, "other")).toEqual({ kind: "toggleBand", band: "mixer" });
    expect(keyAction(k("h"), false, "other")).toEqual({ kind: "toggleBand", band: "history" });
    expect(keyAction(k("p"), false, "other")).toEqual({ kind: "toggleBand", band: "shelf" });
    expect(keyAction(k("u"), false, "other")).toEqual({ kind: "toggleBand", band: "params" });
  });

  it("one-letter keys do not work while typing", () => {
    for (const key of ["a", "e", "m", "h", "p", "u", "r", "l", "c", "t"]) {
      expect(keyAction(k(key), true, "other"), key).toBeNull();
    }
  });

  it("Space does not work while typing either and goes in as a space (Home works)", () => {
    expect(keyAction(k(" ", { code: "Space" }), true, "other")).toBeNull();
    expect(keyAction(k("Home"), true, "other")).toEqual({ kind: "toStart" });
  });

  it("Delete and Shift T do not work while typing either (Delete removes text in the box)", () => {
    expect(keyAction(k("Delete"), true, "other")).toBeNull();
    expect(keyAction(k("T", { shift: true }), true, "other")).toBeNull();
  });

  it("Esc and Mod keys work even while typing", () => {
    expect(keyAction(k("Escape"), true, "other")).toEqual({ kind: "back" });
    expect(keyAction(k(",", { ctrl: true }), true, "other")).toEqual({ kind: "audioSettings" });
    expect(keyAction(k("2", { ctrl: true, alt: true }), true, "other")).toEqual({ kind: "scale", scale: 125 });
  });

  it("Mod is Ctrl on Linux and Windows, Cmd on Mac", () => {
    expect(keyAction(k(",", { ctrl: true }), false, "other")).toEqual({ kind: "audioSettings" });
    expect(keyAction(k(",", { meta: true }), false, "other")).toBeNull();
    expect(keyAction(k(",", { meta: true }), false, "mac")).toEqual({ kind: "audioSettings" });
    expect(keyAction(k(",", { ctrl: true }), false, "mac")).toBeNull();
  });

  it("Mod+Alt+1/2/3 for scale. Reads code even when Option on Mac changes key", () => {
    expect(keyAction(k("1", { ctrl: true, alt: true, code: "Digit1" }), false, "other")).toEqual({ kind: "scale", scale: 100 });
    expect(keyAction(k("3", { ctrl: true, alt: true, code: "Digit3" }), false, "other")).toEqual({ kind: "scale", scale: 150 });
    expect(keyAction(k("¡", { meta: true, alt: true, code: "Digit1" }), false, "mac")).toEqual({ kind: "scale", scale: 100 });
    expect(keyAction(k("4", { ctrl: true, alt: true, code: "Digit4" }), false, "other")).toBeNull();
    expect(keyAction(k("1", { ctrl: true }), false, "other")).toBeNull();
  });

  it("a letter with a modifier key does not work as a one-letter key", () => {
    expect(keyAction(k("e", { ctrl: true }), false, "other")).toBeNull();
    expect(keyAction(k("E", { shift: true }), false, "other")).toBeNull();
    expect(keyAction(k("a", { alt: true }), false, "other")).toBeNull();
  });

  it("A when code is KeyA, even if key is not a Latin letter", () => {
    expect(keyAction(k("ち", { code: "KeyA" }), false, "other")).toEqual({ kind: "arrange" });
  });

  it("does nothing while the IME is converting", () => {
    expect(keyAction(k("Escape", { composing: true }), true, "other")).toBeNull();
  });

  it("↑ ↓ Enter only inside the output chooser", () => {
    expect(keyAction(k("ArrowUp"), false, "other", "outputChooser")).toEqual({ kind: "chooserUp" });
    expect(keyAction(k("ArrowDown"), false, "other", "outputChooser")).toEqual({ kind: "chooserDown" });
    expect(keyAction(k("Enter"), false, "other", "outputChooser")).toEqual({ kind: "chooserConfirm" });
    expect(keyAction(k("ArrowUp"), false, "other", "main")).toEqual({ kind: "nudge", steps: 0, keys: 1 });
    expect(keyAction(k("Enter"), false, "other", "main")).toBeNull();
  });

  it("Space, R, Home, L, C, T, Shift T, Delete (spec 4)", () => {
    expect(keyAction(k(" ", { code: "Space" }), false, "other")).toEqual({ kind: "playStop" });
    expect(keyAction(k("r"), false, "other")).toEqual({ kind: "record" });
    expect(keyAction(k("Home"), false, "other")).toEqual({ kind: "toStart" });
    expect(keyAction(k("l"), false, "other")).toEqual({ kind: "loop" });
    expect(keyAction(k("c"), false, "other")).toEqual({ kind: "metronome" });
    expect(keyAction(k("t"), false, "other")).toEqual({ kind: "stepTake", delta: 1 });
    expect(keyAction(k("T", { shift: true }), false, "other")).toEqual({ kind: "stepTake", delta: -1 });
    expect(keyAction(k("Delete"), false, "other")).toEqual({ kind: "deleteSelected" });
    expect(keyAction(k("ち", { code: "KeyT", shift: true }), false, "other")).toEqual({ kind: "stepTake", delta: -1 });
  });

  it("Space, L, Delete with Shift and T with Mod do nothing", () => {
    expect(keyAction(k(" ", { code: "Space", shift: true }), false, "other")).toBeNull();
    expect(keyAction(k("L", { shift: true }), false, "other")).toBeNull();
    expect(keyAction(k("Delete", { shift: true }), false, "other")).toBeNull();
    expect(keyAction(k("t", { ctrl: true }), false, "other")).toBeNull();
  });

  it("other keys do nothing", () => {
    expect(keyAction(k("x"), false, "other")).toBeNull();
    expect(keyAction(k("F5"), false, "other")).toBeNull();
  });
});

describe("platformOf", () => {
  it("Mac and the others", () => {
    expect(platformOf({ platform: "MacIntel" })).toBe("mac");
    expect(platformOf({ platform: "Linux x86_64" })).toBe("other");
    expect(platformOf({ platform: "Win32" })).toBe("other");
  });
});

describe("isTyping", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });
  const focus = (html: string) => {
    document.body.innerHTML = html;
    (document.body.querySelector("[data-f]") as HTMLElement).focus();
  };

  it("true when the cursor is in a text input, textarea or contenteditable", () => {
    focus(`<input data-f type="text">`);
    expect(isTyping()).toBe(true);
    focus(`<input data-f type="search">`);
    expect(isTyping()).toBe(true);
    focus(`<input data-f type="number">`);
    expect(isTyping()).toBe(true);
    focus(`<textarea data-f></textarea>`);
    expect(isTyping()).toBe(true);
    focus(`<div contenteditable="true"><span data-f tabindex="0">x</span></div>`);
    expect(isTyping()).toBe(true);
  });

  it("false for a button, a checkbox, or nothing", () => {
    focus(`<button data-f>x</button>`);
    expect(isTyping()).toBe(false);
    focus(`<input data-f type="checkbox">`);
    expect(isTyping()).toBe(false);
    focus(`<input data-f type="text" readonly>`);
    expect(isTyping()).toBe(false);
    focus(`<div contenteditable="false"><span data-f tabindex="0">x</span></div>`);
    expect(isTyping()).toBe(false);
    document.body.innerHTML = "";
    expect(isTyping()).toBe(false);
  });
});
