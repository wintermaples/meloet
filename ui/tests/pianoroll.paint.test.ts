// Piano roll painting (paint.ts): notes (a 2px gray.10 left edge, an accent face as strong as the velocity with an accent.ink frame, gray.10 when selected), the border of the position before quantize,
// the border of the old position of moving notes, the velocity bars, and lit keys in the key column.
import { describe, expect, it } from "vitest";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import type { Drag } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import { noteMarks, paintRoll, paintRollKeys, paintVelocityLane, type RollPicture } from "../src/components/pianoroll/paint";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const BOX = { x0: 0, y0: 0, x1: 696, y1: 492 };
const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8 };
const colorOf = (r: number[]) => [r[4], r[5], r[6]].join(",");
const is = (name: Parameters<typeof rgb>[0]) => Array.from(Float32Array.from(rgb(name))).join(",");
/** The accent over gray.1, t of the way (the face of an ordinary note). */
const accentAt = (t: number) => {
  const a = rgb("color.accent");
  const b = rgb("color.gray.1");
  return Array.from(Float32Array.from([0, 1, 2].map((i) => b[i]! + (a[i]! - b[i]!) * t))).join(",");
};

function picture(over: Partial<RollPicture> = {}): RollPicture {
  const n = G.shownOf(take([[0, TPQ, 60, 60], [TPQ, (3 * TPQ) / 4, 62, 80], [2 * TPQ, TPQ / 2, 64, 100]]), 0);
  return {
    g: GEOM,
    sizes: SIZES,
    scale: { root: 0, mode: "major", lock: false, highlight: true },
    meter: { num: 4, den: 4 },
    step: TPQ / 4,
    shown: n,
    origin: null,
    selected: Uint8Array.from([0, 1, 0]),
    live: null,
    drag: null,
    eraseHover: -1,
    ...over,
  };
}

function draw(p: RollPicture, marks: Uint8Array | null = null): number[][] {
  const out = new Batch();
  out.clip(BOX.x0, BOX.y0, BOX.x1, BOX.y1);
  paintRoll(out, BOX, p, marks, 1);
  return out.rects();
}

describe("notes", () => {
  it("the face, a 1px accent.ink frame inside it and a 2px gray.10 left edge on top. Leaves the 1px line, height 11", () => {
    const rects = draw(picture());
    const y = (72 - 60) * 12;
    const face = rects.find((r) => r[0] === 1 && r[1] === y && r[3] === y + 11)!;
    expect(face[2]).toBe(39);
    // Velocity 60: half of the accent.
    expect(colorOf(face)).toBe(accentAt(0.5));
    const frame = rects.filter((r) => colorOf(r) === is("color.accent.ink") && r[0]! >= 1 && r[2]! <= 39 && r[1]! >= y && r[3]! <= y + 11);
    expect(frame.map((r) => r.slice(0, 4))).toEqual([
      [1, y, 39, y + 1],
      [1, y + 10, 39, y + 11],
      [1, y, 2, y + 11],
      [38, y, 39, y + 11],
    ]);
    const edge = rects.find((r) => r[0] === 1 && r[2] === 3 && r[1] === y)!;
    expect(edge.slice(0, 4)).toEqual([1, y, 3, y + 11]);
    expect(colorOf(edge)).toBe(is("color.gray.10"));
    expect(rects.indexOf(edge)).toBeGreaterThan(rects.indexOf(face));
  });

  it("selected notes are gray.10, velocity 100 is all of the accent", () => {
    const rects = draw(picture());
    const faceAt = (key: number, x: number) => rects.find((r) => r[0] === x && r[1] === (72 - key) * 12)!;
    expect(colorOf(faceAt(62, 41))).toBe(is("color.gray.10"));
    expect(colorOf(faceAt(64, 81))).toBe(is("color.accent"));
  });

  it("rows outside a locked scale row.out, black key rows row.black, white key rows row.white", () => {
    const rowAt = (key: number, p: RollPicture) =>
      draw(p).find((r) => r[0] === 0 && r[1] === (72 - key) * 12 && r[2] === 696 && r[3] === (72 - key) * 12 + 11)!;
    const plain = { root: 0, mode: "major", lock: false, highlight: false } as const;
    expect(colorOf(rowAt(61, picture({ scale: plain })))).toBe(is("color.row.black"));
    expect(colorOf(rowAt(60, picture({ scale: plain })))).toBe(is("color.row.white"));
    expect(colorOf(rowAt(61, picture()))).toBe(is("color.row.black"));
    expect(colorOf(rowAt(60, picture()))).toBe(is("color.row.white"));
    expect(colorOf(rowAt(61, picture({ scale: { root: 0, mode: "major", lock: true, highlight: true } })))).toBe(is("color.row.out"));
  });

  it("bar lines line.bar 2 pixels wide (the extra one to the right), beat lines line.beat and grid lines line.grid 1 wide", () => {
    const rects = draw(picture());
    const line = (x: number, w = 1) => rects.find((r) => r[0] === x && r[2] === x + w && r[1] === 0 && r[3] === 492)!;
    expect(colorOf(line(0, 2))).toBe(is("color.line.bar"));
    expect(colorOf(line(40))).toBe(is("color.line.beat"));
    expect(colorOf(line(10))).toBe(is("color.line.grid"));
    expect(colorOf(line(160, 2))).toBe(is("color.line.bar"));
  });

  it("a quantized take shows a gray.7 border at the old position only for notes that moved. Drawn after (on top of) the notes (B-64)", () => {
    const played = G.shownOf(take([[(3 * TPQ) / 40, TPQ, 60], [TPQ, (3 * TPQ) / 4, 62], [2 * TPQ, TPQ / 2, 64]], 3), 0);
    const quantized = G.shownOf(
      take([[0, TPQ, 60], [TPQ, (3 * TPQ) / 4, 62], [2 * TPQ, TPQ / 2, 64]], 4, { origin: 3, froms: Uint32Array.from([0, 1, 2]) }),
      0,
    );
    const rects = draw(picture({ shown: quantized, origin: played }));
    const frames = rects.filter((r) => colorOf(r) === is("color.gray.7"));
    expect(frames).toHaveLength(4);
    expect(Math.min(...frames.map((r) => r[0]!))).toBe(4);
    expect(rects.slice(-4)).toEqual(frames);
  });

  it("the border of the position before quantize is accent only for selected notes (B-64)", () => {
    const played = G.shownOf(take([[(3 * TPQ) / 40, TPQ, 60], [(25 * TPQ) / 24, (3 * TPQ) / 4, 62], [2 * TPQ, TPQ / 2, 64]], 3), 0);
    const quantized = G.shownOf(
      take([[0, TPQ, 60], [TPQ, (3 * TPQ) / 4, 62], [2 * TPQ, TPQ / 2, 64]], 4, { origin: 3, froms: Uint32Array.from([0, 1, 2]) }),
      0,
    );
    const rects = draw(picture({ shown: quantized, origin: played, selected: Uint8Array.from([0, 1, 0]) }));
    const gray = rects.filter((r) => colorOf(r) === is("color.gray.7"));
    expect(gray).toHaveLength(4);
    expect(Math.min(...gray.map((r) => r[0]!))).toBe(4);
    const accentFrames = rects.slice(-4);
    expect(accentFrames.every((r) => colorOf(r) === is("color.accent.ink"))).toBe(true);
    expect(Math.min(...accentFrames.map((r) => r[0]!))).toBe(Math.round(((25 * TPQ) / 24) * GEOM.ppt) + 1);
  });

  it("while moving, the old position is only an accent border, drawn after the other notes (the moving surface draws the notes, B-64)", () => {
    const drag: Drag = {
      kind: "move",
      press: { x: 0, y: 0, moved: true },
      notes: [0],
      grab: 0,
      copy: false,
      ticks: TPQ / 4,
      keys: 0,
      selectOnClick: false,
    };
    const marks = noteMarks(3, drag, -1);
    expect([...marks!]).toEqual([1, 0, 0]);
    const rects = draw(picture({ drag }), marks);
    const y = (72 - 60) * 12;
    expect(rects.filter((r) => r[1] === y && colorOf(r) === is("color.gray.10"))).toEqual([]);
    expect(rects.filter((r) => colorOf(r) === is("color.gray.7"))).toHaveLength(0);
    const frames = rects.slice(-4);
    expect(frames.every((r) => colorOf(r) === is("color.accent.ink"))).toBe(true);
    expect(Math.min(...frames.map((r) => r[1]!))).toBe(y);
    // With copy (Alt), the original notes stay
    expect(noteMarks(3, { ...drag, copy: true }, -1)).toEqual(new Uint8Array(3));
  });

  it("a note under the erase tool is gray.7", () => {
    const rects = draw(picture(), noteMarks(3, null, 2));
    const face = rects.find((r) => r[0] === 81 && r[1] === (72 - 64) * 12)!;
    expect(colorOf(face)).toBe(is("color.gray.7"));
  });
});

describe("velocity band and key column", () => {
  it("bars are 3 wide with a 7 wide line at the top, height 64 at 127. While moving, a gray.7 line at the old height", () => {
    const box = { x0: 0, y0: 0, x1: 696, y1: 71 };
    const out = new Batch();
    out.clip(0, 0, 696, 71);
    const drag: Drag = { kind: "velocity", press: { x: 0, y: 0, moved: true }, notes: [2], grab: 2, delta: 27 };
    paintVelocityLane(out, box, picture({ drag }), 1);
    const rects = out.rects();
    const bar0 = rects.find((r) => r[0] === 1 && r[2] === 4)!;
    expect(bar0.slice(1, 4)).toEqual([71 - Math.round((60 * 64) / 127), 4, 71]);
    // The line is 7 wide from 2 left of the bar (the left end is outside the surface, so it is cut)
    const cap0 = rects.find((r) => r[2] === 6 && r[3]! - r[1]! === 1)!;
    expect(cap0.slice(0, 2)).toEqual([0, bar0[1]]);
    const bar2 = rects.find((r) => r[0] === 81 && r[2] === 84)!;
    expect(bar2[1]).toBe(71 - 64);
    const grayCap = rects.find((r) => colorOf(r) === is("color.gray.7"))!;
    expect(grayCap.slice(0, 4)).toEqual([79, 71 - Math.round((100 * 64) / 127), 86, 71 - Math.round((100 * 64) / 127) + 1]);
  });

  it("key column: white keys key.white, black keys key.black, lit keys accent with a 2px accent.ink mark on the left", () => {
    const out = new Batch();
    out.clip(0, 0, 55, 492);
    paintRollKeys(out, { x0: 0, y0: 0, x1: 55, y1: 492 }, GEOM, (key) => key === 60, 1);
    const at = (key: number) => out.rects().find((r) => r[1] === (72 - key) * 12 && r[3] === (72 - key) * 12 + 11)!;
    expect(colorOf(at(60))).toBe(is("color.accent"));
    const mark = out.rects().find((r) => r[1] === (72 - 60) * 12 && r[2] === 2)!;
    expect(colorOf(mark)).toBe(is("color.accent.ink"));
    expect(colorOf(at(61))).toBe(is("color.key.black"));
    expect(colorOf(at(62))).toBe(is("color.key.white"));
  });
});
