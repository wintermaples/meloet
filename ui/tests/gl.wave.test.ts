// 波形の WebGL の側: ピークをテクスチャに入れる形（折りかた、ふえていく時の書きたし、ためる上限）、四角と波形の順、
// 同じ絵かどうか、録っているクリップの面とトラックの計の面の上下をそろえる事。
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

describe("テクスチャに入れる形", () => {
  it("ピークは幅 2048 の行に折る: 行の残り、まるごとの行、次の行の頭", () => {
    expect(texWrites(0, 10)).toEqual([{ x: 0, y: 0, w: 10, h: 1, from: 0 }]);
    expect(texWrites(0, 3 * W)).toEqual([{ x: 0, y: 0, w: W, h: 3, from: 0 }]);
    expect(texWrites(W - 5, 3 * W + 7)).toEqual([
      { x: W - 5, y: 0, w: 5, h: 1, from: W - 5 },
      { x: 0, y: 1, w: W, h: 2, from: W },
      { x: 0, y: 3, w: 7, h: 1, from: 3 * W },
    ]);
    expect(texWrites(100, 100)).toEqual([]);
    // 行の途中から、同じ行の中で終わる。
    expect(texWrites(W + 10, W + 30)).toEqual([{ x: 10, y: 1, w: 20, h: 1, from: W + 10 }]);
  });

  it("変わらないファイルは、ちょうどの行の数。ふえていく物は 2 倍ずつ先に取り、書くのは前の最後のピークから", () => {
    expect(texturePlan(null, 5000, false)).toMatchObject({ rows: 3, count: 5000 });
    expect(texturePlan(null, 5000, false).writes[0]).toMatchObject({ from: 0 });
    const first = texturePlan(null, 5000, true);
    expect(first.rows).toBe(4);
    // 行が足りている間は、取りなおさない。最後のピーク（まだ変わる）から書く。
    const more = texturePlan({ rows: 4, count: 5000 }, 5019, true);
    expect(more.rows).toBe(4);
    expect(more.writes).toEqual([{ x: 4999 - 2 * W, y: 2, w: 20, h: 1, from: 4999 }]);
    // 足りなくなったら 2 倍にして、ぜんぶ書きなおす。
    const grown = texturePlan({ rows: 4, count: 4 * W - 1 }, 4 * W + 9, true);
    expect(grown.rows).toBe(8);
    expect(grown.writes[0]).toMatchObject({ from: 0 });
    // 同じ数のまま（最後のピークだけ変わった）。
    expect(texturePlan({ rows: 4, count: 5000 }, 5000, true).writes).toEqual([{ x: 4999 - 2 * W, y: 2, w: 1, h: 1, from: 4999 }]);
  });

  it("上限（2048 行）より長い音は、入る所まで", () => {
    const plan = texturePlan(null, WAVE_TEX_MAX_PEAKS + 500, true);
    expect(plan.rows).toBe(WAVE_TEX_MAX_ROWS);
    expect(plan.count).toBe(WAVE_TEX_MAX_PEAKS);
  });
});

/** 呼ばれた事を書きとめる、にせのテクスチャ。 */
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

describe("テクスチャの帳面（WaveTextures）", () => {
  it("同じピークは 1 回だけ入れる。ふえたら、ふえた分だけ書きたす。チャンネル 2 つは 4 つの数で 1 texel", () => {
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
    // 前の最後のピーク（2999）から。
    expect(log).toEqual([`write 1 ${2999 - W},1 41x1 @${2999 * 4} first=${-((2999 * 4) % 1000)}`]);
    expect(book.held()).toEqual({ textures: 1, bytes: 2 * W * 2 * 4 });
  });

  it("ためる上限: その絵が使わなかったテクスチャは、古い順に、上限に入るまで手ばなす。使った物は上限をこえても持つ", () => {
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
    // 3 行ぶんで上限 2 行: いちばん古い a だけ手ばなす。
    expect(log.filter((l) => l.startsWith("free"))).toEqual(["free 1"]);
    expect(book.held()).toEqual({ textures: 2, bytes: 2 * row });
    // 1 枚の絵が 3 つとも使えば、3 つとも持つ。
    book.use(a);
    book.use(b);
    book.use(c);
    book.endPicture();
    expect(book.held().textures).toBe(3);
    // 次の絵が c だけなら、古い順に a ・ b のうち上限に入るまで。
    book.use(c);
    book.endPicture();
    expect(book.held().textures).toBe(2);
  });

  it("文脈が失われたら、ぜんぶ入れなおす", () => {
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

describe("絵の中の波形（Batch）", () => {
  it("四角と同じ箱で切る。入れた順（その前の四角の数）をおぼえる", () => {
    const b = new Batch();
    b.clip(0, 0, 50, 100);
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    b.wave(quad(source(peaks(1000))));
    b.rect(0, 0, 10, 10, rgb("color.gray.4"));
    expect(b.waves).toHaveLength(1);
    // 読むピークは、右のはしの棒が届く所まで（x 50 の棒は 3 px 先まで読む）。
    expect(b.waves[0]).toMatchObject({ x0: 10, x1: 50, y0: 20, y1: 60, at: 30, mapX: 10, end: 44 });
    // 箱の外なら、足さない。
    b.clip(0, 0, 5, 100);
    b.wave(quad(source(peaks(1000))));
    expect(b.waves).toHaveLength(1);
    b.reset();
    expect(b.waves).toHaveLength(0);
  });

  it("同じ絵か: 音量の倍率 ・ フェード ・ 場所 ・ 音がちがえば、ちがう絵", () => {
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

  it("ふえていく音: 見せている所が最後のピークに届いていなければ、ふえても同じ絵（大きな面を描きなおさない）", () => {
    const s = source(peaks(1000, 1, true));
    const make = (q: WaveQuad) => {
      const b = new Batch();
      b.clip(0, 0, 500, 500);
      b.wave(q);
      return b;
    };
    // x 10〜110 が読むのはピーク 0〜103 まで。
    const before = make(quad(s));
    s.peaks = peaks(1020, 1, true);
    s.stamp += 1;
    expect(before.sameAs(make(quad(s)))).toBe(true);
    // 最後のピークまで見せている絵は、ふえたら変わる。
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

describe("四角の並びの中の波形（paintRects）", () => {
  const box = { x0: 100, y0: 50, x1: 600, y1: 400 };
  const rects = [
    { x: 0, y: 0, w: 10, h: 10, color: "color.gray.4" as const },
    { x: 0, y: 0, w: 10, h: 10, color: "color.gray.6" as const },
  ];

  it("after の数の四角のあとに描く（クリップの面の上、あとのクリップの下）", () => {
    const s = source(peaks(500));
    const out = new Batch();
    out.clip(0, 0, 1000, 1000);
    paintRects(out, box, rects, 1, [wave(s, { after: 1 }), wave(s, { after: 2 })]);
    expect(out.rectCount).toBe(2);
    expect(out.waves.map((w) => w.at)).toEqual([30, 60]);
  });

  it("CSS px を画素に: 場所とフェードは率をかけ、ピークの歩みは割る。棒は画素ちょうど（1 より細くしない）", () => {
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
    // 棒は、クリップの頭から数えた所（画素ちょうど）のうち、四角のすぐ左の物から数える。
    expect((w.barsX - Math.round(104.5)) % 5).toBe(0);
    expect(w.barsX).toBeLessThanOrEqual(106);
    expect(w.barsX).toBeGreaterThan(106 - 5);
  });
});

// ---- 描く係（にせの WebGL2） ----

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

describe("描く係と波形", () => {
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

  it("四角 → 波形 → あとの四角の順に描く。ピークは 1 チャンネルなら RG16I、2 チャンネルなら RGBA16I に、1 回だけ入れる", () => {
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
    // 音量だけ変えた絵: テクスチャはそのまま、描きなおすだけ。
    calls.length = 0;
    s.update({ kind: "rects", rects, waves: [wave(mono, { after: 1, gain: 0.25 }), wave(stereo, { after: 2 })] });
    frame();
    expect(calls).toEqual(["drawArrays TRIANGLES 0 6", "drawArrays TRIANGLE_STRIP 0 4", "drawArrays TRIANGLES 6 6", "drawArrays TRIANGLE_STRIP 0 4"]);
    // 同じ絵なら、出さない。
    calls.length = 0;
    s.update({ kind: "rects", rects, waves: [wave(mono, { after: 1, gain: 0.25 }), wave(stereo, { after: 2 })] });
    frame();
    expect(calls).toEqual([]);
    engine.destroy();
  });

  it("alignY: 録っているクリップの先がある間、その面とトラックの計の面は、同じ上と下（2 つを合わせた所）。なくなれば、もとの大きさ", () => {
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
    // 2 行目のトラックに録っている: 先の箱は y 167〜224。
    const tail = slot(box(new DOMRect(300, 167, 129, 57), lanes), { kind: "rects", rects: [], alignY: "trackMeters" });
    frame();
    expect([meters.style.top, meters.height]).toEqual(["53px", 139]);
    expect([rec.style.top, rec.height, rec.style.left, rec.width]).toEqual(["53px", 139, "80px", 129]);
    // 先が計より上や下に出ても、2 つを合わせた所。
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

  it("alignY のない slot（クリップのドラッグ）は、トラックの計の面を変えない", () => {
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

  it("Renderer は、波形のない絵では前と同じ 1 回の描画", () => {
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
