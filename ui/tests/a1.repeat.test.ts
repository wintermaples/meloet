// Round 12 A1, repeating clips: the handle at the right end of the name strip (arrangeDrag.ts), the frames of the copies,
// one RepeatClips on release, Esc; the box of 「くり返す…」 (Mod R, 「編集」, the right-click menu); the fake's answer.
import { flushSync, mount, tick, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, Command } from "../src/bridge";
import { fakeState, reduceClips, type FakeTrack as TrackView } from "../src/bridge/fake";
import { fakeAudioClip, fakeVocalClip } from "../src/bridge/fakeAudio";
import Arrange from "../src/components/Arrange.svelte";
import { audioShapeOf, fadeHandleBox } from "../src/components/arrange/audioClipGeom";
import { closeRepeatBox, repeatBox } from "../src/components/arrange/repeatBox.svelte";
import * as CD from "../src/components/arrangeDrag";
import * as G from "../src/components/arrangeGeom";
import { repeatCountOf, repeatTarget } from "../src/components/clipEdits";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { topMenus } from "../src/components/menu/menus";
import { cancelDrag, dragReading } from "../src/components/pianoroll/view.svelte";
import { watchSlots, type GlRect, type GlSlot } from "../src/gl/slot";
import { handleKeydown, keyActionUsable, type KeyDeps } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { ACTION_ROWS } from "../src/keys/keyList";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
// Audio clips ask the bridge for peaks; these tests do not draw waves.
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  audioPeaks: () => new Promise(() => {}),
  moreAudioPeaks: () => new Promise(() => {}),
  peekAudioPeaks: () => null,
}));

const BAR = 4 * TPQ;
const WIDTH = 1000;
const METER = { num: 4, den: 4 };

function clip(id: number, fromBar: number, toBar: number): ClipView {
  return { id, name: `クリップ ${id}`, start: (fromBar - 1) * BAR, end: (toBar - 1) * BAR, take: 1, takes: 1, takeId: 100 + id, takeOffset: 0, noteCount: 0, audio: null, comp: [] };
}

// 1 bar is 64 px, rows 64 high, clips 3 px in from the row; the name strip is line.mark 2 + 16 (scale 100 %).
const view: G.View = { pxPerTick: 64 / BAR, scrollX: 0, width: WIDTH };
const dims: G.LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
const grip: CD.RepeatGrip = { width: 12, minClip: 28 };

/** Row 1: A (bars 1–2), B (bars 9–10). Row 2: C (bars 2–3), and an audio clip D (bars 11–12) on row 3. */
function tracks(): TrackView[] {
  const base = fakeState().song.tracks[0]!;
  const audio = fakeAudioClip({ id: 7, file: "歌.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 8 * 48000 });
  return [
    { ...base, clips: [clip(1, 1, 3), clip(2, 9, 11)] },
    { ...base, id: 2, number: 2, name: "ベース", color: 2, clips: [clip(3, 2, 4)] },
    {
      ...base,
      id: 3,
      number: 3,
      name: "歌",
      kind: { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false },
      clips: [fakeVocalClip(4, "歌", 10 * BAR, 12 * BAR, audio)],
    },
  ];
}
const input = (t: TrackView[]): CD.DragInput => ({ view, dims, tracks: t, step: BAR, stillPx: 4 });
const none = { shift: false, alt: false };

function drag(t: TrackView[], from: [number, number], toX: number, group: CD.GroupClip[] = []): CD.ClipDrag {
  const hit = CD.clipHit(view, dims, t, from[0], from[1], 6, grip)!;
  let d = CD.startClipDrag(hit, from[0], from[1], none, group);
  d = CD.dragClipTo(d, input(t), (from[0] + toX) / 2, from[1], none);
  return CD.dragClipTo(d, input(t), toX, from[1], none);
}

describe("the repeat handle (arrangeDrag.clipHit)", () => {
  it("the right 12 px of the name strip is the handle; under it the right edge is still the length; the left edge stays", () => {
    const t = tracks();
    // A: x 0–128, y 3–60; the name strip is y 3–21.
    expect(CD.clipHit(view, dims, t, 120, 10, 6, grip)).toMatchObject({ repeat: true, clip: { id: 1 } });
    expect(CD.clipHit(view, dims, t, 115, 10, 6, grip)).toMatchObject({ repeat: false, edge: null });
    expect(CD.clipHit(view, dims, t, 125, 30, 6, grip)).toMatchObject({ repeat: false, edge: "end" });
    expect(CD.clipHit(view, dims, t, 2, 10, 6, grip)).toMatchObject({ repeat: false, edge: "start" });
    expect(CD.hoverCursor(CD.clipHit(view, dims, t, 120, 10, 6, grip), false)).toBe("copy");
    expect(CD.hoverCursor(CD.clipHit(view, dims, t, 125, 30, 6, grip), false)).toBe("ew-resize");
    // Without the grip (the old callers), no handle.
    expect(CD.clipHit(view, dims, t, 120, 10, 6)).toMatchObject({ repeat: false, edge: null });
  });

  it("at most a third of the clip; no handle on a clip narrower than the take tag (28 px)", () => {
    const box = (w: number) => ({ x0: 0, x1: w, y0: 3, y1: 60 });
    expect(CD.repeatGripBox(dims, box(30), grip)).toEqual({ x0: 20, x1: 30, y0: 3, y1: 21 });
    expect(CD.repeatGripBox(dims, box(27), grip)).toBeNull();
    const narrow = [{ ...tracks()[0]!, clips: [{ ...clip(1, 1, 2), end: (TPQ * 3) / 2 }] }];
    expect(CD.clipHit(view, dims, narrow, 20, 10, 6, grip)).toMatchObject({ repeat: false, edge: "end" });
  });
});

describe("dragging the handle (arrangeDrag)", () => {
  it("whole copies, the nearest number of clip lengths; stops before the next clip (B at bar 9: room for 3)", () => {
    const t = tracks();
    const d = drag(t, [120, 10], 120 + 128);
    expect(d).toMatchObject({ kind: "repeat", count: 1, room: 3 });
    expect(CD.clipDragReading(d, METER)).toBe("1 回くり返します");
    expect(CD.clipDragCursor(d)).toBe("copy");
    expect(CD.clipDragCommand(d, t)).toEqual({ type: "repeatClips", clips: [1], count: 1 });
    expect(drag(t, [120, 10], 120 + 96)).toMatchObject({ count: 1 });
    expect(drag(t, [120, 10], 120 + 60)).toMatchObject({ count: 0 });
    expect(drag(t, [120, 10], 120 + 1000)).toMatchObject({ count: 3 });
    expect(drag(t, [120, 10], 0)).toMatchObject({ count: 0 });
  });

  it("no copy, or only a press: nothing is sent and nothing is read", () => {
    const t = tracks();
    const zero = drag(t, [120, 10], 120 + 20);
    expect([CD.clipDragCommand(zero, t), CD.clipDragReading(zero, METER)]).toEqual([null, null]);
    const hit = CD.clipHit(view, dims, t, 120, 10, 6, grip)!;
    expect(CD.clipDragCommand(CD.startClipDrag(hit, 120, 10, none), t)).toBeNull();
    // A clip with another right after it: no room at all, the 「できない」 cursor.
    const packed = [{ ...t[0]!, clips: [clip(1, 1, 3), clip(2, 3, 4)] }];
    const full = drag(packed, [120, 10], 400);
    expect([full.kind === "repeat" && full.room, CD.clipDragCursor(full), CD.clipDragCommand(full, packed)]).toEqual([0, "not-allowed", null]);
  });

  it("several chosen: one block from the earliest start to the latest end, on their own rows", () => {
    const t = tracks();
    const group = CD.groupOf(t, [1, 3]);
    // A (bars 1–2) and C (bars 2–3): a block of 3 bars. A's third copy (bars 10–11) would meet B (bars 9–10).
    expect(CD.repeatBlock(group)).toEqual({ first: 0, last: 3 * BAR, length: 3 * BAR });
    expect(CD.repeatRoom(t, group)).toBe(2);
    const d = drag(t, [120, 10], 120 + 192 * 2, group);
    expect(d).toMatchObject({ count: 2 });
    expect(CD.clipDragCommand(d, t)).toEqual({ type: "repeatClips", clips: [1, 3], count: 2 });
    // Not one of the chosen: only the clip pressed.
    expect(drag(t, [120, 10], 120 + 128, CD.groupOf(t, [3]))).toMatchObject({ group: [{ clip: { id: 1 } }] });
  });

  it("the copies are frames (gray.2 face, gray.7 border) after the clip; an accent bar where the handle is", () => {
    const out: GlRect[] = [];
    const a = { row: 0, start: 0, end: 2 * BAR };
    G.paintRepeats(out, view, dims, [a], 2 * BAR, 2, a);
    const faces = out.filter((r) => r.color === "color.gray.2");
    expect(faces.map((r) => [r.x, r.w])).toEqual([
      [128, 128],
      [256, 128],
    ]);
    expect(out.filter((r) => r.color === "color.gray.7")).toHaveLength(8);
    expect(out.at(-1)).toEqual({ x: 384 - 2, y: 3, w: 2, h: 57, color: "color.accent.ink" });
    const zero: GlRect[] = [];
    G.paintRepeats(zero, view, dims, [a], 2 * BAR, 0, a);
    expect(zero).toEqual([{ x: 128 - 2, y: 3, w: 2, h: 57, color: "color.accent.ink" }]);
  });
});

describe("the box of 「くり返す…」 (clipEdits)", () => {
  it("a whole number, put into 1–99; full-width digits are read; anything else cannot be decided", () => {
    expect(["3", " 12 ", "０７", "0", "150", "", "a", "1.5", "-2"].map(repeatCountOf)).toEqual([3, 12, 7, 1, 99, null, null, null, null]);
  });
  it("on the chosen clips; not while recording or counting in, not with nothing chosen, not in the overview", () => {
    const s = fakeState();
    s.song.tracks = tracks();
    s.arrange.selectedClips = [1, 3];
    expect(repeatTarget(s)).toEqual([1, 3]);
    expect(repeatTarget({ ...s, transport: { ...s.transport, state: "countIn" } })).toBeNull();
    expect(repeatTarget({ ...s, arrange: { ...s.arrange, selectedClips: [] } })).toBeNull();
    expect(repeatTarget({ ...s, workspace: { ...s.workspace, layout: { ...s.workspace.layout, arrangeOverview: true } } })).toBeNull();
  });
});

describe("Mod R and the menus", () => {
  const k = (more: Partial<KeyInput>): KeyInput => ({ key: "r", ctrl: false, meta: false, alt: false, shift: false, ...more });
  it("Mod R (Cmd R on the Mac), even while typing; R alone is still 「録音」", () => {
    expect(keyAction(k({ ctrl: true }), false, "other")).toEqual({ kind: "repeatClips" });
    expect(keyAction(k({ ctrl: true }), true, "other")).toEqual({ kind: "repeatClips" });
    expect(keyAction(k({ key: "", code: "KeyR", ctrl: true }), false, "other")).toEqual({ kind: "repeatClips" });
    expect(keyAction(k({ meta: true }), false, "mac")).toEqual({ kind: "repeatClips" });
    expect(keyAction(k({}), false, "other")).toEqual({ kind: "record" });
    expect(keyAction(k({ ctrl: true, shift: true }), false, "other")).toBeNull();
  });
  it("the key list has the row under 「アレンジ」 (no longer among the rows to come)", () => {
    expect(ACTION_ROWS.repeatClips.map((r) => r.label)).toEqual(["keyList.repeatClips"]);
  });
});

// ---- with Arrange mounted ----

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

describe("Arrange: the handle with the mouse, and the box", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  const slots = new Map<HTMLElement, GlSlot>();
  let stopWatch: () => void = () => {};
  beforeEach(() => {
    app.reset();
    closeRepeatBox();
    target = document.createElement("div");
    target.className = "app";
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
    contextMenu.open = null;
    closeRepeatBox();
    target.remove();
    stopWatch();
  });

  function state(selected: number[] = [1], edit: (s: AppState) => void = () => {}): AppState {
    return app.stateWith((s) => {
      s.song.tracks = tracks();
      s.song.playTarget = 1;
      s.arrange = { selectedClips: selected, selectedClip: selected[0] ?? null, pianoRollClip: null, clipboardClips: 0, tallTrack: null };
      edit(s);
    });
  }
  function show(s: AppState = state()): void {
    if (mounted) unmount(mounted);
    app.setState(s);
    mounted = mount(Arrange, { target });
    flushSync();
  }
  const lanes = () => target.querySelector<HTMLElement>(".lanes")!;
  /** On the lanes; `on` a part inside them, as the real pointer is (the fade handles take a press in the capture phase). */
  function pointer(type: string, x: number, y: number, init: MouseEventInit = {}, on: HTMLElement = lanes()): void {
    on.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0, ...init }));
    flushSync();
  }
  const repeats = () => app.sent.filter((c: Command) => c.type === "repeatClips");
  const dragRects = () => {
    const node = target.querySelector<HTMLElement>(".rec-host .drag-clip");
    const s = node ? slots.get(node) : undefined;
    return s?.kind === "rects" ? s.rects : [];
  };
  const field = () => target.querySelector<HTMLInputElement>('[data-test="repeat-count-field"]');
  const deps = (s: AppState): KeyDeps => ({ state: () => s, send: app.send, nav: { platform: "Linux x86_64" }, playTick: () => 0 });
  function key(s: AppState, init: KeyboardEventInit): KeyboardEvent {
    const ev = new KeyboardEvent("keydown", { cancelable: true, bubbles: true, ...init });
    handleKeydown(ev, deps(s));
    flushSync();
    return ev;
  }
  function type(text: string, init: KeyboardEventInit = {}): void {
    const f = field()!;
    if (text) {
      f.value = text;
      f.dispatchEvent(new Event("input", { bubbles: true }));
    }
    if (init.key) f.dispatchEvent(new KeyboardEvent("keydown", { cancelable: true, bubbles: true, ...init }));
    flushSync();
  }

  it("the pointer on the handle: ⟳ there and the copy cursor; off it, no ⟳", () => {
    show();
    pointer("pointermove", 120, 10);
    const mark = target.querySelector<HTMLElement>('[data-test="repeat-mark"]')!;
    expect(mark.textContent!.trim()).toBe("⟳");
    expect([mark.style.left, mark.style.width]).toEqual(["116px", "12px"]);
    expect(lanes().style.cursor).toBe("copy");
    pointer("pointermove", 100, 10);
    expect(target.querySelector('[data-test="repeat-mark"]')).toBeNull();
    expect(lanes().style.cursor).toBe("move");
  });

  it("held and moved: only the look (the frames of the copies in the small slot, the clips stay as they are); one RepeatClips on release", () => {
    show();
    const big = () => slots.get(target.querySelector<HTMLElement>(".lanes > .fill")!);
    const before = big();
    pointer("pointerdown", 120, 10);
    pointer("pointermove", 180, 10);
    pointer("pointermove", 120 + 256, 10);
    expect(app.sent).toEqual([]);
    expect(dragReading.text).toBe("2 回くり返します");
    expect(target.querySelector('[data-test="arrange-reading"]')!.textContent!.trim()).toBe("2 回くり返します");
    expect(lanes().style.cursor).toBe("copy");
    const box = target.querySelector<HTMLElement>(".rec-host .drag-clip")!;
    expect([box.style.left, box.style.width, box.style.transform]).toEqual(["0px", `${WIDTH}px`, ""]);
    expect(dragRects().filter((r) => r.color === "color.gray.2").map((r) => r.x)).toEqual([128, 256]);
    expect(dragRects().some((r) => r.color === "color.accent.ink")).toBe(true);
    // The big face is not drawn again, and the clip keeps its own look (no gray.7 border where it is).
    expect(big()).toBe(before);
    expect(target.querySelector('[data-test="repeat-mark"]')).not.toBeNull();
    pointer("pointerup", 120 + 256, 10);
    expect(app.sent).toEqual([{ type: "repeatClips", clips: [1], count: 2 }]);
    expect(dragReading.text).toBeNull();
  });

  it("several chosen: the block repeats; pressing a clip not chosen selects it first", () => {
    show(state([1, 3]));
    pointer("pointerdown", 120, 10);
    pointer("pointermove", 120 + 192, 10);
    pointer("pointerup", 120 + 192, 10);
    expect(app.sent).toEqual([{ type: "repeatClips", clips: [1, 3], count: 1 }]);
    app.sent.length = 0;
    show(state([]));
    pointer("pointerdown", 120, 10);
    pointer("pointermove", 120 + 128, 10);
    pointer("pointerup", 120 + 128, 10);
    expect(app.sent).toEqual([
      { type: "selectClip", clip: 1 },
      { type: "repeatClips", clips: [1], count: 1 },
    ]);
  });

  it("Esc (cancelDrag) puts it back and release sends nothing", () => {
    show();
    pointer("pointerdown", 120, 10);
    pointer("pointermove", 120 + 128, 10);
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(target.querySelector(".drag-clip")).toBeNull();
    expect(dragReading.text).toBeNull();
    pointer("pointerup", 120 + 128, 10);
    expect(repeats()).toEqual([]);
  });

  it("the length under the handle and the fade handles of an audio clip still work", () => {
    show();
    pointer("pointerdown", 125, 30);
    pointer("pointermove", 125 + 64, 30);
    pointer("pointerup", 125 + 64, 30);
    expect(app.sent).toEqual([{ type: "trimClip", clip: 1, edge: "end", ticks: BAR }]);
    app.sent.length = 0;
    // The fade-out handle of D overlaps the bottom of its name strip at the right end: the fade comes first.
    const d = tracks()[2]!.clips[0]!;
    const fade = fadeHandleBox(view, dims, G.clipBox(view, dims, 2, d.start, d.end), audioShapeOf(d)!, "fadeOut", 6);
    const x = fade.x0 + 1;
    const y = fade.y0 + 1;
    expect(CD.clipHit(view, dims, tracks(), x, y, 6, grip)).toMatchObject({ repeat: true });
    const face = target.querySelector<HTMLElement>(".lanes > .fill")!;
    pointer("pointermove", x, y, {}, face);
    expect(target.querySelector('[data-test="repeat-mark"]')).toBeNull();
    pointer("pointerdown", x, y, {}, face);
    pointer("pointermove", x - 32, y, {}, face);
    pointer("pointerup", x - 32, y, {}, face);
    expect(repeats()).toEqual([]);
    expect(app.sent).toEqual([{ type: "setClipField", clip: 4, field: "fadeOut", value: BAR / 2 }]);
  });

  it("while recording and counting in: no ⟳ and no repeat (only a choice)", () => {
    for (const st of ["recording", "countIn"] as const) {
      show(
        state([], (s) => {
          s.transport.state = st;
          s.transport.recording = { id: 1, track: 2, from: 0, take: 1, clip: null, lap: 0, overdub: false };
        }),
      );
      pointer("pointermove", 120, 10);
      expect(target.querySelector('[data-test="repeat-mark"]')).toBeNull();
      pointer("pointerdown", 120, 10);
      pointer("pointermove", 120 + 128, 10);
      pointer("pointerup", 120 + 128, 10);
      expect(repeats()).toEqual([]);
    }
  });

  it("Mod R: the box at the end of the clips, 3 at first and chosen; Enter sends one RepeatClips; the page is never reloaded", async () => {
    const s = state([1, 3]);
    show(s);
    const ev = key(s, { key: "r", ctrlKey: true });
    expect(ev.defaultPrevented).toBe(true);
    await tick();
    expect(repeatBox.clips).toEqual([1, 3]);
    const f = field()!;
    expect(f.value).toBe("3");
    expect(document.activeElement).toBe(f);
    // On C's row (C ends last, at bar 4).
    expect(f.parentElement!.style.top).toBe(`${64 + 3 + 2 + 1}px`);
    expect(dragReading.text).toBe("3 回くり返します");
    type("", { key: "ArrowUp" });
    expect(f.value).toBe("4");
    type("", { key: "ArrowDown", shiftKey: true });
    expect(f.value).toBe("1");
    type("5", { key: "Enter" });
    expect(app.sent).toEqual([{ type: "repeatClips", clips: [1, 3], count: 5 }]);
    expect(field()).toBeNull();
    expect(dragReading.text).toBeNull();
  });

  it("the box: Esc and a press outside close it without sending; what is not a number stays open (red)", async () => {
    const s = state([1]);
    show(s);
    key(s, { key: "r", ctrlKey: true });
    await tick();
    type("x", { key: "Enter" });
    expect(field()!.classList.contains("broken")).toBe(true);
    expect(dragReading.text).toBeNull();
    type("", { key: "Escape" });
    expect(field()).toBeNull();
    key(s, { key: "r", ctrlKey: true });
    await tick();
    field()!.blur();
    flushSync();
    expect(field()).toBeNull();
    expect(repeats()).toEqual([]);
    // Recording starting closes it too.
    key(s, { key: "r", ctrlKey: true });
    await tick();
    app.setState(state([1], (x) => (x.transport.state = "countIn")));
    flushSync();
    expect(field()).toBeNull();
  });

  it("Mod R does nothing with no clip chosen or while recording; 「編集 ▸ くり返す…」 opens the same box", async () => {
    const empty = state([]);
    show(empty);
    key(empty, { key: "r", ctrlKey: true });
    const busy = state([1], (x) => (x.transport.state = "recording"));
    key(busy, { key: "r", ctrlKey: true });
    expect(repeatBox.clips).toBeNull();
    expect(keyActionUsable({ kind: "repeatClips" }, deps(empty))).toBe(false);
    const s = state([3]);
    show(s);
    const row = topMenus(s, deps(s), "other")[1]!.entries.find((e): e is MenuRow => e.kind === "row" && e.label === "くり返す…")!;
    expect([row.usable, row.key]).toEqual([true, "Ctrl R"]);
    row.run!();
    flushSync();
    await tick();
    expect(repeatBox.clips).toEqual([3]);
    expect(field()).not.toBeNull();
  });

  it("the right-click menu: 「くり返す…」 after 「複製」 opens the box on the clip right-clicked", async () => {
    show(state([1, 3]));
    lanes().dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 30, clientY: 30, button: 2 }));
    flushSync();
    const rows = (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");
    const at = rows.findIndex((r) => r.label === "menu.repeatClips");
    expect(rows[at - 1]!.label).toBe("menu.duplicate");
    expect(rows[at]!.key).toBe("Ctrl R");
    rows[at]!.run();
    flushSync();
    await tick();
    expect([repeatBox.clips, repeatBox.at]).toEqual([[1, 3], 1]);
    expect(field()!.parentElement!.style.top).toBe(`${3 + 2 + 1}px`);
  });
});

describe("the fake's answer (reduceClips)", () => {
  const s = () => {
    const x = fakeState();
    x.song.tracks = tracks();
    return x;
  };
  it("copies with new ids right after each other, chosen; all or nothing with the notice of an overlap", () => {
    const done = reduceClips(s(), { type: "repeatClips", clips: [1], count: 3 })!;
    expect(done.song.tracks[0]!.clips.map((c) => [c.start / BAR, c.end / BAR])).toEqual([
      [0, 2],
      [2, 4],
      [4, 6],
      [6, 8],
      [8, 10],
    ]);
    expect(done.arrange.selectedClips).toHaveLength(3);
    const block = reduceClips(s(), { type: "repeatClips", clips: [1, 3], count: 2 })!;
    expect(block.song.tracks[1]!.clips.map((c) => c.start / BAR)).toEqual([1, 4, 7]);
    const over = reduceClips(s(), { type: "repeatClips", clips: [1], count: 4 })!;
    expect(over.notice).toEqual({ id: "clipsNotDuplicated" });
    expect(over.song).toEqual(s().song);
    expect(reduceClips(s(), { type: "repeatClips", clips: [99], count: 1 })).toBeNull();
    expect(reduceClips(s(), { type: "repeatClips", clips: [1], count: 0 })).toBeNull();
  });
});
