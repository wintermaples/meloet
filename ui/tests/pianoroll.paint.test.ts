// Piano roll painting (paint.ts): notes (a 2px gray.10 left edge, face by velocity, accent when selected), the border of the position before quantize,
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

function picture(over: Partial<RollPicture> = {}): RollPicture {
  const n = G.shownOf(take([[0, TPQ, 60, 60], [TPQ, (3 * TPQ) / 4, 62, 80], [2 * TPQ, TPQ / 2, 64, 100]]), 0);
  return {
    g: GEOM,
    sizes: SIZES,
    scale: { root: 0, mode: "major", lock: false },
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
  it("a 2px gray.10 left edge and the face. Leaves the 1px line, height 11", () => {
    const rects = draw(picture());
    const y = (72 - 60) * 12;
    const edge = rects.find((r) => r[0] === 1 && r[1] === y && r[3] === y + 11)!;
    expect(edge.slice(0, 4)).toEqual([1, y, 3, y + 11]);
    expect(colorOf(edge)).toBe(is("color.gray.10"));
    const face = rects.find((r) => r[0] === 3 && r[1] === y)!;
    expect(face[2]).toBe(39);
    expect(colorOf(face)).toBe(is("color.gray.8"));
  });

  it("selected notes are accent, velocity 100 is gray.10", () => {
    const rects = draw(picture());
    const faceAt = (key: number, x: number) => rects.find((r) => r[0] === x && r[1] === (72 - key) * 12)!;
    expect(colorOf(faceAt(62, 43))).toBe(is("color.accent"));
    expect(colorOf(faceAt(64, 83))).toBe(is("color.gray.10"));
  });

  it("rows not allowed (outside the scale lock scale) gray.0, black key rows gray.1, white key rows gray.2", () => {
    const rowAt = (key: number, p: RollPicture) =>
      draw(p).find((r) => r[0] === 0 && r[1] === (72 - key) * 12 && r[2] === 696 && r[3] === (72 - key) * 12 + 11)!;
    expect(colorOf(rowAt(61, picture()))).toBe(is("color.gray.1"));
    expect(colorOf(rowAt(60, picture()))).toBe(is("color.gray.2"));
    expect(colorOf(rowAt(61, picture({ scale: { root: 0, mode: "major", lock: true } })))).toBe(is("color.gray.0"));
  });

  it("bar lines gray.6, beat lines gray.4, grid lines gray.3", () => {
    const rects = draw(picture());
    const line = (x: number) => rects.find((r) => r[0] === x && r[2] === x + 1 && r[1] === 0 && r[3] === 492)!;
    expect(colorOf(line(0))).toBe(is("color.gray.6"));
    expect(colorOf(line(40))).toBe(is("color.gray.4"));
    expect(colorOf(line(10))).toBe(is("color.gray.3"));
    expect(colorOf(line(160))).toBe(is("color.gray.6"));
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
    expect(accentFrames.every((r) => colorOf(r) === is("color.accent"))).toBe(true);
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
    expect(frames.every((r) => colorOf(r) === is("color.accent"))).toBe(true);
    expect(Math.min(...frames.map((r) => r[1]!))).toBe(y);
    // With copy (Alt), the original notes stay
    expect(noteMarks(3, { ...drag, copy: true }, -1)).toEqual(new Uint8Array(3));
  });

  it("a note under the erase tool is gray.7", () => {
    const rects = draw(picture(), noteMarks(3, null, 2));
    const face = rects.find((r) => r[0] === 83 && r[1] === (72 - 64) * 12)!;
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

  it("key column: white keys gray.5, black keys gray.0, lit keys accent", () => {
    const out = new Batch();
    out.clip(0, 0, 55, 492);
    paintRollKeys(out, { x0: 0, y0: 0, x1: 55, y1: 492 }, GEOM, (key) => key === 60, 1);
    const at = (key: number) => out.rects().find((r) => r[1] === (72 - key) * 12 && r[3] === (72 - key) * 12 + 11)!;
    expect(colorOf(at(60))).toBe(is("color.accent"));
    expect(colorOf(at(61))).toBe(is("color.gray.0"));
    expect(colorOf(at(62))).toBe(is("color.gray.5"));
  });
});
