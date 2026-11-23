// Checks that the knob position <-> value formulas give the same answers as Rust (Gain, Pan, ParamSpec in crates/meloet-model).
// The answer table is written by `cargo test -p meloet-model --test positions` (ui/tests/fixtures/automation-positions.json).
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

/** Positions in the table are f32. */
const F32 = 1e-6;
/** Rust Gain is in steps of 0.01 dB, the screen's fracToDb in steps of 0.1 dB. */
const DB_ROUNDING = 0.05 + 1e-9;

describe("fader and send position (dbToFrac and fracToDb in scale.ts)", () => {
  // The first half of the table is dB -> position, the second half position -> dB (split by order).
  const fromDb = t.fader.filter((_, i) => Object.keys(t.fader[i]!)[0] === "db");
  const fromPosition = t.fader.filter((_, i) => Object.keys(t.fader[i]!)[0] === "position");

  it("the table exists", () => {
    expect(fromDb.length).toBeGreaterThan(90);
    expect(fromPosition.length).toBe(101);
  });

  it("dB -> position", () => {
    for (const row of fromDb) expect(Math.abs(dbToFrac(row.db) - row.position), String(row.db)).toBeLessThanOrEqual(F32);
  });

  it("position -> dB (the screen rounds to 0.1 dB)", () => {
    for (const row of fromPosition) {
      const got = fracToDb(row.position);
      if (row.db === null || got === null) expect(got, String(row.position)).toBe(row.db);
      else expect(Math.abs(got - row.db), `${row.position}: ${got} / ${row.db}`).toBeLessThanOrEqual(DB_ROUNDING);
    }
  });
});

describe("pan position (panPosition and panAt in laneValue.ts)", () => {
  it("pan -> position -> pan", () => {
    expect(t.pan.length).toBe(21);
    for (const row of t.pan) {
      expect(Math.abs(panPosition(row.pan) - row.position), String(row.pan)).toBeLessThanOrEqual(F32);
      expect(panAt(row.position), String(row.position)).toBe(row.pan);
    }
  });
});

describe("built-in effect parameters (paramRows.ts, and paramPosition and paramAt in laneValue.ts)", () => {
  it("every parameter is in the table", () => {
    const want = Object.entries(EFFECT_PARAMS).flatMap(([effect, params]) => params.map((_, i) => `${effect}/${i}`));
    expect(t.params.map((p) => `${p.effect}/${p.param}`).sort()).toEqual(want.sort());
  });

  for (const row of t.params) {
    const spec = EFFECT_PARAMS[row.effect as EffectKind][row.param]!;
    const what = `${row.effect} ${spec.id}`;
    // Values of steps and on/off are only step numbers (integers). Between them (0.4 and so on) the screen rounds to a step and Rust counts in a straight line.
    it(`${what}: value -> position`, () => {
      for (const [value, position] of row.values) {
        if (spec.shape !== "continuous" && !Number.isInteger(value)) continue;
        expect(Math.abs(paramPosition(spec, value!) - position!), `${value}`).toBeLessThanOrEqual(1e-5);
      }
    });
    // Rust value_at does not round, so round to the digits the screen shows before comparing. Where the Rust (f32) value lands exactly
    // on the middle of a rounding (66.5, 1.15), the screen (f64) may be a little above or below it, so the next step is allowed.
    it(`${what}: position -> value`, () => {
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
