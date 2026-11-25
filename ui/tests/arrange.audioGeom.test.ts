// Geometry of the audio clip (src/components/arrange/audioClipGeom.ts): the waveform area, the volume line <-> dB,
// the fade handles and lines, the 160 row, and where an audio file is dropped. The WebGL side and the HTML side read the same numbers.
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
// mock R11-Audio: track height 64, clip 3 from top and bottom, top edge 2, name row 16. 1 bar is 64 px.
const D: LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
const V: View = { pxPerTick: 64 / BAR, scrollX: 0, width: 1060 };

const scene = fakeScene("audioEdit")!.state;
const vocal = scene.song.tracks.find((t) => t.kind.kind === "vocal")!;
const [left, right] = vocal.clips as [ClipView, ClipView];
const row = scene.song.tracks.indexOf(vocal);

describe("audio clip geometry (mock R11-Audio, F7-AudioEdit)", () => {
  it("waveform area: from below the name row to above the bottom border. The middle line is half of it", () => {
    const box = clipBox(V, D, row, right.start, right.end);
    expect(box).toEqual({ x0: 384, x1: 512, y0: row * 64 + 3, y1: row * 64 + 60 });
    const area = waveArea(D, box);
    expect(area).toEqual({ x0: 385, x1: 511, top: box.y0 + 18, bottom: box.y1 - 1, mid: box.y0 + 18 + 19 });
  });

  it("volume line: the same scale as the fader (spec 3, 39-B). Top edge +12, 0 dB at 20 % from the top, bottom edge −60 (the place of −∞). y to dB in steps of 0.1, never outside the range", () => {
    const area = waveArea(D, clipBox(V, D, 0, 0, BAR));
    const h = area.bottom - area.top;
    expect([gainLineY(area, 12), gainLineY(area, -60)]).toEqual([area.top, area.bottom]);
    expect(gainLineY(area, 0)).toBeCloseTo(area.top + 0.2 * h, 9);
    expect(gainLineY(area, 6)).toBeCloseTo(area.top + 0.1 * h, 9);
    // Table: −3 is 70 %, −6 is 60 %, −12 is 40 %, −24 is 20 %.
    for (const [db, len] of [[-3, 0.7], [-6, 0.6], [-12, 0.4], [-24, 0.2]] as const) expect(gainLineY(area, db)).toBeCloseTo(area.bottom - len * h, 9);
    expect([gainLineY(area, 40), gainLineY(area, -200)]).toEqual([area.top, area.bottom]);
    expect([gainAtY(area, area.top), gainAtY(area, area.bottom), gainAtY(area, -100), gainAtY(area, 1000)]).toEqual([12, -60, 12, -60]);
    for (const db of [-60, -24, -12, -6, -3, 0, 6, 12, -1.5, -40]) expect(gainAtY(area, gainLineY(area, db))).toBeCloseTo(db, 9);
    // The spec's example: if the 40 below the name strip is the scale, 0 dB is 8 down and −3.0 dB is 12 down (y 24 and y 28 with clip height 58).
    const forty = { x0: 0, x1: 100, top: 16, bottom: 56, mid: 36 };
    expect([gainLineY(forty, 0), gainLineY(forty, -3)]).toEqual([24, 28]);
    expect([gainFactor(0), gainFactor(-60)]).toEqual([1, 0.001]);
    expect(gainFactor(6)).toBeCloseTo(1.9953, 4);
  });

  it("fade handle: at the clip's corner without a fade. With a fade, centered on the x where the fade ends", () => {
    const leftShape = audioShapeOf(left)!;
    const rightShape = audioShapeOf(right)!;
    expect(leftShape).toEqual({ start: 4 * BAR, end: 6 * BAR, fadeIn: 0, fadeOut: TPQ });
    expect(fadeXs(V, leftShape)).toEqual({ inX: 256, outX: 384 - 16 });
    const box = clipBox(V, D, row, left.start, left.end);
    const area = waveArea(D, box);
    // Fade in: none -> the left corner (inside the border).
    expect(fadeHandleBox(V, D, box, leftShape, "fadeIn", 6)).toEqual({ x0: area.x0, x1: area.x0 + 6, y0: area.top - 2, y1: area.top + 4 });
    // Fade out: centered 1 beat (16 px) before the end.
    expect(fadeHandleBox(V, D, box, leftShape, "fadeOut", 6)).toMatchObject({ x0: 368 - 3, x1: 368 + 3 });
    // Right clip: fade in 1/16 (4 px), no fade out -> the right corner.
    expect(fadeHandleBox(V, D, clipBox(V, D, row, right.start, right.end), rightShape, "fadeIn", 6)).toMatchObject({ x0: 388 - 3 });
    expect(fadeHandleBox(V, D, clipBox(V, D, row, right.start, right.end), rightShape, "fadeOut", 6)).toMatchObject({ x1: 511 });
    expect(audioShapeOf(scene.song.tracks[0]!.clips[0]!)).toBeNull();
  });

  it("dragging a handle: from 0 to 「クリップの長さ − もう片方のフェード」", () => {
    const shape = { start: 4 * BAR, end: 6 * BAR, fadeIn: 0, fadeOut: TPQ };
    expect(fadeTicksAt(V, shape, "fadeIn", 256 + 32)).toBe(2 * TPQ);
    expect(fadeTicksAt(V, shape, "fadeIn", 100)).toBe(0);
    expect(fadeTicksAt(V, shape, "fadeIn", 2000)).toBe(2 * BAR - TPQ);
    expect(fadeTicksAt(V, shape, "fadeOut", 384 - 48)).toBe(3 * TPQ);
    expect(fadeTicksAt(V, shape, "fadeOut", 9999)).toBe(0);
    expect(fadeTicksAt(V, { ...shape, fadeIn: BAR }, "fadeOut", 0)).toBe(BAR);
  });

  it("fade line: fade in goes from the bottom edge to the top edge, fade out from top to bottom. The shape is fade.ts (curved, above a straight line)", () => {
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

  it("waveform place: tick 0 of the take is the file's firstFrame. A right clip after a cut starts in the middle of the file", () => {
    const map = mapOf(scene.timeline);
    // At 96.00, 2 bars are 5 seconds.
    expect(fileFrameAt(map, left, left.start)).toBeCloseTo(0, 6);
    expect(fileFrameAt(map, left, left.end)).toBeCloseTo(240000, 6);
    expect(right.takeOffset).toBe(-2 * BAR);
    expect(fileFrameAt(map, right, right.start)).toBeCloseTo(240000, 6);
    expect(fileFrameAt(map, right, right.end)).toBeCloseTo(480000, 6);
    expect(right.audio!.frames).toBe(480000);
    expect(fileFrameAt(map, scene.song.tracks[0]!.clips[0]!, 0)).toBe(0);
  });

  it("160 row: only the track whose audio is being edited is taller, and the clip boxes and pressed rows follow it", () => {
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

  it("drop place: over an audio track, that track; over a keyboard track or where there is no track, a new audio track. Snaps to beats", () => {
    const tracks = scene.song.tracks as unknown as TrackView[];
    const map = mapOf(scene.timeline);
    const snap = (tick: number) => nearestBeatTick(map, tick);
    // Over 「6 歌」 (row 5), a little right of beat 2 of bar 6.
    expect(dropTarget(V, D, tracks, 5 * 64 + 16 + 5, 5 * 64 + 20, snap)).toEqual({ track: vocal.id, row: 5, tick: 5 * BAR + TPQ });
    // Over a keyboard track.
    expect(dropTarget(V, D, tracks, 300, 3 * 64 + 20, snap)).toEqual({ track: null, row: 6, tick: 4 * BAR + 3 * TPQ });
    // Below the lowest row, left of the left edge.
    expect(dropTarget(V, D, tracks, -40, 600, snap)).toEqual({ track: null, row: 6, tick: 0 });
    // Frame: for an audio track, that row; for a new track, just below the lowest row.
    expect(dropFrameBox(V, D, { track: vocal.id, row: 5, tick: 4 * BAR }, 6, BAR)).toEqual(clipBox(V, D, 5, 4 * BAR, 5 * BAR));
    expect(dropFrameBox(V, D, { track: null, row: 6, tick: 4 * BAR }, 6, 2 * BAR)).toEqual({ x0: 256, x1: 384, y0: 6 * 64 + 3, y1: 7 * 64 - 4 });
    // The last track has lanes and its 「＋ 帯を足す ▾」 row under it (the track blocks end at 6 * 64 + 2 * 24 + 24): the frame goes below them.
    const bottom = 6 * 64 + 3 * 24;
    expect(dropFrameBox(V, D, { track: null, row: 6, tick: 4 * BAR }, 6, 2 * BAR, bottom)).toEqual({ x0: 256, x1: 384, y0: bottom + 3, y1: bottom + 64 - 4 });
    // Left and right of the middle box (30-C): the left half of the window is 「別々のトラックに」.
    expect([dropModeAt(520, 1280), dropModeAt(639, 1280), dropModeAt(640, 1280), dropModeAt(760, 1280)]).toEqual(["separate", "separate", "inRow", "inRow"]);
  });
});
