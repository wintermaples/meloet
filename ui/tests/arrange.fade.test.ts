// The screen's fade shape (src/components/arrange/fade.ts) gives the same answers as the Rust shape (crates/meloet-model/src/audio/fade.rs).
// The answer table is written by `cargo test -p meloet-model --test fade`.
import { describe, expect, it } from "vitest";
import { clipFadeGain, fadeGain } from "../src/components/arrange/fade";
import table from "../src/components/arrange/fade.table.json";

/** JS Math.sin can differ from Rust in the last digit. */
const near = (got: number, want: number, what: string) => expect(Math.abs(got - want), `${what}: ${got} / ${want}`).toBeLessThanOrEqual(1e-12);

describe("fade shape (spec 3 27-A「曲がった形 1 つ（大きさがへこまない形）」)", () => {
  it("shape: same as the Rust table (0 before 0, 1 after 1)", () => {
    expect(table.shape.length).toBeGreaterThan(60);
    for (const [x, gain] of table.shape) near(fadeGain(x!), gain!, `fadeGain(${x})`);
    expect([fadeGain(0), fadeGain(1), fadeGain(-1), fadeGain(2), fadeGain(Number.NaN)]).toEqual([0, 1, 0, 1, 0]);
  });

  it("the two fades of a clip: same as the Rust table", () => {
    expect(table.clips.length).toBe(60);
    for (const c of table.clips) near(clipFadeGain(c.at, c.length, c.fadeIn, c.fadeOut), c.gain, JSON.stringify(c));
  });

  it("no dip: where a fade-out and a fade-in overlap, the sum of the squares of the two gains is 1. Above a straight line", () => {
    for (let i = 0; i <= 100; i++) {
      const x = i / 100;
      expect(fadeGain(x) ** 2 + fadeGain(1 - x) ** 2).toBeCloseTo(1, 12);
      if (i > 0 && i < 100) expect(fadeGain(x)).toBeGreaterThan(x);
    }
  });
});
