// 何個も選んだクリップを、いっしょに動かす決まり（docs/design-notes.md B-71、arrangeDrag.ts）と、囲んで選ぶ。
import { describe, expect, it } from "vitest";
import type { ClipView, TrackView } from "../src/bridge";
import * as CD from "../src/components/arrangeDrag";
import type * as G from "../src/components/arrangeGeom";

const BAR = 1920;

function clip(id: number, fromBar: number, toBar: number): ClipView {
  return { id, name: `c${id}`, start: (fromBar - 1) * BAR, end: (toBar - 1) * BAR, take: 1, takes: 1, takeId: 100 + id, takeOffset: 0, noteCount: 0 };
}
function track(number: number, clips: ClipView[]): TrackView {
  return { id: number, number, name: `t${number}`, color: number, instrument: "sine", sound: { kind: "builtin", instrument: "sine" }, standIn: null, drumKeys: null, gainDb: 0, pan: 0, sends: [null, null], inserts: [null, null, null, null], slots: [null, null, null, null], muted: false, soloed: false, clips };
}

// 1 小節 64 px、行の高さ 64（拡大率 100 %）。
const view: G.View = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
const dims: G.LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
// 行 0: 1–5、5–9。行 1: 5–9。行 2: 1–3、9–11。
const tracks = () => [track(1, [clip(1, 1, 5), clip(3, 5, 9)]), track(2, [clip(41, 5, 9)]), track(3, [clip(43, 1, 3), clip(45, 9, 11)])];
const input = (t: TrackView[], shift = false): CD.DragInput => ({ view, dims, tracks: t, step: shift ? null : BAR, stillPx: 4 });
const none = { shift: false, alt: false };

/** ids をいっしょに、(fx, fy) でつかんで (tx, ty) へ。 */
function drag(ids: number[], from: [number, number], to: [number, number], mods = none) {
  const t = tracks();
  const hit = CD.clipHit(view, dims, t, from[0], from[1], 6)!;
  let d = CD.startClipDrag(hit, from[0], from[1], mods, CD.groupOf(t, ids));
  d = CD.dragClipTo(d, input(t, mods.shift), to[0], to[1], mods);
  return { d, command: CD.clipDragCommand(d, t) };
}

describe("いっしょに動かす（B-71）", () => {
  it("groupOf はトラックの順、始まりの順。ないクリップは落とす", () => {
    expect(CD.groupOf(tracks(), [41, 1, 999, 3]).map((g) => [g.clip.id, g.row])).toEqual([
      [1, 0],
      [3, 0],
      [41, 1],
    ]);
  });

  it("つかんだクリップがグループになければ、それだけを動かす", () => {
    const { command } = drag([41], [50, 160], [114, 160]);
    expect(command).toEqual({ type: "moveClips", clips: [43], ticks: BAR, tracks: 0, copy: false });
  });

  it("選んだぜんぶを同じだけ。つかんだクリップがグリッドに吸いつき、ほかは同じ間でついてくる", () => {
    const { d, command } = drag([3, 41], [300, 30], [400, 30]);
    expect(d.kind === "move" && d.ok).toBe(true);
    expect(command).toEqual({ type: "moveClips", clips: [3, 41], ticks: 2 * BAR, tracks: 0, copy: false });
  });

  it("どれか 1 つでもトラックの外に出るなら「できない」（離しても送らない）", () => {
    // 行 1 と行 2 のクリップを、1 行下へ: 行 2 のクリップが外に出る。
    const { d, command } = drag([41, 45], [300, 100], [300, 164]);
    expect(d.kind === "move" && d.ok).toBe(false);
    expect(CD.clipDragCursor(d)).toBe("not-allowed");
    expect(command).toBeNull();
  });

  it("どれか 1 つでも、ほかのクリップと重なるなら「できない」。いっしょに動かす物どうしは重なってよい", () => {
    // 3（行 0、5–9）と 41（行 1、5–9）を 1 行下へ: 行 2 は 1–3 と 9–11 なので置ける。
    expect(drag([3, 41], [300, 30], [300, 94]).d).toMatchObject({ ok: true, toRow: 1 });
    // 1（行 0、1–5）と 3 を 1 小節うしろへ: 3 は 41 と重ならない（行がちがう）、1 は 3 の元の場所に入るが 3 も動くのでよい。
    expect(drag([1, 3], [100, 30], [164, 30]).d).toMatchObject({ ok: true, ticks: BAR });
    // 1 だけを 1 小節うしろへ: 3 と重なる。
    expect(drag([1], [100, 30], [164, 30]).d).toMatchObject({ ok: false });
  });

  it("複製（Alt）は、もとのクリップもじゃまになる", () => {
    const alt = { shift: false, alt: true };
    expect(drag([1, 3], [100, 30], [164, 30], alt).d).toMatchObject({ ok: false, copy: true });
    expect(drag([3, 41], [300, 30], [556, 30], alt).command).toEqual({ type: "moveClips", clips: [3, 41], ticks: 4 * BAR, tracks: 0, copy: true });
  });

  it("いちばん前のクリップが曲の頭で止まる（つかんだ物ではなく）", () => {
    // 3（5–9）をつかんで、43（行 2、1–3）とともに左へ大きく: 43 が 0 で止まるので、動くのは 0。
    const { d, command } = drag([3, 43], [300, 30], [0, 30]);
    expect(d).toMatchObject({ ticks: 0 });
    expect(command).toBeNull();
  });

  it("groupPlaceable: 端が触れるだけは重ならない", () => {
    const t = tracks();
    expect(CD.groupPlaceable(t, CD.groupOf(t, [43]), BAR, 0, false)).toBe(true);
    expect(CD.groupPlaceable(t, CD.groupOf(t, [43]), BAR + 1, 0, false)).toBe(true);
    expect(CD.groupPlaceable(t, CD.groupOf(t, [43]), 6 * BAR + 1, 0, false)).toBe(false);
  });
});

describe("囲んで選ぶ（clipsInBox）", () => {
  it("四角にかかるクリップ。どちらの角からでも同じ", () => {
    // x 100–300、y 30–100: 行 0 の 1 と 3、行 1 の 41。
    expect(CD.clipsInBox(view, dims, tracks(), 100, 30, 300, 100)).toEqual([1, 3, 41]);
    expect(CD.clipsInBox(view, dims, tracks(), 300, 100, 100, 30)).toEqual([1, 3, 41]);
    // 行 2 の空き（3–9 小節）だけ: なし。
    expect(CD.clipsInBox(view, dims, tracks(), 140, 140, 500, 180)).toEqual([]);
  });
});
