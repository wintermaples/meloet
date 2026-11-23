// The piano roll with 0 notes: the first placed note does not move the view (where there is no clip, and a clip with 0 notes),
// and after deleting all notes the horizontal position stays a number, the ruler shows, and drawing works.
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, Command, TakeNotes, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { fitQuarterPx } from "../src/components/pianoroll/geom";
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

/** The song (A-PianoRoll), and the clip, track and take of its piano roll. */
async function pianoRollSong(): Promise<{ s: AppState; clip: ClipView; track: TrackView; take: TakeNotes }> {
  const s = fakeScene("pianoRoll")!.state;
  const track = s.song.tracks[0]!;
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  return { s, clip, track, take: decodeTake(await createFakeTransport(s).takeNotes(clip.takeId)) };
}

/** Replaces the state and also the shown clip and take (a core answer, a new take arriving). */
function arrive(s: AppState, clip: ClipView | null, track: TrackView, t: TakeNotes | null): void {
  app.setState(s);
  shown.setShown({ clip, track, take: t });
  flushSync();
}

/** Press and release at (x, y) with the draw tool. Returns the sent addNote. */
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

describe("placing the first note does not change the view", () => {
  it("drawing where there is no clip (B-52): the new clip is not an \"opened\" one", async () => {
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

    // The core answer: a 4-bar clip from the bar start, with one note in it.
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

  it("opening a clip in the arrange from a place with no clip fits the view as before", async () => {
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

  it("drawing in a clip with 0 notes: when the new take comes, the vertical position and zoom do not move", async () => {
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

describe("after deleting all notes, the time position stays a number", () => {
  async function allDeleted(): Promise<{ clip: ClipView; back: () => void }> {
    const { s, clip, track, take: t } = await pianoRollSong();
    arrive(s, clip, track, t);
    show();
    prView.scrollTick = clip.start;
    flushSync();
    // The core answer: a new take with 0 notes in the same clip.
    const gone: ClipView = { ...clip, takeId: 700, noteCount: 0 };
    const next = structuredClone(s);
    next.song.tracks[0]!.clips = next.song.tracks[0]!.clips.map((c) => (c.id === clip.id ? gone : c));
    arrive(next, gone, next.song.tracks[0]!, take([], 700, { length: t.length }));
    return { clip, back: () => arrive(s, clip, track, t) };
  }

  it("the horizontal bar length is a number, scrolling does not make the position NaN. The ruler shows", async () => {
    const { clip } = await allDeleted();
    const bar = q('[data-testid="hscrollbar"]');
    if (bar) expect(Number.isFinite(Number(bar.dataset.total))).toBe(true);
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 120 }));
    flushSync();
    expect(prView.scrollTick).toBeCloseTo(clip.start + 120 / pxPerTick(100));
    expect(target.querySelectorAll(".ruler-marks .mark").length).toBeGreaterThan(0);
  });

  it("turning the wheel on the bar does not give NaN", async () => {
    await allDeleted();
    const bar = q('[data-testid="hscrollbar"]')!;
    expect(bar).not.toBeNull();
    bar.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 100 }));
    flushSync();
    expect(Number.isFinite(prView.scrollTick)).toBe(true);
  });

  it("when undo brings notes back, the scrolled position stays", async () => {
    const { clip, back } = await allDeleted();
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 80 }));
    flushSync();
    back();
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 80 }));
    flushSync();
    expect(prView.scrollTick).toBeCloseTo(clip.start + 160 / pxPerTick(100));
    expect(target.querySelectorAll(".ruler-marks .mark").length).toBeGreaterThan(0);
  });

  it("the draw tool can draw (the start is a number)", async () => {
    const { clip } = await allDeleted();
    q(".middle")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: 40 }));
    flushSync();
    const c = drawAt(100, 30);
    expect(c.take).toBe(700);
    expect(Number.isFinite(c.start)).toBe(true);
    expect(c.start).toBeGreaterThanOrEqual(clip.start);
  });
});

describe("opening a clip with the loop on", () => {
  async function openWith(loopOn: boolean, loop: { start: number; end: number }): Promise<ClipView> {
    const { s, clip, track, take: t } = await pianoRollSong();
    s.song = { ...s.song, loopOn, loopRange: loop };
    const none = structuredClone(s);
    none.arrange = { ...none.arrange, pianoRollClip: null };
    app.setState(none);
    shown.setShown({ clip: null, track, take: null });
    prView.quarterPx = 10;
    prView.scrollTick = 20 * BAR;
    show();
    arrive(s, clip, track, t);
    return clip;
  }

  it("the loop overlapping the clip: the step that just holds the loop, from the loop's head", async () => {
    const { clip } = await pianoRollSong();
    await openWith(true, { start: clip.start + BAR, end: clip.start + 2 * BAR });
    expect(prView.scrollTick).toBe(clip.start + BAR);
    // One bar in 696: 4 quarters of 160.
    expect(prView.quarterPx).toBe(160);
  });

  it("the loop apart from the clip, or off: the whole clip, as before", async () => {
    const { clip } = await pianoRollSong();
    const fit = fitQuarterPx(clip.end - clip.start, W, 100);
    await openWith(true, { start: clip.end + BAR, end: clip.end + 2 * BAR });
    expect(prView.scrollTick).toBe(clip.start);
    expect(prView.quarterPx).toBe(fit);
    unmount(mounted!);
    mounted = null;
    prView.shownClip = null;
    await openWith(false, { start: clip.start + BAR, end: clip.start + 2 * BAR });
    expect(prView.scrollTick).toBe(clip.start);
    expect(prView.quarterPx).toBe(fit);
  });
});
