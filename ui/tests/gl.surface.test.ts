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
import { dbToLength, peakToDb } from "../src/gl/level";
import { everyGlFrame, glRegion, glSlot, type GlSlot, type MeterSource } from "../src/gl/slot";
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
  /** 写しを受けとった canvas（bitmaprenderer。WebGL の文脈ではない）。 */
  const copied: HTMLCanvasElement[] = [];
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(function (this: HTMLCanvasElement, id: string, options?: unknown) {
    if (id === "bitmaprenderer") return { transferFromImageBitmap: () => void copied.push(this) } as never;
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
  return { made, drawn, frame, copied };
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
    slot(region(box(new DOMRect(0, 0, 64, 4)), "inputMeter"), { kind: "velocity" });
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
    const keys = region(box(new DOMRect(10.4, 20, 349, 32)), "pianoRollKeys");
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
    slot(region(box(new DOMRect(0, 0, 300, 20)), "pianoRollKeys"), { kind: "keyboard", size: "full", firstNote: 36, lastNote: 84 });
    frame();
    for (let i = 0; i < 30; i++) {
      const dialog = box(new DOMRect(100, 100, 400, 300));
      const body = box(new DOMRect(100, 130, 400, 240), dialog);
      const meterBox = box(new DOMRect(110, 140, 120, 6), body);
      const keysBox = box(new DOMRect(110, 160, 200, 12), body);
      const r = glRegion(body, "settings");
      const a = slot(meterBox, { kind: "meter", source: "master", gap: 1 });
      const b = slot(keysBox, { kind: "keyboard", size: "slim", firstNote: 36, lastNote: 84 });
      frame();
      expect(dialog.querySelectorAll("canvas")).toHaveLength(1);
      a.destroy();
      b.destroy();
      r.destroy();
      dialog.remove();
      frame();
    }
    expect(made).toHaveLength(2);
    expect(contextsMade()).toBe(2);
    engine.destroy();
  });

  it("入力計: 鍵盤とベロシティの棒で 1 枚。間の文字の所は何も描かず、すける。鍵 1 つで出す面は 1 枚", () => {
    const { made, drawn, frame } = fakePage();
    const meter = region(box(new DOMRect(0, 0, 1280, 52)), "inputMeter");
    const keys = box(new DOMRect(212.5, 10, 349, 32), meter);
    box(new DOMRect(580, 10, 120, 32), meter).textContent = "押さえている鍵";
    const bar = box(new DOMRect(760, 30, 64, 4), meter);
    slot(keys, { kind: "keyboard", size: "full", firstNote: 36, lastNote: 84 });
    slot(bar, { kind: "velocity" });
    const engine = new Engine();
    frame();

    expect(made.map((m) => m.alpha)).toEqual([true]);
    const c = made[0]!.canvas;
    expect(c.parentElement).toBe(meter);
    // 鍵盤の左上から、棒の右はしまで。
    expect([c.width, c.height, c.style.top]).toEqual([611, 32, "10px"]);
    expect(parseFloat(c.style.left)).toBe(213);
    // 四角は、どれも 2 つの slot の中。間（x 349〜547）には 1 つもない。
    const inSlot = (r: number[]) => (r[2]! <= 349 && r[3]! <= 32) || (r[0]! >= 547 && r[1]! >= 20 && r[3]! <= 24);
    expect(drawn.at(-1)!.rects.length).toBeGreaterThan(50);
    expect(drawn.at(-1)!.rects.every(inSlot)).toBe(true);
    expect(drawn.at(-1)!.rects.filter((r) => r[0]! >= 547).map((r) => r.slice(0, 4))).toEqual([[547, 20, 611, 24]]);

    engine.onEvent({ type: "note", note: 60, on: true, velocity: 127, tUs: 1 });
    frame();
    expect(engine.stats).toEqual({ frames: 2, draws: 2 });
    const filled = drawn.at(-1)!.rects.filter((r) => r[0]! >= 547);
    expect(filled.map((r) => r.slice(0, 4))).toEqual([
      [547, 20, 611, 24],
      [547, 20, 611, 24],
    ]);
    expect(filled[1]!.slice(4)).toEqual(f32(rgb("color.accent")));
    engine.destroy();
  });

  it("ミキサー: 縦のメーターが何本でも 1 枚。戻り A ・ B の値が出る。横にスクロールしても 1 枚のままで、slot の位置だけ変わる", () => {
    const { made, drawn, frame } = fakePage();
    vi.spyOn(performance, "now").mockReturnValue(1000);
    // 横にスクロールする箱（見えているのは x 100〜500）。ストリップは 64 ごとに 12 本: トラック 9、戻り A ・ B、マスター。
    const strips = region(box(new DOMRect(100, 300, 400, 200)), "mixerMeters");
    const sources: MeterSource[] = [...Array.from({ length: 9 }, (_, i) => ({ track: i + 1 })), { ret: "a" }, { ret: "b" }, "master"];
    sources.forEach((source, i) => {
      const el = strips.appendChild(document.createElement("div"));
      el.getBoundingClientRect = () => new DOMRect(100 + 64 * i + 40 - strips.scrollLeft, 320, 9, 100);
      slot(el, { kind: "meter", source, gap: 1, vertical: true });
    });
    const engine = new Engine();
    engine.setTracks([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    engine.onTelemetry({
      seq: 0,
      sentUs: 0,
      keys: new Uint32Array(4),
      lastVelocity: 0,
      sustain: 0,
      modulation: 0,
      running: true,
      masterClipped: false,
      pitchBend: 0,
      loadPermille: 0,
      masterPeak: [1, 1],
      position: null,
      returnPeaks: new Float32Array([0.5, 0, 0, 1.5]),
      trackPeaks: new Float32Array(18),
    });
    frame();

    expect(made).toHaveLength(1);
    const c = made[0]!.canvas;
    expect(c.parentElement).toBe(strips);
    // 見えているのは最初の 6 本。面は 1 本めの左（140）から、箱の右はし（500）まで（右にまだ slot があるので）。
    expect([c.width, c.height, c.style.left, c.style.top]).toEqual([360, 100, "40px", "20px"]);
    const ground = f32(rgb("color.gray.0")).join();
    const ok = f32(rgb("color.meter.ok")).join();
    const of = (color: string) => drawn.at(-1)!.rects.filter((r) => r.slice(4).join() === color).map((r) => r.slice(0, 4));
    expect(of(ground)).toHaveLength(12);
    expect(of(ground).slice(0, 2)).toEqual([
      [0, 0, 4, 100],
      [5, 0, 9, 100],
    ]);
    expect(of(ok)).toEqual([]);

    // 右のはしまで送る（中身 768、見える幅 400）。
    strips.scrollLeft = 368;
    strips.dispatchEvent(new Event("scroll"));
    frame();
    expect(made).toHaveLength(1);
    expect(strips.querySelectorAll("canvas")).toHaveLength(1);
    // 見えているのは 6 本めの右の棒の残り（x 100〜101）から、マスター（485 まで）。面は中身といっしょに流れないよう、送った分だけ右へ置く。
    expect([c.width, c.height, c.style.left, c.style.top]).toEqual([385, 100, "368px", "20px"]);
    expect(of(ground)[0]).toEqual([0, 0, 1, 100]);
    expect(of(ground)).toHaveLength(13);
    // 戻り A（10 本め）: 左だけ 0.5。下から、ok の色で。
    const a = 64 * 9 + 40 - 368;
    const half = Math.round(100 * dbToLength(peakToDb(0.5)));
    expect(of(ok)).toEqual([
      [a, 100 - half, a + 4, 100],
      [a + 64 + 5, 30, a + 64 + 9, 100],
      [a + 128, 30, a + 132, 100],
      [a + 133, 30, a + 137, 100],
    ]);
    // 戻り B（11 本め）: 右が 0 dB をこえた。上はしに rec の 2px。
    expect(of(f32(rgb("color.rec")).join())).toEqual([[a + 64 + 5, 0, a + 64 + 9, 2]]);
    engine.destroy();
  });

  it("ミキサー、トラックが 10 本をこえたとき: WebGL の面はページの外に置き、左と右の 2 つの写しを 1 コマに 1 つずつ出す。文脈は 1 つのまま", async () => {
    const { made, drawn, frame, copied } = fakePage();
    vi.spyOn(performance, "now").mockReturnValue(1000);
    const asked: number[][] = [];
    vi.stubGlobal("createImageBitmap", (_c: HTMLCanvasElement, x: number, y: number, w: number, h: number) => {
      asked.push([x, y, w, h]);
      return Promise.resolve({ close() {} });
    });
    // ストリップは 64 ごとに 15 本（トラック 12、戻り A ・ B、マスター）。全部見えている。
    const strips = region(box(new DOMRect(0, 300, 1000, 200)), "mixerMeters");
    const sources: MeterSource[] = [...Array.from({ length: 12 }, (_, i) => ({ track: i + 1 })), { ret: "a" }, { ret: "b" }, "master"];
    sources.forEach((source, i) => {
      const el = strips.appendChild(document.createElement("div"));
      el.getBoundingClientRect = () => new DOMRect(64 * i + 40, 320, 9, 100);
      slot(el, { kind: "meter", source, gap: 1, vertical: true });
    });
    const engine = new Engine();
    const tracks = (n: number) => Array.from({ length: n }, (_, i) => i + 1);
    const levels = (peak: number) =>
      engine.onTelemetry({
        seq: 0,
        sentUs: 0,
        keys: new Uint32Array(4),
        lastVelocity: 0,
        sustain: 0,
        modulation: 0,
        running: true,
        masterClipped: false,
        pitchBend: 0,
        loadPermille: 0,
        masterPeak: [peak, peak],
        position: null,
        returnPeaks: new Float32Array(4),
        trackPeaks: new Float32Array(24).fill(peak),
      });
    engine.setTracks(tracks(12));
    frame();

    expect(made).toHaveLength(1);
    expect(contextsMade()).toBe(1);
    const gl = made[0]!.canvas;
    expect(gl.parentElement).toBeNull();
    const tiles = [...strips.querySelectorAll("canvas")];
    expect(tiles).toHaveLength(2);
    // 左は 1〜7 本め（40 から 393）、右は 8〜15 本め（488 から 945）。間のすきまは、どちらの面にも入らない。
    expect(tiles.map((c) => [c.style.left, c.style.top, c.style.width, c.style.height])).toEqual([
      ["40px", "20px", "393px", "100px"],
      ["488px", "20px", "457px", "100px"],
    ]);
    // WebGL の面は、大きいほうの写しの大きさ。はじめのコマは 2 つとも描く。
    expect([gl.width, gl.height]).toEqual([457, 100]);
    expect(drawn.map((d) => [d.width, d.height])).toEqual([
      [393, 100],
      [457, 100],
    ]);
    expect(asked).toEqual([
      [0, 0, 393, 100],
      [0, 0, 457, 100],
    ]);
    await Promise.resolve();
    expect(copied).toEqual(tiles);

    // 値が変わった: 1 コマに 1 つ。左、次のコマで右、そのあとは描かない。
    levels(0.5);
    frame();
    expect(drawn.slice(2).map((d) => d.width)).toEqual([393]);
    frame();
    expect(drawn.slice(2).map((d) => d.width)).toEqual([393, 457]);
    frame();
    frame();
    expect(drawn).toHaveLength(4);
    // 次も左から。
    levels(0.8);
    frame();
    frame();
    expect(drawn.slice(4).map((d) => d.width)).toEqual([393, 457]);

    // 送る ・ 大きさが変わる（すぐ描くコマ）は、2 つとも同じコマで。
    strips.dispatchEvent(new Event("scroll"));
    levels(0.95);
    frame();
    expect(drawn.slice(6).map((d) => d.width)).toEqual([393, 457]);

    // 10 本に戻ると、WebGL の面そのものを箱に戻す（60 Hz、1 枚）。
    engine.setTracks(tracks(10));
    frame();
    expect([...strips.querySelectorAll("canvas")]).toEqual([gl]);
    expect(made).toHaveLength(1);
    engine.destroy();
    expect(strips.querySelectorAll("canvas")).toHaveLength(0);
  });

  it("写しを作れない所（createImageBitmap がない）では、10 本をこえても WebGL の面 1 枚のまま、33 ms に 1 回描く", () => {
    const { made, drawn, frame } = fakePage();
    const now = vi.spyOn(performance, "now").mockReturnValue(1000);
    const strips = region(box(new DOMRect(0, 300, 1000, 200)), "mixerMeters");
    const el = strips.appendChild(document.createElement("div"));
    el.getBoundingClientRect = () => new DOMRect(40, 320, 9, 100);
    slot(el, { kind: "meter", source: { track: 1 }, gap: 1, vertical: true });
    const engine = new Engine();
    engine.setTracks(Array.from({ length: 12 }, (_, i) => i + 1));
    frame();
    expect([...strips.querySelectorAll("canvas")]).toEqual([made[0]!.canvas]);
    expect(drawn).toHaveLength(1);
    const levels = (peak: number) =>
      engine.onTelemetry({
        seq: 0,
        sentUs: 0,
        keys: new Uint32Array(4),
        lastVelocity: 0,
        sustain: 0,
        modulation: 0,
        running: true,
        masterClipped: false,
        pitchBend: 0,
        loadPermille: 0,
        masterPeak: [0, 0],
        position: null,
        returnPeaks: new Float32Array(4),
        trackPeaks: new Float32Array(24).fill(peak),
      });
    now.mockReturnValue(1016);
    levels(0.5);
    frame();
    expect(drawn).toHaveLength(1);
    now.mockReturnValue(1033);
    frame();
    expect(drawn).toHaveLength(2);
    engine.destroy();
  });

  it("上の帯: マスターの面は、数字の箱（cover）までおおう。ミキサーのメーターを出すコマは、変わっていなくてもマスターの面も出す", () => {
    const { made, frame } = fakePage();
    vi.spyOn(performance, "now").mockReturnValue(1000);
    const bar = region(box(new DOMRect(0, 0, 1000, 48)), "masterMeter");
    slot(box(new DOMRect(300, 10, 80, 30), bar), { kind: "cover" });
    slot(box(new DOMRect(820, 6, 100, 38), bar), { kind: "cover" });
    slot(box(new DOMRect(820, 32, 100, 12), bar), { kind: "meter", source: "master", gap: 1 });
    const engine = new Engine();
    engine.setTracks([1]);
    const telemetry = (track: number) => ({
      seq: 0,
      sentUs: 0,
      keys: new Uint32Array(4),
      lastVelocity: 0,
      sustain: 0,
      modulation: 0,
      running: true,
      masterClipped: false,
      pitchBend: 0,
      loadPermille: 0,
      masterPeak: [0.5, 0.5] as [number, number],
      position: null,
      returnPeaks: new Float32Array(4),
      trackPeaks: new Float32Array([track, track]),
    });
    engine.onTelemetry(telemetry(0.1));
    frame();
    const m = made[0]!;
    expect([m.canvas.width, m.canvas.height, m.canvas.style.left, m.canvas.style.top]).toEqual([620, 38, "300px", "6px"]);
    expect(m.calls.clear).toBe(1);

    // ミキサーがないとき: マスターが同じなら、出さない。
    engine.onTelemetry(telemetry(0.2));
    frame();
    expect(m.calls.clear).toBe(1);

    const strips = region(box(new DOMRect(0, 300, 1000, 200)), "mixerMeters");
    slot(box(new DOMRect(40, 320, 9, 100), strips), { kind: "meter", source: { track: 1 }, gap: 1, vertical: true });
    frame();
    const x = made[1]!;
    expect([m.calls.clear, x.calls.clear]).toEqual([2, 1]);
    // トラックだけ変わった: ミキサーの面が出るので、マスターの面も。
    engine.onTelemetry(telemetry(0.4));
    frame();
    expect([m.calls.clear, x.calls.clear]).toEqual([3, 2]);
    // 何も変わらない: どちらも出さない。
    engine.onTelemetry(telemetry(0.4));
    frame();
    expect([m.calls.clear, x.calls.clear]).toEqual([3, 2]);
    engine.destroy();
  });

  it("区画の数: 窓の高さと帯のどの組み合わせでも 10 枚まで。組み合わせを全部わたり歩いても、作る文脈は 10 個", () => {
    const { made, frame } = fakePage();
    const engine = new Engine();
    // いつもある物。入力計は、太くても細くても箱が 1 つ。
    const always: RegionName[] = ["inputMeter", "masterMeter"];
    // アレンジは、トラックの行のまま（窓が高い、または下の帯がない）か、ちぢんだ形（overview）。
    const arrange: Record<"rows" | "overview", RegionName[]> = {
      rows: ["trackMeters", "arrangeLanes", "recordingClip"],
      overview: ["overview"],
    };
    const pianoRoll: RegionName[] = ["pianoRoll", "pianoRollKeys", "pianoRollLive"];
    let most = 0;
    let mostAt = "";
    for (const form of ["rows", "overview"] as const) {
      for (const pr of [false, true]) {
        for (const mixer of [false, true]) {
          for (const settings of [false, true]) {
            const names = [...always, ...arrange[form], ...(pr ? pianoRoll : []), ...(mixer ? (["mixerMeters"] as const) : []), ...(settings ? (["settings"] as const) : [])];
            const open = names.map((name, i) => glRegion(box(new DOMRect(0, 10 * i, 100, 8)), name));
            frame();
            const count = document.querySelectorAll("canvas").length;
            expect(count).toBe(names.length);
            if (count > most) [most, mostAt] = [count, `${form} pianoRoll=${pr} mixer=${mixer} settings=${settings}`];
            for (const r of open) r.destroy();
            frame();
            expect(document.querySelectorAll("canvas")).toHaveLength(0);
          }
        }
      }
    }
    expect(most).toBe(10);
    expect(mostAt).toBe("rows pianoRoll=true mixer=true settings=true");
    expect(made).toHaveLength(10);
    expect(contextsMade()).toBe(10);
    expect(CONTEXT_BUDGET - most).toBe(2);
    engine.destroy();
  });

  it("区画の箱が来たときに文脈を作り、箱があるうちは slot がなくても持っている（鍵やドラッグの瞬間に作らない）", () => {
    const { made, drawn, frame } = fakePage();
    const engine = new Engine();
    const host = box(new DOMRect(0, 0, 400, 200));
    const r = glRegion(host, "pianoRollLive");
    frame();
    expect(made).toHaveLength(1);
    const canvas = made[0]!.canvas;
    expect(canvas.parentElement).toBe(host);
    expect([canvas.width, canvas.height]).toEqual([0, 0]);
    expect(drawn).toHaveLength(0);

    for (let i = 0; i < 3; i++) {
      const s = glSlot(box(new DOMRect(10, 20, 30, 8), host), { kind: "rects", rects: [{ x: 0, y: 0, w: 30, h: 8, color: "color.rec" }] });
      frame();
      expect([canvas.width, canvas.height]).toEqual([30, 8]);
      expect(drawn.at(-1)!.width).toBe(30);
      s.destroy();
      frame();
      expect(canvas.parentElement).toBe(host);
      expect([canvas.width, canvas.height]).toEqual([0, 0]);
    }
    expect(made).toHaveLength(1);
    expect(contextsMade()).toBe(1);

    // 箱が去ると池に戻り、次の区画が使う。
    r.destroy();
    host.remove();
    frame();
    expect(canvas.parentElement).toBe(null);
    const next = box(new DOMRect(0, 0, 100, 10));
    actions.push(glRegion(next, "recordingClip"));
    frame();
    expect(canvas.parentElement).toBe(next);
    expect(contextsMade()).toBe(1);
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
