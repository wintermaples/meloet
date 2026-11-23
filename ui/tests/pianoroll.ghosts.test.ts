// 「ほかのトラックも見る」 (round 12 P2, ghosts.ts): which tracks and clips, where their notes are drawn (by the take offset, only those
// starting inside the clip), the order in the picture (after the grid lines, before the shown notes), gray.6 with no edge, the limit of
// 20 000 rectangles (the farthest from the play line left out), not on drum rows, and that hit tests do not see them.
import { describe, expect, it } from "vitest";
import type { ClipView, TrackView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { toolbarParts, toolbarWithGhost } from "../src/components/fit";
import * as G from "../src/components/pianoroll/geom";
import {
  GHOST_MAX,
  ghostCandidates,
  ghostClips,
  ghostHidden,
  ghostShown,
  ghostTracks,
  ghostWindow,
  ghostWindowIndex,
  shownTrackId,
  toggleGhostTrack,
  type GhostPicture,
} from "../src/components/pianoroll/ghosts";
import { paintRoll, type RollPicture } from "../src/components/pianoroll/paint";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const BAR = 4 * TPQ;
const BOX = { x0: 0, y0: 0, x1: 696, y1: 492 };
const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8, hatch: 12 };
const colorOf = (r: number[]) => [r[4], r[5], r[6]].join(",");
const is = (name: Parameters<typeof rgb>[0]) => Array.from(Float32Array.from(rgb(name))).join(",");
const GRAY6 = is("color.gray.6");

function clip(id: number, start: number, end: number, over: Partial<ClipView> = {}): ClipView {
  return { id, name: "", start, end, take: 1, takes: 1, takeId: id + 100, takeOffset: 0, noteCount: 0, audio: null, comp: [], ...over };
}

function picture(ghosts: GhostPicture | null, over: Partial<RollPicture> = {}): RollPicture {
  return {
    g: GEOM,
    sizes: SIZES,
    scale: { root: 0, mode: "major", lock: false, highlight: false },
    meter: { num: 4, den: 4 },
    step: TPQ / 4,
    shown: G.shownOf(take([[TPQ, TPQ, 64, 100]]), 0),
    origin: null,
    selected: null,
    live: null,
    drag: null,
    eraseHover: -1,
    ghosts,
    ...over,
  };
}

function draw(p: RollPicture): number[][] {
  const out = new Batch();
  out.clip(BOX.x0, BOX.y0, BOX.x1, BOX.y1);
  paintRoll(out, BOX, p, null, 1);
  return out.rects();
}

/** Rectangles of the note height in gray.6: the ghosts (grid lines and rows have other heights or colors). */
const ghostRects = (rects: number[][]) => rects.filter((r) => colorOf(r) === GRAY6 && r[3]! - r[1]! === 11);

describe("which tracks", () => {
  const tracks = fakeScene("pianoRollGhost")!.state.song.tracks as TrackView[];
  const names = (list: TrackView[]) => list.map((t) => t.name);

  it("key tracks that are not drums, other than the one shown (the drum kit and the vocal track are not offered)", () => {
    expect(names(ghostCandidates(tracks, 1))).toEqual(["ベース", "パッド"]);
    expect(names(ghostCandidates(tracks, 2))).toEqual(["ピアノ", "パッド"]);
    const sf2Kit: TrackView = {
      ...tracks[1]!,
      id: 99,
      sound: { kind: "sf2", file: "kit.sf2", bank: 128, program: 0, name: "Standard", family: "drumKit" },
    };
    expect(names(ghostCandidates([...tracks, sf2Kit], 1))).toEqual(["ベース", "パッド"]);
  });

  it("all are shown at first; taking one off and putting it back", () => {
    expect(names(ghostTracks(tracks, 1, []))).toEqual(["ベース", "パッド"]);
    const off = toggleGhostTrack([], 2);
    expect(off).toEqual([2]);
    expect(names(ghostTracks(tracks, 1, off))).toEqual(["パッド"]);
    expect(toggleGhostTrack(off, 2)).toEqual([]);
  });

  it("what was taken off counts only for the song it was taken off in (track ids are per song)", () => {
    expect(ghostHidden({ hidden: [2], serial: 3 }, 3)).toEqual([2]);
    expect(ghostHidden({ hidden: [2], serial: 3 }, 4)).toEqual([]);
  });

  it("the track shown: the clip's, or where writing makes a clip when there is no clip", () => {
    const c = clip(1, 0, BAR);
    expect(shownTrackId(c, tracks[1]!, 1)).toBe(2);
    expect(shownTrackId(null, null, 4)).toBe(4);
    expect(shownTrackId(null, null, null)).toBeNull();
  });
});

describe("which clips", () => {
  it("a window three visible widths wide, one to the left and two to the right, that moves only by whole widths", () => {
    const ppt = GEOM.ppt;
    const width = 696;
    const widthTicks = width / ppt;
    expect(ghostWindowIndex(0, ppt, width)).toBe(0);
    expect(ghostWindowIndex(widthTicks - 1, ppt, width)).toBe(0);
    expect(ghostWindowIndex(widthTicks, ppt, width)).toBe(1);
    expect(ghostWindow(1, ppt, width)).toEqual({ from: 0, to: 3 * widthTicks });
  });

  it("clips that overlap the window; edges that only touch do not; audio clips have no notes", () => {
    const tracks = [
      { clips: [clip(1, 0, BAR), clip(2, BAR, 2 * BAR), clip(3, 3 * BAR, 4 * BAR)] },
      { clips: [clip(4, BAR, 3 * BAR, { audio: {} as ClipView["audio"] }), clip(5, 2 * BAR - 1, 2 * BAR + 1)] },
    ] as unknown as TrackView[];
    expect(ghostClips(tracks, BAR, 3 * BAR).map((c) => c.id)).toEqual([2, 5]);
  });

  it("the take's 0 is at clip start + take offset, and the same object comes back while nothing changed", () => {
    const t = take([[0, TPQ, 60]]);
    const c = clip(1, BAR, 3 * BAR, { takeOffset: -BAR });
    const s = ghostShown(t, c);
    expect([s.base, s.from, s.to]).toEqual([0, BAR, 3 * BAR]);
    expect(ghostShown(t, c, s)).toBe(s);
    expect(ghostShown(t, { ...c, end: 4 * BAR }, s)).not.toBe(s);
  });
});

describe("the picture", () => {
  it("lines up by the take offset (a trimmed clip): notes that start before or at the clip's end are not drawn", () => {
    // The clip is bars 2–3, its first bar trimmed (offset −1 bar): the take's 0 is at the song's 0.
    const t = take([[0, TPQ, 70], [BAR, TPQ, 70], [BAR + 2 * TPQ, TPQ, 69], [3 * BAR, TPQ, 70]]);
    const s = ghostShown(t, clip(1, BAR, 3 * BAR, { takeOffset: -BAR }));
    const rects = ghostRects(draw(picture({ notes: [s], playTick: 0 })));
    // 40 px a beat: bar 2 is 160 px; key 70 is 2 rows below the top row (72).
    expect(rects.map((r) => r.slice(0, 4))).toEqual([
      [161, 24, 199, 35],
      [241, 36, 279, 47],
    ]);
  });

  it("a clip that does not start at 0 and a positive offset", () => {
    const t = take([[0, TPQ, 72]]);
    const s = ghostShown(t, clip(1, TPQ, BAR, { takeOffset: TPQ }));
    expect(ghostRects(draw(picture({ notes: [s], playTick: 0 }))).map((r) => r.slice(0, 2))).toEqual([[81, 0]]);
  });

  it("gray.6 only (no edge), after the grid lines and before the shown notes", () => {
    const s = ghostShown(take([[TPQ, TPQ, 64]]), clip(1, 0, BAR));
    const rects = draw(picture({ notes: [s], playTick: 0 }));
    const y = (72 - 64) * 12;
    const at = rects.findIndex((r) => colorOf(r) === GRAY6 && r[1] === y && r[3] === y + 11);
    expect(rects[at]!.slice(0, 4)).toEqual([41, y, 79, y + 11]);
    // Nothing else is drawn for it: the next rectangle at that place is the shown note's edge (gray.10), drawn later.
    const lastLine = rects.map((r) => r[1] === 0 && r[3] === 492 && r[2]! - r[0]! === 1).lastIndexOf(true);
    const firstNote = rects.findIndex((r) => r[1] === y && colorOf(r) === is("color.gray.10"));
    expect(lastLine).toBeLessThan(at);
    expect(at).toBeLessThan(firstNote);
    expect(rects.filter((r) => colorOf(r) === GRAY6 && r[1] === y)).toHaveLength(1);
  });

  it("nothing outside the visible rows or ticks", () => {
    const t = take([[0, TPQ, 100], [TPQ, TPQ, 64], [100 * BAR, TPQ, 64]]);
    const s = ghostShown(t, clip(1, 0, 200 * BAR));
    expect(ghostRects(draw(picture({ notes: [s], playTick: 0 })))).toHaveLength(1);
  });

  it("not on drum rows", () => {
    const s = ghostShown(take([[TPQ, TPQ, 38]]), clip(1, 0, BAR));
    const drum = { ...GEOM, drum: { keys: [36, 38, 42], top: 0, hitW: 10 } };
    expect(ghostRects(draw(picture({ notes: [s], playTick: 0 }, { g: drum, shown: null })))).toHaveLength(0);
  });

  it(`at most ${GHOST_MAX} rectangles: the farthest from the play line are left out`, () => {
    // 20 010 notes: 20 000 near the play line (bar 2), 10 far from it (at the left edge); each 1 tick long, in the visible rows.
    const near: [number, number, number][] = Array.from({ length: GHOST_MAX }, (_, i) => [BAR + (i % 64), 1, 72 - (i % 40)]);
    const far: [number, number, number][] = Array.from({ length: 10 }, (_, i) => [i, 1, 60]);
    const notes = [...far, ...near].sort((a, b) => a[0] - b[0]);
    const s = ghostShown(take(notes), clip(1, 0, 4 * BAR));
    const rects = ghostRects(draw(picture({ notes: [s], playTick: BAR + 32 })));
    expect(rects).toHaveLength(GHOST_MAX);
    expect(rects.some((r) => r[0]! < 160)).toBe(false);
  });

  it(`at most ${GHOST_MAX} also when the distances are all the same`, () => {
    const notes: [number, number, number][] = Array.from({ length: GHOST_MAX + 7 }, (_, i) => [0, TPQ, 72 - (i % 40)]);
    const s = ghostShown(take(notes), clip(1, 0, BAR));
    expect(ghostRects(draw(picture({ notes: [s], playTick: 0 })))).toHaveLength(GHOST_MAX);
  });
});

describe("they cannot be pressed", () => {
  it("hit tests and box selection read only the shown take", () => {
    // A ghost at 1.2 on key 64; the shown take has nothing there.
    const shown = G.shownOf(take([[3 * TPQ, TPQ, 60]]), 0);
    const x = 50;
    const y = (72 - 64) * 12 + 5;
    expect(G.hitNote(shown, GEOM, x, y, 6)).toBeNull();
    expect(G.notesInBox(shown, GEOM, 0, 0, 100, 200)).toEqual([]);
  });
});

describe("the tool row", () => {
  const w = { base: 800, keys: 40, info: 200 };
  it("the label 「ほかのトラックも見る」 hides first; then the keys and the clip text as without it", () => {
    expect(toolbarWithGhost(1190, w, 150)).toEqual({ keys: true, info: true, ghost: true });
    expect(toolbarWithGhost(1189, w, 150)).toEqual({ keys: true, info: true, ghost: false });
    expect(toolbarWithGhost(1040, w, 150)).toEqual({ keys: true, info: true, ghost: false });
    expect(toolbarWithGhost(1039, w, 150)).toEqual({ keys: false, info: true, ghost: false });
    expect(toolbarWithGhost(999, w, 150)).toEqual({ keys: false, info: false, ghost: false });
    // Whatever the label's width, the keys and the clip text are what toolbarParts gives.
    for (const available of [700, 999, 1000, 1039, 1040, 1189, 1190, 1400]) {
      const { ghost: _, ...rest } = toolbarWithGhost(available, w, 150);
      expect(rest).toEqual(toolbarParts(available, w));
    }
  });
});
