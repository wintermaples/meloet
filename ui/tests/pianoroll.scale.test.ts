// Round 12 P1: the 9 kinds of scale. The screen's table against Rust's, the row colors (only with scale lock, user:2026-11-24),
// the chords of the key (5- and 6-note scales list their parent's), names, and the toolbar and right panel.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SCALE_MODES, type AppState, type MusicalScale } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { scaleModeName, scaleName } from "../src/components/format";
import ChordRow from "../src/components/pianoroll/ChordRow.svelte";
import { keyChords, triadName } from "../src/components/pianoroll/chords";
import * as G from "../src/components/pianoroll/geom";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import { paintRoll, type RollPicture } from "../src/components/pianoroll/paint";
import table from "../src/components/pianoroll/scaleSteps.table.json";
import Toolbar from "../src/components/pianoroll/Toolbar.svelte";
import { colors } from "../src/design/tokens";
import { Batch } from "../src/gl/batch";
import { hexToRgb } from "../src/gl/color";
import { isBlack } from "../src/gl/keyboard";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";
import { GEOM, take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

describe("the 9 kinds", () => {
  it("SCALE_STEPS is the table Rust writes (crates/meloet-core, write_the_scale_steps_for_the_screen), in the order of the list", () => {
    expect(Object.keys(table)).toEqual([...SCALE_MODES]);
    expect(G.SCALE_STEPS).toEqual(table);
  });

  it("the steps of the spec", () => {
    expect(G.SCALE_STEPS).toEqual({
      major: [0, 2, 4, 5, 7, 9, 11],
      minor: [0, 2, 3, 5, 7, 8, 10],
      harmonicMinor: [0, 2, 3, 5, 7, 8, 11],
      melodicMinor: [0, 2, 3, 5, 7, 9, 11],
      majorPentatonic: [0, 2, 4, 7, 9],
      minorPentatonic: [0, 3, 5, 7, 10],
      blues: [0, 3, 5, 6, 7, 10],
      dorian: [0, 2, 3, 5, 7, 9, 10],
      mixolydian: [0, 2, 4, 5, 7, 9, 10],
    });
  });

  it("names as the spec writes them", () => {
    expect(SCALE_MODES.map(scaleModeName)).toEqual([
      "メジャー",
      "マイナー",
      "ハーモニックマイナー",
      "メロディックマイナー",
      "メジャーペンタトニック",
      "マイナーペンタトニック",
      "ブルース",
      "ドリアン",
      "ミクソリディアン",
    ]);
    expect(scaleName(2, "blues")).toBe("D ブルース");
    expect(scaleName(9, "minorPentatonic")).toBe("A マイナーペンタトニック");
  });
});

describe("chords of the key", () => {
  const list = (root: number, mode: MusicalScale["mode"]) => keyChords({ root, mode }).map((c) => triadName(c.root, c.triad));

  it("7-note scales use their own notes", () => {
    expect(list(2, "dorian")).toEqual(["Dm", "Em", "F", "G", "Am", "Bdim", "C"]);
    expect(list(7, "mixolydian")).toEqual(["G", "Am", "Bdim", "C", "Dm", "Em", "F"]);
    expect(list(9, "melodicMinor")).toEqual(["Am", "Bm", "Caug", "D", "E", "F#dim", "G#dim"]);
  });

  it("major pentatonic lists major; minor pentatonic and blues list minor", () => {
    expect(list(0, "majorPentatonic")).toEqual(list(0, "major"));
    expect(list(9, "minorPentatonic")).toEqual(list(9, "minor"));
    expect(list(2, "blues")).toEqual(["Dm", "Edim", "F", "Gm", "Am", "A#", "C"]);
  });
});

// ---- the note area ----

const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8 };
const BOX = { x0: 0, y0: 0, x1: 696, y1: 492 };
const f32 = (hex: string) => hexToRgb(hex).map(Math.fround).join();

function draw(scale: MusicalScale): Batch {
  const p: RollPicture = {
    g: GEOM,
    sizes: SIZES,
    scale,
    meter: { num: 4, den: 4 },
    step: TPQ / 4,
    shown: G.shownOf(take([[0, TPQ, 64, 100]]), 0),
    origin: null,
    selected: null,
    live: null,
    drag: null,
    eraseHover: -1,
  };
  const out = new Batch();
  out.clip(BOX.x0, BOX.y0, BOX.x1, BOX.y1);
  paintRoll(out, BOX, p, null, 1);
  return out;
}

/** The colors of the row backgrounds, top row first (after the ground). */
const rowColors = (out: Batch) =>
  out
    .rects()
    .slice(1, 1 + G.rowsShown(GEOM))
    .map((r) => r.slice(4, 7).join());

describe("the scale's colors in the note area (only with scale lock; user:2026-11-24)", () => {
  const D_BLUES = { root: 2, mode: "blues", lock: false, highlight: true } as const;
  const keys = Array.from({ length: G.rowsShown(GEOM) }, (_, r) => GEOM.topKey - r);
  const plain = keys.map((k) => f32(colors[isBlack(k) ? "color.row.black" : "color.row.white"]));

  it("lock on: rows outside in row.out, nothing more on them", () => {
    const out = draw({ ...D_BLUES, lock: true });
    expect(rowColors(out)).toEqual(
      keys.map((k) => f32(colors[G.inScale(k, D_BLUES) ? (isBlack(k) ? "color.row.black" : "color.row.white") : "color.row.out"])),
    );
    expect(rowColors(draw({ ...D_BLUES, lock: true, highlight: false }))).toEqual(rowColors(out));
  });

  it("lock off: only white and black rows, whatever highlight says", () => {
    expect(rowColors(draw(D_BLUES))).toEqual(plain);
    expect(rowColors(draw({ ...D_BLUES, highlight: false }))).toEqual(plain);
  });

  it("the scene with the lock on", () => {
    expect(fakeScene("scaleLocked")!.state.pianoRoll.scale).toEqual({ ...D_BLUES, lock: true });
    expect(fakeScene("scaleHighlight")).toBeNull();
  });
});

// ---- toolbar and right panel ----

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

function show(component: Component, scale: MusicalScale): AppState {
  const s = fakeScene("pianoRoll")!.state;
  s.pianoRoll.scale = scale;
  app.setState(s);
  shown.setShown({ clip: null, track: null, take: null });
  mounted = mount(component, { target });
  flushSync();
  return s;
}

describe("the toolbar's scale lock and ▾", () => {
  it("no 「色」 button (user:2026-11-24); scale lock sends highlight back as it came", () => {
    show(Toolbar, { root: 2, mode: "blues", lock: false, highlight: true });
    expect(q("[data-test=pr-scale-highlight]")).toBeNull();
    expect(text(q("[data-test=pr-scale-lock]"))).toBe("スケール固定 D ブルース");
    expect(q("[data-test=pr-scale-menu]")!.nextElementSibling?.classList.contains("ruler")).toBe(true);
    q("[data-test=pr-scale-lock]")!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setMusicalScale", root: 2, mode: "blues", lock: true, highlight: true }]);
  });

  it("the ▾ lists the 9 kinds; choosing one turns the lock on and keeps highlight", () => {
    show(Toolbar, { root: 0, mode: "major", lock: false, highlight: false });
    q("[data-test=pr-scale-menu]")!.click();
    flushSync();
    const menu = document.querySelector<HTMLElement>("[data-test=scale-menu]")!;
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    expect(rows.slice(0, 9).map(text)).toEqual(SCALE_MODES.map(scaleModeName));
    rows[6]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setMusicalScale", root: 0, mode: "blues", lock: true, highlight: false }]);
  });
});

describe("the right panel's 「スケール」", () => {
  it("no 「色をつける」 row (user:2026-11-24); 「固定」 sends highlight back as it came", () => {
    show(NoteInspector, { root: 9, mode: "minorPentatonic", lock: true, highlight: true });
    expect(q("[data-test=inspector-highlight]")).toBeNull();
    expect(text(q("[data-test=inspector-scale]"))).toBe("A マイナーペンタトニック");
    q("[data-test=inspector-lock]")!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setMusicalScale", root: 9, mode: "minorPentatonic", lock: false, highlight: true }]);
  });

  it("choosing a kind here keeps the lock and highlight", () => {
    show(NoteInspector, { root: 0, mode: "major", lock: false, highlight: false });
    q("[data-test=inspector-scale]")!.click();
    flushSync();
    const rows = [...document.querySelector<HTMLElement>("[data-test=scale-menu]")!.querySelectorAll<HTMLButtonElement>("button")];
    rows[8]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setMusicalScale", root: 0, mode: "mixolydian", lock: false, highlight: false }]);
  });
});

describe("chord row", () => {
  it("D blues locked lists the chords of D minor", () => {
    show(ChordRow, { root: 2, mode: "blues", lock: true, highlight: true });
    expect(text(q("[data-test=pr-chord-row] .name"))).toBe("調の和音 D ブルース");
    expect(qa("[data-test=pr-key-chord]").map(text)).toEqual(["Dm", "Edim", "F", "Gm", "Am", "A#", "C"]);
  });
});
