// The CC band (cc.ts, paintLane in paint.ts): value and height, picking points, the rule to fold bands, the mouse (draw, select, move, box, erase, Esc),
// on/off snapping, readouts, the picture (6×6 points, lines, the 0 line of pitch bend). Q, and Delete on points. The fake scene's CC counts, and the frame of the position before aligning.
import { beforeEach, describe, expect, it } from "vitest";
import { type AppState, type Command, type ControlKind, type IndexRange } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import {
  deltaText,
  fitLanes,
  hitPoint,
  lanePoints,
  LaneMouse,
  pointsInBox,
  settle,
  valueToY,
  yToValue,
  type LaneBox,
} from "../src/components/pianoroll/cc";
import * as G from "../src/components/pianoroll/geom";
import { paintLane, type LanePicture } from "../src/components/pianoroll/paint";
import { prView, type Tool } from "../src/components/pianoroll/view.svelte";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const SUSTAIN: ControlKind = { kind: "cc", number: 64 };
const PB: ControlKind = { kind: "pitchBend" };
const MOD: ControlKind = { kind: "cc", number: 1 };
/** 48 each, 9 of space above and below (mock R3-Lanes). */
const LANE: LaneBox = { height: 48, pad: 9 };

/** Sustain 127 and 0 (tick 0 and TPQ), pitch bend 0 and 4096 (TPQ / 2 and 2 * TPQ), modulation 64 ((3 * TPQ) / 2). In tick order. */
const withControls = (id = 7) =>
  take([[0, TPQ, 60]], id, {
    controlTicks: Uint32Array.from([0, TPQ / 2, TPQ, (3 * TPQ) / 2, 2 * TPQ]),
    controlKinds: Uint8Array.from([0, 1, 0, 0, 1]),
    controlNumbers: Uint8Array.from([64, 0, 64, 1, 0]),
    controlValues: Int16Array.from([127, 0, 0, 64, 4096]),
  });
const shown = (base = 0) => G.shownOf(withControls(), base);

describe("value and height", () => {
  it("for CC, 127 is at the top space of 9 and 0 at the bottom space of 9. Pitch bend 0 is in the middle", () => {
    expect(valueToY(MOD, LANE, 127)).toBe(9);
    expect(valueToY(MOD, LANE, 0)).toBe(39);
    expect(valueToY(PB, LANE, 0)).toBe(24);
    expect(valueToY(PB, LANE, -8192)).toBe(39);
    expect(yToValue(MOD, LANE, valueToY(MOD, LANE, 100))).toBe(100);
    expect(yToValue(PB, LANE, valueToY(PB, LANE, 512))).toBe(512);
    expect(yToValue(PB, LANE, -100)).toBe(8191);
  });

  it("on/off CCs (64 to 69) stop only at 0 or 127. Others stop at the range", () => {
    expect(settle(SUSTAIN, 63)).toBe(0);
    expect(settle(SUSTAIN, 64)).toBe(127);
    expect(settle({ kind: "cc", number: 69 }, 100)).toBe(127);
    expect(settle({ kind: "cc", number: 70 }, 100)).toBe(100);
    expect(settle(MOD, 300)).toBe(127);
    expect(yToValue(SUSTAIN, LANE, 20)).toBe(127);
    expect(yToValue(SUSTAIN, LANE, 30)).toBe(0);
  });
});

describe("points on a band", () => {
  it("each band has only points of its kind (in tick order)", () => {
    const t = withControls();
    expect([...lanePoints(t, SUSTAIN)]).toEqual([0, 2]);
    expect([...lanePoints(t, PB)]).toEqual([1, 4]);
    expect([...lanePoints(t, MOD)]).toEqual([3]);
    expect([...lanePoints(t, { kind: "cc", number: 7 })]).toEqual([]);
  });

  it("over a point (within 4 both ways). A box gets the points it covers", () => {
    const n = shown();
    // 1 beat is 40 px: TPQ is 40 px.
    expect(hitPoint(n, SUSTAIN, GEOM, LANE, 40, 39, 4)).toBe(2);
    expect(hitPoint(n, SUSTAIN, GEOM, LANE, 45, 39, 4)).toBe(-1);
    expect(hitPoint(n, SUSTAIN, GEOM, LANE, 40, 9, 4)).toBe(-1);
    expect(pointsInBox(n, PB, GEOM, LANE, 0, 0, 100, 48)).toEqual([1, 4]);
    expect(pointsInBox(n, PB, GEOM, LANE, 0, 20, 100, 48)).toEqual([1]);
  });
});

describe("band height (spec 3: when notes drop under 10 rows, fold the older band)", () => {
  it("fold from the one opened earliest. The newest band stays", () => {
    expect(fitLanes(["pitchBend", "sustain"], 120, 48, 12)).toEqual(["pitchBend", "sustain"]);
    expect(fitLanes(["pitchBend", "sustain"], 119, 48, 12)).toEqual(["sustain"]);
    expect(fitLanes(["modulation", "pitchBend", "sustain"], 80, 48, 12)).toEqual(["pitchBend", "sustain"]);
    expect(fitLanes(["sustain"], 10, 48, 12)).toEqual(["sustain"]);
  });
});

describe("mouse (LaneMouse)", () => {
  let sent: Command[];
  let tool: Tool;
  let selected: IndexRange[];
  let control: ControlKind;
  const none = { shift: false, alt: false, mod: false };
  const shift = { ...none, shift: true };
  const make = (base = 0) =>
    new LaneMouse({
      shown: () => shown(base),
      control: () => control,
      geom: () => GEOM,
      box: () => LANE,
      step: () => TPQ / 4,
      meter: () => ({ num: 4, den: 4 }),
      tool: () => tool,
      selected: () => selected,
      reachPx: () => 4,
      stillPx: () => 4,
      send: (c) => void sent.push(c),
    });
  beforeEach(() => {
    sent = [];
    tool = "select";
    selected = [];
    control = PB;
  });

  it("pressing with the draw tool adds a point. Horizontally snaps to the grid (not with Shift), the value is the pressed height", () => {
    tool = "draw";
    const m = make();
    m.down(33, 24, none); // (33 * TPQ) / 40 ticks -> (3 * TPQ) / 4 on the 1/16 grid (TPQ / 4)
    expect(m.up()).toEqual({ type: "addControl", take: 7, tick: (3 * TPQ) / 4, control: PB, value: 0 });
    m.down(33, 9, shift);
    expect(m.up()).toEqual({ type: "addControl", take: 7, tick: (33 * TPQ) / 40, control: PB, value: 8191 });
  });

  it("on/off CCs are 0 or 127 even when drawn", () => {
    tool = "draw";
    control = SUSTAIN;
    const m = make();
    m.down(80, 20, none);
    expect(m.reading()).toBe("踏む（127）");
    expect(m.up()).toMatchObject({ type: "addControl", control: SUSTAIN, value: 127 });
    m.down(80, 30, none);
    expect(m.reading()).toBe("OFF（0）");
    expect(m.up()).toMatchObject({ value: 0 });
  });

  it("pressing a point with the select tool selects it (Shift adds, Mod toggles)", () => {
    const m = make();
    m.down(20, 24, none);
    expect(sent).toEqual([{ type: "selectControls", take: 7, points: [1], mode: "replace" }]);
    expect(m.up()).toBeNull();
    m.down(80, 15, shift);
    expect(sent[1]).toEqual({ type: "selectControls", take: 7, points: [4], mode: "add" });
    m.up();
    m.down(80, 15, { ...none, mod: true });
    expect(sent[2]).toEqual({ type: "selectControls", take: 7, points: [4], mode: "toggle" });
    expect(m.drag).toBeNull();
  });

  it("moving while pressed: horizontally snaps to the grid, vertically the value. Releasing sends one MoveControls. The readout looks like 「+0512 ・ 1.3.0000」", () => {
    selected = [{ start: 1, end: 2 }, { start: 4, end: 5 }];
    const m = make(4 * TPQ);
    // Point 1 is at 4 * TPQ + TPQ / 2 ticks -> x = 4.5 beats × 40 px = 180. Value 0 is y 24.
    m.down(180, 24, none);
    expect(sent).toEqual([]);
    const up = valueToY(PB, LANE, 512) - 24;
    expect(m.move(180 + 23, 24 + up, none)).toBe(true); // (23 * TPQ) / 40 ticks snaps to TPQ / 2
    expect(m.reading()).toBe("+0512 ・ 2.2.0000");
    expect(m.up()).toEqual({ type: "moveControls", take: 7, points: [1, 4], ticks: TPQ / 2, value: 512 });
  });

  it("pressing a selected point and releasing without moving selects only that point", () => {
    selected = [{ start: 1, end: 2 }, { start: 4, end: 5 }];
    const m = make();
    m.down(20, 24, none);
    expect(m.up()).toEqual({ type: "selectControls", take: 7, points: [1], mode: "replace" });
  });

  it("the value stops where every moved point stays in range. Never before the start of the song", () => {
    selected = [{ start: 1, end: 2 }, { start: 4, end: 5 }];
    const m = make();
    m.down(20, 24, none);
    m.move(-200, -500, none);
    expect(m.up()).toEqual({ type: "moveControls", take: 7, points: [1, 4], ticks: -TPQ / 2, value: 8191 - 4096 });
  });

  it("on/off CCs snap to 0 or 127 even when moved", () => {
    control = SUSTAIN;
    const m = make();
    m.down(0, 9, none); // Point 0 (127)
    m.move(0, 30, shift);
    expect(m.reading()).toBe("OFF（0）");
    expect(m.up()).toEqual({ type: "moveControls", take: 7, points: [0], ticks: 0, value: -127 });
  });

  it("box select from an empty place. A plain press clears the selection", () => {
    const m = make();
    m.down(5, 1, none);
    m.move(100, 47, none);
    expect(m.reading()).toBe("点 2");
    expect(m.up()).toEqual({ type: "selectControls", take: 7, points: [1, 4], mode: "replace" });
    m.down(5, 1, none);
    expect(m.up()).toEqual({ type: "selectControls", take: 7, points: [], mode: "replace" });
  });

  it("the erase tool erases on press and drag (DeleteControls). Remembers the point under the pointer", () => {
    tool = "erase";
    const m = make();
    expect(m.hover(80, 15)).toBe(true);
    expect(m.eraseHover).toBe(4);
    m.down(20, 24, none);
    m.move(80, 15, none);
    expect(m.up()).toEqual({ type: "deleteControls", take: 7, points: [1, 4] });
  });

  it("Esc (cancel) goes back to before the move (sends no intent)", () => {
    const m = make();
    m.down(20, 24, none);
    m.move(60, 10, none);
    expect(m.cancel()).toBe(true);
    expect(m.up()).toBeNull();
    expect(sent).toEqual([{ type: "selectControls", take: 7, points: [1], mode: "replace" }]);
  });

  it("CC difference in 3 digits, pitch bend in 4 digits", () => {
    expect(deltaText(MOD, 12)).toBe("+012");
    expect(deltaText(PB, -512)).toBe("−0512");
  });
});

describe("band picture (paintLane)", () => {
  const BOX = { x0: 0, y0: 0, x1: 696, y1: 48 };
  const colorOf = (r: number[]) => [r[4], r[5], r[6]].join(",");
  const is = (name: Parameters<typeof rgb>[0]) => Array.from(Float32Array.from(rgb(name))).join(",");
  const picture = (over: Partial<LanePicture> = {}): LanePicture => ({
    g: GEOM,
    meter: { num: 4, den: 4 },
    step: TPQ / 4,
    control: PB,
    shown: shown(),
    lane: LANE,
    point: 6,
    selected: Uint8Array.from([0, 0, 0, 0, 1]),
    drag: null,
    eraseHover: -1,
    ...over,
  });
  const draw = (p: LanePicture) => {
    const out = new Batch();
    out.clip(BOX.x0, BOX.y0, BOX.x1, BOX.y1);
    paintLane(out, BOX, p, 1);
    return out.rects();
  };
  const squares = (rects: number[][]) => rects.filter((r) => r[2]! - r[0]! === 6 && r[3]! - r[1]! === 6);

  it("points are 6×6 gray.10, accent when selected. Pitch bend has a gray.5 line at 0", () => {
    const rects = draw(picture());
    const dots = squares(rects);
    expect(dots.map((r) => [r[0], r[1], colorOf(r)])).toEqual([
      [17, 21, is("color.gray.10")],
      [77, Math.round(valueToY(PB, LANE, 4096) - 3), is("color.accent.ink")],
    ]);
    expect(rects.some((r) => r[1] === 24 && r[3] === 25 && r[2]! - r[0]! === 696 && colorOf(r) === is("color.gray.5"))).toBe(true);
  });

  it("the grid lines of the note area: bar lines line.bar 2 pixels wide, beat lines line.beat 1 wide (user:2026-11-24)", () => {
    const rects = draw(picture());
    const line = (x: number) => rects.find((r) => r[0] === x && r[1] === 0 && r[3] === 48 && r[2]! - r[0]! < 10)!;
    expect([line(0)[2], colorOf(line(0))]).toEqual([2, is("color.line.bar")]);
    expect([line(160)[2], colorOf(line(160))]).toEqual([162, is("color.line.bar")]);
    expect([line(40)[2], colorOf(line(40))]).toEqual([41, is("color.line.beat")]);
  });

  it("the point under the erase tool is gray.7", () => {
    const dots = squares(draw(picture({ eraseHover: 1 })));
    expect(colorOf(dots[0]!)).toBe(is("color.gray.7"));
  });

  it("sustain is a step line (keeps the value until the next point): a horizontal line at height 127 from point 0 to point 2", () => {
    const rects = draw(picture({ control: SUSTAIN, selected: null }));
    const line = rects.filter((r) => colorOf(r) === is("color.gray.9"));
    expect(line.some((r) => r[0] === 0 && r[2] === 41 && r[1] === 9)).toBe(true);
    // At point 2, it goes straight down from 127 to 0.
    expect(line.some((r) => r[0] === 40 && r[2] === 41 && r[1] === 9 && r[3] === 40)).toBe(true);
  });

  it("while moving, only a gray.7 frame at the old place (lines and points are on the moving surface)", () => {
    const drag = { kind: "move" as const, press: { x: 0, y: 0, moved: true }, points: [1], grab: 1, ticks: TPQ / 4, value: 0, selectOnClick: false };
    const rects = draw(picture({ drag }));
    expect(rects.some((r) => colorOf(r) === is("color.gray.9"))).toBe(false);
    expect(rects.filter((r) => colorOf(r) === is("color.gray.7"))).toHaveLength(4);
  });
});

describe("Q, and Delete on points (spec 4)", () => {
  const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });
  const n = G.shownOf(withControls(), 0);
  let state: AppState;
  let sent: Command[];
  let deps: KeyDeps;
  const press = (key: string) => handleKeydown(new KeyboardEvent("keydown", { key, cancelable: true }), deps);

  beforeEach(() => {
    state = fakeState();
    state.workspace.layout = { ...state.workspace.layout, lower: [{ band: "pianoRoll", height: 640 }] };
    state.pianoRoll = { ...state.pianoRoll, take: 7, selectedNotes: [], selectedControls: [] };
    sent = [];
    deps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" }, pianoRoll: () => n };
    prView.focus = "pianoRoll";
    prView.picked = "notes";
  });

  it("Q is quantize. Does not work while typing", () => {
    expect(keyAction(k("q"), false, "other")).toEqual({ kind: "quantize" });
    expect(keyAction(k("q"), true, "other")).toBeNull();
    expect(keyAction(k("q", { shift: true }), false, "other")).toBeNull();
  });

  it("Quantize the selected notes, or all if none (notes is empty). Does nothing if the piano roll has no focus", () => {
    press("q");
    state.pianoRoll = { ...state.pianoRoll, selectedNotes: [{ start: 0, end: 1 }] };
    press("q");
    prView.focus = "arrange";
    press("q");
    expect(sent).toEqual([
      { type: "quantize", take: 7, notes: [] },
      { type: "quantize", take: 7, notes: [0] },
    ]);
  });

  it("Delete: if only points are selected, deletes the points. With both selected, the one pressed last", () => {
    state.pianoRoll = { ...state.pianoRoll, selectedControls: [{ start: 1, end: 2 }, { start: 4, end: 5 }] };
    press("Delete");
    state.pianoRoll = { ...state.pianoRoll, selectedNotes: [{ start: 0, end: 1 }] };
    press("Delete");
    prView.picked = "controls";
    press("Delete");
    expect(sent).toEqual([
      { type: "deleteControls", take: 7, points: [1, 4] },
      { type: "deleteNotes", take: 7, notes: [0] },
      { type: "deleteControls", take: 7, points: [1, 4] },
    ]);
  });
});

describe("fake scene pianoRoll", () => {
  it("the number of CC tags is the same as mock A-PianoRoll (5 pedal, 6 points, 0 points). CC 7 as well", async () => {
    const t = fakeScene("pianoRoll")!.state;
    const transport = createFakeTransport(t);
    const shownTake = decodeTake(await transport.takeNotes(t.pianoRoll.take!));
    const cc = G.ccCounts(shownTake);
    expect([cc.sustain, cc.pitchBend, cc.modulation, cc.others]).toEqual([5, 6, 0, [7]]);
  });

  it("the frame of the position before aligning is only on the 3 notes whose position or length differ from the original", async () => {
    const t = fakeScene("pianoRoll")!.state;
    const transport = createFakeTransport(t);
    const q = decodeTake(await transport.takeNotes(t.pianoRoll.take!));
    const o = decodeTake(await transport.takeNotes(q.origin));
    const moved = Array.from({ length: q.starts.length }, (_, i) => G.movedFrom(q, o, i)).filter((j) => j >= 0);
    expect(moved).toEqual([14, 15, 16]);
  });

  it("the fake answers SelectControls with the selected points", async () => {
    const t = fakeScene("pianoRoll")!.state;
    const transport = createFakeTransport(t);
    await transport.send({ type: "selectControls", take: t.pianoRoll.take!, points: [3, 1, 999], mode: "replace" });
    const snap = await transport.snapshot();
    expect(snap.state.pianoRoll.selectedControls).toEqual([{ start: 1, end: 2 }, { start: 3, end: 4 }]);
  });
});

describe("CC drags: the take of the press, and while recording (UI-ROLL-003, UI-ROLL-008)", () => {
  const none = { shift: false, alt: false, mod: false };
  let sent: Command[];
  let tool: Tool;
  let busy: boolean;
  let now: G.Shown | null;
  const make = () =>
    new LaneMouse({
      shown: () => now,
      control: () => PB,
      geom: () => GEOM,
      box: () => LANE,
      step: () => TPQ / 4,
      meter: () => ({ num: 4, den: 4 }),
      tool: () => tool,
      selected: () => [],
      reachPx: () => 4,
      stillPx: () => 4,
      send: (c) => void sent.push(c),
      busy: () => busy,
    });
  beforeEach(() => {
    sent = [];
    tool = "select";
    busy = false;
    now = shown();
  });

  it("release sends the id of the take the points were read from, even if another take is shown by then", () => {
    const m = make();
    m.down(20, 24, none);
    m.move(40, 24, none);
    now = G.shownOf(withControls(8), 0);
    expect(m.up()).toEqual({ type: "moveControls", take: 7, points: [1], ticks: TPQ / 2, value: 0 });
  });

  it("a held drag is dropped when another take is shown; not while the same take is shown", () => {
    const m = make();
    m.down(20, 24, none);
    m.move(40, 24, none);
    expect(m.dropIfTakeChanged()).toBe(false);
    now = G.shownOf(withControls(8), 0);
    expect(m.dropIfTakeChanged()).toBe(true);
    expect(m.up()).toBeNull();
  });

  it("while recording or counting in, a press on a point only selects it. Drawing and erasing start nothing. A box still selects", () => {
    busy = true;
    const m = make();
    m.down(20, 24, none);
    expect(sent).toEqual([{ type: "selectControls", take: 7, points: [1], mode: "replace" }]);
    expect(m.drag).toBeNull();
    tool = "draw";
    m.down(60, 24, none);
    expect(m.drag).toBeNull();
    tool = "erase";
    m.down(20, 24, none);
    expect(m.drag).toBeNull();
    tool = "select";
    m.down(5, 1, none);
    m.move(100, 47, none);
    expect(m.up()).toEqual({ type: "selectControls", take: 7, points: [1, 4], mode: "replace" });
  });
});
