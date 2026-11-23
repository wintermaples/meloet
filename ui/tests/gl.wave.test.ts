// The WebGL side of the waveform: how peaks go into a texture (folding, adding while it grows, the limit kept), the order of
// boxes and waveforms, whether a picture is the same, and lining up the recording clip's surface with the track meter's surface.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Batch, type WaveQuad } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { Engine } from "../src/gl/engine";
import { paintRects } from "../src/gl/paint";
import { resetPool } from "../src/gl/pool";
import { Renderer } from "../src/gl/renderer";
import { glRegion, glSlot, type GlSlot, type GlWave } from "../src/gl/slot";
import type { RegionName } from "../src/gl/budget";
import {
  WAVE_TEX_MAX_PEAKS,
  WAVE_TEX_MAX_ROWS,
  WAVE_TEX_WIDTH,
  WaveTextures,
  newWaveSourceId,
  texWrites,
  texturePlan,
  type TexOps,
  type WavePeaks,
  type WaveSource,
} from "../src/gl/wave";

const W = WAVE_TEX_WIDTH;

function peaks(count: number, channels = 1, growing = false): WavePeaks {
  const values = new Int16Array(count * channels * 2);
  for (let i = 0; i < values.length; i++) values[i] = i % 2 === 0 ? -(i % 1000) : i % 1000;
  return { channels, count, growing, values };
}
const source = (p: WavePeaks): WaveSource => ({ id: newWaveSourceId(), peaks: p, stamp: 0 });

describe("the texture layout", () => {
  it("peaks fold into rows 2048 wide: rest of a row, whole rows, start of the next row", () => {
    expect(texWrites(0, 10)).toEqual([{ x: 0, y: 0, w: 10, h: 1, from: 0 }]);
    expect(texWrites(0, 3 * W)).toEqual([{ x: 0, y: 0, w: W, h: 3, from: 0 }]);
    expect(texWrites(W - 5, 3 * W + 7)).toEqual([
      { x: W - 5, y: 0, w: 5, h: 1, from: W - 5 },
      { x: 0, y: 1, w: W, h: 2, from: W },
      { x: 0, y: 3, w: 7, h: 1, from: 3 * W },
    ]);
    expect(texWrites(100, 100)).toEqual([]);
    // Starts in the middle of a row and ends in the same row.
    expect(texWrites(W + 10, W + 30)).toEqual([{ x: 10, y: 1, w: 20, h: 1, from: W + 10 }]);
  });

  it("a file that does not change gets exactly the rows it needs. A growing one reserves 2× ahead, and writing starts from the last peak before", () => {
    expect(texturePlan(null, 5000, false)).toMatchObject({ rows: 3, count: 5000 });
    expect(texturePlan(null, 5000, false).writes[0]).toMatchObject({ from: 0 });
    const first = texturePlan(null, 5000, true);
    expect(first.rows).toBe(4);
    // While there are enough rows, nothing is reallocated. Writing starts from the last peak (which can still change).
    const more = texturePlan({ rows: 4, count: 5000 }, 5019, true);
    expect(more.rows).toBe(4);
    expect(more.writes).toEqual([{ x: 4999 - 2 * W, y: 2, w: 20, h: 1, from: 4999 }]);
    // When rows run out, double them and write everything again.
    const grown = texturePlan({ rows: 4, count: 4 * W - 1 }, 4 * W + 9, true);
    expect(grown.rows).toBe(8);
    expect(grown.writes[0]).toMatchObject({ from: 0 });
    // Same count (only the last peak changed).
    expect(texturePlan({ rows: 4, count: 5000 }, 5000, true).writes).toEqual([{ x: 4999 - 2 * W, y: 2, w: 1, h: 1, from: 4999 }]);
  });

  it("sound longer than the limit (2048 rows) goes in only as far as it fits", () => {
    const plan = texturePlan(null, WAVE_TEX_MAX_PEAKS + 500, true);
    expect(plan.rows).toBe(WAVE_TEX_MAX_ROWS);
    expect(plan.count).toBe(WAVE_TEX_MAX_PEAKS);
  });
});

/** A fake texture that writes down what was called. */
function fakeOps() {
  const log: string[] = [];
  let made = 0;
  const ops: TexOps<number> = {
    make: () => ++made,
    alloc: (t, channels, rows) => void log.push(`alloc ${t} ${channels}ch ${rows}`),
    write: (t, _, at, values, offset) => void log.push(`write ${t} ${at.x},${at.y} ${at.w}x${at.h} @${offset} first=${values[offset]}`),
    free: (t) => void log.push(`free ${t}`),
  };
  return { ops, log };
}

describe("texture book (WaveTextures)", () => {
  it("the same peaks go in only once. When they grow, only the new part is added. 2 channels are 1 texel of 4 numbers", () => {
    const { ops, log } = fakeOps();
    const book = new WaveTextures(ops);
    const s = source(peaks(3000, 2, true));
    const tex = book.use(s);
    expect(log).toEqual(["alloc 1 2ch 2", "write 1 0,0 2048x1 @0 first=0", `write 1 0,1 ${3000 - W}x1 @${W * 4} first=${-((W * 4) % 1000)}`]);
    log.length = 0;
    expect(book.use(s)).toBe(tex);
    expect(log).toEqual([]);
    s.peaks = peaks(3040, 2, true);
    s.stamp += 1;
    expect(book.use(s)).toBe(tex);
    // From the last peak before (2999).
    expect(log).toEqual([`write 1 ${2999 - W},1 41x1 @${2999 * 4} first=${-((2999 * 4) % 1000)}`]);
    expect(book.held()).toEqual({ textures: 1, bytes: 2 * W * 2 * 4 });
  });

  it("texture limit: textures that the picture did not use are let go, oldest first, until under the limit. Used ones are kept even over the limit", () => {
    const { ops, log } = fakeOps();
    const row = W * 4;
    const book = new WaveTextures(ops, 2 * row);
    const a = source(peaks(100));
    const b = source(peaks(100));
    const c = source(peaks(100));
    book.use(a);
    book.endPicture();
    book.use(b);
    book.endPicture();
    book.use(c);
    book.endPicture();
    // 3 rows with a limit of 2 rows: only the oldest, a, is let go.
    expect(log.filter((l) => l.startsWith("free"))).toEqual(["free 1"]);
    expect(book.held()).toEqual({ textures: 2, bytes: 2 * row });
    // If one picture uses all 3, all 3 are kept.
    book.use(a);
    book.use(b);
    book.use(c);
    book.endPicture();
    expect(book.held().textures).toBe(3);
    // If the next picture uses only c, a and b are let go oldest first until under the limit.
    book.use(c);
    book.endPicture();
    expect(book.held().textures).toBe(2);
  });

  it("when the context is lost, everything is put in again", () => {
    const { ops, log } = fakeOps();
    const book = new WaveTextures(ops);
    const s = source(peaks(10));
    book.use(s);
    book.forget();
    log.length = 0;
    book.use(s);
    expect(log[0]).toBe("alloc 2 1ch 1");
  });
});

const quad = (s: WaveSource, more: Partial<WaveQuad> = {}): WaveQuad => ({
  x0: 10,
  y0: 20,
  x1: 110,
  y1: 60,
  source: s,
  mapX: 10,
  base: 0,
  frac: 0,
  step: 1,
  first: 0,
  end: 1e9,
  top: 20,
  bottom: 60,
  gain: 1,
  fadeX0: 10,
  fadeIn: 0,
  fadeX1: 110,
  fadeOut: 0,
  barsX: 10,
  pitch: 3,
  bar: 1,
  minH: 2,
  color: rgb("color.gray.8"),
  mid: null,
  line: null,
  ...more,
});

describe("waveforms in a picture (Batch)", () => {
  it("cut by the same box as the rectangles. Remembers the order they were added (the number of rectangles before)", () => {
    const b = new Batch();
    b.clip(0, 0, 50, 100);
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    b.wave(quad(source(peaks(1000))));
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    expect(b.waves).toHaveLength(1);
    // Peaks are read as far as the bar at the right edge reaches (a bar at x 50 reads 3 px further).
    expect(b.waves[0]).toMatchObject({ x0: 10, x1: 50, y0: 20, y1: 60, at: 30, mapX: 10, end: 44 });
    // Outside the box, nothing is added.
    b.clip(0, 0, 5, 100);
    b.wave(quad(source(peaks(1000))));
    expect(b.waves).toHaveLength(1);
    b.reset();
    expect(b.waves).toHaveLength(0);
  });

  it("same picture: a different volume factor, fade, place or sound makes a different picture", () => {
    const s = source(peaks(1000));
    const make = (q: WaveQuad) => {
      const b = new Batch();
      b.clip(0, 0, 500, 500);
      b.wave(q);
      return b;
    };
    const a = make(quad(s));
    expect(a.sameAs(make(quad(s)))).toBe(true);
    expect(a.sameAs(make(quad(s, { gain: 0.5 })))).toBe(false);
    expect(a.sameAs(make(quad(s, { fadeIn: 30 })))).toBe(false);
    expect(a.sameAs(make(quad(s, { frac: 0.5 })))).toBe(false);
    expect(a.sameAs(make(quad(s, { mid: rgb("color.gray.5") })))).toBe(false);
    expect(a.sameAs(make(quad(source(peaks(1000)))))).toBe(false);
    expect(a.sameAs(new Batch())).toBe(false);
  });

  it("growing sound: if the shown part does not reach the last peak, growing keeps the same picture (the large surface is not redrawn)", () => {
    const s = source(peaks(1000, 1, true));
    const make = (q: WaveQuad) => {
      const b = new Batch();
      b.clip(0, 0, 500, 500);
      b.wave(q);
      return b;
    };
    // x 10 to 110 reads peaks 0 to 103.
    const before = make(quad(s));
    s.peaks = peaks(1020, 1, true);
    s.stamp += 1;
    expect(before.sameAs(make(quad(s)))).toBe(true);
    // A picture that shows up to the last peak changes when the sound grows.
    const tail = make(quad(s, { base: 950 }));
    s.peaks = peaks(1040, 1, true);
    s.stamp += 1;
    expect(tail.sameAs(make(quad(s, { base: 950 })))).toBe(false);
  });
});

const wave = (s: WaveSource, more: Partial<GlWave> = {}): GlWave => ({
  after: 1,
  x: 4,
  y: 10,
  w: 100,
  h: 30,
  source: s,
  peak: 12.25,
  peaksPerPx: 2,
  first: 0,
  end: 500,
  gain: 0.5,
  clipX0: 3,
  clipX1: 105,
  fadeIn: 8,
  fadeOut: 0,
  pitch: 3,
  bar: 1,
  minH: 2,
  color: "color.gray.8",
  mid: null,
  line: "color.gray.10",
  ...more,
});

describe("waveforms in the rectangle list (paintRects)", () => {
  const box = { x0: 100, y0: 50, x1: 600, y1: 400 };
  const rects = [
    { x: 0, y: 0, w: 10, h: 10, color: "color.gray.4" as const },
    { x: 0, y: 0, w: 10, h: 10, color: "color.gray.6" as const },
  ];

  it("drawn after `after` rectangles (above the clip's surface, below later clips)", () => {
    const s = source(peaks(500));
    const out = new Batch();
    out.clip(0, 0, 1000, 1000);
    paintRects(out, box, rects, 1, [wave(s, { after: 1 }), wave(s, { after: 2 })]);
    expect(out.rectCount).toBe(2);
    expect(out.waves.map((w) => w.at)).toEqual([30, 60]);
  });

  it("CSS px to device pixels: place and fade are multiplied by the scale, the peak step is divided. Bars are whole pixels (never thinner than 1)", () => {
    const s = source(peaks(500));
    const out = new Batch();
    out.clip(0, 0, 2000, 2000);
    paintRects(out, box, rects, 1.5, [wave(s)]);
    const w = out.waves[0]!;
    expect([w.x0, w.y0, w.x1, w.y1]).toEqual([106, 65, 256, 110]);
    expect([w.mapX, w.base, w.frac, w.step]).toEqual([106, 12, 0.25, 2 / 1.5]);
    expect([w.top, w.bottom]).toEqual([65, 110]);
    expect([w.fadeX0, w.fadeIn, w.fadeX1]).toEqual([104.5, 12, 257.5]);
    expect([w.pitch, w.bar, w.minH]).toEqual([5, 2, 3]);
    expect(w.gain).toBe(0.5);
    expect(w.line).toEqual(rgb("color.gray.10"));
    // Bars are counted from the clip start (whole pixels), starting from the one just left of the rectangle.
    expect((w.barsX - Math.round(104.5)) % 5).toBe(0);
    expect(w.barsX).toBeLessThanOrEqual(106);
    expect(w.barsX).toBeGreaterThan(106 - 5);
  });
});

// ---- renderer (fake WebGL2) ----

class FakeResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

function fakePage() {
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) => frames.push(f));
  vi.stubGlobal("cancelAnimationFrame", () => {});
  const calls: string[] = [];
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(function () {
    return new Proxy(
      {},
      {
        get: (_, name: string) => {
          if (name === "getShaderParameter" || name === "getProgramParameter") return () => true;
          if (typeof name === "string" && /^[A-Z_0-9]+$/.test(name)) return name;
          return (...args: unknown[]) => {
            if (name === "drawArrays" || name === "texImage2D" || name === "texSubImage2D") calls.push(`${name} ${args.slice(0, name === "drawArrays" ? 3 : 8).join(" ")}`);
            return {};
          };
        },
      },
    ) as never;
  });
  const frame = () => {
    for (const f of frames.splice(0)) f(0);
  };
  return { calls, frame };
}

function box(rect: DOMRect, parent: HTMLElement = document.body): HTMLElement {
  const el = parent.appendChild(document.createElement("div"));
  el.getBoundingClientRect = () => rect;
  return el;
}

describe("renderer and waveforms", () => {
  const actions: { destroy(): void }[] = [];
  const region = (node: HTMLElement, name: RegionName) => (actions.push(glRegion(node, name)), node);
  const slot = (node: HTMLElement, s: GlSlot) => {
    const a = glSlot(node, s);
    actions.push(a);
    return a;
  };
  beforeEach(() => resetPool());
  afterEach(() => {
    for (const a of actions.splice(0)) a.destroy();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
  });

  it("draws rectangles -> waveforms -> later rectangles. Peaks go in once, as RG16I for 1 channel and RGBA16I for 2 channels", () => {
    const { calls, frame } = fakePage();
    const host = region(box(new DOMRect(0, 0, 400, 100)), "arrangeLanes");
    const rects = [
      { x: 0, y: 0, w: 200, h: 50, color: "color.gray.4" as const },
      { x: 300, y: 0, w: 50, h: 50, color: "color.gray.6" as const },
    ];
    const mono = source(peaks(300, 1));
    const stereo = source(peaks(300, 2));
    const s = slot(host, { kind: "rects", rects, waves: [wave(mono, { after: 1 }), wave(stereo, { after: 2 })] });
    const engine = new Engine();
    frame();
    expect(calls).toEqual([
      "drawArrays TRIANGLES 0 6",
      `texImage2D TEXTURE_2D 0 RG16I ${W} 1 0 RG_INTEGER SHORT`,
      "texSubImage2D TEXTURE_2D 0 0 0 300 1 RG_INTEGER SHORT",
      "drawArrays TRIANGLE_STRIP 0 4",
      "drawArrays TRIANGLES 6 6",
      `texImage2D TEXTURE_2D 0 RGBA16I ${W} 1 0 RGBA_INTEGER SHORT`,
      "texSubImage2D TEXTURE_2D 0 0 0 300 1 RGBA_INTEGER SHORT",
      "drawArrays TRIANGLE_STRIP 0 4",
    ]);
    // A picture with only the volume changed: the texture stays, it is only redrawn.
    calls.length = 0;
    s.update({ kind: "rects", rects, waves: [wave(mono, { after: 1, gain: 0.25 }), wave(stereo, { after: 2 })] });
    frame();
    expect(calls).toEqual(["drawArrays TRIANGLES 0 6", "drawArrays TRIANGLE_STRIP 0 4", "drawArrays TRIANGLES 6 6", "drawArrays TRIANGLE_STRIP 0 4"]);
    // The same picture is not drawn.
    calls.length = 0;
    s.update({ kind: "rects", rects, waves: [wave(mono, { after: 1, gain: 0.25 }), wave(stereo, { after: 2 })] });
    frame();
    expect(calls).toEqual([]);
    engine.destroy();
  });

  it("alignY: while the recording clip's tip exists, its surface and the track meter's surface share the same top and bottom (the union of the two). When it is gone, the original size", () => {
    const { frame } = fakePage();
    const heads = region(box(new DOMRect(0, 100, 220, 400)), "trackMeters");
    slot(box(new DOMRect(12, 153, 199, 11), heads), { kind: "meter", source: { track: 1 }, gap: 1 });
    slot(box(new DOMRect(12, 281, 199, 11), heads), { kind: "meter", source: { track: 3 }, gap: 1 });
    const lanes = region(box(new DOMRect(220, 100, 1000, 400)), "recordingClip");
    const engine = new Engine();
    frame();
    const meters = heads.querySelector("canvas")!;
    const rec = lanes.querySelector("canvas")!;
    expect([meters.style.top, meters.height]).toEqual(["53px", 139]);
    // Recording on the track in row 2: the tip's box is y 167 to 224.
    const tail = slot(box(new DOMRect(300, 167, 129, 57), lanes), { kind: "rects", rects: [], alignY: "trackMeters" });
    frame();
    expect([meters.style.top, meters.height]).toEqual(["53px", 139]);
    expect([rec.style.top, rec.height, rec.style.left, rec.width]).toEqual(["53px", 139, "80px", 129]);
    // Even if the tip goes above or below the meter, the union of the two.
    tail.destroy();
    const high = slot(box(new DOMRect(300, 103, 129, 57), lanes), { kind: "rects", rects: [], alignY: "trackMeters" });
    frame();
    expect([meters.style.top, meters.height]).toEqual(["3px", 189]);
    expect([rec.style.top, rec.height]).toEqual(["3px", 189]);
    high.destroy();
    frame();
    expect([meters.style.top, meters.height]).toEqual(["53px", 139]);
    engine.destroy();
  });

  it("a slot without alignY (clip drag) does not change the track meter's surface", () => {
    const { frame } = fakePage();
    const heads = region(box(new DOMRect(0, 100, 220, 400)), "trackMeters");
    slot(box(new DOMRect(12, 153, 199, 11), heads), { kind: "meter", source: { track: 1 }, gap: 1 });
    const lanes = region(box(new DOMRect(220, 100, 1000, 400)), "recordingClip");
    slot(box(new DOMRect(300, 103, 129, 57), lanes), { kind: "rects", rects: [] });
    const engine = new Engine();
    frame();
    expect(heads.querySelector("canvas")!.height).toBe(11);
    expect(lanes.querySelector("canvas")!.height).toBe(57);
    engine.destroy();
  });

  it("Renderer draws a picture without waveforms in one draw, as before", () => {
    const { calls } = fakePage();
    const r = new Renderer(document.createElement("canvas"), false);
    const b = new Batch();
    b.clip(0, 0, 100, 100);
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    r.draw(b, 100, 100);
    expect(calls).toEqual(["drawArrays TRIANGLES 0 12"]);
  });
});
