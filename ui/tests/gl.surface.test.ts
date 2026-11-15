// 面は、区画（glRegion の箱）ごとに 1 枚。文脈は使いまわし、数は予算の中。WebGL2 のない所（jsdom）でも、画面がこわれない。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Surface from "../src/gl/Surface.svelte";
import { CONTEXT_BUDGET, REGIONS } from "../src/gl/budget";
import type { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { Engine } from "../src/gl/engine";
import { contextsMade, resetPool } from "../src/gl/pool";
import { Renderer } from "../src/gl/renderer";
import { everyGlFrame, glRegion, glSlot, type GlSlot } from "../src/gl/slot";
import type { RegionName } from "../src/gl/budget";

class FakeResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

/** 呼ばれた事だけを数える、にせの WebGL2。 */
function fakeGl() {
  const calls = { drawArrays: 0, clear: 0 };
  const gl = new Proxy(
    {},
    {
      get: (_, name: string) => {
        if (name === "getShaderParameter" || name === "getProgramParameter") return () => true;
        if (name === "drawArrays" || name === "clear") return () => void (calls[name] += 1);
        return typeof name === "string" && /^[A-Z_0-9]+$/.test(name) ? 1 : () => ({});
      },
    },
  );
  return { gl, calls };
}

type Made = { canvas: HTMLCanvasElement; alpha: boolean; calls: { drawArrays: number; clear: number } };

/** にせの WebGL2 と、手で回す rAF。 */
function fakePage() {
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) => frames.push(f));
  vi.stubGlobal("cancelAnimationFrame", () => {});
  const made: Made[] = [];
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(function (this: HTMLCanvasElement, _id: string, options?: unknown) {
    const { gl, calls } = fakeGl();
    made.push({ canvas: this, alpha: (options as { alpha: boolean }).alpha, calls });
    return gl as never;
  });
  const drawn: { canvas: number; rects: number[][]; width: number; height: number }[] = [];
  const draw = Renderer.prototype.draw;
  vi.spyOn(Renderer.prototype, "draw").mockImplementation(function (this: Renderer, batch: Batch, width: number, height: number) {
    drawn.push({ canvas: drawn.length, rects: batch.rects(), width, height });
    draw.call(this, batch, width, height);
  });
  const frame = () => {
    for (const f of frames.splice(0)) f(0);
  };
  return { made, drawn, frame };
}

function box(rect: DOMRect, parent: HTMLElement = document.body): HTMLElement {
  const el = parent.appendChild(document.createElement("div"));
  el.getBoundingClientRect = () => rect;
  return el;
}

/** The batch keeps colors as 32-bit floats. */
const f32 = (c: readonly number[]) => c.map(Math.fround);

const actions: { destroy(): void }[] = [];
function region(node: HTMLElement, name: RegionName): HTMLElement {
  actions.push(glRegion(node, name));
  return node;
}
function slot(node: HTMLElement, s: GlSlot): { destroy(): void } {
  const a = glSlot(node, s);
  actions.push(a);
  return a;
}

describe("WebGL の面", () => {
  beforeEach(() => resetPool());
  afterEach(() => {
    for (const a of actions.splice(0)) a.destroy();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
  });

  it("WebGL2 が取れなければ、箱を空のままにして 1 回だけ知らせる", () => {
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) => frames.push(f));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    slot(region(box(new DOMRect(0, 0, 64, 4)), "inputVelocity"), { kind: "velocity" });
    slot(region(box(new DOMRect(0, 10, 64, 40)), "overview"), { kind: "overview" });
    const surface = mount(Surface, { target: document.body });
    flushSync();
    for (const f of frames.splice(0)) f(0);
    expect(document.querySelector("canvas")).toBe(null);
    expect(error).toHaveBeenCalledTimes(1);
    expect(String(error.mock.calls[0]![0])).toContain("WebGL2");
    unmount(surface);
  });

  it("区画ごとに、slot のある所の大きさの面を 1 枚。中を全部ぬる区画は不透明。変わった面だけを描く", () => {
    const { made, frame } = fakePage();
    // 画素の間から始まる箱: 面は、はしを画素に合わせる。
    const keys = region(box(new DOMRect(10.4, 20, 349, 32)), "inputKeys");
    const meterHost = region(box(new DOMRect(480, 10, 120, 30)), "masterMeter");
    const bar = box(new DOMRect(500, 20, 64, 4), meterHost);
    slot(keys, { kind: "keyboard", size: "full", firstNote: 36, lastNote: 84 });
    slot(bar, { kind: "meter", source: "master", gap: 1 });
    const engine = new Engine();
    frame();

    expect(made.map((m) => m.alpha)).toEqual([false, true]);
    const [k, m] = made as [Made, Made];
    expect(k.canvas.parentElement).toBe(keys);
    expect(k.canvas.getAttribute("aria-hidden")).toBe("true");
    expect([k.canvas.width, k.canvas.height]).toEqual([349, 32]);
    expect(parseFloat(k.canvas.style.left)).toBeCloseTo(-0.4);
    expect(k.canvas.style.width).toBe("349px");
    // メーターの面は、区画の箱ではなく、slot のある所だけ。
    expect(m.canvas.parentElement).toBe(meterHost);
    expect([m.canvas.width, m.canvas.height]).toEqual([64, 4]);
    expect([m.canvas.style.left, m.canvas.style.top]).toEqual(["20px", "10px"]);
    expect([k.calls.clear, m.calls.clear]).toEqual([1, 1]);

    // 鍵: 鍵盤の面だけ。同じ絵なら描かない。
    engine.onEvent({ type: "note", note: 60, on: true, velocity: 90, tUs: 1 });
    frame();
    expect([k.calls.clear, m.calls.clear]).toEqual([2, 1]);
    engine.invalidate();
    frame();
    expect([k.calls.clear, m.calls.clear]).toEqual([2, 1]);
    expect(engine.stats).toEqual({ frames: 3, draws: 3 });
    engine.destroy();
    expect(document.querySelector("canvas")).toBe(null);
  });

  it("トラックが 16 本でも、メーターの面は 1 枚。面は区画の箱の中で切る", () => {
    const { made, drawn, frame } = fakePage();
    const head = region(box(new DOMRect(0, 100, 200, 500)), "trackMeters");
    const engine = new Engine();
    engine.setTracks(Array.from({ length: 16 }, (_, i) => i + 1));
    for (let i = 0; i < 16; i++) {
      slot(box(new DOMRect(12, 150 + 64 * i, 180, 9), head), { kind: "meter", source: { track: i + 1 }, gap: 1 });
    }
    frame();
    expect(made).toHaveLength(1);
    expect(contextsMade()).toBe(1);
    const c = made[0]!.canvas;
    // 一番上のメーターから、箱の下のはし（600）まで。
    expect([c.width, c.height, c.style.top]).toEqual([180, 450, "50px"]);
    // 見えるメーターは 7 本（地が左右で 2 本ずつ）と、切れた 8 本め（598〜607 の、左の地の上だけ）。
    const grounds = drawn.at(-1)!.rects.filter((r) => r.slice(4).join() === f32(rgb("color.gray.0")).join());
    expect(grounds).toHaveLength(15);
    expect(grounds.at(-1)!.slice(0, 4)).toEqual([0, 448, 180, 450]);
    engine.destroy();
  });

  it("ひらいて閉じる区画は、文脈を作りなおさずに使いまわす", () => {
    const { made, frame } = fakePage();
    const engine = new Engine();
    slot(region(box(new DOMRect(0, 0, 300, 20)), "inputKeys"), { kind: "keyboard", size: "full", firstNote: 36, lastNote: 84 });
    frame();
    for (let i = 0; i < 30; i++) {
      const dialog = box(new DOMRect(100, 100, 400, 300));
      const a = slot(region(box(new DOMRect(110, 110, 120, 6), dialog), "settingsMeter"), { kind: "meter", source: "master", gap: 1 });
      const b = slot(region(box(new DOMRect(110, 130, 200, 12), dialog), "settingsKeys"), { kind: "keyboard", size: "slim", firstNote: 36, lastNote: 84 });
      frame();
      expect(dialog.querySelectorAll("canvas")).toHaveLength(2);
      a.destroy();
      b.destroy();
      dialog.remove();
      frame();
    }
    expect(made).toHaveLength(3);
    expect(contextsMade()).toBe(3);
    engine.destroy();
  });

  it("予算をこえて文脈を作ろうとしたら、開発とテストでは落とす", () => {
    const { frame } = fakePage();
    const engine = new Engine();
    for (let i = 0; i <= CONTEXT_BUDGET; i++) {
      slot(region(box(new DOMRect(0, 10 * i, 100, 8)), "masterMeter"), { kind: "meter", source: "master", gap: 1 });
    }
    expect(frame).toThrow(/budget of 12/);
    expect(contextsMade()).toBe(CONTEXT_BUDGET);
    engine.destroy();
  });

  it("区画の外の slot は、開発とテストでは落とす", () => {
    const { frame } = fakePage();
    const engine = new Engine();
    slot(box(new DOMRect(0, 0, 64, 4)), { kind: "velocity" });
    expect(frame).toThrow(/not inside any glRegion/);
    engine.destroy();
  });

  it("四角の並び: CSS px を画素にする。あとの slot が上", () => {
    const { drawn, frame } = fakePage();
    vi.stubGlobal("devicePixelRatio", 1.5);
    const lanes = region(box(new DOMRect(100, 100, 400, 200)), "arrangeLanes");
    const clip = box(new DOMRect(120, 110, 100, 40), lanes);
    const grid = { kind: "arrangeGrid", barPx: 1000, firstBarX: 0 } as const;
    slot(clip, {
      kind: "rects",
      rects: [
        { x: 0, y: 0, w: 100, h: 40, color: "color.track.1" },
        { x: 10.2, y: 3, w: 5, h: 2, color: "color.gray.0" },
      ],
    });
    // 先に印をつけても、ページの順で描く。
    slot(lanes, grid);
    const engine = new Engine();
    frame();
    const rects = drawn.at(-1)!.rects;
    // 小節の線（x = 0）が先、クリップがあと。クリップの箱は (180, 165) から。
    expect(rects.map((r) => r.slice(0, 4))).toEqual([
      [0, 0, 1, 300],
      [30, 15, 180, 75],
      [45, 20, 53, 23],
    ]);
    expect(rects[1]!.slice(4)).toEqual(f32(rgb("color.track.1")));
    engine.destroy();
  });

  it("everyGlFrame: コマの頭で呼ばれ、そこで変えた slot は同じコマで描く。やめるまでコマが続く", () => {
    const { drawn, frame } = fakePage();
    const engine = new Engine();
    const host = region(box(new DOMRect(0, 0, 100, 20)), "recordingClip");
    const a = glSlot(host, { kind: "rects", rects: [] });
    actions.push(a);
    frame();
    let calls = 0;
    const stop = everyGlFrame(() => {
      calls += 1;
      a.update({ kind: "rects", rects: [{ x: 0, y: 0, w: calls, h: 1, color: "color.rec" }] });
    });
    frame();
    expect(calls).toBe(1);
    expect(drawn.at(-1)!.rects[0]!.slice(0, 4)).toEqual([0, 0, 1, 1]);
    frame();
    expect(calls).toBe(2);
    expect(drawn.at(-1)!.rects[0]!.slice(0, 4)).toEqual([0, 0, 2, 1]);
    stop();
    frame();
    frame();
    expect(calls).toBe(2);
    engine.destroy();
  });

  it("区画の表: どの区画も名前がつき、予算は WebKitGTK の限りより小さい", () => {
    expect(Object.keys(REGIONS).length).toBeLessThanOrEqual(CONTEXT_BUDGET);
    expect(CONTEXT_BUDGET).toBeLessThan(16);
  });
});
