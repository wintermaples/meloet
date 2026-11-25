// Songs whose time signature changes (UI-ROLL-004, -005, -006): the readings while dragging, the snapping of CC points and pastes,
// and one step of the 「1 小節」 grid follow the tempo map, not only the song's first time signature.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Command, Grid } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { Auditioner } from "../src/components/pianoroll/audition";
import { LaneMouse, valueText, type LaneBox } from "../src/components/pianoroll/cc";
import { pianoRollCommand } from "../src/components/pianoroll/commands";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import Roll from "../src/components/pianoroll/Roll.svelte";
import { DEFAULT_ROW_PX, dragReading, prView, pxPerTick } from "../src/components/pianoroll/view.svelte";
import { gridTick, mapOf, meterAtOf } from "../src/time/bars";
import type { TimelineShape } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import * as shownFake from "./fake-shown.svelte";
import { GEOM, take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const NONE: Mods = { shift: false, alt: false, mod: false };
const PB = { kind: "pitchBend" } as const;
const LANE: LaneBox = { height: 48, pad: 9 };

/** A time signature at the start, and changes as [bar from 0, num, den]. */
function timeline(num: number, den: number, changes: [number, number, number][]): TimelineShape {
  return {
    tempoBpm: 120,
    meterNum: num,
    meterDen: den,
    headRamp: false,
    tempoPoints: [],
    meterChanges: changes.map(([bar, meterNum, meterDen]) => ({ bar, meterNum, meterDen })),
  };
}
/** 4/4, then 3/4 from bar 5 (tick 30720): bar 6 starts at 36480, bar 7 at 42240. */
const FOUR_THEN_THREE = timeline(4, 4, [[4, 3, 4]]);
/** 4/4, then 6/8 from bar 2 (tick 7680; a beat is 960). */
const FOUR_THEN_SIX = timeline(4, 4, [[1, 6, 8]]);
/** 3/4, then 4/4 from bar 2 (tick 5760). With the grid 1/2 (3840) the lines are 0, 3840, 5760, 9600, 13440 … */
const THREE_THEN_FOUR = timeline(3, 4, [[1, 4, 4]]);

const ppt = GEOM.ppt;
const keyY = (key: number) => (GEOM.topKey - key) * GEOM.rowH + 5;

/** The piano roll's mouse wired the way Roll.svelte wires it. */
function rollDrag(tl: TimelineShape, grid: Grid, shown: G.Shown | null, scrollTick: number, sent: Command[], tool: "draw" | "select" = "draw"): RollDrag {
  const map = mapOf(tl);
  const step = G.gridTicks(grid, { num: tl.meterNum, den: tl.meterDen });
  return new RollDrag({
    shown: () => shown,
    geom: () => ({ ...GEOM, scrollTick }),
    step: (tick) => G.gridTicks(grid, meterAtOf(map, tick)),
    grid: () => (tick, floor) => gridTick(map, tick, step, floor, grid.division === 0),
    scale: () => ({ root: 0, mode: "major", lock: false, highlight: true }),
    meter: () => map,
    tool: () => tool,
    selected: () => [],
    edgePx: () => 6,
    stillPx: () => 4,
    velocityFullPx: () => 64,
    velocityHeight: () => 71,
    send: (c) => void sent.push(c),
    drawVelocity: () => 100,
    mayAudition: () => false,
    ticksToMs: (ticks) => ticks,
    audition: new Auditioner(() => {}),
  });
}

/** A CC lane (pitch bend) wired the way CcLanes.svelte wires it. */
function laneMouse(tl: TimelineShape, grid: Grid, shown: G.Shown | null, sent: Command[], tool: "draw" | "select"): LaneMouse {
  const map = mapOf(tl);
  const step = G.gridTicks(grid, { num: tl.meterNum, den: tl.meterDen });
  return new LaneMouse({
    shown: () => shown,
    control: () => PB,
    geom: () => GEOM,
    box: () => LANE,
    step: () => step,
    grid: () => (tick, floor) => gridTick(map, tick, step, floor, grid.division === 0),
    meter: () => map,
    tool: () => tool,
    selected: () => [],
    reachPx: () => 4,
    stillPx: () => 4,
    send: (c) => void sent.push(c),
  });
}

const SIXTEENTH: Grid = { division: 16, triplet: false };
const EIGHTH: Grid = { division: 8, triplet: false };
const HALF: Grid = { division: 2, triplet: false };
const BAR_GRID: Grid = { division: 0, triplet: false };

describe("readings while dragging follow the time signature changes (UI-ROLL-004)", () => {
  it("drawing at bar 7 beat 1 of a 4/4 song with 3/4 from bar 5 reads 7.1.0000", () => {
    const sent: Command[] = [];
    const d = rollDrag(FOUR_THEN_THREE, SIXTEENTH, null, 20 * TPQ, sent);
    d.down((42240 - 20 * TPQ) * ppt + 2, keyY(50), NONE);
    expect(d.reading()).toBe("D3 ・ 7.1.0000 ・ 長さ 0.0480");
  });

  it("in a 6/8 part, an eighth is one beat: drawn length 1.0000, moved by +1.0000", () => {
    const sent: Command[] = [];
    const draw = rollDrag(FOUR_THEN_SIX, EIGHTH, null, 0, sent);
    draw.down(7680 * ppt + 2, keyY(50), NONE);
    expect(draw.reading()).toBe("D3 ・ 2.1.0000 ・ 長さ 1.0000");
    const n = G.shownOf(take([[7680, 960, 60]]), 0);
    const move = rollDrag(FOUR_THEN_SIX, EIGHTH, n, 0, sent, "select");
    move.down(7680 * ppt + 10, keyY(60), NONE);
    move.move(7680 * ppt + 10 + 960 * ppt, keyY(60), NONE);
    expect(move.reading()).toBe("+1.0000 ・ +0 半音");
  });

  it("a CC point added at bar 7 of the same song reads 7.1.0000", () => {
    const sent: Command[] = [];
    const m = laneMouse(FOUR_THEN_THREE, SIXTEENTH, G.shownOf(take([[0, TPQ, 60]]), 0), sent, "draw");
    m.down(42240 * ppt + 2, 24, NONE);
    expect(m.reading()).toBe(`${valueText(PB, 0)} ・ 7.1.0000`);
  });
});

describe("CC points and pastes land on the grid lines (UI-ROLL-005)", () => {
  it("3/4 with 4/4 from bar 2, grid 1/2: a CC point drawn at tick 9000 lands on 9600", () => {
    const sent: Command[] = [];
    const m = laneMouse(THREE_THEN_FOUR, HALF, G.shownOf(take([[0, TPQ, 60]]), 0), sent, "draw");
    m.down(9000 * ppt, 24, NONE);
    expect(m.up()).toEqual({ type: "addControl", take: 7, tick: 9600, control: PB, value: 0 });
  });

  it("a CC point at 5760 moved to near 9000 lands on 9600", () => {
    const sent: Command[] = [];
    const withPoint = take([[0, TPQ, 60]], 7, {
      controlTicks: Uint32Array.from([5760]),
      controlKinds: Uint8Array.from([1]),
      controlNumbers: Uint8Array.from([0]),
      controlValues: Int16Array.from([0]),
    });
    const m = laneMouse(THREE_THEN_FOUR, HALF, G.shownOf(withPoint, 0), sent, "select");
    m.down(5760 * ppt, 24, NONE);
    m.move(9000 * ppt, 24, NONE);
    expect(m.up()).toEqual({ type: "moveControls", take: 7, points: [0], ticks: 3840, value: 0 });
  });

  it("Mod V at the play line (tick 9000) pastes at 9600", () => {
    const state = fakeState();
    state.timeline = { ...state.timeline, ...THREE_THEN_FOUR };
    state.pianoRoll = { ...state.pianoRoll, clipboardNotes: 2, grid: HALF };
    const n = G.shownOf(take([[0, TPQ, 60]]), 0, { from: 0, to: 20000 });
    expect(pianoRollCommand({ kind: "paste" }, { state, shown: n, playTick: 9000 })).toEqual({ type: "pasteNotes", at: 9600 });
  });
});

describe("the 「1 小節」 grid in bars of another time signature (UI-ROLL-006)", () => {
  it("a note drawn at the head of bar 6 (3/4) is one 3/4 bar long", () => {
    const sent: Command[] = [];
    const d = rollDrag(FOUR_THEN_THREE, BAR_GRID, null, 30720, sent);
    d.down((36480 - 30720) * ppt + 2, keyY(50), NONE);
    expect(d.up()).toEqual({ type: "addNote", take: null, start: 36480, length: 5760, key: 50, velocity: 100 });
  });

  it("→ moves a note at the head of bar 5 by a 3/4 bar; ← by the 4/4 bar before. Mod D copies by the bar at the notes", () => {
    const state = fakeState();
    state.timeline = { ...state.timeline, ...FOUR_THEN_THREE };
    state.pianoRoll = { ...state.pianoRoll, take: 7, selectedNotes: [{ start: 0, end: 1 }], grid: BAR_GRID };
    const at5 = G.shownOf(take([[30720, TPQ, 60]]), 0);
    expect(pianoRollCommand({ kind: "nudge", steps: 1, keys: 0 }, { state, shown: at5, playTick: 0 })).toMatchObject({ ticks: 5760 });
    expect(pianoRollCommand({ kind: "nudge", steps: -1, keys: 0 }, { state, shown: at5, playTick: 0 })).toMatchObject({ ticks: -7680 });
    const at6 = G.shownOf(take([[36480, TPQ, 60]]), 0);
    expect(pianoRollCommand({ kind: "duplicate" }, { state, shown: at6, playTick: 0 })).toMatchObject({ type: "moveNotes", ticks: 5760, copy: true });
  });
});

describe("the piano roll hands the tempo map to its readings (mount)", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  beforeEach(() => {
    app.reset();
    prView.tool = "draw";
    prView.quarterPx = 40;
    prView.rowPx = DEFAULT_ROW_PX;
    prView.topKey = 72;
    prView.scrollTick = 0;
    target = document.createElement("div");
    document.body.append(target);
  });
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    target.remove();
    dragReading.text = null;
    prView.tool = "select";
  });

  it("drawing at bar 7 beat 1 after a change to 3/4 at bar 5 reads 7.1.0000", async () => {
    const s = fakeScene("pianoRoll")!.state;
    s.timeline = { ...s.timeline, meterChanges: [{ bar: 4, meterNum: 3, meterDen: 4 }] };
    expect([s.timeline.meterNum, s.timeline.meterDen, s.pianoRoll.grid.division]).toEqual([4, 4, 16]);
    const track = s.song.tracks[0]!;
    const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
    const shownTake = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
    app.setState(s);
    shownFake.setShown({ clip, track, take: shownTake });
    mounted = mount(Roll, { target });
    flushSync();
    const area = target.querySelector(".area")!;
    area.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0, pointerId: 1, clientX: 42240 * pxPerTick(100) + 2, clientY: keyY(50) }));
    flushSync();
    expect(dragReading.text).toBe("D3 ・ 7.1.0000 ・ 長さ 0.0480");
  });
});
