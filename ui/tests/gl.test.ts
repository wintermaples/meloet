// The pure parts of the WebGL surfaces: dB → length, key positions, color reading, rect lists, measurement records.
import { describe, expect, it } from "vitest";
import { colors } from "../src/design/tokens";
import { Batch } from "../src/gl/batch";
import { hexToRgb, rgb } from "../src/gl/color";
import { CLIP_HOLD_MS, DB_FLOOR, DB_SCALE, EVENT_WINS_US, FALL_DB_PER_S, KEY_FORMS, METER_HOT_FROM, OVERVIEW } from "../src/gl/dims";
import { HeldKeys, isBlack, keyboardLayout } from "../src/gl/keyboard";
import { dbToLength, fall, peakToDb } from "../src/gl/level";
import { Levels, MASTER_CHANNEL, RETURN_A_CHANNEL, RETURN_B_CHANNEL, trackChannel } from "../src/gl/levels";
import { KeyToScreen, REPORT_NAME } from "../src/gl/measure";
import { blackKeyPx, clipMarkPx, paintArrangeGrid, paintKeyboard, paintMeter, paintOverview, paintVelocity } from "../src/gl/paint";
import { glRegion, glSlot, watchSlots } from "../src/gl/slot";
import type { Telemetry } from "../src/bridge";
import { cellText, headers, rows, sectionText, specDocument, tableAfterH3 } from "./design-sources";

interface NodeFs {
  readFileSync(path: URL, encoding: "utf8"): string;
}
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): NodeFs } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const mockHtml = (board: string) => fs.readFileSync(new URL(`../../docs/design/mock/${board}.dc.html`, here), "utf8");
/** The dark grays the mock boards were drawn with, before the contrast fix of user:2026-11-23 (docs/spec/1-tokens.md). */
const mockColors: Partial<Record<keyof typeof colors, string>> = {
  "color.gray.0": "#0A0A0A",
  "color.gray.3": "#1A1A1A",
  "color.gray.5": "#2C2C2C",
  "color.gray.6": "#3A3A3A",
  "color.gray.7": "#5E5E5E",
};

const hex = (name: keyof typeof colors) => [...hexToRgb(colors[name])];
const batch = (w = 4000, h = 4000) => {
  const b = new Batch();
  b.clip(0, 0, w, h);
  return b;
};
/** Only the rects of that color ([x0, y0, x1, y1]). */
const ofColor = (b: Batch, name: keyof typeof colors) =>
  b
    .rects()
    .filter((r) => r.slice(4).every((v, i) => Math.abs(v - hex(name)[i]!) < 1e-6))
    .map((r) => r.slice(0, 4));

describe("color reading", () => {
  it("hex becomes 3 numbers from 0 to 1", () => {
    expect(hexToRgb("#FFB000")).toEqual([1, 176 / 255, 0]);
    expect(hexToRgb("#0a0A0a")).toEqual([10 / 255, 10 / 255, 10 / 255]);
  });
  it("text in a wrong form is refused", () => {
    for (const bad of ["#FFF", "FFB000", "#FFB0000", "rgb(0,0,0)"]) expect(() => hexToRgb(bad)).toThrow();
  });
  it("all token colors can be read. Looking up by name gives the same value", () => {
    for (const name of Object.keys(colors) as (keyof typeof colors)[]) {
      expect(rgb(name)).toEqual(hexToRgb(colors[name]));
    }
  });
});

describe("dB → length", () => {
  it("scale marks are the same as the spec table 「メーターとフェーダーの目盛」", () => {
    const doc = specDocument();
    const table = tableAfterH3(doc, "Scale marks of meters and faders");
    const dbs = headers(table).slice(1).map((t) => Number(t.replace("−", "-")));
    const lengths = rows(table)[0]!.slice(1).map((t) => Number(t.replace(" %", "")) / 100);
    expect(dbs.map((db, i) => [db, lengths[i]])).toEqual(DB_SCALE.map(([db, length]) => [db, length]));
    expect(DB_FLOOR).toBe(-60);

    const text = sectionText(doc, "s2");
    expect(text).toContain("Sounds quieter than −60 dB are 0 % (−∞)");
    expect(text).toContain("fall 20 dB in 1.7 seconds");
    expect(FALL_DB_PER_S).toBeCloseTo(20 / 1.7, 10);
    expect(text).toContain(`Up to −3 dB (${METER_HOT_FROM * 100} % of the length) is meter.ok, above that meter.hot`);
    expect(dbToLength(-3)).toBeCloseTo(METER_HOT_FROM, 10);
    expect(text).toContain(`Above 0 dB, a rec 2px at the top end for ${CLIP_HOLD_MS / 1000} seconds`);
  });
  it("on a mark, the mark value; between, a straight line", () => {
    for (const [db, length] of DB_SCALE) expect(dbToLength(db)).toBeCloseTo(length, 10);
    expect(dbToLength(3)).toBeCloseTo(0.9, 10);
    expect(dbToLength(-1.5)).toBeCloseTo(0.75, 10);
    expect(dbToLength(-18)).toBeCloseTo(0.3, 10);
    expect(dbToLength(-42)).toBeCloseTo(0.1, 10);
    expect(dbToLength(-59)).toBeCloseTo(0.2 / 36, 10);
  });
  it("stops at the ends", () => {
    expect(dbToLength(6.1)).toBe(1);
    expect(dbToLength(100)).toBe(1);
    expect(dbToLength(DB_FLOOR)).toBe(0);
    expect(dbToLength(-60.1)).toBe(0);
    expect(dbToLength(-200)).toBe(0);
    expect(dbToLength(-Infinity)).toBe(0);
  });
  it("louder is longer", () => {
    let last = -1;
    for (let db = DB_FLOOR; db <= 6; db += 0.25) {
      const length = dbToLength(db);
      expect(length).toBeGreaterThanOrEqual(last);
      last = length;
    }
  });
  it("the peak value to dB", () => {
    expect(peakToDb(1)).toBe(0);
    expect(peakToDb(0.5)).toBeCloseTo(-6.0206, 3);
    expect(peakToDb(0)).toBe(-Infinity);
  });
});

describe("meter fall", () => {
  it("rises at once", () => {
    expect(fall(-30, -6, 0)).toBe(-6);
    expect(fall(-Infinity, -6, 16)).toBe(-6);
  });
  it("falls at the set speed", () => {
    expect(fall(0, -Infinity, 1000)).toBeCloseTo(-FALL_DB_PER_S, 10);
    expect(fall(0, -Infinity, 500)).toBeCloseTo(-FALL_DB_PER_S / 2, 10);
  });
  it("does not go below the target. At the bottom, silence", () => {
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
  position: null,
  returnPeaks: new Float32Array(4),
  trackPeaks: new Float32Array(0),
  mics: [],
  ...over,
});

describe("screen level", () => {
  it("rises, then falls with time, and stops at the target", () => {
    const levels = new Levels();
    levels.onTelemetry(telemetry({ masterPeak: [1, 0.5], trackPeaks: new Float32Array([0.25, 0]) }), 1000);
    expect(levels.bar(0, 1000).length).toBeCloseTo(0.8, 6);
    expect(levels.bar(1, 1000).length).toBeCloseTo(dbToLength(peakToDb(0.5)), 6);
    expect(levels.bar(trackChannel(0), 1000).length).toBeCloseTo(dbToLength(peakToDb(0.25)), 6);
    expect(levels.bar(trackChannel(0) + 1, 1000).length).toBe(0);
    expect(levels.bar(trackChannel(4), 1000).length).toBe(0);

    levels.onTelemetry(telemetry({ masterPeak: [0.5, 0.5] }), 1000);
    expect(levels.bar(0, 1000).length).toBeCloseTo(0.8, 6);
    expect(levels.advance(1100)).toBe(true);
    expect(levels.bar(0, 1100).length).toBeCloseTo(dbToLength(-FALL_DB_PER_S / 10), 6);
    expect(levels.advance(3000)).toBe(false);
    expect(levels.bar(0, 3000).length).toBeCloseTo(dbToLength(peakToDb(0.5)), 6);
  });
  it("the clipping mark lasts 2 s", () => {
    const levels = new Levels();
    levels.onTelemetry(telemetry({ masterPeak: [1.2, 0.5], trackPeaks: new Float32Array([0, 1.01]) }), 500);
    expect(levels.bar(0, 500).clipped).toBe(true);
    expect(levels.bar(1, 500).clipped).toBe(false);
    expect(levels.bar(trackChannel(0), 500).clipped).toBe(false);
    expect(levels.bar(trackChannel(0) + 1, 500).clipped).toBe(true);
    expect(levels.nextClipEnd(500)).toBe(500 + CLIP_HOLD_MS);
    expect(levels.bar(0, 500 + CLIP_HOLD_MS - 1).clipped).toBe(true);
    expect(levels.bar(0, 500 + CLIP_HOLD_MS).clipped).toBe(false);
    expect(levels.nextClipEnd(500 + CLIP_HOLD_MS)).toBe(null);
  });
  it("the master clipping flag marks both left and right", () => {
    const levels = new Levels();
    levels.onTelemetry(telemetry({ masterClipped: true, trackPeaks: new Float32Array([0.1, 0.1]) }), 0);
    expect([0, 1, trackChannel(0), trackChannel(0) + 1].map((c) => levels.bar(c, 1).clipped)).toEqual([true, true, false, false]);
    expect([RETURN_A_CHANNEL, RETURN_B_CHANNEL].map((c) => levels.bar(c, 1).clipped)).toEqual([false, false]);
  });
  it("returns A and B are 4 bars between master and tracks (A left / right, B left / right). Same scale, same fall, same clipping mark", () => {
    const levels = new Levels();
    const t = telemetry({ returnPeaks: new Float32Array([0.5, 0.25, 1.2, 0]), trackPeaks: new Float32Array([1, 1]) });
    levels.onTelemetry(t, 1000);
    expect([MASTER_CHANNEL, RETURN_A_CHANNEL, RETURN_B_CHANNEL, trackChannel(0), trackChannel(1)]).toEqual([0, 2, 4, 6, 8]);
    expect(levels.bar(RETURN_A_CHANNEL, 1000).length).toBeCloseTo(dbToLength(peakToDb(0.5)), 6);
    expect(levels.bar(RETURN_A_CHANNEL + 1, 1000).length).toBeCloseTo(dbToLength(peakToDb(0.25)), 6);
    expect(levels.bar(RETURN_B_CHANNEL, 1000)).toEqual({ length: dbToLength(peakToDb(Math.fround(1.2))), clipped: true });
    expect(levels.bar(RETURN_B_CHANNEL + 1, 1000)).toEqual({ length: 0, clipped: false });
    expect(levels.bar(MASTER_CHANNEL, 1000).length).toBe(0);
    expect(levels.bar(trackChannel(0), 1000).length).toBeCloseTo(0.8, 6);

    levels.onTelemetry(telemetry({ returnPeaks: new Float32Array([0.25, 0.25, 0, 0]) }), 1000);
    expect(levels.advance(1100)).toBe(true);
    expect(levels.bar(RETURN_A_CHANNEL, 1100).length).toBeCloseTo(dbToLength(peakToDb(0.5) - FALL_DB_PER_S / 10), 6);
    expect(levels.bar(RETURN_B_CHANNEL, 1000 + CLIP_HOLD_MS).clipped).toBe(false);
  });
});

describe("key positions", () => {
  it("49 keys (C2 to C6): 29 white, 20 black", () => {
    const { whiteCount, keys } = keyboardLayout(36, 84);
    expect(keys.length).toBe(49);
    expect(whiteCount).toBe(29);
    expect(keys.filter((k) => k.black).length).toBe(20);
    expect(keys.filter((k) => !k.black).map((k) => k.index)).toEqual(Array.from({ length: 29 }, (_, i) => i));
  });
  it("black key order matches the Opt-Meter mock (whether white key i has a black key on its right)", () => {
    const blk = [1, 1, 0, 1, 1, 1, 0];
    const fromMock = Array.from({ length: 29 }, (_, i) => i < 28 && blk[i % 7] === 1);
    expect(mockHtml("Opt-Meter")).toContain("black: i < 28 && blk[i % 7] === 1");
    const { keys } = keyboardLayout(36, 84);
    const lines = new Set(keys.filter((k) => k.black).map((k) => k.index));
    // A black key sits on line index (the left edge of white key index). The line right of white key i is i + 1.
    expect(Array.from({ length: 29 }, (_, i) => lines.has(i + 1))).toEqual(fromMock);
  });
  it("black keys are C#, D#, F#, G#, A#", () => {
    expect([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].filter(isBlack)).toEqual([1, 3, 6, 8, 10]);
    expect(isBlack(61)).toBe(true);
    expect(isBlack(60)).toBe(false);
  });
  it("does not break for a range that starts on a black key or has no white keys", () => {
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

describe("held keys", () => {
  const bits = (...notes: number[]) => {
    const keys = new Uint32Array(4);
    for (const n of notes) keys[n >>> 5]! |= 1 << (n & 31);
    return keys;
  };
  it("changes when an Event comes, without waiting for meters", () => {
    const held = new HeldKeys();
    expect(held.onEvent(60, true, 1000)).toBe(true);
    expect(held.isDown(60)).toBe(true);
    expect(held.onEvent(60, true, 1001)).toBe(false);
    expect(held.onEvent(60, false, 2000)).toBe(true);
    expect(held.isDown(60)).toBe(false);
  });
  it("changes with meters alone too (keys not seen in Events)", () => {
    const held = new HeldKeys();
    expect(held.onTelemetry(bits(31, 32, 127), 5)).toBe(true);
    expect([31, 32, 127, 60].map((n) => held.isDown(n))).toEqual([true, true, true, false]);
    expect(held.onTelemetry(bits(32), 6)).toBe(true);
    expect([31, 32, 127].map((n) => held.isDown(n))).toEqual([false, true, false]);
    expect(held.onTelemetry(bits(32), 7)).toBe(false);
  });
  it("not cleared by meters older than the Event or just after it. Follows meters well after", () => {
    const held = new HeldKeys();
    held.onEvent(60, true, 1_000_000);
    expect(held.onTelemetry(bits(), 999_000)).toBe(false);
    expect(held.onTelemetry(bits(), 1_000_000 + EVENT_WINS_US)).toBe(false);
    expect(held.isDown(60)).toBe(true);
    expect(held.onTelemetry(bits(), 1_000_001 + EVENT_WINS_US)).toBe(true);
    expect(held.isDown(60)).toBe(false);
  });
  it("numbers out of range are ignored", () => {
    const held = new HeldKeys();
    expect(held.onEvent(128, true, 0)).toBe(false);
    expect(held.onEvent(-1, true, 0)).toBe(false);
  });
});

describe("rect lists", () => {
  const FULL = KEY_FORMS.full;
  const full = blackKeyPx("full", 100, 1);
  const slim = blackKeyPx("slim", 100, 1);
  it("full keyboard: the same pixels as the mock (F-Connect2). Width 1 + 29 × 12, height 32", () => {
    const b = batch();
    const down = new Set([60, 61]);
    paintKeyboard(b, { x0: 100, y0: 10, x1: 100 + 1 + 29 * FULL.white.w, y1: 10 + FULL.white.h }, full, 36, 84, (n) => down.has(n));

    expect(ofColor(b, "color.gray.6")).toEqual([[100, 10, 449, 42]]);
    const whites = ofColor(b, "color.gray.3");
    expect(whites.length).toBe(28);
    // White key i: from right of its left line to before its right line (11 px). Inside the top and bottom lines.
    expect(whites[0]).toEqual([101, 11, 112, 41]);
    expect(whites[1]).toEqual([113, 11, 124, 41]);
    // The pressed white key (C4 = white key 14) and black key (C#4 = line 15) are accent.
    expect(ofColor(b, "color.accent")).toEqual([
      [100 + 14 * 12 + 1, 11, 100 + 15 * 12, 41],
      [100 + 15 * 12 - 2, 11, 100 + 15 * 12 + 3, 28],
    ]);
    // Black key: width 7 centered on the line, height 19. Border 1 px.
    const frames = ofColor(b, "color.gray.7");
    expect(frames.length).toBe(20);
    expect(frames[0]).toEqual([100 + 12 - 3, 10, 100 + 12 + 4, 29]);
    expect(ofColor(b, "color.gray.0")[0]).toEqual([100 + 12 - 2, 11, 100 + 12 + 3, 28]);
    expect(ofColor(b, "color.gray.0").length).toBe(19);
  });
  it("no keyboard (F-Connect1): only the lines are dark. Same colors as the mock HTML", () => {
    const b = batch();
    paintKeyboard(b, { x0: 0, y0: 0, x1: 1 + 29 * FULL.white.w, y1: FULL.white.h }, full, 36, 84, () => false, true);
    expect(ofColor(b, "color.gray.5")).toEqual([[0, 0, 349, 32]]);
    expect(ofColor(b, "color.gray.6").length).toBe(20);
    expect(ofColor(b, "color.gray.7")).toEqual([]);
    expect(ofColor(b, "color.gray.3").length).toBe(29);
    expect(ofColor(b, "color.gray.0").length).toBe(20);

    const html = mockHtml("F-Connect1");
    const white = `width:${FULL.white.w}px;height:${FULL.white.h}px;border:1px solid ${mockColors["color.gray.5"]};border-left:0;background:${mockColors["color.gray.3"]}`;
    const black = `width:${FULL.black.w}px;height:${FULL.black.h}px;background:${mockColors["color.gray.0"]};border:1px solid ${mockColors["color.gray.6"]}`;
    expect(html.split(white).length - 1).toBe(29);
    expect(html.split(black).length - 1).toBe(20);
    expect(html).toContain(`<div class="f" style="border-left:1px solid ${mockColors["color.gray.5"]};flex-shrink:0">`);
  });
  it("slim keyboard: draws black keys too (1 + 29 × 8, height 12; black keys width 5, height 7). A pressed black key is accent", () => {
    const b = batch();
    const down = new Set([64, 61]);
    paintKeyboard(b, { x0: 0, y0: 0, x1: 233, y1: 12 }, slim, 36, 84, (n) => down.has(n));
    expect(b.rectCount).toBe(1 + 29 + 2 * 20);
    expect(ofColor(b, "color.gray.3")[28 - 1]).toEqual([225, 1, 232, 11]);
    // E4 = white key 16, C#4 = line 15.
    expect(ofColor(b, "color.accent")).toEqual([
      [16 * 8 + 1, 1, 17 * 8, 11],
      [15 * 8 - 1, 1, 15 * 8 + 2, 6],
    ]);
    const frames = ofColor(b, "color.gray.7");
    expect(frames.length).toBe(20);
    expect(frames[0]).toEqual([8 - 2, 0, 8 + 3, 7]);
    expect(ofColor(b, "color.gray.0")[0]).toEqual([8 - 1, 1, 8 + 2, 6]);
    expect(ofColor(b, "color.gray.0").length).toBe(19);
  });
  it("slim keyboard and slim key sizes match the F-MeterLongName mock and the spec", () => {
    const { slim: s, narrow: n } = KEY_FORMS;
    const html = mockHtml("F-MeterLongName");
    const key = (white: { w: number; h: number }, black: { w: number; h: number }) =>
      `<div class="rel" style="width:${white.w}px;height:${white.h}px;border:1px solid ${mockColors["color.gray.6"]};border-left:0;background:${mockColors["color.gray.3"]}">` +
      `<div class="abs" style="z-index:1;top:-1px;right:-${(black.w + 1) / 2}px;width:${black.w}px;height:${black.h}px;background:${mockColors["color.gray.0"]};border:1px solid ${mockColors["color.gray.7"]}">`;
    expect(html.split(key(s.white, s.black)).length - 1).toBeGreaterThan(0);
    expect(html.split(key(n.white, n.black)).length - 1).toBeGreaterThan(0);
    expect(html.split(key(FULL.white, FULL.black)).length - 1).toBeGreaterThan(0);

    const doc = specDocument();
    const s3 = sectionText(doc, "s3");
    expect(s3).toContain(`The keyboard in the slim input meter also draws black keys (width ${s.black.w}, height ${s.black.h}). A pressed black key lights up in accent.`);
    expect(cellText(doc.body)).toContain(`the keyboard picture to thin keys (width ${n.white.w})`);
  });
  it("slim keys (when the full input meter is narrow): white key 8, black key width 5, height 19", () => {
    const b = batch();
    paintKeyboard(b, { x0: 0, y0: 0, x1: 233, y1: 32 }, blackKeyPx("narrow", 100, 1), 36, 84, (n) => n === 61);
    expect(ofColor(b, "color.gray.6")).toEqual([[0, 0, 233, 32]]);
    expect(ofColor(b, "color.gray.3")[0]).toEqual([1, 1, 8, 31]);
    expect(ofColor(b, "color.gray.7")[0]).toEqual([8 - 2, 0, 8 + 3, 19]);
    expect(ofColor(b, "color.gray.0")[0]).toEqual([8 - 1, 1, 8 + 2, 18]);
    expect(ofColor(b, "color.accent")).toEqual([[15 * 8 - 1, 1, 15 * 8 + 2, 18]]);
  });
  it("black key size: multiply by the scale and round (.5 goes up). Then to screen pixels", () => {
    const at = (form: "full" | "narrow" | "slim") => [100, 125, 150].map((scale) => blackKeyPx(form, scale, 1));
    expect(at("full")).toEqual([{ w: 7, h: 19 }, { w: 9, h: 24 }, { w: 11, h: 29 }]);
    expect(at("narrow")).toEqual([{ w: 5, h: 19 }, { w: 6, h: 24 }, { w: 8, h: 29 }]);
    expect(at("slim")).toEqual([{ w: 5, h: 7 }, { w: 6, h: 9 }, { w: 8, h: 11 }]);
    expect(blackKeyPx("slim", 150, 2)).toEqual({ w: 16, h: 22 });
    expect(blackKeyPx("slim", 100, 1.5)).toEqual({ w: 8, h: 11 });
  });
  it("black keys with even width (slim keyboard at 125 %): 2 left of the line, 3 right", () => {
    const b = batch();
    paintKeyboard(b, { x0: 0, y0: 0, x1: 1 + 29 * 10, y1: 15 }, blackKeyPx("slim", 125, 1), 36, 84, () => false);
    expect(ofColor(b, "color.gray.7")[0]).toEqual([10 - 2, 0, 10 + 4, 9]);
    expect(ofColor(b, "color.gray.0")[0]).toEqual([10 - 1, 1, 10 + 3, 8]);
  });
  it("lines stay 1 px when zoomed (150 % × 2x screen = white key 36 px)", () => {
    const b = batch();
    paintKeyboard(b, { x0: 0, y0: 0, x1: 1 + 29 * 36, y1: 96 }, blackKeyPx("full", 150, 2), 36, 84, () => false);
    const whites = ofColor(b, "color.gray.3");
    expect(whites[0]).toEqual([1, 1, 36, 95]);
    expect(whites[1]).toEqual([37, 1, 72, 95]);
    const frame = ofColor(b, "color.gray.7")[0]!;
    const inner = ofColor(b, "color.gray.0")[0]!;
    expect(frame).toEqual([36 - 10, 0, 36 + 12, 58]);
    expect(inner).toEqual([frame[0]! + 1, 1, frame[2]! - 1, 57]);
  });
  it("velocity bar: 87 is 68 % (same as the mock)", () => {
    const b = batch();
    paintVelocity(b, { x0: 0, y0: 0, x1: 100, y1: 4 }, 87);
    expect(ofColor(b, "color.gray.4")).toEqual([[0, 0, 100, 4]]);
    expect(ofColor(b, "color.accent")).toEqual([[0, 0, 69, 4]]);
    const none = batch();
    paintVelocity(none, { x0: 0, y0: 0, x1: 100, y1: 4 }, 0);
    expect(none.rectCount).toBe(1);
  });
  it("meter: 2 bars, ok up to 70 % (−3 dB), hot above, clipping marked at the right end", () => {
    const b = batch();
    paintMeter(b, { x0: 10, y0: 20, x1: 110, y1: 32 }, 4, 2, { length: 0.5, clipped: false }, { length: 0.9, clipped: true });
    expect(ofColor(b, "color.gray.0")).toEqual([
      [10, 20, 110, 24],
      [10, 28, 110, 32],
    ]);
    expect(ofColor(b, "color.meter.ok")).toEqual([
      [10, 20, 60, 24],
      [10, 28, 80, 32],
    ]);
    expect(METER_HOT_FROM).toBe(0.7);
    expect(ofColor(b, "color.meter.hot")).toEqual([[80, 28, 100, 32]]);
    expect(ofColor(b, "color.rec")).toEqual([[108, 28, 110, 32]]);
  });
  it("vertical meter: L left, R right. 0 % at the bottom, 100 % at the top. Dividers and the clipping mark have the same length as horizontal, mark at the top", () => {
    const b = batch();
    paintMeter(b, { x0: 20, y0: 10, x1: 32, y1: 110 }, 4, 2, { length: 0.5, clipped: false }, { length: 0.9, clipped: true }, true);
    expect(ofColor(b, "color.gray.0")).toEqual([
      [20, 10, 24, 110],
      [28, 10, 32, 110],
    ]);
    expect(ofColor(b, "color.meter.ok")).toEqual([
      [20, 60, 24, 110],
      [28, 40, 32, 110],
    ]);
    expect(ofColor(b, "color.meter.hot")).toEqual([[28, 20, 32, 40]]);
    expect(ofColor(b, "color.rec")).toEqual([[28, 10, 32, 12]]);

    // The horizontal meter turned 90 degrees; same number and length of rects.
    const flat = batch();
    paintMeter(flat, { x0: 10, y0: 20, x1: 110, y1: 32 }, 4, 2, { length: 0.5, clipped: false }, { length: 0.9, clipped: true });
    const sizes = (bb: Batch, along: 0 | 1) => bb.rects().map((r) => [r[2 + along]! - r[along]!, r[3 - along]! - r[1 - along]!]);
    expect(sizes(b, 1)).toEqual(sizes(flat, 0));
  });
  it("clipping mark thickness: 2px times the scale, rounded (2 / 3 / 3). Then to screen pixels", () => {
    expect([100, 125, 150].map((scale) => clipMarkPx(scale, 1))).toEqual([2, 3, 3]);
    expect(clipMarkPx(125, 2)).toBe(6);
    const b = batch();
    const clipped = { length: 1, clipped: true };
    paintMeter(b, { x0: 0, y0: 0, x1: 100, y1: 10 }, 2, clipMarkPx(125, 1), clipped, clipped);
    expect(ofColor(b, "color.rec")).toEqual([
      [97, 0, 100, 4],
      [97, 6, 100, 10],
    ]);
  });
  it("meter: only the background when empty", () => {
    const b = batch();
    const silent = { length: 0, clipped: false };
    paintMeter(b, { x0: 0, y0: 0, x1: 132, y1: 12 }, 4, 2, silent, silent);
    expect(b.rectCount).toBe(2);
  });
  it("bar lines: from firstBarX every barPx. Only inside the box. Fractions snap to pixels", () => {
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
  it("when beat lines are drawn too (B-61): bars gray.6, beats gray.4 (split the same way as the piano roll)", () => {
    const b = batch();
    b.clip(0, 0, 300, 10);
    paintArrangeGrid(b, { x0: 0, y0: 0, x1: 300, y1: 10 }, 0, 128, 4);
    expect(ofColor(b, "color.gray.6").map((r) => r[0])).toEqual([0, 128, 256]);
    expect(ofColor(b, "color.gray.4").map((r) => r[0])).toEqual([32, 64, 96, 160, 192, 224, 288]);
    const scrolled = batch();
    scrolled.clip(0, 0, 100, 10);
    paintArrangeGrid(scrolled, { x0: 0, y0: 0, x1: 100, y1: 10 }, -150, 120, 6);
    expect(ofColor(scrolled, "color.gray.6").map((r) => r[0])).toEqual([90]);
    expect(ofColor(scrolled, "color.gray.4").map((r) => r[0])).toEqual([10, 30, 50, 70]);
  });
  it("the border gap of the shrunk arrange is the A-PianoRoll mock value", () => {
    expect(mockHtml("A-PianoRoll")).toContain(`top:${OVERVIEW.inset}px;width:256px;height:37px;border:1px solid ${colors[OVERVIEW.frame]}`);
  });
  it("shrunk arrange: only a 1 px border", () => {
    const b = batch();
    paintOverview(b, { x0: 0, y0: 0, x1: 256, y1: 39 }, 1);
    expect(ofColor(b, "color.accent")).toEqual([
      [0, 1, 256, 2],
      [0, 37, 256, 38],
      [0, 1, 1, 38],
      [255, 1, 256, 38],
    ]);
  });
  it("rects do not leave the clip box. The same list is seen as the same", () => {
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

describe("key → screen measurement", () => {
  it("records the receive time and draw time, and sends them with the clock difference", async () => {
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
  it("does not stop when the clock cannot be matched or sending fails. Matches again after 10 s", async () => {
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

describe("watching slots", () => {
  it("tells about existing boxes first, then on every add, change, remove. Nothing after watching stops", () => {
    const a = document.createElement("i");
    const b = document.createElement("i");
    const host = document.createElement("div");
    const seen: [string, HTMLElement, string | null][] = [];
    const firstRegion = glRegion(host, "overview");
    const first = glSlot(a, { kind: "velocity" });
    const stop = watchSlots({
      attach: (node, slot) => seen.push(["attach", node, slot.kind]),
      update: (node, slot) => seen.push(["update", node, slot.kind]),
      detach: (node) => seen.push(["detach", node, null]),
      region: (node, name) => seen.push(["region", node, name]),
      regionGone: (node) => seen.push(["regionGone", node, null]),
    });
    const second = glSlot(b, { kind: "overview" });
    second.update({ kind: "arrangeGrid", barPx: 64, firstBarX: 0 });
    expect(b.dataset.glSlot).toBe("arrangeGrid");
    first.destroy();
    firstRegion.destroy();
    stop();
    second.destroy();
    expect(seen).toEqual([
      ["region", host, "overview"],
      ["attach", a, "velocity"],
      ["attach", b, "overview"],
      ["update", b, "arrangeGrid"],
      ["detach", a, null],
      ["regionGone", host, null],
    ]);
  });
});
