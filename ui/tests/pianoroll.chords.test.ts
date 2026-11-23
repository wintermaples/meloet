// Chord help in the piano roll (spec 3 "Rules added in round 10", A-27; mocks P10-ChordStamp and P10-ChordTable A):
// the shapes and their keys, the chords of a key, the root from the pressed row, what is sent, drums, and the height.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, Command, TakeNotes, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { Auditioner } from "../src/components/pianoroll/audition";
import ChordRow from "../src/components/pianoroll/ChordRow.svelte";
import { chordHover, chordPick } from "../src/components/pianoroll/chordPick.svelte";
import {
  CHORD_SHAPES,
  chordsOfSteps,
  keyChords,
  nearestRoot,
  pickName,
  SHAPE_STEPS,
  shapeName,
  shapeOnC,
  SINGLE,
  stampKeys,
  stampStatus,
  triadName,
  type ChordPick,
} from "../src/components/pianoroll/chords";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import { paintLive } from "../src/components/pianoroll/paint";
import Toolbar from "../src/components/pianoroll/Toolbar.svelte";
import { prView, type Tool } from "../src/components/pianoroll/view.svelte";
import { findTerm, TERM_SEPARATOR, t } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";
import { GEOM, take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const proposal = (name: string) => {
  const html = fs.readFileSync(new URL(`../../docs/design/proposals/${name}.dc.html`, here), "utf8");
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("script, style, helmet, title").forEach((n) => n.remove());
  return (doc.body.textContent ?? "").replace(/\s+/g, " ");
};

const C_MAJOR = { root: 0, mode: "major" } as const;
const name = (k: number) => G.noteName(k);
const names = (keys: readonly number[]) => keys.map(name).join(" ");

describe("chord shapes (mock P10-ChordStamp)", () => {
  it("list names and the notes for root C are as the mock writes them", () => {
    const stamp = proposal("P10-ChordStamp");
    const rows = CHORD_SHAPES.map((s) => [shapeName(s), shapeOnC(s)]);
    expect(rows).toEqual([
      ["1 音（ふだん）", "C"],
      ["メジャー", "C E G"],
      ["マイナー", "C E♭ G"],
      ["7", "C E G B♭"],
      ["M7", "C E G B"],
      ["m7", "C E♭ G B♭"],
      ["sus4", "C F G"],
      ["dim", "C E♭ G♭"],
      ["aug", "C E G♯"],
    ]);
    for (const [label, notes] of rows) expect(stamp).toContain(`${label}${notes}`);
    for (const key of ["pr.chordShapes", "pr.chordRootC"] as const) expect(stamp).toContain(t(key));
  });

  it("the shape's notes are at the same pitch as the notes for root C (♭ and ♯ are the same key)", () => {
    const flat: Record<string, number> = { C: 0, "E♭": 3, E: 4, F: 5, "G♭": 6, G: 7, "G♯": 8, "B♭": 10, B: 11 };
    for (const shape of CHORD_SHAPES) {
      const onC = shapeOnC(shape).split(" ").map((n) => flat[n]);
      expect(SHAPE_STEPS[shape], shape).toEqual(onC);
    }
  });

  it("the pressed row is the root. Notes above 127 are not written", () => {
    const major: ChordPick = { kind: "shape", shape: "major" };
    expect(names(stampKeys(major, 60))).toBe("C4 E4 G4");
    expect(names(stampKeys({ kind: "shape", shape: "minorSeventh" }, 62))).toBe("D4 F4 A4 C5");
    expect(stampKeys(major, 120)).toEqual([120, 124, 127]);
    expect(stampKeys(SINGLE, 61)).toEqual([61]);
  });

  it("draw button name and status line (mock text)", () => {
    const major: ChordPick = { kind: "shape", shape: "major" };
    expect(pickName(SINGLE)).toBeNull();
    expect(pickName(major)).toBe("メジャー");
    expect(stampStatus(major, 60)).toBe("書く ・ メジャー ・ 根音 C4（C4 E4 G4）");
    expect(stampStatus(SINGLE, 60)).toBeNull();
    expect(proposal("P10-ChordStamp")).toContain(stampStatus(major, 60));
  });
});

describe("chords of the key (mock P10-ChordTable A)", () => {
  const list = (scale: Parameters<typeof keyChords>[0]) => keyChords(scale).map((c) => triadName(c.root, c.triad));

  it("major key: C, Dm, Em, F, G, Am, Bdim. Other tonics only shift", () => {
    expect(list(C_MAJOR)).toEqual(["C", "Dm", "Em", "F", "G", "Am", "Bdim"]);
    expect(list({ root: 7, mode: "major" })).toEqual(["G", "Am", "Bm", "C", "D", "Em", "F#dim"]);
  });

  it("a minor key starts from the tonic: Am, Bdim, C, Dm, Em, F, G", () => {
    expect(list({ root: 9, mode: "minor" })).toEqual(["Am", "Bdim", "C", "Dm", "Em", "F", "G"]);
  });

  it("scales that are not 7 notes are also stacked every other note, and shapes with no name are left out", () => {
    // Harmonic minor: the third chord is aug.
    expect(chordsOfSteps(9, [0, 2, 3, 5, 7, 8, 11]).map((c) => triadName(c.root, c.triad))).toEqual(["Am", "Bdim", "Caug", "Dm", "E", "F", "G#dim"]);
    // Major pentatonic: no stack of every other note is a named triad.
    expect(chordsOfSteps(0, [0, 2, 4, 7, 9])).toEqual([]);
    // Whole tone: every stack is aug.
    expect(chordsOfSteps(0, [0, 2, 4, 6, 8, 10]).map((c) => c.triad)).toEqual(["aug", "aug", "aug", "aug", "aug", "aug"]);
    expect(chordsOfSteps(0, [0, 7])).toEqual([]);
  });

  it("the root nearest the pressed row (the lower one when equally near). Within 0 to 127", () => {
    expect(nearestRoot(60, 2)).toBe(62);
    expect(nearestRoot(71, 2)).toBe(74);
    expect(nearestRoot(66, 0)).toBe(60);
    expect(nearestRoot(67, 0)).toBe(72);
    expect(nearestRoot(2, 11)).toBe(11);
    expect(nearestRoot(127, 0)).toBe(120);
    const dm: ChordPick = { kind: "key", root: 2, triad: "minor" };
    expect(names(stampKeys(dm, 60))).toBe("D4 F4 A4");
    expect(stampStatus(dm, 60)).toBe("書く ・ Dm（D4 F4 A4）");
    expect(proposal("P10-ChordTable")).toContain(stampStatus(dm, 60));
    expect(pickName(dm)).toBe("Dm");
  });

  it("term tips are as the mock writes them (user:2026-11-23)", () => {
    for (const [word, board] of [
      ["和音のスタンプ", "P10-ChordStamp"],
      ["根音", "P10-ChordStamp"],
      ["調の和音", "P10-ChordTable"],
    ] as const) {
      const found = findTerm(word)!;
      expect(found.source).toBe("user:2026-11-23");
      expect(proposal(board)).toContain(`${found.term}${TERM_SEPARATOR}${found.description}`);
    }
    expect(proposal("P10-ChordTable")).toContain(t("pr.keyChordsOff"));
  });
});

describe("drawing chords with the draw tool (drag.ts)", () => {
  const NONE: Mods = { shift: false, alt: false, mod: false };
  const n = G.shownOf(take([[0, TPQ, 60, 80]]), 0);
  const y = (key: number) => (GEOM.topKey - key) * GEOM.rowH + 5;
  const x = (tick: number) => tick * GEOM.ppt;
  let sent: Command[];
  let pick: ChordPick;
  let tool: Tool;
  let lock: boolean;
  let drag: RollDrag;
  beforeEach(() => {
    sent = [];
    pick = { kind: "shape", shape: "major" };
    tool = "draw";
    lock = false;
    drag = new RollDrag({
      shown: () => n,
      geom: () => GEOM,
      step: () => TPQ / 4,
      scale: () => ({ ...C_MAJOR, lock }),
      meter: () => ({ num: 4, den: 4 }),
      tool: () => tool,
      selected: () => [],
      edgePx: () => 6,
      stillPx: () => 4,
      velocityFullPx: () => 64,
      velocityHeight: () => 71,
      send: (c) => void sent.push(c),
      drawVelocity: () => 90,
      mayAudition: () => true,
      ticksToMs: (ticks) => ticks,
      audition: new Auditioner((c) => void sent.push(c)),
      chord: () => pick,
    });
  });

  it("over the keys, a shadow where it will be written. Not over a note, with the select tool, or for one note", () => {
    const h = drag.hover(x(2 * TPQ + 10), y(62), NONE);
    expect(h).toMatchObject({ cursor: "pencil", shadowChanged: true });
    expect(drag.shadow).toEqual({ start: 2 * TPQ, length: TPQ / 4, keys: [62, 66, 69], key: 62 });
    expect(drag.hover(x(2 * TPQ + 20), y(62), NONE).shadowChanged).toBe(false);
    expect(drag.hover(x(10), y(60), NONE).shadowChanged).toBe(true);
    expect(drag.shadow).toBeNull();
    tool = "select";
    drag.hover(x(2 * TPQ), y(62), NONE);
    expect(drag.shadow).toBeNull();
    tool = "draw";
    pick = SINGLE;
    drag.hover(x(2 * TPQ), y(62), NONE);
    expect(drag.shadow).toBeNull();
  });

  it("press and release sends one addNotes. Held and moved right, they all grow together. Velocity is the same as the draw tool", () => {
    drag.down(x(2 * TPQ + 10), y(60), NONE);
    expect(sent).toEqual([{ type: "audition", keys: [60, 64, 67], velocity: 90 }]);
    sent.length = 0;
    drag.move(x(3 * TPQ + 10), y(60), NONE);
    expect(sent).toEqual([]);
    const c = drag.up();
    expect(c).toEqual({ type: "addNotes", take: 7, start: 2 * TPQ, length: TPQ, keys: [60, 64, 67], velocity: 90 });
    expect(sent).toEqual([{ type: "audition", keys: [], velocity: 0 }, c]);
  });

  it("chords of the key are placed with the root nearest the pressed row", () => {
    pick = { kind: "key", root: 2, triad: "minor" };
    drag.down(x(2 * TPQ), y(60), NONE);
    expect(drag.up()).toMatchObject({ type: "addNotes", keys: [62, 65, 69] });
  });

  it("even with scale lock, notes outside the scale are written as they are (the owner's decision)", () => {
    lock = true;
    pick = { kind: "shape", shape: "major" };
    expect(drag.hover(x(2 * TPQ), y(61), NONE).cursor).toBe("pencil");
    drag.down(x(2 * TPQ), y(62), NONE);
    expect(drag.up()).toMatchObject({ type: "addNotes", keys: [62, 66, 69] });
  });

  it("cancelling with Esc sends nothing", () => {
    drag.down(x(2 * TPQ), y(60), NONE);
    sent.length = 0;
    expect(drag.cancel()).toBe(true);
    expect(sent).toEqual([{ type: "audition", keys: [], velocity: 0 }]);
    expect(drag.up()).toBeNull();
  });

  it("for one note, addNote as before", () => {
    pick = SINGLE;
    drag.down(x(2 * TPQ), y(60), NONE);
    expect(drag.up()).toEqual({ type: "addNote", take: 7, start: 2 * TPQ, length: TPQ / 4, key: 60, velocity: 90 });
  });
});

describe("shadow picture (paintLive in paint.ts)", () => {
  it("where it will be written, only a 1px accent border the same size as a note (inside not filled)", () => {
    const out = new Batch();
    const sizes = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8 };
    const box = { x0: 0, y0: 0, x1: 41, y1: 12 };
    paintLive(out, box, { g: GEOM, sizes, left: 0, top: 0, ty: 0, areaH: 492, notes: [{ start: 0, end: TPQ, key: 72, face: "color.accent", outline: true }] }, 1);
    const rects = out.rects();
    const accent = Array.from(Float32Array.from(rgb("color.accent"))).join(",");
    expect(rects.map((r) => [r[0], r[1], r[2], r[3]])).toEqual([
      [1, 0, 39, 1],
      [1, 10, 39, 11],
      [1, 0, 2, 11],
      [38, 0, 39, 11],
    ]);
    expect(rects.every((r) => [r[4], r[5], r[6]].join(",") === accent)).toBe(true);
  });
});

describe("the ▾ in the toolbar and the row of chords of the key (mount)", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  beforeEach(() => {
    app.reset();
    prView.tool = "select";
    chordPick.pick = SINGLE;
    chordHover.text = null;
    target = document.createElement("div");
    document.body.append(target);
  });
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    target.remove();
    document.querySelectorAll("[data-test=pr-chord-shapes]").forEach((n) => n.parentElement?.remove());
  });
  const q = (sel: string) => target.querySelector<HTMLElement>(sel);
  const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
  const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
  function show(component: Component, state: AppState, where: { clip: ClipView | null; track: TrackView | null; take: TakeNotes | null }): void {
    app.setState(state);
    shown.setShown(where);
    mounted = mount(component, { target });
    flushSync();
  }
  function keysScene(lock: boolean): AppState {
    const s = fakeScene("pianoRoll")!.state;
    s.pianoRoll.scale = { root: 0, mode: "major", lock };
    return s;
  }
  const noClip = { clip: null, track: null, take: null };
  async function drumScene(): Promise<{ s: AppState; where: { clip: ClipView; track: TrackView; take: TakeNotes } }> {
    const s = fakeScene("drumRoll")!.state;
    const track = s.song.tracks.find((x) => x.clips.some((c) => c.id === s.arrange.pianoRollClip))!;
    const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
    const tk = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
    return { s, where: { clip, track, take: tk } };
  }

  it("▾ gives the 「和音の形」 list. Choosing one switches to the draw tool, 「書く メジャー 2」, and ▾ is accent. The shape stays when the tool changes", () => {
    show(Toolbar, keysScene(false), noClip);
    q("[data-test=pr-chord-menu]")!.click();
    flushSync();
    const menu = document.querySelector("[data-test=pr-chord-shapes]")!;
    expect(text(menu.querySelector(".head"))).toBe("和音の形根音が C のとき");
    const rows = [...menu.querySelectorAll("button")];
    expect(rows.map((b) => text(b))[1]).toBe("メジャー C E G");
    expect(rows[0]!.getAttribute("aria-checked")).toBe("true");
    rows[1]!.click();
    flushSync();
    expect(document.querySelector("[data-test=pr-chord-shapes]")).toBeNull();
    expect(prView.tool).toBe("draw");
    expect(text(q("[data-test=pr-tool-draw]"))).toBe("書くメジャー2");
    expect(q("[data-test=pr-chord-menu]")!.classList.contains("on")).toBe(true);
    q("[data-test=pr-tool-select]")!.click();
    flushSync();
    expect(chordPick.pick).toEqual({ kind: "shape", shape: "major" });
    expect(app.sent).toEqual([]);
  });

  it("a hint text when scale lock is off. Chord buttons of the key when on", () => {
    show(ChordRow, keysScene(false), noClip);
    expect(text(q("[data-test=pr-chord-row]"))).toBe("調の和音 スケール固定をオンにすると、その調の和音がここにならびます。");
    expect(qa("[data-test=pr-key-chord]")).toEqual([]);
    unmount(mounted!);
    show(ChordRow, keysScene(true), noClip);
    expect(text(q("[data-test=pr-chord-row] .name"))).toBe("調の和音 C メジャー");
    expect(qa("[data-test=pr-key-chord]").map(text)).toEqual(["C", "Dm", "Em", "F", "G", "Am", "Bdim"]);
  });

  it("pressing a chord plays only while held and sets the draw tool shape (either this or the stamp shape)", () => {
    show(ChordRow, keysScene(true), noClip);
    const dm = qa("[data-test=pr-key-chord]")[1]!;
    dm.dispatchEvent(new PointerEvent("pointerdown", { button: 0, bubbles: true }));
    flushSync();
    expect(app.sent).toEqual([{ type: "audition", keys: [62, 65, 69], velocity: 100 }]);
    expect(chordPick.pick).toEqual({ kind: "key", root: 2, triad: "minor" });
    expect(prView.tool).toBe("draw");
    expect(dm.classList.contains("on")).toBe(true);
    dm.dispatchEvent(new PointerEvent("pointerup", { button: 0, bubbles: true }));
    expect(app.sent.at(-1)).toEqual({ type: "audition", keys: [], velocity: 0 });
    chordPick.pick = { kind: "shape", shape: "minor" };
    flushSync();
    expect(dm.classList.contains("on")).toBe(false);
  });

  it("on drums, the ▾ and the chord row are disabled. The draw tool stays at one note", async () => {
    const { s, where } = await drumScene();
    s.pianoRoll.scale = { root: 0, mode: "major", lock: true };
    chordPick.pick = { kind: "shape", shape: "major" };
    show(Toolbar, s, where);
    const stub = q("[data-test=pr-chord-menu]")!;
    expect(stub.getAttribute("aria-disabled")).toBe("true");
    stub.click();
    flushSync();
    expect(document.querySelector("[data-test=pr-chord-shapes]")).toBeNull();
    expect(text(q("[data-test=pr-tool-draw]"))).toBe("書く2");
    unmount(mounted!);
    show(ChordRow, s, where);
    const row = q("[data-test=pr-chord-row]")!;
    expect(row.getAttribute("aria-disabled")).toBe("true");
    expect(text(row)).toBe("調の和音");
    expect(qa("[data-test=pr-key-chord]")).toEqual([]);
    expect(app.sent).toEqual([]);
  });
});
