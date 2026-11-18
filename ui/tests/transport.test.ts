// 再生まわりの帯、状態行の札と知らせ、入力計の録音中の鳴る先、アレンジの上の案内を mount して確かめる。
import { flushSync, mount, tick, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, StatusMessage, TransportReading } from "../src/bridge";
import InputMeter from "../src/components/InputMeter.svelte";
import SoundHint from "../src/components/SoundHint.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import Transport from "../src/components/Transport.svelte";
import { closeMenu, menuVisible } from "../src/lib/menu";
import { tipView } from "../src/lib/tip.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;

beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (menuVisible()) closeMenu();
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  document.body.innerHTML = "";
});

function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}

const q = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const text = (sel: string) => q(sel)?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const btn = (name: string) => q<HTMLButtonElement>(`[data-test="transport-${name}"]`)!;
const disabled = (name: string) => btn(name).getAttribute("aria-disabled") === "true";

const KEYBOARD = { id: "kb", name: "鍵盤 49", portName: "鍵盤 49 MIDI 1", address: "24:0" };
const PIANO = { number: 1, name: "ピアノ" };

/** 鍵盤と出口があり、ピアノが録音待機。 */
const ready = (edit: (s: AppState) => void = () => {}) =>
  app.stateWith((s) => {
    s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
    s.outputs.chosen = s.outputs.list[0]!.id;
    s.transport.canPlay = true;
    s.transport.canRecord = true;
    edit(s);
  });

const reading = (r: Partial<TransportReading>): TransportReading => ({
  tick: 0,
  ticksPerUs: 0,
  state: "stopped",
  countInBeatsLeft: 0,
  lap: 0,
  ...r,
});

const BAR = 1920;

describe("再生まわりのボタン", () => {
  it("音の出口がまだ: ▶ と ● は使えない見た目で、押しても何も送らない。⏮ ■ ループ メトロノームは使える", () => {
    show(Transport, app.stateWith(() => {}));
    expect(disabled("play")).toBe(true);
    expect(disabled("record")).toBe(true);
    for (const b of ["to-start", "stop", "loop", "metronome"]) expect(disabled(b), b).toBe(false);
    btn("play").click();
    btn("record").click();
    expect(app.sent).toEqual([]);
    btn("to-start").click();
    btn("loop").click();
    btn("metronome").click();
    expect(app.sent).toEqual([{ type: "goToStart" }, { type: "toggleLoop" }, { type: "toggleMetronome" }]);
  });

  it("止まっているとき: ■ が明るく、▶ で playStop、■ は何も送らない。● は rec の輪", () => {
    show(Transport, ready());
    expect(btn("stop").classList.contains("hi")).toBe(true);
    expect(btn("play").classList.contains("hi")).toBe(false);
    expect(btn("record").classList.contains("recording")).toBe(false);
    btn("stop").click();
    btn("play").click();
    btn("record").click();
    expect(app.sent).toEqual([{ type: "playStop" }, { type: "record" }]);
  });

  it("再生中: ▶ が明るく、■ で playStop、▶ は何も送らない", () => {
    show(Transport, ready((s) => (s.transport.state = "playing")));
    expect(btn("play").classList.contains("hi")).toBe(true);
    expect(btn("stop").classList.contains("hi")).toBe(false);
    btn("play").click();
    btn("stop").click();
    expect(app.sent).toEqual([{ type: "playStop" }]);
  });

  it("録音待機のトラックがない: ● は使えない見た目だが、押すと record を送る（Rust が理由を出す）", () => {
    show(Transport, ready((s) => (s.transport.canRecord = false)));
    expect(disabled("record")).toBe(true);
    btn("record").click();
    expect(app.sent).toEqual([{ type: "record" }]);
  });

  it("録音中とカウントイン中は ● が rec のぬり", () => {
    show(Transport, ready((s) => (s.transport.state = "recording")));
    expect(btn("record").classList.contains("recording")).toBe(true);
    expect(disabled("record")).toBe(false);
    app.setState(ready((s) => (s.transport.state = "countIn")));
    flushSync();
    expect(btn("record").classList.contains("recording")).toBe(true);
  });

  it("ループとメトロノームのオンは accent（on）", () => {
    show(
      Transport,
      ready((s) => {
        s.song.loopOn = true;
        s.transport.metronome = true;
      }),
    );
    expect(btn("loop").classList.contains("on")).toBe(true);
    expect(btn("metronome").classList.contains("on")).toBe(true);
    app.setState(ready());
    flushSync();
    expect(btn("loop").classList.contains("on")).toBe(false);
    expect(btn("metronome").classList.contains("on")).toBe(false);
  });
});

describe("位置と時間", () => {
  it("計器がなければ transport.position", () => {
    show(Transport, ready((s) => (s.transport.position = 4 * BAR)));
    expect(text('[data-test="transport-position"]')).toBe("5. 1. 000");
    expect(text('[data-test="transport-time"]')).toBe("00:00:10.000");
  });

  it("計器があれば、その位置（モック F-ArrangeRec の 7. 3. 240 ・ 00:00:16.562）", () => {
    show(Transport, ready((s) => (s.transport.state = "recording")));
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: 6 * BAR + 960 + 240, state: "recording" }) }));
    flushSync();
    expect(text('[data-test="transport-position"]')).toBe("7. 3. 240");
    expect(text('[data-test="transport-time"]')).toBe("00:00:16.562");
  });

  it("カウントイン中は、録りはじめる位置のまま", () => {
    show(
      Transport,
      ready((s) => {
        s.transport.state = "countIn";
        s.transport.position = 4 * BAR;
        s.transport.recording = { id: 1, track: 1, from: 4 * BAR, take: 1, clip: null, lap: 0 };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: 4 * BAR - 720, state: "countIn", countInBeatsLeft: 2 }) }));
    flushSync();
    expect(text('[data-test="transport-position"]')).toBe("5. 1. 000");
  });

  it("拍子とテンポで換算する（6/8、120.00）", () => {
    show(
      Transport,
      ready((s) => {
        s.song.meterNum = 6;
        s.song.meterDen = 8;
        s.song.tempoBpm = 120;
        s.transport.position = 1440 + 250;
      }),
    );
    expect(text('[data-test="transport-position"]')).toBe("2. 2. 020");
    // 1690 ティック = 7.0416… 拍（1 拍 240）、120 拍/分で 3.5208… 秒
    expect(text('[data-test="transport-time"]')).toBe("00:00:03.520");
  });
});

describe("カウントイン", () => {
  // jsdom には幅がないので、帯の幅を 1920 にする（カウントインが引っこまないように）。
  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1920);
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("止まっているときはボタン。小節数と「なし」", () => {
    show(Transport, ready());
    expect(text('[data-test="transport-count-in"]')).toBe("カウントイン1 小節");
    app.setState(ready((s) => (s.transport.countInBars = 0)));
    flushSync();
    expect(text('[data-test="transport-count-in"]')).toBe("カウントインなし");
  });

  it("押すと一覧がひらき、えらぶと setCountIn を送ってとじる。Esc の順の「メニュー」に入る", async () => {
    show(Transport, ready());
    btn("count-in").click();
    flushSync();
    await tick();
    const menu = q('[data-test="count-in-menu"]')!;
    expect(menu).not.toBeNull();
    expect(menuVisible()).toBe(true);
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    expect(rows.map((r) => r.textContent!.trim())).toEqual(["なし", "1 小節", "2 小節"]);
    expect(rows.map((r) => r.getAttribute("aria-checked"))).toEqual(["false", "true", "false"]);
    expect(menu.querySelector(".head")!.textContent!.trim()).toBe("カウントイン");
    rows[2]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setCountIn", bars: 2 }]);
    expect(q('[data-test="count-in-menu"]')).toBeNull();
    expect(menuVisible()).toBe(false);
  });

  it("一覧は ↑ ↓ Enter でえらべる。Esc（closeMenu）でとじる", async () => {
    show(Transport, ready());
    btn("count-in").click();
    flushSync();
    await new Promise((r) => setTimeout(r, 0));
    const menu = q('[data-test="count-in-menu"]')!;
    expect(document.activeElement?.textContent?.trim()).toBe("1 小節");
    menu.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }));
    expect(document.activeElement?.textContent?.trim()).toBe("なし");
    document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    flushSync();
    expect(app.sent).toEqual([{ type: "setCountIn", bars: 0 }]);

    btn("count-in").click();
    flushSync();
    expect(q('[data-test="count-in-menu"]')).not.toBeNull();
    closeMenu();
    flushSync();
    expect(q('[data-test="count-in-menu"]')).toBeNull();
  });

  it("カウントイン中は、オンのボタンに「カウントイン」と四角 ■■■□。「3 / 4」の字は出さない（仕様書 3 章、モック F-Connect4）", () => {
    show(
      Transport,
      ready((s) => {
        s.transport.state = "countIn";
        s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0 };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: -720, state: "countIn", countInBeatsLeft: 2 }) }));
    flushSync();
    const counting = q('[data-test="transport-counting"]')!;
    expect(text('[data-test="transport-counting"]')).toBe("カウントイン");
    expect(counting.classList.contains("m-btn")).toBe(true);
    expect(counting.classList.contains("on")).toBe(true);
    const boxes = [...counting.querySelectorAll(".box")];
    expect(boxes.map((b) => b.classList.contains("done"))).toEqual([true, true, true, false]);
    expect(q('[data-test="transport-count-in"]')).toBeNull();
  });

  it("拍子の上が 12 なら四角は 12", () => {
    show(
      Transport,
      ready((s) => {
        s.song.meterNum = 12;
        s.song.meterDen = 8;
        s.transport.state = "countIn";
        s.transport.countInBeats = 12;
        s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0 };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: -100, state: "countIn", countInBeatsLeft: 1 }) }));
    flushSync();
    const boxes = [...q('[data-test="transport-counting"]')!.querySelectorAll(".box")];
    expect(boxes.map((b) => b.classList.contains("done"))).toEqual(Array(12).fill(true));
    expect(boxes).toHaveLength(12);
  });

  it("2 小節のカウントインでも箱は 1 小節ぶん。小節ごとに数えなおす", () => {
    show(
      Transport,
      ready((s) => {
        s.transport.state = "countIn";
        s.transport.countInBars = 2;
        s.transport.countInBeats = 8;
        s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0 };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: -1440, state: "countIn", countInBeatsLeft: 3 }) }));
    flushSync();
    const boxes = [...q('[data-test="transport-counting"]')!.querySelectorAll(".box")];
    expect(boxes.map((b) => b.classList.contains("done"))).toEqual([true, true, false, false]);
  });
});

describe("横が足りないとき（仕様書 1 章 (1)、2 章「曲の ▾」「メトロノームの ▾」、モック F-Menus）", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  /** jsdom には幅がないので、部品ごとの幅と帯の幅をきめる。ボタンの右はしのキーは幅 0（ボタンの幅に入っている物として）。 */
  function widths(row: number, part: (name: string | undefined) => number): void {
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(row);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("key") || this.dataset.part?.startsWith("key-")) return 0;
      return part(this.dataset.part);
    });
  }
  const folded = () => widths(0, () => 10);

  it("曲の名前が引っこむと 28 の ▾。押すと曲の名前の一覧（印はから）", async () => {
    folded();
    show(Transport, ready());
    expect(q('[data-test="transport-song"]')).toBeNull();
    const stub = btn("song-stub");
    expect(stub.textContent).toBe("▾");
    expect(stub.getAttribute("aria-label")).toBe("曲（はじめての曲）");
    stub.click();
    flushSync();
    await tick();
    const menu = q('[data-test="song-menu"]')!;
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    expect(rows.map((r) => r.querySelector(".label")!.textContent)).toEqual(["新しい曲", "ひらく…", "保存"]);
    expect(rows.every((r) => r.querySelector(".choice:not(.selected)") !== null)).toBe(true);
    expect(rows.map((r) => r.getAttribute("aria-checked"))).toEqual([null, null, null]);
    rows[2]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "saveSong" }]);
  });

  it("曲の ▾ に乗せると「曲 ・ （曲の名前）」", () => {
    vi.useFakeTimers();
    folded();
    show(Transport, ready());
    btn("song-stub").dispatchEvent(new PointerEvent("pointerenter"));
    vi.advanceTimersByTime(600);
    flushSync();
    expect(tipView.value?.body).toBe("曲 ・ はじめての曲");
    expect(tipView.value?.term).toBeNull();
  });

  it("カウントインが引っこむとメトロノームの ▾。メトロノームがオンなら ▾ もオン", () => {
    folded();
    show(Transport, ready((s) => (s.transport.metronome = true)));
    expect(q('[data-test="transport-count-in"]')).toBeNull();
    const stub = btn("metronome-stub");
    expect(stub.getAttribute("aria-label")).toBe("メトロノームとカウントイン");
    expect(stub.classList.contains("on")).toBe(true);
    expect(stub.previousElementSibling).toBe(btn("metronome"));
    app.setState(ready());
    flushSync();
    expect(btn("metronome-stub").classList.contains("on")).toBe(false);
  });

  it("メトロノームの ▾ の一覧: メトロノーム（C）、見出し「カウントイン」、なし ・ 1 小節 ・ 2 小節", async () => {
    folded();
    show(Transport, ready((s) => (s.transport.metronome = true)));
    btn("metronome-stub").click();
    flushSync();
    await tick();
    const menu = q('[data-test="metronome-menu"]')!;
    expect(menu.getAttribute("aria-label")).toBe("メトロノームとカウントイン");
    expect([...menu.children].map((c) => c.textContent!.replace(/\s+/g, " ").trim())).toEqual([
      "メトロノーム C",
      "カウントイン",
      "なし",
      "1 小節",
      "2 小節",
    ]);
    expect(menu.children[1]!.classList.contains("head")).toBe(true);
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    expect(rows.map((r) => r.getAttribute("aria-checked"))).toEqual(["true", "false", "true", "false"]);
    rows[3]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setCountIn", bars: 2 }]);
    expect(q('[data-test="metronome-menu"]')).toBeNull();
    btn("metronome-stub").click();
    flushSync();
    await tick();
    q('[data-test="metronome-menu"]')!.querySelector<HTMLButtonElement>("button")!.click();
    flushSync();
    expect(app.sent.at(-1)).toEqual({ type: "toggleMetronome" });
  });

  it("一覧が窓からはみ出すときは、窓の中に入るまで左と上へずらす", async () => {
    folded();
    vi.spyOn(window, "innerWidth", "get").mockReturnValue(1280);
    vi.spyOn(window, "innerHeight", "get").mockReturnValue(800);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      // 一覧はボタンの下、右はしは窓の外（1200 + 220）、下も窓の外（700 + 150）。
      const rect = this.classList.contains("menu") ? { left: 1200, top: 700, width: 220, height: 150 } : { left: 0, top: 0, width: 0, height: 0 };
      return { ...rect, right: rect.left + rect.width, bottom: rect.top + rect.height, x: rect.left, y: rect.top, toJSON: () => ({}) } as DOMRect;
    });
    show(Transport, ready());
    btn("metronome-stub").click();
    flushSync();
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
    expect(q<HTMLElement>('[data-test="metronome-menu"]')!.style.transform).toBe("translate(-140px, -50px)");
  });

  it("窓に入る一覧はずらさない", async () => {
    folded();
    vi.spyOn(window, "innerWidth", "get").mockReturnValue(1280);
    vi.spyOn(window, "innerHeight", "get").mockReturnValue(800);
    show(Transport, ready());
    btn("song-stub").click();
    flushSync();
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
    expect(q<HTMLElement>('[data-test="song-menu"]')!.style.transform).toBe("");
  });

  it("カウントインが始まっても、引っこむ物は変わらない（カウントインの幅は、ふだんとカウントイン中の大きいほう）", () => {
    const shown = () =>
      ["transport-song", "transport-song-stub", "transport-metronome-stub"].map((t) => q(`[data-test="${t}"]`) !== null).join();
    const seen = new Set<string>();
    for (let room = 300; room <= 900; room += 10) {
      vi.restoreAllMocks();
      widths(room, (part) => (part === "countInCounting" ? 140 : part === "countIn" ? 100 : 40));
      show(Transport, ready());
      const before = shown();
      seen.add(before);
      app.setState(
        ready((s) => {
          s.transport.state = "countIn";
          s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0 };
        }),
      );
      flushSync();
      expect(shown(), String(room)).toBe(before);
      unmount(mounted!);
      mounted = null;
    }
    expect([...seen].sort()).toEqual(["false,true,false", "false,true,true", "true,false,false"]);
  });
});

describe("テンポと拍子の打ちこみ", () => {
  const field = (name: "tempo" | "meter") => q<HTMLInputElement>(`input[data-test="transport-${name}"]`);
  const press = (el: HTMLElement, key: string, more: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...more });
    el.dispatchEvent(ev);
    flushSync();
    return ev;
  };
  const type = (el: HTMLInputElement, value: string) => {
    el.value = value;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
  };
  const open = async (name: "tempo" | "meter") => {
    q<HTMLButtonElement>(`button[data-test="transport-${name}"]`)!.click();
    flushSync();
    await tick();
    return field(name)!;
  };

  it("押すと入力の枠になり、いまの値が入ってえらばれている。Enter で setTempo", async () => {
    show(Transport, ready());
    expect(field("tempo")).toBeNull();
    const input = await open("tempo");
    expect(input.value).toBe("96.00");
    expect(document.activeElement).toBe(input);
    type(input, "120.5");
    expect(press(input, "Enter").defaultPrevented).toBe(true);
    expect(app.sent).toEqual([{ type: "setTempo", bpm: 120.5 }]);
    await tick();
    expect(field("tempo")).toBeNull();
  });

  it("Esc でやめる（何も送らない）", async () => {
    show(Transport, ready());
    const input = await open("tempo");
    type(input, "120");
    press(input, "Escape");
    await tick();
    expect(field("tempo")).toBeNull();
    expect(app.sent).toEqual([]);
  });

  it("数字として読めない値は「こわれた」の枠で、Enter でも決めない。状態行で言うように頼む（もとの値）", async () => {
    show(Transport, ready());
    const input = await open("tempo");
    for (const bad of ["abc", "", "1e2", "12a"]) {
      type(input, bad);
      expect(input.classList.contains("broken"), bad).toBe(true);
      expect(input.getAttribute("aria-invalid")).toBe("true");
      press(input, "Enter");
    }
    expect(app.sent).toEqual(Array(4).fill({ type: "rejectValue", field: "tempo", kept: "96.00", range: null }));
    expect(field("tempo")).not.toBeNull();
    type(input, "29.99");
    expect(input.classList.contains("broken")).toBe(false);
  });

  it("範囲の外の数は近いはしにして決め、そのあと clampValue（B-68）。小数は第 2 位までに四捨五入", async () => {
    show(Transport, ready());
    for (const [typed, bpm, said] of [
      ["29.99", 30, "30.00"],
      ["999", 300, "300.00"],
      ["-5", 30, "30.00"],
      ["120.555", 120.56, null],
    ] as const) {
      app.sent.length = 0;
      const input = await open("tempo");
      type(input, typed);
      press(input, "Enter");
      expect(app.sent, typed).toEqual([
        { type: "setTempo", bpm },
        ...(said ? [{ type: "clampValue", field: "tempo", value: said, range: null }] : []),
      ]);
      await tick();
    }
  });

  it("↑ ↓ で 1 ずつ、Shift で 10。範囲の外へは出ない", async () => {
    show(Transport, ready());
    const input = await open("tempo");
    press(input, "ArrowUp");
    expect(input.value).toBe("97.00");
    press(input, "ArrowDown", { shiftKey: true });
    expect(input.value).toBe("87.00");
    type(input, "295.5");
    press(input, "ArrowUp", { shiftKey: true });
    expect(input.value).toBe("300.00");
    expect(app.sent).toEqual([]);
  });

  it("拍子: Enter で setMeter、↑ ↓ は上の数、読めない値は決めない", async () => {
    show(Transport, ready());
    const input = await open("meter");
    expect(input.value).toBe("4/4");
    press(input, "ArrowUp");
    expect(input.value).toBe("5/4");
    type(input, "6/あ");
    expect(input.classList.contains("broken")).toBe(true);
    press(input, "Enter");
    type(input, "4");
    press(input, "Enter");
    expect(app.sent).toEqual([
      { type: "rejectValue", field: "meterDen", kept: "4", range: null },
      { type: "rejectValue", field: "meterNum", kept: "4", range: null },
    ]);
    type(input, "6/8");
    press(input, "Enter");
    expect(app.sent.at(-1)).toEqual({ type: "setMeter", num: 6, den: 8 });
  });

  it("拍子の範囲の外: 上は 1〜12、下はいちばん近い 2 ・ 4 ・ 8 ・ 16（B-68）", async () => {
    show(Transport, ready());
    for (const [typed, num, den, said] of [
      ["13/4", 12, 4, { field: "meterNum", value: "12" }],
      ["0/4", 1, 4, { field: "meterNum", value: "1" }],
      ["6/7", 6, 8, { field: "meterDen", value: "8" }],
      ["3/3", 3, 4, { field: "meterDen", value: "4" }],
      ["3/99", 3, 16, { field: "meterDen", value: "16" }],
      ["99/1", 12, 2, { field: "meterNum", value: "12" }],
    ] as const) {
      app.sent.length = 0;
      const input = await open("meter");
      type(input, typed);
      expect(input.classList.contains("broken")).toBe(false);
      press(input, "Enter");
      expect(app.sent, typed).toEqual([
        { type: "setMeter", num, den },
        { type: "clampValue", ...said, range: null },
      ]);
      await tick();
    }
  });

  it("位置「1. 1. 000」も入力の枠（B-61）。「5」「5.2」「5.2.240」で locate、↑ ↓ は使わない、Esc でやめる", async () => {
    show(Transport, ready());
    const openPos = async () => {
      q<HTMLButtonElement>(`button[data-test="transport-position"]`)!.click();
      flushSync();
      await tick();
      return q<HTMLInputElement>(`input[data-test="transport-position"]`)!;
    };
    let input = await openPos();
    expect(input.value).toBe("1. 1. 000");
    expect(press(input, "ArrowUp").defaultPrevented).toBe(false);
    expect(input.value).toBe("1. 1. 000");
    press(input, "Escape");
    await tick();
    expect(app.sent).toEqual([]);
    for (const [typed, tick_] of [
      ["5", 4 * BAR],
      ["5.2", 4 * BAR + 480],
      ["5. 2. 240", 4 * BAR + 720],
      ["５．２．２４０", 4 * BAR + 720],
    ] as const) {
      input = await openPos();
      type(input, typed);
      press(input, "Enter");
      expect(app.sent.at(-1), typed).toEqual({ type: "locate", tick: tick_ });
      await tick();
    }
    input = await openPos();
    type(input, "5.x");
    expect(input.classList.contains("broken")).toBe(true);
    press(input, "Enter");
    expect(app.sent.at(-1)).toEqual({ type: "rejectValue", field: "playPosition", kept: "1. 1. 000", range: null });
  });

  it("位置の範囲の外: 小節 ・ 拍 ・ ティックをそれぞれ丸め、いちばん大きい単位のことだけ言う（B-68）", async () => {
    show(Transport, ready());
    for (const [typed, tick_, said] of [
      ["3.7", 2 * BAR + 3 * 480, { field: "beat", value: "3. 4. 000", range: "1〜4" }],
      ["3.2.999", 2 * BAR + 480 + 479, { field: "tick", value: "3. 2. 479", range: "0〜479" }],
      ["0.9.999", 3 * 480 + 479, { field: "bar", value: "1. 4. 479", range: null }],
      ["2.0", BAR, { field: "beat", value: "2. 1. 000", range: "1〜4" }],
    ] as const) {
      app.sent.length = 0;
      q<HTMLButtonElement>(`button[data-test="transport-position"]`)!.click();
      flushSync();
      await tick();
      const input = q<HTMLInputElement>(`input[data-test="transport-position"]`)!;
      type(input, typed);
      press(input, "Enter");
      expect(app.sent, typed).toEqual([{ type: "locate", tick: tick_ }, { type: "clampValue", ...said }]);
      await tick();
    }
  });

  it("枠の外へ出たら、やめる", async () => {
    show(Transport, ready());
    const input = await open("meter");
    type(input, "3/4");
    input.blur();
    flushSync();
    expect(field("meter")).toBeNull();
    expect(app.sent).toEqual([]);
  });
});

describe("状態行の札と知らせ", () => {
  const badge = () => q('[data-test="status-badge"]')!;
  const left = () => text("footer.status")!;

  it.each([
    ["stopped", "■ 停止中", false],
    ["playing", "▶ 再生中", false],
    ["recording", "● 録音中", true],
    ["countIn", "● カウントイン中", true],
  ] as const)("%s: 「%s」", (state, label, rec) => {
    show(StatusLine, ready((s) => (s.transport.state = state)));
    expect(badge().textContent).toBe(label);
    expect(badge().classList.contains("rec")).toBe(rec);
  });

  const msg = (m: StatusMessage, edit: (s: AppState) => void = () => {}) =>
    ready((s) => {
      s.status.message = m;
      edit(s);
    });

  it.each([
    [{ id: "recording", track: PIANO, take: 2 }, "1 ピアノ へ録音 ・ 止めると テイク 2 として残ります"],
    [
      { id: "loopRecording", track: PIANO, fromBar: 5, toBar: 9, lap: 2 },
      "1 ピアノ へ録音 ・ ループ 5–9 の 2 周目 ・ 周ごとにテイクを分けています",
    ],
    [
      { id: "takeSwitched", track: PIANO, clip: "Aメロ", take: 2, takes: 3 },
      "1 ピアノ ・ Aメロ をテイク 2 / 3 に切りかえました",
    ],
    [{ id: "noArmedTrack" }, "録るトラックが決まっていません。トラックの ● を押すと、そこに録ります。"],
    [{ id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null }, "ループ 5–9 ・ 選択なし"],
    [{ id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: false, notes: null }, "ループ 5–9"],
    [{ id: "summary", loopBars: null, nothingSelected: true, notes: null }, "選択なし"],
    [
      { id: "outputChosen", output: "アナログヘッドフォン", track: PIANO },
      "出口をアナログヘッドフォンにしました。1 ピアノ が録音待機になっています。",
    ],
    [{ id: "outputChosen", output: "アナログヘッドフォン", track: null }, "出口をアナログヘッドフォンにしました。"],
    [
      { id: "newSong", tempoBpm: 96, meterNum: 4, meterDen: 4, track: PIANO },
      "新しい曲 ・ テンポ 96.00 ・ 4/4 ・ 1 ピアノ が録音待機",
    ],
    [{ id: "newSong", tempoBpm: 120.5, meterNum: 6, meterDen: 8, track: null }, "新しい曲 ・ テンポ 120.50 ・ 6/8"],
  ] as [StatusMessage, string][])("%o", (m, expected) => {
    show(StatusLine, msg(m));
    expect(left()).toContain(`■ 停止中 ${expected}`);
  });

  it.each([
    [{ id: "bandFoldedThird", opened: "history", folded: "mixer" }, "取り消しの履歴をひらいたので、いちばん前にひらいたミキサーをたたみました。下の帯は、2 つまでひらけます。"],
    [{ id: "bandFoldedScale", scale: 150, band: "mixer" }, "拡大率を 150 % にしたので、ミキサーをたたみました。帯札を押すと、またひらきます。"],
    [{ id: "valueRejected", field: "velocity", kept: "77" }, "ベロシティは数字で打ちます。もとの 77 のままです。"],
    [{ id: "valueRejected", field: "meterDen", kept: "4" }, "拍子の下は数字で打ちます。もとの 4 のままです。"],
    [{ id: "valueRejected", field: "tempo", kept: "96.00" }, "テンポは数字で打ちます。もとの 96.00 のままです。"],
    [{ id: "outputMissing", output: "ヘッドホン" }, "ヘッドホンが見つからなくなりました。再生を止めています。 音の設定をひらく"],
  ] as [StatusMessage, string][])("新しい知らせ %o", (m, expected) => {
    show(StatusLine, msg(m));
    expect(left()).toContain(expected);
  });

  it.each([
    [[4], "ループ 5–9 ・ 選択なし ・ 4 パッド はミュート"],
    [[4, 5], "ループ 5–9 ・ 選択なし ・ 4 パッド ・ 5 ベース はミュート"],
  ])("ミュートしたトラックは、止まっているときの左の終わりに（モック Main）: %o", (muted, expected) => {
    show(
      StatusLine,
      msg({ id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null }, (s) => {
        s.song.tracks = [1, 2, 3, 4, 5].map((n) => ({
          ...s.song.tracks[0]!,
          number: n,
          name: ["ピアノ", "ドラム", "ギター", "パッド", "ベース"][n - 1]!,
          muted: muted.includes(n),
        }));
      }),
    );
    expect(left()).toContain(`■ 停止中 ${expected}`);
  });

  it("ミュートしたトラックは、ほかに言うことがなくても出す。動いているときと、ほかの知らせの間は出さない", () => {
    const mutePiano = (s: AppState) => (s.song.tracks[0]!.muted = true);
    show(StatusLine, msg({ id: "summary", loopBars: null, nothingSelected: false, notes: null }, mutePiano));
    expect(left()).toContain("■ 停止中 1 ピアノ はミュート");
    unmount(mounted!);
    mounted = null;
    show(StatusLine, msg({ id: "noArmedTrack" }, mutePiano));
    expect(left()).not.toContain("ミュート");
    unmount(mounted!);
    mounted = null;
    show(
      StatusLine,
      msg({ id: "summary", loopBars: null, nothingSelected: true, notes: null }, (s) => {
        mutePiano(s);
        s.transport.state = "playing";
      }),
    );
    expect(left()).not.toContain("ミュート");
  });

  it("summary で何もないときは、札だけ", () => {
    show(StatusLine, msg({ id: "summary", loopBars: null, nothingSelected: false, notes: null }));
    expect(text(".status .message")).toBeNull();
  });

  it("カウントイン中: 札と合わせて仕様書の文。残りの拍は計器から", () => {
    show(StatusLine, msg({ id: "countingIn", beats: 4 }, (s) => (s.transport.state = "countIn")));
    expect(left()).toContain("● カウントイン中 あと 4 拍で録音が始まります。止めるには Space");
    app.setTelemetry(app.telemetryWith({ position: reading({ state: "countIn", countInBeatsLeft: 2 }) }));
    flushSync();
    expect(left()).toContain("● カウントイン中 あと 2 拍で録音が始まります。止めるには Space");
  });
});

describe("入力計の右（弾くと鳴る先）", () => {
  it("止まっているときは「1 ピアノ ・ 仮の音」", () => {
    show(InputMeter, ready());
    expect(text('[data-test="meter-target"]')).toBe("1 ピアノ ・ 仮の音");
  });

  it("カウントイン中と録音中は「1 ピアノ ● テイク 1」。● から後ろが rec", () => {
    show(
      InputMeter,
      ready((s) => {
        s.transport.state = "countIn";
        s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0 };
      }),
    );
    expect(text('[data-test="meter-target"]')).toBe("1 ピアノ ● テイク 1");
    expect(q('[data-test="meter-target"] .rec')!.textContent).toBe("● テイク 1");
  });

  it("テイクの番号は recording.take そのまま（計器の周は足さない）", () => {
    show(
      InputMeter,
      ready((s) => {
        s.transport.state = "recording";
        s.transport.recording = { id: 1, track: 1, from: 0, take: 2, clip: 3, lap: 1 };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ state: "recording", lap: 1 }) }));
    flushSync();
    expect(text('[data-test="meter-target"]')).toBe("1 ピアノ ● テイク 2");
  });
});

describe("アレンジの上の案内", () => {
  it("音が出ました: 録音ボタンが使えるときだけ 2 文目とキー（モック F-Connect3）", () => {
    show(SoundHint, ready((s) => (s.workspace.hint = "soundCameOut")));
    expect(text('[data-test="sound-hint"]')).toBe(
      "音が出ました。 赤い丸の録音ボタンを押すと、カウントインのあとに録りはじめます。 録音 R ・ 再生と停止 Space",
    );
    app.setState(
      ready((s) => {
        s.workspace.hint = "soundCameOut";
        s.transport.canRecord = false;
      }),
    );
    flushSync();
    expect(text('[data-test="sound-hint"]')).toBe("音が出ました。");
  });

  it("からっぽのアレンジ: 2 文（モック F-Empty の 3 文目は出さない）", () => {
    show(SoundHint, ready((s) => (s.workspace.hint = "emptyArrange")));
    expect(text('[data-test="empty-hint"]')).toBe("ここに、録った音がならびます。 鍵盤を弾いて音を確かめたら、赤い丸の録音ボタンへ。");
    expect(q('[data-test="sound-hint"]')).toBeNull();
  });
});
