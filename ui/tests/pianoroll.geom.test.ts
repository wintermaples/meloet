// ピアノロールの形の計算（geom.ts）: グリッド、音階、行の色、ひらいたときの見せかた、ドラッグの量、読み値の文、CC の数。
import { describe, expect, it } from "vitest";
import * as G from "../src/components/pianoroll/geom";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const METER = { num: 4, den: 4 };

describe("グリッドと音階", () => {
  it("グリッド 1 つのティック（1 小節、1/2〜1/32、三連符は 2/3）", () => {
    expect(G.gridTicks({ division: 0, triplet: false }, METER)).toBe(4 * TPQ);
    expect(G.gridTicks({ division: 0, triplet: false }, { num: 3, den: 4 })).toBe(3 * TPQ);
    expect(G.gridTicks({ division: 16, triplet: false }, METER)).toBe(TPQ / 4);
    expect(G.gridTicks({ division: 16, triplet: true }, METER)).toBe(TPQ / 6);
    expect(G.gridTicks({ division: 32, triplet: true }, METER)).toBe(TPQ / 12);
  });

  it("音階（メジャー ・ マイナー）と、スケール固定で書けない行", () => {
    const cMajor = { root: 0, mode: "major" as const, lock: true };
    expect([60, 62, 64, 65, 67, 69, 71].every((k) => G.inScale(k, cMajor))).toBe(true);
    expect([61, 63, 66, 68, 70].some((k) => G.inScale(k, cMajor))).toBe(false);
    expect(G.inScale(70, { root: 9, mode: "minor" })).toBe(false);
    expect(G.inScale(67, { root: 9, mode: "minor" })).toBe(true);
    expect(G.writable(61, cMajor)).toBe(false);
    expect(G.writable(61, { ...cMajor, lock: false })).toBe(true);
  });

  it("行の地: 白鍵 gray.2、黒鍵 gray.1、書けない行 gray.0（A-PianoRoll）", () => {
    const off = { root: 0, mode: "major" as const, lock: false };
    expect(G.rowColor(60, off)).toBe("color.gray.2");
    expect(G.rowColor(61, off)).toBe("color.gray.1");
    expect(G.rowColor(61, { ...off, lock: true })).toBe("color.gray.0");
    expect(G.rowColor(66, { root: 7, mode: "major", lock: true })).toBe("color.gray.1");
  });

  it("ノートの面はベロシティで gray.8 ・ 9 ・ 10", () => {
    expect([G.velocityColor(69), G.velocityColor(70), G.velocityColor(89), G.velocityColor(90)]).toEqual([
      "color.gray.8",
      "color.gray.9",
      "color.gray.9",
      "color.gray.10",
    ]);
  });

  it("音の名前（C4 = 60）", () => {
    expect([G.noteName(60), G.noteName(50), G.noteName(36), G.noteName(127)]).toEqual(["C4", "D3", "C2", "G9"]);
  });
});

describe("ひらいたときの見せかた", () => {
  it("縦: A-PianoRoll（41 行で E5）と F-PR150（18 行で D4）。C3〜C5 のノート", () => {
    expect(G.fitTopKey({ low: 48, high: 72 }, 41)).toBe(76);
    expect(G.fitTopKey({ low: 48, high: 72 }, 18)).toBe(62);
    expect(G.fitTopKey(null, 41)).toBe(80);
    expect(G.fitTopKey({ low: 120, high: 127 }, 41)).toBe(127);
    expect(G.fitTopKey({ low: 0, high: 2 }, 41)).toBe(40);
  });

  it("横: クリップが入るいちばん大きい段（1280 で 1 拍 40、1920 で 80、150 % も 40）", () => {
    expect(G.fitQuarterPx(16 * TPQ, 696, 100)).toBe(40);
    expect(G.fitQuarterPx(16 * TPQ, 1336, 100)).toBe(80);
    expect(G.fitQuarterPx(16 * TPQ, 1016, 150)).toBe(40);
    expect(G.fitQuarterPx(1600 * TPQ, 696, 100)).toBe(10);
  });

  it("縦の足し算の行の数（窓 448 ・ 720 ・ 768 ・ 800 ・ 1080 で 11 ・ 34 ・ 38 ・ 41 ・ 49）", () => {
    expect([140, 412, 460, 492, 588].map((h) => G.wholeRows(h, 12))).toEqual([11, 34, 38, 41, 49]);
  });
});

describe("当たりと囲み", () => {
  const n = G.shownOf(take([[0, TPQ, 60], [TPQ, TPQ, 62], [2 * TPQ, TPQ / 2, 64]]), 4 * TPQ);

  it("ノートの上、右はし 6px（長さ）、空いている所", () => {
    const y60 = (72 - 60) * 12 + 5;
    // ノート 0 は x 160〜200（曲の 4 * TPQ から）。
    expect(G.hitNote(n, GEOM, 170, y60, 6)).toEqual({ note: 0, edge: false });
    expect(G.hitNote(n, GEOM, 196, y60, 6)).toEqual({ note: 0, edge: true });
    expect(G.hitNote(n, GEOM, 205, y60, 6)).toBeNull();
    expect(G.hitNote(n, GEOM, 170, y60 + 12, 6)).toBeNull();
  });

  it("囲んだ枠にかかるノート（どの向きでも）", () => {
    const y = (k: number) => (72 - k) * 12 + 5;
    expect(G.notesInBox(n, GEOM, 150, y(64), 250, y(60))).toEqual([0, 1, 2]);
    expect(G.notesInBox(n, GEOM, 250, y(60), 150, y(62))).toEqual([0, 1]);
    expect(G.notesInBox(n, GEOM, 0, 0, 100, 100)).toEqual([]);
  });
});

describe("ドラッグの量", () => {
  const n = G.shownOf(take([[0, TPQ, 60], [TPQ, TPQ, 62], [2 * TPQ, TPQ / 2, 127]]), 0);

  it("動かす: つかんだノートの始まりがグリッドに吸いつく。Shift（step なし）では吸いつかない", () => {
    expect(G.moveDelta(n, [0, 1], 1, (13 * TPQ) / 48, 2, TPQ / 4)).toEqual({ ticks: TPQ / 4, keys: 2 });
    expect(G.moveDelta(n, [0, 1], 1, (5 * TPQ) / 48, 0, TPQ / 4)).toEqual({ ticks: 0, keys: 0 });
    expect(G.moveDelta(n, [0, 1], 1, (5 * TPQ) / 48, 0, null)).toEqual({ ticks: (5 * TPQ) / 48, keys: 0 });
  });

  it("動かす: 曲の頭より前と、鍵の外へは出ない", () => {
    expect(G.moveDelta(n, [0, 1], 1, -4 * TPQ, -100, TPQ / 4)).toEqual({ ticks: 0, keys: -60 });
    expect(G.moveDelta(n, [2], 2, 0, 5, TPQ / 4)).toEqual({ ticks: 0, keys: 0 });
  });

  it("長さ: 終わりがグリッドに吸いつき、1 ティックより短くならない（吸いつくときはグリッド 1 つ）", () => {
    expect(G.resizeDelta(n, [0], 0, (5 * TPQ) / 24, TPQ / 4)).toBe(TPQ / 4);
    expect(G.resizeDelta(n, [0], 0, -2 * TPQ, TPQ / 4)).toBe((-3 * TPQ) / 4);
    expect(G.resizeDelta(n, [0, 2], 0, -2 * TPQ, null)).toBe(-(TPQ / 2 - 1));
    expect(G.resizeDelta(n, [0], 0, 7, null)).toBe(7);
  });

  it("書く: 押したグリッドの頭から、いちばん短くてグリッド 1 つ", () => {
    expect(G.drawStart((25 * TPQ) / 48, TPQ / 4)).toBe(TPQ / 2);
    expect(G.drawStart((25 * TPQ) / 48, null)).toBe((25 * TPQ) / 48);
    expect(G.drawLength(TPQ / 2, (25 * TPQ) / 48, TPQ / 4, true)).toBe(TPQ / 4);
    expect(G.drawLength(TPQ / 2, (35 * TPQ) / 24, TPQ / 4, true)).toBe(TPQ);
    expect(G.drawLength(TPQ / 2, (35 * TPQ) / 24, TPQ / 4, false)).toBe((23 * TPQ) / 24);
  });

  it("ベロシティ: 上へ動かした px を値に。1〜127 で止まる", () => {
    expect(G.velocityDelta(32, 64)).toBe(64);
    expect(G.velocityDelta(-6, 64)).toBe(-12);
    expect(G.clampVelocity(200)).toBe(127);
    expect(G.clampVelocity(-5)).toBe(1);
  });

  it("Mod D: 範囲の長さをグリッドに切り上げた分", () => {
    expect(G.duplicateTicks(n, [0, 1], TPQ / 4)).toBe(2 * TPQ);
    expect(G.duplicateTicks(n, [2], 4 * TPQ)).toBe(4 * TPQ);
    expect(G.duplicateTicks(n, [0, 2], (25 * TPQ) / 48)).toBe((125 * TPQ) / 48);
  });
});

describe("文", () => {
  it("位置「3.1.0000」、長さ「1.0960」、符号つき「+0.0960」「−2」", () => {
    expect(G.positionText(8 * TPQ, METER)).toBe("3.1.0000");
    expect(G.positionText(8 * TPQ + TPQ + TPQ / 2, METER)).toBe("3.2.0960");
    expect(G.lengthText((3 * TPQ) / 2, METER)).toBe("1.0960");
    expect(G.lengthText((3 * TPQ) / 4, METER)).toBe("0.1440");
    expect(G.signedLength(TPQ / 2, METER)).toBe("+0.0960");
    expect(G.signedLength(-TPQ, METER)).toBe("−1.0000");
    expect(G.signed(2)).toBe("+2");
    expect(G.signed(-12)).toBe("−12");
    expect(G.barsText(0, 16 * TPQ, METER)).toBe("1–4");
    expect(G.barsText(16 * TPQ, 32 * TPQ, METER)).toBe("5–8");
  });
});

describe("CC の札の数", () => {
  it("サステインは踏んだ回数、ピッチベンドとモジュレーションは点の数、ほかの CC は番号", () => {
    const tk = take([], 1, {
      controlTicks: Uint32Array.from([0, 10, 20, 30, 40, 50, 60, 70]),
      controlKinds: Uint8Array.from([0, 0, 0, 0, 1, 1, 0, 0]),
      controlNumbers: Uint8Array.from([64, 64, 64, 64, 0, 0, 1, 11]),
      controlValues: Int16Array.from([127, 0, 127, 127, 100, -100, 40, 90]),
    });
    expect(G.ccCounts(tk)).toEqual({ sustain: 2, pitchBend: 2, modulation: 1, others: [11], total: 8 });
  });
});
