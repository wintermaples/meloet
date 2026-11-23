// Pure functions around the piano roll: how list numbers are written, a summary of the selected notes, the notes playing, the arrange's horizontal zoom and the overview frame.
import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import * as G from "../src/components/arrangeGeom";
import { gridText, noteLengthText, notePositionText, pitchClassName, scaleName } from "../src/components/format";
import { clipNoteIndexes, inRanges, selectedSummary } from "../src/components/pianoroll/notes";
import { sameKeys, soundingKeys } from "../src/components/pianoroll/sounding";
import { ARRANGE_BAR_STEPS, arrangeView, DEFAULT_ARRANGE_BAR, stepFrom, zoomArrangeBy } from "../src/components/pianoroll/view.svelte";
import { noteName } from "../src/components/meterFormat";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const FOUR = { num: 4, den: 4 };
const BAR = 4 * TPQ;
const at = (bar: number, beat: number, tick: number) => (bar - 1) * BAR + (beat - 1) * TPQ + tick;

/** The take shown in the fake scene pianoRoll (mock A-PianoRoll). */
async function sceneTake(name = "pianoRoll") {
  const scene = fakeScene(name)!;
  const s = scene.state;
  const clip = s.song.tracks[0]!.clips[0]!;
  const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  return { s, clip, take };
}

describe("note list numbers (mock A-PianoRoll)", () => {
  it("position is 「小節.拍.ティック」, length is 「拍.ティック」", () => {
    expect(notePositionText(at(3, 1, 0), FOUR)).toBe("3.1.0000");
    expect(notePositionText(at(1, 4, TPQ / 2), FOUR)).toBe("1.4.0960");
    expect(noteLengthText(4 * TPQ, FOUR)).toBe("4.0000");
    expect(noteLengthText((3 * TPQ) / 4, FOUR)).toBe("0.1440");
    expect(noteLengthText((3 * TPQ) / 2, FOUR)).toBe("1.0960");
  });

  it("in an eighth-note time signature, the beat is an eighth note (the display splits the beat into 480)", () => {
    const sixEight = { num: 6, den: 8 };
    expect(notePositionText((TPQ / 2) * 7, sixEight)).toBe("2.2.0000");
    expect(noteLengthText((3 * TPQ) / 4, sixEight)).toBe("1.0480");
  });

  it("grid and scale names", () => {
    expect(gridText(0, false)).toBe("1 小節");
    expect(gridText(16, false)).toBe("1/16");
    expect(gridText(8, true)).toBe("1/8 三連符");
    expect(scaleName(0, "major")).toBe("C メジャー");
    expect(scaleName(9, "minor")).toBe("A マイナー");
    expect(pitchClassName(63)).toBe("D#");
  });
});

describe("summary of the selected notes (the right panel of mock A-PianoRoll)", () => {
  it("D4 E4 F4: position is the earliest, 3.1.0000; lengths differ (null); velocity is 77 – 83", async () => {
    const { s, clip, take } = await sceneTake();
    const sum = selectedSummary(take, clip, s.pianoRoll.selectedNotes, FOUR)!;
    expect(sum).toEqual({ count: 3, position: "3.1.0000", keys: "D4 E4 F4", length: null, velocityLow: 77, velocityHigh: 83 });
  });

  it("with only one, the length is its value. null when nothing is selected", async () => {
    const { clip, take } = await sceneTake();
    expect(selectedSummary(take, clip, [{ start: 14, end: 15 }], FOUR)).toMatchObject({ count: 1, length: "0.1440", velocityLow: 80, velocityHigh: 80 });
    expect(selectedSummary(take, clip, [], FOUR)).toBeNull();
  });

  it("position is in the song: if the clip starts at bar 5, later by that much", async () => {
    const { clip, take } = await sceneTake();
    const later = { ...clip, start: clip.start + 4 * BAR, end: clip.end + 4 * BAR };
    expect(selectedSummary(take, later, [{ start: 14, end: 15 }], FOUR)!.position).toBe("7.1.0000");
  });

  it("checks with a binary search whether it is inside the selected range", () => {
    const ranges = [
      { start: 2, end: 4 },
      { start: 10, end: 11 },
    ];
    expect([0, 2, 3, 4, 9, 10, 11].map((i) => inRanges(ranges, i))).toEqual([false, true, true, false, false, true, false]);
  });

  it("the list shows only notes that touch the clip range", () => {
    const take = { starts: [0, 100, 2000], lengths: [50, 50, 10] };
    expect([...clipNoteIndexes({ start: 0, end: 1000, takeOffset: 0 }, take)]).toEqual([0, 1]);
    expect([...clipNoteIndexes({ start: 0, end: 1000, takeOffset: -60 }, take)]).toEqual([1]);
  });
});

describe("notes playing (mock F-PRPlay 「鳴っている音 C3 E3 G3 C4」)", () => {
  it("at 4.1.0960, C3 E3 G3 C4 are playing (low to high)", async () => {
    const { clip, take } = await sceneTake("pianoRollPlay");
    expect(soundingKeys(take, clip, at(4, 1, TPQ / 2)).map(noteName)).toEqual(["C3", "E3", "G3", "C4"]);
  });

  it("not playing exactly at the end of a note. Nothing plays outside the clip", () => {
    const take = { id: 7, starts: [0, 0], lengths: [TPQ, 2 * TPQ], keys: [64, 60] };
    const clip = { start: 0, end: BAR, takeOffset: 0, takeId: 7 };
    expect(soundingKeys(take, clip, 0)).toEqual([60, 64]);
    expect(soundingKeys(take, clip, TPQ)).toEqual([60]);
    expect(soundingKeys(take, { ...clip, end: 400 }, 400)).toEqual([]);
    expect(soundingKeys(take, { ...clip, start: BAR, end: 2 * BAR }, BAR)).toEqual([60, 64]);
    // when the clip cuts the start of the take, notes that start in the cut part do not play (the same as Rust's track_events).
    expect(soundingKeys(take, { ...clip, takeOffset: -TPQ }, 0)).toEqual([]);
    expect(soundingKeys({ ...take, starts: [0, TPQ] }, { ...clip, takeOffset: -TPQ }, 0)).toEqual([60]);
  });

  it("nothing plays while fetching the clip's take (still the old take) or when there is no take or clip", () => {
    const take = { id: 7, starts: [0], lengths: [TPQ], keys: [60] };
    const clip = { start: 0, end: BAR, takeOffset: 0, takeId: 8 };
    expect(soundingKeys(take, clip, 0)).toEqual([]);
    expect(soundingKeys(null, clip, 0)).toEqual([]);
    expect(soundingKeys(take, null, 0)).toEqual([]);
  });

  it("the same key twice counts as one", () => {
    expect(soundingKeys({ id: 1, starts: [0, 10], lengths: [100, 100], keys: [60, 60] }, { start: 0, end: 500, takeOffset: 0, takeId: 1 }, 50)).toEqual([60]);
    expect(sameKeys([60, 64], [60, 64])).toBe(true);
    expect(sameKeys([60], [60, 64])).toBe(false);
  });
});

describe("arrange horizontal zoom in and out", () => {
  it("steps are centered on 64, 6 steps below and 4 above (at least 4 each way)", () => {
    const i = ARRANGE_BAR_STEPS.indexOf(DEFAULT_ARRANGE_BAR);
    expect(i).toBeGreaterThanOrEqual(4);
    expect(ARRANGE_BAR_STEPS.length - 1 - i).toBeGreaterThanOrEqual(4);
    expect([...ARRANGE_BAR_STEPS]).toEqual([...ARRANGE_BAR_STEPS].sort((a, b) => a - b));
  });

  it("moves one step at a time and stops at the ends", () => {
    expect(stepFrom(ARRANGE_BAR_STEPS, 64, 1, 64)).toBe(91);
    expect(stepFrom(ARRANGE_BAR_STEPS, 8, -1, 64)).toBe(8);
    expect(stepFrom(ARRANGE_BAR_STEPS, 999, 1, 64)).toBe(91);
    arrangeView.barPx = 256;
    expect(zoomArrangeBy(1)).toBe(false);
    expect(zoomArrangeBy(-1)).toBe(true);
    expect(arrangeView.barPx).toBe(181);
    arrangeView.barPx = DEFAULT_ARRANGE_BAR;
  });

  it("when zoomed out and bar numbers overlap, skips some (every 1, 2, 4, 8 bars)", () => {
    expect(G.labelEvery(64, 32)).toBe(1);
    expect(G.labelEvery(23, 32)).toBe(2);
    expect(G.labelEvery(8, 32)).toBe(4);
  });

  it("Mod + wheel: turning up zooms in. At most one step per turn. Small moves add up first", () => {
    expect(G.wheelZoomSteps(0, -100, 0)).toEqual({ steps: 1, acc: 0 });
    expect(G.wheelZoomSteps(0, 3, 1)).toEqual({ steps: -1, acc: 0 });
    let z = { steps: 0, acc: 0 };
    const steps: number[] = [];
    for (let i = 0; i < 10; i++) {
      z = G.wheelZoomSteps(z.acc, -10, 0);
      steps.push(z.steps);
    }
    expect(steps).toEqual([0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    expect(G.wheelZoomSteps(-40, 10, 0)).toEqual({ steps: 0, acc: 10 });
  });
});

describe("the overview's 「見えている範囲の枠」 (spec 3, mock A-PianoRoll, F-PRPlay)", () => {
  const view = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
  const o = { inset: 1, hair: 1, dpr: 1 };

  it("A-PianoRoll: the frame for bars 1 to 5 (4 bars) is x 0 to 256, a 1px accent line leaving 1 above and below", () => {
    const rects = G.rangeFrameRects(view, { start: 0, end: 4 * BAR }, 39, o);
    expect(rects).toEqual([
      { x: 0, y: 1, w: 256, h: 1, color: "color.accent" },
      { x: 0, y: 37, w: 256, h: 1, color: "color.accent" },
      { x: 0, y: 1, w: 1, h: 37, color: "color.accent" },
      { x: 255, y: 1, w: 1, h: 37, color: "color.accent" },
    ]);
  });

  it("edges outside the visible width are not drawn. Nothing if it is all outside", () => {
    const left = G.rangeFrameRects({ ...view, scrollX: 100 }, { start: 0, end: 4 * BAR }, 39, o);
    expect(left).toHaveLength(3);
    expect(left[0]).toMatchObject({ x: 0, w: 156 });
    expect(G.rangeFrameRects(view, { start: 100 * BAR, end: 104 * BAR }, 39, o)).toEqual([]);
  });

  it("sends when the frame goes outside the overview. Does not move if it is inside", () => {
    expect(G.revealRange(100, 300, 0, 1000)).toBe(0);
    expect(G.revealRange(1200, 1400, 0, 1000)).toBe(800);
    expect(G.revealRange(1200, 2600, 0, 1000)).toBe(1200);
    expect(G.revealRange(50, 250, 100, 1000)).toBe(0);
  });
});
