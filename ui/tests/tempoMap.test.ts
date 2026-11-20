// 画面のテンポの地図が、Rust の地図（crates/meloet-model/src/tempo_map.rs）と同じ答えを出すか。
// 答えの表は `cargo test -p meloet-model --test tempo_map` が書く。
import { describe, expect, it } from "vitest";
import { clicksPerBar, TempoMap, type TimelineShape } from "../src/time/tempoMap";
import table from "../src/time/tempoMap.table.json";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

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

const BAR = 4 * TPQ;
/** 9 小節目（3/4）の 2 拍目、その拍の中のティック TPQ / 3 の所。 */
const IN_BAR_9 = 8 * BAR + (4 * TPQ) / 3;

// F6-Tempo: 96.00 ・ 4/4、5 小節目 104 から 9 小節目 120 へだんだん、9 小節目 3/4、13 小節目 96 ・ 4/4。
const F6: TimelineShape = {
  tempoBpm: 96,
  meterNum: 4,
  meterDen: 4,
  headRamp: false,
  tempoPoints: [
    { tick: 4 * BAR, tempoBpm: 104, ramp: true },
    { tick: 8 * BAR, tempoBpm: 120, ramp: false },
    { tick: 44 * TPQ, tempoBpm: 96, ramp: false },
  ],
  meterChanges: [
    { bar: 8, meterNum: 3, meterDen: 4 },
    { bar: 12, meterNum: 4, meterDen: 4 },
  ],
};

describe("テンポの地図", () => {
  it("答えの表には地図が 6 つある", () => {
    expect(table.maps.length).toBe(6);
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

      it("メトロノームの次に鳴る所", () => {
        for (const row of entry.clicks) {
          const c = map.nextClick(row.tick);
          expect([c.tick, c.downbeat], `nextClick(${row.tick})`).toEqual([row.next, row.downbeat]);
        }
      });

      it("整数のティックは、秒にして戻すと同じティック", () => {
        for (const row of entry.positions) expect(Math.round(map.tickAt(map.secondsAt(row.tick)))).toBe(row.tick);
      });
    });
  }

  it("仕様書の例: 7 小節目で 112.00、だんだんの全体は 8.586 秒", () => {
    const map = new TempoMap(F6);
    expect(map.tempoAt(6 * BAR)).toBe(112);
    expect(map.barBeatTick(6 * BAR)).toEqual({ bar: 7, beat: 1, tick: 0 });
    expect(map.secondsAt(4 * BAR)).toBe(10);
    expect(map.secondsAt(8 * BAR) - 10).toBeCloseTo(60 * Math.log(120 / 104), 12);
    expect(map.secondsAt(8 * BAR) - 10).toBeCloseTo(8.586, 3);
    expect(map.pieceCount).toBe(4);
  });

  it("小節の頭と、その所の拍子", () => {
    const map = new TempoMap(F6);
    expect([0, 1, 8, 9, 12, 13].map((bar) => map.tickOfBar(bar))).toEqual([0, BAR, 8 * BAR, 35 * TPQ, 44 * TPQ, 12 * BAR]);
    expect(map.meterAt(8 * BAR - 1)).toEqual({ num: 4, den: 4, bar: 7, barTick: 7 * BAR });
    expect(map.meterAt(IN_BAR_9)).toEqual({ num: 3, den: 4, bar: 8, barTick: 8 * BAR });
    expect(map.meterAt(35 * TPQ)).toEqual({ num: 3, den: 4, bar: 9, barTick: 35 * TPQ });
    expect(map.meterAt(44 * TPQ)).toEqual({ num: 4, den: 4, bar: 12, barTick: 44 * TPQ });
    expect(map.barBeatTick(44 * TPQ - 1)).toEqual({ bar: 12, beat: 3, tick: TPQ - 1 });
    expect(map.barBeatTick(-5)).toEqual({ bar: 1, beat: 1, tick: 0 });
    expect(map.barBeatTick(6 * BAR + 0.9)).toEqual({ bar: 7, beat: 1, tick: 0 });
  });

  it("次の拍の頭", () => {
    const map = new TempoMap(F6);
    expect(map.nextBeat(0)).toEqual({ tick: 0, downbeat: true });
    expect(map.nextBeat(0.5)).toEqual({ tick: TPQ, downbeat: false });
    expect(map.nextBeat(8 * BAR + 1)).toEqual({ tick: 33 * TPQ, downbeat: false });
    expect(map.nextBeat(34 * TPQ + 0.5)).toEqual({ tick: 35 * TPQ, downbeat: true });
    expect(map.nextBeat(44 * TPQ - 0.5)).toEqual({ tick: 44 * TPQ, downbeat: true });
    expect(map.nextBeat(-1)).toEqual({ tick: 0, downbeat: true });
    expect(map.nextBeat(-TPQ - 1)).toEqual({ tick: -TPQ, downbeat: false });
    expect(map.nextBeat(-BAR)).toEqual({ tick: -BAR, downbeat: true });
  });

  it("テンポは 4 分音符の数で、拍子の旗の前後で変わらない", () => {
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
    expect([-10, 0, BAR - 1, BAR, BAR + 3 * TPQ - 1, BAR + 3 * TPQ].map((tick) => map.tempoAt(tick))).toEqual([96, 96, 96, 96, 96, 96]);
    expect(map.secondsAt(BAR + 3 * TPQ) - map.secondsAt(BAR)).toBeCloseTo((3 * 60) / 96, 12);
  });

  it("6/8 のテンポ 120.00 で、1 小節は 1.5 秒、メトロノームは付点 4 分ごと", () => {
    const map = new TempoMap({ tempoBpm: 120, meterNum: 6, meterDen: 8, headRamp: false, tempoPoints: [], meterChanges: [] });
    expect(map.secondsAt(3 * TPQ)).toBeCloseTo(1.5, 12);
    expect(map.nextClick(1)).toEqual({ tick: (3 * TPQ) / 2, downbeat: false });
    expect(map.nextClick((3 * TPQ) / 2 + 1)).toEqual({ tick: 3 * TPQ, downbeat: true });
    expect(map.nextBeat(1)).toEqual({ tick: TPQ / 2, downbeat: false });
    expect(map.barBeatTick(3 * TPQ - 1)).toEqual({ bar: 1, beat: 6, tick: TPQ / 2 - 1 });
    expect(map.clicksPerBarAt(0)).toBe(2);
  });

  it("1 小節の鳴る数", () => {
    const cases: [number, number, number][] = [
      [6, 8, 2],
      [9, 8, 3],
      [12, 8, 4],
      [5, 8, 5],
      [7, 8, 7],
      [3, 8, 3],
      [4, 4, 4],
      [3, 4, 3],
      [2, 2, 2],
      [6, 16, 6],
    ];
    for (const [num, den, n] of cases) expect(clicksPerBar(num, den), `${num}/${den}`).toBe(n);
  });

  it("入れ物をわたせば、新しい物を作らない", () => {
    const map = new TempoMap(F6);
    const meter = { num: 0, den: 0, bar: 0, barTick: 0 };
    const position = { bar: 0, beat: 0, tick: 0 };
    const beat = { tick: 0, downbeat: false };
    expect(map.meterAt(IN_BAR_9, meter)).toBe(meter);
    expect(map.barBeatTick(IN_BAR_9, position)).toBe(position);
    expect(map.nextBeat(IN_BAR_9, beat)).toBe(beat);
    expect(position).toEqual({ bar: 9, beat: 2, tick: TPQ / 3 });
  });

  it("形のくずれた入力でも、Rust と同じ 1 つの地図になる", () => {
    const point = (tick: number, tempoBpm: number) => ({ tick, tempoBpm, ramp: false });
    const head = { tempoBpm: 96, meterNum: 4, meterDen: 4, headRamp: false };
    const broken = new TempoMap({
      ...head,
      tempoPoints: [point(BAR, 120), point(0, 200), point(2 * TPQ, 100), point(BAR, 110)],
      meterChanges: [
        { bar: 2, meterNum: 3, meterDen: 4 },
        { bar: 0, meterNum: 7, meterDen: 8 },
        { bar: 2, meterNum: 6, meterDen: 8 },
      ],
    });
    const fixed = new TempoMap({
      ...head,
      tempoPoints: [point(2 * TPQ, 100), point(BAR, 110)],
      meterChanges: [{ bar: 2, meterNum: 6, meterDen: 8 }],
    });
    expect(broken.pieceCount).toBe(fixed.pieceCount);
    for (const tick of [0, 500, 2 * TPQ, BAR - 1, BAR, 2 * BAR, 5000, 100_000]) {
      expect(broken.secondsAt(tick)).toBe(fixed.secondsAt(tick));
      expect(broken.tempoAt(tick)).toBe(fixed.tempoAt(tick));
      expect(broken.meterAt(tick)).toEqual(fixed.meterAt(tick));
    }
    expect(broken.tempoAt(0)).toBe(96);
    expect(broken.meterAt(0)).toEqual({ num: 4, den: 4, bar: 0, barTick: 0 });
  });
});
