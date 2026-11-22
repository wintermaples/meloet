// Round 9: a double click on a number opens the same typing field that Enter (or the existing way) opens.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import Arrange from "../src/components/Arrange.svelte";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import SettingsDialog from "../src/components/SettingsDialog.svelte";
import TrackHeads from "../src/components/TrackHeads.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const scene = (name: string): AppState => fakeScene(name)!.state;

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

function show(state: AppState, component: Component<any>): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const field = () => q('[data-test="param-input"]') as HTMLInputElement | null;
function frame(): void {
  vi.advanceTimersToNextFrame();
  flushSync();
}

/** A real double click at (x, y): two presses and clicks, then dblclick. jsdom has no PointerEvent, so the pointer events are MouseEvents. */
function doubleClick(el: HTMLElement, clientX = 0, clientY = 0): void {
  const init = (detail: number) => ({ bubbles: true, cancelable: true, button: 0, detail, clientX, clientY });
  for (let i = 1; i <= 2; i++) {
    for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) {
      el.dispatchEvent(new MouseEvent(type, init(i)));
      flushSync();
    }
  }
  el.dispatchEvent(new MouseEvent("dblclick", init(2)));
  flushSync();
}
function type(input: HTMLInputElement, value: string, key: string): void {
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
  flushSync();
}

describe("Mixer: a double click on the number opens the typing field (spec 3 86-7)", () => {
  const inStrip = (key: string, sel: string) => q(`[data-strip="${key}"]`)!.querySelector<HTMLElement>(sel)!;

  const cases = [
    { name: "fader (track)", strip: "track-11", sel: "[data-test=mixer-gain]", input: "-3", sent: { type: "setGain", strip: { kind: "track", id: 11 }, db: -3, commit: true } },
    { name: "fader (master)", strip: "master", sel: "[data-test=mixer-gain]", input: "-1.5", sent: { type: "setGain", strip: { kind: "master" }, db: -1.5, commit: true } },
    { name: "fader (return)", strip: "return-a", sel: "[data-test=mixer-gain]", input: "-∞", sent: { type: "setGain", strip: { kind: "return", bus: "a" }, db: null, commit: true } },
    { name: "pan", strip: "track-11", sel: "[data-test=mixer-pan-text]", input: "L 30", sent: { type: "setPan", strip: { kind: "track", id: 11 }, pan: -30, commit: true } },
    { name: "send A", strip: "track-15", sel: "[data-test=mixer-send-text]", input: "-6", sent: { type: "setSend", track: 15, bus: "a", db: -6, commit: true } },
  ] as const;

  for (const c of cases) {
    it(`${c.name}: the presses send nothing, Enter sends one commit: true`, () => {
      show(scene("mixer"), MixerStrips);
      expect(field()).toBeNull();
      doubleClick(inStrip(c.strip, c.sel));
      frame();
      expect(app.sent).toEqual([]);
      const input = field()!;
      expect(input).not.toBeNull();
      expect(document.activeElement).toBe(input);
      type(input, c.input, "Enter");
      frame();
      expect(app.sent).toEqual([c.sent]);
      expect(field()).toBeNull();
    });

    it(`${c.name}: Esc closes the field and sends nothing`, () => {
      show(scene("mixer"), MixerStrips);
      doubleClick(inStrip(c.strip, c.sel));
      type(field()!, c.input, "Escape");
      frame();
      expect(field()).toBeNull();
      expect(app.sent).toEqual([]);
    });
  }

  it("the second send (B) opens its own field", () => {
    show(scene("mixer"), MixerStrips);
    const texts = [...q('[data-strip="track-15"]')!.querySelectorAll<HTMLElement>("[data-test=mixer-send-text]")];
    expect(texts).toHaveLength(2);
    doubleClick(texts[1]!);
    type(field()!, "-10", "Enter");
    frame();
    expect(app.sent).toEqual([{ type: "setSend", track: 15, bus: "b", db: -10, commit: true }]);
  });

  it("text that cannot be read keeps the field open and sends nothing", () => {
    show(scene("mixer"), MixerStrips);
    doubleClick(inStrip("track-11", "[data-test=mixer-gain]"));
    type(field()!, "abc", "Enter");
    frame();
    expect(field()).not.toBeNull();
    expect(app.sent).toEqual([]);
  });
});

describe("Sound settings: a double click on the latency number opens the same field as 「手で決める…」 (spec 3 94-19)", () => {
  const KEYBOARD = { id: "kb", name: "鍵盤 49", portName: "鍵盤 49 MIDI 1", address: "24:0" };
  const withAdvanced = () =>
    app.stateWith((s) => {
      s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
      s.workspace.settings = { draftOutput: null, advancedOpen: true };
    });

  it("Enter sends one settingsSetManualLatency, Esc sends nothing", () => {
    show(withAdvanced(), SettingsDialog);
    doubleClick(q('[data-test="latency-used"]')!);
    expect(app.sent).toEqual([]);
    type(field()!, "9.5", "Enter");
    expect(app.sent).toEqual([{ type: "settingsSetManualLatency", ms: 9.5 }]);
    expect(field()).toBeNull();

    app.sent.length = 0;
    doubleClick(q('[data-test="latency-used"]')!);
    type(field()!, "20", "Escape");
    expect(field()).toBeNull();
    expect(app.sent).toEqual([]);
  });
});

describe("Tempo band: a double click on a point's number opens the point's value field (spec: 値は 2 回押して打ちこむ)", () => {
  const WIDTH = 1000;
  const proto = HTMLElement.prototype;
  const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
  beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH }));
  afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

  /** The 104.00 point of the "tempo" scene is at bar 5. */
  const BAR5 = 4 * 4 * TPQ;

  it("does not add a point; Enter sends one moveTempoPoint", () => {
    show(scene("tempo"), Arrange);
    const label = [...target.querySelectorAll<HTMLElement>('[data-test="tempo-value"]')].find((l) => l.textContent === "104.00")!;
    doubleClick(label, parseFloat(label.style.left) + 8, parseFloat(label.style.top) + 4);
    expect(app.sent.some((c) => c.type === "addTempoPoint")).toBe(false);
    const input = q('[data-test="tempo-point-field"]') as HTMLInputElement;
    expect(input).not.toBeNull();
    const before = app.sent.length;
    type(input, "130", "Enter");
    expect(app.sent.slice(before)).toEqual([{ type: "moveTempoPoint", tick: BAR5, toTick: BAR5, bpm: 130 }]);
  });

  it("Esc closes the field and sends nothing", () => {
    show(scene("tempo"), Arrange);
    const label = [...target.querySelectorAll<HTMLElement>('[data-test="tempo-value"]')].find((l) => l.textContent === "104.00")!;
    doubleClick(label, parseFloat(label.style.left) + 8, parseFloat(label.style.top) + 4);
    const before = app.sent.length;
    type(q('[data-test="tempo-point-field"]') as HTMLInputElement, "130", "Escape");
    expect(q('[data-test="tempo-point-field"]')).toBeNull();
    expect(app.sent.slice(before)).toEqual([]);
  });
});

describe("Track head: a double click on the dB opens a field that sends the mixer fader's intent", () => {
  const keysTracks = (s: AppState) => s.song.tracks.filter((tr) => tr.kind.kind === "keys");
  const gainOf = (id: number) => {
    const i = app.current.state!.song.tracks.findIndex((tr) => tr.id === id);
    return [...target.querySelectorAll<HTMLElement>('[data-test="track-head"]')][i]!.querySelector<HTMLElement>('[data-test="track-gain"]')!;
  };

  it("the selected track: the presses send nothing, Enter sends one setGain commit: true", () => {
    const s = scene("mixer");
    const tr = keysTracks(s)[0]!;
    s.song.playTarget = tr.id;
    show(s, TrackHeads);
    doubleClick(gainOf(tr.id));
    expect(app.sent).toEqual([]);
    const input = field()!;
    expect(document.activeElement).toBe(input);
    type(input, "-4.5", "Enter");
    expect(app.sent).toEqual([{ type: "setGain", strip: { kind: "track", id: tr.id }, db: -4.5, commit: true }]);
    expect(field()).toBeNull();
  });

  it("another track: the presses only select it (as a click on the head always does; the fake state does not follow, so twice)", () => {
    const s = scene("mixer");
    const tr = keysTracks(s).find((x) => x.id !== s.song.playTarget)!;
    show(s, TrackHeads);
    doubleClick(gainOf(tr.id));
    expect(app.sent.length).toBeGreaterThan(0);
    expect(app.sent.every((c) => c.type === "selectTrack" && c.track === tr.id)).toBe(true);
    type(field()!, "-∞", "Enter");
    expect(app.sent.at(-1)).toEqual({ type: "setGain", strip: { kind: "track", id: tr.id }, db: null, commit: true });
  });

  it("Esc sends nothing; text that cannot be read keeps the field open", () => {
    const s = scene("mixer");
    const tr = keysTracks(s)[0]!;
    s.song.playTarget = tr.id;
    show(s, TrackHeads);
    doubleClick(gainOf(tr.id));
    type(field()!, "abc", "Enter");
    expect(field()).not.toBeNull();
    type(field()!, "-3", "Escape");
    expect(field()).toBeNull();
    expect(app.sent).toEqual([]);
  });

  it("a vocal track's head has no dB", () => {
    show(scene("audioTall"), TrackHeads);
    const tracks = app.current.state!.song.tracks;
    const heads = [...target.querySelectorAll<HTMLElement>('[data-test="track-head"]')];
    const vocal = tracks.findIndex((tr) => tr.kind.kind === "vocal");
    expect(vocal).toBeGreaterThanOrEqual(0);
    expect(heads[vocal]!.querySelector('[data-test="track-gain"]')).toBeNull();
  });
});

describe("Piano roll quantize strength and swing: a double click on 「75 %」 opens a field", () => {
  async function inspector(): Promise<void> {
    const s = fakeScene("pianoRoll")!.state;
    const track = s.song.tracks[0]!;
    const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
    const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
    shown.setShown({ clip, track, take });
    show(s, NoteInspector);
  }
  const quantize = (change: object) => ({ type: "setQuantize", division: 16, triplet: false, strength: 75, swing: 0, align: "start", ...change });

  it("strength: the presses send nothing, Enter sends one setQuantize (the same as a drag)", async () => {
    await inspector();
    doubleClick(q('[data-test="quantize-strength-value"]')!);
    expect(app.sent).toEqual([]);
    const input = field()!;
    expect(input.value).toBe("75");
    type(input, "40 %", "Enter");
    expect(app.sent).toEqual([quantize({ strength: 40 })]);
    expect(field()).toBeNull();
  });

  it("swing: out of range goes to the nearest end", async () => {
    await inspector();
    doubleClick(q('[data-test="quantize-swing-value"]')!);
    type(field()!, "250", "Enter");
    expect(app.sent).toEqual([quantize({ swing: 100 })]);
  });

  it("Esc sends nothing; the same value sends nothing; text that cannot be read keeps the field open", async () => {
    await inspector();
    doubleClick(q('[data-test="quantize-strength-value"]')!);
    type(field()!, "60", "Escape");
    expect(field()).toBeNull();
    doubleClick(q('[data-test="quantize-strength-value"]')!);
    type(field()!, "abc", "Enter");
    expect(field()).not.toBeNull();
    type(field()!, "75", "Enter");
    expect(field()).toBeNull();
    expect(app.sent).toEqual([]);
  });
});
