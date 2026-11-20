// ピアノロールのまわりの純粋な関数: 一覧の数の書きかた、選んだノートのまとめ、鳴っている音、アレンジの横の拡大と全体図の枠。
import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import * as G from "../src/components/arrangeGeom";
import { gridText, noteLengthText, notePositionText, pitchClassName, scaleName } from "../src/components/format";
import { clipNoteIndexes, inRanges, selectedSummary } from "../src/components/pianoroll/notes";
import { sameKeys, soundingKeys } from "../src/components/pianoroll/sounding";
import { ARRANGE_BAR_STEPS, arrangeView, DEFAULT_ARRANGE_BAR, stepFrom, zoomArrangeBy } from "../src/components/pianoroll/view.svelte";
import { noteName } from "../src/components/meterFormat";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const FOUR = { num: 4, den: 4 };
const BAR = 4 * TPQ;
const at = (bar: number, beat: number, tick: number) => (bar - 1) * BAR + (beat - 1) * TPQ + tick;

/** 偽物の場面 pianoRoll（モック A-PianoRoll）の、見せているテイク。 */
async function sceneTake(name = "pianoRoll") {
  const scene = fakeScene(name)!;
  const s = scene.state;
  const clip = s.song.tracks[0]!.clips[0]!;
  const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  return { s, clip, take };
}

describe("ノートの一覧の数（モック A-PianoRoll）", () => {
  it("位置は「小節.拍.ティック」、長さは「拍.ティック」", () => {
    expect(notePositionText(at(3, 1, 0), FOUR)).toBe("3.1.000");
    expect(notePositionText(at(1, 4, TPQ / 2), FOUR)).toBe("1.4.240");
    expect(noteLengthText(4 * TPQ, FOUR)).toBe("4.000");
    expect(noteLengthText((3 * TPQ) / 4, FOUR)).toBe("0.360");
    expect(noteLengthText((3 * TPQ) / 2, FOUR)).toBe("1.240");
  });

  it("8 分の拍子では、拍は 8 分音符（表示は拍の中を 480 に分ける）", () => {
    const sixEight = { num: 6, den: 8 };
    expect(notePositionText((TPQ / 2) * 7, sixEight)).toBe("2.2.000");
    expect(noteLengthText((3 * TPQ) / 4, sixEight)).toBe("1.240");
  });

  it("グリッドと音階の名前", () => {
    expect(gridText(0, false)).toBe("1 小節");
    expect(gridText(16, false)).toBe("1/16");
    expect(gridText(8, true)).toBe("1/8 三連符");
    expect(scaleName(0, "major")).toBe("C メジャー");
    expect(scaleName(9, "minor")).toBe("A マイナー");
    expect(pitchClassName(63)).toBe("D#");
  });
});

describe("選んだノートのまとめ（モック A-PianoRoll の右の欄）", () => {
  it("D4 E4 F4: 位置はいちばん早い 3.1.000、長さはばらばら（null）、ベロシティは 77 – 83", async () => {
    const { s, clip, take } = await sceneTake();
    const sum = selectedSummary(take, clip, s.pianoRoll.selectedNotes, FOUR)!;
    expect(sum).toEqual({ count: 3, position: "3.1.000", keys: "D4 E4 F4", length: null, velocityLow: 77, velocityHigh: 83 });
  });

  it("1 つだけなら長さはその値。何も選んでいなければ null", async () => {
    const { clip, take } = await sceneTake();
    expect(selectedSummary(take, clip, [{ start: 14, end: 15 }], FOUR)).toMatchObject({ count: 1, length: "0.360", velocityLow: 80, velocityHigh: 80 });
    expect(selectedSummary(take, clip, [], FOUR)).toBeNull();
  });

  it("位置は曲の中: クリップが 5 小節目から始まれば、そのぶん後ろ", async () => {
    const { clip, take } = await sceneTake();
    const later = { ...clip, start: clip.start + 4 * BAR, end: clip.end + 4 * BAR };
    expect(selectedSummary(take, later, [{ start: 14, end: 15 }], FOUR)!.position).toBe("7.1.000");
  });

  it("選んだ範囲の中にあるかを二分探索で見る", () => {
    const ranges = [
      { start: 2, end: 4 },
      { start: 10, end: 11 },
    ];
    expect([0, 2, 3, 4, 9, 10, 11].map((i) => inRanges(ranges, i))).toEqual([false, true, true, false, false, true, false]);
  });

  it("一覧に出すのは、クリップの範囲にかかるノートだけ", () => {
    const take = { starts: [0, 100, 2000], lengths: [50, 50, 10] };
    expect([...clipNoteIndexes({ start: 0, end: 1000, takeOffset: 0 }, take)]).toEqual([0, 1]);
    expect([...clipNoteIndexes({ start: 0, end: 1000, takeOffset: -60 }, take)]).toEqual([1]);
  });
});

describe("鳴っている音（モック F-PRPlay「鳴っている音 C3 E3 G3 C4」）", () => {
  it("4.1.240 で鳴っているのは C3 E3 G3 C4（低い順）", async () => {
    const { clip, take } = await sceneTake("pianoRollPlay");
    expect(soundingKeys(take, clip, at(4, 1, TPQ / 2)).map(noteName)).toEqual(["C3", "E3", "G3", "C4"]);
  });

  it("ノートの終わりちょうどでは鳴っていない。クリップの外では鳴らない", () => {
    const take = { id: 7, starts: [0, 0], lengths: [TPQ, 2 * TPQ], keys: [64, 60] };
    const clip = { start: 0, end: BAR, takeOffset: 0, takeId: 7 };
    expect(soundingKeys(take, clip, 0)).toEqual([60, 64]);
    expect(soundingKeys(take, clip, TPQ)).toEqual([60]);
    expect(soundingKeys(take, { ...clip, end: 400 }, 400)).toEqual([]);
    expect(soundingKeys(take, { ...clip, start: BAR, end: 2 * BAR }, BAR)).toEqual([60, 64]);
    // クリップがテイクの頭を切っていると、始まりが切られた所にあるノートは鳴らない（Rust の track_events と同じ）。
    expect(soundingKeys(take, { ...clip, takeOffset: -TPQ }, 0)).toEqual([]);
    expect(soundingKeys({ ...take, starts: [0, TPQ] }, { ...clip, takeOffset: -TPQ }, 0)).toEqual([60]);
  });

  it("クリップのテイクを取りにいっている間（前のテイクのまま）や、テイクもクリップもないときは何も鳴らない", () => {
    const take = { id: 7, starts: [0], lengths: [TPQ], keys: [60] };
    const clip = { start: 0, end: BAR, takeOffset: 0, takeId: 8 };
    expect(soundingKeys(take, clip, 0)).toEqual([]);
    expect(soundingKeys(null, clip, 0)).toEqual([]);
    expect(soundingKeys(take, null, 0)).toEqual([]);
  });

  it("同じ鍵が 2 つ重なっても 1 つ", () => {
    expect(soundingKeys({ id: 1, starts: [0, 10], lengths: [100, 100], keys: [60, 60] }, { start: 0, end: 500, takeOffset: 0, takeId: 1 }, 50)).toEqual([60]);
    expect(sameKeys([60, 64], [60, 64])).toBe(true);
    expect(sameKeys([60], [60, 64])).toBe(false);
  });
});

describe("アレンジの横の拡大 ・ 縮小", () => {
  it("段は 64 をまん中に、下に 6 段、上に 4 段（少なくとも 4 段ずつ）", () => {
    const i = ARRANGE_BAR_STEPS.indexOf(DEFAULT_ARRANGE_BAR);
    expect(i).toBeGreaterThanOrEqual(4);
    expect(ARRANGE_BAR_STEPS.length - 1 - i).toBeGreaterThanOrEqual(4);
    expect([...ARRANGE_BAR_STEPS]).toEqual([...ARRANGE_BAR_STEPS].sort((a, b) => a - b));
  });

  it("1 段ずつ動き、はしで止まる", () => {
    expect(stepFrom(ARRANGE_BAR_STEPS, 64, 1, 64)).toBe(91);
    expect(stepFrom(ARRANGE_BAR_STEPS, 8, -1, 64)).toBe(8);
    expect(stepFrom(ARRANGE_BAR_STEPS, 999, 1, 64)).toBe(91);
    arrangeView.barPx = 256;
    expect(zoomArrangeBy(1)).toBe(false);
    expect(zoomArrangeBy(-1)).toBe(true);
    expect(arrangeView.barPx).toBe(181);
    arrangeView.barPx = DEFAULT_ARRANGE_BAR;
  });

  it("縮小して小節の番号が重なるときは、間引く（1・2・4・8 小節おき）", () => {
    expect(G.labelEvery(64, 32)).toBe(1);
    expect(G.labelEvery(23, 32)).toBe(2);
    expect(G.labelEvery(8, 32)).toBe(4);
  });

  it("Mod ＋ ホイール: 上へ回すと拡大。1 回で 1 段まで。細かい動きはたまってから", () => {
    expect(G.wheelZoomSteps(0, -100, 0)).toEqual({ steps: 1, acc: 0 });
    expect(G.wheelZoomSteps(0, 3, 1)).toEqual({ steps: -1, acc: 0 });
    let z = { steps: 0, acc: 0 };
    const steps: number[] = [];
    for (let i = 0; i < 10; i++) {
      z = G.wheelZoomSteps(z.acc, -10, 0);
      steps.push(z.steps);
    }
    expect(steps).toEqual([0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    expect(G.wheelZoomSteps(-40, 10, 0)).toEqual({ steps: 0, acc: 10 });
  });
});

describe("全体図の「見えている範囲の枠」（仕様書 3 章、モック A-PianoRoll・F-PRPlay）", () => {
  const view = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
  const o = { inset: 1, hair: 1, dpr: 1 };

  it("A-PianoRoll: 1〜5 小節（4 小節ぶん）の枠は x 0〜256、上下 1 あけた accent の 1px", () => {
    const rects = G.rangeFrameRects(view, { start: 0, end: 4 * BAR }, 39, o);
    expect(rects).toEqual([
      { x: 0, y: 1, w: 256, h: 1, color: "color.accent" },
      { x: 0, y: 37, w: 256, h: 1, color: "color.accent" },
      { x: 0, y: 1, w: 1, h: 37, color: "color.accent" },
      { x: 255, y: 1, w: 1, h: 37, color: "color.accent" },
    ]);
  });

  it("見えている幅の外の辺は描かない。全部外なら何もない", () => {
    const left = G.rangeFrameRects({ ...view, scrollX: 100 }, { start: 0, end: 4 * BAR }, 39, o);
    expect(left).toHaveLength(3);
    expect(left[0]).toMatchObject({ x: 0, w: 156 });
    expect(G.rangeFrameRects(view, { start: 100 * BAR, end: 104 * BAR }, 39, o)).toEqual([]);
  });

  it("枠が全体図の外へ出たら送る。入っていれば動かさない", () => {
    expect(G.revealRange(100, 300, 0, 1000)).toBe(0);
    expect(G.revealRange(1200, 1400, 0, 1000)).toBe(800);
    expect(G.revealRange(1200, 2600, 0, 1000)).toBe(1200);
    expect(G.revealRange(50, 250, 100, 1000)).toBe(0);
  });
});
