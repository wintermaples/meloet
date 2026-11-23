// One surface per section (glRegion box). Contexts are reused and their number stays in budget. Without WebGL2 (jsdom) the screen does not break.
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

/** A fake WebGL2 that only counts calls. */
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

/** A fake WebGL2 and a rAF turned by hand. */
function fakePage() {
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) => frames.push(f));
  vi.stubGlobal("cancelAnimationFrame", () => {});
  const made: Made[] = [];
  /** A canvas that receives a copy (bitmaprenderer, not a WebGL context). */
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

describe("WebGL surfaces", () => {
  beforeEach(() => resetPool());
  afterEach(() => {
    for (const a of actions.splice(0)) a.destroy();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
  });

  it("if WebGL2 cannot be got, leave the box empty and report once", () => {
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

  it("one surface per section, the size of where the slots are. A section that fills everything is opaque. Only changed surfaces are drawn", () => {
    const { made, frame } = fakePage();
    // A box starting between pixels: the surface aligns its edges to pixels.
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
    // The meter surface covers only where the slots are, not the section box.
    expect(m.canvas.parentElement).toBe(meterHost);
    expect([m.canvas.width, m.canvas.height]).toEqual([64, 4]);
    expect([m.canvas.style.left, m.canvas.style.top]).toEqual(["20px", "10px"]);
    expect([k.calls.clear, m.calls.clear]).toEqual([1, 1]);

    // Keys: only the keyboard surface. The same picture is not drawn.
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

  it("even with 16 tracks, one meter surface. The surface is cut inside the section box", () => {
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
    // From the top meter to the bottom of the box (600).
    expect([c.width, c.height, c.style.top]).toEqual([180, 450, "50px"]);
    // 7 meters are visible (the background has 2 each on left and right), and a cut 8th (598–607, only on the left background).
    const grounds = drawn.at(-1)!.rects.filter((r) => r.slice(4).join() === f32(rgb("color.meter.ground")).join());
    expect(grounds).toHaveLength(15);
    expect(grounds.at(-1)!.slice(0, 4)).toEqual([0, 448, 180, 450]);
    engine.destroy();
  });

  it("a section that opens and closes reuses its context without making a new one", () => {
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

  it("input meter: one surface for the keyboard and the velocity bar. The text area between draws nothing and is see-through. One key shows one surface", () => {
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
    // From the top left of the keyboard to the right end of the bar.
    expect([c.width, c.height, c.style.top]).toEqual([611, 32, "10px"]);
    expect(parseFloat(c.style.left)).toBe(213);
    // Every rectangle is inside the two slots. None in between (x 349–547).
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

  it("mixer: one surface for any number of vertical meters. Values of returns A and B show. Scrolling sideways keeps one surface; only slot positions change", () => {
    const { made, drawn, frame } = fakePage();
    vi.spyOn(performance, "now").mockReturnValue(1000);
    // A box that scrolls sideways (visible x 100–500). 12 strips every 64: 9 tracks, returns A and B, master.
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
      mics: [],
    });
    frame();

    expect(made).toHaveLength(1);
    const c = made[0]!.canvas;
    expect(c.parentElement).toBe(strips);
    // The first 6 are visible. The surface runs from the left of the 1st (140) to the right end of the box (500), since there are more slots to the right.
    expect([c.width, c.height, c.style.left, c.style.top]).toEqual([360, 100, "40px", "20px"]);
    const ground = f32(rgb("color.meter.ground")).join();
    const ok = f32(rgb("color.meter.ok")).join();
    const of = (color: string) => drawn.at(-1)!.rects.filter((r) => r.slice(4).join() === color).map((r) => r.slice(0, 4));
    expect(of(ground)).toHaveLength(12);
    expect(of(ground).slice(0, 2)).toEqual([
      [0, 0, 4, 100],
      [5, 0, 9, 100],
    ]);
    expect(of(ok)).toEqual([]);

    // Scroll to the right end (content 768, visible width 400).
    strips.scrollLeft = 368;
    strips.dispatchEvent(new Event("scroll"));
    frame();
    expect(made).toHaveLength(1);
    expect(strips.querySelectorAll("canvas")).toHaveLength(1);
    // Visible: from the rest of the 6th's right bar (x 100–101) to the master (up to 485). The surface is moved right by the scrolled amount so it does not flow with the content.
    expect([c.width, c.height, c.style.left, c.style.top]).toEqual([385, 100, "368px", "20px"]);
    expect(of(ground)[0]).toEqual([0, 0, 1, 100]);
    expect(of(ground)).toHaveLength(13);
    // Return A (10th): only the left at 0.5. From the bottom, in the ok color.
    const a = 64 * 9 + 40 - 368;
    const half = Math.round(100 * dbToLength(peakToDb(0.5)));
    expect(of(ok)).toEqual([
      [a, 100 - half, a + 4, 100],
      [a + 64 + 5, 30, a + 64 + 9, 100],
      [a + 128, 30, a + 132, 100],
      [a + 133, 30, a + 137, 100],
    ]);
    // Return B (11th): the right went over 0 dB. A 2px meter.clip at the top end.
    expect(of(f32(rgb("color.meter.clip")).join())).toEqual([[a + 64 + 5, 0, a + 64 + 9, 2]]);
    engine.destroy();
  });

  it("mixer, always: the WebGL surface sits outside the page, and the left and right copies are shown one per frame. Still one context", async () => {
    const { made, drawn, frame, copied } = fakePage();
    vi.spyOn(performance, "now").mockReturnValue(1000);
    const asked: number[][] = [];
    vi.stubGlobal("createImageBitmap", (_c: HTMLCanvasElement, x: number, y: number, w: number, h: number) => {
      asked.push([x, y, w, h]);
      return Promise.resolve({ close() {} });
    });
    // 15 strips every 64 (12 tracks, returns A and B, master). All visible.
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
        mics: [],
      });
    engine.setTracks(tracks(12));
    frame();

    expect(made).toHaveLength(1);
    expect(contextsMade()).toBe(1);
    const gl = made[0]!.canvas;
    expect(gl.parentElement).toBeNull();
    const tiles = [...strips.querySelectorAll("canvas")];
    expect(tiles).toHaveLength(2);
    // Left is the 1st–7th (40 to 393), right is the 8th–15th (488 to 945). The gap between goes in neither surface.
    expect(tiles.map((c) => [c.style.left, c.style.top, c.style.width, c.style.height])).toEqual([
      ["40px", "20px", "393px", "100px"],
      ["488px", "20px", "457px", "100px"],
    ]);
    // The WebGL surface is the size of the larger copy. The first frame draws both.
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

    // A value changed: one per frame. Left, then right on the next frame, then no draws.
    levels(0.5);
    frame();
    expect(drawn.slice(2).map((d) => d.width)).toEqual([393]);
    frame();
    expect(drawn.slice(2).map((d) => d.width)).toEqual([393, 457]);
    frame();
    frame();
    expect(drawn).toHaveLength(4);
    // The next also starts from the left.
    levels(0.8);
    frame();
    frame();
    expect(drawn.slice(4).map((d) => d.width)).toEqual([393, 457]);

    // Scrolling and resizing (frames that draw at once) do both in the same frame.
    strips.dispatchEvent(new Event("scroll"));
    levels(0.95);
    frame();
    expect(drawn.slice(6).map((d) => d.width)).toEqual([393, 457]);

    // Even with tracks down to 10, it stays split (always 30 Hz, user:2026-11-20).
    engine.setTracks(tracks(10));
    frame();
    expect([...strips.querySelectorAll("canvas")]).toEqual(tiles);
    expect(made).toHaveLength(1);
    engine.destroy();
    expect(strips.querySelectorAll("canvas")).toHaveLength(0);
  });

  it("where copies cannot be made (no createImageBitmap), one WebGL surface, drawn once per 33 ms", () => {
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
        mics: [],
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

  it("top band: the master surface covers up to the number box (cover). A frame that shows the mixer meters also shows the master surface, even unchanged", () => {
    const { made, frame } = fakePage();
    const now = vi.spyOn(performance, "now").mockReturnValue(1000);
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
      mics: [],
    });
    engine.onTelemetry(telemetry(0.1));
    frame();
    const m = made[0]!;
    expect([m.canvas.width, m.canvas.height, m.canvas.style.left, m.canvas.style.top]).toEqual([620, 38, "300px", "6px"]);
    expect(m.calls.clear).toBe(1);

    // Without the mixer: if the master is the same, it is not shown.
    engine.onTelemetry(telemetry(0.2));
    frame();
    expect(m.calls.clear).toBe(1);

    const strips = region(box(new DOMRect(0, 300, 1000, 200)), "mixerMeters");
    slot(box(new DOMRect(40, 320, 9, 100), strips), { kind: "meter", source: { track: 1 }, gap: 1, vertical: true });
    frame();
    const x = made[1]!;
    expect([m.calls.clear, x.calls.clear]).toEqual([2, 1]);
    // Only tracks changed: the mixer surface shows, so the master surface too (the mixer is 30 Hz, so 33 ms later).
    now.mockReturnValue(1040);
    engine.onTelemetry(telemetry(0.4));
    frame();
    expect([m.calls.clear, x.calls.clear]).toEqual([3, 2]);
    // Nothing changed: neither is shown.
    now.mockReturnValue(1080);
    engine.onTelemetry(telemetry(0.4));
    frame();
    expect([m.calls.clear, x.calls.clear]).toEqual([3, 2]);
    engine.destroy();
  });

  it("slots inside a box that scrolls its content (data-gl-clip) draw only what that box shows. The surface too goes only that far (the arrange track rows)", () => {
    const { made, drawn, frame } = fakePage();
    // Section box (y 100–400). The top 50 is a band that does not scroll; below it (y 150–400) is the scrolling box.
    const wrap = region(box(new DOMRect(0, 100, 300, 300)), "arrangeLanes");
    slot(box(new DOMRect(0, 100, 300, 50), wrap), { kind: "rects", rects: [{ x: 10, y: 10, w: 20, h: 20, color: "color.rec" }] });
    const view = box(new DOMRect(0, 150, 300, 250), wrap);
    view.setAttribute("data-gl-clip", "");
    // The content is 1000 tall, scrolled up by 120 (from y 30). Rectangles at content y 0, 100, 200, 900.
    const rows = box(new DOMRect(0, 30, 300, 1000), view);
    slot(rows, { kind: "rects", rects: [0, 100, 200, 900].map((y) => ({ x: 0, y, w: 50, h: 40, color: "color.accent" as const })) });
    const engine = new Engine();
    frame();

    const c = made[0]!.canvas;
    expect(c.parentElement).toBe(wrap);
    // The surface fills the section box (from the top band to the bottom of the scrolling box). Content sticking out above the scrolling box does not grow it.
    expect([c.width, c.height, c.style.top]).toEqual([300, 300, "0px"]);
    const accent = f32(rgb("color.accent")).join();
    const got = drawn.at(-1)!.rects.filter((r) => r.slice(4).join() === accent).map((r) => r.slice(0, 4));
    // Window y 30 (hidden), 130–170 (cut at the scrolling box top 150), 230–270, 930 (below the box). The surface's top left is y 100.
    expect(got).toEqual([
      [0, 50, 50, 70],
      [0, 130, 50, 170],
    ]);
    engine.destroy();
  });

  it("number of sections: at most 10 for any window height and band mix. Walking through every mix makes 10 contexts", () => {
    const { made, frame } = fakePage();
    const engine = new Engine();
    // Always there. The input meter has one box, full or slim.
    const always: RegionName[] = ["inputMeter", "masterMeter"];
    // The arrange is either track rows (tall window, or no lower band) or the shrunk form (overview).
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

  it("contexts are made when the section box comes, and kept while the box exists even with no slots (not made at the moment of a key or a drag)", () => {
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

    // When the box goes, the context returns to the pool and the next section uses it.
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

  it("making a context over the budget fails in development and tests", () => {
    const { frame } = fakePage();
    const engine = new Engine();
    for (let i = 0; i <= CONTEXT_BUDGET; i++) {
      slot(region(box(new DOMRect(0, 10 * i, 100, 8)), "masterMeter"), { kind: "meter", source: "master", gap: 1 });
    }
    expect(frame).toThrow(/budget of 12/);
    expect(contextsMade()).toBe(CONTEXT_BUDGET);
    engine.destroy();
  });

  it("a slot outside its section fails in development and tests", () => {
    const { frame } = fakePage();
    const engine = new Engine();
    slot(box(new DOMRect(0, 0, 64, 4)), { kind: "velocity" });
    expect(frame).toThrow(/not inside any glRegion/);
    engine.destroy();
  });

  it("order of rectangles: CSS px become pixels. Later slots are on top", () => {
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
    // Even when marked first, they are drawn in page order.
    slot(lanes, grid);
    const engine = new Engine();
    frame();
    const rects = drawn.at(-1)!.rects;
    // The bar line (x = 0) first, the clip after. The clip box starts at (180, 165).
    expect(rects.map((r) => r.slice(0, 4))).toEqual([
      [0, 0, 1, 300],
      [30, 15, 180, 75],
      [45, 20, 53, 23],
    ]);
    expect(rects[1]!.slice(4)).toEqual(f32(rgb("color.track.1")));
    engine.destroy();
  });

  it("everyGlFrame: called at the start of the frame; slots changed there are drawn in the same frame. Frames go on until it stops", () => {
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

  it("section table: every section has a name, and the budget is below the WebKitGTK limit", () => {
    expect(Object.keys(REGIONS).length).toBeLessThanOrEqual(CONTEXT_BUDGET);
    expect(CONTEXT_BUDGET).toBeLessThan(16);
  });
});
