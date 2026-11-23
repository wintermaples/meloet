// Screen: 「帯」 in the head (36-B), the input meter in the smallest window (37-A), the mic meter (38-B), automation value text (the rest of 92).
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
import { t } from "../src/text";
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

describe("the 「帯」 button in the head (spec 2, spec 3 (94-9, 36-B), mock F8-FirstLane)", () => {
  it("every head has 「帯」 left of M on the first line. Pressing gives the same list as 「＋ 帯を足す ▾」. While open, accent and the status line text", () => {
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
    expect(app.sent.at(-1)).toEqual({ type: "addLane", home: { kind: "track", track: second.id }, target: { kind: "volume" } });
    closeContextMenu();
    flushSync();
    expect(button.classList.contains("on")).toBe(false);
    expect(text(target)).not.toContain(t("status.laneAddOpen", { track: "" }).slice(1, 12));
  });

  it("disabled while recording (pressing shows no list)", () => {
    const s = scene("tracks6");
    s.transport.state = "recording";
    show(Arrange, s);
    const button = q('[data-test="track-lanes"]')!;
    expect(button.getAttribute("aria-disabled")).toBe("true");
    button.click();
    expect(contextMenu.open).toBeNull();
  });
});

describe("mic meter (spec 3 (94-17, 38-B), mic meter background and line)", () => {
  const hex = (name: keyof typeof colors) => [...hexToRgb(colors[name])];
  const ofColor = (b: Batch, name: keyof typeof colors) =>
    b
      .rects()
      .filter((r) => r.slice(4).every((v, i) => Math.abs(v - hex(name)[i]!) < 1e-6))
      .map((r) => r.slice(0, 4));

  it("scale: −60 to −12 is the left half, −12 to 0 the right half (both linear in dB). −∞ and below −60 are 0, above 0 dB is 1", () => {
    expect(MIC_MID_DB).toBe(-12);
    expect([-Infinity, -70, -60, -36, -12, -6, 0, 3].map(micDbToLength)).toEqual([0, 0, 0, 0.25, 0.5, 0.75, 1, 1]);
  });

  it("background meter.ground.mic. The ok/hot border is −3 dB, the same as the master. The white vertical line is 1 pixel at the 1-second peak, across 2 lines", () => {
    const b = new Batch();
    b.clip(0, 0, 4000, 4000);
    const at = (db: number) => ({ length: micDbToLength(db), clipped: false });
    paintMeter(b, { x0: 0, y0: 0, x1: 200, y1: 10 }, 2, 2, at(-6), at(-1), false, { peak: micDbToLength(-12) });
    expect(ofColor(b, "color.meter.ground.mic")).toEqual([
      [0, 0, 200, 4],
      [0, 6, 200, 10],
    ]);
    expect(ofColor(b, "color.meter.ground")).toEqual([]);
    // −3 dB is 0.875 (175).
    expect(ofColor(b, "color.meter.ok")).toEqual([
      [0, 0, 150, 4],
      [0, 6, 175, 10],
    ]);
    expect(ofColor(b, "color.meter.hot")).toEqual([[175, 6, 192, 10]]);
    expect(ofColor(b, "color.meter.peak")).toEqual([[100, 0, 101, 10]]);
  });

  it("no line when there is no peak (−∞). The 0 dB line is the 1 pixel at the right end", () => {
    const silent = { length: 0, clipped: false };
    const none = new Batch();
    none.clip(0, 0, 4000, 4000);
    paintMeter(none, { x0: 0, y0: 0, x1: 200, y1: 10 }, 2, 2, silent, silent, false, { peak: 0 });
    expect(none.rectCount).toBe(2);
    const top = new Batch();
    top.clip(0, 0, 4000, 4000);
    paintMeter(top, { x0: 0, y0: 0, x1: 200, y1: 10 }, 2, 2, silent, silent, false, { peak: 1 });
    expect(ofColor(top, "color.meter.peak")).toEqual([[199, 0, 200, 10]]);
  });

  it("the input meter's mic meter slot passes the same number as the dB text (the whole-number 1-second peak) to the line. Scale text is −60, −12, 0", () => {
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

describe("input meter split left and right in the smallest window (spec 3 (94-16, 37-A), mock R12-Meters, Opt8-NarrowMeter)", () => {
  // fakes the width of ruler (the hidden copy) and of the half box.
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
  // the visible half (things inside the hidden copy are not counted).
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
      "まだ入らない（幅 1024）: 鍵盤の絵、名前は 88",
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
