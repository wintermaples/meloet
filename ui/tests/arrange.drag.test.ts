// アレンジのクリップのドラッグ（仕様書 3 章「アレンジのクリップ」、4 章、モック R3-Arrange）: 決まり（arrangeDrag.ts）と、
// Arrange を mount してマウスで動かしたときに送る物、ドラッグ中の絵、Mod D、偽物の Rust の答え。
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, Command, TrackView } from "../src/bridge";
import { arrangeOf, fakeState, reduceClips } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import * as CD from "../src/components/arrangeDrag";
import * as G from "../src/components/arrangeGeom";
import * as PG from "../src/components/pianoroll/geom";
import { cancelDrag, dragReading, prView } from "../src/components/pianoroll/view.svelte";
import { watchSlots, type GlRect, type GlSlot } from "../src/gl/slot";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import * as app from "./fake-app.svelte";
import { take as fixtureTake } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 1920;
const WIDTH = 1000;
const METER = { num: 4, den: 4 };

function clip(id: number, name: string, fromBar: number, toBar: number): ClipView {
  return { id, name, start: (fromBar - 1) * BAR, end: (toBar - 1) * BAR, take: 1, takes: 1, takeId: 100 + id, takeOffset: 0, noteCount: 0 };
}
function track(number: number, clips: ClipView[]): TrackView {
  return { number, name: `トラック ${number}`, color: number, instrument: "placeholder", gainDb: 0, armed: false, muted: false, soloed: false, clips };
}

// 1 小節 64 px、行の高さ 64、クリップは行の上下 3 あける（拡大率 100 %）。
const view: G.View = { pxPerTick: 64 / BAR, scrollX: 0, width: WIDTH };
const dims: G.LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
const twoTracks = () => [track(1, [clip(1, "イントロ", 1, 5), clip(3, "Aメロ", 5, 9)]), track(2, [clip(5, "ベース A", 3, 5)])];
const input = (tracks: TrackView[], shift = false): CD.DragInput => ({ view, dims, tracks, step: shift ? null : BAR, stillPx: 4 });
const none = { shift: false, alt: false };

function drag(tracks: TrackView[], from: [number, number], to: [number, number], mods = none): CD.ClipDrag {
  const hit = CD.clipHit(view, dims, tracks, from[0], from[1], 6)!;
  let d = CD.startClipDrag(hit, from[0], from[1], mods);
  d = CD.dragClipTo(d, input(tracks, mods.shift), (from[0] + to[0]) / 2, (from[1] + to[1]) / 2, mods);
  return CD.dragClipTo(d, input(tracks, mods.shift), to[0], to[1], mods);
}

describe("押した所（arrangeDrag.clipHit）", () => {
  it("クリップの左右のはし 6px は長さ、まん中は動かす。せまいクリップは幅の 1/3 まで", () => {
    const tracks = twoTracks();
    expect(CD.clipHit(view, dims, tracks, 257, 30, 6)).toMatchObject({ edge: "start", row: 0 });
    expect(CD.clipHit(view, dims, tracks, 300, 30, 6)).toMatchObject({ edge: null });
    expect(CD.clipHit(view, dims, tracks, 507, 30, 6)).toMatchObject({ edge: "end" });
    expect(CD.clipHit(view, dims, tracks, 300, 1, 6)).toBeNull();
    const narrow = [track(1, [{ ...clip(1, "x", 1, 2), end: 240 }])];
    // 幅 8 px: はしは 8/3 px
    expect(CD.clipHit(view, dims, narrow, 4, 30, 6)!.edge).toBeNull();
    expect(CD.clipHit(view, dims, narrow, 1, 30, 6)!.edge).toBe("start");
  });

  it("カーソル: はしは左右、クリップは動かす（Alt で複製）、ほかは矢印", () => {
    const tracks = twoTracks();
    expect(CD.hoverCursor(CD.clipHit(view, dims, tracks, 507, 30, 6), false)).toBe("ew-resize");
    expect(CD.hoverCursor(CD.clipHit(view, dims, tracks, 300, 30, 6), false)).toBe("move");
    expect(CD.hoverCursor(CD.clipHit(view, dims, tracks, 300, 30, 6), true)).toBe("copy");
    expect(CD.hoverCursor(null, false)).toBe("default");
  });
});

describe("動かす（arrangeDrag）", () => {
  it("グリッド（1 小節）に吸いつき、離すと MoveClip を 1 つ。読み値は「6.1.000（+1 小節）」", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [300, 30], [370, 30]);
    expect(d).toMatchObject({ kind: "move", ticks: BAR, toRow: 0, ok: true });
    expect(CD.clipDragReading(d, METER)).toBe("6.1.000（+1 小節）");
    expect(CD.clipDragCursor(d)).toBe("move");
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "moveClips", clips: [3], ticks: BAR, tracks: 0, copy: false });
  });

  it("Shift で吸いつかない（読み値は小節の小数）", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [300, 30], [330, 30], { shift: true, alt: false });
    expect(d).toMatchObject({ ticks: 900 });
    expect(CD.clipDragReading(d, METER)).toBe("5.2.420（+0.469 小節）");
  });

  it("押しただけ（space.1 より動かない）なら何も送らず、読み値も出さない", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [300, 30], [302, 31]);
    expect(d.press.moved).toBe(false);
    expect(CD.clipDragReading(d, METER)).toBeNull();
    expect(CD.clipDragCommand(d, tracks)).toBeNull();
  });

  it("ほかのクリップと重なる所は「できない」のカーソルで、離しても何も送らない。端が触れるだけは置ける", () => {
    const tracks = twoTracks();
    const over = drag(tracks, [100, 30], [164, 30]);
    expect(over).toMatchObject({ ok: false });
    expect(CD.clipDragCursor(over)).toBe("not-allowed");
    expect(CD.clipDragCommand(over, tracks)).toBeNull();
    expect(CD.placeable(tracks, 1, 0, 2 * BAR, null)).toBe(true);
  });

  it("曲の頭より前へは行かない（0 で止まる。動かなければ何も送らない）", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [100, 30], [10, 30]);
    expect(d).toMatchObject({ ticks: 0, ok: true });
    expect(CD.clipDragCommand(d, tracks)).toBeNull();
    const b = drag(tracks, [140, 94], [0, 94]);
    expect(b).toMatchObject({ ticks: -2 * BAR, ok: true });
  });

  it("ほかのトラックへ。行に吸いつき、トラックの番号で送る。行の外は、いちばん上・下の行", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [300, 30], [300 + 128, 94]);
    expect(d).toMatchObject({ toRow: 1, ok: true });
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "moveClips", clips: [3], ticks: 2 * BAR, tracks: 1, copy: false });
    expect(drag(tracks, [300, 30], [300, 400])).toMatchObject({ toRow: 1 });
    // ベース A（3–4 小節）の所へは置けない
    expect(drag(tracks, [300, 30], [300 - 128, 94])).toMatchObject({ toRow: 1, ok: false });
  });

  it("Alt で複製: もとは残り、写しが動く。読み値「複製 ・ 9.1.000」、カーソル copy。もとの場所には置けない", () => {
    const tracks = twoTracks();
    const alt = { shift: false, alt: true };
    const d = drag(tracks, [300, 30], [300 + 256, 30], alt);
    expect(CD.clipDragReading(d, METER)).toBe("複製 ・ 9.1.000");
    expect(CD.clipDragCursor(d)).toBe("copy");
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "moveClips", clips: [3], ticks: 4 * BAR, tracks: 0, copy: true });
    const same = drag(tracks, [300, 30], [310, 30], alt);
    expect(same).toMatchObject({ ticks: 0, ok: false });
    expect(CD.clipDragCommand(same, tracks)).toBeNull();
  });
});

describe("はしをつめる ・ のばす（arrangeDrag）", () => {
  it("右のはしを 1 小節つめる。読み値「長さ 3.0.000」、離して TrimClip を 1 つ", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [509, 30], [445, 30]);
    expect(d).toMatchObject({ kind: "trim", edge: "end", start: 4 * BAR, end: 7 * BAR });
    expect(CD.clipDragCursor(d)).toBe("ew-resize");
    expect(CD.clipDragReading(d, METER)).toBe("長さ 3.0.000");
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "trimClip", clip: 3, edge: "end", ticks: -BAR });
  });

  it("Shift で吸いつかない（モック R3-Arrange「長さ 2.2.000」）", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [509, 30], [509 - 96, 30], { shift: true, alt: false });
    expect(CD.clipDragReading(d, METER)).toBe("長さ 2.2.000");
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "trimClip", clip: 3, edge: "end", ticks: -1.5 * BAR });
  });

  it("となりのクリップの手前で止まる。左のはしは曲の頭で止まる", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [258, 30], [100, 30]);
    expect(d).toMatchObject({ start: 4 * BAR });
    expect(CD.clipDragCommand(d, tracks)).toBeNull();
    const head = drag(tracks, [1, 30], [-100, 30], { shift: true, alt: false });
    expect(head).toMatchObject({ start: 0 });
    const right = drag(tracks, [254, 30], [600, 30]);
    expect(right).toMatchObject({ end: 4 * BAR });
  });

  it("短くても 1 ティック（吸いつくときはグリッド 1 つ）より短くならない。のばすのは、どこまでも", () => {
    const tracks = twoTracks();
    expect(drag(tracks, [509, 30], [0, 30])).toMatchObject({ start: 4 * BAR, end: 5 * BAR });
    expect(drag(tracks, [509, 30], [0, 30], { shift: true, alt: false })).toMatchObject({ end: 4 * BAR + 1 });
    const out = drag(tracks, [509, 30], [509 + 640, 30]);
    expect(CD.clipDragCommand(out, tracks)).toEqual({ type: "trimClip", clip: 3, edge: "end", ticks: 10 * BAR });
  });

  it("長さの文は 小節 . 拍 . ティック（どれも 0 から）", () => {
    expect(CD.clipLengthText(2.5 * BAR, METER)).toBe("2.2.000");
    expect(CD.clipLengthText(BAR + 120, METER)).toBe("1.0.120");
    expect(CD.clipLengthText(3 * 480 + 240, { num: 3, den: 4 })).toBe("1.0.240");
  });
});

describe("つめたクリップの絵", () => {
  const take = { starts: [0, 960, 1920, 3840], lengths: [480, 1440, 480, 480], keys: [60, 62, 64, 67] };

  it("アレンジ: 始まりがクリップの範囲の中のノートだけ（鳴る物と同じ）。はみ出す終わりは描く所で切る", () => {
    // 頭を 1 小節つめた: テイクの 0 は曲の 1920 より前（offset −1920）
    const trimmed = { start: 1920, end: 3840, takeOffset: -1920 };
    expect(G.takeShapes(trimmed, take)).toEqual([
      { start: 1920, length: 480, key: 64 },
    ]);
    // 終わりをつめた: 2 つ目は始まりが中なので描き、終わりは paintClip が右の枠で切る
    const end = { start: 0, end: 1500, takeOffset: 0 };
    expect(G.takeShapes(end, take).map((n) => n.key)).toEqual([60, 62]);
    const out: GlRect[] = [];
    G.paintClip(out, view, dims, G.clipBox(view, dims, 0, end.start, end.end), G.CLIP_PLAIN("color.track.1"), G.takeShapes(end, take));
    const notes = out.slice(3);
    expect(Math.max(...notes.map((r) => r.x + r.w))).toBeLessThanOrEqual(G.tickToX(view, 1500) - dims.hair);
  });

  it("もとの場所の枠は gray.7 の 1 本線（中は描かない）", () => {
    const out: GlRect[] = [];
    G.paintFrame(out, view, dims, G.clipBox(view, dims, 1, 0, BAR), "color.gray.7");
    expect(out).toEqual([
      { x: 0, y: 67, w: 64, h: 1, color: "color.gray.7" },
      { x: 0, y: 123, w: 64, h: 1, color: "color.gray.7" },
      { x: 0, y: 67, w: 1, h: 57, color: "color.gray.7" },
      { x: 63, y: 67, w: 1, h: 57, color: "color.gray.7" },
    ]);
  });

  it("ピアノロール: 範囲の外に始まるノートは、描かない ・ 押せない ・ 囲めない ・ 鍵の範囲に入れない", () => {
    const t = fixtureTake(take.starts.map((st, i) => [st, take.lengths[i]!, take.keys[i]!]), 9);
    const n = PG.shownOf(t, -1920, { from: 0, to: 1920 });
    const seen: number[] = [];
    PG.eachNoteIn(n, -10000, 10000, (i) => seen.push(i));
    expect(seen).toEqual([2]);
    const g: PG.RollGeom = { scrollTick: 0, ppt: 0.1, topKey: 70, rowH: 10, width: 400, height: 200 };
    expect(PG.hitNote(n, g, PG.tickToX(g, 0) + 2, PG.keyToY(g, 64) + 2, 6)).toEqual({ note: 2, edge: false });
    expect(PG.notesInBox(n, g, 0, 0, 400, 200)).toEqual([2]);
    expect(PG.keyRangeOf(n)).toEqual({ low: 64, high: 64 });
  });
});

describe("偽物の Rust の答え（reduceClips）", () => {
  const state = (): AppState => {
    const s = fakeState();
    s.song.tracks = twoTracks();
    return s;
  };
  it("MoveClip で動かし、写しは新しい id。動かしたクリップ（写し）をえらぶ。重なるときは何もしない", () => {
    const moved = reduceClips(state(), { type: "moveClips", clips: [3], ticks: BAR, tracks: 1, copy: false })!;
    expect(moved.song.tracks[0]!.clips.map((c) => c.id)).toEqual([1]);
    expect(moved.song.tracks[1]!.clips.map((c) => [c.id, c.start])).toEqual([[5, 2 * BAR], [3, 5 * BAR]]);
    expect(moved.arrange.selectedClip).toBe(3);
    const copied = reduceClips(state(), { type: "moveClips", clips: [3], ticks: 4 * BAR, tracks: 0, copy: true })!;
    const ids = copied.song.tracks[0]!.clips.map((c) => c.id);
    expect(ids.slice(0, 2)).toEqual([1, 3]);
    expect(copied.arrange.selectedClip).toBe(ids[2]);
    expect(reduceClips(state(), { type: "moveClips", clips: [1], ticks: BAR, tracks: 0, copy: false })).toBeNull();
  });
  it("DuplicateClip はすぐうしろ。TrimClip はテイクの 0 を動かさない", () => {
    const dup = reduceClips(state(), { type: "duplicateClips", clips: [3] })!;
    expect(dup.song.tracks[0]!.clips.map((c) => [c.start, c.end])).toEqual([[0, 4 * BAR], [4 * BAR, 8 * BAR], [8 * BAR, 12 * BAR]]);
    const trim = reduceClips(state(), { type: "trimClip", clip: 3, edge: "start", ticks: BAR })!;
    expect(trim.song.tracks[0]!.clips[1]).toMatchObject({ start: 5 * BAR, end: 8 * BAR, takeOffset: -BAR });
  });
});

// ---- Arrange を mount して ----

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

describe("Arrange のマウス", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  const slots = new Map<HTMLElement, GlSlot>();
  let stopWatch: () => void = () => {};
  beforeEach(() => {
    app.reset();
    target = document.createElement("div");
    document.body.append(target);
    slots.clear();
    stopWatch = watchSlots({
      attach: (node, slot) => slots.set(node, slot),
      update: (node, slot) => slots.set(node, slot),
      detach: (node) => slots.delete(node),
    });
  });
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    target.remove();
    stopWatch();
  });

  function show(edit: (s: AppState) => void = () => {}): void {
    app.setState(
      app.stateWith((s) => {
        s.song.tracks = twoTracks();
        s.arrange = arrangeOf(3);
        edit(s);
      }),
    );
    mounted = mount(Arrange, { target });
    flushSync();
  }
  const lanes = () => target.querySelector<HTMLElement>(".lanes")!;
  function pointer(type: string, x: number, y: number, init: MouseEventInit = {}): void {
    lanes().dispatchEvent(new MouseEvent(type, { bubbles: true, clientX: x, clientY: y, button: 0, ...init }));
    flushSync();
  }
  const moves = () => app.sent.filter((c: Command) => c.type !== "selectClip");
  const laneRects = () => {
    const node = target.querySelector<HTMLElement>(".lanes > .fill")!;
    const s = slots.get(node);
    return s?.kind === "rects" ? s.rects : [];
  };

  it("押したまま動かす間は何も送らず、見た目だけ: もとの場所は gray.7 の枠、動くクリップは小さな区画を transform で。離して MoveClip 1 つ", () => {
    show();
    pointer("pointerdown", 300, 30);
    pointer("pointermove", 340, 30);
    pointer("pointermove", 366, 30);
    expect(moves()).toEqual([]);
    expect(dragReading.text).toBe("6.1.000（+1 小節）");
    expect(target.querySelector('[data-test="arrange-reading"]')!.textContent!.trim()).toBe("6.1.000（+1 小節）");
    expect(lanes().style.cursor).toBe("move");
    const box = target.querySelector<HTMLElement>(".rec-host .drag-clip")!;
    // jsdom は「translate(64px, 0px)」を「translate(64px)」に書きかえる
    expect(box.style.transform).toBe("translate(64px)");
    expect(box.style.left).toBe("256px");
    expect(slots.get(box)?.kind).toBe("rects");
    const frame = laneRects().filter((r) => r.color === "color.gray.7");
    expect(frame).toHaveLength(4);
    // 大きな面は、ドラッグの量では描きなおさない
    const before = laneRects();
    pointer("pointermove", 430, 30);
    expect(laneRects()).toBe(before);
    expect(box.style.transform).toBe("translate(128px)");
    pointer("pointerup", 366, 30);
    expect(moves()).toEqual([{ type: "moveClips", clips: [3], ticks: 2 * BAR, tracks: 0, copy: false }]);
    expect(dragReading.text).toBeNull();
  });

  it("押した所のクリップをえらぶ（今の作り）。ほかのトラックへ動かすと、トラック 2 の番号で送る", () => {
    show();
    pointer("pointerdown", 100, 30);
    pointer("pointermove", 100, 94);
    pointer("pointermove", 100 + 384, 94);
    pointer("pointerup", 0, 0);
    expect(app.sent).toEqual([
      { type: "selectClip", clip: 1 },
      { type: "moveClips", clips: [1], ticks: 6 * BAR, tracks: 1, copy: false },
    ]);
  });

  it("置けない所は「できない」のカーソルで、離しても何も送らない", () => {
    show();
    pointer("pointerdown", 100, 30);
    pointer("pointermove", 164, 30);
    expect(lanes().style.cursor).toBe("not-allowed");
    pointer("pointerup", 164, 30);
    expect(moves()).toEqual([]);
  });

  it("Esc（cancelDrag）でドラッグの前に戻り、離しても何も送らない", () => {
    show();
    pointer("pointerdown", 300, 30);
    pointer("pointermove", 366, 30);
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(target.querySelector(".drag-clip")).toBeNull();
    expect(laneRects().some((r) => r.color === "color.gray.7")).toBe(false);
    expect(dragReading.text).toBeNull();
    pointer("pointerup", 366, 30);
    expect(moves()).toEqual([]);
    expect(cancelDrag()).toBe(false);
  });

  it("Alt ＋ ドラッグで複製: もとはそのまま描き（枠にしない）、カーソルは copy", () => {
    show();
    pointer("pointerdown", 300, 30, { altKey: true });
    pointer("pointermove", 556, 30, { altKey: true });
    expect(lanes().style.cursor).toBe("copy");
    expect(dragReading.text).toBe("複製 ・ 9.1.000");
    expect(laneRects().some((r) => r.color === "color.gray.7")).toBe(false);
    pointer("pointerup", 556, 30, { altKey: true });
    expect(moves()).toEqual([{ type: "moveClips", clips: [3], ticks: 4 * BAR, tracks: 0, copy: true }]);
  });

  it("はし: 乗せると左右のカーソル。つめている間は行の幅の箱に描き、つめた所はもとの枠の中。離して TrimClip 1 つ", () => {
    show();
    pointer("pointermove", 509, 30);
    expect(lanes().style.cursor).toBe("ew-resize");
    pointer("pointerdown", 509, 30);
    pointer("pointermove", 445, 30);
    expect(dragReading.text).toBe("長さ 3.0.000");
    const box = target.querySelector<HTMLElement>(".drag-clip")!;
    expect([box.style.left, box.style.width, box.style.transform]).toEqual(["0px", `${WIDTH}px`, ""]);
    const rects = (slots.get(box) as Extract<GlSlot, { kind: "rects" }>).rects;
    expect(Math.max(...rects.map((r) => r.x + r.w))).toBe(G.tickToX({ ...view }, 7 * BAR));
    expect(laneRects().filter((r) => r.color === "color.gray.7")).toHaveLength(4);
    pointer("pointerup", 445, 30);
    expect(moves()).toEqual([{ type: "trimClip", clip: 3, edge: "end", ticks: -BAR }]);
  });

  it("録音中とカウントイン中は、えらぶだけでドラッグしない", () => {
    for (const state of ["recording", "countIn"] as const) {
      show((s) => {
        s.transport.state = state;
        s.transport.recording = { id: 1, track: 2, from: 0, take: 1, clip: null, lap: 0 };
      });
      pointer("pointermove", 300, 30);
      expect(lanes().style.cursor).toBe("default");
      pointer("pointerdown", 100, 30);
      pointer("pointermove", 300, 30);
      pointer("pointerup", 300, 30);
      expect(app.sent).toEqual([{ type: "selectClip", clip: 1 }]);
      expect(dragReading.text).toBeNull();
      unmount(mounted!);
      mounted = null;
      app.sent.length = 0;
    }
  });
});

describe("Mod D（仕様書 4 章「複製」）", () => {
  const press = (deps: KeyDeps) => {
    const ev = new KeyboardEvent("keydown", { key: "d", ctrlKey: true, cancelable: true });
    handleKeydown(ev, deps);
    return ev;
  };
  afterEach(() => (prView.focus = "arrange"));

  it("アレンジにフォーカスがあり、クリップをえらんでいれば DuplicateClip。えらんでいなければ何も送らない", () => {
    const state = fakeState();
    const sent: Command[] = [];
    const deps: KeyDeps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" } };
    state.arrange = arrangeOf(3);
    expect(press(deps).defaultPrevented).toBe(true);
    state.arrange = arrangeOf(null);
    press(deps);
    expect(sent).toEqual([{ type: "duplicateClips", clips: [3] }]);
  });

  it("ピアノロールがひらいていても、最後に押したのがアレンジならクリップ。ピアノロールならノートのまま", () => {
    const state = fakeState();
    state.workspace.layout = { ...state.workspace.layout, lower: [{ band: "pianoRoll", height: 640 }] };
    state.arrange = arrangeOf(3);
    state.pianoRoll = { ...state.pianoRoll, take: 7, selectedNotes: [] };
    const sent: Command[] = [];
    const deps: KeyDeps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" }, pianoRoll: () => null };
    prView.focus = "arrange";
    press(deps);
    prView.focus = "pianoRoll";
    press(deps);
    expect(sent).toEqual([{ type: "duplicateClips", clips: [3] }]);
  });

  it("録音中とカウントイン中は送らない", () => {
    const state = fakeState();
    state.arrange = arrangeOf(3);
    state.transport.state = "recording";
    const sent: Command[] = [];
    press({ state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" } });
    expect(sent).toEqual([]);
  });
});
