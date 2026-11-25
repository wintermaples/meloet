// The highlighted row (user:2026-11-25): the row under the pointer, from the key column to the right edge of the note area, as the row's
// ground (under the grid lines and the notes); while notes are dragged, the row where the grabbed note lies. Only a look: it goes when the
// pointer leaves. And the letters of the key column: every row carries its letter (sharps, the octave only on C) when a row is as tall as
// the label's text; only the C rows below that. Drum rows keep their names.
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, TakeNotes, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { chordPick } from "../src/components/pianoroll/chordPick.svelte";
import { SINGLE } from "../src/components/pianoroll/chords";
import { heldRow, type Drag } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import { DRUM_NAME_HIGHLIGHT, paintRoll, paintRollKeys, ROW_HIGHLIGHT, type RollPicture } from "../src/components/pianoroll/paint";
import Roll from "../src/components/pianoroll/Roll.svelte";
import { cancelDrag, drumView, prPanels, prView } from "../src/components/pianoroll/view.svelte";
import { Batch } from "../src/gl/batch";
import { rgb, setColorTheme } from "../src/gl/color";
import { DEFAULT_THEME } from "../src/design/themes";
import { watchSlots, type GlSlot } from "../src/gl/slot";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";
import { GEOM, take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const colorOf = (r: number[]) => [r[4], r[5], r[6]].join(",");
const is = (name: Parameters<typeof rgb>[0]) => Array.from(Float32Array.from(rgb(name))).join(",");
/** A batch that keeps what falls in w × h (a batch keeps nothing until it has a clip box). */
function clipped(w: number, h: number): Batch {
  const out = new Batch();
  out.clip(0, 0, w, h);
  return out;
}

describe("the letters of the key column (geom)", () => {
  it("C rows keep the octave; with every letter the others get theirs alone, black keys with a sharp and never a flat", () => {
    expect(G.rowLabel(60, true)).toBe("C4");
    expect(G.rowLabel(48, false)).toBe("C3");
    expect(G.rowLabel(0, false)).toBe("C-1");
    expect([61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71].map((k) => G.rowLabel(k, true))).toEqual([
      "C#",
      "D",
      "D#",
      "E",
      "F",
      "F#",
      "G",
      "G#",
      "A",
      "A#",
      "B",
    ]);
    expect(G.rowLabel(61, false)).toBeNull();
    expect(G.rowLabel(127, true)).toBe("G");
  });

  it("every letter only when a row is as tall as the label's text size (text.s: 12 at 100 %, 15 at 125 %)", () => {
    expect(G.allRowLetters(8, 12)).toBe(false);
    expect(G.allRowLetters(12, 12)).toBe(true);
    expect(G.allRowLetters(10, 15)).toBe(false);
    expect(G.allRowLetters(15, 15)).toBe(true);
  });
});

describe("the row a drag holds (heldRow)", () => {
  const n = G.shownOf(take([[0, TPQ, 60], [TPQ, TPQ, 64]]), 0);
  const press = { x: 0, y: 0, moved: true };
  it("a move: the grabbed note's row moved by the keys the move allows; a length change: the grabbed note's row", () => {
    const move: Drag = { kind: "move", press, notes: [0, 1], grab: 1, copy: false, ticks: 0, keys: 3, selectOnClick: false };
    expect(heldRow(move, null, n)).toBe(67);
    expect(heldRow({ ...move, keys: 0 }, null, n)).toBe(64);
    expect(heldRow({ kind: "resize", press, notes: [0], grab: 0, edge: "end", ticks: TPQ }, null, n)).toBe(60);
  });
  it("drawing: the row (or chord root) being written; the chord shadow: its root; other drags and nothing held: none", () => {
    expect(heldRow({ kind: "draw", press, start: 0, key: 55, keys: [55, 59, 62], length: TPQ, velocity: 100 }, null, n)).toBe(55);
    const shadow = { start: 0, length: TPQ, keys: [67, 71, 74], key: 64 };
    expect(heldRow(null, shadow, n, { kind: "key", root: 7, triad: "major" })).toBe(67);
    expect(heldRow(null, shadow, n, { kind: "shape", shape: "major" })).toBe(64);
    expect(heldRow({ kind: "box", press, x: 0, y: 0, add: false, hits: [] }, null, n)).toBeNull();
    expect(heldRow({ kind: "velocity", press, notes: [0], grab: 0, delta: 3 }, null, n)).toBeNull();
    expect(heldRow({ kind: "erase", press, x: 0, y: 0, hits: new Set() }, null, n)).toBeNull();
    expect(heldRow(null, null, n)).toBeNull();
  });
});

describe("the highlighted row in the pictures (paint input `row`)", () => {
  const BOX = { x0: 0, y0: 0, x1: 696, y1: 492 };
  const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8 };
  function picture(over: Partial<RollPicture> = {}): RollPicture {
    return {
      g: GEOM,
      sizes: SIZES,
      scale: { root: 0, mode: "major", lock: true, highlight: true },
      meter: { num: 4, den: 4 },
      step: TPQ / 4,
      shown: G.shownOf(take([[0, TPQ, 64, 100], [0, TPQ, 61, 100]]), 0),
      origin: null,
      selected: null,
      live: null,
      drag: null,
      eraseHover: -1,
      ...over,
    };
  }
  const rowRect = (rects: number[][], key: number) => rects.find((r) => r[0] === 0 && r[1] === (72 - key) * 12 && r[2] === 696);
  function draw(p: RollPicture): number[][] {
    const out = new Batch();
    out.clip(BOX.x0, BOX.y0, BOX.x1, BOX.y1);
    paintRoll(out, BOX, p, null, 1);
    return out.rects();
  }

  it("the row's ground becomes the highlight, on a white-key row and on a row outside a locked scale alike; the other rows keep theirs", () => {
    const rects = draw(picture({ row: 64 }));
    expect(colorOf(rowRect(rects, 64)!)).toBe(is(ROW_HIGHLIGHT));
    expect(colorOf(rowRect(rects, 65)!)).toBe(is("color.row.white"));
    expect(colorOf(rowRect(rects, 63)!)).toBe(is("color.row.out"));
    const out = draw(picture({ row: 61 }));
    expect(colorOf(rowRect(out, 61)!)).toBe(is(ROW_HIGHLIGHT));
    expect(draw(picture()).some((r) => r[2] === 696 && colorOf(r) === is(ROW_HIGHLIGHT))).toBe(false);
  });

  it("drawn under the grid lines and the notes: the note on the row and the lines come after its ground", () => {
    const rects = draw(picture({ row: 64 }));
    const ground = rects.indexOf(rowRect(rects, 64)!);
    const y = (72 - 64) * 12;
    const note = rects.findIndex((r) => r[1] === y && r[3] === y + 11 && colorOf(r) === is("color.accent"));
    const line = rects.findIndex((r) => r[0] === 0 && r[2] === 2 && colorOf(r) === is("color.line.bar"));
    expect(note).toBeGreaterThan(ground);
    expect(line).toBeGreaterThan(ground);
  });

  it("the key column: the key of the row gets the highlight; a lit key keeps its lit look", () => {
    const keys = (row: number | null, lit: (k: number) => boolean) => {
      const out = clipped(56, 492);
      paintRollKeys(out, { x0: 0, y0: 0, x1: 56, y1: 492 }, GEOM, lit, 1, undefined, row);
      return out.rects();
    };
    const at = (rects: number[][], key: number) => rects.find((r) => r[0] === 0 && r[1] === (72 - key) * 12 && r[2] === 56)!;
    expect(colorOf(at(keys(61, () => false), 61))).toBe(is(ROW_HIGHLIGHT));
    expect(colorOf(at(keys(61, () => false), 62))).toBe(is("color.key.white"));
    expect(colorOf(at(keys(61, (k) => k === 61), 61))).toBe(is("color.accent"));
    expect(colorOf(at(keys(null, () => false), 61))).toBe(is("color.key.black"));
  });

  it("drum rows: the note area row and the name column row (gray.6 on the gray.5 column); a lit row keeps gray.5 and its mark", () => {
    const g: G.RollGeom = { ...GEOM, rowH: 36, drum: { keys: [49, 51, 46, 44, 42, 50, 47, 45, 39, 38, 37, 36], top: 0, hitW: 8 } };
    const out = new Batch();
    out.clip(BOX.x0, BOX.y0, BOX.x1, BOX.y1);
    paintRoll(out, BOX, picture({ g, shown: null, row: 39 }), null, 1);
    const row = (rects: number[][], i: number, w: number) => rects.find((r) => r[0] === 0 && r[1] === i * 36 && r[2] === w)!;
    expect(colorOf(row(out.rects(), 8, 696))).toBe(is(ROW_HIGHLIGHT));
    expect(colorOf(row(out.rects(), 9, 696))).toBe(is("color.row.black"));
    const names = (lit: (k: number) => boolean) => {
      const b = clipped(132, 440);
      paintRollKeys(b, { x0: 0, y0: 0, x1: 132, y1: 440 }, g, lit, 1, undefined, 39);
      return b.rects();
    };
    expect(colorOf(row(names(() => false), 8, 132))).toBe(is(DRUM_NAME_HIGHLIGHT));
    expect(colorOf(row(names(() => false), 9, 132))).toBe(is("color.gray.5"));
    const lit = names((k) => k === 39);
    expect(colorOf(row(lit, 8, 132))).toBe(is("color.gray.5"));
    expect(lit.some((r) => r[1] === 8 * 36 && r[2] === 2 && colorOf(r) === is("color.accent.ink"))).toBe(true);
  });
});

// ---- Mounted ----

// jsdom does not measure sizes: the note area is 668 × 440, and boxes are at 0 (a pointer's client place is its place in the note area).
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
const updates = new Map<HTMLElement, number>();
let stopWatch: () => void = () => {};
beforeEach(() => {
  app.reset();
  // The light colors: there the highlight (gray.5) differs from every key and row (in the dark themes it is the white keys' gray).
  setColorTheme("light");
  prView.tool = "select";
  prView.shownClip = null;
  prView.drums = false;
  prView.follow = false;
  prView.rowPx = 12;
  prPanels.list = true;
  prPanels.inspector = true;
  drumView.rowPx = null;
  drumView.topRow = 0;
  chordPick.pick = SINGLE;
  slots.clear();
  updates.clear();
  stopWatch = watchSlots({
    attach: (node, slot) => slots.set(node, slot),
    update: (node, slot) => {
      slots.set(node, slot);
      updates.set(node, (updates.get(node) ?? 0) + 1);
    },
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
  setColorTheme(DEFAULT_THEME);
  prView.tool = "select";
  prView.rowPx = 12;
  chordPick.pick = SINGLE;
});

const q = (sel: string) => target.querySelector<HTMLElement>(sel)!;
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const pointer = (el: Element, type: string, init: PointerEventInit = {}) =>
  el.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, ...init }));
/** The middle of the row of `key` (rows of 12 px). */
const yOf = (key: number) => (prView.topKey - key) * 12 + 6;
const hover = (key: number, x = 300) => {
  pointer(q(".area"), "pointermove", { clientX: x, clientY: yOf(key) });
  flushSync();
};

/** The pianoRoll scene, its clip showing `notes` when given (selected: the first `selected` of them). */
async function open(notes?: [number, number, number, number?][], selected = 0, edit?: (s: AppState) => void): Promise<AppState> {
  const s = fakeScene("pianoRoll")!.state;
  edit?.(s);
  const track = s.song.tracks[0]!;
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  let t: TakeNotes = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  if (notes) {
    t = take(notes, clip.takeId);
    s.pianoRoll = { ...s.pianoRoll, take: t.id, selectedNotes: selected > 0 ? [{ start: 0, end: selected }] : [] };
  }
  app.setState(s);
  shown.setShown({ clip, track, take: t });
  mounted = mount(Roll, { target });
  flushSync();
  return s;
}

const CTX = { dpr: 1, scale: 100, isDown: () => false };
/** The keys whose row the note area paints with the highlight. */
function rollRows(): number[] {
  const slot = slots.get(q(".area .fill"))!;
  if (slot.kind !== "paint") throw new Error(slot.kind);
  const out = new Batch();
  out.clip(0, 0, 668, 440);
  slot.paint(out, { x0: 0, y0: 0, x1: 668, y1: 440 }, CTX);
  return out
    .rects()
    .filter((r) => r[0] === 0 && r[2] === 668 && colorOf(r) === is(ROW_HIGHLIGHT))
    .map((r) => prView.topKey - Math.round(r[1]! / 12));
}
/** The keys the key column paints with the highlight. */
function keyRows(): number[] {
  const slot = slots.get(q(".key-shapes"))!;
  if (slot.kind !== "paint") throw new Error(slot.kind);
  const out = clipped(56, 440);
  slot.paint(out, { x0: 0, y0: 0, x1: 56, y1: 440 }, CTX);
  return out
    .rects()
    .filter((r) => r[2] === 56 && colorOf(r) === is(ROW_HIGHLIGHT))
    .map((r) => prView.topKey - Math.round(r[1]! / 12));
}
/** Where the band of the name field is (CSS px from the top), or null when hidden. */
function band(): number | null {
  const el = q("[data-test=pr-row-band]");
  if (el.style.visibility === "hidden") return null;
  return Number(/translateY\((-?[\d.]+)px\)/.exec(el.style.transform)![1]);
}
/**
 * The highlighted row seen in the note area, the name field and (with `keys`) the key column; they must agree. While a drag plays
 * notes the key column lights them instead (a lit key keeps its lit look), so drags check only the other two.
 */
function highlighted(keys = true): number | null {
  const roll = rollRows();
  const y = band();
  const fromBand = y === null ? null : prView.topKey - Math.round(y / 12);
  if (keys) expect(keyRows()).toEqual(roll);
  expect(roll.length).toBeLessThanOrEqual(1);
  expect(fromBand).toBe(roll[0] ?? null);
  return roll[0] ?? null;
}

describe("the letters of the key column (mounted)", () => {
  it("at the first row height (12) every row in view has its letter, C rows with the octave; the label boxes are placed from key 127 down", async () => {
    await open();
    const labels = qa(".key-name");
    expect(labels).toHaveLength(128);
    const textOf = (key: number) => labels.find((l) => Number(l.dataset.key) === key)!.textContent;
    expect([60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72].map(textOf)).toEqual([
      "C4",
      "C#",
      "D",
      "D#",
      "E",
      "F",
      "F#",
      "G",
      "G#",
      "A",
      "A#",
      "B",
      "C5",
    ]);
    expect(labels.some((l) => /b/.test(l.textContent ?? ""))).toBe(false);
    expect(textOf(64) && labels.find((l) => l.dataset.key === "64")!.style.top).toBe(`${(127 - 64) * 12}px`);
    expect(q(".key-labels").style.transform).toBe(`translateY(${-(127 - prView.topKey) * 12}px)`);
  });

  it("at the row height 8 only the C rows, as before", async () => {
    prView.rowPx = 8;
    await open();
    const texts = qa(".key-name").map((l) => l.textContent);
    expect(texts).toEqual(["C9", "C8", "C7", "C6", "C5", "C4", "C3", "C2", "C1", "C0", "C-1"]);
  });

  it("at 125 %: every letter from the row height 12 (15 px rows, 15 px text), only C at 8 (10 px rows)", async () => {
    await open(undefined, 0, (s) => (s.workspace.scale = 125));
    expect(qa(".key-name")).toHaveLength(128);
    unmount(mounted!);
    mounted = null;
    prView.rowPx = 8;
    await open(undefined, 0, (s) => (s.workspace.scale = 125));
    expect(qa(".key-name")).toHaveLength(11);
  });

  it("a vertical scroll moves only the box around the labels: no label changes", async () => {
    await open();
    const before = qa(".key-name");
    const tops = before.map((l) => l.style.top);
    prView.topKey -= 5;
    flushSync();
    const after = qa(".key-name");
    expect(after.every((l, i) => l === before[i])).toBe(true);
    expect(after.map((l) => l.style.top)).toEqual(tops);
    expect(q(".key-labels").style.transform).toBe(`translateY(${-(127 - prView.topKey) * 12}px)`);
  });
});

describe("the highlighted row (mounted)", () => {
  it("none at first; the row under the pointer in the note area, in the note area, the key column and the name field at once", async () => {
    await open();
    expect(highlighted()).toBeNull();
    hover(64);
    expect(highlighted()).toBe(64);
    hover(61, 20);
    expect(highlighted()).toBe(61);
  });

  it("over the key column too (left of the note area)", async () => {
    await open();
    pointer(q(".keys"), "pointermove", { clientX: -20, clientY: yOf(67) });
    flushSync();
    expect(highlighted()).toBe(67);
  });

  it("gone when the pointer leaves the piano roll, goes below the note area (velocity lane), or the window loses focus", async () => {
    await open();
    hover(64);
    pointer(q(".roll"), "pointerleave");
    flushSync();
    expect(highlighted()).toBeNull();
    hover(64);
    pointer(q(".bars"), "pointermove", { clientX: 300, clientY: 470 });
    flushSync();
    expect(highlighted()).toBeNull();
    hover(64);
    window.dispatchEvent(new Event("blur"));
    flushSync();
    expect(highlighted()).toBeNull();
  });

  it("a move inside one row asks for no picture; crossing a row asks once for the note area and the key column", async () => {
    await open();
    hover(64);
    const roll = q(".area .fill");
    const keys = q(".key-shapes");
    const labels = qa(".key-name");
    const [r0, k0] = [updates.get(roll) ?? 0, updates.get(keys) ?? 0];
    hover(64, 320);
    hover(64, 340);
    expect([updates.get(roll) ?? 0, updates.get(keys) ?? 0]).toEqual([r0, k0]);
    hover(65, 340);
    expect([updates.get(roll) ?? 0, updates.get(keys) ?? 0]).toEqual([r0 + 1, k0 + 1]);
    expect(qa(".key-name").every((l, i) => l === labels[i])).toBe(true);
  });

  it("a scroll puts another row under a still pointer", async () => {
    await open();
    hover(64);
    prView.topKey -= 2;
    flushSync();
    expect(highlighted()).toBe(62);
  });

  it("moving notes: the row where the grabbed note lies, also where the move stops before the pointer (key 127)", async () => {
    // Two selected notes, C9 (120) and F#9 (126); C9 is grabbed and the pointer goes 5 rows up: the group can go only 1.
    await open([[0, TPQ, 120], [0, TPQ, 126]], 2);
    expect(prView.topKey).toBe(127);
    const x = 10;
    pointer(q(".area"), "pointerdown", { clientX: x, clientY: yOf(120) });
    flushSync();
    expect(highlighted(false)).toBe(120);
    pointer(q(".area"), "pointermove", { clientX: x, clientY: yOf(125) });
    flushSync();
    expect(highlighted(false)).toBe(121);
    // Esc: the drag stops, the row under the pointer again.
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(highlighted(false)).toBe(125);
  });

  it("moving a note across rows follows the note; on release the row under the pointer", async () => {
    await open([[0, TPQ, 120]], 1);
    pointer(q(".area"), "pointerdown", { clientX: 10, clientY: yOf(120) });
    pointer(q(".area"), "pointermove", { clientX: 10, clientY: yOf(117) });
    flushSync();
    expect(highlighted(false)).toBe(117);
    pointer(q(".area"), "pointerup", { clientX: 10, clientY: yOf(115) });
    pointer(q(".area"), "pointermove", { clientX: 10, clientY: yOf(115) });
    flushSync();
    expect(highlighted(false)).toBe(115);
  });

  it("the draw tool: the row being written (A8, in the locked C major) stays while the pointer drags its length up or down", async () => {
    await open([[0, TPQ, 120]]);
    prView.tool = "draw";
    flushSync();
    pointer(q(".area"), "pointerdown", { clientX: 300, clientY: yOf(117) });
    flushSync();
    expect(highlighted(false)).toBe(117);
    pointer(q(".area"), "pointermove", { clientX: 420, clientY: yOf(113) });
    flushSync();
    expect(highlighted(false)).toBe(117);
  });

  it("the chord shadow of a chord of the key: the root it would be written at, not the row under the pointer", async () => {
    await open([[0, TPQ, 120]]);
    prView.tool = "draw";
    chordPick.pick = { kind: "key", root: 7, triad: "major" };
    flushSync();
    // Over G#8 (116) a chord on G is written on the nearest G: G8 (115).
    hover(116);
    expect(highlighted()).toBe(115);
    chordPick.pick = { kind: "shape", shape: "major" };
    flushSync();
    hover(116, 320);
    expect(highlighted()).toBe(116);
  });

  it("box selecting keeps the row under the pointer", async () => {
    await open([[0, TPQ, 120]]);
    pointer(q(".area"), "pointerdown", { clientX: 300, clientY: yOf(110) });
    pointer(q(".area"), "pointermove", { clientX: 400, clientY: yOf(105) });
    flushSync();
    expect(highlighted()).toBe(105);
  });
});

describe("the highlighted row on drum rows (mounted)", () => {
  async function drums(edit?: (s: AppState) => void): Promise<{ clip: ClipView; track: TrackView }> {
    const s = fakeScene("drumRoll")!.state;
    edit?.(s);
    const track = s.song.tracks.find((x) => x.clips.some((c) => c.id === s.arrange.pianoRollClip))!;
    const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
    const t = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
    app.setState(s);
    shown.setShown({ clip, track, take: t });
    mounted = mount(Roll, { target });
    flushSync();
    return { clip, track };
  }

  it("the row under the pointer in the note area and the name column; the names stay as they are; no letters", async () => {
    await drums();
    const names = qa(".drum-name");
    const texts = names.map((n) => n.textContent);
    expect(qa(".key-name")).toHaveLength(0);
    expect(target.querySelector("[data-test=pr-row-band]")).toBeNull();
    const rowH = parseFloat(names[0]!.style.height);
    const i = names.findIndex((n) => n.dataset.key === "39");
    pointer(q(".area"), "pointermove", { clientX: 300, clientY: i * rowH + rowH / 2 });
    flushSync();
    const out = new Batch();
    out.clip(0, 0, 668, 440);
    (slots.get(q(".area .fill")) as Extract<GlSlot, { kind: "paint" }>).paint(out, { x0: 0, y0: 0, x1: 668, y1: 440 }, CTX);
    const rows = out.rects().filter((r) => r[0] === 0 && r[2] === 668 && colorOf(r) === is(ROW_HIGHLIGHT));
    expect(rows.map((r) => Math.round(r[1]! / rowH))).toEqual([i]);
    const col = clipped(132, 440);
    (slots.get(q(".key-shapes")) as Extract<GlSlot, { kind: "paint" }>).paint(col, { x0: 0, y0: 0, x1: 132, y1: 440 }, CTX);
    const lit = col.rects().filter((r) => r[2] === 132 && colorOf(r) === is(DRUM_NAME_HIGHLIGHT));
    expect(lit.map((r) => Math.round(r[1]! / rowH))).toEqual([i]);
    expect(qa(".drum-name.hl").map((n) => n.dataset.key)).toEqual(["39"]);
    expect(qa(".drum-name").map((n) => n.textContent)).toEqual(texts);
    pointer(q(".roll"), "pointerleave");
    flushSync();
    const after = clipped(132, 440);
    (slots.get(q(".key-shapes")) as Extract<GlSlot, { kind: "paint" }>).paint(after, { x0: 0, y0: 0, x1: 132, y1: 440 }, CTX);
    expect(after.rects().some((r) => colorOf(r) === is(DRUM_NAME_HIGHLIGHT))).toBe(false);
    expect(qa(".drum-name.hl")).toHaveLength(0);
  });

  it("below the last drum row: none", async () => {
    await drums();
    const names = qa(".drum-name");
    const rowH = parseFloat(names[0]!.style.height);
    pointer(q(".area"), "pointermove", { clientX: 300, clientY: names.length * rowH + 2 });
    flushSync();
    expect(qa(".drum-name.hl")).toHaveLength(0);
  });
});
