// Piano roll shape math (geom.ts): grid, scales, row colors, how it shows when opened, drag amounts, reading text, CC counts.
import { describe, expect, it } from "vitest";
import * as G from "../src/components/pianoroll/geom";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const METER = { num: 4, den: 4 };

describe("grid and scales", () => {
  it("ticks of one grid step (1 bar, 1/2–1/32, triplets are 2/3)", () => {
    expect(G.gridTicks({ division: 0, triplet: false }, METER)).toBe(4 * TPQ);
    expect(G.gridTicks({ division: 0, triplet: false }, { num: 3, den: 4 })).toBe(3 * TPQ);
    expect(G.gridTicks({ division: 16, triplet: false }, METER)).toBe(TPQ / 4);
    expect(G.gridTicks({ division: 16, triplet: true }, METER)).toBe(TPQ / 6);
    expect(G.gridTicks({ division: 32, triplet: true }, METER)).toBe(TPQ / 12);
  });

  it("scales (major, minor) and the rows that scale lock does not allow", () => {
    const cMajor = { root: 0, mode: "major" as const, lock: true };
    expect([60, 62, 64, 65, 67, 69, 71].every((k) => G.inScale(k, cMajor))).toBe(true);
    expect([61, 63, 66, 68, 70].some((k) => G.inScale(k, cMajor))).toBe(false);
    expect(G.inScale(70, { root: 9, mode: "minor" })).toBe(false);
    expect(G.inScale(67, { root: 9, mode: "minor" })).toBe(true);
    expect(G.writable(61, cMajor)).toBe(false);
    expect(G.writable(61, { ...cMajor, lock: false })).toBe(true);
  });

  it("row background: white keys row.white, black keys row.black, rows not allowed row.out", () => {
    const off = { root: 0, mode: "major" as const, lock: false };
    expect(G.rowColor(60, off)).toBe("color.row.white");
    expect(G.rowColor(61, off)).toBe("color.row.black");
    expect(G.rowColor(61, { ...off, lock: true })).toBe("color.row.out");
    expect(G.rowColor(66, { root: 7, mode: "major", lock: true })).toBe("color.row.black");
  });

  it("the note face is gray.8, 9, 10 by velocity", () => {
    expect([G.velocityColor(69), G.velocityColor(70), G.velocityColor(89), G.velocityColor(90)]).toEqual([
      "color.gray.8",
      "color.gray.9",
      "color.gray.9",
      "color.gray.10",
    ]);
  });

  it("note names (C4 = 60)", () => {
    expect([G.noteName(60), G.noteName(50), G.noteName(36), G.noteName(127)]).toEqual(["C4", "D3", "C2", "G9"]);
  });
});

describe("how it shows when opened", () => {
  it("vertical: A-PianoRoll (E5 with 41 rows) and F-PR150 (D4 with 18 rows). Notes C3–C5", () => {
    expect(G.fitTopKey({ low: 48, high: 72 }, 41)).toBe(76);
    expect(G.fitTopKey({ low: 48, high: 72 }, 18)).toBe(62);
    expect(G.fitTopKey(null, 41)).toBe(80);
    expect(G.fitTopKey({ low: 120, high: 127 }, 41)).toBe(127);
    expect(G.fitTopKey({ low: 0, high: 2 }, 41)).toBe(40);
  });

  it("horizontal: the largest step where the clip fits (40 per beat at 1280, 80 at 1920, 40 at 150 % too)", () => {
    expect(G.fitQuarterPx(16 * TPQ, 696, 100)).toBe(40);
    expect(G.fitQuarterPx(16 * TPQ, 1336, 100)).toBe(80);
    expect(G.fitQuarterPx(16 * TPQ, 1016, 150)).toBe(40);
    expect(G.fitQuarterPx(1600 * TPQ, 696, 100)).toBe(10);
  });

  it("opening with the loop on: the loop when it overlaps the clip, else the clip", () => {
    const clip = { start: 16 * TPQ, end: 80 * TPQ };
    const loop = { start: 32 * TPQ, end: 48 * TPQ };
    expect(G.openSpan(clip, loop)).toEqual(loop);
    // Over one end of the clip too: the whole loop.
    expect(G.openSpan(clip, { start: 0, end: 32 * TPQ })).toEqual({ start: 0, end: 32 * TPQ });
    // Only touching, apart, or the loop off.
    expect(G.openSpan(clip, { start: 0, end: 16 * TPQ })).toEqual(clip);
    expect(G.openSpan(clip, { start: 80 * TPQ, end: 96 * TPQ })).toEqual(clip);
    expect(G.openSpan(clip, null)).toEqual(clip);
    // The clip is 64 quarters (step 10 at 696), the loop 16 (step 40).
    expect(G.fitQuarterPx(clip.end - clip.start, 696, 100)).toBe(10);
    const span = G.openSpan(clip, loop);
    expect(G.fitQuarterPx(span.end - span.start, 696, 100)).toBe(40);
  });

  it("row count from adding up the height (11, 34, 38, 41, 49 for windows 448, 720, 768, 800, 1080)", () => {
    expect([140, 412, 460, 492, 588].map((h) => G.wholeRows(h, 12))).toEqual([11, 34, 38, 41, 49]);
  });
});

describe("hit test and box selection", () => {
  const n = G.shownOf(take([[0, TPQ, 60], [TPQ, TPQ, 62], [2 * TPQ, TPQ / 2, 64]]), 4 * TPQ);

  it("on a note, the right 6px (length), empty space", () => {
    const y60 = (72 - 60) * 12 + 5;
    // Note 0 is at x 160–200 (from 4 * TPQ in the song).
    expect(G.hitNote(n, GEOM, 170, y60, 6)).toEqual({ note: 0, edge: false });
    expect(G.hitNote(n, GEOM, 196, y60, 6)).toEqual({ note: 0, edge: true });
    expect(G.hitNote(n, GEOM, 205, y60, 6)).toBeNull();
    expect(G.hitNote(n, GEOM, 170, y60 + 12, 6)).toBeNull();
  });

  it("notes touching the box (in any direction)", () => {
    const y = (k: number) => (72 - k) * 12 + 5;
    expect(G.notesInBox(n, GEOM, 150, y(64), 250, y(60))).toEqual([0, 1, 2]);
    expect(G.notesInBox(n, GEOM, 250, y(60), 150, y(62))).toEqual([0, 1]);
    expect(G.notesInBox(n, GEOM, 0, 0, 100, 100)).toEqual([]);
  });
});

describe("drag amounts", () => {
  const n = G.shownOf(take([[0, TPQ, 60], [TPQ, TPQ, 62], [2 * TPQ, TPQ / 2, 127]]), 0);

  it("move: the start of the held note snaps to the grid. With Shift (no step) it does not snap", () => {
    expect(G.moveDelta(n, [0, 1], 1, (13 * TPQ) / 48, 2, TPQ / 4)).toEqual({ ticks: TPQ / 4, keys: 2 });
    expect(G.moveDelta(n, [0, 1], 1, (5 * TPQ) / 48, 0, TPQ / 4)).toEqual({ ticks: 0, keys: 0 });
    expect(G.moveDelta(n, [0, 1], 1, (5 * TPQ) / 48, 0, null)).toEqual({ ticks: (5 * TPQ) / 48, keys: 0 });
  });

  it("move: not before the song start, not outside the keys", () => {
    expect(G.moveDelta(n, [0, 1], 1, -4 * TPQ, -100, TPQ / 4)).toEqual({ ticks: 0, keys: -60 });
    expect(G.moveDelta(n, [2], 2, 0, 5, TPQ / 4)).toEqual({ ticks: 0, keys: 0 });
  });

  it("length: the end snaps to the grid and is never shorter than 1 tick (one grid step when snapping)", () => {
    expect(G.resizeDelta(n, [0], 0, (5 * TPQ) / 24, TPQ / 4)).toBe(TPQ / 4);
    expect(G.resizeDelta(n, [0], 0, -2 * TPQ, TPQ / 4)).toBe((-3 * TPQ) / 4);
    expect(G.resizeDelta(n, [0, 2], 0, -2 * TPQ, null)).toBe(-(TPQ / 2 - 1));
    expect(G.resizeDelta(n, [0], 0, 7, null)).toBe(7);
  });

  it("draw: from the start of the pressed grid step, at least one grid step", () => {
    expect(G.drawStart((25 * TPQ) / 48, TPQ / 4)).toBe(TPQ / 2);
    expect(G.drawStart((25 * TPQ) / 48, null)).toBe((25 * TPQ) / 48);
    expect(G.drawLength(TPQ / 2, (25 * TPQ) / 48, TPQ / 4, true)).toBe(TPQ / 4);
    expect(G.drawLength(TPQ / 2, (35 * TPQ) / 24, TPQ / 4, true)).toBe(TPQ);
    expect(G.drawLength(TPQ / 2, (35 * TPQ) / 24, TPQ / 4, false)).toBe((23 * TPQ) / 24);
  });

  it("velocity: px moved up become the value. Stops at 1–127", () => {
    expect(G.velocityDelta(32, 64)).toBe(64);
    expect(G.velocityDelta(-6, 64)).toBe(-12);
    expect(G.clampVelocity(200)).toBe(127);
    expect(G.clampVelocity(-5)).toBe(1);
  });

  it("Mod D: the range length rounded up to the grid", () => {
    expect(G.duplicateTicks(n, [0, 1], TPQ / 4)).toBe(2 * TPQ);
    expect(G.duplicateTicks(n, [2], 4 * TPQ)).toBe(4 * TPQ);
    expect(G.duplicateTicks(n, [0, 2], (25 * TPQ) / 48)).toBe((125 * TPQ) / 48);
  });
});

describe("text", () => {
  it("position「3.1.0000」, length「1.0960」, signed「+0.0960」「−2」", () => {
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

describe("CC tag counts", () => {
  it("sustain is the number of presses, pitch bend and modulation the number of points, other CCs the number", () => {
    const tk = take([], 1, {
      controlTicks: Uint32Array.from([0, 10, 20, 30, 40, 50, 60, 70]),
      controlKinds: Uint8Array.from([0, 0, 0, 0, 1, 1, 0, 0]),
      controlNumbers: Uint8Array.from([64, 64, 64, 64, 0, 0, 1, 11]),
      controlValues: Int16Array.from([127, 0, 127, 127, 100, -100, 40, 90]),
    });
    expect(G.ccCounts(tk)).toEqual({ sustain: 2, pitchBend: 2, modulation: 1, others: [11], total: 8 });
  });
});
