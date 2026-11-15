// アレンジの形の計算（arrangeGeom.ts）。クリップの絵、札の出しかた、目盛のドラッグ → 命令、ページ送り、全体図。
import { describe, expect, it } from "vitest";
import type { TrackView } from "../src/bridge";
import * as G from "../src/components/arrangeGeom";
import type { GlRect } from "../src/gl/slot";

const BAR = 1920;
/** 1 小節 64 px（100 %、mock.arrange.bar）。 */
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
});
const track = (number: number, clips: ReturnType<typeof clip>[], color = number): TrackView => ({
  number,
  name: "ピアノ",
  color,
  instrument: "placeholder",
  gainDb: 0,
  armed: false,
  clips,
});

describe("クリップの絵（仕様書 2 章「クリップ」、モック Main）", () => {
  it("箱: 行の中で上下に inset。下の線（line.hair）の上まで", () => {
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

  it("ふつう: 枠 gray.6、上辺 line.mark はトラックの色、面 gray.4", () => {
    const out: GlRect[] = [];
    G.paintClip(out, view(), dims, G.clipBox(view(), dims, 0, 0, BAR), G.CLIP_PLAIN("color.track.1"), []);
    expect(out).toEqual([
      { x: 0, y: 3, w: 64, h: 57, color: "color.gray.6" },
      { x: 0, y: 3, w: 64, h: 2, color: "color.track.1" },
      { x: 1, y: 5, w: 62, h: 54, color: "color.gray.4" },
    ]);
  });

  it("えらんだ: 枠と上辺が accent。録音中: 枠と上辺と中身が rec", () => {
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

  it("ノート: 名前の行の下に、そのテイクの鍵の範囲で。高い鍵が上。長さは横に", () => {
    const out: GlRect[] = [];
    const notes = [
      { start: 0, length: 480, key: 60 },
      { start: 960, length: 240, key: 72 },
      { start: 1440, length: 960, key: 66 },
    ];
    G.paintClip(out, view(), dims, G.clipBox(view(), dims, 0, 0, BAR), G.CLIP_PLAIN("color.track.1"), notes);
    const drawn = out.slice(3);
    // 上は 3 + 2 + 16 + 4 = 25、下は 60 − 1 − 4 − 2 = 53。
    expect(drawn).toEqual([
      { x: 1, y: 53, w: 15, h: 2, color: "color.gray.8" },
      { x: 32, y: 25, w: 8, h: 2, color: "color.gray.8" },
      { x: 48, y: 39, w: 15, h: 2, color: "color.gray.8" },
    ]);
  });

  it("見えている幅の外は描かず、外へ出たはしは枠を描かない。鍵が 1 つだけならまん中", () => {
    const out: GlRect[] = [];
    G.paintClip(
      out,
      view(128, 200),
      dims,
      G.clipBox(view(128, 200), dims, 0, 0, 8 * BAR),
      G.CLIP_PLAIN("color.track.1"),
      [
        { start: 0, length: 100, key: 60 },
        { start: 4 * BAR, length: 480, key: 60 },
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

  it("テイクのノートは clip.start + takeOffset から。クリップの外は落とす", () => {
    const take = {
      starts: [0, 960, 4000],
      lengths: [480, 480, 480],
      keys: [60, 62, 64],
    };
    const shapes = G.takeShapes({ start: 1920, end: 3840, takeOffset: -960 }, take);
    expect(shapes).toEqual([{ start: 1920, length: 480, key: 62 }]);
  });

  it("録っているノート: 押している鍵（長さ null）は今の位置まで", () => {
    const live = [
      { lap: 0, index: 0, key: 60, velocity: 80, start: 100, length: 50 },
      { lap: 0, index: 1, key: 64, velocity: 80, start: 300, length: null },
    ];
    expect(G.liveShapes(live, 500)).toEqual([
      { start: 100, length: 50, key: 60 },
      { start: 300, length: 200, key: 64 },
    ]);
  });
});

describe("テイクの札（仕様書 2 章「テイクの印と切りかえ」）", () => {
  it("テイクが 1 つなら札なし。2 つ以上で「2/3」。幅 28 未満は 6×6 の四角だけ", () => {
    expect(G.takeMark({ takes: 1 }, 10, 28)).toBe("none");
    expect(G.takeMark({ takes: 3 }, 28, 28)).toBe("tag");
    expect(G.takeMark({ takes: 3 }, 27.5, 28)).toBe("square");
  });
});

describe("目盛のドラッグ → 命令（ヤチヨさんの決め: ループは目盛のドラッグ）", () => {
  const v = view();

  it("小節の線の番号は、いちばん近い線。スクロールしていても", () => {
    expect(G.barLineAt(v, 250, BAR)).toBe(5);
    expect(G.barLineAt(v, 290, BAR)).toBe(6);
    expect(G.barLineAt(view(640), 10, BAR)).toBe(11);
    expect(G.barLineAt(v, -50, BAR)).toBe(1);
  });

  it("新しくドラッグ: 押した所の線から、いまの線まで。逆向きも、短くても 1 小節", () => {
    const d = G.startRulerDrag(v, 256, BAR, null, 8);
    expect(d).toEqual({ kind: "range", anchor: 5 });
    expect(G.dragRange(d, v, 512, BAR)).toEqual({ from: 5, to: 9 });
    expect(G.dragRange(d, v, 130, BAR)).toEqual({ from: 3, to: 5 });
    expect(G.dragRange(d, v, 270, BAR)).toEqual({ from: 5, to: 6 });
    expect(G.dragRange(d, v, 245, BAR)).toEqual({ from: 4, to: 5 });
  });

  it("範囲のはしの近くを押すと、そのはしを動かす", () => {
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

  it("離したとき: 動かしていれば setLoop、押すだけなら locate（小節の頭に寄せるのは Rust）", () => {
    const d = G.startRulerDrag(v, 256, BAR, null, 8);
    expect(G.rulerCommand(d, v, 256, 512, BAR, 4)).toEqual({
      type: "setLoop",
      fromBar: 5,
      toBar: 9,
    });
    expect(G.rulerCommand(d, v, 300, 302, BAR, 4)).toEqual({
      type: "locate",
      tick: 9000,
    });
    expect(G.rulerCommand(d, view(64), 0, 0, BAR, 4)).toEqual({
      type: "locate",
      tick: 1920,
    });
  });

  it("ループの範囲（ティック）→ 小節の線の番号", () => {
    expect(G.loopBars({ start: 4 * BAR, end: 8 * BAR }, BAR)).toEqual({
      from: 5,
      to: 9,
    });
    expect(G.loopBars(null, BAR)).toBeNull();
  });
});

describe("横のスクロールとページ送り（ヤチヨさんの決め）", () => {
  it("線が右はしに来たら 1 画面ぶん送る。見えていれば動かさない。後ろへ戻ったら、その画面へ", () => {
    expect(G.followScroll(500, 0, 1000)).toBe(0);
    expect(G.followScroll(1000, 0, 1000)).toBe(1000);
    expect(G.followScroll(2500, 0, 1000)).toBe(2000);
    expect(G.followScroll(1300, 2000, 1000)).toBe(1000);
    expect(G.followScroll(10, 2000, 1000)).toBe(0);
    expect(G.followScroll(10, 2000, 0)).toBe(2000);
  });

  it("ホイール: 0 より左へは行かない。右は曲の後ろの物が画面のまん中に来るまで", () => {
    const tracks = [track(1, [clip(1, 1, 21)])];
    const max = G.scrollMax(tracks, null, 0, {
      pxPerTick: 64 / BAR,
      width: 1000,
    });
    expect(max).toBe(20 * 64 - 500);
    expect(G.wheelScroll(100, -300, max)).toBe(0);
    expect(G.wheelScroll(700, 300, max)).toBe(max);
    // ページ送りで先にいれば、そこから戻すだけ。
    expect(G.wheelScroll(2000, 10, max)).toBe(2000);
  });
});

describe("押した所のクリップ", () => {
  const tracks = [track(1, [clip(1, 1, 5), clip(3, 5, 9)]), track(2, [clip(4, 2, 3)])];
  it("クリップの箱の中ならその id、すき間や行の外は null", () => {
    expect(G.clipAt(view(), dims, tracks, 300, 30)).toBe(3);
    expect(G.clipAt(view(), dims, tracks, 100, 64 + 30)).toBe(4);
    expect(G.clipAt(view(), dims, tracks, 300, 1)).toBeNull();
    expect(G.clipAt(view(), dims, tracks, 600, 30)).toBeNull();
    expect(G.clipAt(view(), dims, tracks, 100, 64 * 3)).toBeNull();
  });
});

describe("再生の位置の線と、録っているクリップ", () => {
  it("周の頭: 最初の周は録りはじめた所、2 周目からはループの頭", () => {
    expect(G.lapStart(1000, 0, { start: 3840, end: 7680 })).toBe(1000);
    expect(G.lapStart(1000, 2, { start: 3840, end: 7680 })).toBe(3840);
  });

  it("できた小節は大きな面へ。小さな面は、その 1 小節前から今まで（2 小節より広くならない）", () => {
    expect(G.recordingSplit(3840, 3840 + 100, BAR)).toEqual({
      commit: 3840,
      tailFrom: 3840,
    });
    expect(G.recordingSplit(3840, 3840 + BAR + 5, BAR)).toEqual({
      commit: 3840 + BAR,
      tailFrom: 3840,
    });
    expect(G.recordingSplit(3840, 3840 + 3 * BAR + 5, BAR)).toEqual({
      commit: 3840 + 3 * BAR,
      tailFrom: 3840 + 2 * BAR,
    });
  });
});

describe("全体図（仕様書 3 章: 高さ 136 未満）", () => {
  const base = { overviewH: 40, lineH: 4, gap: 2, hair: 1 };
  const tracks = [track(1, [clip(1, 1, 5)]), track(2, [clip(2, 5, 9)])];

  it("トラックごとに、クリップの線（トラックの色）。となりのクリップとの間に gap", () => {
    expect(G.overviewRects(view(), tracks, 40, base, null)).toEqual([
      { x: 0, y: 15, w: 254, h: 4, color: "color.track.1" },
      { x: 256, y: 21, w: 254, h: 4, color: "color.track.2" },
    ]);
  });

  it("高いときは縦にのばす。行が入りきらなければ、行をちぢめる", () => {
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

  it("録っている所は rec の線", () => {
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
