// 画面のテンポの地図が、Rust の地図（crates/meloet-model/src/tempo_map.rs）と同じ答えを出すか。
// 答えの表は `cargo test -p meloet-model --test tempo_map` が書く。
import { describe, expect, it } from "vitest";
import { TempoMap, type TimelineShape } from "../src/time/tempoMap";
import table from "../src/time/tempoMap.table.json";

interface TableTimeline {
  tempo: number;
  meter: number[];
  headRamp: boolean;
  tempoPoints: { tick: number; tempo: number; ramp: boolean }[];
  meterChanges: { bar: number; meter: number[] }[];
}

const shapeOf = (t: TableTimeline): TimelineShape => ({
  tempoBpm: t.tempo,
  meterNum: t.meter[0]!,
  meterDen: t.meter[1]!,
  headRamp: t.headRamp,
  tempoPoints: t.tempoPoints.map((p) => ({ tick: p.tick, tempoBpm: p.tempo, ramp: p.ramp })),
  meterChanges: t.meterChanges.map((c) => ({ bar: c.bar, meterNum: c.meter[0]!, meterDen: c.meter[1]! })),
});

/** 相対 1e-9（JS の log1p と expm1 は、最後の桁が Rust とちがうことがある）。 */
function near(got: number, want: number, what: string): void {
  expect(Math.abs(got - want), `${what}: ${got} / ${want}`).toBeLessThanOrEqual(1e-9 * Math.abs(want));
}

// F6-Tempo: 96.00 ・ 4/4、5 小節目 104 から 9 小節目 120 へだんだん、9 小節目 3/4、13 小節目 96 ・ 4/4。
const F6: TimelineShape = {
  tempoBpm: 96,
  meterNum: 4,
  meterDen: 4,
  headRamp: false,
  tempoPoints: [
    { tick: 7680, tempoBpm: 104, ramp: true },
    { tick: 15360, tempoBpm: 120, ramp: false },
    { tick: 21120, tempoBpm: 96, ramp: false },
  ],
  meterChanges: [
    { bar: 8, meterNum: 3, meterDen: 4 },
    { bar: 12, meterNum: 4, meterDen: 4 },
  ],
};

describe("テンポの地図", () => {
  it("答えの表には地図が 5 つある", () => {
    expect(table.maps.length).toBe(5);
  });

  for (const entry of table.maps) {
    describe(entry.name, () => {
      const map = new TempoMap(shapeOf(entry.timeline));

      it("ティック → 秒、その所のテンポ", () => {
        for (const row of entry.atTicks) {
          near(map.secondsAt(row.tick), row.seconds, `secondsAt(${row.tick})`);
          near(map.tempoAt(row.tick), row.tempo, `tempoAt(${row.tick})`);
        }
      });

      it("秒 → ティック", () => {
        for (const row of entry.atSeconds) near(map.tickAt(row.seconds), row.tick, `tickAt(${row.seconds})`);
      });

      it("小節 . 拍 . ティック", () => {
        for (const row of entry.positions) {
          const b = map.barBeatTick(row.tick);
          expect([b.bar, b.beat, b.tick], `barBeatTick(${row.tick})`).toEqual(row.barBeatTick);
        }
      });

      it("整数のティックは、秒にして戻すと同じティック", () => {
        for (const row of entry.positions) expect(Math.round(map.tickAt(map.secondsAt(row.tick)))).toBe(row.tick);
      });
    });
  }

  it("仕様書の例: 7 小節目で 112.00、だんだんの全体は 8.586 秒", () => {
    const map = new TempoMap(F6);
    expect(map.tempoAt(11520)).toBe(112);
    expect(map.barBeatTick(11520)).toEqual({ bar: 7, beat: 1, tick: 0 });
    expect(map.secondsAt(7680)).toBe(10);
    expect(map.secondsAt(15360) - 10).toBeCloseTo(60 * Math.log(120 / 104), 12);
    expect(map.secondsAt(15360) - 10).toBeCloseTo(8.586, 3);
    expect(map.pieceCount).toBe(4);
  });

  it("小節の頭と、その所の拍子", () => {
    const map = new TempoMap(F6);
    expect([0, 1, 8, 9, 12, 13].map((bar) => map.tickOfBar(bar))).toEqual([0, 1920, 15360, 16800, 21120, 23040]);
    expect(map.meterAt(15359)).toEqual({ num: 4, den: 4, bar: 7, barTick: 13440 });
    expect(map.meterAt(16000)).toEqual({ num: 3, den: 4, bar: 8, barTick: 15360 });
    expect(map.meterAt(16800)).toEqual({ num: 3, den: 4, bar: 9, barTick: 16800 });
    expect(map.meterAt(21120)).toEqual({ num: 4, den: 4, bar: 12, barTick: 21120 });
    expect(map.barBeatTick(21119)).toEqual({ bar: 12, beat: 3, tick: 479 });
    expect(map.barBeatTick(-5)).toEqual({ bar: 1, beat: 1, tick: 0 });
    expect(map.barBeatTick(11520.9)).toEqual({ bar: 7, beat: 1, tick: 0 });
  });

  it("次の拍の頭", () => {
    const map = new TempoMap(F6);
    expect(map.nextBeat(0)).toEqual({ tick: 0, downbeat: true });
    expect(map.nextBeat(0.5)).toEqual({ tick: 480, downbeat: false });
    expect(map.nextBeat(15361)).toEqual({ tick: 15840, downbeat: false });
    expect(map.nextBeat(16320.5)).toEqual({ tick: 16800, downbeat: true });
    expect(map.nextBeat(21119.5)).toEqual({ tick: 21120, downbeat: true });
    expect(map.nextBeat(-1)).toEqual({ tick: 0, downbeat: true });
    expect(map.nextBeat(-481)).toEqual({ tick: -480, downbeat: false });
    expect(map.nextBeat(-1920)).toEqual({ tick: -1920, downbeat: true });
  });

  it("4 分音符の 1 分間の数は、拍子の下の数で変わる", () => {
    const map = new TempoMap({
      tempoBpm: 96,
      meterNum: 4,
      meterDen: 4,
      headRamp: false,
      tempoPoints: [],
      meterChanges: [
        { bar: 1, meterNum: 6, meterDen: 8 },
        { bar: 2, meterNum: 2, meterDen: 2 },
      ],
    });
    expect([-10, 0, 1919, 1920, 3359, 3360].map((tick) => map.quarterBpmAt(tick))).toEqual([96, 96, 96, 48, 48, 192]);
    expect(map.tempoAt(3000)).toBe(96);
  });

  it("入れ物をわたせば、新しい物を作らない", () => {
    const map = new TempoMap(F6);
    const meter = { num: 0, den: 0, bar: 0, barTick: 0 };
    const position = { bar: 0, beat: 0, tick: 0 };
    const beat = { tick: 0, downbeat: false };
    expect(map.meterAt(16000, meter)).toBe(meter);
    expect(map.barBeatTick(16000, position)).toBe(position);
    expect(map.nextBeat(16000, beat)).toBe(beat);
    expect(position).toEqual({ bar: 9, beat: 2, tick: 160 });
  });

  it("形のくずれた入力でも、Rust と同じ 1 つの地図になる", () => {
    const point = (tick: number, tempoBpm: number) => ({ tick, tempoBpm, ramp: false });
    const head = { tempoBpm: 96, meterNum: 4, meterDen: 4, headRamp: false };
    const broken = new TempoMap({
      ...head,
      tempoPoints: [point(1920, 120), point(0, 200), point(960, 100), point(1920, 110)],
      meterChanges: [
        { bar: 2, meterNum: 3, meterDen: 4 },
        { bar: 0, meterNum: 7, meterDen: 8 },
        { bar: 2, meterNum: 6, meterDen: 8 },
      ],
    });
    const fixed = new TempoMap({
      ...head,
      tempoPoints: [point(960, 100), point(1920, 110)],
      meterChanges: [{ bar: 2, meterNum: 6, meterDen: 8 }],
    });
    expect(broken.pieceCount).toBe(fixed.pieceCount);
    for (const tick of [0, 500, 960, 1919, 1920, 3840, 5000, 100_000]) {
      expect(broken.secondsAt(tick)).toBe(fixed.secondsAt(tick));
      expect(broken.tempoAt(tick)).toBe(fixed.tempoAt(tick));
      expect(broken.meterAt(tick)).toEqual(fixed.meterAt(tick));
    }
    expect(broken.tempoAt(0)).toBe(96);
    expect(broken.meterAt(0)).toEqual({ num: 4, den: 4, bar: 0, barTick: 0 });
  });
});
