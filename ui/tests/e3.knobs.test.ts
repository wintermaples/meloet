// Round 12 E3: the knobs of a sound that comes with meloet in the parameters panel, the fake's copy, the status line and
// the history row.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, reduceMixer, type FakeState } from "../src/bridge/fake";
import { handAt, noteHand } from "../src/components/automation/handValue";
import { stepWhatText } from "../src/components/history/stepText";
import { typedKnob } from "../src/components/params/knobRows";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

let target: HTMLElement;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let mounted: any = null;
let frames: FrameRequestCallback[] = [];
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
  frames = [];
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
  vi.stubGlobal("cancelAnimationFrame", () => {});
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  vi.unstubAllGlobals();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
function nextFrame(): void {
  const run = frames;
  frames = [];
  for (const cb of run) cb(0);
  flushSync();
}
function pointer(el: Element, type: string, clientX: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX, clientY: 5, button: 0 }));
  flushSync();
}
function sizeBar(el: HTMLElement): void {
  el.getBoundingClientRect = () => ({ left: 0, right: 100, width: 100, top: 0, bottom: 12, height: 12, x: 0, y: 0, toJSON: () => ({}) });
}
const PIANO = 11;
const knobCommand = (knob: string, value: number, commit: boolean): Command =>
  ({ type: "setSoundKnob", track: PIANO, instrument: "grandPiano", knob, value, commit }) as Command;

describe("the four rows of a sound that comes with meloet", () => {
  it("「明るさ」「立ち上がり」「のび」「ゆれ」 with their values, the chosen row, and 「音源を入れかえる…」 under them", () => {
    show(ParamsPanel, scene("paramsSoundKnobs"), { onfold: () => {} });
    expect(text(q('[data-test="params-name"]'))).toBe("グランドピアノ");
    const rows = qa("[data-test=param-row]");
    expect(rows.map((r) => text(r.querySelector(".name")))).toEqual(["明るさ", "立ち上がり", "のび", "ゆれ"]);
    expect(qa("[data-test=param-value]").map(text)).toEqual(["35 %", "50 %", "70 %", "25 %"]);
    expect(rows.map((r) => r.classList.contains("selected"))).toEqual([true, false, false, false]);
    expect(q('[data-test="params-no-sound-params"]')).toBeNull();
    expect(text(q('[data-test="params-sound-replace"]'))).toBe("音源を入れかえる…");
    expect(qa("[data-test=param-automation]")).toHaveLength(4);
  });

  it("UI-PARITY-011: with the focus on the list, → moves the chosen row (「明るさ」 35 %) and goes no further", () => {
    show(ParamsPanel, scene("paramsSoundKnobs"), { onfold: () => {} });
    const list = q("[data-test=sound-knobs]")!;
    list.focus();
    const reached: string[] = [];
    const seen = (ev: KeyboardEvent) => reached.push(ev.key);
    document.addEventListener("keydown", seen);
    app.sent.length = 0;
    list.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }));
    flushSync();
    document.removeEventListener("keydown", seen);
    expect(app.sent).toEqual([knobCommand("brightness", 40, true)]);
    expect(reached).toEqual([]);
  });

  it("a drag is heard (commit: false once a frame) and is one step when let go; Esc sends CancelMixPreview", () => {
    show(ParamsPanel, scene("paramsSoundKnobs"), { onfold: () => {} });
    const slider = qa("[data-test=param-slider]")[2]!;
    sizeBar(slider);
    app.sent.length = 0;
    pointer(slider, "pointerdown", 40);
    pointer(slider, "pointermove", 45);
    expect(text(qa("[data-test=param-value]")[2]!)).toBe("45 %");
    nextFrame();
    pointer(slider, "pointerup", 60);
    expect(app.sent.filter((c) => c.type === "setSoundKnob")).toEqual([knobCommand("release", 45, false), knobCommand("release", 60, true)]);
    expect(app.sent.filter((c) => c.type === "selectParam")).toEqual([{ type: "selectParam", param: 2 }]);

    app.sent.length = 0;
    pointer(slider, "pointerdown", 10);
    nextFrame();
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(app.sent.map((c) => c.type)).toEqual(["selectParam", "setSoundKnob", "cancelMixPreview"]);
  });

  it("a value typed after a double click is sent with commit: true; a text that is no number sends nothing", () => {
    show(ParamsPanel, scene("paramsSoundKnobs"), { onfold: () => {} });
    app.sent.length = 0;
    const typeInto = (row: number, typed: string) => {
      qa("[data-test=param-value]")[row]!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
      flushSync();
      const input = q("input:not(.search)") as HTMLInputElement;
      input.value = typed;
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
      flushSync();
    };
    typeInto(3, "42 %");
    typeInto(3, "たくさん");
    expect(app.sent).toEqual([knobCommand("vibrato", 42, true)]);
    expect([typedKnob("35"), typedKnob("35%"), typedKnob("−5"), typedKnob(""), typedKnob("abc")]).toEqual([35, 35, -5, null, null]);
  });

  it("a drum kit's 「立ち上がり」 and 「ゆれ」 are disabled: nothing moves them and their mark cannot add a lane", () => {
    show(ParamsPanel, scene("paramsSoundDrums"), { onfold: () => {} });
    const rows = qa("[data-test=param-row]");
    expect(rows.map((r) => r.classList.contains("disabled"))).toEqual([false, true, false, true]);
    const slider = qa("[data-test=param-slider]")[1]!;
    sizeBar(slider);
    app.sent.length = 0;
    pointer(rows[1]!, "pointerdown", 5);
    pointer(slider, "pointerdown", 80);
    pointer(slider, "pointerup", 80);
    nextFrame();
    qa("[data-test=param-automation]")[1]!.click();
    expect(app.sent).toEqual([]);
    qa("[data-test=param-automation]")[2]!.click();
    expect(app.sent).toEqual([{ type: "addLane", home: { kind: "track", track: 13 }, target: { kind: "instrument", param: 2 } }]);
  });

  it("the hand's place for a lane being written", () => {
    noteHand(knobCommand("vibrato", 30, false));
    expect(handAt(PIANO, { kind: "instrument", param: 3 })).toBeCloseTo(0.3);
  });
});

describe("the fake's copy (as core)", () => {
  it("choosing a sound that comes with meloet shows its knobs and no 「パラメータがありません」; rows; a value let go", () => {
    const s = scene("paramsSoundDrums");
    const run = (state: FakeState, c: Command) => {
      const done = reduceMixer(state, state.shelf, state.song, c)!;
      return { ...state, params: done.params, notice: done.notice };
    };
    let st = run({ ...s, params: { ...s.params, selected: null } }, { type: "selectInstrument", track: PIANO });
    expect(st.notice).toBeNull();
    expect(st.params.selected).toEqual({
      kind: "sound",
      track: PIANO,
      sound: { kind: "builtin", instrument: "grandPiano" },
      knobs: { values: [50, 50, 50, 0], usable: [true, true, true, true], row: null },
    });
    st = run(st, { type: "selectParam", param: 3 });
    st = run(st, knobCommand("brightness", 130, true));
    st = run(st, knobCommand("attack", 20, false));
    st = run(st, { ...knobCommand("attack", 20, true), instrument: "strings" } as Command);
    expect(st.params.selected?.kind === "sound" && st.params.selected.knobs).toEqual({
      values: [100, 50, 50, 0],
      usable: [true, true, true, true],
      row: 3,
    });
    st = run(st, { type: "selectInstrument", track: PIANO + 2 });
    st = run(st, { type: "selectParam", param: 1 });
    expect(st.params.selected?.kind === "sound" && st.params.selected.knobs?.row).toBeNull();
  });
});

describe("words", () => {
  it("the status line while a row is chosen, and the history row", () => {
    show(StatusLine, scene("paramsSoundKnobs"));
    expect(text(q(".status"))).toContain("1 ピアノ ・ 「明るさ」をえらんでいます");
    const s = scene("paramsSoundKnobs");
    const what = stepWhatText(
      { kind: "setSoundKnob" },
      { kind: "soundKnob", track: { number: 1, name: "ピアノ" }, knob: "brightness", from: 50, to: 25 },
      { song: s.song, library: s.library },
    );
    expect(what).toBe("1 ピアノ ・ 明るさ 50 → 25 %");
  });
});
