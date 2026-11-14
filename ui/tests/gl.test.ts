// WebGL の面の、純粋な部分: dB → 長さ、鍵の位置、色の読みかえ、四角の並び、計測の記録。
import { describe, expect, it } from "vitest";
import { colors } from "../src/design/tokens";
import { Batch } from "../src/gl/batch";
import { hexToRgb, rgb } from "../src/gl/color";
import { CLIP_HOLD_MS, DB_FLOOR, DB_SCALE, EVENT_WINS_US, FALL_DB_PER_S, KEYBOARD, METER_HOT_FROM } from "../src/gl/dims";
import { HeldKeys, isBlack, keyboardLayout } from "../src/gl/keyboard";
import { dbToLength, fall, peakToDb } from "../src/gl/level";
import { Levels } from "../src/gl/levels";
import { KeyToScreen, REPORT_NAME } from "../src/gl/measure";
import { paintArrangeGrid, paintKeyboard, paintMeter, paintOverview, paintVelocity } from "../src/gl/paint";
import type { Telemetry } from "../src/bridge";

interface NodeFs {
  readFileSync(path: URL, encoding: "utf8"): string;
}
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): NodeFs } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const mockHtml = (board: string) => fs.readFileSync(new URL(`../../docs/design/mock/${board}.dc.html`, here), "utf8");

const hex = (name: keyof typeof colors) => [...hexToRgb(colors[name])];
const batch = (w = 4000, h = 4000) => {
  const b = new Batch();
  b.clip(0, 0, w, h);
  return b;
};
/** 四角のうち、その色の物だけ（[x0, y0, x1, y1]）。 */
const ofColor = (b: Batch, name: keyof typeof colors) =>
  b
    .rects()
    .filter((r) => r.slice(4).every((v, i) => Math.abs(v - hex(name)[i]!) < 1e-6))
    .map((r) => r.slice(0, 4));

describe("色の読みかえ", () => {
  it("16 進数を 0〜1 の 3 つの数にする", () => {
    expect(hexToRgb("#FFB000")).toEqual([1, 176 / 255, 0]);
    expect(hexToRgb("#0a0A0a")).toEqual([10 / 255, 10 / 255, 10 / 255]);
  });
  it("形のちがう文字は断る", () => {
    for (const bad of ["#FFF", "FFB000", "#FFB0000", "rgb(0,0,0)"]) expect(() => hexToRgb(bad)).toThrow();
  });
  it("tokens の色は全部読める。名前で引くと同じ値", () => {
    for (const name of Object.keys(colors) as (keyof typeof colors)[]) {
      expect(rgb(name)).toEqual(hexToRgb(colors[name]));
    }
  });
});

describe("dB → 長さ", () => {
  it("目盛は A-Mixer のモックの位置と同じ", () => {
    const html = mockHtml("A-Mixer");
    const marks = [...html.matchAll(/<span class="abs" style="right:0;top:(\d+)%?">([^<]+)<\/span>/g)];
    const fromMock = new Map(marks.map((m) => [Number(m[2]!.replace("−", "-")), 1 - Number(m[1]) / 100]));
    expect(fromMock.size).toBe(DB_SCALE.length);
    for (const [db, length] of DB_SCALE) expect(fromMock.get(db)).toBeCloseTo(length, 10);
    expect(html).toContain('<span class="abs" style="right:0;bottom:0">−∞</span>');
  });
  it("目盛の上では目盛の値、間は直線", () => {
    for (const [db, length] of DB_SCALE) expect(dbToLength(db)).toBeCloseTo(length, 10);
    expect(dbToLength(3)).toBeCloseTo(0.91, 10);
    expect(dbToLength(-3)).toBeCloseTo(0.72, 10);
    expect(dbToLength(-18)).toBeCloseTo(0.35, 10);
    expect(dbToLength((-24 + DB_FLOOR) / 2)).toBeCloseTo(0.13, 10);
  });
  it("はしでは止まる", () => {
    expect(dbToLength(6.1)).toBe(1);
    expect(dbToLength(100)).toBe(1);
    expect(dbToLength(DB_FLOOR)).toBe(0);
    expect(dbToLength(-200)).toBe(0);
    expect(dbToLength(-Infinity)).toBe(0);
  });
  it("大きい音ほど長い", () => {
    let last = -1;
    for (let db = DB_FLOOR; db <= 6; db += 0.25) {
      const length = dbToLength(db);
      expect(length).toBeGreaterThanOrEqual(last);
      last = length;
    }
  });
  it("最大値を dB にする", () => {
    expect(peakToDb(1)).toBe(0);
    expect(peakToDb(0.5)).toBeCloseTo(-6.0206, 3);
    expect(peakToDb(0)).toBe(-Infinity);
  });
});

describe("メーターの落ちかた", () => {
  it("上がるときはすぐ", () => {
    expect(fall(-30, -6, 0)).toBe(-6);
    expect(fall(-Infinity, -6, 16)).toBe(-6);
  });
  it("下がるときは決めた速さで", () => {
    expect(fall(0, -Infinity, 1000)).toBeCloseTo(-FALL_DB_PER_S, 10);
    expect(fall(0, -Infinity, 500)).toBeCloseTo(-FALL_DB_PER_S / 2, 10);
  });
  it("行き先より下には行かない。底についたら無音", () => {
    expect(fall(0, -3, 1000)).toBe(-3);
    expect(fall(DB_FLOOR + 1, -Infinity, 1000)).toBe(-Infinity);
    expect(fall(-Infinity, -Infinity, 1000)).toBe(-Infinity);
  });
});

const telemetry = (over: Partial<Telemetry> = {}): Telemetry => ({
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
  trackPeaks: new Float32Array(0),
  ...over,
});

describe("画面のレベル", () => {
  it("上がってから、時間で落ちて、行き先で止まる", () => {
    const levels = new Levels();
    levels.onTelemetry(telemetry({ masterPeak: [1, 0.5], trackPeaks: new Float32Array([0.25, 0]) }), 1000);
    expect(levels.bar(0, 1000).length).toBeCloseTo(0.82, 6);
    expect(levels.bar(1, 1000).length).toBeCloseTo(dbToLength(peakToDb(0.5)), 6);
    expect(levels.bar(2, 1000).length).toBeCloseTo(dbToLength(peakToDb(0.25)), 6);
    expect(levels.bar(3, 1000).length).toBe(0);
    expect(levels.bar(9, 1000).length).toBe(0);

    levels.onTelemetry(telemetry({ masterPeak: [0.5, 0.5] }), 1000);
    expect(levels.bar(0, 1000).length).toBeCloseTo(0.82, 6);
    expect(levels.advance(1100)).toBe(true);
    expect(levels.bar(0, 1100).length).toBeCloseTo(dbToLength(-FALL_DB_PER_S / 10), 6);
    expect(levels.advance(3000)).toBe(false);
    expect(levels.bar(0, 3000).length).toBeCloseTo(dbToLength(peakToDb(0.5)), 6);
  });
  it("音割れの印は 2 秒", () => {
    const levels = new Levels();
    levels.onTelemetry(telemetry({ masterPeak: [1.2, 0.5], trackPeaks: new Float32Array([0, 1.01]) }), 500);
    expect(levels.bar(0, 500).clipped).toBe(true);
    expect(levels.bar(1, 500).clipped).toBe(false);
    expect(levels.bar(2, 500).clipped).toBe(false);
    expect(levels.bar(3, 500).clipped).toBe(true);
    expect(levels.nextClipEnd(500)).toBe(500 + CLIP_HOLD_MS);
    expect(levels.bar(0, 500 + CLIP_HOLD_MS - 1).clipped).toBe(true);
    expect(levels.bar(0, 500 + CLIP_HOLD_MS).clipped).toBe(false);
    expect(levels.nextClipEnd(500 + CLIP_HOLD_MS)).toBe(null);
  });
  it("マスターが割れた旗は、左右の両方に印", () => {
    const levels = new Levels();
    levels.onTelemetry(telemetry({ masterClipped: true, trackPeaks: new Float32Array([0.1, 0.1]) }), 0);
    expect([0, 1, 2, 3].map((c) => levels.bar(c, 1).clipped)).toEqual([true, true, false, false]);
  });
});

describe("鍵の位置", () => {
  it("49 鍵（C2〜C6）は白 29、黒 20", () => {
    const { whiteCount, keys } = keyboardLayout(36, 84);
    expect(keys.length).toBe(49);
    expect(whiteCount).toBe(29);
    expect(keys.filter((k) => k.black).length).toBe(20);
    expect(keys.filter((k) => !k.black).map((k) => k.index)).toEqual(Array.from({ length: 29 }, (_, i) => i));
  });
  it("黒鍵のならびは Opt-Meter のモックと同じ（白鍵 i の右に黒鍵があるか）", () => {
    const blk = [1, 1, 0, 1, 1, 1, 0];
    const fromMock = Array.from({ length: 29 }, (_, i) => i < 28 && blk[i % 7] === 1);
    expect(mockHtml("Opt-Meter")).toContain("black: i < 28 && blk[i % 7] === 1");
    const { keys } = keyboardLayout(36, 84);
    const lines = new Set(keys.filter((k) => k.black).map((k) => k.index));
    // 黒鍵は線 index（白鍵 index の左はし）に乗る。白鍵 i の右の線は i + 1。
    expect(Array.from({ length: 29 }, (_, i) => lines.has(i + 1))).toEqual(fromMock);
  });
  it("黒鍵は C#・D#・F#・G#・A#", () => {
    expect([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].filter(isBlack)).toEqual([1, 3, 6, 8, 10]);
    expect(isBlack(61)).toBe(true);
    expect(isBlack(60)).toBe(false);
  });
  it("黒鍵から始まる範囲や、白鍵のない範囲でもこわれない", () => {
    expect(keyboardLayout(61, 62)).toEqual({
      whiteCount: 1,
      keys: [
        { note: 61, black: true, index: 0 },
        { note: 62, black: false, index: 0 },
      ],
    });
    expect(keyboardLayout(61, 61).whiteCount).toBe(0);
    expect(keyboardLayout(10, 5).keys).toEqual([]);
  });
});

describe("押している鍵", () => {
  const bits = (...notes: number[]) => {
    const keys = new Uint32Array(4);
    for (const n of notes) keys[n >>> 5]! |= 1 << (n & 31);
    return keys;
  };
  it("Event が来たら、計器を待たずに変わる", () => {
    const held = new HeldKeys();
    expect(held.onEvent(60, true, 1000)).toBe(true);
    expect(held.isDown(60)).toBe(true);
    expect(held.onEvent(60, true, 1001)).toBe(false);
    expect(held.onEvent(60, false, 2000)).toBe(true);
    expect(held.isDown(60)).toBe(false);
  });
  it("計器だけでも変わる（Event を見ていない鍵）", () => {
    const held = new HeldKeys();
    expect(held.onTelemetry(bits(31, 32, 127), 5)).toBe(true);
    expect([31, 32, 127, 60].map((n) => held.isDown(n))).toEqual([true, true, true, false]);
    expect(held.onTelemetry(bits(32), 6)).toBe(true);
    expect([31, 32, 127].map((n) => held.isDown(n))).toEqual([false, true, false]);
    expect(held.onTelemetry(bits(32), 7)).toBe(false);
  });
  it("Event より古い計器、すぐ後の計器では消えない。十分あとの計器には従う", () => {
    const held = new HeldKeys();
    held.onEvent(60, true, 1_000_000);
    expect(held.onTelemetry(bits(), 999_000)).toBe(false);
    expect(held.onTelemetry(bits(), 1_000_000 + EVENT_WINS_US)).toBe(false);
    expect(held.isDown(60)).toBe(true);
    expect(held.onTelemetry(bits(), 1_000_001 + EVENT_WINS_US)).toBe(true);
    expect(held.isDown(60)).toBe(false);
  });
  it("範囲の外の番号は無視", () => {
    const held = new HeldKeys();
    expect(held.onEvent(128, true, 0)).toBe(false);
    expect(held.onEvent(-1, true, 0)).toBe(false);
  });
});

describe("四角の並び", () => {
  it("太い鍵盤: モック（F-Connect2）と同じ画素。幅 1 + 29 × 12、高さ 32", () => {
    const b = batch();
    const down = new Set([60, 61]);
    paintKeyboard(b, { x0: 100, y0: 10, x1: 100 + 1 + 29 * KEYBOARD.white.w, y1: 10 + KEYBOARD.white.h }, "full", 36, 84, (n) => down.has(n));

    expect(ofColor(b, "color.gray.6")).toEqual([[100, 10, 449, 42]]);
    const whites = ofColor(b, "color.gray.3");
    expect(whites.length).toBe(28);
    // 白鍵 i: 左の線の右どなりから、右の線の手前まで（11 画素）。上下の線の内がわ。
    expect(whites[0]).toEqual([101, 11, 112, 41]);
    expect(whites[1]).toEqual([113, 11, 124, 41]);
    // 押した白鍵（C4 = 白鍵 14）と黒鍵（C#4 = 線 15）は accent。
    expect(ofColor(b, "color.accent")).toEqual([
      [100 + 14 * 12 + 1, 11, 100 + 15 * 12, 41],
      [100 + 15 * 12 - 2, 11, 100 + 15 * 12 + 3, 28],
    ]);
    // 黒鍵: 線の上にまん中を合わせた幅 7、高さ 19。枠は 1 画素。
    const frames = ofColor(b, "color.gray.7");
    expect(frames.length).toBe(20);
    expect(frames[0]).toEqual([100 + 12 - 3, 10, 100 + 12 + 4, 29]);
    expect(ofColor(b, "color.gray.0")[0]).toEqual([100 + 12 - 2, 11, 100 + 12 + 3, 28]);
    expect(ofColor(b, "color.gray.0").length).toBe(19);
  });
  it("細い鍵盤: 白鍵の四角だけ（1 + 29 × 8、高さ 12）", () => {
    const b = batch();
    paintKeyboard(b, { x0: 0, y0: 0, x1: 233, y1: 12 }, "slim", 36, 84, (n) => n === 64);
    expect(b.rectCount).toBe(30);
    expect(ofColor(b, "color.gray.3")[28 - 1]).toEqual([225, 1, 232, 11]);
    expect(ofColor(b, "color.accent")).toEqual([[16 * 8 + 1, 1, 17 * 8, 11]]);
    expect(ofColor(b, "color.gray.7")).toEqual([]);
  });
  it("拡大しても線は 1 画素（150 % × 2 倍の画面 = 白鍵 36 画素）", () => {
    const b = batch();
    paintKeyboard(b, { x0: 0, y0: 0, x1: 1 + 29 * 36, y1: 96 }, "full", 36, 84, () => false);
    const whites = ofColor(b, "color.gray.3");
    expect(whites[0]).toEqual([1, 1, 36, 95]);
    expect(whites[1]).toEqual([37, 1, 72, 95]);
    const frame = ofColor(b, "color.gray.7")[0]!;
    const inner = ofColor(b, "color.gray.0")[0]!;
    expect(frame).toEqual([36 - 10, 0, 36 + 11, 57]);
    expect(inner).toEqual([frame[0]! + 1, 1, frame[2]! - 1, 56]);
  });
  it("ベロシティの棒: 87 は 68 %（モックと同じ）", () => {
    const b = batch();
    paintVelocity(b, { x0: 0, y0: 0, x1: 100, y1: 4 }, 87);
    expect(ofColor(b, "color.gray.4")).toEqual([[0, 0, 100, 4]]);
    expect(ofColor(b, "color.accent")).toEqual([[0, 0, 69, 4]]);
    const none = batch();
    paintVelocity(none, { x0: 0, y0: 0, x1: 100, y1: 4 }, 0);
    expect(none.rectCount).toBe(1);
  });
  it("メーター: 2 本、72 % までは ok、その上は hot、音割れは右はしに印", () => {
    const b = batch();
    paintMeter(b, { x0: 10, y0: 20, x1: 110, y1: 32 }, 4, 2, { length: 0.5, clipped: false }, { length: 0.9, clipped: true });
    expect(ofColor(b, "color.gray.0")).toEqual([
      [10, 20, 110, 24],
      [10, 28, 110, 32],
    ]);
    expect(ofColor(b, "color.meter.ok")).toEqual([
      [10, 20, 60, 24],
      [10, 28, 10 + 100 * METER_HOT_FROM, 32],
    ]);
    expect(ofColor(b, "color.meter.hot")).toEqual([[82, 28, 100, 32]]);
    expect(ofColor(b, "color.rec")).toEqual([[108, 28, 110, 32]]);
  });
  it("メーター: 空のときは地だけ", () => {
    const b = batch();
    const silent = { length: 0, clipped: false };
    paintMeter(b, { x0: 0, y0: 0, x1: 132, y1: 12 }, 4, 2, silent, silent);
    expect(b.rectCount).toBe(2);
  });
  it("小節の線: firstBarX から barPx ごと。箱の中だけ。端数は画素に合わせる", () => {
    const b = batch();
    b.clip(100, 0, 300, 50);
    paintArrangeGrid(b, { x0: 100, y0: 0, x1: 300, y1: 50 }, 0, 64);
    expect(ofColor(b, "color.gray.4")).toEqual([
      [100, 0, 101, 50],
      [164, 0, 165, 50],
      [228, 0, 229, 50],
      [292, 0, 293, 50],
    ]);
    const scrolled = batch();
    paintArrangeGrid(scrolled, { x0: 0, y0: 0, x1: 100, y1: 10 }, -70.5, 40.25);
    expect(ofColor(scrolled, "color.gray.4").map((r) => r[0])).toEqual([10, 50, 91]);
    const none = batch();
    paintArrangeGrid(none, { x0: 0, y0: 0, x1: 100, y1: 10 }, 0, 0);
    expect(none.rectCount).toBe(0);
  });
  it("ちぢんだアレンジ: 1 画素の枠だけ", () => {
    const b = batch();
    paintOverview(b, { x0: 0, y0: 0, x1: 256, y1: 39 }, 1);
    expect(ofColor(b, "color.accent")).toEqual([
      [0, 1, 256, 2],
      [0, 37, 256, 38],
      [0, 1, 1, 38],
      [255, 1, 256, 38],
    ]);
  });
  it("四角は切りぬきの箱の外に出ない。同じ並びは同じと分かる", () => {
    const a = new Batch();
    a.clip(10, 10, 20, 20);
    a.rect(0, 0, 100, 100, [1, 0, 0]);
    a.rect(30, 30, 40, 40, [1, 0, 0]);
    expect(a.rects()).toEqual([[10, 10, 20, 20, 1, 0, 0]]);
    const c = new Batch();
    c.clip(0, 0, 100, 100);
    c.rect(10, 10, 20, 20, [1, 0, 0]);
    expect(a.sameAs(c)).toBe(true);
    c.rect(10, 10, 21, 20, [1, 0, 0]);
    expect(a.sameAs(c)).toBe(false);
    for (let i = 0; i < 1000; i++) c.rect(0, 0, 1, 1, [0, 0, 0]);
    expect(c.rectCount).toBe(1002);
  });
});

describe("鍵 → 画面の計測", () => {
  it("受けた時刻と描いた時刻を記録し、時計の差といっしょに送る", async () => {
    let now = 100;
    const sent: [string, string][] = [];
    const m = new KeyToScreen({
      clockOffsetUs: async () => 7_000_000,
      report: async (name, json) => void sent.push([name, json]),
      now: () => now,
    });
    await m.tick();
    expect(sent).toEqual([]);

    m.noteOn(60, 7_100_500);
    now = 101;
    m.noteOn(64, 7_101_200);
    m.drawn(102);
    now = 110;
    m.noteOn(67, 7_110_000);
    await m.tick();
    expect(sent.length).toBe(1);
    expect(sent[0]![0]).toBe(REPORT_NAME);
    expect(JSON.parse(sent[0]![1])).toEqual({
      offsetUs: 7_000_000,
      offsetAtMs: 100,
      keys: [
        { note: 60, tUs: 7_100_500, recvMs: 100, drawMs: 102 },
        { note: 64, tUs: 7_101_200, recvMs: 101, drawMs: 102 },
        { note: 67, tUs: 7_110_000, recvMs: 110, drawMs: null },
      ],
    });
    await m.tick();
    expect(sent.length).toBe(1);
    m.drawn(120);
    expect(JSON.parse(m.json()).keys[2].drawMs).toBe(120);
  });
  it("時計を合わせられなくても、送れなくても、止まらない。10 秒たったら合わせなおす", async () => {
    let now = 0;
    let clockCalls = 0;
    let fail = true;
    const sent: string[] = [];
    const m = new KeyToScreen({
      clockOffsetUs: async () => {
        clockCalls += 1;
        if (fail) throw new Error("not connected");
        return 5;
      },
      report: async (_, json) => {
        if (fail) throw new Error("no");
        sent.push(json);
      },
      now: () => now,
    });
    m.noteOn(60, 1);
    await m.tick();
    expect(sent).toEqual([]);
    fail = false;
    now = 2000;
    await m.tick();
    expect(clockCalls).toBe(2);
    expect(JSON.parse(sent[0]!).offsetUs).toBe(5);
    now = 4000;
    await m.tick();
    expect(clockCalls).toBe(2);
    now = 12000;
    await m.tick();
    expect(clockCalls).toBe(3);
  });
});
