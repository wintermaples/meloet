// Time functions. The examples are the same as crates/meloet-model/src/time/tests.rs (to check that the answers match Rust).
import { describe, expect, it } from "vitest";
import type { Telemetry, TransportView } from "../src/bridge";
import {
  barBeatTick,
  clockText,
  parseMeter,
  parseTempo,
  positionText,
  shownTick,
  stepMeter,
  stepTempo,
  ticksPerBar,
  ticksPerBeat,
} from "../src/components/time";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const bbt = (tick: number, num: number, den: number) => {
  const b = barBeatTick(tick, { num, den });
  return [b.bar, b.beat, b.tick];
};

const BAR = 4 * TPQ;

describe("barBeatTick (the same examples as Rust's bar_beat_tick). The tick is the tick within that beat as it is", () => {
  it("4/4", () => {
    expect(bbt(0, 4, 4)).toEqual([1, 1, 0]);
    expect(bbt(TPQ - 1, 4, 4)).toEqual([1, 1, TPQ - 1]);
    expect(bbt(TPQ, 4, 4)).toEqual([1, 2, 0]);
    expect(bbt(BAR - 1, 4, 4)).toEqual([1, 4, TPQ - 1]);
    expect(bbt(BAR, 4, 4)).toEqual([2, 1, 0]);
    expect(bbt(BAR + 2 * TPQ + 123, 4, 4)).toEqual([2, 3, 123]);
  });

  it("3/4", () => {
    expect(bbt(3 * TPQ - 1, 3, 4)).toEqual([1, 3, TPQ - 1]);
    expect(bbt(3 * TPQ, 3, 4)).toEqual([2, 1, 0]);
    expect(bbt(3 * TPQ * 4 + TPQ + 20, 3, 4)).toEqual([5, 2, 20]);
  });

  it("6/8 counts eighth notes (tick is 0 to TPQ / 2 − 1)", () => {
    expect(bbt(TPQ / 2, 6, 8)).toEqual([1, 2, 0]);
    expect(bbt(TPQ / 2 + 10, 6, 8)).toEqual([1, 2, 10]);
    expect(bbt(3 * TPQ - 1, 6, 8)).toEqual([1, 6, TPQ / 2 - 1]);
    expect(bbt(3 * TPQ, 6, 8)).toEqual([2, 1, 0]);
  });

  it("2/2 counts half notes (tick is 0 to 2 TPQ − 1)", () => {
    expect(bbt(2 * TPQ - 1, 2, 2)).toEqual([1, 1, 2 * TPQ - 1]);
    expect(bbt(2 * TPQ, 2, 2)).toEqual([1, 2, 0]);
    expect(bbt(2 * TPQ + 1, 2, 2)).toEqual([1, 2, 1]);
    expect(bbt(BAR, 2, 2)).toEqual([2, 1, 0]);
  });

  it("7/16", () => {
    expect(bbt(TPQ / 4 + 5, 7, 16)).toEqual([1, 2, 5]);
    expect(bbt((7 * TPQ) / 4 - 1, 7, 16)).toEqual([1, 7, TPQ / 4 - 1]);
    expect(bbt((7 * TPQ) / 4, 7, 16)).toEqual([2, 1, 0]);
  });

  it("fractional ticks are cut off; before 0 is 1. 1. 0000", () => {
    expect(bbt(TPQ - 0.1, 4, 4)).toEqual([1, 1, TPQ - 1]);
    expect(bbt(-(3 * TPQ) / 2, 4, 4)).toEqual([1, 1, 0]);
  });

  it("ticks in one beat and one bar", () => {
    expect(ticksPerBar({ num: 4, den: 4 })).toBe(4 * TPQ);
    expect(ticksPerBeat({ num: 6, den: 8 })).toBe(TPQ / 2);
  });
});

describe("display (the form of the mock's 「5. 1. 000」「00:00:16.562」. Ticks have 4 digits)", () => {
  const four = { num: 4, den: 4 };
  it("position", () => {
    expect(positionText(4 * 4 * TPQ, four)).toBe("5. 1. 0000");
    expect(positionText(6 * 4 * TPQ + 2 * TPQ + TPQ / 2, four)).toBe("7. 3. 0960");
    expect(positionText(0, four)).toBe("1. 1. 0000");
    expect(positionText(5, four)).toBe("1. 1. 0005");
    expect(positionText(3 * TPQ - 1, { num: 6, den: 8 })).toBe("1. 6. 0959");
    expect(positionText(2 * TPQ - 1, { num: 2, den: 2 })).toBe("1. 1. 3839");
  });

  it("time. Milliseconds are cut off", () => {
    expect(clockText(10)).toBe("00:00:10.000");
    expect(clockText(16.5625)).toBe("00:00:16.562");
    expect(clockText(0)).toBe("00:00:00.000");
    expect(clockText(3 * 3600 + 25 * 60 + 7.0049)).toBe("03:25:07.004");
    expect(clockText(-1)).toBe("00:00:00.000");
  });
});

describe("typing tempo and time signature (the owner's decision: 30.00 to 300.00, top 1 to 12, bottom 2, 4, 8, 16)", () => {
  it("tempo", () => {
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

  it("time signature", () => {
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

  it("↑ ↓ do not go outside the range", () => {
    expect(stepTempo(96, 1)).toBe(97);
    expect(stepTempo(96.5, -10)).toBe(86.5);
    expect(stepTempo(295, 10)).toBe(300);
    expect(stepTempo(31, -10)).toBe(30);
    expect(stepMeter({ num: 4, den: 4 }, 1)).toEqual({ num: 5, den: 4 });
    expect(stepMeter({ num: 4, den: 8 }, 10)).toEqual({ num: 12, den: 8 });
    expect(stepMeter({ num: 4, den: 4 }, -10)).toEqual({ num: 1, den: 4 });
  });
});

describe("shownTick (the transport numbers and the play line)", () => {
  const transport = (state: TransportView["state"]): TransportView => ({
    state,
    position: 8 * TPQ,
    metronome: false,
    countInBars: 1,
    countInBeats: 4,
    canPlay: true,
    canRecord: true,
    canKeepPlayed: false,
    recording: state === "countIn" || state === "recording" ? { id: 1, track: 1, from: 4 * TPQ, take: 1, clip: null, lap: 0, overdub: false } : null,
    recordingVocals: [],
    recordingFaults: null,
    overdub: false,
    clickVolumeDb: 0,
    clickSound: "beep",
    clickWhen: "always",
  });
  const tel = (tick: number): Telemetry =>
    ({ sentUs: 1000, position: { tick, ticksPerUs: 0.5, state: "playing", countInBeatsLeft: 0, lap: 0 } }) as Telemetry;

  it("when stopped, transport.position even with meter values", () => {
    expect(shownTick(transport("stopped"), tel(9999), 2000)).toBe(8 * TPQ);
  });
  it("during count-in, where recording starts", () => {
    expect(shownTick(transport("countIn"), tel(-(3 * TPQ) / 2), 2000)).toBe(4 * TPQ);
  });
  it("when moving, the meter position moved forward to now. If the clock is not ready, the meter position as it is. transport.position with no meter values", () => {
    expect(shownTick(transport("playing"), tel(5000), 2000)).toBe(5500);
    expect(shownTick(transport("recording"), tel(5000), null)).toBe(5000);
    expect(shownTick(transport("playing"), null, 2000)).toBe(8 * TPQ);
  });
});
