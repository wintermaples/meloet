// Dragging clips in the arrange (spec 3 (arrange clips), spec 4, mock R3-Arrange): the rules (arrangeDrag.ts), and,
// with Arrange mounted and moved by mouse, what it sends, the picture during the drag, Mod D, and the fake Rust answers.
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, Command } from "../src/bridge";
import { arrangeOf, fakeState, reduceClips, type FakeState, type FakeTrack as TrackView } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import * as CD from "../src/components/arrangeDrag";
import * as G from "../src/components/arrangeGeom";
import * as PG from "../src/components/pianoroll/geom";
import { cancelDrag, dragReading, prView } from "../src/components/pianoroll/view.svelte";
import { watchSlots, type GlRect, type GlSlot } from "../src/gl/slot";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import { take as fixtureTake } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 4 * TPQ;
const WIDTH = 1000;
const METER = { num: 4, den: 4 };

function clip(id: number, name: string, fromBar: number, toBar: number): ClipView {
  return { id, name, start: (fromBar - 1) * BAR, end: (toBar - 1) * BAR, take: 1, takes: 1, takeId: 100 + id, takeOffset: 0, noteCount: 0, audio: null };
}
function track(number: number, clips: ClipView[]): TrackView {
  return { id: number, number, name: `トラック ${number}`, color: number, instrument: "sine", kind: { kind: "keys" }, sound: { kind: "builtin", instrument: "sine" }, standIn: null, drumKeys: null, gainDb: 0, pan: 0, sends: [null, null], sendDefaults: [null, null], inserts: [null, null, null, null], slots: [null, null, null, null], muted: false, soloed: false, lanes: [], clips };
}

// 1 bar is 64 px, row height 64, clips leave 3 above and below in the row (scale 100 %).
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

describe("where it was pressed (arrangeDrag.clipHit)", () => {
  it("6px at the left and right edges of a clip is length, the middle moves. For a narrow clip, up to 1/3 of the width", () => {
    const tracks = twoTracks();
    expect(CD.clipHit(view, dims, tracks, 257, 30, 6)).toMatchObject({ edge: "start", row: 0 });
    expect(CD.clipHit(view, dims, tracks, 300, 30, 6)).toMatchObject({ edge: null });
    expect(CD.clipHit(view, dims, tracks, 507, 30, 6)).toMatchObject({ edge: "end" });
    expect(CD.clipHit(view, dims, tracks, 300, 1, 6)).toBeNull();
    const narrow = [track(1, [{ ...clip(1, "x", 1, 2), end: TPQ / 2 }])];
    // width 8 px: the edges are 8/3 px
    expect(CD.clipHit(view, dims, narrow, 4, 30, 6)!.edge).toBeNull();
    expect(CD.clipHit(view, dims, narrow, 1, 30, 6)!.edge).toBe("start");
  });

  it("cursor: left-right at the edges, move on a clip (duplicate with Alt), arrow elsewhere", () => {
    const tracks = twoTracks();
    expect(CD.hoverCursor(CD.clipHit(view, dims, tracks, 507, 30, 6), false)).toBe("ew-resize");
    expect(CD.hoverCursor(CD.clipHit(view, dims, tracks, 300, 30, 6), false)).toBe("move");
    expect(CD.hoverCursor(CD.clipHit(view, dims, tracks, 300, 30, 6), true)).toBe("copy");
    expect(CD.hoverCursor(null, false)).toBe("default");
  });
});

describe("moving (arrangeDrag)", () => {
  it("snaps to the grid (1 bar) and sends one MoveClip on release. The reading is 「6.1.0000（+1 小節）」", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [300, 30], [370, 30]);
    expect(d).toMatchObject({ kind: "move", ticks: BAR, toRow: 0, ok: true });
    expect(CD.clipDragReading(d, METER)).toBe("6.1.0000（+1 小節）");
    expect(CD.clipDragCursor(d)).toBe("move");
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "moveClips", clips: [3], ticks: BAR, tracks: 0, copy: false });
  });

  it("Shift does not snap (the reading is in parts of a bar)", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [300, 30], [330, 30], { shift: true, alt: false });
    expect(d).toMatchObject({ ticks: (30 * BAR) / 64 });
    expect(CD.clipDragReading(d, METER)).toBe("5.2.1680（+0.469 小節）");
  });

  it("only a press (moved less than space.1) sends nothing and shows no reading", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [300, 30], [302, 31]);
    expect(d.press.moved).toBe(false);
    expect(CD.clipDragReading(d, METER)).toBeNull();
    expect(CD.clipDragCommand(d, tracks)).toBeNull();
  });

  it("where it overlaps another clip, the 「できない」 cursor, and release sends nothing. Only touching edges can be placed", () => {
    const tracks = twoTracks();
    const over = drag(tracks, [100, 30], [164, 30]);
    expect(over).toMatchObject({ ok: false });
    expect(CD.clipDragCursor(over)).toBe("not-allowed");
    expect(CD.clipDragCommand(over, tracks)).toBeNull();
    expect(CD.placeable(tracks, 1, 0, 2 * BAR, null)).toBe(true);
  });

  it("does not go before the start of the song (stops at 0. Sends nothing if it did not move)", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [100, 30], [10, 30]);
    expect(d).toMatchObject({ ticks: 0, ok: true });
    expect(CD.clipDragCommand(d, tracks)).toBeNull();
    const b = drag(tracks, [140, 94], [0, 94]);
    expect(b).toMatchObject({ ticks: -2 * BAR, ok: true });
  });

  it("to another track. Snaps to rows and sends the track number. Outside the rows, the top or bottom row", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [300, 30], [300 + 128, 94]);
    expect(d).toMatchObject({ toRow: 1, ok: true });
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "moveClips", clips: [3], ticks: 2 * BAR, tracks: 1, copy: false });
    expect(drag(tracks, [300, 30], [300, 400])).toMatchObject({ toRow: 1 });
    // cannot be placed where Bass A is (bars 3–4)
    expect(drag(tracks, [300, 30], [300 - 128, 94])).toMatchObject({ toRow: 1, ok: false });
  });

  it("duplicate with Alt: the original stays and the copy moves. Reading 「複製 ・ 9.1.0000」, cursor copy. Cannot be placed at the original place", () => {
    const tracks = twoTracks();
    const alt = { shift: false, alt: true };
    const d = drag(tracks, [300, 30], [300 + 256, 30], alt);
    expect(CD.clipDragReading(d, METER)).toBe("複製 ・ 9.1.0000");
    expect(CD.clipDragCursor(d)).toBe("copy");
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "moveClips", clips: [3], ticks: 4 * BAR, tracks: 0, copy: true });
    const same = drag(tracks, [300, 30], [310, 30], alt);
    expect(same).toMatchObject({ ticks: 0, ok: false });
    expect(CD.clipDragCommand(same, tracks)).toBeNull();
  });
});

describe("trimming and extending the edges (arrangeDrag)", () => {
  it("trim the right edge by 1 bar. Reading 「長さ 3.0.0000」, sends one TrimClip on release", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [509, 30], [445, 30]);
    expect(d).toMatchObject({ kind: "trim", edge: "end", start: 4 * BAR, end: 7 * BAR });
    expect(CD.clipDragCursor(d)).toBe("ew-resize");
    expect(CD.clipDragReading(d, METER)).toBe("長さ 3.0.0000");
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "trimClip", clip: 3, edge: "end", ticks: -BAR });
  });

  it("Shift does not snap (the form of mock R3-Arrange 「長さ 2.2.000」)", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [509, 30], [509 - 96, 30], { shift: true, alt: false });
    expect(CD.clipDragReading(d, METER)).toBe("長さ 2.2.0000");
    expect(CD.clipDragCommand(d, tracks)).toEqual({ type: "trimClip", clip: 3, edge: "end", ticks: -1.5 * BAR });
  });

  it("stops before the next clip. The left edge stops at the start of the song", () => {
    const tracks = twoTracks();
    const d = drag(tracks, [258, 30], [100, 30]);
    expect(d).toMatchObject({ start: 4 * BAR });
    expect(CD.clipDragCommand(d, tracks)).toBeNull();
    const head = drag(tracks, [1, 30], [-100, 30], { shift: true, alt: false });
    expect(head).toMatchObject({ start: 0 });
    const right = drag(tracks, [254, 30], [600, 30]);
    expect(right).toMatchObject({ end: 4 * BAR });
  });

  it("never shorter than 1 tick (one grid step when snapping). Extending has no limit", () => {
    const tracks = twoTracks();
    expect(drag(tracks, [509, 30], [0, 30])).toMatchObject({ start: 4 * BAR, end: 5 * BAR });
    expect(drag(tracks, [509, 30], [0, 30], { shift: true, alt: false })).toMatchObject({ end: 4 * BAR + 1 });
    const out = drag(tracks, [509, 30], [509 + 640, 30]);
    expect(CD.clipDragCommand(out, tracks)).toEqual({ type: "trimClip", clip: 3, edge: "end", ticks: 10 * BAR });
  });

  it("the length text is bar . beat . tick (all from 0)", () => {
    expect(CD.clipLengthText(2.5 * BAR, METER)).toBe("2.2.0000");
    expect(CD.clipLengthText(BAR + TPQ / 4, METER)).toBe("1.0.0480");
    expect(CD.clipLengthText(3 * TPQ + TPQ / 2, { num: 3, den: 4 })).toBe("1.0.0960");
  });
});

describe("picture of a trimmed clip", () => {
  const take = { starts: [0, 2 * TPQ, BAR, 2 * BAR], lengths: [TPQ, 3 * TPQ, TPQ, TPQ], keys: [60, 62, 64, 67] };

  it("arrange: only notes that start inside the clip range (the same as what plays). Ends that go past are cut where drawn", () => {
    // trimmed 1 bar from the start: the take's 0 is before the song's BAR (offset −BAR)
    const trimmed = { start: BAR, end: 2 * BAR, takeOffset: -BAR };
    expect(G.takeShapes(trimmed, take)).toEqual([
      { start: BAR, length: TPQ, key: 64 },
    ]);
    // trimmed the end: the second note starts inside, so it is drawn; paintClip cuts its end at the right border
    const end = { start: 0, end: (25 * TPQ) / 8, takeOffset: 0 };
    expect(G.takeShapes(end, take).map((n) => n.key)).toEqual([60, 62]);
    const out: GlRect[] = [];
    G.paintClip(out, view, dims, G.clipBox(view, dims, 0, end.start, end.end), G.CLIP_PLAIN("color.track.1"), G.takeShapes(end, take));
    const notes = out.slice(3);
    expect(Math.max(...notes.map((r) => r.x + r.w))).toBeLessThanOrEqual(G.tickToX(view, end.end) - dims.hair);
  });

  it("the border at the original place is one gray.7 line (inside not drawn)", () => {
    const out: GlRect[] = [];
    G.paintFrame(out, view, dims, G.clipBox(view, dims, 1, 0, BAR), "color.gray.7");
    expect(out).toEqual([
      { x: 0, y: 67, w: 64, h: 1, color: "color.gray.7" },
      { x: 0, y: 123, w: 64, h: 1, color: "color.gray.7" },
      { x: 0, y: 67, w: 1, h: 57, color: "color.gray.7" },
      { x: 63, y: 67, w: 1, h: 57, color: "color.gray.7" },
    ]);
  });

  it("piano roll: notes that start outside the range are not drawn, cannot be pressed or boxed, and are not in the key range", () => {
    const t = fixtureTake(take.starts.map((st, i) => [st, take.lengths[i]!, take.keys[i]!]), 9);
    const n = PG.shownOf(t, -BAR, { from: 0, to: BAR });
    const seen: number[] = [];
    PG.eachNoteIn(n, -10000, 10000, (i) => seen.push(i));
    expect(seen).toEqual([2]);
    const g: PG.RollGeom = { scrollTick: 0, ppt: 0.1, topKey: 70, rowH: 10, width: 400, height: 200 };
    expect(PG.hitNote(n, g, PG.tickToX(g, 0) + 2, PG.keyToY(g, 64) + 2, 6)).toEqual({ note: 2, edge: false });
    expect(PG.notesInBox(n, g, 0, 0, 400, 200)).toEqual([2]);
    expect(PG.keyRangeOf(n)).toEqual({ low: 64, high: 64 });
  });
});

describe("fake Rust answers (reduceClips)", () => {
  const state = (): FakeState => {
    const s = fakeState();
    s.song.tracks = twoTracks();
    return s;
  };
  it("MoveClip moves it, a copy gets a new id. The moved clip (the copy) is selected. On overlap it does nothing and says so in the status line", () => {
    const moved = reduceClips(state(), { type: "moveClips", clips: [3], ticks: BAR, tracks: 1, copy: false })!;
    expect(moved.song.tracks[0]!.clips.map((c) => c.id)).toEqual([1]);
    expect(moved.song.tracks[1]!.clips.map((c) => [c.id, c.start])).toEqual([[5, 2 * BAR], [3, 5 * BAR]]);
    expect(moved.arrange.selectedClip).toBe(3);
    const copied = reduceClips(state(), { type: "moveClips", clips: [3], ticks: 4 * BAR, tracks: 0, copy: true })!;
    const ids = copied.song.tracks[0]!.clips.map((c) => c.id);
    expect(ids.slice(0, 2)).toEqual([1, 3]);
    expect(copied.arrange.selectedClip).toBe(ids[2]);
    const s = state();
    expect(reduceClips(s, { type: "moveClips", clips: [1], ticks: BAR, tracks: 0, copy: false })).toEqual({ song: s.song, arrange: s.arrange, notice: { id: "clipsNotMoved" } });
    expect(reduceClips(s, { type: "moveClips", clips: [1], ticks: BAR, tracks: 2, copy: false })).toBeNull();
  });
  it("DuplicateClip goes just after. TrimClip does not move the take's 0", () => {
    const dup = reduceClips(state(), { type: "duplicateClips", clips: [3] })!;
    expect(dup.song.tracks[0]!.clips.map((c) => [c.start, c.end])).toEqual([[0, 4 * BAR], [4 * BAR, 8 * BAR], [8 * BAR, 12 * BAR]]);
    const trim = reduceClips(state(), { type: "trimClip", clip: 3, edge: "start", ticks: BAR })!;
    expect(trim.song.tracks[0]!.clips[1]).toMatchObject({ start: 5 * BAR, end: 8 * BAR, takeOffset: -BAR });
  });
  it("DeleteTake goes to the take before (or the next if it is the first). Does nothing when there is only one take", () => {
    const s = state();
    s.song.tracks[0]!.clips[1] = { ...s.song.tracks[0]!.clips[1]!, take: 2, takes: 3 };
    const once = reduceClips(s, { type: "deleteTake", clip: 3 })!;
    expect(once.song.tracks[0]!.clips[1]).toMatchObject({ take: 1, takes: 2 });
    const twice = reduceClips({ ...s, song: once.song }, { type: "deleteTake", clip: 3 })!;
    expect(twice.song.tracks[0]!.clips[1]).toMatchObject({ take: 1, takes: 1 });
    expect(reduceClips({ ...s, song: twice.song }, { type: "deleteTake", clip: 3 })).toBeNull();
    expect(reduceClips(s, { type: "deleteTake", clip: 99 })).toBeNull();
  });
});

// ---- with Arrange mounted ----

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

describe("mouse in Arrange", () => {
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

  it("while held and moved nothing is sent, only the look: the original place is a gray.7 border, the moving clip is a small region with transform. One MoveClip on release", () => {
    show();
    pointer("pointerdown", 300, 30);
    pointer("pointermove", 340, 30);
    pointer("pointermove", 366, 30);
    expect(moves()).toEqual([]);
    expect(dragReading.text).toBe("6.1.0000（+1 小節）");
    expect(target.querySelector('[data-test="arrange-reading"]')!.textContent!.trim()).toBe("6.1.0000（+1 小節）");
    expect(lanes().style.cursor).toBe("move");
    const box = target.querySelector<HTMLElement>(".rec-host .drag-clip")!;
    // jsdom rewrites 「translate(64px, 0px)」 as 「translate(64px)」
    expect(box.style.transform).toBe("translate(64px)");
    expect(box.style.left).toBe("256px");
    expect(slots.get(box)?.kind).toBe("rects");
    const frame = laneRects().filter((r) => r.color === "color.gray.7");
    expect(frame).toHaveLength(4);
    // the big surface is not redrawn for the drag amount
    const before = laneRects();
    pointer("pointermove", 430, 30);
    expect(laneRects()).toBe(before);
    expect(box.style.transform).toBe("translate(128px)");
    pointer("pointerup", 366, 30);
    expect(moves()).toEqual([{ type: "moveClips", clips: [3], ticks: 2 * BAR, tracks: 0, copy: false }]);
    expect(dragReading.text).toBeNull();
  });

  it("selects the clip that was pressed (the current design). Moving to another track sends track 2's number", () => {
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

  it("where it cannot be placed, the 「できない」 cursor, and release sends nothing", () => {
    show();
    pointer("pointerdown", 100, 30);
    pointer("pointermove", 164, 30);
    expect(lanes().style.cursor).toBe("not-allowed");
    pointer("pointerup", 164, 30);
    expect(moves()).toEqual([]);
  });

  it("Esc (cancelDrag) goes back to before the drag, and release sends nothing", () => {
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

  it("duplicate with Alt + drag: the original is drawn as it is (not a border), cursor copy", () => {
    show();
    pointer("pointerdown", 300, 30, { altKey: true });
    pointer("pointermove", 556, 30, { altKey: true });
    expect(lanes().style.cursor).toBe("copy");
    expect(dragReading.text).toBe("複製 ・ 9.1.0000");
    expect(laneRects().some((r) => r.color === "color.gray.7")).toBe(false);
    pointer("pointerup", 556, 30, { altKey: true });
    expect(moves()).toEqual([{ type: "moveClips", clips: [3], ticks: 4 * BAR, tracks: 0, copy: true }]);
  });

  it("edges: hover gives the left-right cursor. While trimming it draws in a box the width of the row, and the trimmed part is inside the original border. One TrimClip on release", () => {
    show();
    pointer("pointermove", 509, 30);
    expect(lanes().style.cursor).toBe("ew-resize");
    pointer("pointerdown", 509, 30);
    pointer("pointermove", 445, 30);
    expect(dragReading.text).toBe("長さ 3.0.0000");
    const box = target.querySelector<HTMLElement>(".drag-clip")!;
    expect([box.style.left, box.style.width, box.style.transform]).toEqual(["0px", `${WIDTH}px`, ""]);
    const rects = (slots.get(box) as Extract<GlSlot, { kind: "rects" }>).rects;
    expect(Math.max(...rects.map((r) => r.x + r.w))).toBe(G.tickToX({ ...view }, 7 * BAR));
    expect(laneRects().filter((r) => r.color === "color.gray.7")).toHaveLength(4);
    pointer("pointerup", 445, 30);
    expect(moves()).toEqual([{ type: "trimClip", clip: 3, edge: "end", ticks: -BAR }]);
  });

  it("while recording and during count-in, only selects, no drag", () => {
    for (const state of ["recording", "countIn"] as const) {
      show((s) => {
        s.transport.state = state;
        s.transport.recording = { id: 1, track: 2, from: 0, take: 1, clip: null, lap: 0, overdub: false };
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

describe("Mod D (spec 4 (Duplicate))", () => {
  const press = (deps: KeyDeps) => {
    const ev = new KeyboardEvent("keydown", { key: "d", ctrlKey: true, cancelable: true });
    handleKeydown(ev, deps);
    return ev;
  };
  afterEach(() => (prView.focus = "arrange"));

  it("with focus on the arrange and a clip selected, DuplicateClip. Nothing is sent when nothing is selected", () => {
    const state = fakeState();
    const sent: Command[] = [];
    const deps: KeyDeps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" } };
    state.arrange = arrangeOf(3);
    expect(press(deps).defaultPrevented).toBe(true);
    state.arrange = arrangeOf(null);
    press(deps);
    expect(sent).toEqual([{ type: "duplicateClips", clips: [3] }]);
  });

  it("even with the piano roll open, a clip if the arrange was pressed last. Notes if it was the piano roll", () => {
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

  it("nothing is sent while recording and during count-in", () => {
    const state = fakeState();
    state.arrange = arrangeOf(3);
    state.transport.state = "recording";
    const sent: Command[] = [];
    press({ state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" } });
    expect(sent).toEqual([]);
  });
});
