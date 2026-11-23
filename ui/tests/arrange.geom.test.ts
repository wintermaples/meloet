// Shape math of the arrange (arrangeGeom.ts). Clip pictures, how tags show, ruler drag → command, page turning, overview.
import { describe, expect, it } from "vitest";
import type { TrackView } from "../src/bridge";
import * as G from "../src/components/arrangeGeom";
import type { GlRect } from "../src/gl/slot";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const BAR = 4 * TPQ;
/** 1 bar is 64 px (100 %, mock.arrange.bar). */
const view = (scrollX = 0, width = 1000): G.View => ({
  pxPerTick: 64 / BAR,
  scrollX,
  width,
});
const dims: G.LaneDims = {
  trackH: 64,
  inset: 3,
  hair: 1,
  mark: 2,
  nameH: 16,
  pad: 4,
  noteH: 2,
};

const clip = (id: number, fromBar: number, toBar: number, takes = 1) => ({
  id,
  name: `c${id}`,
  start: (fromBar - 1) * BAR,
  end: (toBar - 1) * BAR,
  take: 1,
  takes,
  takeId: 100 + id,
  takeOffset: 0,
  noteCount: 0,
  audio: null,
});
const track = (number: number, clips: ReturnType<typeof clip>[], color = number): TrackView => ({
  id: number,
  number,
  name: "ピアノ",
  color,
  muted: false,
  soloed: false,
  kind: { kind: "keys" },
  sound: { kind: "builtin", instrument: "sine" },
  standIn: null,
  drumKeys: null,
  gainDb: 0,
  pan: 0,
  sends: [null, null],
  sendDefaults: [null, null],
  slots: [null, null, null, null],
  lanes: [],
  clips,
});

describe("clip picture (spec 2 (Clip), mock Main)", () => {
  it("box: inset above and below inside the row. Down to just above the bottom line (line.hair)", () => {
    expect(G.clipBox(view(), dims, 1, 4 * BAR, 8 * BAR)).toEqual({
      x0: 256,
      x1: 512,
      y0: 67,
      y1: 124,
    });
    expect(G.clipBox(view(64), dims, 0, 0, BAR)).toEqual({
      x0: -64,
      x1: 0,
      y0: 3,
      y1: 60,
    });
  });

  it("normal: border gray.6, top edge line.mark in the track color, face gray.4", () => {
    const out: GlRect[] = [];
    G.paintClip(out, view(), dims, G.clipBox(view(), dims, 0, 0, BAR), G.CLIP_PLAIN("color.track.1"), []);
    expect(out).toEqual([
      { x: 0, y: 3, w: 64, h: 57, color: "color.gray.6" },
      { x: 0, y: 3, w: 64, h: 2, color: "color.track.1" },
      { x: 1, y: 5, w: 62, h: 54, color: "color.gray.4" },
    ]);
  });

  it("selected: border and top edge accent. Recording: border, top edge and contents rec", () => {
    expect(G.CLIP_SELECTED).toEqual({
      frame: "color.accent",
      top: "color.accent",
      note: "color.gray.8",
    });
    expect(G.CLIP_RECORDING).toEqual({
      frame: "color.rec",
      top: "color.rec",
      note: "color.rec",
    });
    expect(G.trackColor({ color: 7 })).toBe("color.track.2");
  });

  it("notes: below the name line, in the key range of that take. Higher keys on top. Length across", () => {
    const out: GlRect[] = [];
    const notes = [
      { start: 0, length: TPQ, key: 60 },
      { start: 2 * TPQ, length: TPQ / 2, key: 72 },
      { start: 3 * TPQ, length: 2 * TPQ, key: 66 },
    ];
    G.paintClip(out, view(), dims, G.clipBox(view(), dims, 0, 0, BAR), G.CLIP_PLAIN("color.track.1"), notes);
    const drawn = out.slice(3);
    // top is 3 + 2 + 16 + 4 = 25, bottom is 60 − 1 − 4 − 2 = 53.
    expect(drawn).toEqual([
      { x: 1, y: 53, w: 15, h: 2, color: "color.gray.8" },
      { x: 32, y: 25, w: 8, h: 2, color: "color.gray.8" },
      { x: 48, y: 39, w: 15, h: 2, color: "color.gray.8" },
    ]);
  });

  it("muted track: only the inside lines gray.7. Border, color strip and background stay (even when selected)", () => {
    const notes = [{ start: 0, length: TPQ, key: 60 }];
    for (const colors of [G.CLIP_PLAIN("color.track.1"), G.CLIP_SELECTED]) {
      const plain: GlRect[] = [];
      const muted: GlRect[] = [];
      G.paintClip(plain, view(), dims, G.clipBox(view(), dims, 0, 0, BAR), colors, notes);
      G.paintClip(muted, view(), dims, G.clipBox(view(), dims, 0, 0, BAR), G.clipColorsFor(colors, true), notes);
      expect(muted.slice(0, 3)).toEqual(plain.slice(0, 3));
      expect(muted.slice(3).map((r) => r.color)).toEqual(["color.gray.7"]);
      expect(G.clipColorsFor(colors, false)).toBe(colors);
    }
  });

  it("nothing is drawn outside the visible width, and edges that go outside have no border. With only one key, the middle", () => {
    const out: GlRect[] = [];
    G.paintClip(
      out,
      view(128, 200),
      dims,
      G.clipBox(view(128, 200), dims, 0, 0, 8 * BAR),
      G.CLIP_PLAIN("color.track.1"),
      [
        { start: 0, length: 100, key: 60 },
        { start: 4 * BAR, length: TPQ, key: 60 },
      ],
    );
    expect(out[0]).toEqual({
      x: -1,
      y: 3,
      w: 202,
      h: 57,
      color: "color.gray.6",
    });
    expect(out[2]).toEqual({
      x: -1,
      y: 5,
      w: 202,
      h: 54,
      color: "color.gray.4",
    });
    expect(out.slice(3)).toEqual([{ x: 128, y: 39, w: 16, h: 2, color: "color.gray.8" }]);
  });

  it("take notes start from clip.start + takeOffset. Notes outside the clip are dropped", () => {
    const take = {
      starts: [0, 2 * TPQ, 9 * TPQ],
      lengths: [TPQ, TPQ, TPQ],
      keys: [60, 62, 64],
    };
    const shapes = G.takeShapes({ start: BAR, end: 2 * BAR, takeOffset: -2 * TPQ }, take);
    expect(shapes).toEqual([{ start: BAR, length: TPQ, key: 62 }]);
  });

  it("notes being recorded: held keys (length null) go up to the current position", () => {
    const live = [
      { lap: 0, index: 0, key: 60, velocity: 80, start: 100, length: 50 },
      { lap: 0, index: 1, key: 64, velocity: 80, start: 300, length: null },
    ];
    expect(G.liveShapes(live, 500)).toEqual([
      { start: 100, length: 50, key: 60 },
      { start: 300, length: 200, key: 64 },
    ]);
    // when drawing only the growing end: notes that ended before it are not made into shapes.
    expect(G.liveShapes(live, 500, 200)).toEqual([{ start: 300, length: 200, key: 64 }]);
  });
});

describe("take tag (spec 2 (Take mark and switch))", () => {
  it("no tag with one take. 「2/3」 with two or more. Under width 28, only a 6×6 square", () => {
    expect(G.takeMark({ takes: 1 }, 10, 28)).toBe("none");
    expect(G.takeMark({ takes: 3 }, 28, 28)).toBe("tag");
    expect(G.takeMark({ takes: 3 }, 27.5, 28)).toBe("square");
  });
});

describe("ruler drag → command (the owner's decision: the loop is a ruler drag)", () => {
  const v = view();

  it("the bar line number is the nearest line. Even when scrolled", () => {
    expect(G.barLineAt(v, 250, BAR)).toBe(5);
    expect(G.barLineAt(v, 290, BAR)).toBe(6);
    expect(G.barLineAt(view(640), 10, BAR)).toBe(11);
    expect(G.barLineAt(v, -50, BAR)).toBe(1);
  });

  it("new drag: from the line where it was pressed to the current line. Also backwards, and at least 1 bar", () => {
    const d = G.startRulerDrag(v, 256, BAR, null, 8);
    expect(d).toEqual({ kind: "range", anchor: 5 });
    expect(G.dragRange(d, v, 512, BAR)).toEqual({ from: 5, to: 9 });
    expect(G.dragRange(d, v, 130, BAR)).toEqual({ from: 3, to: 5 });
    expect(G.dragRange(d, v, 270, BAR)).toEqual({ from: 5, to: 6 });
    expect(G.dragRange(d, v, 245, BAR)).toEqual({ from: 4, to: 5 });
  });

  it("pressing near an edge of the range moves that edge", () => {
    const loop = { from: 5, to: 9 };
    const right = G.startRulerDrag(v, 515, BAR, loop, 8);
    expect(right).toEqual({ kind: "edge", fixed: 5 });
    expect(G.dragRange(right, v, 704, BAR)).toEqual({ from: 5, to: 12 });
    const left = G.startRulerDrag(v, 250, BAR, loop, 8);
    expect(left).toEqual({ kind: "edge", fixed: 9 });
    expect(G.dragRange(left, v, 64, BAR)).toEqual({ from: 2, to: 9 });
    expect(G.startRulerDrag(v, 384, BAR, loop, 8)).toEqual({
      kind: "range",
      anchor: 7,
    });
  });

  it("on release: setLoop if it moved, locate if only pressed (no snap when step is null)", () => {
    const d = G.startRulerDrag(v, 256, BAR, null, 8);
    expect(G.rulerCommand(d, v, 256, 512, BAR, 4, BAR)).toEqual({
      type: "setLoop",
      fromBar: 5,
      toBar: 9,
    });
    expect(G.rulerCommand(d, v, 300, 302, BAR, 4, null)).toEqual({
      type: "locate",
      tick: (300 * BAR) / 64,
    });
    expect(G.rulerCommand(d, view(64), 0, 0, BAR, 4, null)).toEqual({
      type: "locate",
      tick: BAR,
    });
  });

  it("locate on a press snaps to the lines shown on the ruler (B-61): the nearer bar start, beats when zoomed in", () => {
    const d = G.startRulerDrag(v, 256, BAR, null, 8);
    // 300 px is 300 / 64 bars. With bar steps, the nearer of the start of bar 5 (4 * BAR) and the start of bar 6 (5 * BAR).
    expect(G.rulerCommand(d, v, 300, 300, BAR, 4, BAR)).toEqual({ type: "locate", tick: 5 * BAR });
    expect(G.rulerCommand(d, v, 300, 300, BAR, 4, TPQ)).toEqual({ type: "locate", tick: 19 * TPQ });
    expect(G.rulerCommand(d, v, 300, 300, BAR, 4, 2 * BAR)).toEqual({ type: "locate", tick: 4 * BAR });
  });

  it("snap step: usually 1 bar, every numbered bar when numbers are skipped, a beat when one beat is wider than the gap between numbers", () => {
    expect(G.rulerStep(64, BAR, TPQ, 1, 32)).toBe(BAR);
    expect(G.rulerStep(16, BAR, TPQ, 2, 32)).toBe(2 * BAR);
    expect(G.rulerStep(8, BAR, TPQ, 4, 32)).toBe(4 * BAR);
    expect(G.rulerStep(127, BAR, TPQ, 1, 32)).toBe(BAR);
    expect(G.rulerStep(128, BAR, TPQ, 1, 32)).toBe(TPQ);
    // in 6/8 one beat is an eighth note (TPQ / 2).
    expect(G.rulerStep(192, 3 * TPQ, TPQ / 2, 1, 32)).toBe(TPQ / 2);
  });

  it("beat lines are drawn under the same condition as the ruler snapping to beats (B-61)", () => {
    for (const [barPx, perBar, perBeat] of [[127, BAR, TPQ], [128, BAR, TPQ], [64, BAR, TPQ], [191, 3 * TPQ, TPQ / 2], [192, 3 * TPQ, TPQ / 2]]) {
      expect(G.beatsShown(barPx, perBar, perBeat, 32)).toBe(G.rulerStep(barPx, perBar, perBeat, 1, 32) === perBeat);
    }
    expect(G.beatsShown(127, BAR, TPQ, 32)).toBe(false);
    expect(G.beatsShown(128, BAR, TPQ, 32)).toBe(true);
  });

  it("loop range (ticks) → bar line numbers", () => {
    expect(G.loopBars({ start: 4 * BAR, end: 8 * BAR }, BAR)).toEqual({
      from: 5,
      to: 9,
    });
    expect(G.loopBars(null, BAR)).toBeNull();
  });
});

describe("horizontal scroll and page turning (the owner's decision)", () => {
  it("when the line reaches the right end, turn one screen. Do not move if it is visible. If it went back, go to that screen", () => {
    expect(G.followScroll(500, 0, 1000)).toBe(0);
    expect(G.followScroll(1000, 0, 1000)).toBe(1000);
    expect(G.followScroll(2500, 0, 1000)).toBe(2000);
    expect(G.followScroll(1300, 2000, 1000)).toBe(1000);
    expect(G.followScroll(10, 2000, 1000)).toBe(0);
    expect(G.followScroll(10, 2000, 0)).toBe(2000);
  });

  it("wheel: never left of 0. To the right, until the last thing in the song is in the middle of the screen", () => {
    const tracks = [track(1, [clip(1, 1, 21)])];
    const max = G.scrollMax(tracks, null, 0, {
      pxPerTick: 64 / BAR,
      width: 1000,
    });
    expect(max).toBe(20 * 64 - 500);
    expect(G.wheelScroll(100, -300, max)).toBe(0);
    expect(G.wheelScroll(700, 300, max)).toBe(max);
    // if page turning is already ahead, only go back from there.
    expect(G.wheelScroll(2000, 10, max)).toBe(2000);
  });
});

describe("the clip that was pressed", () => {
  const tracks = [track(1, [clip(1, 1, 5), clip(3, 5, 9)]), track(2, [clip(4, 2, 3)])];
  it("its id inside a clip box, null in a gap or outside the rows", () => {
    expect(G.clipAt(view(), dims, tracks, 300, 30)).toBe(3);
    expect(G.clipAt(view(), dims, tracks, 100, 64 + 30)).toBe(4);
    expect(G.clipAt(view(), dims, tracks, 300, 1)).toBeNull();
    expect(G.clipAt(view(), dims, tracks, 600, 30)).toBeNull();
    expect(G.clipAt(view(), dims, tracks, 100, 64 * 3)).toBeNull();
  });
});

describe("the play line and the clip being recorded", () => {
  it("start of a pass: the first pass from where recording began, from the second pass the loop start", () => {
    expect(G.lapStart(1000, 0, { start: 2 * BAR, end: 4 * BAR })).toBe(1000);
    expect(G.lapStart(1000, 2, { start: 2 * BAR, end: 4 * BAR })).toBe(2 * BAR);
  });

  it("finished bars go to the big surface. The small surface is from 1 bar before them up to now (never wider than 2 bars)", () => {
    expect(G.recordingSplit(2 * BAR, 2 * BAR + 100, BAR)).toEqual({
      commit: 2 * BAR,
      tailFrom: 2 * BAR,
    });
    expect(G.recordingSplit(2 * BAR, 2 * BAR + BAR + 5, BAR)).toEqual({
      commit: 2 * BAR + BAR,
      tailFrom: 2 * BAR,
    });
    expect(G.recordingSplit(2 * BAR, 2 * BAR + 3 * BAR + 5, BAR)).toEqual({
      commit: 2 * BAR + 3 * BAR,
      tailFrom: 2 * BAR + 2 * BAR,
    });
  });
});

describe("overview (spec 3: height under 136)", () => {
  const base = { overviewH: 40, lineH: 4, gap: 2, hair: 1 };
  const tracks = [track(1, [clip(1, 1, 5)]), track(2, [clip(2, 5, 9)])];

  it("for each track, clip lines (the track color). A gap between neighboring clips", () => {
    expect(G.overviewRects(view(), tracks, 40, base, null)).toEqual([
      { x: 0, y: 15, w: 254, h: 4, color: "color.track.1" },
      { x: 256, y: 21, w: 254, h: 4, color: "color.track.2" },
    ]);
  });

  it("when tall, it stretches vertically. If the rows do not fit, they get smaller", () => {
    const tall = G.overviewRects(view(), tracks, 80, base, null);
    expect(tall.map((r) => [r.y, r.h])).toEqual([
      [30, 8],
      [42, 8],
    ]);
    const many = Array.from({ length: 16 }, (_, i) => track(i + 1, [clip(i + 1, 1, 2)]));
    const squeezed = G.overviewRects(view(), many, 40, base, null);
    expect(squeezed).toHaveLength(16);
    expect(squeezed.at(-1)!.y + squeezed.at(-1)!.h).toBeLessThanOrEqual(40);
  });

  it("the part being recorded is a rec line", () => {
    const out = G.overviewRects(view(), tracks, 40, base, {
      track: 2,
      from: 9 * BAR,
      now: 10 * BAR,
    });
    expect(out.at(-1)).toEqual({
      x: 576,
      y: 21,
      w: 62,
      h: 4,
      color: "color.rec",
    });
  });
});
