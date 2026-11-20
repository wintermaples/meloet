// つまみの位置 ↔ 値の式が、Rust（crates/meloet-model の Gain ・ Pan ・ ParamSpec）と同じ答えを出すか。
// 答えの表は `cargo test -p meloet-model --test positions` が書く（ui/tests/fixtures/automation-positions.json）。
import { describe, expect, it } from "vitest";
import { EFFECT_PARAMS, type EffectKind } from "../src/bridge/types";
import { panAt, panPosition, paramAt, paramPosition } from "../src/components/automation/laneValue";
import { dbToFrac, fracToDb } from "../src/components/mixer/scale";
import { decimals, roundTo } from "../src/components/params/paramRows";
import table from "./fixtures/automation-positions.json";

interface Table {
  fader: { db: number | null; position: number }[];
  pan: { pan: number; position: number }[];
  params: { effect: string; param: number; values: number[][]; positions: number[][] }[];
}
const t = table as Table;

/** 表の位置は f32。 */
const F32 = 1e-6;
/** Rust の Gain は 0.01 dB きざみ、画面の fracToDb は 0.1 dB きざみ。 */
const DB_ROUNDING = 0.05 + 1e-9;

describe("フェーダー ・ センドの位置（scale.ts の dbToFrac ・ fracToDb）", () => {
  // 表の前半は dB → 位置、後半は位置 → dB（並びで分かれる）。
  const fromDb = t.fader.filter((_, i) => Object.keys(t.fader[i]!)[0] === "db");
  const fromPosition = t.fader.filter((_, i) => Object.keys(t.fader[i]!)[0] === "position");

  it("表がある", () => {
    expect(fromDb.length).toBeGreaterThan(90);
    expect(fromPosition.length).toBe(101);
  });

  it("dB → 位置", () => {
    for (const row of fromDb) expect(Math.abs(dbToFrac(row.db) - row.position), String(row.db)).toBeLessThanOrEqual(F32);
  });

  it("位置 → dB（画面は 0.1 dB に丸める）", () => {
    for (const row of fromPosition) {
      const got = fracToDb(row.position);
      if (row.db === null || got === null) expect(got, String(row.position)).toBe(row.db);
      else expect(Math.abs(got - row.db), `${row.position}: ${got} / ${row.db}`).toBeLessThanOrEqual(DB_ROUNDING);
    }
  });
});

describe("パンの位置（laneValue.ts の panPosition ・ panAt）", () => {
  it("パン → 位置 → パン", () => {
    expect(t.pan.length).toBe(21);
    for (const row of t.pan) {
      expect(Math.abs(panPosition(row.pan) - row.position), String(row.pan)).toBeLessThanOrEqual(F32);
      expect(panAt(row.position), String(row.position)).toBe(row.pan);
    }
  });
});

describe("最初から入っているエフェクトのパラメータ（paramRows.ts ・ laneValue.ts の paramPosition ・ paramAt）", () => {
  it("表にどのパラメータもある", () => {
    const want = Object.entries(EFFECT_PARAMS).flatMap(([effect, params]) => params.map((_, i) => `${effect}/${i}`));
    expect(t.params.map((p) => `${p.effect}/${p.param}`).sort()).toEqual(want.sort());
  });

  for (const row of t.params) {
    const spec = EFFECT_PARAMS[row.effect as EffectKind][row.param]!;
    const what = `${row.effect} ${spec.id}`;
    // 段階とオン ・ オフの値は段の番号（整数）だけ。表の間の値（0.4 など）は、画面は段に丸め、Rust はまっすぐ数える。
    it(`${what}: 値 → 位置`, () => {
      for (const [value, position] of row.values) {
        if (spec.shape !== "continuous" && !Number.isInteger(value)) continue;
        expect(Math.abs(paramPosition(spec, value!) - position!), `${value}`).toBeLessThanOrEqual(1e-5);
      }
    });
    // Rust の value_at は丸めないので、画面と同じに見せるけたへ丸めてからくらべる。Rust（f32）の値が丸めのちょうど
    // まん中（66.5、1.15）に落ちた所は、画面（f64）がまん中のわずかに上か下かで、となりの段になってよい。
    it(`${what}: 位置 → 値`, () => {
      for (const [position, value] of row.positions) {
        const got = paramAt(spec, position!);
        const want = roundTo(spec, value!);
        const unit = spec.shape === "continuous" ? 10 ** -decimals(spec) : 1;
        const tie = Math.abs(Math.abs((value! / unit) % 1) - 0.5) < 1e-4;
        if (tie) expect(Math.abs(got - want), `${position}: ${got} / ${value}`).toBeLessThanOrEqual(unit * (1 + 1e-9));
        else expect(got, `${position}`).toBe(want);
      }
    });
  }
});
