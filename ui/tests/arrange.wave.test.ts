// 歌のクリップの波形の、アレンジの側（仕様書 3 章「波形」「クリップの音量」「切る ・ フェード ・ 重なり」「音のファイルが見つからないとき」、
// モック F7-AudioEdit ・ R11-Audio）: 段のえらび、波形の場所、音量の倍率、フェード、見つからない ・ 読みこみ中、ピークの帳面、
// Arrange を mount したときの絵。
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView } from "../src/bridge";
import { FAKE_AUDIO, fakeScene, type FakeState } from "../src/bridge/fake";
import { fakeAudioClip, fakePeaks, fakeStretch, fakeVocalClip, steadyTempo, type FakeAudio } from "../src/bridge/fakeAudio";
import { PEAK_LEVELS } from "../src/bridge/generated/audio";
import { PeakStore, type LevelPeaks } from "../src/bridge/peaks";
import Arrange from "../src/components/Arrange.svelte";
import { contextMenu } from "../src/components/menu/contextMenu.svelte";
import { fileFrameAt, gainFactor, waveArea } from "../src/components/arrange/audioClipGeom";
import { audioDrag } from "../src/components/arrange/audioDrag.svelte";
import { clipFadeGain } from "../src/components/arrange/fade";
import * as W from "../src/components/arrange/wave";
import { WAVE_POLL_MS, WaveBook, type WaveFetch } from "../src/components/arrange/wavePeaks.svelte";
import * as G from "../src/components/arrangeGeom";
import { mock, tokensAt } from "../src/design/tokens";
import { watchSlots, type GlSlot, type GlWave } from "../src/gl/slot";
import type { WaveSource } from "../src/gl/wave";
import { TempoMap } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

// 橋のピーク: 偽物の音のファイル（fakeAudio.ts）から、本物の `PeakStore` で。
const store = vi.hoisted(() => ({ current: null as null | { get: (a: number, l: number) => Promise<unknown>; more: (a: number, l: number) => Promise<unknown>; peek: (a: number, l: number) => unknown } }));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  audioPeaks: (audio: number, level: number) => store.current!.get(audio, level),
  moreAudioPeaks: (audio: number, level: number) => store.current!.more(audio, level),
  peekAudioPeaks: (audio: number, level: number) => store.current!.peek(audio, level),
}));

const BAR = 4 * TPQ;
const steady = new TempoMap({ tempoBpm: 120, meterNum: 4, meterDen: 4, headRamp: false, tempoPoints: [], meterChanges: [] });
/** 3 小節目の頭で 120 → 60。 */
const slower = new TempoMap({ tempoBpm: 120, meterNum: 4, meterDen: 4, headRamp: false, tempoPoints: [{ tick: 2 * BAR, tempoBpm: 60, ramp: false }], meterChanges: [] });

// 1 小節 64 px、行の高さ 64（拡大率 100 %）。
const view: G.View = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
const dims: G.LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
const sizes = { pitch: 3, bar: 1, minH: 2 };
const look = W.waveLook(sizes, "color.gray.8", false);

const TAKE: FakeAudio = { id: 7, file: "歌.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 8 * 48000 };
const sourceOf = (count: number, growing = false): WaveSource => ({ id: 1, stamp: 0, peaks: { channels: 1, count, growing, values: new Int16Array(count * 2) } });
/** ファイルの速さのまま鳴るクリップ（「この音のテンポ」が「—」）。ついていく物は `more` に `fakeStretch` で。 */
const vocal = (start: number, end: number, more: Parameters<typeof fakeAudioClip>[1] = {}, takeOffset = 0): ClipView =>
  fakeVocalClip(1, "歌", start, end, fakeAudioClip(TAKE, { ...fakeStretch(null, 120), ...more }), takeOffset);

// docs/ は ui/ の外なので、screen.test.ts と同じく Node の fs で読む。
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const readMock = (board: string) => fs.readFileSync(new URL(`../../docs/design/mock/${board}.dc.html`, here), "utf8");

describe("寸法（モック F7-AudioEdit）", () => {
  it.each(Object.entries(mock).filter(([k]) => k.startsWith("mock.wave.")))("%s の値はモックに書いてあり、率をかけた値は整数", (name, tok) => {
    expect(readMock(tok.board)).toContain(`${tok.value}px`);
    for (const s of [100, 125, 150] as const) expect(Number.isInteger(tokensAt(s, 1)[name as keyof typeof mock] as number)).toBe(true);
  });
});

describe("波形の場所（どの x が、ファイルのどこか）", () => {
  it("frameAtTick は、クリップでは fileFrameAt と同じ答え（テンポが変わる曲、頭をつめたクリップでも）", () => {
    const clip = vocal(2 * BAR, 6 * BAR, { firstFrame: 1200 }, -BAR);
    const shape = W.waveClipOf(clip)!;
    for (const map of [steady, slower]) {
      for (const tick of [0, BAR, 2 * BAR, 2.5 * BAR, 5 * BAR]) expect(W.frameAtTick(map, shape.timing, tick)).toBeCloseTo(fileFrameAt(map, clip, tick), 6);
    }
    // 鍵盤のクリップには波形がない。
    expect(W.waveClipOf({ ...clip, audio: null })).toBeNull();
  });

  it("クリップ 1 つに四角 1 つ: 名前の行の下から下の枠の上まで。左はしのピークと、1 px のピークの数", () => {
    const clip = vocal(4 * BAR, 6 * BAR);
    const box = G.clipBox(view, dims, 0, clip.start, clip.end);
    const area = waveArea(dims, box);
    const out: GlWave[] = [];
    // テンポ 120 ・ 48 kHz: 1 小節 2 秒 = 96000 こま = 64 px。段 1024 こまなら 1 px に 1500 / 1024 個。
    W.clipWaves(out, 3, steady, view, area, W.waveClipOf(clip)!, sourceOf(1000), 1024, look);
    expect(out).toHaveLength(1);
    const w = out[0]!;
    expect([w.x, w.y, w.w, w.h]).toEqual([257, 3 + 2 + 16, 126, 57 - 2 - 16 - 1]);
    expect(w.after).toBe(3);
    // クリップの頭（x 256）がこま 0。四角は 1 px 右から。
    expect(w.peak).toBeCloseTo(1500 / 1024, 9);
    expect(w.peaksPerPx).toBeCloseTo(1500 / 1024, 9);
    expect(w.first).toBe(0);
    expect(w.end).toBe(Math.ceil((8 * 48000) / 1024));
    expect([w.clipX0, w.clipX1]).toEqual([256, 384]);
    expect([w.pitch, w.bar, w.minH, w.color, w.mid]).toEqual([3, 1, 2, "color.gray.8", null]);
  });

  it("頭をつめたクリップと、同じテイクの続きのクリップは、ファイルの続きから", () => {
    const clip = vocal(6 * BAR, 8 * BAR, { firstFrame: 500 }, -2 * BAR);
    const out: GlWave[] = [];
    W.clipWaves(out, 0, steady, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), W.waveClipOf(clip)!, sourceOf(1000), 256, look);
    // テイクの 0 ティックは 2 小節前: クリップの頭は 500 + 2 × 96000 こま。
    expect(out[0]!.peak).toBeCloseTo((500 + 2 * 96000 + 1500) / 256, 6);
  });

  it("見えている幅の外は描かない。横に送っても、四角はクリップ 1 つに 1 つのまま", () => {
    const clip = vocal(0, 400 * BAR);
    const area = (v: G.View) => waveArea(dims, G.clipBox(v, dims, 0, clip.start, clip.end));
    const one = (v: G.View) => {
      const out: GlWave[] = [];
      W.clipWaves(out, 0, steady, v, area(v), W.waveClipOf(clip)!, sourceOf(1000), 1024, look);
      return out;
    };
    const far = { ...view, scrollX: 5000 };
    expect(one(far)).toHaveLength(1);
    const w = one(far)[0]!;
    expect(w.x).toBe(-3);
    expect(w.x + w.w).toBe(1003);
    expect(w.peak).toBeCloseTo((4997 * 1500) / 1024, 6);
    // 見えていないクリップ。
    expect(one({ ...view, scrollX: 400 * 64 + 10 })).toEqual([]);
  });

  it("テンポが途中で変わるクリップは、まっすぐな所ごとに分ける（どの切れ目でも、こまは地図どおり）", () => {
    const clip = vocal(0, 6 * BAR);
    const out: GlWave[] = [];
    W.clipWaves(out, 0, slower, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), W.waveClipOf(clip)!, sourceOf(4000), 256, look);
    expect(out.length).toBeGreaterThan(1);
    expect(out.length).toBeLessThanOrEqual(64);
    const shape = W.waveClipOf(clip)!;
    for (const w of out) {
      // 四角の中のどの px でも、まっすぐに読んだこまと地図のこまの差は 1 px ぶんより小さい。
      for (const part of [0, 0.3, 0.7, 1]) {
        const x = w.x + w.w * part;
        const drawn = (w.peak + (x - w.x) * w.peaksPerPx) * 256;
        const real = W.frameAtTick(slower, shape.timing, G.xToTick(view, x));
        expect(Math.abs(drawn - real)).toBeLessThan(3000);
      }
    }
    // となりどうしは、すきまなくつながる。
    for (let i = 1; i < out.length; i++) expect(out[i]!.x).toBeCloseTo(out[i - 1]!.x + out[i - 1]!.w, 9);
    // 3 小節目から先は半分の速さ: 1 px のこまも半分…ではなく倍（同じ px に 2 倍の秒）。
    expect(out.at(-1)!.peaksPerPx).toBeCloseTo(2 * out[0]!.peaksPerPx, 6);
  });

  it("曲のテンポについていくクリップは「この音のテンポ」の地図で置く（曲のテンポが変わっても、同じティックに同じこま）", () => {
    // 96.00 で歌った歌。曲が 120 でも、3 小節目から 60 でも、1 小節（4 拍）は 2.5 秒 = 120000 こま。
    const clip = vocal(0, 6 * BAR, fakeStretch(steadyTempo(96), 120));
    const shape = W.waveClipOf(clip)!;
    expect(shape.timing.source).not.toBeNull();
    // 同じ SourceTempoView からは、同じ地図（クリップごとに 1 回だけ作る）。
    expect(W.waveClipOf(clip)!.timing.source).toBe(shape.timing.source);
    for (const map of [steady, slower]) {
      for (const tick of [0, BAR, 2.5 * BAR, 5 * BAR]) {
        expect(W.frameAtTick(map, shape.timing, tick)).toBeCloseTo((tick / BAR) * 120000, 6);
        expect(fileFrameAt(map, clip, tick)).toBeCloseTo((tick / BAR) * 120000, 6);
      }
    }
    // 曲のテンポが変わる所でも、ついていくクリップは分けない（こまはティックにまっすぐ）。
    const out: GlWave[] = [];
    W.clipWaves(out, 0, slower, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), shape, sourceOf(4000), 256, look);
    expect(out.length).toBe(1);
    expect(out[0]!.peaksPerPx).toBeCloseTo(120000 / 64 / 256, 9);
  });

  it("straightParts: まっすぐなら分けない。だんだん変わる所は半分ずつ、64 個まで", () => {
    expect(W.straightParts((t) => 3 * t + 1, 0, 100, 0.01)).toEqual([0, 100]);
    const bent = W.straightParts((t) => t * t, 0, 100, 1);
    expect(bent.length).toBeGreaterThan(3);
    expect(bent.length).toBeLessThanOrEqual(65);
    expect(bent[0]).toBe(0);
    expect(bent.at(-1)).toBe(100);
    expect(W.straightParts((t) => t, 5, 5, 1)).toEqual([5]);
  });

  it("録っているクリップ: [from, to] の中だけ。こま 0 は録りはじめ、長さはまだ決まっていない", () => {
    const out: GlWave[] = [];
    const box = G.clipBox(view, dims, 0, 5 * BAR, 6.5 * BAR);
    const shape: W.WaveClip = { timing: { start: 4 * BAR, takeOffset: 0, firstFrame: 0, sampleRate: 48000, source: null }, start: 4 * BAR, end: 6.5 * BAR, fadeIn: 0, fadeOut: 0, gain: 1, endFrame: Infinity };
    W.clipWaves(out, 0, steady, view, { ...waveArea(dims, box), x0: box.x0, x1: box.x1 }, shape, sourceOf(1000, true), 256, W.waveLook(sizes, "color.rec", false), { from: 5 * BAR, to: 6.5 * BAR });
    expect(out).toHaveLength(1);
    expect([out[0]!.x, out[0]!.w]).toEqual([320, 96]);
    expect(out[0]!.peak).toBeCloseTo(96000 / 256, 6);
    expect(out[0]!.clipX0).toBe(256);
    expect(out[0]!.end).toBe(Infinity);
    expect(out[0]!.color).toBe("color.rec");
  });
});

describe("段のえらび", () => {
  it("棒 1 本（3 px）の幅より細かい段のうち、いちばん粗い物", () => {
    // 1 px 100 こま → 棒 1 本 300 こま → 256 の段。
    expect(W.waveLevel(100, 3)).toBe(0);
    expect(W.waveLevel(1500, 3)).toBe(2);
    expect(W.waveLevel(1e7, 3)).toBe(PEAK_LEVELS.length - 1);
    // いちばん細かい段より拡大したら、その段のまま（同じピークを横にのばす）。
    expect(W.waveLevel(2, 3)).toBe(0);
  });

  it("1 px のこまは、見えている所のテンポで数える", () => {
    const clip = vocal(0, 6 * BAR);
    const t = W.waveClipOf(clip)!.timing;
    expect(W.framesPerPx(steady, view, t, 0, 6 * BAR)).toBeCloseTo(1500, 6);
    // 3 小節目から先だけ見えている: 半分の速さなので、1 px のこまは倍。
    expect(W.framesPerPx(slower, { ...view, scrollX: 3 * 64 }, t, 0, 6 * BAR)).toBeCloseTo(3000, 6);
    // 見えていないクリップでも、数は出る。
    expect(W.framesPerPx(steady, { ...view, scrollX: 1e6 }, t, 0, 6 * BAR)).toBeCloseTo(1500, 6);
  });
});

describe("音量の倍率とフェード", () => {
  it("クリップの音量は倍率で渡す（テクスチャは同じ）。ドラッグしている間は audioDrag の値", () => {
    const clip = vocal(0, 2 * BAR, { gainDb: -6 });
    expect(W.waveClipOf(clip)!.gain).toBeCloseTo(gainFactor(-6), 12);
    expect(W.waveClipOf(clip, { clip: 1, gainDb: 6 })!.gain).toBeCloseTo(gainFactor(6), 12);
    // ほかのクリップのドラッグは効かない。
    expect(W.waveClipOf(clip, { clip: 2, gainDb: 6 })!.gain).toBeCloseTo(gainFactor(-6), 12);
    // 範囲の外は −60〜+12 に入れる。
    expect(W.waveClipOf(clip, { clip: 1, gainDb: 40 })!.gain).toBeCloseTo(gainFactor(12), 12);
  });

  it("フェードは px で渡し、線（gray.10）はフェードのあるクリップにだけ。ドラッグ中の長さも", () => {
    const clip = vocal(4 * BAR, 6 * BAR, { fadeIn: TPQ, fadeOut: 2 * TPQ });
    const area = waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end));
    const draw = (shape: W.WaveClip) => {
      const out: GlWave[] = [];
      W.clipWaves(out, 0, steady, view, area, shape, sourceOf(1000), 1024, look);
      return out[0]!;
    };
    const w = draw(W.waveClipOf(clip)!);
    expect([w.fadeIn, w.fadeOut, w.line]).toEqual([16, 32, "color.gray.10"]);
    expect(draw(W.waveClipOf(vocal(4 * BAR, 6 * BAR))!).line).toBeNull();
    const dragged = draw(W.waveClipOf(clip, { clip: 1, fadeIn: 2 * TPQ })!);
    expect([dragged.fadeIn, dragged.fadeOut]).toEqual([32, 32]);
  });

  it("シェーダーに渡す形（クリップのはしと px の長さ）は、fade.ts の clipFadeGain と同じ大きさになる", () => {
    const clip = vocal(4 * BAR, 6 * BAR, { fadeIn: TPQ, fadeOut: 2 * TPQ });
    const out: GlWave[] = [];
    W.clipWaves(out, 0, steady, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), W.waveClipOf(clip)!, sourceOf(1000), 1024, look);
    const w = out[0]!;
    // シェーダーの fadeAt（renderer.ts）を同じ式で。
    const rise = (t: number) => (t >= 1 ? 1 : t > 0 ? Math.sin((t * Math.PI) / 2) : 0);
    const shader = (x: number) => (w.fadeIn > 0 ? rise((x - w.clipX0) / w.fadeIn) : 1) * (w.fadeOut > 0 ? rise((w.clipX1 - x) / w.fadeOut) : 1);
    for (const x of [257, 260, 272, 300, 352, 370, 383]) {
      const ticks = G.xToTick(view, x) - clip.start;
      expect(shader(x)).toBeCloseTo(clipFadeGain(ticks, clip.end - clip.start, TPQ, 2 * TPQ), 9);
    }
  });

  it("160 の行は、まん中に gray.5 の線。棒の色は、クリップの中の線の色", () => {
    expect(W.waveLook(sizes, "color.gray.8", true).mid).toBe("color.gray.5");
    expect(W.waveLook(sizes, "color.gray.7", false)).toEqual({ ...sizes, color: "color.gray.7", mid: null });
  });

  it("shiftWaves: 小さな区画の箱の左上から数えなおす", () => {
    const clip = vocal(4 * BAR, 6 * BAR);
    const out: GlWave[] = [];
    W.clipWaves(out, 0, steady, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), W.waveClipOf(clip)!, sourceOf(1000), 1024, look);
    W.shiftWaves(out, -256, -3);
    expect([out[0]!.x, out[0]!.y, out[0]!.clipX0, out[0]!.clipX1]).toEqual([1, 18, 0, 128]);
  });
});

// ---- ピークの帳面 ----

function fakeFetch() {
  const have = new Map<string, LevelPeaks>();
  const waiting = new Map<string, { resolve: (p: LevelPeaks) => void; reject: (e: Error) => void }>();
  const asked: string[] = [];
  const key = (a: number, l: number) => `${a}/${l}`;
  const peaksOf = (audio: number, level: number, count: number, growing = false): LevelPeaks => ({ audio, level, channels: 1, framesPerPeak: PEAK_LEVELS[level]!, growing, count, values: new Int16Array(count * 2) });
  const ask = (kind: string) => (audio: number, level: number) => {
    asked.push(`${kind} ${key(audio, level)}`);
    return new Promise<LevelPeaks>((resolve, reject) => waiting.set(key(audio, level), { resolve, reject }));
  };
  const fetch: WaveFetch = { get: ask("get"), more: ask("more"), peek: (a, l) => have.get(key(a, l)) };
  const arrive = async (audio: number, level: number, count: number, growing = false) => {
    const p = peaksOf(audio, level, count, growing);
    have.set(key(audio, level), p);
    waiting.get(key(audio, level))?.resolve(p);
    waiting.delete(key(audio, level));
    await Promise.resolve();
    await Promise.resolve();
    return p;
  };
  const fail = async (audio: number, level: number) => {
    waiting.get(key(audio, level))?.reject(new Error("not made yet"));
    waiting.delete(key(audio, level));
    await Promise.resolve();
    await Promise.resolve();
  };
  return { fetch, have, asked, arrive, fail, peaksOf, key };
}

describe("ピークの帳面（WaveBook）", () => {
  let book: WaveBook;
  let now = 0;
  const make = (f: WaveFetch) => (book = new WaveBook(f, () => now));
  beforeEach(() => {
    now = 0;
    vi.useFakeTimers();
  });
  afterEach(() => {
    book.stop();
    vi.useRealTimers();
  });

  it("まだない段は 1 回だけ頼み、来たら version を上げる。来るまでは、いまある近い段で描く", async () => {
    const f = fakeFetch();
    make(f.fetch);
    expect(book.sourceFor(1, 7, 2)).toBeNull();
    expect(book.sourceFor(1, 7, 2)).toBeNull();
    expect(f.asked).toEqual(["get 7/2"]);
    await f.arrive(7, 2, 100);
    expect(book.version).toBe(1);
    const found = book.sourceFor(1, 7, 2)!;
    expect(found.framesPerPeak).toBe(4096);
    expect(found.source.peaks.count).toBe(100);
    // 拡大した: 段 0 を頼み、来るまでは段 2 のまま。
    const meanwhile = book.sourceFor(1, 7, 0)!;
    expect(meanwhile.source).toBe(found.source);
    expect(f.asked).toEqual(["get 7/2", "get 7/0"]);
    await f.arrive(7, 0, 1600);
    expect(book.version).toBe(2);
    expect(book.sourceFor(1, 7, 0)!.framesPerPeak).toBe(256);
    expect(book.sourceFor(1, 7, 0)!.source.id).not.toBe(found.source.id);
  });

  it("ふえていく音: 使われている間、1 秒に 10 回ほど続きを取りにいく。番号は同じまま、印（stamp）だけ進む", async () => {
    const f = fakeFetch();
    make(f.fetch);
    await f.arrive(9, 0, 50, true);
    const first = book.sourceFor(1, 9, 0)!;
    const id = first.source.id;
    expect(first.source.stamp).toBe(0);
    vi.advanceTimersByTime(WAVE_POLL_MS);
    expect(f.asked).toEqual(["more 9/0"]);
    await f.arrive(9, 0, 70, true);
    // 曲のクリップが使った物は、ふえたら描きなおす。
    expect(book.version).toBe(1);
    const grown = book.sourceFor(1, 9, 0)!;
    expect([grown.source.id, grown.source.stamp, grown.source.peaks.count]).toEqual([id, 1, 70]);
    expect(grown.source).toBe(first.source);
    // 録っているクリップの先（毎コマ読む）だけが使った物は、ふえても version を上げない。
    book.sourceFor(1, 9, 0, true);
    now += WAVE_POLL_MS;
    vi.advanceTimersByTime(WAVE_POLL_MS);
    book.sourceFor(1, 9, 0, true);
    now += WAVE_POLL_MS;
    vi.advanceTimersByTime(WAVE_POLL_MS);
    await f.arrive(9, 0, 90, true);
    expect(book.version).toBe(1);
    expect(book.sourceFor(1, 9, 0, true)!.source.stamp).toBe(2);
    // 使われなくなったら、取りにいかない。
    const before = f.asked.length;
    now += 5000;
    vi.advanceTimersByTime(10 * WAVE_POLL_MS);
    expect(f.asked.length).toBe(before);
    // ふえおわった。
    await f.arrive(9, 0, 95, false);
    expect(book.sourceFor(1, 9, 0)!.source.peaks.growing).toBe(false);
  });

  it("ほかの曲の同じ番号の音は、別の物（番号は曲ごとに 0 から）", async () => {
    const f = fakeFetch();
    make(f.fetch);
    await f.arrive(3, 1, 40);
    const a = book.sourceFor(1, 3, 1)!.source;
    expect(book.sourceFor(1, 3, 1)!.source.id).toBe(a.id);
    const b = book.sourceFor(2, 3, 1)!.source;
    expect(b.id).not.toBe(a.id);
  });

  it("まだできていないピーク（読みこみが始まったばかり）は、間をあけて頼みなおす: 0.5 秒、1 秒、2 秒…", async () => {
    const f = fakeFetch();
    make(f.fetch);
    expect(book.sourceFor(1, 5, 0)).toBeNull();
    await f.fail(5, 0);
    // 待っている間は頼まない。
    now = 300;
    vi.advanceTimersByTime(300);
    book.sourceFor(1, 5, 0);
    expect(f.asked).toEqual(["get 5/0"]);
    // 0.5 秒たったら、描く所を回して、もう一度。
    now = 600;
    vi.advanceTimersByTime(300);
    expect(book.version).toBe(1);
    book.sourceFor(1, 5, 0);
    expect(f.asked).toEqual(["get 5/0", "get 5/0"]);
    await f.fail(5, 0);
    now = 1200;
    vi.advanceTimersByTime(600);
    expect(book.version).toBe(1);
    now = 1700;
    vi.advanceTimersByTime(500);
    expect(book.version).toBe(2);
    book.sourceFor(1, 5, 0);
    await f.arrive(5, 0, 10);
    expect(book.sourceFor(1, 5, 0)!.source.peaks.count).toBe(10);
  });

  it("ためる上限は橋のまま: 橋が手ばなしたピークと、ほかの曲の物は、帳面も手ばなす", async () => {
    const f = fakeFetch();
    make(f.fetch);
    await f.arrive(1, 0, 10);
    await f.arrive(2, 0, 10);
    book.sourceFor(1, 1, 0);
    book.sourceFor(1, 2, 0);
    expect(book.held()).toBe(2);
    f.have.delete(f.key(2, 0));
    book.trim(1);
    expect(book.held()).toBe(1);
    book.trim(2);
    expect(book.held()).toBe(0);
  });
});

// ---- Arrange を mount して ----

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => 1000 }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

describe("Arrange の歌のクリップ", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  const slots = new Map<HTMLElement, GlSlot>();
  let stopWatch: () => void = () => {};
  beforeEach(() => {
    app.reset();
    audioDrag.now = null;
    const peaks = new PeakStore(async (audio, level, first, count) => fakePeaks(FAKE_AUDIO.get(audio)!, level, first, count));
    store.current = { get: (a, l) => peaks.get(a, l), more: (a, l) => peaks.more(a, l), peek: (a, l) => peaks.peek(a, l) };
    target = document.createElement("div");
    document.body.append(target);
    slots.clear();
    stopWatch = watchSlots({
      attach: (node, slot) => slots.set(node, slot),
      update: (node, slot) => slots.set(node, slot),
      detach: (node) => slots.delete(node),
    });
  });
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    target.remove();
    stopWatch();
    audioDrag.now = null;
  });

  const scene = (name: string) => fakeScene(name)!.state as FakeState;
  async function show(state: AppState): Promise<void> {
    app.setState(state);
    mounted = mount(Arrange, { target });
    flushSync();
    // ピークが届くまで。
    for (let i = 0; i < 20; i++) {
      await new Promise((r) => setTimeout(r, 0));
      flushSync();
    }
  }
  type Rects = Extract<GlSlot, { kind: "rects" }>;
  const lane = () => slots.get(target.querySelector<HTMLElement>(".lanes > .fill")!) as Rects;
  const vocalClips = (s: AppState) => s.song.tracks.find((t) => t.kind.kind === "vocal")!.clips;

  it("audioEdit: クリップ 2 つに波形 2 つ。右は −3.0 dB の倍率とフェードイン、左はフェードアウト。同じファイルは 1 つのピーク", async () => {
    const s = scene("audioEdit");
    await show(s);
    const waves = lane().waves!;
    expect(waves).toHaveLength(2);
    const [left, right] = waves as [GlWave, GlWave];
    expect(left.source).toBe(right.source);
    expect(left.source.peaks.count).toBeGreaterThan(0);
    expect(left.gain).toBe(1);
    expect(right.gain).toBeCloseTo(gainFactor(-3), 12);
    expect([left.fadeIn, left.fadeOut > 0, left.line]).toEqual([0, true, "color.gray.10"]);
    expect([right.fadeIn > 0, right.fadeOut, right.line]).toEqual([true, 0, "color.gray.10"]);
    expect([left.color, left.mid]).toEqual(["color.gray.8", null]);
    // 右のクリップは、同じテイクの続き。
    expect(right.peak).toBeGreaterThan(left.peak);
    // 波形は、そのクリップの面（gray.4）のすぐ上、次のクリップの四角より下。
    const rects = lane().rects;
    expect(rects[left.after - 1]!.color).toBe("color.gray.4");
    expect(left.after).toBeLessThan(right.after);
    expect(right.after).toBeLessThanOrEqual(rects.length);
    // フェードの取っ手（クリップ 1 つに 2 つ）は、どのクリップと波形よりも上: 絵の最後の四角。
    const handles = rects.slice(-4);
    expect(handles.map((r) => [r.w, r.h, r.color])).toEqual(Array(4).fill([6, 6, "color.gray.10"]));
    expect(rects.length - 4).toBeGreaterThanOrEqual(right.after);
  });

  it("のび縮みの印（×1.25 ・ +2 ・ −1 ・ +30）は名前の札の中: 札の数はクリップの数のまま。音を作っている間は下に「音を作っています 34 %」", async () => {
    const labels = () => [...target.querySelectorAll<HTMLElement>(".labels > .clip-label")];
    const marks = () => labels().map((l) => [...l.querySelectorAll(".clip-mark")].map((m) => m.textContent));
    const clipCount = (s: AppState) => s.song.tracks.reduce((n, t) => n + t.clips.length, 0);
    const s = scene("stretchRendering");
    await show(s);
    expect(labels()).toHaveLength(clipCount(s));
    expect(marks().filter((m) => m.length > 0)).toEqual([["×1.25", "+2"]]);
    expect([...target.querySelectorAll('[data-test="audio-clip-rendering"]')].map((n) => n.textContent?.trim())).toEqual(["音を作っています 34 %"]);
    unmount(mounted!);
    mounted = null;
    await show(scene("stretchPitch"));
    expect(marks().filter((m) => m.length > 0)).toEqual([["−1 ・ +30"]]);
    unmount(mounted!);
    mounted = null;
    // だんだんの中: 地図ごと録った物は比 1（印なし）、96.00 で歌った物は頭で ×1.13。
    await show(scene("stretchRamp"));
    expect(marks().filter((m) => m.length > 0)).toEqual([["×1.13"]]);
  });

  it("歌のクリップの右クリックの一覧の下に「逆にする」「ノーマライズ」（あとの回の席、使えない）", async () => {
    await show(scene("stretch"));
    const label = [...target.querySelectorAll<HTMLElement>(".labels > .clip-label")].find((l) => l.textContent?.includes("歌 テイク 1"))!;
    const x = parseFloat(label.style.left) + 4;
    const y = parseFloat(label.style.top) + 4;
    target.querySelector(".lanes")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 2 }));
    flushSync();
    const entries = contextMenu.open!.entries;
    expect(entries.slice(-3).map((e) => (e.kind === "row" ? [e.label, e.usable] : "-"))).toEqual(["-", ["menu.reverse", false], ["menu.normalize", false]]);
    contextMenu.open = null;
  });

  it("audioTall: 160 の行のトラックは、まん中に gray.5 の線", async () => {
    await show(scene("audioTall"));
    expect(lane().waves!.map((w) => w.mid)).toEqual(["color.gray.5", "color.gray.5"]);
  });

  it("audioMissing: 見つからない音のクリップは、地 gray.2、上の線 gray.7、波形なし（ピークも頼まない）", async () => {
    const s = scene("audioMissing");
    const get = vi.spyOn(store.current!, "get");
    await show(s);
    expect(lane().waves).toEqual([]);
    expect(get).not.toHaveBeenCalled();
    const grounds = lane().rects.filter((r) => r.color === "color.gray.2");
    expect(grounds).toHaveLength(2);
    // えらんでいないほう（ハモり）の上の線は gray.7。
    const second = G.clipBox({ pxPerTick: 64 / BAR, scrollX: 0, width: 1000 }, dims, 0, vocalClips(s)[1]!.start, vocalClips(s)[1]!.end);
    expect(lane().rects.some((r) => r.color === "color.gray.7" && r.x === second.x0 && r.h === 2)).toBe(true);
  });

  it("audioLoading: 読みこんでいる途中は、ある所まで（ふえていくピーク）", async () => {
    const s = scene("audioLoading");
    await show(s);
    const waves = lane().waves!;
    expect(waves).toHaveLength(1);
    const w = waves[0]!;
    expect(w.source.peaks.growing).toBe(true);
    // ファイルは 180 秒、読めたのは 72 秒: ピークは 72 秒ぶんだけ。描いてよいのは、ファイルの終わりまで。
    expect(w.source.peaks.count).toBeGreaterThan(0);
    expect(w.source.peaks.count).toBeLessThan(w.end);
    expect(w.source.peaks.count / w.end).toBeCloseTo(72 / 180, 2);
  });

  it("音量の線 ・ 取っ手のドラッグ（audioDrag）: 大きな面はそのまま、そのクリップだけを小さな区画に、ドラッグの値で描く", async () => {
    const s = scene("audioEdit");
    await show(s);
    const big = lane();
    const right = vocalClips(s)[1]!;
    expect(target.querySelector('[data-test="audio-live"]')).toBeNull();
    audioDrag.now = { clip: right.id, gainDb: 6 };
    flushSync();
    const node = target.querySelector<HTMLElement>('[data-test="audio-live"]')!;
    expect(node.closest("[data-gl-region]")!.getAttribute("data-gl-region")).toBe("recordingClip");
    const live = slots.get(node) as Rects;
    expect(live.waves).toHaveLength(1);
    expect(live.waves![0]!.gain).toBeCloseTo(gainFactor(6), 12);
    // 箱はクリップの箱。中の絵は、箱の左上から。
    expect([node.style.left, node.style.width]).toEqual(["384px", "128px"]);
    expect(live.waves![0]!.x).toBe(1);
    expect(live.rects[0]).toMatchObject({ x: 0, y: 0, w: 128 });
    // 大きな面は、同じ絵のまま（作りなおしていない）。
    expect(lane()).toBe(big);
    // フェードの取っ手。
    audioDrag.now = { clip: right.id, fadeIn: TPQ };
    flushSync();
    const faded = slots.get(target.querySelector<HTMLElement>('[data-test="audio-live"]')!) as Rects;
    expect(faded.waves![0]!.fadeIn).toBe(16);
    expect(faded.waves![0]!.gain).toBeCloseTo(gainFactor(-3), 12);
    expect(lane()).toBe(big);
  });

  it("録っている歌のクリップの先（vocalRec）: 区画の上下をトラックの計の面とそろえる印（alignY）がつく", async () => {
    await show(scene("vocalRec"));
    const tail = slots.get(target.querySelector<HTMLElement>('[data-test="vocal-rec-tail"]')!) as Rects;
    expect(tail.alignY).toBe("trackMeters");
  });
});
