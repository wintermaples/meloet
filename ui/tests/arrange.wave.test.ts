// The arrange side of audio clip waveforms (spec 3 (Waveform, Clip gain, cutting / fades / overlaps, When an audio file is missing),
// mocks F7-AudioEdit / R11-Audio): level choice, waveform place, gain factor, fades, missing / loading, the peak book,
// and the picture when Arrange is mounted.
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
import { runFrameCallbacks, watchSlots, type GlSlot, type GlWave } from "../src/gl/slot";
import type { WaveSource } from "../src/gl/wave";
import { TempoMap } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

// Bridge peaks: from fake audio files (fakeAudio.ts), with the real `PeakStore`.
const store = vi.hoisted(() => ({ current: null as null | { get: (a: number, l: number) => Promise<unknown>; more: (a: number, l: number) => Promise<unknown>; peek: (a: number, l: number) => unknown } }));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  audioPeaks: (audio: number, level: number) => store.current!.get(audio, level),
  moreAudioPeaks: (audio: number, level: number) => store.current!.more(audio, level),
  peekAudioPeaks: (audio: number, level: number) => store.current!.peek(audio, level),
}));

const BAR = 4 * TPQ;
const steady = new TempoMap({ tempoBpm: 120, meterNum: 4, meterDen: 4, headRamp: false, tempoPoints: [], meterChanges: [] });
/** 120 → 60 at the start of bar 3. */
const slower = new TempoMap({ tempoBpm: 120, meterNum: 4, meterDen: 4, headRamp: false, tempoPoints: [{ tick: 2 * BAR, tempoBpm: 60, ramp: false }], meterChanges: [] });

// 1 bar is 64 px, row height 64 (scale 100 %).
const view: G.View = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
const dims: G.LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
const sizes = { pitch: 3, bar: 1, minH: 2 };
const look = W.waveLook(sizes, "color.gray.8", false);

const TAKE: FakeAudio = { id: 7, file: "歌.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 8 * 48000 };
const sourceOf = (count: number, growing = false): WaveSource => ({ id: 1, stamp: 0, peaks: { channels: 1, count, growing, values: new Int16Array(count * 2) } });
/** A clip that plays at the file's speed (「この音のテンポ」 is 「—」). Following clips get `fakeStretch` in `more`. */
const vocal = (start: number, end: number, more: Parameters<typeof fakeAudioClip>[1] = {}, takeOffset = 0): ClipView =>
  fakeVocalClip(1, "歌", start, end, fakeAudioClip(TAKE, { ...fakeStretch(null, 120), ...more }), takeOffset);

// docs/ is outside ui/, so read it with Node fs, as in screen.test.ts.
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const readMock = (board: string) => fs.readFileSync(new URL(`../../docs/design/mock/${board}.dc.html`, here), "utf8");

describe("sizes (mock F7-AudioEdit)", () => {
  it.each(Object.entries(mock).filter(([k]) => k.startsWith("mock.wave.")))("%s の値はモックに書いてあり、率をかけた値は整数", (name, tok) => {
    expect(readMock(tok.board)).toContain(`${tok.value}px`);
    for (const s of [100, 125] as const) expect(Number.isInteger(tokensAt(s, 1)[name as keyof typeof mock] as number)).toBe(true);
  });
});

describe("waveform place (which x is which part of the file)", () => {
  it("frameAtTick gives the same answer as fileFrameAt for clips (also with tempo changes and trimmed heads)", () => {
    const clip = vocal(2 * BAR, 6 * BAR, { firstFrame: 1200 }, -BAR);
    const shape = W.waveClipOf(clip)!;
    for (const map of [steady, slower]) {
      for (const tick of [0, BAR, 2 * BAR, 2.5 * BAR, 5 * BAR]) expect(W.frameAtTick(map, shape.timing, tick)).toBeCloseTo(fileFrameAt(map, clip, tick), 6);
    }
    // Keyboard clips have no waveform.
    expect(W.waveClipOf({ ...clip, audio: null })).toBeNull();
  });

  it("one rect per clip: from below the name row to above the bottom border. The left edge peak, and peaks per 1 px", () => {
    const clip = vocal(4 * BAR, 6 * BAR);
    const box = G.clipBox(view, dims, 0, clip.start, clip.end);
    const area = waveArea(dims, box);
    const out: GlWave[] = [];
    // Tempo 120, 48 kHz: 1 bar is 2 s = 96000 frames = 64 px. With level 1024, 1 px is 1500 / 1024 items.
    W.clipWaves(out, 3, steady, view, area, W.waveClipOf(clip)!, sourceOf(1000), 1024, look);
    expect(out).toHaveLength(1);
    const w = out[0]!;
    expect([w.x, w.y, w.w, w.h]).toEqual([257, 3 + 2 + 16, 126, 57 - 2 - 16 - 1]);
    expect(w.after).toBe(3);
    // The clip start (x 256) is frame 0. The rect starts 1 px to the right.
    expect(w.peak).toBeCloseTo(1500 / 1024, 9);
    expect(w.peaksPerPx).toBeCloseTo(1500 / 1024, 9);
    expect(w.first).toBe(0);
    expect(w.end).toBe(Math.ceil((8 * 48000) / 1024));
    expect([w.clipX0, w.clipX1]).toEqual([256, 384]);
    expect([w.pitch, w.bar, w.minH, w.color, w.mid]).toEqual([3, 1, 2, "color.gray.8", null]);
  });

  it("a trimmed clip, and a clip that continues the same take, start from the rest of the file", () => {
    const clip = vocal(6 * BAR, 8 * BAR, { firstFrame: 500 }, -2 * BAR);
    const out: GlWave[] = [];
    W.clipWaves(out, 0, steady, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), W.waveClipOf(clip)!, sourceOf(1000), 256, look);
    // Tick 0 of the take is 2 bars earlier: the clip starts at 500 + 2 × 96000 frames.
    expect(out[0]!.peak).toBeCloseTo((500 + 2 * 96000 + 1500) / 256, 6);
  });

  it("nothing outside the visible width is drawn. After scrolling, still one rect per clip", () => {
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
    // A clip that is not visible.
    expect(one({ ...view, scrollX: 400 * 64 + 10 })).toEqual([]);
  });

  it("a clip with a tempo change inside is split into straight parts (at every cut, frames follow the map)", () => {
    const clip = vocal(0, 6 * BAR);
    const out: GlWave[] = [];
    W.clipWaves(out, 0, slower, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), W.waveClipOf(clip)!, sourceOf(4000), 256, look);
    expect(out.length).toBeGreaterThan(1);
    expect(out.length).toBeLessThanOrEqual(64);
    const shape = W.waveClipOf(clip)!;
    for (const w of out) {
      // At any px in the rect, the straight frame and the map frame differ by less than 1 px.
      for (const part of [0, 0.3, 0.7, 1]) {
        const x = w.x + w.w * part;
        const drawn = (w.peak + (x - w.x) * w.peaksPerPx) * 256;
        const real = W.frameAtTick(slower, shape.timing, G.xToTick(view, x));
        expect(Math.abs(drawn - real)).toBeLessThan(3000);
      }
    }
    // Neighbors join with no gap.
    for (let i = 1; i < out.length; i++) expect(out[i]!.x).toBeCloseTo(out[i - 1]!.x + out[i - 1]!.w, 9);
    // From bar 3 the speed is half: frames per 1 px are not half but double (twice the seconds in the same px).
    expect(out.at(-1)!.peaksPerPx).toBeCloseTo(2 * out[0]!.peaksPerPx, 6);
  });

  it("a clip that follows the song tempo is placed by the 「この音のテンポ」 map (same tick, same frame, even when the song tempo changes)", () => {
    // A song sung at 96.00. Whether the song is 120, or 60 from bar 3, 1 bar (4 beats) is 2.5 s = 120000 frames.
    const clip = vocal(0, 6 * BAR, fakeStretch(steadyTempo(96), 120));
    const shape = W.waveClipOf(clip)!;
    expect(shape.timing.source).not.toBeNull();
    // The same SourceTempoView gives the same map (made only once per clip).
    expect(W.waveClipOf(clip)!.timing.source).toBe(shape.timing.source);
    for (const map of [steady, slower]) {
      for (const tick of [0, BAR, 2.5 * BAR, 5 * BAR]) {
        expect(W.frameAtTick(map, shape.timing, tick)).toBeCloseTo((tick / BAR) * 120000, 6);
        expect(fileFrameAt(map, clip, tick)).toBeCloseTo((tick / BAR) * 120000, 6);
      }
    }
    // A following clip is not split where the song tempo changes (frames are straight in ticks).
    const out: GlWave[] = [];
    W.clipWaves(out, 0, slower, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), shape, sourceOf(4000), 256, look);
    expect(out.length).toBe(1);
    expect(out[0]!.peaksPerPx).toBeCloseTo(120000 / 64 / 256, 9);
  });

  it("straightParts: no split when straight. Ramps are split in halves, up to 64", () => {
    expect(W.straightParts((t) => 3 * t + 1, 0, 100, 0.01)).toEqual([0, 100]);
    const bent = W.straightParts((t) => t * t, 0, 100, 1);
    expect(bent.length).toBeGreaterThan(3);
    expect(bent.length).toBeLessThanOrEqual(65);
    expect(bent[0]).toBe(0);
    expect(bent.at(-1)).toBe(100);
    expect(W.straightParts((t) => t, 5, 5, 1)).toEqual([5]);
  });

  it("a clip being recorded: only inside [from, to]. Frame 0 is the start of recording, the length is not fixed yet", () => {
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

describe("level choice", () => {
  it("the coarsest level that is finer than one bar (3 px)", () => {
    // 100 frames per 1 px → 300 frames per bar → level 256.
    expect(W.waveLevel(100, 3)).toBe(0);
    expect(W.waveLevel(1500, 3)).toBe(2);
    expect(W.waveLevel(1e7, 3)).toBe(PEAK_LEVELS.length - 1);
    // Zoomed in past the finest level, it stays at that level (the same peaks are stretched).
    expect(W.waveLevel(2, 3)).toBe(0);
  });

  it("frames per 1 px are counted at the tempo of the visible part", () => {
    const clip = vocal(0, 6 * BAR);
    const t = W.waveClipOf(clip)!.timing;
    expect(W.framesPerPx(steady, view, t, 0, 6 * BAR)).toBeCloseTo(1500, 6);
    // Only bar 3 and later is visible: half speed, so frames per 1 px are doubled.
    expect(W.framesPerPx(slower, { ...view, scrollX: 3 * 64 }, t, 0, 6 * BAR)).toBeCloseTo(3000, 6);
    // A clip that is not visible still gets a number.
    expect(W.framesPerPx(steady, { ...view, scrollX: 1e6 }, t, 0, 6 * BAR)).toBeCloseTo(1500, 6);
  });
});

describe("gain factor and fades", () => {
  it("clip gain is passed as a factor (same texture). During a drag, the audioDrag value", () => {
    const clip = vocal(0, 2 * BAR, { gainDb: -6 });
    expect(W.waveClipOf(clip)!.gain).toBeCloseTo(gainFactor(-6), 12);
    expect(W.waveClipOf(clip, { clip: 1, gainDb: 6 })!.gain).toBeCloseTo(gainFactor(6), 12);
    // A drag on another clip has no effect.
    expect(W.waveClipOf(clip, { clip: 2, gainDb: 6 })!.gain).toBeCloseTo(gainFactor(-6), 12);
    // Out of range is clamped to −60 to +12.
    expect(W.waveClipOf(clip, { clip: 1, gainDb: 40 })!.gain).toBeCloseTo(gainFactor(12), 12);
    // While many lines move together (39-B): a moved clip gets its own gain + byDb (stops at −60 to +12).
    expect(W.waveClipOf(clip, { clip: 2, gainDb: 0, shift: { clips: [1], byDb: 3 } })!.gain).toBeCloseTo(gainFactor(-3), 12);
    expect(W.waveClipOf(clip, { clip: 2, gainDb: 0, shift: { clips: [1], byDb: -80 } })!.gain).toBeCloseTo(gainFactor(-60), 12);
    expect(W.waveClipOf(clip, { clip: 2, gainDb: 0, shift: { clips: [3], byDb: 3 } })!.gain).toBeCloseTo(gainFactor(-6), 12);
  });

  it("fades are passed in px, and the line (gray.10) only for clips with fades. Also the length during a drag", () => {
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

  it("the form passed to the shader (clip edges and px lengths) gives the same size as clipFadeGain in fade.ts", () => {
    const clip = vocal(4 * BAR, 6 * BAR, { fadeIn: TPQ, fadeOut: 2 * TPQ });
    const out: GlWave[] = [];
    W.clipWaves(out, 0, steady, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), W.waveClipOf(clip)!, sourceOf(1000), 1024, look);
    const w = out[0]!;
    // The shader's fadeAt (renderer.ts), with the same formula.
    const rise = (t: number) => (t >= 1 ? 1 : t > 0 ? Math.sin((t * Math.PI) / 2) : 0);
    const shader = (x: number) => (w.fadeIn > 0 ? rise((x - w.clipX0) / w.fadeIn) : 1) * (w.fadeOut > 0 ? rise((w.clipX1 - x) / w.fadeOut) : 1);
    for (const x of [257, 260, 272, 300, 352, 370, 383]) {
      const ticks = G.xToTick(view, x) - clip.start;
      expect(shader(x)).toBeCloseTo(clipFadeGain(ticks, clip.end - clip.start, TPQ, 2 * TPQ), 9);
    }
  });

  it("a 160 row has a gray.5 center line. The bar color is the color of the lines in the clip", () => {
    expect(W.waveLook(sizes, "color.gray.8", true).mid).toBe("color.gray.5");
    expect(W.waveLook(sizes, "color.gray.7", false)).toEqual({ ...sizes, color: "color.gray.7", mid: null });
  });

  it("shiftWaves: counts again from the top left of the small slot box", () => {
    const clip = vocal(4 * BAR, 6 * BAR);
    const out: GlWave[] = [];
    W.clipWaves(out, 0, steady, view, waveArea(dims, G.clipBox(view, dims, 0, clip.start, clip.end)), W.waveClipOf(clip)!, sourceOf(1000), 1024, look);
    W.shiftWaves(out, -256, -3);
    expect([out[0]!.x, out[0]!.y, out[0]!.clipX0, out[0]!.clipX1]).toEqual([1, 18, 0, 128]);
  });
});

// ---- the peak book ----

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

describe("peak book (WaveBook)", () => {
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

  it("a missing level is asked for once, and version goes up when it comes. Until then, draw with the nearest level there is", async () => {
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
    // Zoomed in: ask for level 0, keep level 2 until it comes.
    const meanwhile = book.sourceFor(1, 7, 0)!;
    expect(meanwhile.source).toBe(found.source);
    expect(f.asked).toEqual(["get 7/2", "get 7/0"]);
    await f.arrive(7, 0, 1600);
    expect(book.version).toBe(2);
    expect(book.sourceFor(1, 7, 0)!.framesPerPeak).toBe(256);
    expect(book.sourceFor(1, 7, 0)!.source.id).not.toBe(found.source.id);
  });

  it("growing audio: while used, fetch the rest about 10 times a second. The number stays, only the stamp moves", async () => {
    const f = fakeFetch();
    make(f.fetch);
    await f.arrive(9, 0, 50, true);
    const first = book.sourceFor(1, 9, 0)!;
    const id = first.source.id;
    expect(first.source.stamp).toBe(0);
    vi.advanceTimersByTime(WAVE_POLL_MS);
    expect(f.asked).toEqual(["more 9/0"]);
    await f.arrive(9, 0, 70, true);
    // What song clips use is drawn again when it grows.
    expect(book.version).toBe(1);
    const grown = book.sourceFor(1, 9, 0)!;
    expect([grown.source.id, grown.source.stamp, grown.source.peaks.count]).toEqual([id, 1, 70]);
    expect(grown.source).toBe(first.source);
    // What only the head of the clip being recorded uses (read every frame) does not raise version when it grows.
    book.sourceFor(1, 9, 0, true);
    now += WAVE_POLL_MS;
    vi.advanceTimersByTime(WAVE_POLL_MS);
    book.sourceFor(1, 9, 0, true);
    now += WAVE_POLL_MS;
    vi.advanceTimersByTime(WAVE_POLL_MS);
    await f.arrive(9, 0, 90, true);
    expect(book.version).toBe(1);
    expect(book.sourceFor(1, 9, 0, true)!.source.stamp).toBe(2);
    // When no longer used, it is not fetched.
    const before = f.asked.length;
    now += 5000;
    vi.advanceTimersByTime(10 * WAVE_POLL_MS);
    expect(f.asked.length).toBe(before);
    // Done growing.
    await f.arrive(9, 0, 95, false);
    expect(book.sourceFor(1, 9, 0)!.source.peaks.growing).toBe(false);
  });

  it("audio with the same number in another song is a different thing (numbers start at 0 per song)", async () => {
    const f = fakeFetch();
    make(f.fetch);
    await f.arrive(3, 1, 40);
    const a = book.sourceFor(1, 3, 1)!.source;
    expect(book.sourceFor(1, 3, 1)!.source.id).toBe(a.id);
    const b = book.sourceFor(2, 3, 1)!.source;
    expect(b.id).not.toBe(a.id);
  });

  it("peaks not ready yet (loading just started) are asked for again with gaps: 0.5 s, 1 s, 2 s…", async () => {
    const f = fakeFetch();
    make(f.fetch);
    expect(book.sourceFor(1, 5, 0)).toBeNull();
    await f.fail(5, 0);
    // No asking while waiting.
    now = 300;
    vi.advanceTimersByTime(300);
    book.sourceFor(1, 5, 0);
    expect(f.asked).toEqual(["get 5/0"]);
    // After 0.5 s, run the draw and ask again.
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

  it("the keep limit is the bridge's: peaks the bridge let go, and those of other songs, the book lets go too", async () => {
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

  it("another song does not wait on the gaps of the last one (audio numbers start again per song). The same song keeps them", async () => {
    const f = fakeFetch();
    make(f.fetch);
    book.trim(1);
    book.sourceFor(1, 5, 0);
    await f.fail(5, 0);
    book.trim(1);
    book.sourceFor(1, 5, 0);
    expect(f.asked).toEqual(["get 5/0"]);
    book.trim(2);
    book.sourceFor(2, 5, 0);
    expect(f.asked).toEqual(["get 5/0", "get 5/0"]);
  });
});

// ---- with Arrange mounted ----

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => 1000 }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

describe("audio clips in Arrange", () => {
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
    // Until the peaks arrive.
    for (let i = 0; i < 20; i++) {
      await new Promise((r) => setTimeout(r, 0));
      flushSync();
    }
  }
  type Rects = Extract<GlSlot, { kind: "rects" }>;
  const lane = () => slots.get(target.querySelector<HTMLElement>(".lanes > .fill")!) as Rects;
  const vocalClips = (s: AppState) => s.song.tracks.find((t) => t.kind.kind === "vocal")!.clips;

  it("audioEdit: 2 clips, 2 waveforms. The right one has a −3.0 dB factor and a fade in, the left a fade out. One file gives one peak", async () => {
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
    // The right clip continues the same take.
    expect(right.peak).toBeGreaterThan(left.peak);
    // The waveform is just above the clip face (gray.4), below the rect of the next clip.
    const rects = lane().rects;
    expect(rects[left.after - 1]!.color).toBe("color.gray.4");
    expect(left.after).toBeLessThan(right.after);
    expect(right.after).toBeLessThanOrEqual(rects.length);
    // Fade handles (2 per clip) are above every clip and waveform: the last rects in the picture.
    const handles = rects.slice(-4);
    expect(handles.map((r) => [r.w, r.h, r.color])).toEqual(Array(4).fill([6, 6, "color.gray.10"]));
    expect(rects.length - 4).toBeGreaterThanOrEqual(right.after);
  });

  it("stretch marks (×1.25, +2, −1, +30) are in the name tag: as many tags as clips. While the sound is made, 「音を作っています 34 %」 below", async () => {
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
    // Inside a ramp: one recorded with the map has ratio 1 (no mark), one sung at 96.00 gets ×1.13 at the start.
    await show(scene("stretchRamp"));
    expect(marks().filter((m) => m.length > 0)).toEqual([["×1.13"]]);
  });

  it("under the audio clip context menu, 「逆にする」「ノーマライズ」", async () => {
    await show(scene("stretch"));
    const label = [...target.querySelectorAll<HTMLElement>(".labels > .clip-label")].find((l) => l.textContent?.includes("歌 テイク 1"))!;
    const x = parseFloat(label.style.left) + 4;
    const y = parseFloat(label.style.top) + 4;
    target.querySelector(".lanes")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 2 }));
    flushSync();
    const entries = contextMenu.open!.entries;
    expect(entries.slice(-3).map((e) => (e.kind === "row" ? [e.label, e.usable] : "-"))).toEqual(["-", ["menu.reverse", true], ["menu.normalize", true]]);
    contextMenu.open = null;
  });

  it("audioTall: a track with a 160 row has a gray.5 center line", async () => {
    await show(scene("audioTall"));
    expect(lane().waves!.map((w) => w.mid)).toEqual(["color.gray.5", "color.gray.5"]);
  });

  it("audioMissing: a clip with missing audio has background gray.2, top line gray.7, no waveform (and asks for no peaks)", async () => {
    const s = scene("audioMissing");
    const get = vi.spyOn(store.current!, "get");
    await show(s);
    expect(lane().waves).toEqual([]);
    expect(get).not.toHaveBeenCalled();
    const grounds = lane().rects.filter((r) => r.color === "color.gray.2");
    expect(grounds).toHaveLength(2);
    // The top line of the one not chosen (the harmony) is gray.7.
    const second = G.clipBox({ pxPerTick: 64 / BAR, scrollX: 0, width: 1000 }, dims, 0, vocalClips(s)[1]!.start, vocalClips(s)[1]!.end);
    // Rects of the large surface start from the window's left edge (drawn shiftX to the left).
    const shift = lane().shiftX ?? 0;
    expect(lane().rects.some((r) => r.color === "color.gray.7" && Math.abs(r.x - shift - second.x0) < 1e-9 && r.h === 2)).toBe(true);
  });

  it("audioLoading: while loading, up to where it has read (growing peaks)", async () => {
    const s = scene("audioLoading");
    await show(s);
    const waves = lane().waves!;
    expect(waves).toHaveLength(1);
    const w = waves[0]!;
    expect(w.source.peaks.growing).toBe(true);
    // The file is 180 s, 72 s has been read: peaks for 72 s only. It may draw up to the end of the file.
    expect(w.source.peaks.count).toBeGreaterThan(0);
    expect(w.source.peaks.count).toBeLessThan(w.end);
    expect(w.source.peaks.count / w.end).toBeCloseTo(72 / 180, 2);
  });

  it("dragging the gain line or a handle (audioDrag): the large surface stays, only that clip is drawn in a small slot with the drag value", async () => {
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
    // The box is the clip box. The picture inside starts from the box's top left.
    expect([node.style.left, node.style.width]).toEqual(["384px", "128px"]);
    expect(live.waves![0]!.x).toBe(1);
    expect(live.rects[0]).toMatchObject({ x: 0, y: 0, w: 128 });
    // The large surface keeps the same picture (not made again).
    expect(lane()).toBe(big);
    // Fade handles.
    audioDrag.now = { clip: right.id, fadeIn: TPQ };
    flushSync();
    const faded = slots.get(target.querySelector<HTMLElement>('[data-test="audio-live"]')!) as Rects;
    expect(faded.waves![0]!.fadeIn).toBe(16);
    expect(faded.waves![0]!.gain).toBeCloseTo(gainFactor(-3), 12);
    expect(lane()).toBe(big);
  });

  it("while many lines move together (39-B): moved clips are drawn in one slot around them, with the shifted gain. The large surface stays", async () => {
    const s = scene("multiClip");
    await show(s);
    const big = lane();
    const ids = s.params.clips!.clips;
    const all = s.song.tracks.flatMap((t) => t.clips);
    const [grabbed, ...others] = ids.map((id) => all.find((c) => c.id === id)!);
    audioDrag.now = { clip: grabbed!.id, gainDb: grabbed!.audio!.gainDb + 2, shift: { clips: others.map((c) => c.id), byDb: 2 } };
    flushSync();
    const nodes = [...target.querySelectorAll<HTMLElement>('[data-test="audio-live"]')];
    expect(nodes).toHaveLength(1);
    const live = slots.get(nodes[0]!) as Rects;
    expect(live.waves!.map((w) => w.gain).sort()).toEqual([grabbed!, ...others].map((c) => gainFactor(c.audio!.gainDb + 2)).sort());
    // The slot is a box around the 3 clips (across 2 rows).
    const rowH = 64;
    expect(parseFloat(nodes[0]!.style.height)).toBeGreaterThan(rowH);
    expect(lane()).toBe(big);
  });

  it("the head of an audio clip being recorded (vocalRec): gets the mark (alignY) that lines up the slot top and bottom with the track meter face", async () => {
    await show(scene("vocalRec"));
    const tail = slots.get(target.querySelector<HTMLElement>('[data-test="vocal-rec-tail"]')!) as Rects;
    expect(tail.alignY).toBe("trackMeters");
  });

  it("the name of a voice being recorded is measured again when its take number changes (a later lap): cut at the clip's right end", async () => {
    const hadW = Object.getOwnPropertyDescriptor(proto, "offsetWidth");
    let widthOf = (_text: string) => 1e6;
    Object.defineProperty(proto, "offsetWidth", {
      configurable: true,
      get(this: HTMLElement) {
        return this.dataset.test === "vocal-rec-label" ? widthOf((this.textContent ?? "").trim()) : 0;
      },
    });
    try {
      const s = scene("vocalRec");
      const rec = s.transport.recordingVocals[0]!;
      app.setTelemetry(app.telemetryWith({ position: { tick: rec.from + 2 * BAR, ticksPerUs: 0, state: "recording", countInBeatsLeft: 0, lap: 0 } }));
      const label = () => target.querySelector<HTMLElement>('[data-test="vocal-rec-label"]')!;
      const frame = () => {
        runFrameCallbacks();
        flushSync();
      };
      // How wide the clip is: a very long name is cut there.
      await show(s);
      frame();
      const clipPx = Number(/calc\(100% - ([\d.]+)px\)/.exec(label().style.clipPath)![1]);
      unmount(mounted!);
      mounted = null;
      // Take 1 fits in the clip; take 2 is a little wider than the clip.
      widthOf = (text) => (text.endsWith("2") ? clipPx + 4 : clipPx - 4);
      await show(s);
      frame();
      expect(label().style.clipPath).toBe("");
      app.setState({ ...s, transport: { ...s.transport, recordingVocals: [{ ...rec, take: 2 }] } });
      flushSync();
      frame();
      expect(label().style.clipPath).toContain(`calc(100% - ${clipPx}px)`);
    } finally {
      if (hadW) Object.defineProperty(proto, "offsetWidth", hadW);
      else delete (proto as unknown as Record<string, unknown>).offsetWidth;
    }
  });
});
