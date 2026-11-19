// 第 4 回 区切り3: パラメータの欄（モック F4-Mixer ・ R6-Delay）と状態行の「…をえらんでいます」。
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

describe("値の書きかた（仕様書 3 章「2.40 s」「20 ms」「−6.0 dB」「35 %」「6000 Hz」、19-3 の表）", () => {
  it("単位と小数のけた", () => {
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

  it("マイナスもある範囲だけ「+」。0 は「0.0」（−0.0 にしない）", () => {
    expect(valueText("eq4", spec("eq4", "lowGain"), 3)).toBe("+3.0 dB");
    expect(valueText("eq4", spec("eq4", "lowGain"), -0.01)).toBe("0.0 dB");
    expect(valueText("compressor", spec("compressor", "makeup"), 3)).toBe("3.0 dB");
  });

  it("段階は段の名前、スイッチはオン ・ オフ", () => {
    expect(valueText("delay", spec("delay", "intervalStep"), 2)).toBe("1/8");
    expect(valueText("delay", spec("delay", "intervalStep"), 1)).toBe("1/8 付点");
    expect(valueText("delay", spec("delay", "sync"), 1)).toBe("オン");
    expect(valueText("delay", spec("delay", "pingPong"), 0)).toBe("オフ");
  });

  it("棒: 高さ ・ 時間で広い物は対数、ほかは直線。棒 → 値は見せるけたに丸める", () => {
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

  it("← → は棒の 1/20、Shift は見せるけたの 1 つ。段階は 1 段、はしで止まる", () => {
    const mix = spec("reverb", "mix");
    expect(keyStep(mix, 50, 1, false)).toBe(55);
    expect(keyStep(mix, 50, -1, true)).toBe(49);
    expect(keyStep(mix, 100, 1, false)).toBe(100);
    expect(keyStep(spec("reverb", "decay"), 2.4, 1, true)).toBe(2.41);
    expect(keyStep(spec("delay", "intervalStep"), 4, 1, false)).toBe(4);
    expect(keyStep(spec("delay", "intervalStep"), 2, -1, false)).toBe(1);
  });
});

describe("行の並び", () => {
  it("リバーブは 7 行、見出しなし", () => {
    const groups = allRows("reverb", []);
    expect(groups.map((g) => g.group)).toEqual([null]);
    expect(groups[0]!.rows.map((r) => r.name)).toEqual(["響きの長さ", "部屋の大きさ", "プリディレイ", "高い音を落とす", "低い音を落とす", "広がり", "ミックス"]);
  });

  it("ディレイの「間隔」は 1 行。テンポに合わせるがオンなら段階、オフなら ms（A-14）", () => {
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

  it("EQ は 4 つの見出しの下に", () => {
    const groups = allRows("eq4", []);
    expect(groups.map((g) => [g.name, g.rows.map((r) => r.name)])).toEqual([
      ["低い音", ["高さ", "上げ下げ"]],
      ["中の低い音", ["高さ", "上げ下げ", "幅"]],
      ["中の高い音", ["高さ", "上げ下げ", "幅"]],
      ["高い音", ["高さ", "上げ下げ"]],
    ]);
  });

  it("さがす枠: 名前にふくまれる行だけ。残らない見出しは出さない。↑ ↓ ははしで止まる", () => {
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
/** 棒の箱を 0〜100 px にする（jsdom は大きさを持たない）。 */
function sizeBar(el: HTMLElement): void {
  el.getBoundingClientRect = () => ({ left: 0, right: 100, width: 100, top: 0, bottom: 12, height: 12, x: 0, y: 0, toJSON: () => ({}) });
}

describe("パラメータの欄（モック F4-Mixer）", () => {
  it("名前、最初から入っている、居場所、さがす枠と数、行と値。えらんだ行の印", () => {
    show(RightRail, scene("mixer"));
    expect(text(q("[data-test=params-name]"))).toBe("リバーブ");
    expect(text(q("[data-test=params] .who"))).toBe("リバーブ 最初から入っている A リバーブ ・ インサート 1");
    expect((q("[data-test=params-search]") as HTMLInputElement).placeholder).toBe("名前");
    expect(text(q("[data-test=params-count]"))).toBe("7 / 7");
    expect(qa("[data-test=param-value]").map(text)).toEqual(["2.40 s", "60 %", "20 ms", "6000 Hz", "120 Hz", "100 %", "100 %"]);
    expect(qa("[data-test=param-row]").map((r) => r.classList.contains("selected"))).toEqual([true, false, false, false, false, false, false]);
    // 「この子は窓を持ちません。」の行は出さない（仕様書 3 章 Y8）。
    expect(target.textContent).not.toContain("窓を持ちません");
    // オートメーションはまだないので「使えない」。
    expect(qa("[data-test=param-row] button[aria-label=オートメーションに書く]").every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
  });

  it("行を押すと SelectParam（えらんでいる行では送らない）", () => {
    show(RightRail, scene("mixer"));
    const rows = qa("[data-test=param-row]");
    pointer(rows[2]!, "pointerdown", 5);
    pointer(rows[0]!, "pointerdown", 5);
    expect(app.sent).toEqual([{ type: "selectParam", param: 2 }]);
  });

  it("ドラッグ: 画面が先に動き、1 コマに 1 回 commit: false、離して commit: true が 1 つ。もとの位置は gray.7 の枠", () => {
    show(RightRail, scene("mixer"));
    const slider = qa("[data-test=param-slider]")[1]!; // 部屋の大きさ 60 %
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
    // Rust の答えが来るまで、離した値のまま。
    expect(text(qa("[data-test=param-value]")[1]!)).toBe("50 %");
  });

  it("ドラッグ中の Esc は、もとの値に戻して CancelMixPreview。commit は送らない", () => {
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

  it("押して動かさずに離したら、何も送らない（値が同じ）", () => {
    show(RightRail, scene("mixer"));
    const slider = qa("[data-test=param-slider]")[0]!; // 響きの長さ
    app.sent.length = 0;
    // 大きさのない棒では、押した所はいまの値のまま。
    pointer(slider, "pointerdown", 30);
    pointer(slider, "pointerup", 30);
    expect(app.sent.filter((c) => c.type === "setParam")).toEqual([]);
  });

  it("← → で 1 手ずつ。↑ ↓ で行をえらぶ", () => {
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

  it("さがす枠: 数が変わり、合わなければ文と「さがす枠を空にする」", () => {
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

  it("何もえらんでいなければ見出しだけ", () => {
    show(RightRail, scene("params"));
    expect(text(q("[data-test=params]"))).toBe("▾ パラメータ U");
  });
});

describe("ディレイ（モック R6-Delay）", () => {
  it("間隔は段階（◂ 5 つの四角 ▸）、テンポに合わせるはオン、左右にゆれるはオフ", () => {
    show(RightRail, scene("paramsDelay"));
    expect(text(q("[data-test=params] .who"))).toBe("ディレイ 最初から入っている B ディレイ ・ インサート 1");
    expect(text(q("[data-test=params-count]"))).toBe("7 / 7");
    expect(qa("[data-test=param-value]").map(text)).toEqual(["1/8", "オン", "35 %", "5000 Hz", "200 Hz", "オフ", "100 %"]);
    expect(qa(".square").map((s) => s.classList.contains("on"))).toEqual([false, false, true, false, false]);
    expect(qa("[data-test=param-switch]").map((s) => s.getAttribute("aria-checked"))).toEqual(["true", "false"]);
    expect(qa("[data-test=param-row]").some((r) => r.classList.contains("selected"))).toBe(false);
  });

  it("段階の ◂ ▸ と四角、スイッチは押すと 1 手", () => {
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

  it("テンポに合わせるがオフなら、間隔は ms の棒", () => {
    const s = scene("paramsDelay");
    builtinOf(s.params)!.values[param("delay", "sync")] = 0;
    show(RightRail, s);
    expect(qa("[data-test=param-row]").length).toBe(7);
    expect(text(qa("[data-test=param-value]")[0]!)).toBe("250 ms");
    expect(qa("[data-test=param-row]")[0]!.querySelector("[data-test=param-slider]")).not.toBeNull();
  });
});

describe("縦の見出しと状態行", () => {
  it("パラメータがうしろにあるとき、縦の見出しにえらんだ子の名前と数（モック F-WindowOpen）", () => {
    const s = scene("mixer");
    s.workspace.layout.rightFront = "shelf";
    show(RightRail, s);
    expect(text(q("[data-test=params-tab-count]"))).toBe("リバーブ 7");
  });

  it("状態行: 「ループ 5–9 ・ A リバーブの「響きの長さ」をえらんでいます」（モック F4-Mixer）", () => {
    show(StatusLine, scene("mixer"));
    expect(text(q(".status"))).toContain("ループ 5–9 ・ A リバーブの「響きの長さ」をえらんでいます");
  });

  it("状態行: トラックのストリップと、ループのないとき", () => {
    const s = scene("mixer");
    s.status.message = { id: "paramSelected", loopBars: null, insert: { strip: { kind: "track", id: s.song.tracks[1]!.id }, slot: 0 }, effect: "compressor", param: 1 };
    show(StatusLine, s);
    expect(text(q(".status"))).toContain(`${s.song.tracks[1]!.number} ${s.song.tracks[1]!.name}の「おさえる強さ」をえらんでいます`);
    expect(text(q(".status"))).not.toContain("ループ");
  });
});
