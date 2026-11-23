// DOM-free rules for docs/history/design-notes.ja.md B-60, 62, 63, 66, 68, 69:
// vertical zoom row counts and steps, the left edge of notes, playing or not, rounding out-of-range values, the context menu items.
import { beforeEach, describe, expect, it } from "vitest";
import type { AppState, Command, IndexRange } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import {
  Auditioner,
  chordOf,
  fixedOr100,
  MAX_CHORD,
  NUDGE_MS,
  notesMayAudition,
  onNudgeKeys,
  ticksToMs,
  type Timer,
} from "../src/components/pianoroll/audition";
import { pianoRollCommand } from "../src/components/pianoroll/commands";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import { noteFieldSpec, readField, clampCommand } from "../src/components/pianoroll/noteFields";
import { noteMarks } from "../src/components/pianoroll/paint";
import { rollMenu, type RollMenuFacts } from "../src/components/pianoroll/rollMenu";
import { DEFAULT_ROW_PX, prView, ROW_PX_STEPS, rowHeight, zoomRowsBy } from "../src/components/pianoroll/view.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const NONE: Mods = { shift: false, alt: false, mod: false };
const SHIFT: Mods = { ...NONE, shift: true };
const M44 = { num: 4, den: 4 };
const BAR = 4 * TPQ;
const at = (bar: number, beat: number, tick: number) => (bar - 1) * BAR + (beat - 1) * TPQ + tick;

describe("vertical zoom (B-63)", () => {
  beforeEach(() => {
    prView.rowPx = DEFAULT_ROW_PX;
    prView.topKey = 84;
    prView.areaHeight = 480;
  });

  it("5 steps: 8, 12, 16, 20, 24. Stops at the ends. Row height stays an integer after the scale", () => {
    expect([...ROW_PX_STEPS]).toEqual([8, 12, 16, 20, 24]);
    expect(zoomRowsBy(1)).toBe(true);
    expect(prView.rowPx).toBe(16);
    expect(zoomRowsBy(5)).toBe(true);
    expect(prView.rowPx).toBe(24);
    expect(zoomRowsBy(1)).toBe(false);
    expect(zoomRowsBy(-10)).toBe(true);
    expect(prView.rowPx).toBe(8);
    expect(zoomRowsBy(-1)).toBe(false);
    for (const px of ROW_PX_STEPS) {
      prView.rowPx = px;
      for (const scale of [100, 125, 150]) expect(Number.isInteger(rowHeight(scale))).toBe(true);
    }
  });

  it("visible rows are height ÷ row (480 px: 60 rows at 8, 40 at 12, 20 at 24)", () => {
    expect([8, 12, 24].map((px) => G.wholeRows(480, px))).toEqual([60, 40, 20]);
  });

  it("the middle row does not move (the middle of 480 px is row 20 at 12. At 24 the top row goes down by 10 rows)", () => {
    // 12 px: the middle (240 px) is topKey − 20. At 24 px, the same key is in the middle.
    zoomRowsBy(3);
    expect(prView.rowPx).toBe(24);
    expect(prView.topKey).toBe(84 - 20 + 10);
    zoomRowsBy(-3);
    expect(prView.topKey).toBe(84);
  });

  it("the row under the mouse (120 px from the top) does not move", () => {
    zoomRowsBy(-1, 120);
    expect(prView.rowPx).toBe(8);
    // Key 74 in row 10 at 12 px is also at 120 px from the top (row 15) at 8 px.
    expect(prView.topKey - 120 / 8).toBe(84 - 120 / 12);
  });

  it("never sends above the top (G9)", () => {
    prView.topKey = 127;
    zoomRowsBy(-1, 0);
    expect(prView.topKey).toBe(127);
    zoomRowsBy(1, 480);
    expect(prView.topKey).toBeLessThanOrEqual(127);
  });
});

describe("left edge of notes (B-69)", () => {
  // 1 beat is 40 px. Note 0: C4 from 1.1 for 1 beat, note 1: D4 from 1.2 for 1 beat, note 2: E4 from 1.3 for 1/16 (TPQ / 4 ticks, 10 px).
  const n = G.shownOf(
    take([
      [0, TPQ, 60, 80],
      [TPQ, TPQ, 62, 100],
      [2 * TPQ, TPQ / 4, 64, 120],
    ]),
    0,
  );
  const y = (key: number) => (GEOM.topKey - key) * GEOM.rowH + 5;
  const x = (tick: number) => tick * GEOM.ppt;

  it("left 6px is the start, right 6px is the end, the middle moves", () => {
    expect(G.hitNote(n, GEOM, x(0) + 2, y(60), 6, true)).toEqual({ note: 0, edge: true, start: true });
    expect(G.hitNote(n, GEOM, x(TPQ) - 2, y(60), 6, true)).toEqual({ note: 0, edge: true });
    expect(G.hitNote(n, GEOM, x(TPQ / 2), y(60), 6, true)).toEqual({ note: 0, edge: false });
    // When the left edge is not used (the picture inside an arrange clip and so on), it works as before.
    expect(G.hitNote(n, GEOM, x(0) + 2, y(60), 6)).toEqual({ note: 0, edge: false });
  });

  it("for a short note (10 px) where the left and right 6px overlap, each side is 1/3 of the width", () => {
    expect(G.edgeWidth(10, 6, true)).toBeCloseTo(10 / 3);
    expect(G.edgeWidth(40, 6, true)).toBe(6);
    const x0 = x(2 * TPQ);
    expect(G.hitNote(n, GEOM, x0 + 3, y(64), 6, true)).toEqual({ note: 2, edge: true, start: true });
    expect(G.hitNote(n, GEOM, x0 + 5, y(64), 6, true)).toEqual({ note: 2, edge: false });
    expect(G.hitNote(n, GEOM, x0 + 7, y(64), 6, true)).toEqual({ note: 2, edge: true });
  });

  it("the start snaps to the grid, not with Shift. The end does not move (+ makes it shorter)", () => {
    expect(G.resizeStartDelta(n, [1], 1, (5 * TPQ) / 24, TPQ / 4)).toBe(TPQ / 4);
    expect(G.resizeStartDelta(n, [1], 1, -(5 * TPQ) / 24, TPQ / 4)).toBe(-TPQ / 4);
    expect(G.resizeStartDelta(n, [1], 1, 7, null)).toBe(7);
  });

  it("never shorter than one grid step (1 tick with Shift). Never before the start of the song. All selected notes by the same amount", () => {
    expect(G.resizeStartDelta(n, [1], 1, (25 * TPQ) / 12, TPQ / 4)).toBe((3 * TPQ) / 4);
    expect(G.resizeStartDelta(n, [1, 2], 1, (25 * TPQ) / 12, null)).toBe(TPQ / 4 - 1);
    expect(G.resizeStartDelta(n, [0, 1], 1, -(25 * TPQ) / 12, TPQ / 4)).toBe(0);
    expect(G.resizeStartDelta(n, [1], 1, -(25 * TPQ) / 12, TPQ / 4)).toBe(-TPQ);
  });

  describe("drag", () => {
    let sent: Command[];
    let selected: IndexRange[];
    let drag: RollDrag;
    beforeEach(() => {
      sent = [];
      selected = [];
      drag = new RollDrag({
        shown: () => n,
        geom: () => GEOM,
        step: () => TPQ / 4,
        scale: () => ({ root: 0, mode: "major", lock: false }),
        meter: () => M44,
        tool: () => "select",
        selected: () => selected,
        edgePx: () => 6,
        stillPx: () => 4,
        velocityFullPx: () => 64,
        velocityHeight: () => 71,
        send: (c) => void sent.push(c),
        drawVelocity: () => 100,
        mayAudition: () => false,
        ticksToMs: (t) => t,
        audition: { play: () => {}, stop: () => {} },
      });
    });

    it("the cursor is the left-right shape. Release sends one ResizeNotes (edge: start). The readout is the length", () => {
      expect(drag.hover(x(TPQ) + 2, y(62), NONE).cursor).toBe("ew-resize");
      drag.down(x(TPQ) + 2, y(62), NONE);
      expect(sent).toEqual([{ type: "selectNotes", take: 7, notes: [1], mode: "replace" }]);
      drag.move(x(TPQ) + 2 + 10, y(62), NONE);
      expect(drag.reading()).toBe("長さ 1.0000 → 0.1440");
      expect(noteMarks(3, drag.drag, -1)).toEqual(Uint8Array.from([0, 5, 0]));
      expect(drag.up()).toEqual({ type: "resizeNotes", take: 7, notes: [1], edge: "start", ticks: TPQ / 4 });
    });

    it("with selected notes, all by the same amount. No snapping with Shift. Esc puts it back", () => {
      selected = [{ start: 1, end: 3 }];
      drag.down(x(TPQ) + 2, y(62), SHIFT);
      drag.move(x(TPQ) + 2 - 3, y(62), SHIFT);
      drag.move(x(TPQ) + 2 - 5, y(62), SHIFT);
      expect(drag.up()).toEqual({ type: "resizeNotes", take: 7, notes: [1, 2], edge: "start", ticks: -TPQ / 8 });
      sent.length = 0;
      drag.down(x(TPQ) + 2, y(62), NONE);
      drag.move(x(TPQ) + 40, y(62), NONE);
      expect(drag.cancel()).toBe(true);
      expect(drag.up()).toBeNull();
      expect(sent).toEqual([]);
    });
  });
});

describe("playing or not (B-62, 66)", () => {
  let sent: Command[];
  let timers: { ms: number; run: () => void }[];
  const clock: Timer = { set: (run, ms) => timers.push({ ms, run }), clear: () => (timers.length = 0) };
  let lit: number[];
  let audition: Auditioner;
  beforeEach(() => {
    sent = [];
    timers = [];
    lit = [];
    audition = new Auditioner((c) => void sent.push(c), (k) => (lit = [...k]), clock);
  });

  it("strength from the key column and the draw tool: 100, or the fixed velocity when it is on", () => {
    const s = fakeScene("pianoRoll")!.state;
    expect(fixedOr100(s)).toBe(100);
    s.keyboards.velocityFixed = { on: true, value: 64 };
    expect(fixedOr100(s)).toBe(64);
    s.keyboards.velocityFixed = { on: false, value: 64 };
    expect(fixedOr100(s)).toBe(100);
  });

  it("notes play only when stopped (not while playing, recording or during count-in)", () => {
    const s = fakeScene("pianoRoll")!.state;
    const at = (state: AppState["transport"]["state"]) => notesMayAudition({ transport: { ...s.transport, state } });
    expect(at("stopped")).toBe(true);
    expect([at("playing"), at("recording"), at("countIn")]).toEqual([false, false, false]);
  });

  it("playing lights it up; after the time or on stop, an empty audition stops it and the light goes out", () => {
    audition.play([60], 90, 500);
    expect(sent).toEqual([{ type: "audition", keys: [60], velocity: 90 }]);
    expect(lit).toEqual([60]);
    expect(timers.map((t) => t.ms)).toEqual([500]);
    timers[0]!.run();
    expect(sent.at(-1)).toEqual({ type: "audition", keys: [], velocity: 0 });
    expect(lit).toEqual([]);
    // Once stopped, nothing more is sent.
    audition.stop();
    expect(sent).toHaveLength(2);
  });

  it("swapping drops the old timer (Rust stops the old sound)", () => {
    audition.play([60], 90, 500);
    audition.play([62], 90);
    expect(timers).toEqual([]);
    expect(sent).toEqual([
      { type: "audition", keys: [60], velocity: 90 },
      { type: "audition", keys: [62], velocity: 90 },
    ]);
  });

  it("chords are within 0 to 127, no duplicates, up to 16. The time of a note length comes from the tempo", () => {
    expect(chordOf([60, 64, 67, 60], 2)).toEqual([62, 66, 69]);
    expect(chordOf([120, 127, 0], 5)).toEqual([5, 125]);
    expect(chordOf(Array.from({ length: 30 }, (_, i) => i), 0)).toHaveLength(MAX_CHORD);
    expect(ticksToMs(TPQ, 120)).toBe(500);
    expect(ticksToMs(2 * TPQ, 60)).toBe(2000);
  });

  describe("pressing and moving notes", () => {
    const n = G.shownOf(
      take([
        [0, TPQ, 60, 80],
        [TPQ, 2 * TPQ, 62, 100],
      ]),
      0,
    );
    const y = (key: number) => (GEOM.topKey - key) * GEOM.rowH + 5;
    const x = (tick: number) => tick * GEOM.ppt;
    let may: boolean;
    let selected: IndexRange[];
    let tool: "select" | "draw" | "erase";
    let drag: RollDrag;
    beforeEach(() => {
      may = true;
      selected = [];
      tool = "select";
      drag = new RollDrag({
        shown: () => n,
        geom: () => GEOM,
        step: () => TPQ / 4,
        scale: () => ({ root: 0, mode: "major", lock: false }),
        meter: () => M44,
        tool: () => tool,
        selected: () => selected,
        edgePx: () => 6,
        stillPx: () => 4,
        velocityFullPx: () => 64,
        velocityHeight: () => 71,
        send: (c) => void sent.push(c),
        drawVelocity: () => 64,
        mayAudition: () => may,
        ticksToMs: (t) => t * 2,
        audition,
      });
    });

    it("pressing plays at the note's pitch and velocity for the note's length. Release stops it", () => {
      drag.down(x((35 * TPQ) / 24), y(62), NONE);
      expect(sent).toEqual([
        { type: "selectNotes", take: 7, notes: [1], mode: "replace" },
        { type: "audition", keys: [62], velocity: 100 },
      ]);
      expect(timers.map((t) => t.ms)).toEqual([2 * 2 * TPQ]);
      drag.up();
      expect(sent.at(-1)).toEqual({ type: "audition", keys: [], velocity: 0 });
    });

    it("each time the pitch changes while dragging, the moved notes play as a chord (moving only sideways does not play again)", () => {
      selected = [{ start: 0, end: 2 }];
      drag.down(x((35 * TPQ) / 24), y(62), NONE);
      sent.length = 0;
      drag.move(x((35 * TPQ) / 24) + 40, y(62), NONE);
      expect(sent).toEqual([]);
      drag.move(x((35 * TPQ) / 24) + 40, y(62) - 12, NONE);
      expect(sent).toEqual([{ type: "audition", keys: [61, 63], velocity: 100 }]);
      drag.move(x((35 * TPQ) / 24) + 40, y(62) - 24, NONE);
      expect(sent.at(-1)).toEqual({ type: "audition", keys: [62, 64], velocity: 100 });
    });

    it("not played: when not stopped, on box select, on adding with Mod", () => {
      may = false;
      drag.down(x((35 * TPQ) / 24), y(62), NONE);
      drag.move(x((35 * TPQ) / 24), y(62) - 12, NONE);
      drag.up();
      expect(sent.filter((c) => c.type === "audition")).toEqual([]);
      may = true;
      drag.down(x((125 * TPQ) / 12), y(50), NONE);
      drag.move(x(0), y(70), NONE);
      drag.up();
      drag.down(x((35 * TPQ) / 24), y(62), { ...NONE, mod: true });
      drag.up();
      expect(sent.filter((c) => c.type === "audition")).toEqual([]);
    });

    it("draw tool: draws with the fixed velocity and auditions at that value", () => {
      tool = "draw";
      drag.down(x((25 * TPQ) / 4), y(50), NONE);
      expect(sent).toEqual([{ type: "audition", keys: [50], velocity: 64 }]);
      drag.up();
      expect(sent.at(-1)).toMatchObject({ type: "addNote", key: 50, velocity: 64 });
    });
  });

  it("intents made by ↑ ↓ (nudge in pianoRollCommand) are told to the piano roll component. ← → are not", () => {
    const s = fakeScene("pianoRoll")!.state;
    const shown = G.shownOf(take([[0, TPQ, 60, 80]], s.pianoRoll.take!), 0);
    s.pianoRoll.selectedNotes = [{ start: 0, end: 1 }];
    const heard: Command[] = [];
    const stop = onNudgeKeys((c) => heard.push(c));
    try {
      const up = pianoRollCommand({ kind: "nudge", steps: 0, keys: 1 }, { state: s, shown, playTick: 0 });
      pianoRollCommand({ kind: "nudge", steps: 1, keys: 0 }, { state: s, shown, playTick: 0 });
      expect(heard).toEqual([up]);
    } finally {
      stop();
    }
    expect(NUDGE_MS).toBe(300);
  });
});

describe("out-of-range values (B-68)", () => {
  const clip = { start: 0, end: 4 * BAR, takeOffset: 0 };
  const notes = take([
    [at(3, 1, 0), (3 * TPQ) / 4, 62, 80],
    [at(3, 1, (3 * TPQ) / 4), (3 * TPQ) / 4, 64, 83],
  ]);
  const spec = (name: "position" | "key" | "length" | "velocity", picked = [0]) => noteFieldSpec(name, notes, clip, picked, M44)!;

  it("velocity: the nearer end, and the velocity range text (screen text, so range is null). Decimals are rounded", () => {
    expect(readField(spec("velocity"), "200")).toEqual({ value: 127, clamped: { field: "velocity", value: "127", range: null } });
    expect(readField(spec("velocity"), "0")).toEqual({ value: 1, clamped: { field: "velocity", value: "1", range: null } });
    expect(readField(spec("velocity"), "-5")).toEqual({ value: 1, clamped: { field: "velocity", value: "1", range: null } });
    expect(readField(spec("velocity"), "64.5")).toEqual({ value: 65, clamped: null });
    expect(readField(spec("velocity"), "200.4")!.value).toBe(127);
    expect(readField(spec("velocity"), "96")).toEqual({ value: 96, clamped: null });
  });

  it("what cannot be read as a number is null (stays 「こわれた」)", () => {
    for (const s of ["あ", "", "1.2.3", "abc"]) expect(readField(spec("velocity"), s), s).toBeNull();
    for (const s of ["あ", "", "3.1.00000", "x.1.0000"]) expect(readField(spec("position"), s), s).toBeNull();
    expect(readField(spec("key"), "H4")).toBeNull();
  });

  it("position: beat 1 to the meter's top number, tick 0 to 1919 (4/4), bar from 1. Each is put in range and only the rounded part is told", () => {
    expect(readField(spec("position"), "2.5.0000")).toEqual({ value: at(2, 4, 0), clamped: { field: "beat", value: "2.4.0000", range: "1〜4" } });
    expect(readField(spec("position"), "2.0.0000")).toEqual({ value: at(2, 1, 0), clamped: { field: "beat", value: "2.1.0000", range: "1〜4" } });
    expect(readField(spec("position"), "2.2.9999")).toEqual({ value: at(2, 2, TPQ - 1), clamped: { field: "tick", value: "2.2.1919", range: "0〜1919" } });
    expect(readField(spec("position"), "0.1.0000")).toEqual({ value: 0, clamped: { field: "bar", value: "1.1.0000", range: null } });
  });

  it("when 2 or more are rounded, only the largest unit (bar -> beat -> tick)", () => {
    expect(readField(spec("position"), "0.9.9999")!.clamped).toEqual({ field: "bar", value: "1.4.1919", range: null });
    expect(readField(spec("position"), "2.9.9999")!.clamped).toEqual({ field: "beat", value: "2.4.1919", range: "1〜4" });
  });

  it("if the position is outside the clip, the clip's range text (told before other rounding)", () => {
    expect(readField(spec("position"), "9.1.0000")).toEqual({
      value: at(4, 4, TPQ / 4),
      clamped: { field: "position", value: "4.4.0480", range: "1.1.0000〜4.4.0480" },
    });
    expect(readField(spec("position"), "9.9.0000")!.clamped!.field).toBe("position");
  });

  it("pitch and length: the nearer end (pitch G9 or C-1; for 2 or more, where the highest does not go over G9)", () => {
    expect(readField(spec("key"), "C10")).toEqual({ value: 127, clamped: { field: "key", value: "G9", range: "C-1〜G9" } });
    expect(readField(spec("key"), "C-2")).toEqual({ value: 0, clamped: { field: "key", value: "C-1", range: "C-1〜G9" } });
    expect(readField(spec("key", [0, 1]), "G9")).toEqual({ value: 125, clamped: { field: "key", value: "F9", range: "C-1〜F9" } });
    expect(readField(spec("length"), "0.0000")).toEqual({ value: 1, clamped: { field: "length", value: "0.0001", range: "0.0001〜8.0000" } });
    expect(readField(spec("length"), "1.2000")).toEqual({ value: TPQ + TPQ - 1, clamped: { field: "tick", value: "1.1919", range: "0〜1919" } });
  });

  it("intent to tell about the rounding", () => {
    expect(clampCommand({ field: "beat", value: "3.4.0000", range: "1〜4" })).toEqual({ type: "clampValue", field: "beat", value: "3.4.0000", range: "1〜4" });
  });
});

describe("context menu (B-60)", () => {
  const facts: RollMenuFacts = { onNote: true, busy: false, clipboard: true, origin: false, pasteAt: 2 * TPQ, mac: false };
  const did: unknown[] = [];
  const act = { key: (k: unknown) => did.push(k), unquantize: () => did.push("unquantize"), pasteAt: (t: number) => did.push(t) };
  const rows = (f: RollMenuFacts) =>
    rollMenu(f, act).map((e) => (e.kind === "separator" ? "／" : `${e.label}${e.key ? ` [${e.key}]` : ""}${e.usable ? "" : " ×"}`));

  it("over a note: cut, copy, paste, duplicate, delete / quantize, back to the original position (only when there is an original take)", () => {
    expect(rows(facts)).toEqual([
      "menu.cut [Ctrl X]",
      "menu.copy [Ctrl C]",
      "menu.paste [Ctrl V]",
      "menu.duplicate [Ctrl D]",
      "menu.delete [Delete]",
      "／",
      "menu.quantize [Q]",
      "menu.unquantize ×",
    ]);
    expect(rows({ ...facts, origin: true }).at(-1)).toBe("menu.unquantize");
    expect(rows({ ...facts, mac: true })[0]).toBe("menu.cut [Cmd X]");
  });

  it("with nothing to paste, paste is disabled. While recording, rows that change the song are disabled", () => {
    expect(rows({ ...facts, clipboard: false })[2]).toBe("menu.paste [Ctrl V] ×");
    expect(rows({ ...facts, busy: true })).toEqual([
      "menu.cut [Ctrl X] ×",
      "menu.copy [Ctrl C]",
      "menu.paste [Ctrl V] ×",
      "menu.duplicate [Ctrl D] ×",
      "menu.delete [Delete] ×",
      "／",
      "menu.quantize [Q] ×",
      "menu.unquantize ×",
    ]);
  });

  it("empty place: paste (at the pressed place snapped to the grid), select all", () => {
    const f = { ...facts, onNote: false };
    expect(rows(f)).toEqual(["menu.paste [Ctrl V]", "menu.selectAll [Ctrl A]"]);
    did.length = 0;
    for (const e of rollMenu(f, act)) if (e.kind === "row") e.run();
    expect(did).toEqual([2 * TPQ, { kind: "selectAll" }]);
  });
});
