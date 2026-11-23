// Writing and moving outside the clip (docs/history/design-notes.ja.md B-82): the screen sends intents without stopping outside the clip, and Rust does the extending.
import { beforeEach, describe, expect, it } from "vitest";
import type { Command, IndexRange } from "../src/bridge";
import { fakeScene, fakeState } from "../src/bridge/fake";
import { Auditioner } from "../src/components/pianoroll/audition";
import { pianoRollCommand } from "../src/components/pianoroll/commands";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import type { Tool } from "../src/components/pianoroll/view.svelte";
import { t } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const NONE: Mods = { shift: false, alt: false, mod: false };
const BAR = 4 * TPQ;

// the clip is bar 2 [BAR, 2 * BAR). Note 0 is C4 for 1 beat from 2.1, note 1 is D4 for 1 beat from 2.4 (up to the clip end).
const n = G.shownOf(
  take([
    [0, TPQ, 60],
    [3 * TPQ, TPQ, 62],
  ]),
  BAR,
  { from: BAR, to: 2 * BAR },
);
// visible from the start of bar 1 to the end of bar 3.
const geom: G.RollGeom = { ...GEOM, ppt: GEOM.width / (3 * BAR) };
const y = (key: number) => (geom.topKey - key) * geom.rowH + 5;
const x = (tick: number) => (tick - geom.scrollTick) * geom.ppt;

let sent: Command[];
let tool: Tool;
let selected: IndexRange[];
let drag: RollDrag;

beforeEach(() => {
  sent = [];
  tool = "select";
  selected = [];
  drag = new RollDrag({
    shown: () => n,
    geom: () => geom,
    step: () => TPQ / 4,
    scale: () => ({ root: 0, mode: "major", lock: false }),
    meter: () => ({ num: 4, den: 4 }),
    tool: () => tool,
    selected: () => selected,
    edgePx: () => 6,
    stillPx: () => 4,
    velocityFullPx: () => 64,
    velocityHeight: () => 71,
    send: (c) => void sent.push(c),
    drawVelocity: () => G.DRAW_VELOCITY,
    mayAudition: () => false,
    ticksToMs: (ticks) => ticks,
    audition: new Auditioner(() => {}),
  });
});

describe("draw tool", () => {
  it("pressing after the clip sends AddNote to that take (not rounded)", () => {
    tool = "draw";
    drag.down(x(2 * BAR + (25 * TPQ) / 24), y(64), NONE);
    expect(drag.up()).toEqual({ type: "addNote", take: 7, start: 2 * BAR + TPQ, length: TPQ / 4, key: 64, velocity: G.DRAW_VELOCITY });
  });

  it("the same before the clip (bar 1)", () => {
    tool = "draw";
    drag.down(x((25 * TPQ) / 48), y(64), NONE);
    expect(drag.up()).toEqual({ type: "addNote", take: 7, start: TPQ / 2, length: TPQ / 4, key: 64, velocity: G.DRAW_VELOCITY });
  });

  it("holding and stretching right from inside the clip to outside sends the stretched length", () => {
    tool = "draw";
    drag.down(x(2 * BAR - TPQ / 2), y(64), NONE);
    drag.move(x(2 * BAR + (5 * TPQ) / 4), y(64), NONE);
    expect(drag.up()).toMatchObject({ type: "addNote", start: 2 * BAR - TPQ / 2, length: (7 * TPQ) / 4 });
  });
});

describe("select tool", () => {
  it("dragging can move notes after the clip", () => {
    drag.down(x(BAR + (25 * TPQ) / 8), y(62), NONE);
    drag.move(x(BAR + (25 * TPQ) / 8 + 2 * TPQ), y(62), NONE);
    expect(drag.up()).toEqual({ type: "moveNotes", take: 7, notes: [1], ticks: 2 * TPQ, keys: 0, copy: false });
  });

  it("dragging can move notes before the clip. Stops at the start of the song", () => {
    drag.down(x(BAR + (5 * TPQ) / 24), y(60), NONE);
    drag.move(x(0) - 200, y(60), NONE);
    expect(drag.up()).toEqual({ type: "moveNotes", take: 7, notes: [0], ticks: -BAR, keys: 0, copy: false });
  });

  it("the right edge can be stretched outside the clip", () => {
    drag.down(x(2 * BAR) - 2, y(62), NONE);
    drag.move(x(2 * BAR + 2 * TPQ) - 2, y(62), NONE);
    expect(drag.up()).toEqual({ type: "resizeNotes", take: 7, notes: [1], edge: "end", ticks: 2 * TPQ });
  });
});

describe("← →", () => {
  it("a note at the clip end also becomes an intent to move one grid step outside", () => {
    const state = fakeState();
    state.pianoRoll = { ...state.pianoRoll, take: 7, selectedNotes: [{ start: 1, end: 2 }], grid: { division: 16, triplet: false } };
    expect(pianoRollCommand({ kind: "nudge", steps: 1, keys: 0 }, { state, shown: n, playTick: 0 })).toEqual({
      type: "moveNotes",
      take: 7,
      notes: [1],
      ticks: TPQ / 4,
      keys: 0,
      copy: false,
    });
    state.pianoRoll = { ...state.pianoRoll, selectedNotes: [{ start: 0, end: 1 }] };
    expect(pianoRollCommand({ kind: "nudge", steps: -1, keys: 0 }, { state, shown: n, playTick: 0 })).toMatchObject({ notes: [0], ticks: -TPQ / 4 });
  });
});

describe("notices", () => {
  it("text when it could not extend (written, moved)", () => {
    expect(fakeScene("notesNotWritten")!.state.status.message).toEqual({ id: "notesNotWritten" });
    expect(t("msg.notesNotWritten")).toBe("ほかのクリップと重なるので、書けませんでした。");
    expect(t("msg.notesNotResized")).toBe("ほかのクリップと重なるので、長さを変えられませんでした。");
    expect(t("msg.clipsNotMoved")).toBe("ほかのクリップと重なるので、動かしませんでした。");
  });

  it("scene of an extended clip: bars 1–5, the written note is visible", () => {
    const s = fakeScene("pianoRollGrown")!.state;
    const clip = s.song.tracks[0]!.clips[0]!;
    expect([clip.start, clip.end]).toEqual([0, 5 * BAR]);
    expect(G.barsText(clip.start, clip.end, { num: 4, den: 4 })).toBe("1–5");
  });
});
