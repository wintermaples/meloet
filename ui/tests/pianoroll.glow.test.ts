// 「鳴っている音を光らせる」 (round 12 E1, sounding.ts and glow.ts): which notes sound (starts, ends, overlaps, the clip's edges, a loop
// wrap, a tempo change read through the screen's tempo map), when they light (the setting, the transport, mute and solo, drags), the
// width limit and the change in two frames, the picture in the track's color at the place of the note, and the fake's copy of the setting.
import { describe, expect, it } from "vitest";
import type { AppState, Command, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { trackColor } from "../src/components/arrangeGeom";
import { noteName } from "../src/components/meterFormat";
import * as G from "../src/components/pianoroll/geom";
import {
  GLOW_AREA,
  GLOW_STEP,
  glowBox,
  glowFit,
  glowMaxWidth,
  glowNext,
  glowPlace,
  glowShows,
  trackPlays,
  type GlowSpan,
  type GlowWhen,
} from "../src/components/pianoroll/glow";
import { paintLive, paintRoll, type RollPicture } from "../src/components/pianoroll/paint";
import { keysOf, soundingKeys, soundingNotes } from "../src/components/pianoroll/sounding";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { TempoMap } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const BAR = 4 * TPQ;
const CLIP = { start: 0, end: 4 * BAR, takeOffset: 0, takeId: 7 };
const span = (note: number, x0: number, x1: number): GlowSpan => ({ note, x0, x1 });
const notesOf = (spans: readonly GlowSpan[]) => spans.map((s) => s.note);

describe("the notes sounding at the play position", () => {
  const t = take([
    [0, TPQ, 60],
    [0, 2 * TPQ, 64],
    [TPQ, TPQ, 67],
    [TPQ, BAR, 60],
    [2 * TPQ, TPQ / 2, 72],
  ]);

  it("a note sounds from its start (included) to its end (not included)", () => {
    expect(soundingNotes(t, CLIP, 0)).toEqual([0, 1]);
    expect(soundingNotes(t, CLIP, TPQ - 1)).toEqual([0, 1]);
    expect(soundingNotes(t, CLIP, TPQ)).toEqual([1, 2, 3]);
    expect(soundingNotes(t, CLIP, 2 * TPQ)).toEqual([3, 4]);
    expect(soundingNotes(t, CLIP, 2 * TPQ + TPQ / 2)).toEqual([3]);
  });

  it("overlapping notes of the same key are each lit; their key is said once", () => {
    const same = take([
      [0, BAR, 60],
      [TPQ, TPQ, 60],
    ]);
    expect(soundingNotes(same, CLIP, TPQ + 1)).toEqual([0, 1]);
    expect(keysOf(same, [0, 1])).toEqual([60]);
    expect(soundingKeys(t, CLIP, TPQ)).toEqual([60, 64, 67]);
    expect(keysOf(t, soundingNotes(t, CLIP, TPQ))).toEqual(soundingKeys(t, CLIP, TPQ));
  });

  it("nothing outside the clip, nothing of notes trimmed off its start, nothing while the clip's take is still coming", () => {
    expect(soundingNotes(t, { ...CLIP, end: TPQ }, TPQ)).toEqual([]);
    expect(soundingNotes(t, { ...CLIP, start: BAR, end: 2 * BAR }, BAR)).toEqual([0, 1]);
    expect(soundingNotes(t, { ...CLIP, start: TPQ, takeOffset: -TPQ }, TPQ)).toEqual([2, 3]);
    expect(soundingNotes(t, { ...CLIP, takeId: 8 }, 0)).toEqual([]);
    expect(soundingNotes(null, CLIP, 0)).toEqual([]);
    expect(soundingNotes(t, null, 0)).toEqual([]);
  });

  it("asked every frame, it writes into the same array (no new array per frame)", () => {
    const out: number[] = [];
    expect(soundingNotes(t, CLIP, 0, out)).toBe(out);
    expect(out).toEqual([0, 1]);
    soundingNotes(t, CLIP, 3 * BAR, out);
    expect(out).toEqual([]);
  });

  it("a loop wrap: from the loop's end straight back to its start, the notes of the end go out and those of the start light", () => {
    const loop = take([
      [0, TPQ, 60],
      [BAR - TPQ / 2, TPQ / 2, 62],
    ]);
    const end = soundingNotes(loop, CLIP, BAR - 1);
    const start = soundingNotes(loop, CLIP, 0);
    expect([end, start]).toEqual([[1], [0]]);
    // At the default zoom the two are far apart: the change is shown in two frames, never as one box around both.
    const shown = [span(1, 600, 640)];
    const target = [span(0, 0, 40)];
    const first = glowNext({ x0: 600, x1: 704 }, shown, target, 500, 800, 1, false);
    expect(first).toEqual({ lit: [], box: null, pending: true });
    expect(glowNext(first.box, first.lit, target, 500, 800, 1, false)).toEqual({ lit: target, box: { x0: 0, x1: 96 }, pending: false });
  });

  it("a tempo change read through the screen's tempo map: the same note sounds longer in seconds after the song slows down", () => {
    const map = new TempoMap({
      tempoBpm: 120,
      meterNum: 4,
      meterDen: 4,
      headRamp: false,
      tempoPoints: [{ tick: BAR, tempoBpm: 60, ramp: false }],
      meterChanges: [],
    });
    const notes = take([
      [0, TPQ, 60],
      [BAR, TPQ, 64],
    ]);
    const at = (seconds: number) => soundingNotes(notes, CLIP, map.tickAt(seconds));
    // Bar 1 at 120 (half a second a beat), then 60 (a second a beat).
    expect(at(0.25)).toEqual([0]);
    expect(at(0.55)).toEqual([]);
    expect(at(2.05)).toEqual([1]);
    expect(at(2.9)).toEqual([1]);
    expect(at(3.05)).toEqual([]);
    // Had the tempo stayed at 120, 2.9 s would be past that note.
    expect(soundingNotes(notes, CLIP, 2.9 * 2 * TPQ)).toEqual([]);
  });

  it("the scene pianoRollGlow: at 1.3.240 C3 G3 and C5 sound (E4 and G4 ended exactly at 1.2.000 and 1.3.000)", async () => {
    const s = fakeScene("pianoRollGlow")!.state;
    const clip = s.song.tracks[0]!.clips[0]!;
    const shownTake = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
    expect(soundingKeys(shownTake, clip, 2 * TPQ + TPQ / 2).map(noteName)).toEqual(["C3", "G3", "C5"]);
  });
});

describe("when notes light", () => {
  const tracks = [
    { id: 1, muted: false, soloed: false },
    { id: 2, muted: false, soloed: false },
  ] as TrackView[];
  const when = (over: Partial<GlowWhen> = {}): GlowWhen => ({
    setting: true,
    state: "playing",
    recordingShown: false,
    dragging: false,
    tracks,
    track: tracks[0]!,
    ...over,
  });

  it("while playing and recording, with the setting on", () => {
    expect(glowShows(when())).toBe(true);
    expect(glowShows(when({ state: "recording" }))).toBe(true);
  });

  it("not with the setting off, not stopped, not in the count-in", () => {
    expect(glowShows(when({ setting: false }))).toBe(false);
    expect(glowShows(when({ state: "stopped" }))).toBe(false);
    expect(glowShows(when({ state: "countIn" }))).toBe(false);
  });

  it("not while the clip being recorded is shown, nor while notes are dragged, nor without a clip", () => {
    expect(glowShows(when({ recordingShown: true }))).toBe(false);
    expect(glowShows(when({ dragging: true }))).toBe(false);
    expect(glowShows(when({ track: null }))).toBe(false);
  });

  it("not on a muted track, nor when another track is soloed; a soloed track lights", () => {
    const muted = { ...tracks[0]!, muted: true };
    expect(glowShows(when({ track: muted }))).toBe(false);
    const other = [tracks[0]!, { ...tracks[1]!, soloed: true }];
    expect(trackPlays(other, tracks[0]!)).toBe(false);
    expect(glowShows(when({ tracks: other }))).toBe(false);
    const both = [{ ...tracks[0]!, soloed: true }, other[1]!];
    expect(glowShows(when({ tracks: both, track: both[0]! }))).toBe(true);
    expect(trackPlays(both, { ...both[0]!, muted: true })).toBe(false);
  });
});

describe("the box of the lit notes", () => {
  it("at most 15 % of the window's area over the note area's height (about 530 px at 1920x1080 with the note area of 588)", () => {
    expect(GLOW_AREA).toBe(0.15);
    expect(glowMaxWidth(1920, 1080, 588)).toBeCloseTo(528.98, 1);
    expect(glowMaxWidth(1280, 800, 444)).toBeCloseTo(345.95, 1);
    expect(glowMaxWidth(1280, 800, 0)).toBe(0);
  });

  it("the widest notes are left out until the rest fit; the order is kept; when all fit, the same list comes back", () => {
    const all = [span(0, 100, 700), span(1, 300, 400), span(2, 350, 600), span(3, 380, 420)];
    expect(glowBox(all)).toEqual({ x0: 100, x1: 700 });
    expect(notesOf(glowFit(all, 500))).toEqual([1, 2, 3]);
    expect(notesOf(glowFit(all, 200))).toEqual([1, 3]);
    expect(notesOf(glowFit(all, 10))).toEqual([]);
    const fits = all.slice(1);
    expect(glowFit(fits, 500)).toBe(fits);
    expect(glowBox([])).toBeNull();
  });

  it("two notes as wide: the later one goes first", () => {
    expect(notesOf(glowFit([span(0, 0, 300), span(1, 250, 550)], 400))).toEqual([0]);
  });

  it("a new box starts at the lit notes and is one or two steps wider to their right, in whole steps, at most the limit", () => {
    expect(GLOW_STEP).toBe(32);
    expect(glowPlace({ x0: 100.4, x1: 140.2 }, 500, 1000, 1)).toEqual({ x0: 100, x1: 196 });
    expect(glowPlace({ x0: 100, x1: 164 }, 500, 1000, 1)).toEqual({ x0: 100, x1: 196 });
    expect(glowPlace({ x0: 100, x1: 165 }, 500, 1000, 1)).toEqual({ x0: 100, x1: 228 });
    // At 150 %: edges on device pixels (two thirds of a CSS px).
    const b = glowPlace({ x0: 100.5, x1: 140 }, 500, 1000, 1.5);
    expect([b.x0 * 1.5, b.x1 * 1.5]).toEqual([150, 294]);
    // Never wider than the limit, and never narrower than the notes.
    expect(glowPlace({ x0: 0, x1: 480 }, 500, 1000, 1)).toEqual({ x0: 0, x1: 500 });
    expect(glowPlace({ x0: 0, x1: 500.5 }, 500, 1000, 1)).toEqual({ x0: 0, x1: 501 });
    // Near the right edge it moves left, keeping its width.
    expect(glowPlace({ x0: 950, x1: 990 }, 500, 1000, 1)).toEqual({ x0: 904, x1: 1000 });
  });

  it("the box stays while the lit notes change inside it: the canvas is drawn again, not moved or resized", () => {
    const kept = { x0: 100, x1: 196 };
    const shown = [span(0, 100, 140)];
    const next = [span(0, 100, 140), span(1, 130, 160)];
    expect(glowNext(kept, shown, next, 500, 1000, 1, false)).toEqual({ lit: next, box: kept, pending: false });
    // Nothing lit for a moment (a rest): the box stays, empty.
    expect(glowNext(kept, next, [], 500, 1000, 1, false)).toEqual({ lit: [], box: kept, pending: false });
    // Past its right edge: a new box from the lit notes' left edge.
    const on = [span(1, 130, 160), span(2, 150, 210)];
    expect(glowNext(kept, next, on, 500, 1000, 1, false).box).toEqual({ x0: 130, x1: 258 });
    // Much wider than the lit notes need (a wide chord ended): a narrower box.
    const wide = { x0: 100, x1: 400 };
    expect(glowNext(wide, [span(3, 100, 400)], [span(4, 300, 320)], 500, 1000, 1, false).box).toEqual({ x0: 300, x1: 364 });
    // From nothing, and after a page turn, zoom or another take (fresh), a new box at once.
    expect(glowNext(null, [], next, 500, 1000, 1, false).box).toEqual({ x0: 100, x1: 196 });
    expect(glowNext({ x0: 700, x1: 796 }, shown, next, 500, 1000, 1, true)).toEqual({ lit: next, box: { x0: 100, x1: 196 }, pending: false });
  });

  it("a change in one frame when the old and new boxes fit in the limit together; else first only the notes that stay lit, then the new ones", () => {
    const shown = [span(0, 100, 300), span(1, 200, 400)];
    const kept = { x0: 100, x1: 400 };
    const near = [span(1, 200, 400), span(2, 380, 500)];
    expect(glowNext(kept, shown, near, 500, 1000, 1, false)).toEqual({ lit: near, box: { x0: 200, x1: 552 }, pending: false });
    const far = [span(1, 200, 400), span(3, 390, 700)];
    const first = glowNext(kept, shown, far, 500, 1000, 1, false);
    expect(notesOf(first.lit)).toEqual([1]);
    expect(first).toMatchObject({ box: { x0: 200, x1: 400 }, pending: true });
    expect(glowNext(first.box, first.lit, far, 500, 1000, 1, false)).toEqual({ lit: far, box: { x0: 200, x1: 700 }, pending: false });
  });

  it("all go out at once, and light from nothing at once", () => {
    const shown = [span(0, 0, 600)];
    expect(glowNext({ x0: 0, x1: 600 }, shown, [], 600, 1000, 1, false)).toEqual({ lit: [], box: { x0: 0, x1: 600 }, pending: false });
    expect(glowNext(null, shown, [], 600, 1000, 1, false)).toEqual({ lit: [], box: null, pending: false });
    const target = [span(1, 0, 400)];
    expect(glowNext(null, [], target, 400, 1000, 1, false)).toEqual({ lit: target, box: { x0: 0, x1: 400 }, pending: false });
  });
});

describe("the picture", () => {
  const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8, hatch: 12 };
  const shown = G.shownOf(
    take([
      [TPQ, 2 * TPQ, 64, 100],
      [TPQ / 2, TPQ, 60, 40],
    ]),
    0,
  );
  const colorOf = (r: number[]) => [r[4], r[5], r[6]].join(",");
  const is = (name: Parameters<typeof rgb>[0]) => Array.from(Float32Array.from(rgb(name))).join(",");

  it("a lit note is drawn in the track's color, with the gray.10 edge of every note, exactly where the note area draws it", () => {
    const roll: RollPicture = {
      g: GEOM,
      sizes: SIZES,
      scale: { root: 0, mode: "major", lock: false, highlight: false },
      meter: { num: 4, den: 4 },
      step: TPQ / 4,
      shown,
      origin: null,
      selected: null,
      live: null,
      drag: null,
      eraseHover: -1,
    };
    const big = new Batch();
    big.clip(0, 0, 696, 492);
    paintRoll(big, { x0: 0, y0: 0, x1: 696, y1: 492 }, roll, null, 1);
    // The note's two rectangles (the row behind it starts at the left edge).
    const faceOf = (rects: number[][], y: number) => rects.filter((r) => r[0]! > 0 && r[1] === y && r[3]! - r[1]! === 11);

    // Note 0 (E4, 8 rows below C5): from 40 px to 120 px. Its box starts at 40 and is as tall as the note area.
    const face = trackColor({ color: 3 });
    const left = 40;
    const lit = new Batch();
    lit.clip(0, 0, 80, 492);
    paintLive(lit, { x0: 0, y0: 0, x1: 80, y1: 492 }, { g: GEOM, sizes: SIZES, left, top: 0, ty: 0, areaH: 492, notes: [{ start: TPQ, end: 3 * TPQ, key: 64, face }] }, 1);
    const glowRects = faceOf(lit.rects(), 96).map((r) => [r[0]! + left, r[1], r[2]! + left, r[3], colorOf(r)]);
    const rollRects = faceOf(big.rects(), 96).map((r) => [r[0], r[1], r[2], r[3], colorOf(r)]);
    expect(glowRects.map((r) => r.slice(0, 4))).toEqual(rollRects.map((r) => r.slice(0, 4)));
    expect(glowRects.map((r) => r[4])).toEqual([is("color.gray.10"), is(face)]);
    expect(face).toBe("color.track.3");
  });
});

describe("the fake keeps the setting", () => {
  it("on at first; setNoteGlow turns it off and on again", async () => {
    const t = createFakeTransport(fakeState());
    await t.subscribe(
      () => {},
      () => {},
    );
    const now = async () => ((await t.snapshot()).state as AppState).workspace.noteGlow;
    expect(await now()).toBe(true);
    await t.send({ type: "setNoteGlow", on: false } satisfies Command);
    expect(await now()).toBe(false);
    await t.send({ type: "setNoteGlow", on: true } satisfies Command);
    expect(await now()).toBe(true);
  });
});
