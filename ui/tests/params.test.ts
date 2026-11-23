// The params panel (mock F4-Mixer, R6-Delay) and 「…をえらんでいます」 in the status line.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, EffectKind } from "../src/bridge";
import { EFFECT_PARAMS } from "../src/bridge/types";
import { fakeScene } from "../src/bridge/fake";
import { allRows, fractionOf, keyStep, searchRows, stepRow, valueAt, valueText } from "../src/components/params/paramRows";
import { builtinOf } from "../src/components/params/selected";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import RightRail from "../src/components/RightRail.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const param = (effect: EffectKind, id: string) => EFFECT_PARAMS[effect].findIndex((p) => p.id === id);
const spec = (effect: EffectKind, id: string) => EFFECT_PARAMS[effect][param(effect, id)]!;
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

describe("how values are written (spec 3 「2.40 s」「20 ms」「−6.0 dB」「35 %」「6000 Hz」, the table of 19-3)", () => {
  it("units and decimal places", () => {
    expect(valueText("reverb", spec("reverb", "decay"), 2.4)).toBe("2.40 s");
    expect(valueText("reverb", spec("reverb", "preDelay"), 20)).toBe("20 ms");
    expect(valueText("reverb", spec("reverb", "highCut"), 6000)).toBe("6000 Hz");
    expect(valueText("delay", spec("delay", "feedback"), 35)).toBe("35 %");
    expect(valueText("compressor", spec("compressor", "threshold"), -6)).toBe("−6.0 dB");
    expect(valueText("compressor", spec("compressor", "ratio"), 4)).toBe("4.0 : 1");
    expect(valueText("compressor", spec("compressor", "attack"), 10)).toBe("10.0 ms");
    expect(valueText("compressor", spec("compressor", "release"), 100)).toBe("100 ms");
    expect(valueText("chorus", spec("chorus", "rate"), 0.8)).toBe("0.80 Hz");
    expect(valueText("eq4", spec("eq4", "lowMidWidth"), 1)).toBe("1.0 oct");
    expect(valueText("limiter", spec("limiter", "ceiling"), -0.3)).toBe("−0.3 dB");
  });

  it("「+」 only for ranges that also go below zero. 0 is 「0.0」 (not −0.0)", () => {
    expect(valueText("eq4", spec("eq4", "lowGain"), 3)).toBe("+3.0 dB");
    expect(valueText("eq4", spec("eq4", "lowGain"), -0.01)).toBe("0.0 dB");
    expect(valueText("compressor", spec("compressor", "makeup"), 3)).toBe("3.0 dB");
  });

  it("steps use the step name, switches on and off", () => {
    expect(valueText("delay", spec("delay", "intervalStep"), 2)).toBe("1/8");
    expect(valueText("delay", spec("delay", "intervalStep"), 1)).toBe("1/8 付点");
    expect(valueText("delay", spec("delay", "sync"), 1)).toBe("オン");
    expect(valueText("delay", spec("delay", "pingPong"), 0)).toBe("オフ");
  });

  it("bar: wide ranges of frequency and time are logarithmic, others are linear. bar → value rounds to the digits shown", () => {
    const highCut = spec("reverb", "highCut");
    expect(fractionOf(highCut, 1000)).toBe(0);
    expect(fractionOf(highCut, 20000)).toBe(1);
    expect(valueAt(highCut, fractionOf(highCut, 6000))).toBe(6000);
    const mix = spec("reverb", "mix");
    expect(fractionOf(mix, 30)).toBeCloseTo(0.3);
    expect(valueAt(mix, 0.333)).toBe(33);
    expect(valueAt(spec("reverb", "decay"), 1)).toBe(10);
    expect(valueAt(spec("reverb", "decay"), -1)).toBe(0.2);
    expect(valueAt(spec("chorus", "rate"), 0.5)).toBe(Math.round(Math.sqrt(0.05 * 5) * 100) / 100);
  });

  it("← → is 1/20 of the bar, Shift is one shown digit. Steps move one step and stop at the ends", () => {
    const mix = spec("reverb", "mix");
    expect(keyStep(mix, 50, 1, false)).toBe(55);
    expect(keyStep(mix, 50, -1, true)).toBe(49);
    expect(keyStep(mix, 100, 1, false)).toBe(100);
    expect(keyStep(spec("reverb", "decay"), 2.4, 1, true)).toBe(2.41);
    expect(keyStep(spec("delay", "intervalStep"), 4, 1, false)).toBe(4);
    expect(keyStep(spec("delay", "intervalStep"), 2, -1, false)).toBe(1);
  });
});

describe("row order", () => {
  it("reverb has 7 rows and no headings", () => {
    const groups = allRows("reverb", []);
    expect(groups.map((g) => g.group)).toEqual([null]);
    expect(groups[0]!.rows.map((r) => r.name)).toEqual(["響きの長さ", "部屋の大きさ", "プリディレイ", "高い音を落とす", "低い音を落とす", "広がり", "ミックス"]);
  });

  it("the delay 「間隔」 is one row. Steps when tempo sync is on, ms when off (A-14)", () => {
    const values = EFFECT_PARAMS.delay.map((p) => p.default);
    const on = allRows("delay", values)[0]!.rows;
    expect(on.map((r) => r.name)).toEqual(["間隔", "テンポに合わせる", "くり返し", "高い音を落とす", "低い音を落とす", "左右にゆれる", "ミックス"]);
    expect(on[0]!.param).toBe(param("delay", "intervalStep"));
    values[param("delay", "sync")] = 0;
    const off = allRows("delay", values)[0]!.rows;
    expect(off.length).toBe(7);
    expect(off[0]!.param).toBe(param("delay", "intervalMs"));
    expect(off[0]!.covers).toEqual([param("delay", "intervalStep"), param("delay", "intervalMs")]);
  });

  it("EQ is under 4 headings", () => {
    const groups = allRows("eq4", []);
    expect(groups.map((g) => [g.name, g.rows.map((r) => r.name)])).toEqual([
      ["低い音", ["高さ", "上げ下げ"]],
      ["中の低い音", ["高さ", "上げ下げ", "幅"]],
      ["中の高い音", ["高さ", "上げ下げ", "幅"]],
      ["高い音", ["高さ", "上げ下げ"]],
    ]);
  });

  it("search box: only rows whose name contains it. Headings with no rows left are not shown. ↑ ↓ stop at the ends", () => {
    const eq = allRows("eq4", []);
    expect(searchRows(eq, "幅").map((g) => [g.group, g.rows.length])).toEqual([
      ["lowMid", 1],
      ["highMid", 1],
    ]);
    expect(searchRows(allRows("reverb", []), "落とす")[0]!.rows.map((r) => r.name)).toEqual(["高い音を落とす", "低い音を落とす"]);
    expect(searchRows(allRows("reverb", []), "ない")).toEqual([]);
    const rows = allRows("reverb", [])[0]!.rows;
    expect(stepRow(rows, null, 1)).toBe(0);
    expect(stepRow(rows, null, -1)).toBe(6);
    expect(stepRow(rows, 0, -1)).toBe(0);
    expect(stepRow(rows, 2, 1)).toBe(3);
  });
});

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
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
function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
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
/** Makes the bar box 0 to 100 px (jsdom has no sizes). */
function sizeBar(el: HTMLElement): void {
  el.getBoundingClientRect = () => ({ left: 0, right: 100, width: 100, top: 0, bottom: 12, height: 12, x: 0, y: 0, toJSON: () => ({}) });
}

describe("params panel (mock F4-Mixer)", () => {
  it("name, built in, location, search box and count, rows and values. The mark of the chosen row", () => {
    show(RightRail, scene("mixer"));
    expect(text(q("[data-test=params-name]"))).toBe("リバーブ");
    // the insert's plugin is 「とばす B」 (spec 3 (86-1)). Built-in effects have no window button.
    expect(text(q("[data-test=params] .who"))).toBe("リバーブ 最初から入っている とばす B A リバーブ ・ インサート 1");
    expect(q("[data-test=params-window]")).toBeNull();
    expect((q("[data-test=params-search]") as HTMLInputElement).placeholder).toBe("名前");
    expect(text(q("[data-test=params-count]"))).toBe("7 / 7");
    expect(qa("[data-test=param-value]").map(text)).toEqual(["2.40 s", "60 %", "20 ms", "6000 Hz", "120 Hz", "100 %", "100 %"]);
    expect(qa("[data-test=param-row]").map((r) => r.classList.contains("selected"))).toEqual([true, false, false, false, false, false, false]);
    // the 「この子は窓を持ちません。」 row is not shown (spec 3 (Y8)).
    expect(target.textContent).not.toContain("窓を持ちません");
    // the mark for writing to automation can be pressed on a track insert (on returns and the master it is disabled. docs/history/design-notes.ja.md A-19 item 7).
    expect(qa("[data-test=param-row] button[aria-label=オートメーションに書く]").length).toBe(7);
  });

  it("pressing a row sends SelectParam (not sent for the chosen row)", () => {
    show(RightRail, scene("mixer"));
    const rows = qa("[data-test=param-row]");
    pointer(rows[2]!, "pointerdown", 5);
    pointer(rows[0]!, "pointerdown", 5);
    expect(app.sent).toEqual([{ type: "selectParam", param: 2 }]);
  });

  it("drag: the screen moves first, commit: false at most once per frame, and one commit: true on release. The original position is a gray.7 border", () => {
    show(RightRail, scene("mixer"));
    const slider = qa("[data-test=param-slider]")[1]!; // room size 60 %
    sizeBar(slider);
    app.sent.length = 0;
    pointer(slider, "pointerdown", 30);
    pointer(slider, "pointermove", 35);
    pointer(slider, "pointermove", 40);
    expect(text(qa("[data-test=param-value]")[1]!)).toBe("40 %");
    expect(slider.querySelector(".ghost")).not.toBeNull();
    nextFrame();
    pointer(slider, "pointermove", 45);
    nextFrame();
    pointer(slider, "pointerup", 50);
    const insert = { strip: { kind: "return", bus: "a" }, slot: 0 };
    expect(app.sent.filter((c) => c.type === "setParam")).toEqual([
      { type: "setParam", insert, effect: "reverb", param: 1, value: 40, commit: false },
      { type: "setParam", insert, effect: "reverb", param: 1, value: 45, commit: false },
      { type: "setParam", insert, effect: "reverb", param: 1, value: 50, commit: true },
    ]);
    expect(slider.querySelector(".ghost")).toBeNull();
    // keeps the released value until the Rust answer comes.
    expect(text(qa("[data-test=param-value]")[1]!)).toBe("50 %");
  });

  it("Esc during a drag goes back to the original value and sends CancelMixPreview. No commit is sent", () => {
    show(RightRail, scene("mixer"));
    const slider = qa("[data-test=param-slider]")[1]!;
    sizeBar(slider);
    app.sent.length = 0;
    pointer(slider, "pointerdown", 30);
    nextFrame();
    expect(cancelDrag()).toBe(true);
    flushSync();
    pointer(slider, "pointerup", 30);
    expect(app.sent.map((c) => c.type)).toEqual(["selectParam", "setParam", "cancelMixPreview"]);
    expect(text(qa("[data-test=param-value]")[1]!)).toBe("60 %");
    expect(cancelDrag()).toBe(false);
  });

  it("pressing and releasing without moving sends nothing (the value is the same)", () => {
    show(RightRail, scene("mixer"));
    const slider = qa("[data-test=param-slider]")[0]!; // decay time
    app.sent.length = 0;
    // on a bar with no size, the pressed place keeps the current value.
    pointer(slider, "pointerdown", 30);
    pointer(slider, "pointerup", 30);
    expect(app.sent.filter((c) => c.type === "setParam")).toEqual([]);
  });

  it("← → move one step at a time. ↑ ↓ choose the row", () => {
    show(RightRail, scene("mixer"));
    const slider = qa("[data-test=param-slider]")[1]!;
    app.sent.length = 0;
    slider.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }));
    slider.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true }));
    flushSync();
    expect(app.sent).toEqual([
      { type: "setParam", insert: { strip: { kind: "return", bus: "a" }, slot: 0 }, effect: "reverb", param: 1, value: 65, commit: true },
      { type: "selectParam", param: 1 },
    ]);
  });

  it("search box: the count changes, and with no match the text and 「さがす枠を空にする」", () => {
    show(RightRail, scene("mixer"));
    const input = q("[data-test=params-search]") as HTMLInputElement;
    input.value = "落とす";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    expect(text(q("[data-test=params-count]"))).toBe("2 / 7");
    expect(qa("[data-test=param-row] .name").map(text)).toEqual(["高い音を落とす", "低い音を落とす"]);
    input.value = "ゆれ";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    expect(text(q("[data-test=params-count]"))).toBe("0 / 7");
    expect(text(q("[data-test=params-none] span"))).toBe("「ゆれ」に合うパラメータはありません。");
    q("[data-test=params-clear]")!.click();
    flushSync();
    expect(qa("[data-test=param-row]").length).toBe(7);
  });

  it("with nothing chosen, two rows below the heading (spec 3 (86-5))", () => {
    show(RightRail, scene("params"));
    expect(text(q("[data-test=params]"))).toBe(
      "▾ パラメータ U ミキサーでインサートの枠をえらぶと、ここにパラメータが出ます。 音源の枠をえらぶと、音源のパラメータが出ます。",
    );
    expect(q("[data-test=params-nothing] .m-lbl")?.textContent).toBe("音源の枠をえらぶと、音源のパラメータが出ます。");
  });
});

describe("gate, de-esser and saturation (round 12 M6)", () => {
  it("the names, the rows and the values as they are new", () => {
    const rows = () => qa("[data-test=param-row]").map(text);
    show(RightRail, scene("paramsGate"));
    expect(text(q("[data-test=params] .who"))).toContain("ゲート");
    expect(text(q("[data-test=params-count]"))).toBe("3 / 3");
    expect(qa("[data-test=param-value]").map(text)).toEqual(["−45.0 dB", "−60.0 dB", "120 ms"]);
    expect(rows().join(" ")).toMatch(/しきい値.*下げる量.*戻る速さ/);
    unmount(mounted!);
    show(RightRail, scene("paramsDeEsser"));
    expect(text(q("[data-test=params] .who"))).toContain("ディエッサー");
    expect(qa("[data-test=param-value]").map(text)).toEqual(["6500 Hz", "−30.0 dB", "8.0 dB"]);
    expect(rows().join(" ")).toMatch(/周波数.*しきい値.*下げる量/);
    unmount(mounted!);
    show(RightRail, scene("paramsSaturation"));
    expect(text(q("[data-test=params] .who"))).toContain("サチュレーション");
    expect(qa("[data-test=param-value]").map(text)).toEqual(["6.0 dB", "20000 Hz", "100 %", "0.0 dB"]);
    expect(rows().join(" ")).toMatch(/ひずみ.*明るさ.*まぜる量.*出る大きさ/);
  });

  it("values are written like the other effects' (dB one place, Hz and ms none, 「+」 where the range goes below zero)", () => {
    expect(valueText("gate", spec("gate", "range"), -12)).toBe("−12.0 dB");
    expect(valueText("gate", spec("gate", "release"), 5)).toBe("5 ms");
    expect(valueText("deEsser", spec("deEsser", "reduction"), 18)).toBe("18.0 dB");
    expect(valueText("saturation", spec("saturation", "output"), 6)).toBe("+6.0 dB");
    expect(valueText("saturation", spec("saturation", "drive"), 36)).toBe("36.0 dB");
    expect(valueText("saturation", spec("saturation", "tone"), 1000)).toBe("1000 Hz");
  });
});

describe("delay (mock R6-Delay)", () => {
  it("interval is steps (◂ 5 squares ▸), tempo sync is on, ping-pong is off", () => {
    show(RightRail, scene("paramsDelay"));
    expect(text(q("[data-test=params] .who"))).toBe("ディレイ 最初から入っている とばす B B ディレイ ・ インサート 1");
    expect(text(q("[data-test=params-count]"))).toBe("7 / 7");
    expect(qa("[data-test=param-value]").map(text)).toEqual(["1/8", "オン", "35 %", "5000 Hz", "200 Hz", "オフ", "100 %"]);
    expect(qa(".square").map((s) => s.classList.contains("on"))).toEqual([false, false, true, false, false]);
    expect(qa("[data-test=param-switch]").map((s) => s.getAttribute("aria-checked"))).toEqual(["true", "false"]);
    expect(qa("[data-test=param-row]").some((r) => r.classList.contains("selected"))).toBe(false);
  });

  it("step ◂ ▸ and squares, and switches, are one step when pressed", () => {
    show(RightRail, scene("paramsDelay"));
    app.sent.length = 0;
    q("[data-test=param-next]")!.click();
    qa(".square")[0]!.click();
    qa("[data-test=param-switch]")[1]!.click();
    const insert = { strip: { kind: "return", bus: "b" }, slot: 0 };
    expect(app.sent.filter((c) => c.type === "setParam")).toEqual([
      { type: "setParam", insert, effect: "delay", param: 0, value: 3, commit: true },
      { type: "setParam", insert, effect: "delay", param: 0, value: 0, commit: true },
      { type: "setParam", insert, effect: "delay", param: param("delay", "pingPong"), value: 1, commit: true },
    ]);
  });

  it("when tempo sync is off, the interval is an ms bar", () => {
    const s = scene("paramsDelay");
    builtinOf(s.params)!.values[param("delay", "sync")] = 0;
    show(RightRail, s);
    expect(qa("[data-test=param-row]").length).toBe(7);
    expect(text(qa("[data-test=param-value]")[0]!)).toBe("250 ms");
    expect(qa("[data-test=param-row]")[0]!.querySelector("[data-test=param-slider]")).not.toBeNull();
  });
});

describe("vertical heading and status line", () => {
  it("when the parameters are behind, the vertical heading shows the chosen plugin's name and count (mock F-WindowOpen)", () => {
    const s = scene("mixer");
    s.workspace.layout.rightFront = "shelf";
    show(RightRail, s);
    expect(text(q("[data-test=params-tab-count]"))).toBe("リバーブ 7");
  });

  it("status line: 「ループ 5–9 ・ A リバーブの「響きの長さ」をえらんでいます」 (mock F4-Mixer)", () => {
    show(StatusLine, scene("mixer"));
    expect(text(q(".status"))).toContain("ループ 5–9 ・ A リバーブの「響きの長さ」をえらんでいます");
  });

  it("status line: a track strip is 「2 ベース ・ コンプレッサーの「…」」 (spec 3 (86-10)), and the case with no loop", () => {
    const s = scene("mixer");
    s.status.message = { id: "paramSelected", loopBars: null, insert: { strip: { kind: "track", id: s.song.tracks[1]!.id }, slot: 0 }, effect: "compressor", param: 1 };
    show(StatusLine, s);
    expect(text(q(".status"))).toContain(`${s.song.tracks[1]!.number} ${s.song.tracks[1]!.name} ・ コンプレッサーの「おさえる強さ」をえらんでいます`);
    expect(text(q(".status"))).not.toContain("ループ");
  });
});
