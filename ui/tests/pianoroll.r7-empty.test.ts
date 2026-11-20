// ノートが 0 のときのピアノロール: 初めて置いたノートで見ている範囲が動かないこと（クリップのない所 ・ ノートが 0 のクリップ）、
// ノートをぜんぶ消しても横の位置が数のままで、目盛が出て、書けること。
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, Command, TakeNotes, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import Roll from "../src/components/pianoroll/Roll.svelte";
import { DEFAULT_ROW_PX, prPanels, prView, pxPerTick, rowHeight } from "../src/components/pianoroll/view.svelte";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const BAR = 4 * TPQ;
const W = 696;
const H = 492;
const proto = HTMLElement.prototype;
const hadW = Object.getOwnPropertyDescriptor(proto, "clientWidth");
const hadH = Object.getOwnPropertyDescriptor(proto, "clientHeight");
beforeAll(() => {
  Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => W });
  Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => H });
});
afterAll(() => {
  if (hadW) Object.defineProperty(proto, "clientWidth", hadW);
  if (hadH) Object.defineProperty(proto, "clientHeight", hadH);
});

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  prPanels.list = true;
  prPanels.inspector = true;
  prView.tool = "select";
  prView.rowPx = DEFAULT_ROW_PX;
  prView.quarterPx = 80;
  prView.topKey = 72;
  prView.scrollTick = 0;
  prView.follow = false;
  prView.shownClip = null;
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  prView.tool = "select";
});

const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const pointer = (el: Element, type: string, init: PointerEventInit) =>
  el.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, ...init }));

function show(): void {
  mounted = mount(Roll, { target, props: {} });
  flushSync();
}

/** 曲（A-PianoRoll）と、そのピアノロールのクリップとトラックとテイク。 */
async function pianoRollSong(): Promise<{ s: AppState; clip: ClipView; track: TrackView; take: TakeNotes }> {
  const s = fakeScene("pianoRoll")!.state;
  const track = s.song.tracks[0]!;
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  return { s, clip, track, take: decodeTake(await createFakeTransport(s).takeNotes(clip.takeId)) };
}

/** 状態を入れかえ、見せているクリップとテイクも合わせる（core の答え ・ 新しいテイクが来たところ）。 */
function arrive(s: AppState, clip: ClipView | null, track: TrackView, t: TakeNotes | null): void {
  app.setState(s);
  shown.setShown({ clip, track, take: t });
  flushSync();
}

/** 書く道具で (x, y) を押して離す。送った addNote を返す。 */
function drawAt(x: number, y: number): Extract<Command, { type: "addNote" }> {
  prView.tool = "draw";
  const area = q(".area")!;
  pointer(area, "pointerdown", { clientX: x, clientY: y });
  pointer(area, "pointerup", { clientX: x, clientY: y });
  const c = [...app.sent].reverse().find((c) => c.type === "addNote");
  expect(c).toBeDefined();
  return c as Extract<Command, { type: "addNote" }>;
}

const viewNow = () => ({ quarterPx: prView.quarterPx, rowPx: prView.rowPx, scrollTick: prView.scrollTick, topKey: prView.topKey });

describe("初めてノートを置いても、見ている範囲は変わらない", () => {
  it("クリップのない所で書く（B-52）: できたクリップは「ひらいた」のではない", async () => {
    const { s, track } = await pianoRollSong();
    const empty = structuredClone(s);
    empty.song.tracks[0]!.clips = [];
    empty.arrange = { ...empty.arrange, selectedClips: [], selectedClip: null, pianoRollClip: null };
    app.setState(empty);
    shown.setShown({ clip: null, track, take: null });
    prView.quarterPx = 40;
    prView.rowPx = 16;
    prView.scrollTick = 3 * BAR;
    prView.topKey = 70;
    show();
    const before = viewNow();
    const rowH = rowHeight(100);
    const y = 5 * rowH + 3;
    const c = drawAt(200, y);
    expect(c.take).toBeNull();
    expect(c.key).toBe(70 - 5);
    expect(Number.isFinite(c.start)).toBe(true);

    // core の答え: 小節の頭から 4 小節のクリップができ、そこに 1 つ。
    const head = Math.floor(c.start / BAR) * BAR;
    const made: ClipView = { id: 900, name: track.name, start: head, end: head + 4 * BAR, take: 1, takes: 1, takeId: 901, takeOffset: 0, noteCount: 1, audio: null };
    const next = structuredClone(empty);
    next.song.tracks[0]!.clips = [made];
    next.arrange = { ...next.arrange, selectedClips: [900], selectedClip: 900, pianoRollClip: 900 };
    arrive(next, made, next.song.tracks[0]!, null);
    arrive(next, made, next.song.tracks[0]!, take([[c.start - head, c.length, c.key]], 901));

    expect(viewNow()).toEqual(before);
    expect(prView.shownClip).toBe(900);
  });

  it("クリップのない所から、アレンジでクリップをひらいたときは、これまでどおり合わせる", async () => {
    const { s, clip, track, take: t } = await pianoRollSong();
    const none = structuredClone(s);
    none.arrange = { ...none.arrange, pianoRollClip: null };
    app.setState(none);
    shown.setShown({ clip: null, track, take: null });
    prView.quarterPx = 320;
    prView.scrollTick = 20 * BAR;
    show();
    arrive(s, clip, track, t);
    expect(prView.scrollTick).toBe(clip.start);
    expect(prView.quarterPx).not.toBe(320);
  });

  it("ノートが 0 のクリップに書く: 新しいテイクが来ても、縦の位置も拡大も動かない", async () => {
    const { s, clip, track } = await pianoRollSong();
    const blank: ClipView = { ...clip, takeId: 500, noteCount: 0 };
    const st = structuredClone(s);
    st.song.tracks[0]!.clips = st.song.tracks[0]!.clips.map((c) => (c.id === clip.id ? blank : c));
    arrive(st, blank, st.song.tracks[0]!, take([], 500));
    show();
    prView.rowPx = 20;
    prView.topKey = 90;
    prView.scrollTick = clip.start + BAR;
    flushSync();
    const before = viewNow();
    const y = 2 * rowHeight(100) + 1;
    const c = drawAt(100, y);
    expect(c.take).toBe(500);
    expect(c.key).toBe(88);
    const filled: ClipView = { ...blank, takeId: 501, noteCount: 1 };
    const next = structuredClone(st);
    next.song.tracks[0]!.clips = next.song.tracks[0]!.clips.map((x) => (x.id === clip.id ? filled : x));
    arrive(next, filled, next.song.tracks[0]!, take([[c.start - clip.start - clip.takeOffset, c.length, c.key]], 501));
    expect(viewNow()).toEqual(before);
  });
});

describe("ノートをぜんぶ消しても、時間の位置は数のまま", () => {
  async function allDeleted(): Promise<{ clip: ClipView; back: () => void }> {
    const { s, clip, track, take: t } = await pianoRollSong();
    arrive(s, clip, track, t);
    show();
    prView.scrollTick = clip.start;
    flushSync();
    // core の答え: 同じクリップに、ノートが 0 の新しいテイク。
    const gone: ClipView = { ...clip, takeId: 700, noteCount: 0 };
    const next = structuredClone(s);
    next.song.tracks[0]!.clips = next.song.tracks[0]!.clips.map((c) => (c.id === clip.id ? gone : c));
    arrive(next, gone, next.song.tracks[0]!, take([], 700, { length: t.length }));
    return { clip, back: () => arrive(s, clip, track, t) };
  }

  it("横の棒の長さが数で、横に送っても位置が NaN にならない。目盛も出る", async () => {
    const { clip } = await allDeleted();
    const bar = q('[data-testid="hscrollbar"]');
    if (bar) expect(Number.isFinite(Number(bar.dataset.total))).toBe(true);
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 120 }));
    flushSync();
    expect(prView.scrollTick).toBeCloseTo(clip.start + 120 / pxPerTick(100));
    expect(target.querySelectorAll(".ruler-marks .mark").length).toBeGreaterThan(0);
  });

  it("棒の上でホイールを回しても NaN にならない", async () => {
    await allDeleted();
    const bar = q('[data-testid="hscrollbar"]')!;
    expect(bar).not.toBeNull();
    bar.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 100 }));
    flushSync();
    expect(Number.isFinite(prView.scrollTick)).toBe(true);
  });

  it("取り消しでノートが戻っても、送った位置のまま", async () => {
    const { clip, back } = await allDeleted();
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 80 }));
    flushSync();
    back();
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 80 }));
    flushSync();
    expect(prView.scrollTick).toBeCloseTo(clip.start + 160 / pxPerTick(100));
    expect(target.querySelectorAll(".ruler-marks .mark").length).toBeGreaterThan(0);
  });

  it("書く道具で書ける（始まりが数）", async () => {
    const { clip } = await allDeleted();
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 40 }));
    flushSync();
    const c = drawAt(100, 30);
    expect(c.take).toBe(700);
    expect(Number.isFinite(c.start)).toBe(true);
    expect(c.start).toBeGreaterThanOrEqual(clip.start);
  });
});
