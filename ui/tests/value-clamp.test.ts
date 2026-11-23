// Out-of-range values (docs/history/design-notes.ja.md B-68): set to the nearest end and build the ClampValue content. Unreadable gives null.
import { describe, expect, it } from "vitest";
import { nearestDen, readMeter, readPosition, readScaled, readTempo, readVelocity } from "../src/components/valueClamp";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const M44 = { num: 4, den: 4 };
const M68 = { num: 6, den: 8 };

describe("reading numbers", () => {
  it("rounding counts on the text (120.555 → 120.56). Reads full-width, U+2212, leading 0", () => {
    expect(readScaled("120.555", 2)).toBe(12056);
    expect(readScaled("120.554", 2)).toBe(12055);
    expect(readScaled("２．５", 0)).toBe(3);
    expect(readScaled("−5", 0)).toBe(-5);
    expect(readScaled(".5", 0)).toBe(1);
    expect(readScaled("007", 0)).toBe(7);
  });

  it("unreadable gives null (empty, letters, exponents, a minus only)", () => {
    for (const bad of ["", " ", "あ", "1e2", "-", ".", "1.2.3", "12a"]) expect(readScaled(bad, 0), bad).toBeNull();
  });
});

describe("tempo", () => {
  it("put into 30.00 to 300.00. Rounding inside the range is not told", () => {
    expect(readTempo("29.99")).toEqual({ value: 30, clamp: { field: "tempo", value: "30.00", range: null } });
    expect(readTempo("1000")).toEqual({ value: 300, clamp: { field: "tempo", value: "300.00", range: null } });
    expect(readTempo("96.004")).toEqual({ value: 96, clamp: null });
    expect(readTempo("abc")).toBeNull();
  });
});

describe("time signature bottom (nearest value; the larger when equally far)", () => {
  it("3 → 4, 6 → 8, 12 → 16, 99 → 16, 0 / 1 → 2, valid values stay", () => {
    expect([3, 6, 12, 99, 0, 1, -5, 2, 4, 8, 16, 5, 10, 13].map(nearestDen)).toEqual([4, 8, 16, 16, 2, 2, 2, 2, 4, 8, 16, 4, 8, 16]);
  });

  it("time signature: top is 1 to 12. If both are rounded, only tell about the top. Decimals are rounded", () => {
    expect(readMeter("4/3")).toEqual({ value: { num: 4, den: 4 }, clamp: { field: "meterDen", value: "4", range: null } });
    expect(readMeter("20/99")).toEqual({ value: { num: 12, den: 16 }, clamp: { field: "meterNum", value: "12", range: null } });
    expect(readMeter("3.4/8")).toEqual({ value: { num: 3, den: 8 }, clamp: null });
    expect(readMeter("６／８")).toEqual({ value: { num: 6, den: 8 }, clamp: null });
    expect(readMeter("4")).toBeNull();
    expect(readMeter("4/")).toBeNull();
    expect(readMeter("4/4/4")).toBeNull();
  });
});

describe("velocity", () => {
  it("1〜127", () => {
    expect(readVelocity("200")).toEqual({ value: 127, clamp: { field: "velocity", value: "127", range: null } });
    expect(readVelocity("0")).toEqual({ value: 1, clamp: { field: "velocity", value: "1", range: null } });
    expect(readVelocity("99.5")).toEqual({ value: 100, clamp: null });
    expect(readVelocity("")).toBeNull();
  });
});

describe("play position (bar, beat, tick each into range)", () => {
  it("「5」「5.2」「5.2.0960」. Spaces around the dots are allowed", () => {
    expect(readPosition("5", M44)).toEqual({ value: 4 * 4 * TPQ, clamp: null });
    expect(readPosition("5.2", M44)).toEqual({ value: 4 * 4 * TPQ + TPQ, clamp: null });
    expect(readPosition("5. 2. 0960", M44)).toEqual({ value: 4 * 4 * TPQ + (3 * TPQ) / 2, clamp: null });
    expect(readPosition("1. 1. 0000", M44)).toEqual({ value: 0, clamp: null });
  });

  it("beat is 1 to the top number, tick is 0 to 1919 (4/4), bar from 1. Only the largest unit is told", () => {
    expect(readPosition("3.5", M44)?.clamp).toEqual({ field: "beat", value: "3. 4. 0000", range: "1〜4" });
    expect(readPosition("3.2.1920", M44)?.clamp).toEqual({ field: "tick", value: "3. 2. 1919", range: "0〜1919" });
    expect(readPosition("0", M44)?.clamp).toEqual({ field: "bar", value: "1. 1. 0000", range: null });
    expect(readPosition("-3.9.9999", M44)).toEqual({ value: 3 * TPQ + TPQ - 1, clamp: { field: "bar", value: "1. 4. 1919", range: null } });
    expect(readPosition("2.9.9999", M44)?.clamp?.field).toBe("beat");
  });

  it("6/8: 1 beat is an eighth note. Ticks are the ticks inside that beat as they are (0 to 0959)", () => {
    expect(readPosition("2.7", M68)?.clamp).toEqual({ field: "beat", value: "2. 6. 0000", range: "1〜6" });
    expect(readPosition("1.2.0480", M68)).toEqual({ value: TPQ / 2 + TPQ / 4, clamp: null });
    expect(readPosition("1.1.0959", M68)).toEqual({ value: TPQ / 2 - 1, clamp: null });
    expect(readPosition("1.1.0960", M68)?.clamp).toEqual({ field: "tick", value: "1. 1. 0959", range: "0〜0959" });
  });

  it("unreadable gives null", () => {
    for (const bad of ["", "あ", "1.2.3.4", "1..2", "1.x", "1.5e1"]) expect(readPosition(bad, M44), bad).toBeNull();
  });
});
