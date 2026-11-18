// ピアノロールの絵（paint.ts）: ノート（左はし gray.10 の 2px、面はベロシティ、選んだら accent）、そろえる前の位置の枠、
// 動かしているノートのもとの位置の枠、ベロシティの棒、鍵盤の列の光る鍵。
import { describe, expect, it } from "vitest";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import type { Drag } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import { noteMarks, paintRoll, paintRollKeys, paintVelocityLane, type RollPicture } from "../src/components/pianoroll/paint";
import { GEOM, take } from "./pianoroll-fixture";

const BOX = { x0: 0, y0: 0, x1: 696, y1: 492 };
const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8 };
const colorOf = (r: number[]) => [r[4], r[5], r[6]].join(",");
const is = (name: Parameters<typeof rgb>[0]) => Array.from(Float32Array.from(rgb(name))).join(",");

function picture(over: Partial<RollPicture> = {}): RollPicture {
  const n = G.shownOf(take([[0, 480, 60, 60], [480, 360, 62, 80], [960, 240, 64, 100]]), 0);
  return {
    g: GEOM,
    sizes: SIZES,
    scale: { root: 0, mode: "major", lock: false },
    meter: { num: 4, den: 4 },
    step: 120,
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

describe("ノート", () => {
  it("左はし gray.10 の 2px と面。線の 1px をあけ、高さ 11", () => {
    const rects = draw(picture());
    const y = (72 - 60) * 12;
    const edge = rects.find((r) => r[0] === 1 && r[1] === y && r[3] === y + 11)!;
    expect(edge.slice(0, 4)).toEqual([1, y, 3, y + 11]);
    expect(colorOf(edge)).toBe(is("color.gray.10"));
    const face = rects.find((r) => r[0] === 3 && r[1] === y)!;
    expect(face[2]).toBe(39);
    expect(colorOf(face)).toBe(is("color.gray.8"));
  });

  it("選んだノートは accent、ベロシティ 100 は gray.10", () => {
    const rects = draw(picture());
    const faceAt = (key: number, x: number) => rects.find((r) => r[0] === x && r[1] === (72 - key) * 12)!;
    expect(colorOf(faceAt(62, 43))).toBe(is("color.accent"));
    expect(colorOf(faceAt(64, 83))).toBe(is("color.gray.10"));
  });

  it("書けない行（スケール固定の音階の外）は gray.0、黒鍵の行は gray.1、白鍵の行は gray.2", () => {
    const rowAt = (key: number, p: RollPicture) =>
      draw(p).find((r) => r[0] === 0 && r[1] === (72 - key) * 12 && r[2] === 696 && r[3] === (72 - key) * 12 + 11)!;
    expect(colorOf(rowAt(61, picture()))).toBe(is("color.gray.1"));
    expect(colorOf(rowAt(60, picture()))).toBe(is("color.gray.2"));
    expect(colorOf(rowAt(61, picture({ scale: { root: 0, mode: "major", lock: true } })))).toBe(is("color.gray.0"));
  });

  it("小節の線 gray.6、拍の線 gray.4、グリッドの線 gray.3", () => {
    const rects = draw(picture());
    const line = (x: number) => rects.find((r) => r[0] === x && r[2] === x + 1 && r[1] === 0 && r[3] === 492)!;
    expect(colorOf(line(0))).toBe(is("color.gray.6"));
    expect(colorOf(line(40))).toBe(is("color.gray.4"));
    expect(colorOf(line(10))).toBe(is("color.gray.3"));
    expect(colorOf(line(160))).toBe(is("color.gray.6"));
  });

  it("クオンタイズしたテイクは、もとと位置がちがうノートだけ、もとの位置に gray.7 の枠。ノートのあとに（上に）描く（B-64）", () => {
    const played = G.shownOf(take([[36, 480, 60], [480, 360, 62], [960, 240, 64]], 3), 0);
    const quantized = G.shownOf(
      take([[0, 480, 60], [480, 360, 62], [960, 240, 64]], 4, { origin: 3, froms: Uint32Array.from([0, 1, 2]) }),
      0,
    );
    const rects = draw(picture({ shown: quantized, origin: played }));
    const frames = rects.filter((r) => colorOf(r) === is("color.gray.7"));
    expect(frames).toHaveLength(4);
    expect(Math.min(...frames.map((r) => r[0]!))).toBe(4);
    expect(rects.slice(-4)).toEqual(frames);
  });

  it("そろえる前の位置の枠は、選んでいるノートの分だけ accent（B-64）", () => {
    const played = G.shownOf(take([[36, 480, 60], [500, 360, 62], [960, 240, 64]], 3), 0);
    const quantized = G.shownOf(
      take([[0, 480, 60], [480, 360, 62], [960, 240, 64]], 4, { origin: 3, froms: Uint32Array.from([0, 1, 2]) }),
      0,
    );
    const rects = draw(picture({ shown: quantized, origin: played, selected: Uint8Array.from([0, 1, 0]) }));
    const gray = rects.filter((r) => colorOf(r) === is("color.gray.7"));
    expect(gray).toHaveLength(4);
    expect(Math.min(...gray.map((r) => r[0]!))).toBe(4);
    const accentFrames = rects.slice(-4);
    expect(accentFrames.every((r) => colorOf(r) === is("color.accent"))).toBe(true);
    expect(Math.min(...accentFrames.map((r) => r[0]!))).toBe(Math.round((500 * 40) / 480) + 1);
  });

  it("動かしている間は、もとの位置が accent の枠だけになり、ほかのノートのあとに描く（ノートは動く面が描く。B-64）", () => {
    const drag: Drag = {
      kind: "move",
      press: { x: 0, y: 0, moved: true },
      notes: [0],
      grab: 0,
      copy: false,
      ticks: 120,
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
    // 複製（Alt）のときは、もとのノートはそのまま
    expect(noteMarks(3, { ...drag, copy: true }, -1)).toEqual(new Uint8Array(3));
  });

  it("消す道具で乗せているノートは gray.7", () => {
    const rects = draw(picture(), noteMarks(3, null, 2));
    const face = rects.find((r) => r[0] === 83 && r[1] === (72 - 64) * 12)!;
    expect(colorOf(face)).toBe(is("color.gray.7"));
  });
});

describe("ベロシティの帯と鍵盤の列", () => {
  it("棒は幅 3、上はしに幅 7 の横線、127 で高さ 64。動かしている間は、もとの高さに gray.7 の横線", () => {
    const box = { x0: 0, y0: 0, x1: 696, y1: 71 };
    const out = new Batch();
    out.clip(0, 0, 696, 71);
    const drag: Drag = { kind: "velocity", press: { x: 0, y: 0, moved: true }, notes: [2], grab: 2, delta: 27 };
    paintVelocityLane(out, box, picture({ drag }), 1);
    const rects = out.rects();
    const bar0 = rects.find((r) => r[0] === 1 && r[2] === 4)!;
    expect(bar0.slice(1, 4)).toEqual([71 - Math.round((60 * 64) / 127), 4, 71]);
    // 横線は棒の 2 つ左から幅 7（左はしは面の外なので切れる）
    const cap0 = rects.find((r) => r[2] === 6 && r[3]! - r[1]! === 1)!;
    expect(cap0.slice(0, 2)).toEqual([0, bar0[1]]);
    const bar2 = rects.find((r) => r[0] === 81 && r[2] === 84)!;
    expect(bar2[1]).toBe(71 - 64);
    const grayCap = rects.find((r) => colorOf(r) === is("color.gray.7"))!;
    expect(grayCap.slice(0, 4)).toEqual([79, 71 - Math.round((100 * 64) / 127), 86, 71 - Math.round((100 * 64) / 127) + 1]);
  });

  it("鍵盤の列: 白鍵 gray.5、黒鍵 gray.0、光る鍵 accent", () => {
    const out = new Batch();
    out.clip(0, 0, 55, 492);
    paintRollKeys(out, { x0: 0, y0: 0, x1: 55, y1: 492 }, GEOM, (key) => key === 60, 1);
    const at = (key: number) => out.rects().find((r) => r[1] === (72 - key) * 12 && r[3] === (72 - key) * 12 + 11)!;
    expect(colorOf(at(60))).toBe(is("color.accent"));
    expect(colorOf(at(61))).toBe(is("color.gray.0"));
    expect(colorOf(at(62))).toBe(is("color.gray.5"));
  });
});
