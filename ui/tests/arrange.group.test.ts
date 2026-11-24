// Rules for moving many selected clips together (docs/history/design-notes.ja.md B-71, arrangeDrag.ts), and box select.
import { describe, expect, it } from "vitest";
import type { ClipView, TrackView } from "../src/bridge";
import * as CD from "../src/components/arrangeDrag";
import type * as G from "../src/components/arrangeGeom";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const BAR = 4 * TPQ;

function clip(id: number, fromBar: number, toBar: number): ClipView {
  return { id, name: `c${id}`, start: (fromBar - 1) * BAR, end: (toBar - 1) * BAR, take: 1, takes: 1, takeId: 100 + id, takeOffset: 0, noteCount: 0, audio: null, comp: [] };
}
function track(number: number, clips: ClipView[]): TrackView {
  return { id: number, number, name: `t${number}`, color: number, kind: { kind: "keys" }, sound: { kind: "builtin", instrument: "sine" }, standIn: null, drumKeys: null, keyNames: null, gainDb: 0, pan: 0, sends: [null, null], sendDefaults: [null, null], slots: [null, null, null, null], muted: false, soloed: false, lanes: [], clips, arp: { on: false, shape: "up", rate: "sixteenth", triplet: false, octaves: 1, hold: false } };
}

// 1 bar is 64 px, row height 64 (scale 100 %).
const view: G.View = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
const dims: G.LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
// Row 0: 1–5, 5–9. Row 1: 5–9. Row 2: 1–3, 9–11.
const tracks = () => [track(1, [clip(1, 1, 5), clip(3, 5, 9)]), track(2, [clip(41, 5, 9)]), track(3, [clip(43, 1, 3), clip(45, 9, 11)])];
const input = (t: TrackView[], shift = false): CD.DragInput => ({ view, dims, tracks: t, step: shift ? null : BAR, stillPx: 4 });
const none = { shift: false, alt: false };

/** Hold ids together at (fx, fy) and move them to (tx, ty). */
function drag(ids: number[], from: [number, number], to: [number, number], mods = none) {
  const t = tracks();
  const hit = CD.clipHit(view, dims, t, from[0], from[1], 6)!;
  let d = CD.startClipDrag(hit, from[0], from[1], mods, CD.groupOf(t, ids));
  d = CD.dragClipTo(d, input(t, mods.shift), to[0], to[1], mods);
  return { d, command: CD.clipDragCommand(d, t) };
}

describe("move together (B-71)", () => {
  it("groupOf is in track order, then start order. Missing clips are dropped", () => {
    expect(CD.groupOf(tracks(), [41, 1, 999, 3]).map((g) => [g.clip.id, g.row])).toEqual([
      [1, 0],
      [3, 0],
      [41, 1],
    ]);
  });

  it("if the held clip is not in the group, only it moves", () => {
    const { command } = drag([41], [50, 160], [114, 160]);
    expect(command).toEqual({ type: "moveClips", clips: [43], ticks: BAR, tracks: 0, copy: false });
  });

  it("all selected move by the same amount. The held clip snaps to the grid, the others follow at the same distance", () => {
    const { d, command } = drag([3, 41], [300, 30], [400, 30]);
    expect(d.kind === "move" && d.ok).toBe(true);
    expect(command).toEqual({ type: "moveClips", clips: [3, 41], ticks: 2 * BAR, tracks: 0, copy: false });
  });

  it("if even one leaves the tracks, it is 「できない」 (nothing is sent on release)", () => {
    // Clips on rows 1 and 2 go 1 row down: the row 2 clip leaves the tracks.
    const { d, command } = drag([41, 45], [300, 100], [300, 164]);
    expect(d.kind === "move" && d.ok).toBe(false);
    expect(CD.clipDragCursor(d)).toBe("not-allowed");
    expect(command).toBeNull();
  });

  it("if even one overlaps another clip, it is 「できない」. Clips moved together may overlap each other", () => {
    // 3 (row 0, 5–9) and 41 (row 1, 5–9) go 1 row down: row 2 has 1–3 and 9–11, so they fit.
    expect(drag([3, 41], [300, 30], [300, 94]).d).toMatchObject({ ok: true, toRow: 1 });
    // 1 (row 0, 1–5) and 3 go 1 bar later: 3 does not overlap 41 (different row); 1 enters the old place of 3, but 3 moves too, so it is fine.
    expect(drag([1, 3], [100, 30], [164, 30]).d).toMatchObject({ ok: true, ticks: BAR });
    // Only 1 goes 1 bar later: it overlaps 3.
    expect(drag([1], [100, 30], [164, 30]).d).toMatchObject({ ok: false });
  });

  it("a copy (Alt) is also blocked by the original clip", () => {
    const alt = { shift: false, alt: true };
    expect(drag([1, 3], [100, 30], [164, 30], alt).d).toMatchObject({ ok: false, copy: true });
    expect(drag([3, 41], [300, 30], [556, 30], alt).command).toEqual({ type: "moveClips", clips: [3, 41], ticks: 4 * BAR, tracks: 0, copy: true });
  });

  it("the earliest clip stops at the song start (not the held one)", () => {
    // Hold 3 (5–9) and move far left with 43 (row 2, 1–3): 43 stops at 0, so the move is 0.
    const { d, command } = drag([3, 43], [300, 30], [0, 30]);
    expect(d).toMatchObject({ ticks: 0 });
    expect(command).toBeNull();
  });

  it("groupPlaceable: edges that only touch do not overlap", () => {
    const t = tracks();
    expect(CD.groupPlaceable(t, CD.groupOf(t, [43]), BAR, 0, false)).toBe(true);
    expect(CD.groupPlaceable(t, CD.groupOf(t, [43]), BAR + 1, 0, false)).toBe(true);
    expect(CD.groupPlaceable(t, CD.groupOf(t, [43]), 6 * BAR + 1, 0, false)).toBe(false);
  });
});

describe("box select (clipsInBox)", () => {
  it("clips that touch the box. The same from either corner", () => {
    // x 100–300, y 30–100: 1 and 3 on row 0, 41 on row 1.
    expect(CD.clipsInBox(view, dims, tracks(), 100, 30, 300, 100)).toEqual([1, 3, 41]);
    expect(CD.clipsInBox(view, dims, tracks(), 300, 100, 100, 30)).toEqual([1, 3, 41]);
    // Only the empty part of row 2 (bars 3–9): none.
    expect(CD.clipsInBox(view, dims, tracks(), 140, 140, 500, 180)).toEqual([]);
  });
});
