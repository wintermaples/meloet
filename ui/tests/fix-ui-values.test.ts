// Bug sweep 2026-11-25: values of the mixer, the params panel rows, the typing box, the strength and swing bars,
// and the history rows of the arpeggio and the sound knobs.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, EffectKind, HistoryRow } from "../src/bridge";
import { EFFECT_PARAMS } from "../src/bridge/types";
import { fakeScene } from "../src/bridge/fake";
import { historyRowText } from "../src/components/history/stepText";
import { parseClickDb } from "../src/components/metronome";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { parseDb, parsePan } from "../src/components/mixer/scale";
import { typedKnob } from "../src/components/params/knobRows";
import ParamRow from "../src/components/params/ParamRow.svelte";
import { keyStep } from "../src/components/params/paramRows";
import PercentSlider from "../src/components/pianoroll/PercentSlider.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string): AppState => fakeScene(name)!.state;
const param = (effect: EffectKind, id: string) => EFFECT_PARAMS[effect].findIndex((p) => p.id === id);
const spec = (effect: EffectKind, id: string) => EFFECT_PARAMS[effect][param(effect, id)]!;

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
  vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  vi.useRealTimers();
});

function show(state: AppState, component: Component<any>, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const field = () => q('[data-test="param-input"]') as HTMLInputElement | null;
const T1 = '[data-strip="track-11"]';

function box(el: HTMLElement, width: number, height: number): void {
  el.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: width, bottom: height, width, height, toJSON: () => ({}) });
}
function pointer(el: Element, type: string, x: number, y: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 }));
  flushSync();
}
function frame(): void {
  vi.advanceTimersToNextFrame();
  flushSync();
}
function key(el: HTMLElement, k: string): void {
  el.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
  el.dispatchEvent(new KeyboardEvent("keyup", { key: k, bubbles: true }));
  flushSync();
}
function doubleClick(el: HTMLElement): void {
  el.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true, button: 0, detail: 2 }));
  flushSync();
}
function type(input: HTMLInputElement, value: string): void {
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
  flushSync();
}
/** Lets the promises of send and tick settle, then draws. */
async function answered(): Promise<void> {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  flushSync();
}

const mixRow = (auto: number | null) => ({
  insert: { strip: { kind: "track", id: 11 }, slot: 0 },
  effect: "reverb",
  param: param("reverb", "mix"),
  name: "ミックス",
  value: 30,
  selected: false,
  onselect: () => {},
  mark: { on: false, usable: false, add: () => {} },
  auto,
});

describe("UI-MIX-001: a drag whose part goes away is cancelled", () => {
  it("fader: unmounted while moving sends CancelMixPreview, and no later commit: false", () => {
    show(scene("mixer"), MixerStrips);
    const fader = q(`${T1} [data-test=mixer-fader]`)!;
    box(fader, 30, 100);
    pointer(fader, "pointerdown", 10, 50);
    pointer(fader, "pointermove", 10, 70);
    frame();
    pointer(fader, "pointermove", 10, 90);
    unmount(mounted!);
    mounted = null;
    frame();
    expect(app.sent.map((c) => c.type)).toEqual(["setGain", "cancelMixPreview"]);
  });

  it("built-in param row: unmounted while moving sends CancelMixPreview", () => {
    show(app.stateWith(() => {}), ParamRow, mixRow(null));
    const bar = q("[data-test=param-slider]")!;
    box(bar, 100, 12);
    pointer(bar, "pointerdown", 40, 6);
    frame();
    unmount(mounted!);
    mounted = null;
    frame();
    expect(app.sent.map((c) => c.type)).toEqual(["setParam", "cancelMixPreview"]);
  });
});

describe("UI-MIX-002 / UI-MIX-006: a param row moved by a lane", () => {
  it("after a drag whose answer leaves the song value as it was, the row follows the lane again", async () => {
    show(app.stateWith(() => {}), ParamRow, mixRow(50));
    const bar = q("[data-test=param-slider]")!;
    box(bar, 100, 12);
    pointer(bar, "pointerdown", 80, 6);
    pointer(bar, "pointerup", 80, 6);
    expect(app.sent.at(-1)).toMatchObject({ type: "setParam", value: 80, commit: true });
    expect(bar.getAttribute("aria-valuenow")).toBe("80");
    await answered();
    expect(bar.getAttribute("aria-valuenow")).toBe("50");
  });

  it("the start frame is where the handle was (the lane's value), not the song value", () => {
    show(app.stateWith(() => {}), ParamRow, mixRow(50));
    const bar = q("[data-test=param-slider]")!;
    box(bar, 100, 12);
    pointer(bar, "pointerdown", 80, 6);
    expect(q(".knob.ghost")!.style.left).toBe("50%");
  });
});

describe("UI-MIX-003: → moves at the bottom of log-scale rows", () => {
  it("compressor attack, delay interval and limiter release", () => {
    expect(keyStep(spec("compressor", "attack"), 0.1, 1, false)).toBe(0.2);
    expect(keyStep(spec("delay", "intervalMs"), 1, 1, false)).toBe(2);
    expect(keyStep(spec("limiter", "release"), 1, 1, false)).toBe(2);
    expect(keyStep(spec("limiter", "release"), 1, -1, false)).toBe(1);
  });
});

describe("UI-MIX-005: a typed value Rust keeps as it was", () => {
  it("after the answer the strip shows the song value, not the typed number", async () => {
    show(scene("mixer"), MixerStrips);
    const gain = () => q(`${T1} [data-test=mixer-gain]`);
    const before = text(gain());
    doubleClick(gain()!);
    type(field()!, "12");
    expect(app.sent).toEqual([{ type: "setGain", strip: { kind: "track", id: 11 }, db: 12, commit: true }]);
    await answered();
    expect(text(gain())).toBe(before);
  });
});

describe("UI-MIX-007: grabbing a fader right after arrow keys", () => {
  it("makes the key step first and starts from the shown value", () => {
    const s = scene("mixer");
    show(s, MixerStrips);
    const fader = q(`${T1} [data-test=mixer-fader]`)!;
    const db0 = s.song.tracks.find((t) => t.id === 11)!.gainDb!;
    box(fader, 30, 100);
    key(fader, "ArrowUp");
    key(fader, "ArrowUp");
    key(fader, "ArrowUp");
    pointer(fader, "pointerdown", 10, 50);
    expect(app.sent.at(-1)).toEqual({ type: "setGain", strip: { kind: "track", id: 11 }, db: db0 + 3, commit: true });
    expect(fader.dataset.db).toBe(String(db0 + 3));
    pointer(fader, "pointerup", 10, 50);
  });
});

describe("UI-PARITY-007: after an Enter that could not be read, leaving the box closes it", () => {
  it("mixer fader", async () => {
    show(scene("mixer"), MixerStrips);
    doubleClick(q(`${T1} [data-test=mixer-gain]`)!);
    type(field()!, "あ");
    expect(field()).not.toBeNull();
    await answered();
    field()!.dispatchEvent(new FocusEvent("blur"));
    flushSync();
    expect(field()).toBeNull();
    expect(app.sent).toEqual([]);
  });
});

describe("UI-PARITY-009: full-width digits", () => {
  it("dB, pan, metronome volume and sound knobs", () => {
    expect(parseDb("－３")).toBe(-3);
    expect(parseDb("－３．５ ｄＢ")).toBe(-3.5);
    expect(parsePan("Ｌ２０")).toBe(-20);
    expect(parsePan("２０")).toBe(20);
    expect(parseClickDb("－１２")).toBe(-12);
    expect(typedKnob("３５")).toBe(35);
    expect(typedKnob("３５％")).toBe(35);
  });
});

describe("UI-PARITY-013: the strength and swing bars show where the drag started", () => {
  it("a gray.7 frame at the old value while held, none after release", () => {
    show(app.stateWith(() => {}), PercentSlider, { value: 75, label: "強さ", test: "s", onchange: () => {} });
    const bar = q('[data-test="s"]')!;
    box(bar, 100, 12);
    expect(q(".knob.ghost")).toBeNull();
    pointer(bar, "pointerdown", 20, 6);
    expect(q(".knob.ghost")!.style.left).toBe("75%");
    pointer(bar, "pointerup", 20, 6);
    expect(q(".knob.ghost")).toBeNull();
  });
});

describe("CORE-PLAY-A-003: rows whose right column is the track do not name it again", () => {
  const ctx = scene("historyUndone3");
  const ref = { number: 1, name: "ピアノ" };
  it("「アルペジオを変える」 is the bare row; 「音のつまみを変える」 says the knob and the values", () => {
    const arp: HistoryRow = { index: 1, step: 1, name: { kind: "setArp" }, what: { kind: "track", track: ref }, place: { kind: "track", track: ref } };
    expect(historyRowText(arp, ctx)).toBe("「アルペジオを変える」");
    const knob: HistoryRow = {
      index: 2,
      step: 2,
      name: { kind: "setSoundKnob" },
      what: { kind: "soundKnob", track: ref, knob: "brightness", from: 50, to: 25 },
      place: { kind: "track", track: ref },
    };
    expect(historyRowText(knob, ctx)).toBe("「音のつまみを変える」・ 明るさ 50 → 25 %");
  });
});
