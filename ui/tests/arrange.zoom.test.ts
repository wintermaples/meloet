// Horizontal zoom of the arrange (− ＋, Mod + wheel), the horizontal scroll bar, and the overview's「見えている範囲の枠」(spec 3, mock R3-Arrange, A-PianoRoll).
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView } from "../src/bridge";
import Arrange from "../src/components/Arrange.svelte";
import { arrangeView, DEFAULT_ARRANGE_BAR, prView } from "../src/components/pianoroll/view.svelte";
import { watchSlots, type GlRect, type GlSlot } from "../src/gl/slot";
import HScrollbar from "../src/lib/HScrollbar.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 4 * TPQ;
const WIDTH = 1000;

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
const hadH = Object.getOwnPropertyDescriptor(proto, "clientHeight");
beforeAll(() => {
  Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH });
  Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 39 });
});
afterAll(() => {
  if (had) Object.defineProperty(proto, "clientWidth", had);
  if (hadH) Object.defineProperty(proto, "clientHeight", hadH);
});

const slots = new Map<HTMLElement, GlSlot>();
let stopWatching: () => void = () => {};
let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  slots.clear();
  stopWatching = watchSlots({
    attach: (n, s) => slots.set(n, s),
    update: (n, s) => slots.set(n, s),
    detach: (n) => slots.delete(n),
  });
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  stopWatching();
  arrangeView.barPx = DEFAULT_ARRANGE_BAR;
  Object.assign(prView, { scrollTick: 0, areaWidth: 0, follow: true, focus: "arrange" });
});

const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

function clip(id: number, fromBar: number, toBar: number): ClipView {
  return { id, name: `c${id}`, start: (fromBar - 1) * BAR, end: (toBar - 1) * BAR, take: 1, takes: 1, takeId: 100 + id, takeOffset: 0, noteCount: 0, audio: null };
}

function show(state: AppState): void {
  app.setState(state);
  mounted = mount(Arrange, { target });
  flushSync();
}

const longSong = () => app.stateWith((s) => (s.song.tracks[0]!.clips = [clip(1, 1, 5), clip(2, 30, 34)]));

describe("the − ＋ and「グリッド 1 小節」in the arrange heading row (mock R3-Arrange)", () => {
  it("the heading row is「アレンジ − ＋ グリッド 1 小節」", () => {
    show(longSong());
    expect(text(q(".head .row"))).toBe("アレンジ − ＋ グリッド 1 小節");
    expect(qa(".zoom-btn").map((b) => b.getAttribute("aria-label"))).toEqual(["横に縮小", "横に拡大"]);
  });

  it("＋ makes 1 bar 64 → 91 px. The ruler numbers move with it. − goes back", () => {
    show(longSong());
    qa(".zoom-btn")[1]!.click();
    flushSync();
    expect(arrangeView.barPx).toBe(91);
    expect(qa(".bar-number")[1]!.style.left).toBe("91px");
    qa(".zoom-btn")[0]!.click();
    flushSync();
    expect(qa(".bar-number")[1]!.style.left).toBe("64px");
  });

  it("when zoomed out and the numbers overlap, skip some (at 16 px per bar, every 2 bars)", () => {
    arrangeView.barPx = 16;
    show(longSong());
    expect(qa(".bar-number").slice(0, 3).map(text)).toEqual(["1", "3", "5"]);
  });

  it("Ctrl + wheel zooms around the cursor (the tick there stays at the same place on screen)", () => {
    show(longSong());
    const time = q(".time")!;
    // Cursor at 640 px from the song start (start of bar 11). The left edge is 0.
    time.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: -100, ctrlKey: true, clientX: 640 }));
    flushSync();
    expect(arrangeView.barPx).toBe(91);
    // The start of bar 11 (10 × 91 = 910 px) is still at 640 px: the left edge is 270 px.
    // The numbers are placed by x from the song start, and the whole number strip is moved with transform.
    const eleven = qa(".bar-number").find((b) => text(b) === "11")!;
    const shift = Number(/translateX\((-?[\d.]+)px\)/.exec(q(".bar-strip")!.style.transform)![1]);
    expect(parseFloat(eleven.style.left) + shift).toBeCloseTo(640, 5);
  });

  it("pressing the arrange makes Mod ＋, Mod − go to the arrange", () => {
    prView.focus = "pianoRoll";
    show(longSong());
    q(".lanes")!.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0 }));
    expect(prView.focus).toBe("arrange");
  });
});

describe("scrolling sideways (architecture 23-13)", () => {
  const lane = () => slots.get(q(".lanes > .fill")!) as Extract<GlSlot, { kind: "rects" }>;
  const sideways = (dx: number) => {
    q(".time")!.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: dx, shiftKey: true }));
    flushSync();
  };
  /** Screen x of the border of clip 2 (from bar 30). */
  const secondClipX = () => {
    const s = lane();
    return Math.min(...s.rects.filter((r) => r.color === "color.gray.6").map((r) => r.x - (s.shiftX ?? 0)).filter((x) => x > 500));
  };

  it("the rectangles of the big surface are not rebuilt; shiftX changes by the scrolled amount. The ruler numbers move as a strip with transform", () => {
    show(longSong());
    const before = lane();
    expect(secondClipX()).toBeCloseTo(29 * 64, 6);
    const strip = q(".bar-strip")!;
    const fifth = () => qa(".bar-number").find((b) => text(b) === "5")!;
    expect(fifth().style.left).toBe("256px");
    sideways(100);
    expect(lane().rects).toBe(before.rects);
    expect(lane().shiftX! - before.shiftX!).toBeCloseTo(100, 6);
    expect(secondClipX()).toBeCloseTo(29 * 64 - 100, 6);
    expect(strip.style.transform).toBe("translateX(-100px)");
    expect(fifth().style.left).toBe("256px");
  });

  it("the tag of a long clip over the left edge keeps its width when scrolled (cut at two visible widths)", () => {
    show(app.stateWith((s) => (s.song.tracks[0]!.clips = [clip(1, 1, 40)])));
    const label = () => q(".clip-label")!;
    sideways(100);
    expect([label().style.left, label().style.width]).toEqual(["1px", `${2 * WIDTH - 2}px`]);
    sideways(100);
    expect([label().style.left, label().style.width]).toEqual(["1px", `${2 * WIDTH - 2}px`]);
  });

  it("scrolling by one visible width rebuilds the window (screen x uses the same formula)", () => {
    show(longSong());
    const before = lane();
    sideways(WIDTH + 50);
    expect(lane().rects).not.toBe(before.rects);
    expect(secondClipX()).toBeCloseTo(29 * 64 - WIDTH - 50, 6);
  });
});

describe("horizontal scroll bar (spec 2 (Scroll bar), laid on its side)", () => {
  it("not shown when the song fits. When it does not fit, laid over the bottom of the track rows", () => {
    show(app.stateWith((s) => (s.song.tracks[0]!.clips = [clip(1, 1, 5)])));
    expect(q('[data-testid="hscrollbar"]')).toBeNull();
    unmount(mounted!);
    mounted = null;
    show(longSong());
    const bar = q('[data-testid="hscrollbar"]')!;
    expect(bar.parentElement!.classList.contains("lanes-box")).toBe(true);
    // Right end: the start of bar 34 (33 × 64 = 2112 px) up to the middle of the screen. Content is 2112 − 500 + 1000 = 2612.
    expect(Number(bar.dataset.total)).toBeCloseTo(2612, 5);
    expect(bar.dataset.visible).toBe(String(WIDTH));
  });

  it("pressing outside the handle scrolls one screen (does not follow until the next play)", () => {
    show(longSong());
    const bar = q('[data-testid="hscrollbar"]')!;
    bar.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 900 }));
    flushSync();
    expect(q('[data-testid="hscrollbar"]')!.dataset.offset).toBe("1000");
    expect(text(q(".bar-number"))).toBe("16");
  });

  it("HScrollbar alone: the handle length is in proportion, shortest 8. Dragging moves it", () => {
    const seen: number[] = [];
    app.setState(app.stateWith(() => {}));
    mounted = mount(HScrollbar, { target, props: { total: 100_000, visible: 1000, offset: 0, onscroll: (o: number) => seen.push(o) } });
    flushSync();
    const thumb = q('[data-testid="hscrollbar-thumb"]')!;
    expect(thumb.style.width).toBe("10px");
    thumb.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 0 }));
    thumb.dispatchEvent(new MouseEvent("pointermove", { bubbles: true, clientX: 100 }));
    expect(seen.at(-1)).toBeCloseTo(10_000, 5);
    unmount(mounted);
    mounted = mount(HScrollbar, { target, props: { total: 1_000_000, visible: 1000, offset: 0, onscroll: () => {} } });
    flushSync();
    expect(q('[data-testid="hscrollbar-thumb"]')!.style.width).toBe("8px");
  });
});

describe("the overview's「見えている範囲の枠」", () => {
  const overview = (pianoRoll: boolean) =>
    app.stateWith((s) => {
      s.song.tracks[0]!.clips = [clip(1, 1, 5), clip(2, 30, 34)];
      s.workspace.layout = { ...s.workspace.layout, arrangeOverview: true, arrangeHeight: 40, lower: pianoRoll ? [{ band: "pianoRoll", height: 600 }] : [] };
    });
  const accent = () => {
    const rects = [...slots.values()].flatMap((s) => (s.kind === "rects" ? s.rects : [])) as GlRect[];
    return rects.filter((r) => r.color === "color.accent");
  };

  it("with the piano roll closed, the frame fills the overview as before (the overview slot)", () => {
    show(overview(false));
    expect(qa(".overview-slot [data-gl-slot]").map((e) => e.dataset.glSlot)).toEqual(["rects", "overview"]);
    expect(accent()).toEqual([]);
  });

  it("with the piano roll open, the range visible in the piano roll (A-PianoRoll: bars 1–5 → x 0–256)", () => {
    Object.assign(prView, { scrollTick: 0, areaWidth: 1280, quarterPx: 80 });
    show(overview(true));
    expect(qa(".overview-slot [data-gl-slot]").map((e) => e.dataset.glSlot)).toEqual(["rects"]);
    const top = accent()[0]!;
    expect(top.x).toBe(0);
    expect(top.w).toBe(256);
  });

  it("dragging the frame scrolls the piano roll sideways (moved by hand, so it does not follow until the next play)", () => {
    Object.assign(prView, { scrollTick: 0, areaWidth: 1280, quarterPx: 80 });
    show(overview(true));
    const slot = q(".overview-slot")!;
    slot.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 100 }));
    slot.dispatchEvent(new MouseEvent("pointermove", { bubbles: true, clientX: 164 }));
    slot.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));
    flushSync();
    expect(prView.scrollTick).toBeCloseTo(BAR, 5);
    expect(prView.follow).toBe(false);
    expect(accent()[0]!.x).toBe(64);
  });

  it("when the piano roll is scrolled outside the overview, the overview scrolls to show the frame", () => {
    Object.assign(prView, { scrollTick: 0, areaWidth: 1280, quarterPx: 80 });
    show(overview(true));
    prView.scrollTick = 20 * BAR;
    flushSync();
    const top = accent()[0]!;
    expect(top.x).toBeGreaterThanOrEqual(0);
    expect(top.x + top.w).toBeLessThanOrEqual(WIDTH);
  });
});

describe("scrolling the track rows up and down (temporary form, user:2026-11-20)", () => {
  /** 6 tracks (row height 64 × 6 = 384). The visible height is 39, from the beforeAll above. */
  const six = (target = 100) =>
    app.stateWith((s) => {
      const first = s.song.tracks[0]!;
      s.song.tracks = Array.from({ length: 6 }, (_, i) => ({ ...first, id: 100 + i, number: i + 1, clips: i === 0 ? [clip(1, 1, 5)] : [] }));
      s.song.playTarget = target;
    });
  const wheel = (el: Element, init: WheelEventInit) => {
    const e = new WheelEvent("wheel", { bubbles: true, cancelable: true, ...init });
    el.dispatchEvent(e);
    flushSync();
    return e;
  };
  const tops = () => [q(".lanes-view")!.scrollTop, q(".heads-view")!.scrollTop];

  it("a plain wheel scrolls the heading column and the lanes by the same amount. Stops at the bottom. The top row is outside the scrolling box", () => {
    show(six());
    expect(tops()).toEqual([0, 0]);
    const e = wheel(q(".time")!, { deltaY: 30 });
    expect(e.defaultPrevented).toBe(true);
    expect(tops()).toEqual([30, 30]);
    wheel(q(".head")!, { deltaY: 50 });
    expect(tops()).toEqual([80, 80]);
    wheel(q(".time")!, { deltaY: 10000 });
    expect(tops()).toEqual([384 + 32 - 39, 384 + 32 - 39]);
    wheel(q(".time")!, { deltaY: -10000 });
    expect(tops()).toEqual([0, 0]);
    for (const sel of [".ruler", ".tempo-head", ".title-row"]) expect(q(sel)!.closest(".lanes-view, .heads-view"), sel).toBeNull();
    expect(q(".heads-view")!.dataset.glRegion).toBe("trackMeters");
    expect(q(".lanes-view")!.hasAttribute("data-gl-clip")).toBe(true);
  });

  it("Shift + wheel stays sideways and Mod + wheel stays zoom (no vertical scroll)", () => {
    show(six());
    wheel(q(".time")!, { deltaY: 40, shiftKey: true });
    expect(tops()).toEqual([0, 0]);
    wheel(q(".time")!, { deltaY: -100, ctrlKey: true });
    expect(tops()).toEqual([0, 0]);
  });

  it("a vertical scroll bar at the right end (not shown when it fits). It can scroll", () => {
    show(six());
    const bar = q("[data-test=arrange-vscrollbar]")!;
    expect(bar.parentElement!.classList.contains("lanes-box")).toBe(true);
    bar.getBoundingClientRect = () => new DOMRect(0, 0, 8, 39);
    bar.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true, button: 0, clientY: 38 }));
    flushSync();
    expect(tops()[0]).toBeGreaterThan(0);
    expect(tops()[0]).toBe(tops()[1]);
    unmount(mounted!);
    mounted = null;
    const h = Object.getOwnPropertyDescriptor(proto, "clientHeight")!;
    Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 1000 });
    try {
      show(six());
      expect(q("[data-test=arrange-vscrollbar]")).toBeNull();
      expect(wheel(q(".time")!, { deltaY: 30 }).defaultPrevented).toBe(false);
    } finally {
      Object.defineProperty(proto, "clientHeight", h);
    }
  });

  it("when the selected track changes, scroll until that track is visible. Do not scroll if it stays the same", () => {
    show(six());
    app.setState(six(105));
    flushSync();
    // Row 6 is 320–384. It is taller than the visible 39, so align to the top of the row.
    expect(tops()).toEqual([320, 320]);
    wheel(q(".time")!, { deltaY: -100 });
    expect(tops()).toEqual([220, 220]);
    app.setState(six(105));
    flushSync();
    expect(tops()).toEqual([220, 220]);
    app.setState(six(100));
    flushSync();
    expect(tops()).toEqual([0, 0]);
  });

  it("do not scroll to a track selected with the mouse. Scroll to a track selected by keys or added (spec 3, the rest of 92)", () => {
    show(six());
    document.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0 }));
    app.setState(six(105));
    flushSync();
    expect(tops()).toEqual([0, 0]);
    // An added track is scrolled to even when added with the mouse.
    app.setState(
      app.stateWith((s) => {
        const first = s.song.tracks[0]!;
        s.song.tracks = Array.from({ length: 7 }, (_, i) => ({ ...first, id: 100 + i, number: i + 1, clips: [] }));
        s.song.playTarget = 106;
      }),
    );
    flushSync();
    expect(tops()).toEqual([384, 384]);
    wheel(q(".time")!, { deltaY: -10000 });
    document.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "ArrowDown" }));
    app.setState(
      app.stateWith((s) => {
        const first = s.song.tracks[0]!;
        s.song.tracks = Array.from({ length: 7 }, (_, i) => ({ ...first, id: 100 + i, number: i + 1, clips: [] }));
        s.song.playTarget = 105;
      }),
    );
    flushSync();
    expect(tops()).toEqual([320, 320]);
  });
});
