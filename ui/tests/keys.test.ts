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
  it("一文字のキーで帯をひらく・たたむ（A は アレンジへ）", () => {
    expect(keyAction(k("a"), false, "other")).toEqual({ kind: "arrange" });
    expect(keyAction(k("E"), false, "other")).toEqual({ kind: "toggleBand", band: "pianoRoll" });
    expect(keyAction(k("m"), false, "other")).toEqual({ kind: "toggleBand", band: "mixer" });
    expect(keyAction(k("h"), false, "other")).toEqual({ kind: "toggleBand", band: "history" });
    expect(keyAction(k("p"), false, "other")).toEqual({ kind: "toggleBand", band: "shelf" });
    expect(keyAction(k("u"), false, "other")).toEqual({ kind: "toggleBand", band: "params" });
  });

  it("文字を打っている最中は、一文字のキーが効かない", () => {
    for (const key of ["a", "e", "m", "h", "p", "u", "r", "l", "c"]) {
      expect(keyAction(k(key), true, "other"), key).toBeNull();
    }
  });

  it("文字を打っている最中は Space も効かず、空白として入る（Home は効く）", () => {
    expect(keyAction(k(" ", { code: "Space" }), true, "other")).toBeNull();
    expect(keyAction(k("Home"), true, "other")).toEqual({ kind: "notYet", what: "toStart" });
  });

  it("文字を打っている最中でも Esc と Mod つきは効く", () => {
    expect(keyAction(k("Escape"), true, "other")).toEqual({ kind: "back" });
    expect(keyAction(k(",", { ctrl: true }), true, "other")).toEqual({ kind: "audioSettings" });
    expect(keyAction(k("2", { ctrl: true, alt: true }), true, "other")).toEqual({ kind: "scale", scale: 125 });
  });

  it("Mod は Linux と Windows で Ctrl、Mac で Cmd", () => {
    expect(keyAction(k(",", { ctrl: true }), false, "other")).toEqual({ kind: "audioSettings" });
    expect(keyAction(k(",", { meta: true }), false, "other")).toBeNull();
    expect(keyAction(k(",", { meta: true }), false, "mac")).toEqual({ kind: "audioSettings" });
    expect(keyAction(k(",", { ctrl: true }), false, "mac")).toBeNull();
  });

  it("Mod+Alt+1/2/3 で拡大率。Mac の Option で key が変わっても code で読む", () => {
    expect(keyAction(k("1", { ctrl: true, alt: true, code: "Digit1" }), false, "other")).toEqual({ kind: "scale", scale: 100 });
    expect(keyAction(k("3", { ctrl: true, alt: true, code: "Digit3" }), false, "other")).toEqual({ kind: "scale", scale: 150 });
    expect(keyAction(k("¡", { meta: true, alt: true, code: "Digit1" }), false, "mac")).toEqual({ kind: "scale", scale: 100 });
    expect(keyAction(k("4", { ctrl: true, alt: true, code: "Digit4" }), false, "other")).toBeNull();
    expect(keyAction(k("1", { ctrl: true }), false, "other")).toBeNull();
  });

  it("修飾キーつきの一文字は、一文字のキーとしては効かない", () => {
    expect(keyAction(k("a", { ctrl: true }), false, "other")).toBeNull();
    expect(keyAction(k("M", { shift: true }), false, "other")).toBeNull();
    expect(keyAction(k("a", { alt: true }), false, "other")).toBeNull();
  });

  it("英字でない key でも、code が KeyA なら A", () => {
    expect(keyAction(k("ち", { code: "KeyA" }), false, "other")).toEqual({ kind: "arrange" });
  });

  it("IME で変換している最中は何もしない", () => {
    expect(keyAction(k("Escape", { composing: true }), true, "other")).toBeNull();
  });

  it("↑ ↓ Enter は出口えらびの中だけ", () => {
    expect(keyAction(k("ArrowUp"), false, "other", "outputChooser")).toEqual({ kind: "chooserUp" });
    expect(keyAction(k("ArrowDown"), false, "other", "outputChooser")).toEqual({ kind: "chooserDown" });
    expect(keyAction(k("Enter"), false, "other", "outputChooser")).toEqual({ kind: "chooserConfirm" });
    expect(keyAction(k("ArrowUp"), false, "other", "main")).toBeNull();
    expect(keyAction(k("Enter"), false, "other", "main")).toBeNull();
  });

  it("Space、R、Home、L、C は「まだ使えない」", () => {
    expect(keyAction(k(" ", { code: "Space" }), false, "other")).toEqual({ kind: "notYet", what: "playStop" });
    expect(keyAction(k("r"), false, "other")).toEqual({ kind: "notYet", what: "record" });
    expect(keyAction(k("Home"), false, "other")).toEqual({ kind: "notYet", what: "toStart" });
    expect(keyAction(k("l"), false, "other")).toEqual({ kind: "notYet", what: "loop" });
    expect(keyAction(k("c"), false, "other")).toEqual({ kind: "notYet", what: "metronome" });
  });

  it("ほかのキーは何もしない", () => {
    expect(keyAction(k("x"), false, "other")).toBeNull();
    expect(keyAction(k("F5"), false, "other")).toBeNull();
  });
});

describe("platformOf", () => {
  it("Mac と、それ以外", () => {
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

  it("文字の input、textarea、contenteditable にカーソルがあると true", () => {
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

  it("ボタン、チェックボックス、何もないときは false", () => {
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
