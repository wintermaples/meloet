// R2 「キーで弾く」: the PC keyboard plays notes. The key map, what stays a shortcut, repeats, typing, Esc and Mod K, letting
// go (the window, the octave, the switch), the input meter, the status line, the settings, the menu row, the fake.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TYPED_KEYBOARD_ID, type AppState, type Command, type ServerMessage } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { typedKeyboard, typedOn } from "../src/bridge/fakeTyped";
import InputMeter from "../src/components/InputMeter.svelte";
import { keyNames, topMenus, type ListRow } from "../src/components/menu/menus";
import { meterFit, type MeterFitInput } from "../src/components/meterFit";
import SettingsDialog from "../src/components/SettingsDialog.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { attachKeys, handleKeydown, handleKeyup, type KeyDeps } from "../src/keys/attach";
import { keyListGroups } from "../src/keys/keyList";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { resetTypedKeys, typedHeld, typedKeys, typedNote } from "../src/keys/typed.svelte";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });

/** A state with 「キーで弾く」 on (or off), a keyboard and an output. */
function typedState(on = true): AppState {
  const s = scene("keepPlayedReady");
  if (on) s.keyboards = typedOn(s.keyboards, false);
  return s;
}

interface Rig {
  deps: KeyDeps;
  sent: Command[];
  posted: Command[];
  state: { now: AppState };
}

function rig(state: AppState, offset: number | null = null): Rig {
  const sent: Command[] = [];
  const posted: Command[] = [];
  const box = { now: state };
  const deps: KeyDeps = {
    state: () => box.now,
    send: (c) => {
      sent.push(c);
      if (c.type === "typedKeys") box.now = { ...box.now, keyboards: c.on ? typedOn(box.now.keyboards, false) : { ...box.now.keyboards, typedKeys: false } };
    },
    post: (c) => void posted.push(c),
    clockOffsetUs: () => (offset === null ? new Promise<number>(() => {}) : Promise.resolve(offset)),
    nav: { platform: "Linux x86_64" },
    playTick: () => 0,
  };
  return { deps, sent, posted, state: box };
}

function keydown(r: Rig, code: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const key = init.key ?? (code.startsWith("Key") ? code.slice(3).toLowerCase() : code.startsWith("Digit") ? code.slice(5) : code);
  const ev = new KeyboardEvent("keydown", { code, key, cancelable: true, ...init });
  handleKeydown(ev, r.deps);
  return ev;
}

function keyup(r: Rig, code: string): void {
  handleKeyup(new KeyboardEvent("keyup", { code, key: code.slice(3).toLowerCase(), cancelable: true }), document, r.deps);
}

const keys = (posted: Command[]) => posted.map((c) => (c.type === "typedKey" ? [c.note, c.down] : c.type));

beforeEach(() => {
  resetTypedKeys();
  app.reset();
});

describe("the wording of R2", () => {
  it("is the spec's", () => {
    expect(t("input.typedKeys")).toBe("キーで弾く");
    expect(t("input.typedKeyboard")).toBe("パソコンのキーボード");
    expect(t("input.octave")).toBe("オクターブ");
    expect(t("menu.typedKeys")).toBe("キーで弾く");
    expect(t("msg.typedKeysOn")).toBe("キーで弾く：オン。1文字のショートカットは止まっています。Z ／ X でオクターブ、Esc でやめる");
  });
});

describe("Mod K", () => {
  it("is 「キーで弾く」 (Cmd on the Mac), also while typing; not with Shift or Alt", () => {
    expect(keyAction(k("k", { ctrl: true }), false, "other")).toEqual({ kind: "typedKeys" });
    expect(keyAction(k("k", { ctrl: true }), true, "other")).toEqual({ kind: "typedKeys" });
    expect(keyAction(k("", { ctrl: true, code: "KeyK" }), false, "other")).toEqual({ kind: "typedKeys" });
    expect(keyAction(k("k", { meta: true }), false, "mac")).toEqual({ kind: "typedKeys" });
    expect(keyAction(k("k", { ctrl: true, shift: true }), false, "other")).toBeNull();
    expect(keyAction(k("k", { ctrl: true, alt: true }), false, "other")).toBeNull();
  });

  it("turns it on and off; off lets go of the keys held first", () => {
    const r = rig(typedState(false));
    const ev = keydown(r, "KeyK", { ctrlKey: true });
    expect(ev.defaultPrevented).toBe(true);
    expect(r.sent).toEqual([{ type: "typedKeys", on: true }]);
    keydown(r, "KeyA");
    keydown(r, "KeyK", { ctrlKey: true });
    expect(r.sent.at(-1)).toEqual({ type: "typedKeys", on: false });
    expect(keys(r.posted)).toEqual([
      [48, true],
      [48, false],
    ]);
    keyup(r, "KeyA");
    expect(r.posted).toHaveLength(2);
  });

  it("is in the key list under 「再生と録音」", () => {
    const play = keyListGroups(keyNames("other")).find((g) => g.group === "play")!;
    expect(play.rows.filter((r) => r.label === "キーで弾く")).toEqual([{ label: "キーで弾く", keys: "Ctrl K" }]);
  });

  it("is the last row of the 「再生」 menu, checked while on", () => {
    for (const on of [false, true]) {
      const r = rig(typedState(on));
      const play = topMenus(r.state.now, r.deps, "other").find((m) => m.label === "再生")!;
      const row = play.entries.filter((e): e is ListRow => e.kind === "row").at(-1)!;
      expect([row.label, row.key, row.checked, row.usable]).toEqual(["キーで弾く", "Ctrl K", on, true]);
      row.run!();
      expect(r.sent).toEqual([{ type: "typedKeys", on: !on }]);
    }
  });
});

describe("the keys", () => {
  it("by their place: A S D F G H J K L ; and W E, T Y U, O P from C3; Z ／ X move it from C0 to C7", () => {
    const white = ["KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon"].map((c) => typedNote(c, 3));
    expect(white).toEqual([48, 50, 52, 53, 55, 57, 59, 60, 62, 64]);
    const black = ["KeyW", "KeyE", "KeyT", "KeyY", "KeyU", "KeyO", "KeyP"].map((c) => typedNote(c, 3));
    expect(black).toEqual([49, 51, 54, 56, 58, 61, 63]);
    expect(typedNote("KeyQ", 3)).toBeNull();
    const r = rig(typedState());
    expect(typedKeys.octave).toBe(3);
    for (let i = 0; i < 6; i++) keydown(r, "KeyZ");
    expect(typedKeys.octave).toBe(0);
    keydown(r, "KeyA");
    for (let i = 0; i < 9; i++) keydown(r, "KeyX");
    expect(typedKeys.octave).toBe(7);
    keydown(r, "Semicolon", { key: ";" });
    expect(keys(r.posted)).toEqual([
      [12, true],
      [12, false],
      [112, true],
    ]);
    expect(r.sent).toEqual([]);
  });

  it("play once while held, the repeats are dropped, and keyup lets go", () => {
    const r = rig(typedState());
    expect(keydown(r, "KeyA").defaultPrevented).toBe(true);
    expect(keydown(r, "KeyA", { repeat: true }).defaultPrevented).toBe(true);
    keydown(r, "KeyA");
    keydown(r, "KeyW");
    keyup(r, "KeyA");
    keyup(r, "KeyA");
    expect(keys(r.posted)).toEqual([
      [48, true],
      [49, true],
      [48, false],
    ]);
    expect(typedHeld()).toEqual([49]);
  });

  it("stop the one-letter and digit shortcuts, also those of keys that play nothing", () => {
    const r = rig(typedState());
    for (const code of ["KeyA", "KeyE", "KeyM", "KeyR", "KeyQ", "KeyC", "KeyB", "Digit1", "Numpad2"]) {
      expect(keydown(r, code).defaultPrevented, code).toBe(true);
    }
    expect(r.sent).toEqual([]);
    // Off: the same letters are shortcuts again.
    const off = rig(typedState(false));
    keydown(off, "KeyE");
    keydown(off, "KeyM");
    expect(off.sent).toEqual([
      { type: "toggleBand", band: "pianoRoll" },
      { type: "toggleBand", band: "mixer" },
    ]);
    expect(off.posted).toEqual([]);
  });

  it("leave Space, Mod keys, Shift keys, arrows, Home, Delete and F keys as they were", () => {
    const r = rig(typedState());
    keydown(r, "Space", { key: " " });
    keydown(r, "KeyS", { ctrlKey: true });
    keydown(r, "KeyR", { shiftKey: true, key: "R" });
    keydown(r, "Home", { key: "Home" });
    keydown(r, "ArrowLeft", { key: "ArrowLeft" });
    keydown(r, "Delete", { key: "Delete" });
    expect(r.sent.map((c) => c.type)).toEqual(["playStop", "saveSong", "keepPlayed", "goToStart", "deleteSelected"]);
    expect(r.posted).toEqual([]);
  });

  it("are not played while text is typed, nor while the input method has them", () => {
    const r = rig(typedState());
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    expect(keydown(r, "KeyA").defaultPrevented).toBe(false);
    input.remove();
    keydown(r, "KeyA", { isComposing: true });
    keydown(r, "KeyS", { key: "Process" });
    expect(r.posted).toEqual([]);
  });

  it("take the event's time on Rust's clock once the offset is known (0 before: Rust's now)", async () => {
    const r = rig(typedState(), 5_000_000);
    keydown(r, "KeyA");
    await Promise.resolve();
    const ev = new KeyboardEvent("keydown", { code: "KeyS", key: "s", cancelable: true });
    Object.defineProperty(ev, "timeStamp", { value: performance.now() - 2 });
    handleKeydown(ev, r.deps);
    const [first, second] = r.posted as Extract<Command, { type: "typedKey" }>[];
    expect(first!.tUs).toBe(0);
    expect(second!.tUs).toBe(Math.round(ev.timeStamp * 1000 + 5_000_000));
  });
});

describe("letting go", () => {
  it("Esc turns it off after the menus and before going back, letting go of the keys", () => {
    const r = rig(typedState());
    keydown(r, "KeyA");
    keydown(r, "Escape", { key: "Escape" });
    expect(r.sent).toEqual([{ type: "typedKeys", on: false }]);
    expect(keys(r.posted)).toEqual([
      [48, true],
      [48, false],
    ]);
    keydown(r, "Escape", { key: "Escape" });
    expect(r.sent.at(-1)).toEqual({ type: "back" });
  });

  it("the octave: the keys held are let go before it moves", () => {
    const r = rig(typedState());
    keydown(r, "KeyA");
    keydown(r, "KeyK");
    keydown(r, "KeyX");
    keyup(r, "KeyA");
    keydown(r, "KeyA");
    expect(keys(r.posted)).toEqual([
      [48, true],
      [60, true],
      [48, false],
      [60, false],
      [60, true],
    ]);
  });

  it("the window losing the keys (blur, hidden) lets go of all; attachKeys listens", () => {
    const r = rig(typedState());
    const detach = attachKeys(r.deps);
    document.dispatchEvent(new KeyboardEvent("keydown", { code: "KeyA", key: "a", cancelable: true }));
    document.dispatchEvent(new KeyboardEvent("keydown", { code: "KeyD", key: "d", cancelable: true }));
    window.dispatchEvent(new Event("blur"));
    document.dispatchEvent(new KeyboardEvent("keydown", { code: "KeyF", key: "f", cancelable: true }));
    Object.defineProperty(document, "visibilityState", { value: "hidden", configurable: true });
    document.dispatchEvent(new Event("visibilitychange"));
    Object.defineProperty(document, "visibilityState", { value: "visible", configurable: true });
    detach();
    window.dispatchEvent(new Event("blur"));
    expect(keys(r.posted)).toEqual([
      [48, true],
      [52, true],
      [48, false],
      [52, false],
      [53, true],
      [53, false],
    ]);
  });

  it("turned off elsewhere: Rust let go already, so a later keyup says nothing", () => {
    const r = rig(typedState());
    keydown(r, "KeyA");
    r.state.now = typedState(false);
    keydown(r, "ArrowLeft", { key: "ArrowLeft" });
    keyup(r, "KeyA");
    expect(keys(r.posted)).toEqual([[48, true]]);
  });
});

// ---- the screen ----

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);

describe("the input meter", () => {
  // jsdom lays nothing out: a wide row, so the switch and the octave have their room.
  beforeEach(() => Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, get: () => 4000 }));
  afterEach(() => delete (HTMLElement.prototype as { clientWidth?: number }).clientWidth);

  it("names the PC keyboard, with 「+1」; the switch is on and the octave shows", () => {
    show(InputMeter, scene("typedKeys"));
    expect(q('[data-test="keyboard-name"]')!.textContent).toBe("パソコンのキーボード");
    expect(q('[data-test="other-keyboards"]')!.textContent?.trim()).toBe("+1");
    const button = q('[data-test="typed-keys"]')!;
    expect([button.textContent, button.classList.contains("on"), button.getAttribute("aria-pressed")]).toEqual(["キーで弾く", true, "true"]);
    expect(q('[data-test="typed-octave"]')!.textContent).toBe("C3");
    expect(button.previousElementSibling!.classList.contains("who")).toBe(true);
  });

  it("the switch: off shows no octave; pressing sends the intent", () => {
    show(InputMeter, scene("keepPlayedReady"));
    const button = q('[data-test="typed-keys"]')!;
    expect(button.classList.contains("on")).toBe(false);
    expect(q('[data-test="typed-octave"]')).toBeNull();
    button.click();
    expect(app.sent).toEqual([{ type: "typedKeys", on: true }]);
    show(InputMeter, scene("typedKeys"));
    q('[data-test="typed-keys"]')!.click();
    expect(app.sent.at(-1)).toEqual({ type: "typedKeys", on: false });
  });

  it("the switch, then the octave, show only in the room the rest leaves: they never hide what spec 1 shows", () => {
    // Without them: 680 + 108 + 90 + 110 = 988; the switch 70, the octave 50.
    const full: MeterFitInput = { meter: "full", scale: 100, nameChars: 5, available: 1108, rest: 800, name: 60, pitchBend: 90, modulation: 110, typed: [70, 50] };
    expect(meterFit(full)).toMatchObject({ typed: 2, modulation: true });
    expect(meterFit({ ...full, available: 1107 })).toMatchObject({ typed: 1, modulation: true });
    expect(meterFit({ ...full, available: 1057 })).toMatchObject({ typed: 0, modulation: true });
    expect(meterFit({ ...full, available: 988 })).toMatchObject({ typed: 0, modulation: true, pitchBend: true });
    // Modulation hidden as it was without them (988 > 987): the switch takes the room left (878 + 70).
    expect(meterFit({ ...full, available: 987 })).toMatchObject({ typed: 1, modulation: false, pitchBend: true });
    // Slim, without them: 500 + 176 + 90 + 110 = 876.
    const slim: MeterFitInput = { ...full, meter: "slim", name: 200, rest: 620, available: 996 };
    expect(meterFit(slim)).toMatchObject({ typed: 2, modulation: true, nameWidth: "size.meter.name.slim" });
    expect(meterFit({ ...slim, available: 945 })).toMatchObject({ typed: 0, modulation: true });
    expect(meterFit({ ...slim, available: 875 })).toMatchObject({ typed: 1, modulation: false, pitchBend: true });
  });
});

describe("the status line and the settings", () => {
  it("say why the letters do nothing, and that the input method keeps the keys", () => {
    show(StatusLine, scene("typedKeys"));
    expect(target.textContent).toContain(`${t("msg.typedKeysOn")} ・ ${t("msg.typedKeysIme")}`);
  });

  it("「見つかりました」 names the PC keyboard when it is the only one", () => {
    const s = fakeState();
    s.keyboards = typedOn(s.keyboards, false);
    s.status.message = { id: "keyboardFound", keyboard: "" };
    show(StatusLine, s);
    expect(target.textContent).toContain("パソコンのキーボード が見つかりました。");
  });

  it("the sound settings list it among the keyboards", () => {
    show(SettingsDialog, scene("typedKeysSettings"));
    expect(target.textContent).toContain("パソコンのキーボード");
    expect(target.textContent).toContain("Keystation 49 MK3");
  });
});

describe("the fake", () => {
  it("takes the switch and the keys as Rust does", async () => {
    const transport = createFakeTransport(scene("keepPlayedReady"));
    const messages: ServerMessage[] = [];
    await transport.subscribe(
      (m) => void messages.push(m),
      () => {},
    );
    await transport.post({ type: "typedKey", note: 60, down: true, tUs: 1 });
    expect(messages).toEqual([]);
    await transport.send({ type: "typedKeys", on: true });
    let state = (await transport.snapshot()).state;
    expect(state.keyboards.list.at(-1)).toEqual(typedKeyboard());
    expect(state.status.message).toEqual({ id: "typedKeysOn" });
    await transport.post({ type: "typedKey", note: 60, down: true, tUs: 7 });
    await transport.post({ type: "typedKey", note: 60, down: true, tUs: 8 });
    state = (await transport.snapshot()).state;
    expect(state.keyboards.active).toBe(TYPED_KEYBOARD_ID);
    await transport.send({ type: "typedKeys", on: false });
    state = (await transport.snapshot()).state;
    expect([state.keyboards.typedKeys, state.keyboards.list.some((k) => k.id === TYPED_KEYBOARD_ID)]).toEqual([false, false]);
    const notes = messages.filter((m) => m.kind === "event").map((m) => (m.kind === "event" && m.type === "note" ? [m.note, m.on, m.velocity] : null));
    expect(notes).toEqual([
      [60, true, 100],
      [60, false, 0],
    ]);
  });
});
