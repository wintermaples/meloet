// 第 8 回 区切り2（画面）: 見出しの「帯」（36-B）、いちばん小さい窓の入力計（37-A）、マイクの計（38-B）、オートメーションの値の字（92 の残り）。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import InputMeter from "../src/components/InputMeter.svelte";
import { closeContextMenu, contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { colors } from "../src/design/tokens";
import { Batch } from "../src/gl/batch";
import { hexToRgb } from "../src/gl/color";
import { MIC_MID_DB } from "../src/gl/dims";
import { micDbToLength } from "../src/gl/level";
import { paintMeter } from "../src/gl/paint";
import { watchSlots, type GlSlot } from "../src/gl/slot";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const mounted: Record<string, unknown>[] = [];
let target: HTMLElement;
const slots = new Map<HTMLElement, GlSlot>();
let stopWatch: () => void = () => {};
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
  slots.clear();
  stopWatch = watchSlots({
    attach: (node, slot) => slots.set(node, slot),
    update: (node, slot) => slots.set(node, slot),
    detach: (node) => slots.delete(node),
  });
});
afterEach(() => {
  for (const m of mounted.splice(0)) unmount(m);
  target.remove();
  stopWatch();
  closeContextMenu();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState | null = null): void {
  if (state) app.setState(state);
  mounted.push(mount(component, { target }));
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const scene = (name: string) => fakeScene(name)!.state as FakeState;
const menuRows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");

describe("見出しの「帯」のボタン（仕様書 2 章、3 章 94-9 ・ 36-B、モック F8-FirstLane）", () => {
  it("どの見出しにも、1 段目の M の左に「帯」。押すと「＋ 帯を足す ▾」と同じ一覧。ひらいている間は accent と状態行の文", () => {
    const s = scene("tracks6");
    s.status.message = null;
    show(Arrange, s);
    show(StatusLine);
    const heads = qa('[data-test="track-head"]');
    for (const head of heads) {
      const button = head.querySelector<HTMLElement>('[data-test="track-lanes"]')!;
      expect(text(button)).toBe("帯");
      expect(button.nextElementSibling?.getAttribute("data-test")).toBe("track-mute");
    }
    const second = s.song.tracks[1]!;
    const button = heads[1]!.querySelector<HTMLElement>('[data-test="track-lanes"]')!;
    expect(button.classList.contains("on")).toBe(false);
    button.click();
    flushSync();
    expect(menuRows().map((r) => r.text ?? t(r.label))).toEqual(["音量", "パン", "ミュート", "センド A", "センド B", "エフェクトのパラメータ…"]);
    expect(button.classList.contains("on")).toBe(true);
    expect(heads[0]!.querySelector('[data-test="track-lanes"]')!.classList.contains("on")).toBe(false);
    expect(text(target)).toContain(
      t("status.laneAddOpen", { track: t("track.label", { number: second.number, name: second.name }) }),
    );
    menuRows()[0]!.run();
    expect(app.sent.at(-1)).toEqual({ type: "addLane", track: second.id, target: { kind: "volume" } });
    closeContextMenu();
    flushSync();
    expect(button.classList.contains("on")).toBe(false);
    expect(text(target)).not.toContain(t("status.laneAddOpen", { track: "" }).slice(1, 12));
  });

  it("録音中は使えない（押しても一覧は出ない）", () => {
    const s = scene("tracks6");
    s.transport.state = "recording";
    show(Arrange, s);
    const button = q('[data-test="track-lanes"]')!;
    expect(button.getAttribute("aria-disabled")).toBe("true");
    button.click();
    expect(contextMenu.open).toBeNull();
  });
});

describe("マイクの計（仕様書 3 章 94-17 ・ 38-B、「マイクの計の地と線」）", () => {
  const hex = (name: keyof typeof colors) => [...hexToRgb(colors[name])];
  const ofColor = (b: Batch, name: keyof typeof colors) =>
    b
      .rects()
      .filter((r) => r.slice(4).every((v, i) => Math.abs(v - hex(name)[i]!) < 1e-6))
      .map((r) => r.slice(0, 4));

  it("目盛: −60〜−12 が左半分、−12〜0 が右半分（どちらも dB でまっすぐ）。−∞ と −60 より下は 0、0 dB より上は 1", () => {
    expect(MIC_MID_DB).toBe(-12);
    expect([-Infinity, -70, -60, -36, -12, -6, 0, 3].map(micDbToLength)).toEqual([0, 0, 0, 0.25, 0.5, 0.75, 1, 1]);
  });

  it("地は gray.4。ok と hot の境はマスターと同じ −3 dB。白い縦の線は 1 秒のピークの所に 1 画素、2 本にまたがる", () => {
    const b = new Batch();
    b.clip(0, 0, 4000, 4000);
    const at = (db: number) => ({ length: micDbToLength(db), clipped: false });
    paintMeter(b, { x0: 0, y0: 0, x1: 200, y1: 10 }, 2, 2, at(-6), at(-1), false, { peak: micDbToLength(-12) });
    expect(ofColor(b, "color.gray.4")).toEqual([
      [0, 0, 200, 4],
      [0, 6, 200, 10],
    ]);
    expect(ofColor(b, "color.gray.0")).toEqual([]);
    // −3 dB は 0.875（175）。
    expect(ofColor(b, "color.meter.ok")).toEqual([
      [0, 0, 150, 4],
      [0, 6, 175, 10],
    ]);
    expect(ofColor(b, "color.meter.hot")).toEqual([[175, 6, 192, 10]]);
    expect(ofColor(b, "color.gray.10")).toEqual([[100, 0, 101, 10]]);
  });

  it("ピークがない（−∞）ときは線を描かない。0 dB の線は右はしの 1 画素", () => {
    const silent = { length: 0, clipped: false };
    const none = new Batch();
    none.clip(0, 0, 4000, 4000);
    paintMeter(none, { x0: 0, y0: 0, x1: 200, y1: 10 }, 2, 2, silent, silent, false, { peak: 0 });
    expect(none.rectCount).toBe(2);
    const top = new Batch();
    top.clip(0, 0, 4000, 4000);
    paintMeter(top, { x0: 0, y0: 0, x1: 200, y1: 10 }, 2, 2, silent, silent, false, { peak: 1 });
    expect(ofColor(top, "color.gray.10")).toEqual([[199, 0, 200, 10]]);
  });

  it("入力計のマイクの計の slot は、dB の字と同じ数（整数の 1 秒のピーク）を線に渡す。目盛の字は −60 ・ −12 ・ 0", () => {
    const s = scene("vocalTrack");
    show(InputMeter, s);
    const id = s.song.tracks.find((x) => x.kind.kind === "vocal")!.id;
    app.setTelemetry(app.telemetryWith({ mics: [{ track: id, peak: [0.025, 0.025], clipped: false, stereo: false }] }));
    flushSync();
    expect(text(q('[data-test="mic-db"]'))).toBe("−32 dB");
    expect(slots.get(q('[data-test="mic-bar"]')!)).toMatchObject({ kind: "meter", mic: { peakDb: -32 } });
    expect(qa('.meter.split:not(.ruler) > [data-test="meter-mic-half"] .mic-scale span').map((x) => x.textContent)).toEqual(["−60", "−12", "0"]);
  });
});

describe("いちばん小さい窓の、左右に分けた入力計（仕様書 3 章 94-16 ・ 37-A、モック R12-Meters ・ Opt8-NarrowMeter）", () => {
  // ruler（見えない写し）の幅と、半分の箱の幅をにせる。
  function withWidths(parts: Record<string, number>, boxes: { left: number; right: number }, run: () => void): void {
    const proto = HTMLElement.prototype;
    const offset = Object.getOwnPropertyDescriptor(proto, "offsetWidth");
    const client = Object.getOwnPropertyDescriptor(proto, "clientWidth");
    Object.defineProperty(proto, "offsetWidth", {
      configurable: true,
      get(this: HTMLElement) {
        return parts[this.dataset.part ?? ""] ?? 0;
      },
    });
    Object.defineProperty(proto, "clientWidth", {
      configurable: true,
      get(this: HTMLElement) {
        const part = this.dataset.part;
        return part === "left" || part === "right" ? boxes[part] : 0;
      },
    });
    try {
      run();
    } finally {
      if (offset) Object.defineProperty(proto, "offsetWidth", offset);
      else delete (proto as unknown as Record<string, unknown>).offsetWidth;
      if (client) Object.defineProperty(proto, "clientWidth", client);
      else delete (proto as unknown as Record<string, unknown>).clientWidth;
    }
  }
  const PARTS = { left: 500, right: 400, who: 100, keys: 200, target: 80, monitor: 60 };
  // 見えている半分（見えない写しの中の物は数えない）。
  const L = '.meter.split:not(.ruler) > [data-test="meter-keys-half"]';
  const R = '.meter.split:not(.ruler) > [data-test="meter-mic-half"]';
  const shown = () => ({
    target: q(`${L} [data-test="meter-right"]`) !== null,
    monitor: q(`${R} [data-test="mic-monitor"]`) !== null,
    keys: q(`${L} [data-gl-slot="keyboard"]`) !== null,
    held: q(`${L} [data-test="held"]`) !== null,
    fixed: q(`${L} [data-test="velocity-fixed"]`) !== null,
    name: q<HTMLElement>(`${L} .who`)!.style.maxWidth,
  });
  const cases: [string, { left: number; right: number }, ReturnType<typeof shown>][] = [
    ["入る: ぜんぶ出す", { left: 600, right: 600 }, { target: true, monitor: true, keys: true, held: true, fixed: true, name: "" }],
    ["左が入らない: 弾くと鳴る先", { left: 450, right: 600 }, { target: false, monitor: true, keys: true, held: true, fixed: true, name: "" }],
    ["右も入らない: 次にモニター", { left: 450, right: 380 }, { target: false, monitor: false, keys: true, held: true, fixed: true, name: "" }],
    [
      "まだ入らない（幅 853）: 鍵盤の絵、名前は 88",
      { left: 300, right: 300 },
      { target: false, monitor: false, keys: false, held: true, fixed: true, name: "var(--part-meter-name-split-narrow)" },
    ],
  ];
  for (const [name, boxes, want] of cases) {
    it(name, () => {
      withWidths(PARTS, boxes, () => {
        const s = scene("vocalTrack");
        s.keyboards.velocityFixed = { on: false, value: 100 };
        show(InputMeter, s);
        app.setTelemetry(app.telemetryWith({ keys: [60] }));
        flushSync();
        expect(shown()).toEqual(want);
      });
    });
  }
});
