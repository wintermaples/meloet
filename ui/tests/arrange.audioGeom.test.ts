// 歌のクリップの幾何（src/components/arrange/audioClipGeom.ts）: 波形の所、音量の線 ↔ dB、フェードの取っ手と線、
// 160 の行、音のファイルを落とす所。WebGL の係と HTML の係が同じ数を読む。
import { describe, expect, it } from "vitest";
import type { ClipView, TrackView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { clipBox, rowHeight, trackRowAt, type LaneDims, type View } from "../src/components/arrangeGeom";
import {
  audioShapeOf,
  dropFrameBox,
  dropModeAt,
  dropTarget,
  fadeHandleBox,
  fadeLinePoints,
  fadeLineY,
  fadeTicksAt,
  fadeXs,
  fileFrameAt,
  gainAtY,
  gainFactor,
  gainLineY,
  rowHeights,
  waveArea,
} from "../src/components/arrange/audioClipGeom";
import { nearestBeatTick, mapOf } from "../src/time/bars";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const BAR = 4 * TPQ;
// モック R11-Audio: トラックの高さ 64、クリップは上下 3、上辺 2、名前の行 16。1 小節 64 px。
const D: LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
const V: View = { pxPerTick: 64 / BAR, scrollX: 0, width: 1060 };

const scene = fakeScene("audioEdit")!.state;
const vocal = scene.song.tracks.find((t) => t.kind.kind === "vocal")!;
const [left, right] = vocal.clips as [ClipView, ClipView];
const row = scene.song.tracks.indexOf(vocal);

describe("歌のクリップの幾何（モック R11-Audio ・ F7-AudioEdit）", () => {
  it("波形の所: 名前の行の下から、下の枠の上まで。まん中の線はその半分", () => {
    const box = clipBox(V, D, row, right.start, right.end);
    expect(box).toEqual({ x0: 384, x1: 512, y0: row * 64 + 3, y1: row * 64 + 60 });
    const area = waveArea(D, box);
    expect(area).toEqual({ x0: 385, x1: 511, top: box.y0 + 18, bottom: box.y1 - 1, mid: box.y0 + 18 + 19 });
  });

  it("音量の線: 上のはしが +12 dB、下のはしが −60 dB、dB でまっすぐ。y から dB は 0.1 きざみで、範囲の外には出ない", () => {
    const area = waveArea(D, clipBox(V, D, 0, 0, BAR));
    expect([gainLineY(area, 12), gainLineY(area, -60)]).toEqual([area.top, area.bottom]);
    expect(gainLineY(area, -24)).toBe((area.top + area.bottom) / 2);
    expect(gainLineY(area, 40)).toBe(area.top);
    expect([gainAtY(area, area.top), gainAtY(area, area.bottom), gainAtY(area, -100), gainAtY(area, 1000)]).toEqual([12, -60, 12, -60]);
    for (const db of [-60, -24, -3, 0, 6, 12]) expect(gainAtY(area, gainLineY(area, db))).toBeCloseTo(db, 9);
    expect(gainAtY(area, area.top + 7)).toBe(Math.round((12 - (7 / (area.bottom - area.top)) * 72) * 10) / 10);
    expect([gainFactor(0), gainFactor(-60)]).toEqual([1, 0.001]);
    expect(gainFactor(6)).toBeCloseTo(1.9953, 4);
  });

  it("フェードの取っ手: フェードがなければクリップの角。あれば、フェードの切れ目の x をまん中に", () => {
    const leftShape = audioShapeOf(left)!;
    const rightShape = audioShapeOf(right)!;
    expect(leftShape).toEqual({ start: 4 * BAR, end: 6 * BAR, fadeIn: 0, fadeOut: TPQ });
    expect(fadeXs(V, leftShape)).toEqual({ inX: 256, outX: 384 - 16 });
    const box = clipBox(V, D, row, left.start, left.end);
    const area = waveArea(D, box);
    // 入り: フェードなし → 左の角（枠の内がわ）。
    expect(fadeHandleBox(V, D, box, leftShape, "fadeIn", 6)).toEqual({ x0: area.x0, x1: area.x0 + 6, y0: area.top - 2, y1: area.top + 4 });
    // 出: 1 拍（16 px）前がまん中。
    expect(fadeHandleBox(V, D, box, leftShape, "fadeOut", 6)).toMatchObject({ x0: 368 - 3, x1: 368 + 3 });
    // 右のクリップ: 入りが 1/16（4 px）、出はなし → 右の角。
    expect(fadeHandleBox(V, D, clipBox(V, D, row, right.start, right.end), rightShape, "fadeIn", 6)).toMatchObject({ x0: 388 - 3 });
    expect(fadeHandleBox(V, D, clipBox(V, D, row, right.start, right.end), rightShape, "fadeOut", 6)).toMatchObject({ x1: 511 });
    expect(audioShapeOf(scene.song.tracks[0]!.clips[0]!)).toBeNull();
  });

  it("取っ手をひっぱる: 0 から「クリップの長さ − もう片方のフェード」まで", () => {
    const shape = { start: 4 * BAR, end: 6 * BAR, fadeIn: 0, fadeOut: TPQ };
    expect(fadeTicksAt(V, shape, "fadeIn", 256 + 32)).toBe(2 * TPQ);
    expect(fadeTicksAt(V, shape, "fadeIn", 100)).toBe(0);
    expect(fadeTicksAt(V, shape, "fadeIn", 2000)).toBe(2 * BAR - TPQ);
    expect(fadeTicksAt(V, shape, "fadeOut", 384 - 48)).toBe(3 * TPQ);
    expect(fadeTicksAt(V, shape, "fadeOut", 9999)).toBe(0);
    expect(fadeTicksAt(V, { ...shape, fadeIn: BAR }, "fadeOut", 0)).toBe(BAR);
  });

  it("フェードの線: 入りは下のはしから上のはしへ、出は上から下へ。形は fade.ts（曲がっていて、まっすぐな線より上）", () => {
    const shape = { start: 0, end: 2 * BAR, fadeIn: BAR, fadeOut: BAR / 2 };
    const area = waveArea(D, clipBox(V, D, 0, shape.start, shape.end));
    const into = fadeLinePoints(V, area, shape, "fadeIn", 2);
    expect(into.length).toBe(33);
    expect([into[0], into.at(-1)]).toEqual([
      { x: 0, y: area.bottom },
      { x: 64, y: area.top },
    ]);
    expect(into[16]!.y).toBeCloseTo(fadeLineY(area, Math.SQRT1_2), 9);
    expect(into[16]!.y).toBeLessThan((area.top + area.bottom) / 2);
    const out = fadeLinePoints(V, area, shape, "fadeOut", 2);
    expect([out[0], out.at(-1)]).toEqual([
      { x: 96, y: area.top },
      { x: 128, y: area.bottom },
    ]);
    expect(fadeLinePoints(V, area, { ...shape, fadeIn: 0 }, "fadeIn")).toEqual([]);
    expect([fadeLineY(area, 0), fadeLineY(area, 1)]).toEqual([area.bottom, area.top]);
  });

  it("波形の場所: テイクの 0 ティックがファイルの firstFrame。切った右のクリップは、ファイルの途中から", () => {
    const map = mapOf(scene.timeline);
    // 96.00 で 2 小節は 5 秒。
    expect(fileFrameAt(map, left, left.start)).toBeCloseTo(0, 6);
    expect(fileFrameAt(map, left, left.end)).toBeCloseTo(240000, 6);
    expect(right.takeOffset).toBe(-2 * BAR);
    expect(fileFrameAt(map, right, right.start)).toBeCloseTo(240000, 6);
    expect(fileFrameAt(map, right, right.end)).toBeCloseTo(480000, 6);
    expect(right.audio!.frames).toBe(480000);
    expect(fileFrameAt(map, scene.song.tracks[0]!.clips[0]!, 0)).toBe(0);
  });

  it("160 の行: 歌を直しているトラックだけ高く、クリップの箱も押した行もそれに合う", () => {
    const tracks = scene.song.tracks;
    const heights = rowHeights(D, tracks, vocal.id, 160);
    expect(heights).toEqual([64, 64, 64, 64, 64, 160]);
    expect(rowHeights(D, tracks, null, 160)).toEqual([64, 64, 64, 64, 64, 64]);
    const tops = heights.map((_, i) => heights.slice(0, i).reduce((a, b) => a + b, 0));
    const tall: LaneDims = { ...D, tops, heights };
    expect([rowHeight(tall, 5), rowHeight(tall, 0), rowHeight(D, 5)]).toEqual([160, 64, 64]);
    const box = clipBox(V, tall, 5, right.start, right.end);
    expect([box.y0, box.y1]).toEqual([320 + 3, 320 + 160 - 1 - 3]);
    expect([trackRowAt(tall, 320 + 159), trackRowAt(tall, 320 + 160), trackRowAt(tall, 100)]).toEqual([5, -1, 1]);
    const area = waveArea(tall, box);
    expect(area.bottom - area.top).toBe(160 - 6 - 1 - 18 - 1);
  });

  it("落とす所: 歌のトラックの上ならそのトラック、鍵盤のトラックの上やトラックのない所なら新しい歌のトラック。拍に吸いつく", () => {
    const tracks = scene.song.tracks as unknown as TrackView[];
    const map = mapOf(scene.timeline);
    const snap = (tick: number) => nearestBeatTick(map, tick);
    // 「6 歌」の上（行 5）、6 小節目の 2 拍目の少し右。
    expect(dropTarget(V, D, tracks, 5 * 64 + 16 + 5, 5 * 64 + 20, snap)).toEqual({ track: vocal.id, row: 5, tick: 5 * BAR + TPQ });
    // 鍵盤のトラックの上。
    expect(dropTarget(V, D, tracks, 300, 3 * 64 + 20, snap)).toEqual({ track: null, row: 6, tick: 4 * BAR + 3 * TPQ });
    // いちばん下の行より下、左はしより左。
    expect(dropTarget(V, D, tracks, -40, 600, snap)).toEqual({ track: null, row: 6, tick: 0 });
    // 枠: 歌のトラックならその行、新しいトラックならいちばん下の行のすぐ下。
    expect(dropFrameBox(V, D, { track: vocal.id, row: 5, tick: 4 * BAR }, 6, BAR)).toEqual(clipBox(V, D, 5, 4 * BAR, 5 * BAR));
    expect(dropFrameBox(V, D, { track: null, row: 6, tick: 4 * BAR }, 6, 2 * BAR)).toEqual({ x0: 256, x1: 384, y0: 6 * 64 + 3, y1: 7 * 64 - 4 });
    // まん中の箱の左右（30-C）: 窓の左半分が「別々のトラックに」。
    expect([dropModeAt(520, 1280), dropModeAt(639, 1280), dropModeAt(640, 1280), dropModeAt(760, 1280)]).toEqual(["separate", "separate", "inRow", "inRow"]);
  });
});
