// 画面のフェードの形（src/components/arrange/fade.ts）が、Rust の形（crates/meloet-model/src/audio/fade.rs）と同じ答えを出すか。
// 答えの表は `cargo test -p meloet-model --test fade` が書く。
import { describe, expect, it } from "vitest";
import { clipFadeGain, fadeGain } from "../src/components/arrange/fade";
import table from "../src/components/arrange/fade.table.json";

/** JS の Math.sin は、最後の桁が Rust とちがうことがある。 */
const near = (got: number, want: number, what: string) => expect(Math.abs(got - want), `${what}: ${got} / ${want}`).toBeLessThanOrEqual(1e-12);

describe("フェードの形（仕様書 3 章 27-A「曲がった形 1 つ（大きさがへこまない形）」）", () => {
  it("形: Rust の表と同じ（0 より前は 0、1 より後は 1）", () => {
    expect(table.shape.length).toBeGreaterThan(60);
    for (const [x, gain] of table.shape) near(fadeGain(x!), gain!, `fadeGain(${x})`);
    expect([fadeGain(0), fadeGain(1), fadeGain(-1), fadeGain(2), fadeGain(Number.NaN)]).toEqual([0, 1, 0, 1, 0]);
  });

  it("クリップの 2 つのフェード: Rust の表と同じ", () => {
    expect(table.clips.length).toBe(60);
    for (const c of table.clips) near(clipFadeGain(c.at, c.length, c.fadeIn, c.fadeOut), c.gain, JSON.stringify(c));
  });

  it("へこまない: 出るフェードと入るフェードが重なる所で、2 つの大きさの 2 乗の和が 1。まっすぐな線より上", () => {
    for (let i = 0; i <= 100; i++) {
      const x = i / 100;
      expect(fadeGain(x) ** 2 + fadeGain(1 - x) ** 2).toBeCloseTo(1, 12);
      if (i > 0 && i < 100) expect(fadeGain(x)).toBeGreaterThan(x);
    }
  });
});
