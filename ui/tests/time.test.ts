// 時間の関数。例は crates/meloet-model/src/time/tests.rs と同じ（Rust と同じ答えになるか）。
import { describe, expect, it } from "vitest";
import {
  barBeatTick,
  clockText,
  parseMeter,
  parseTempo,
  positionText,
  stepMeter,
  stepTempo,
  ticksPerBar,
  ticksPerBeat,
  ticksToSeconds,
} from "../src/components/time";

const bbt = (tick: number, num: number, den: number) => {
  const b = barBeatTick(tick, { num, den });
  return [b.bar, b.beat, b.tick];
};

describe("barBeatTick（Rust の bar_beat_tick と同じ例）", () => {
  it("4/4", () => {
    expect(bbt(0, 4, 4)).toEqual([1, 1, 0]);
    expect(bbt(479, 4, 4)).toEqual([1, 1, 479]);
    expect(bbt(480, 4, 4)).toEqual([1, 2, 0]);
    expect(bbt(1919, 4, 4)).toEqual([1, 4, 479]);
    expect(bbt(1920, 4, 4)).toEqual([2, 1, 0]);
    expect(bbt(1920 + 960 + 123, 4, 4)).toEqual([2, 3, 123]);
  });

  it("3/4", () => {
    expect(bbt(1439, 3, 4)).toEqual([1, 3, 479]);
    expect(bbt(1440, 3, 4)).toEqual([2, 1, 0]);
    expect(bbt(1440 * 4 + 500, 3, 4)).toEqual([5, 2, 20]);
  });

  it("6/8 は 8 分音符を数える", () => {
    expect(bbt(240, 6, 8)).toEqual([1, 2, 0]);
    expect(bbt(250, 6, 8)).toEqual([1, 2, 20]);
    expect(bbt(1439, 6, 8)).toEqual([1, 6, 478]);
    expect(bbt(1440, 6, 8)).toEqual([2, 1, 0]);
  });

  it("2/2 は 2 分音符を数える", () => {
    expect(bbt(959, 2, 2)).toEqual([1, 1, 479]);
    expect(bbt(960, 2, 2)).toEqual([1, 2, 0]);
    expect(bbt(961, 2, 2)).toEqual([1, 2, 0]);
    expect(bbt(1920, 2, 2)).toEqual([2, 1, 0]);
  });

  it("7/16", () => {
    expect(bbt(125, 7, 16)).toEqual([1, 2, 20]);
    expect(bbt(839, 7, 16)).toEqual([1, 7, 476]);
    expect(bbt(840, 7, 16)).toEqual([2, 1, 0]);
  });

  it("小数のティックは切りすて、0 より前は 1. 1. 000", () => {
    expect(bbt(479.9, 4, 4)).toEqual([1, 1, 479]);
    expect(bbt(-720, 4, 4)).toEqual([1, 1, 0]);
  });

  it("1 拍と 1 小節のティック", () => {
    expect(ticksPerBar({ num: 4, den: 4 })).toBe(1920);
    expect(ticksPerBeat({ num: 6, den: 8 })).toBe(240);
  });
});

describe("ticksToSeconds（Rust の ticks_to_seconds と同じ例）", () => {
  it("96.00 の 4/4 と、120.00 の 6/8", () => {
    expect(ticksToSeconds(480, 96, { num: 4, den: 4 })).toBeCloseTo(0.625, 12);
    expect(ticksToSeconds(1920, 96, { num: 4, den: 4 })).toBeCloseTo(2.5, 12);
    expect(ticksToSeconds(240, 120, { num: 6, den: 8 })).toBeCloseTo(0.5, 12);
    expect(ticksToSeconds(0, 120, { num: 6, den: 8 })).toBe(0);
  });
});

describe("見せかた（モックの「5. 1. 000」「00:00:16.562」）", () => {
  const four = { num: 4, den: 4 };
  it("位置", () => {
    expect(positionText(4 * 1920, four)).toBe("5. 1. 000");
    expect(positionText(6 * 1920 + 960 + 240, four)).toBe("7. 3. 240");
    expect(positionText(0, four)).toBe("1. 1. 000");
  });

  it("時間。ミリ秒は切りすて", () => {
    expect(clockText(ticksToSeconds(4 * 1920, 96, four))).toBe("00:00:10.000");
    expect(clockText(ticksToSeconds(6 * 1920 + 960 + 240, 96, four))).toBe("00:00:16.562");
    expect(clockText(0)).toBe("00:00:00.000");
    expect(clockText(3 * 3600 + 25 * 60 + 7.0049)).toBe("03:25:07.004");
    expect(clockText(-1)).toBe("00:00:00.000");
  });
});

describe("テンポと拍子の打ちこみ（ヤチヨさんの決め: 30.00〜300.00、上 1〜12、下 2・4・8・16）", () => {
  it("テンポ", () => {
    expect(parseTempo("96")).toBe(96);
    expect(parseTempo("96.5")).toBe(96.5);
    expect(parseTempo(" 120.25 ")).toBe(120.25);
    expect(parseTempo("３０")).toBe(30);
    expect(parseTempo("300.00")).toBe(300);
    expect(parseTempo("29.99")).toBeNull();
    expect(parseTempo("300.01")).toBeNull();
    expect(parseTempo("96.004")).toBeNull();
    expect(parseTempo("")).toBeNull();
    expect(parseTempo("abc")).toBeNull();
    expect(parseTempo("-96")).toBeNull();
    expect(parseTempo("1e2")).toBeNull();
  });

  it("拍子", () => {
    expect(parseMeter("4/4")).toEqual({ num: 4, den: 4 });
    expect(parseMeter("12 / 16")).toEqual({ num: 12, den: 16 });
    expect(parseMeter("６／８")).toEqual({ num: 6, den: 8 });
    expect(parseMeter("1/2")).toEqual({ num: 1, den: 2 });
    expect(parseMeter("0/4")).toBeNull();
    expect(parseMeter("13/4")).toBeNull();
    expect(parseMeter("4/3")).toBeNull();
    expect(parseMeter("4/32")).toBeNull();
    expect(parseMeter("4")).toBeNull();
    expect(parseMeter("4/4/4")).toBeNull();
  });

  it("↑ ↓ は範囲の外へ出ない", () => {
    expect(stepTempo(96, 1)).toBe(97);
    expect(stepTempo(96.5, -10)).toBe(86.5);
    expect(stepTempo(295, 10)).toBe(300);
    expect(stepTempo(31, -10)).toBe(30);
    expect(stepMeter({ num: 4, den: 4 }, 1)).toEqual({ num: 5, den: 4 });
    expect(stepMeter({ num: 4, den: 8 }, 10)).toEqual({ num: 12, den: 8 });
    expect(stepMeter({ num: 4, den: 4 }, -10)).toEqual({ num: 1, den: 4 });
  });
});
