// 「ほかのトラックも見る ▾」 in the piano roll tool row (round 12 P2): pressing turns it on (screen-only state, kept in view.svelte.ts),
// the ▾ lists the key tracks that are not drums other than the one shown, all on at first; a row takes a track off or puts it back and the
// list stays open. On a drum track both are 「使えない」. With no other track to show, the ▾ is 「使えない」.
// The note area (Roll) fetches the shown takes of the clips in a window three widths wide and draws their notes in gray.6.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TakeNotes } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import Roll from "../src/components/pianoroll/Roll.svelte";
import Toolbar from "../src/components/pianoroll/Toolbar.svelte";
import { ghostView, prPanels, prView } from "../src/components/pianoroll/view.svelte";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { watchSlots, type GlSlot } from "../src/gl/slot";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";
import { take } from "./pianoroll-fixture";

const book = vi.hoisted(() => ({
  takes: new Map<number, unknown>(),
  asked: [] as number[],
  fetch: (_id: number): Promise<unknown> => Promise.reject(new Error("no song")),
}));
vi.mock("../src/bridge", async (importOriginal) => {
  const real = await importOriginal<typeof import("../src/bridge")>();
  return {
    ...real,
    peekTakeNotes: (id: number) => book.takes.get(id),
    takeNotes: (id: number) => {
      book.asked.push(id);
      return book.fetch(id);
    },
  };
});
vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

// jsdom does not measure sizes: the note area is 668 × 440.
const proto = HTMLElement.prototype;
const hadW = Object.getOwnPropertyDescriptor(proto, "clientWidth");
const hadH = Object.getOwnPropertyDescriptor(proto, "clientHeight");
beforeAll(() => {
  Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => 668 });
  Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 440 });
});
afterAll(() => {
  if (hadW) Object.defineProperty(proto, "clientWidth", hadW);
  if (hadH) Object.defineProperty(proto, "clientHeight", hadH);
});

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
const slots = new Map<HTMLElement, GlSlot>();
let stopWatch: () => void = () => {};
let component: Component = Toolbar;
beforeEach(() => {
  app.reset();
  prView.focus = "arrange";
  prView.shownClip = null;
  prView.drums = false;
  prPanels.list = true;
  prPanels.inspector = true;
  ghostView.on = false;
  ghostView.hidden = [];
  ghostView.serial = 0;
  component = Toolbar;
  book.takes.clear();
  book.asked.length = 0;
  slots.clear();
  stopWatch = watchSlots({
    attach: (node, slot) => slots.set(node, slot),
    update: (node, slot) => slots.set(node, slot),
    detach: (node) => slots.delete(node),
  });
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  stopWatch();
});

const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

/** The scene pianoRollGhost with the clip of track `number` shown. */
function song(number = 1, edit?: (s: AppState) => void): AppState {
  const s = fakeScene("pianoRollGhost")!.state;
  edit?.(s);
  const track = s.song.tracks[number - 1]!;
  const clip = track.clips[0] ?? null;
  if (clip) s.arrange = { ...s.arrange, pianoRollClip: clip.id, selectedClip: clip.id, selectedClips: [clip.id] };
  app.setState(s);
  shown.setShown({ clip, track: clip ? track : null, take: clip ? take([[0, 1, 60]], clip.takeId) : null });
  const transport = createFakeTransport(s);
  book.fetch = async (id) => {
    const t = decodeTake(await transport.takeNotes(id));
    book.takes.set(id, t);
    return t;
  };
  mounted = mount(component, { target });
  flushSync();
  return s;
}

const rows = () => qa("[data-test=pr-ghost-tracks] [data-row]");
const rowText = () => rows().map((r) => `${r.getAttribute("aria-checked")} ${text(r)}`);

describe("「ほかのトラックも見る ▾」", () => {
  it("off at first; pressing turns it on and off (no command: screen-only)", () => {
    song();
    const button = q("[data-test=pr-ghost]")!;
    expect(text(button)).toBe("ほかのトラックも見る");
    expect(button.getAttribute("aria-pressed")).toBe("false");
    button.click();
    flushSync();
    expect(ghostView.on).toBe(true);
    expect(button.getAttribute("aria-pressed")).toBe("true");
    expect(q("[data-test=pr-ghost-menu]")!.classList.contains("on")).toBe(true);
    button.click();
    flushSync();
    expect(ghostView.on).toBe(false);
    expect(app.sent).toEqual([]);
  });

  it("the ▾ lists the key tracks that are not drums, other than the shown one, all on; a row takes a track off and the list stays", () => {
    const s = song();
    q("[data-test=pr-ghost-menu]")!.click();
    flushSync();
    expect(text(q("[data-test=pr-ghost-tracks] .head"))).toBe("見せるトラック");
    expect(rowText()).toEqual(["true 2 ベース", "true 4 パッド"]);
    rows()[0]!.click();
    flushSync();
    expect(ghostView.hidden).toEqual([2]);
    expect(ghostView.serial).toBe(s.song.serial);
    expect(rowText()).toEqual(["false 2 ベース", "true 4 パッド"]);
    rows()[0]!.click();
    flushSync();
    expect(ghostView.hidden).toEqual([]);
    expect(app.sent).toEqual([]);
  });

  it("showing another track's clip offers the first track instead", () => {
    song(2);
    q("[data-test=pr-ghost-menu]")!.click();
    flushSync();
    expect(rowText()).toEqual(["true 1 ピアノ", "true 4 パッド"]);
  });

  it("what was taken off in another song does not count", () => {
    ghostView.hidden = [2];
    ghostView.serial = -5;
    song();
    q("[data-test=pr-ghost-menu]")!.click();
    flushSync();
    expect(rowText()).toEqual(["true 2 ベース", "true 4 パッド"]);
  });

  it("on a drum track both are 「使えない」 (the switch keeps its state for the next key track)", () => {
    ghostView.on = true;
    song(3);
    const button = q("[data-test=pr-ghost]")!;
    const menu = q("[data-test=pr-ghost-menu]")!;
    expect(button.getAttribute("aria-disabled")).toBe("true");
    expect(menu.getAttribute("aria-disabled")).toBe("true");
    expect(menu.classList.contains("on")).toBe(false);
    button.click();
    menu.click();
    flushSync();
    expect(ghostView.on).toBe(true);
    expect(q("[data-test=pr-ghost-tracks]")).toBeNull();
  });

  it("with no other track to show, the ▾ is 「使えない」", () => {
    song(1, (s) => (s.song.tracks = s.song.tracks.slice(0, 1)));
    const menu = q("[data-test=pr-ghost-menu]")!;
    expect(menu.getAttribute("aria-disabled")).toBe("true");
    menu.click();
    flushSync();
    expect(q("[data-test=pr-ghost-tracks]")).toBeNull();
    expect(q("[data-test=pr-ghost]")!.getAttribute("aria-disabled")).toBeNull();
  });
});

describe("the note area", () => {
  const GRAY6 = Array.from(Float32Array.from(rgb("color.gray.6"))).join(",");
  function ghostRects(): number[][] {
    const slot = slots.get(target.querySelector<HTMLElement>(".area .fill")!)!;
    if (slot.kind !== "paint") throw new Error(slot.kind);
    const out = new Batch();
    out.clip(0, 0, 668, 440);
    slot.paint(out, { x0: 0, y0: 0, x1: 668, y1: 440 }, { dpr: 1, scale: 100, isDown: () => false });
    return out.rects().filter((r) => [r[4], r[5], r[6]].join(",") === GRAY6 && r[3]! - r[1]! < 20);
  }

  it("fetches the takes of the clips in a window three widths wide, and draws their notes when they come", async () => {
    component = Roll;
    ghostView.on = true;
    const s = song();
    const takeOf = (name: string) => s.song.tracks.flatMap((t) => t.clips).filter((c) => c.name === name).map((c) => c.takeId);
    // 4 bars fit at 40 px a beat: the window reaches 8.3 bars to the right, so the clips from bar 13 are not asked for. Drums are not either.
    const near = [...takeOf("ベース イントロ"), ...takeOf("ベース A"), ...takeOf("コード")];
    expect([...book.asked].sort((a, b) => a - b)).toEqual(near.sort((a, b) => a - b));
    expect(ghostRects()).toEqual([]);
    await vi.waitFor(() => expect(book.takes.size).toBe(near.length));
    flushSync();
    const both = ghostRects().length;
    expect(both).toBeGreaterThan(0);

    ghostView.hidden = [2];
    ghostView.serial = s.song.serial;
    flushSync();
    const pad = ghostRects().length;
    expect(pad).toBeGreaterThan(0);
    expect(pad).toBeLessThan(both);

    ghostView.on = false;
    flushSync();
    expect(ghostRects()).toEqual([]);
    expect(book.asked).toHaveLength(near.length);
  });

  it("asks for nothing while it is off", () => {
    component = Roll;
    song();
    expect(book.asked).toEqual([]);
    expect(ghostRects()).toEqual([]);
  });
});
