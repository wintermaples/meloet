// 第 4 回 区切り3 の画面: ミキサーの帯（モック F4-Mixer）。ストリップの並び、フェーダー ・ パン ・ センドのドラッグ、インサートの枠、録音中、スクロール。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import LowerBand from "../src/components/LowerBand.svelte";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { dbToFrac, fracToDb, panText, SCALE_MARKS } from "../src/components/mixer/scale";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import { trackContextMenu } from "../src/components/trackMenu";
import { regionOf, watchSlots, type GlSlot } from "../src/gl/slot";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

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
  contextMenu.open = null;
  vi.useRealTimers();
});

function show(state: AppState, component: Component<any> = MixerStrips, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const strip = (key: string) => q(`[data-strip="${key}"]`)!;
const inStrip = (key: string, sel: string) => strip(key).querySelector<HTMLElement>(sel)!;
const allIn = (key: string, sel: string) => [...strip(key).querySelectorAll<HTMLElement>(sel)];

/** 部品の箱の大きさ（jsdom は 0 を返す）。 */
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
const setGains = () => app.sent.filter((c): c is Extract<Command, { type: "setGain" }> => c.type === "setGain");

const T1 = "track-11";

describe("目盛と数の見せかた", () => {
  it("フェーダーとメーターの目盛（仕様書 2 章）。いちばん下は −∞", () => {
    expect([6, 0, -3, -6, -12, -24, -60].map(dbToFrac)).toEqual([1, 0.8, 0.7, 0.6, 0.4, 0.2, 0]);
    expect(dbToFrac(null)).toBe(0);
    expect(dbToFrac(-61)).toBe(0);
    expect(dbToFrac(-15)).toBeCloseTo(0.35);
    expect([1, 0.8, 0.6, 0.35, 0.2].map(fracToDb)).toEqual([6, 0, -6, -15, -24]);
    expect(fracToDb(0)).toBeNull();
    expect(SCALE_MARKS.map((m) => m.label)).toEqual(["+6", "0", "−6", "−12", "−24", "−60"]);
  });

  it("パンは C ・ L 20 ・ R 20", () => {
    expect([0, -20, 20].map(panText)).toEqual(["C", "L 20", "R 20"]);
  });
});

describe("ストリップ（モック F4-Mixer）", () => {
  it("トラックの並び、戻り A ・ B、マスターの順。見出し、枠、インサート、センド、パン、値", () => {
    show(scene("mixer"));
    const keys = qa("[data-test=mixer-strip]").map((e) => e.dataset.strip);
    expect(keys).toEqual(["track-11", "track-12", "track-13", "track-14", "track-15", "return-a", "return-b", "master"]);
    expect(qa("[data-test=mixer-strip] .head").map(text)).toEqual([
      "1 ピアノ",
      "2 ベース",
      "3 ドラム",
      "4 パッド",
      "5 シンセ",
      "A リバーブ",
      "B ディレイ",
      "マスター",
    ]);
    expect(text(inStrip(T1, "[data-test=mixer-frame]"))).toBe("音源 ・ グランドピアノ");
    expect(text(inStrip("return-a", "[data-test=mixer-frame]"))).toBe("センド A の戻り");
    expect(text(inStrip("master", "[data-test=mixer-frame]"))).toMatch(/^出口 ・ /);
    expect(allIn("track-12", "[data-test=mixer-insert]").map(text)).toEqual(["コンプレッサー", "空き＋", "空き＋", "空き＋"]);
    expect(allIn("track-15", "[data-test=mixer-send]").map((e) => e.dataset.db)).toEqual(["-15", "-18"]);
    expect(allIn("track-12", "[data-test=mixer-send] + .value").map(text)).toEqual(["−∞", "−∞"]);
    expect(text(inStrip("track-14", "[data-test=mixer-pan-text]"))).toBe("L 20");
    expect(text(inStrip("track-14", "[data-test=mixer-gain]"))).toBe("−3.0");
    // 戻りとマスターにセンドはない。
    expect(allIn("return-a", "[data-test=mixer-send]")).toEqual([]);
    expect(allIn("master", "[data-test=mixer-send]")).toEqual([]);
  });

  it("えらんでいるインサート（params.selected）の枠だけ selected", () => {
    show(scene("mixer"));
    const selected = qa("[data-test=mixer-insert].selected");
    expect(selected).toHaveLength(1);
    expect(selected[0]!.closest<HTMLElement>("[data-test=mixer-strip]")!.dataset.strip).toBe("return-a");
    expect(selected[0]!.dataset.slot).toBe("0");
  });

  it("戻りは M だけ使える。S ・ ● とマスターの M ・ S ・ ● は「使えない」", () => {
    show(scene("mixer"));
    const usable = (key: string) => ["mute", "solo", "arm"].map((b) => inStrip(key, `[data-test=mixer-${b}]`).getAttribute("aria-disabled") === null);
    expect(usable(T1)).toEqual([true, true, true]);
    expect(usable("return-a")).toEqual([true, false, false]);
    expect(usable("return-b")).toEqual([true, false, false]);
    expect(usable("master")).toEqual([false, false, false]);
    for (const key of ["return-a", "master"]) for (const b of ["mute", "solo", "arm"]) inStrip(key, `[data-test=mixer-${b}]`).click();
    inStrip(T1, "[data-test=mixer-mute]").click();
    inStrip(T1, "[data-test=mixer-solo]").click();
    inStrip("track-12", "[data-test=mixer-arm]").click();
    expect(app.sent).toEqual([
      { type: "toggleReturnMute", bus: "a" },
      { type: "toggleMute", track: 11 },
      { type: "toggleSolo", track: 11 },
      { type: "selectTrack", track: 12, fromArm: true },
    ]);
  });

  it("音源の枠を押すと、トラックの見出しの音源の名前と同じく、棚が入れかえでひらく", () => {
    show(scene("mixer"));
    inStrip("track-13", "[data-test=mixer-frame]").click();
    expect(app.sent).toEqual([{ type: "openShelf", mode: { kind: "replace", track: 13 } }]);
  });

  it("メーターは縦の slot で、ぜんぶ mixerMeters の区画の中", () => {
    const seen: { node: HTMLElement; slot: GlSlot }[] = [];
    const stop = watchSlots({ attach: (node, slot) => seen.push({ node, slot }), update: () => {}, detach: () => {} });
    try {
      show(scene("mixer"));
      const meters = seen.filter((s) => s.node.isConnected && s.slot.kind === "meter");
      expect(meters.map((m) => (m.slot as Extract<GlSlot, { kind: "meter" }>).source)).toEqual([
        { track: 1 },
        { track: 2 },
        { track: 3 },
        { track: 4 },
        { track: 5 },
        { ret: "a" },
        { ret: "b" },
        "master",
      ]);
      expect(meters.every((m) => (m.slot as Extract<GlSlot, { kind: "meter" }>).vertical === true)).toBe(true);
      expect(meters.every((m) => regionOf(m.node)?.name === "mixerMeters")).toBe(true);
    } finally {
      stop();
    }
  });

  it("帯の道具の列: 音の流れの文とメーターの単位", () => {
    show(scene("mixer"), LowerBand, { band: "mixer", height: 516 });
    expect(text(q(".head"))).toContain("音は上から下へ流れます: 音源 → インサート → センド → パン → フェーダー → マスター");
    expect(text(q(".head"))).toContain("メーターの単位 dB");
    expect(q("[data-test=mixer-view]")!.dataset.glRegion).toBe("mixerMeters");
  });
});

describe("フェーダー ・ パン ・ センドのドラッグ（1 回で 1 手）", () => {
  it("動かす間は commit: false を 1 コマに 1 回まで（いちばん新しい値）、離したら commit: true を 1 つ。もとの位置は枠", () => {
    show(scene("mixer"));
    const fader = inStrip(T1, "[data-test=mixer-fader]");
    box(fader, 30, 100);
    pointer(fader, "pointerdown", 10, 100);
    pointer(fader, "pointermove", 10, 105);
    pointer(fader, "pointermove", 10, 110);
    pointer(fader, "pointermove", 10, 120);
    expect(fader.dataset.db).toBe("-6");
    expect(inStrip(T1, "[data-test=mixer-fader-from]")).not.toBeNull();
    expect(app.sent).toEqual([]);
    frame();
    expect(setGains()).toEqual([{ type: "setGain", strip: { kind: "track", id: 11 }, db: -6, commit: false }]);
    pointer(fader, "pointermove", 10, 80);
    pointer(fader, "pointerup", 10, 80);
    frame();
    expect(setGains()).toEqual([
      { type: "setGain", strip: { kind: "track", id: 11 }, db: -6, commit: false },
      { type: "setGain", strip: { kind: "track", id: 11 }, db: 6, commit: true },
    ]);
    expect(setGains().filter((c) => c.commit)).toHaveLength(1);
    expect(strip(T1).querySelector("[data-test=mixer-fader-from]")).toBeNull();
  });

  it("いちばん下まで下げると −∞（db: null）", () => {
    show(scene("mixer"));
    const fader = inStrip("return-b", "[data-test=mixer-fader]");
    box(fader, 30, 100);
    pointer(fader, "pointerdown", 10, 0);
    pointer(fader, "pointermove", 10, 500);
    expect(fader.dataset.db).toBe("-inf");
    expect(text(inStrip("return-b", "[data-test=mixer-gain]"))).toBe("−∞");
    pointer(fader, "pointerup", 10, 500);
    expect(app.sent).toEqual([{ type: "setGain", strip: { kind: "return", bus: "b" }, db: null, commit: true }]);
  });

  it("動かさずに離したら何も送らない", () => {
    show(scene("mixer"));
    const fader = inStrip(T1, "[data-test=mixer-fader]");
    pointer(fader, "pointerdown", 10, 50);
    pointer(fader, "pointerup", 10, 50);
    frame();
    expect(app.sent).toEqual([]);
  });

  it("Esc（キーの順のいちばん前）でやめると CancelMixPreview。画面はもとの値に戻り、離しても送らない", () => {
    show(scene("mixer"));
    const fader = inStrip(T1, "[data-test=mixer-fader]");
    box(fader, 30, 100);
    pointer(fader, "pointerdown", 10, 50);
    pointer(fader, "pointermove", 10, 70);
    frame();
    pointer(fader, "pointermove", 10, 90);
    expect(cancelDrag()).toBe(true);
    flushSync();
    frame();
    pointer(fader, "pointerup", 10, 90);
    expect(app.sent.map((c) => c.type)).toEqual(["setGain", "cancelMixPreview"]);
    expect(fader.dataset.db).toBe("0");
    expect(strip(T1).querySelector("[data-test=mixer-fader-from]")).toBeNull();
    expect(cancelDrag()).toBe(false);
  });

  it("パン: 右か上へ動かすと右へ。値は「R 20」。離したら setPan を 1 手", () => {
    show(scene("mixer"));
    const knob = inStrip(T1, "[data-test=mixer-pan]");
    pointer(knob, "pointerdown", 0, 0);
    pointer(knob, "pointermove", 10, -10);
    expect(text(inStrip(T1, "[data-test=mixer-pan-text]"))).toBe("R 20");
    expect(inStrip(T1, "[data-test=mixer-pan-from]")).not.toBeNull();
    pointer(knob, "pointermove", -200, 200);
    expect(knob.dataset.pan).toBe("-100");
    pointer(knob, "pointerup", -200, 200);
    expect(app.sent).toEqual([{ type: "setPan", strip: { kind: "track", id: 11 }, pan: -100, commit: true }]);
  });

  it("センド: 左右に動かす。いちばん左は −∞。track は id、bus は a ・ b", () => {
    show(scene("mixer"));
    const [a, b] = allIn("track-15", "[data-test=mixer-send]");
    box(a!, 100, 24);
    box(b!, 100, 24);
    pointer(a!, "pointerdown", 0, 0);
    pointer(a!, "pointermove", 5, 0);
    expect(a!.dataset.db).toBe("-12");
    expect(inStrip("track-15", "[data-test=mixer-send-from]")).not.toBeNull();
    frame();
    pointer(a!, "pointerup", 5, 0);
    pointer(b!, "pointerdown", 0, 0);
    pointer(b!, "pointermove", -100, 0);
    pointer(b!, "pointerup", -100, 0);
    expect(app.sent).toEqual([
      { type: "setSend", track: 15, bus: "a", db: -12, commit: false },
      { type: "setSend", track: 15, bus: "a", db: -12, commit: true },
      { type: "setSend", track: 15, bus: "b", db: null, commit: true },
    ]);
  });
});

describe("インサートの枠", () => {
  it("入っている枠を押すとえらぶ。えらんでいる枠をもう一度押しても送らない", () => {
    show(scene("mixer"));
    allIn("track-12", "[data-test=mixer-insert]")[0]!.click();
    allIn("return-a", "[data-test=mixer-insert]")[0]!.click();
    expect(app.sent).toEqual([{ type: "selectInsert", insert: { strip: { kind: "track", id: 12 }, slot: 0 } }]);
  });

  it("「空き ＋」を押すと、棚が insertEffect でひらく", () => {
    show(scene("mixer"));
    allIn("master", "[data-test=mixer-insert]")[2]!.click();
    allIn(T1, "[data-test=mixer-insert]")[0]!.click();
    expect(app.sent).toEqual([
      { type: "openShelf", mode: { kind: "insertEffect", strip: { kind: "master" }, slot: 2 } },
      { type: "openShelf", mode: { kind: "insertEffect", strip: { kind: "track", id: 11 }, slot: 0 } },
    ]);
  });

  it("右クリック: えらばずに一覧（入れかえる… ・ 上へ ・ 下へ ・ とばす ／ 外す。モック R9-Inserts）。行はその枠をさす。空きでは何も出さない", () => {
    show(scene("mixer"));
    const slot = allIn("track-14", "[data-test=mixer-insert]")[0]!;
    slot.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 5 }));
    const entries = contextMenu.open!.entries;
    expect(entries.map((e) => (e.kind === "row" ? [e.label, e.usable, e.key ?? null, e.checked ?? null] : "-"))).toEqual([
      ["insert.replace", true, null, null],
      ["insert.moveUp", false, null, null],
      ["insert.moveDown", true, null, null],
      ["params.bypass", true, "B", false],
      "-",
      ["mixer.removeEffect", true, "Delete", null],
    ]);
    // えらぶ意図は送らない（プラグインでは、えらぶと窓がひらく）。
    expect(app.sent).toEqual([]);
    const rows = entries.filter((e): e is MenuRow => e.kind === "row");
    for (const r of rows) if (r.usable) r.run();
    const insert = { strip: { kind: "track", id: 14 }, slot: 0 };
    expect(app.sent).toEqual([
      { type: "openShelf", mode: { kind: "replaceEffect", strip: insert.strip, slot: 0 } },
      { type: "moveInsert", insert, to: 1 },
      { type: "toggleBypass", insert },
      { type: "removeEffect", insert },
    ]);
    contextMenu.open = null;
    const empty = allIn("track-14", "[data-test=mixer-insert]")[1]!;
    const ev = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    empty.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
    expect(contextMenu.open).toBeNull();
  });

  it("えらんでいる枠は Delete で外す。ほかの「消す」には渡さない。えらんでいない枠では何もしない", () => {
    show(scene("mixer"));
    const onDoc = vi.fn();
    document.addEventListener("keydown", onDoc);
    try {
      const press = (el: HTMLElement) => {
        const ev = new KeyboardEvent("keydown", { key: "Delete", bubbles: true, cancelable: true });
        el.dispatchEvent(ev);
        return ev;
      };
      expect(press(allIn("return-a", "[data-test=mixer-insert]")[0]!).defaultPrevented).toBe(true);
      press(allIn("track-12", "[data-test=mixer-insert]")[0]!);
      expect(onDoc).not.toHaveBeenCalled();
      expect(app.sent).toEqual([{ type: "removeEffect", insert: { strip: { kind: "return", bus: "a" }, slot: 0 } }]);
    } finally {
      document.removeEventListener("keydown", onDoc);
    }
  });
});

describe("録音中 ・ カウントイン中（A-14）", () => {
  it("フェーダー ・ インサート ・ 空き ＋ ・ M は使える。音源の枠（入れかえ）だけ「使えない」", () => {
    for (const state of ["recording", "countIn"] as const) {
      const s = scene("mixer");
      s.transport.state = state;
      show(s);
      const fader = inStrip(T1, "[data-test=mixer-fader]");
      box(fader, 30, 100);
      pointer(fader, "pointerdown", 0, 50);
      pointer(fader, "pointermove", 0, 70);
      pointer(fader, "pointerup", 0, 70);
      allIn("track-12", "[data-test=mixer-insert]")[0]!.click();
      allIn("track-12", "[data-test=mixer-insert]")[1]!.click();
      inStrip(T1, "[data-test=mixer-mute]").click();
      inStrip("return-b", "[data-test=mixer-mute]").click();
      const frame = inStrip(T1, "[data-test=mixer-frame]");
      expect(frame.getAttribute("aria-disabled")).toBe("true");
      frame.click();
      expect(app.sent.map((c) => c.type)).toEqual(["setGain", "selectInsert", "openShelf", "toggleMute", "toggleReturnMute"]);
      unmount(mounted!);
      mounted = null;
      app.reset();
    }
  });

  it("トラックの右クリックの「ミュート」も使える", () => {
    const s = scene("mixer");
    s.transport.state = "recording";
    const track = s.song.tracks[0]!;
    const sent: Command[] = [];
    const rows = trackContextMenu(s, track, true, (c) => sent.push(c), (l) => l).filter((e): e is MenuRow => e.kind === "row");
    const mute = rows.find((r) => r.label === "menu.mute")!;
    expect(mute.usable).toBe(true);
    mute.run();
    expect(sent).toEqual([{ type: "toggleMute", track: track.id }]);
    expect(rows.find((r) => r.label === "menu.renameTrack")!.usable).toBe(false);
  });
});

describe("スクロール（仕様書 2 章: ストリップだけ縦に。はじめは下）", () => {
  it("はじめは下を見せ、高さが変わっても下からの位置を保つ", () => {
    const observed: (() => void)[] = [];
    const Real = globalThis.ResizeObserver;
    globalThis.ResizeObserver = class {
      constructor(private cb: () => void) {
        observed.push(cb);
      }
      observe(): void {}
      unobserve(): void {}
      disconnect(): void {}
    } as unknown as typeof ResizeObserver;
    try {
      show(scene("mixer"));
      const view = q("[data-test=mixer-view]")!;
      let clientHeight = 300;
      Object.defineProperty(view, "scrollHeight", { configurable: true, get: () => 484 });
      Object.defineProperty(view, "clientHeight", { configurable: true, get: () => clientHeight });
      const resize = () => {
        for (const cb of observed) cb();
        flushSync();
      };
      resize();
      expect(view.scrollTop).toBe(184);
      // 上へスクロールしてインサートを見る。
      view.scrollTop = 0;
      view.dispatchEvent(new Event("scroll"));
      clientHeight = 400;
      resize();
      expect(view.scrollTop).toBe(0);
      view.scrollTop = 40;
      view.dispatchEvent(new Event("scroll"));
      clientHeight = 420;
      resize();
      expect(view.scrollTop).toBe(20);
    } finally {
      globalThis.ResizeObserver = Real;
    }
  });
});
