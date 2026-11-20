// 第 3 回の続きの区切り1（docs/design-notes.md B-60・62・63・66・68・69）の、DOM を知らない決まり:
// 縦の拡大の行の数と送り、ノートの左はし、鳴らす ・ 鳴らさない、範囲の外の丸め、右クリックの一覧の中身。
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

describe("縦の拡大（B-63）", () => {
  beforeEach(() => {
    prView.rowPx = DEFAULT_ROW_PX;
    prView.topKey = 84;
    prView.areaHeight = 480;
  });

  it("8 ・ 12 ・ 16 ・ 20 ・ 24 の 5 段。はしで止まる。行の高さは拡大率をかけても整数", () => {
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

  it("見える行の数は高さ ÷ 行（480 px: 8 で 60 行、12 で 40 行、24 で 20 行）", () => {
    expect([8, 12, 24].map((px) => G.wholeRows(480, px))).toEqual([60, 40, 20]);
  });

  it("まん中の行が動かない（480 px のまん中は 12 で 20 行目。24 にすると上の行が 10 行ぶん下がる）", () => {
    // 12 px: まん中（240 px）は topKey − 20。24 px にしても、まん中に同じ鍵。
    zoomRowsBy(3);
    expect(prView.rowPx).toBe(24);
    expect(prView.topKey).toBe(84 - 20 + 10);
    zoomRowsBy(-3);
    expect(prView.topKey).toBe(84);
  });

  it("マウスの所（上から 120 px）の行が動かない", () => {
    zoomRowsBy(-1, 120);
    expect(prView.rowPx).toBe(8);
    // 12 px で 10 行目の鍵 74 が、8 px でも上から 120 px（15 行目）に来る。
    expect(prView.topKey - 120 / 8).toBe(84 - 120 / 12);
  });

  it("いちばん上（G9）より上へは送らない", () => {
    prView.topKey = 127;
    zoomRowsBy(-1, 0);
    expect(prView.topKey).toBe(127);
    zoomRowsBy(1, 480);
    expect(prView.topKey).toBeLessThanOrEqual(127);
  });
});

describe("ノートの左はし（B-69）", () => {
  // 1 拍 40 px。ノート 0: 1.1 から 1 拍の C4、ノート 1: 1.2 から 1 拍の D4、ノート 2: 1.3 から 1/16（TPQ / 4 ティック、10 px）の E4。
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

  it("左はし 6px は始まり、右はし 6px は終わり、まん中は動かす", () => {
    expect(G.hitNote(n, GEOM, x(0) + 2, y(60), 6, true)).toEqual({ note: 0, edge: true, start: true });
    expect(G.hitNote(n, GEOM, x(TPQ) - 2, y(60), 6, true)).toEqual({ note: 0, edge: true });
    expect(G.hitNote(n, GEOM, x(TPQ / 2), y(60), 6, true)).toEqual({ note: 0, edge: false });
    // 左はしを見ないとき（アレンジのクリップの中の絵など）は、いままでどおり。
    expect(G.hitNote(n, GEOM, x(0) + 2, y(60), 6)).toEqual({ note: 0, edge: false });
  });

  it("左右の 6px が重なる短いノート（10 px）は、左右それぞれ幅の 1/3", () => {
    expect(G.edgeWidth(10, 6, true)).toBeCloseTo(10 / 3);
    expect(G.edgeWidth(40, 6, true)).toBe(6);
    const x0 = x(2 * TPQ);
    expect(G.hitNote(n, GEOM, x0 + 3, y(64), 6, true)).toEqual({ note: 2, edge: true, start: true });
    expect(G.hitNote(n, GEOM, x0 + 5, y(64), 6, true)).toEqual({ note: 2, edge: false });
    expect(G.hitNote(n, GEOM, x0 + 7, y(64), 6, true)).toEqual({ note: 2, edge: true });
  });

  it("始まりはグリッドに吸いつき、Shift で吸いつかない。終わりは動かない（+ で短くなる）", () => {
    expect(G.resizeStartDelta(n, [1], 1, (5 * TPQ) / 24, TPQ / 4)).toBe(TPQ / 4);
    expect(G.resizeStartDelta(n, [1], 1, -(5 * TPQ) / 24, TPQ / 4)).toBe(-TPQ / 4);
    expect(G.resizeStartDelta(n, [1], 1, 7, null)).toBe(7);
  });

  it("グリッド 1 つより短くならない（Shift では 1 ティック）。曲の頭より前へ出ない。選んだノートはぜんぶ同じだけ", () => {
    expect(G.resizeStartDelta(n, [1], 1, (25 * TPQ) / 12, TPQ / 4)).toBe((3 * TPQ) / 4);
    expect(G.resizeStartDelta(n, [1, 2], 1, (25 * TPQ) / 12, null)).toBe(TPQ / 4 - 1);
    expect(G.resizeStartDelta(n, [0, 1], 1, -(25 * TPQ) / 12, TPQ / 4)).toBe(0);
    expect(G.resizeStartDelta(n, [1], 1, -(25 * TPQ) / 12, TPQ / 4)).toBe(-TPQ);
  });

  describe("ドラッグ", () => {
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

    it("カーソルは左右の形。離したときに ResizeNotes（edge: start）を 1 つ。読み値は長さ", () => {
      expect(drag.hover(x(TPQ) + 2, y(62), NONE).cursor).toBe("ew-resize");
      drag.down(x(TPQ) + 2, y(62), NONE);
      expect(sent).toEqual([{ type: "selectNotes", take: 7, notes: [1], mode: "replace" }]);
      drag.move(x(TPQ) + 2 + 10, y(62), NONE);
      expect(drag.reading()).toBe("長さ 1.000 → 0.360");
      expect(noteMarks(3, drag.drag, -1)).toEqual(Uint8Array.from([0, 5, 0]));
      expect(drag.up()).toEqual({ type: "resizeNotes", take: 7, notes: [1], edge: "start", ticks: TPQ / 4 });
    });

    it("選んだノートがあれば、ぜんぶ同じだけ。Shift で吸いつかない。Esc で戻す", () => {
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

describe("鳴らす ・ 鳴らさない（B-62・66）", () => {
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

  it("鍵盤の列と書く道具の強さ: 100、ベロシティ固定がオンならその値", () => {
    const s = fakeScene("pianoRoll")!.state;
    expect(fixedOr100(s)).toBe(100);
    s.keyboards.velocityFixed = { on: true, value: 64 };
    expect(fixedOr100(s)).toBe(64);
    s.keyboards.velocityFixed = { on: false, value: 64 };
    expect(fixedOr100(s)).toBe(100);
  });

  it("ノートを鳴らすのは止まっているときだけ（再生中、録音中、カウントイン中は鳴らさない）", () => {
    const s = fakeScene("pianoRoll")!.state;
    const at = (state: AppState["transport"]["state"]) => notesMayAudition({ transport: { ...s.transport, state } });
    expect(at("stopped")).toBe(true);
    expect([at("playing"), at("recording"), at("countIn")]).toEqual([false, false, false]);
  });

  it("鳴らすと光り、時間がたつか止めると、空の audition で止めて光りも消す", () => {
    audition.play([60], 90, 500);
    expect(sent).toEqual([{ type: "audition", keys: [60], velocity: 90 }]);
    expect(lit).toEqual([60]);
    expect(timers.map((t) => t.ms)).toEqual([500]);
    timers[0]!.run();
    expect(sent.at(-1)).toEqual({ type: "audition", keys: [], velocity: 0 });
    expect(lit).toEqual([]);
    // 止まっていれば、もう送らない。
    audition.stop();
    expect(sent).toHaveLength(2);
  });

  it("入れかえると前の時間は捨てる（Rust が前の音を止める）", () => {
    audition.play([60], 90, 500);
    audition.play([62], 90);
    expect(timers).toEqual([]);
    expect(sent).toEqual([
      { type: "audition", keys: [60], velocity: 90 },
      { type: "audition", keys: [62], velocity: 90 },
    ]);
  });

  it("和音は 0〜127 の中で、重ならずに 16 個まで。ノートの長さの時間はテンポから", () => {
    expect(chordOf([60, 64, 67, 60], 2)).toEqual([62, 66, 69]);
    expect(chordOf([120, 127, 0], 5)).toEqual([5, 125]);
    expect(chordOf(Array.from({ length: 30 }, (_, i) => i), 0)).toHaveLength(MAX_CHORD);
    expect(ticksToMs(TPQ, 120)).toBe(500);
    expect(ticksToMs(2 * TPQ, 60)).toBe(2000);
  });

  describe("ノートを押す ・ 動かす", () => {
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

    it("押すと、そのノートの高さとベロシティで、ノートの長さの時間だけ。離すと止める", () => {
      drag.down(x((35 * TPQ) / 24), y(62), NONE);
      expect(sent).toEqual([
        { type: "selectNotes", take: 7, notes: [1], mode: "replace" },
        { type: "audition", keys: [62], velocity: 100 },
      ]);
      expect(timers.map((t) => t.ms)).toEqual([2 * 2 * TPQ]);
      drag.up();
      expect(sent.at(-1)).toEqual({ type: "audition", keys: [], velocity: 0 });
    });

    it("ドラッグで高さが変わるたびに、動かしているノートを和音で（横だけなら鳴らしなおさない）", () => {
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

    it("鳴らさない: 止まっていないとき、囲んでえらんだ時、Mod で足した時", () => {
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

    it("書く道具: ベロシティ固定の値で書き、その値で試聴する", () => {
      tool = "draw";
      drag.down(x((25 * TPQ) / 4), y(50), NONE);
      expect(sent).toEqual([{ type: "audition", keys: [50], velocity: 64 }]);
      drag.up();
      expect(sent.at(-1)).toMatchObject({ type: "addNote", key: 50, velocity: 64 });
    });
  });

  it("↑ ↓（pianoRollCommand の nudge）で作った意図を、ピアノロールの部品に知らせる。← → は知らせない", () => {
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

describe("範囲の外の値（B-68）", () => {
  const clip = { start: 0, end: 4 * BAR, takeOffset: 0 };
  const notes = take([
    [at(3, 1, 0), (3 * TPQ) / 4, 62, 80],
    [at(3, 1, (3 * TPQ) / 4), (3 * TPQ) / 4, 64, 83],
  ]);
  const spec = (name: "position" | "key" | "length" | "velocity", picked = [0]) => noteFieldSpec(name, notes, clip, picked, M44)!;

  it("ベロシティ: 近いはしにして、ベロシティの範囲の文（画面の文なので range は null）。小数は四捨五入", () => {
    expect(readField(spec("velocity"), "200")).toEqual({ value: 127, clamped: { field: "velocity", value: "127", range: null } });
    expect(readField(spec("velocity"), "0")).toEqual({ value: 1, clamped: { field: "velocity", value: "1", range: null } });
    expect(readField(spec("velocity"), "-5")).toEqual({ value: 1, clamped: { field: "velocity", value: "1", range: null } });
    expect(readField(spec("velocity"), "64.5")).toEqual({ value: 65, clamped: null });
    expect(readField(spec("velocity"), "200.4")!.value).toBe(127);
    expect(readField(spec("velocity"), "96")).toEqual({ value: 96, clamped: null });
  });

  it("数字として読めない物は null（「こわれた」のまま）", () => {
    for (const s of ["あ", "", "1.2.3", "abc"]) expect(readField(spec("velocity"), s), s).toBeNull();
    for (const s of ["あ", "", "3.1.24", "x.1.000"]) expect(readField(spec("position"), s), s).toBeNull();
    expect(readField(spec("key"), "H4")).toBeNull();
  });

  it("位置: 拍は 1〜拍子の上、ティックは 0〜479、小節は 1 から。それぞれ範囲に入れてから、丸めた所だけ言う", () => {
    expect(readField(spec("position"), "2.5.000")).toEqual({ value: at(2, 4, 0), clamped: { field: "beat", value: "2.4.000", range: "1〜4" } });
    expect(readField(spec("position"), "2.0.000")).toEqual({ value: at(2, 1, 0), clamped: { field: "beat", value: "2.1.000", range: "1〜4" } });
    expect(readField(spec("position"), "2.2.999")).toEqual({ value: at(2, 2, 479), clamped: { field: "tick", value: "2.2.479", range: "0〜479" } });
    expect(readField(spec("position"), "0.1.000")).toEqual({ value: 0, clamped: { field: "bar", value: "1.1.000", range: null } });
  });

  it("2 つ以上丸めたら、いちばん大きい単位だけ（小節 → 拍 → ティック）", () => {
    expect(readField(spec("position"), "0.9.999")!.clamped).toEqual({ field: "bar", value: "1.4.479", range: null });
    expect(readField(spec("position"), "2.9.999")!.clamped).toEqual({ field: "beat", value: "2.4.479", range: "1〜4" });
  });

  it("位置がクリップの外なら、クリップの範囲の文（ほかの丸めより先に言う）", () => {
    expect(readField(spec("position"), "9.1.000")).toEqual({
      value: at(4, 4, TPQ / 4),
      clamped: { field: "position", value: "4.4.120", range: "1.1.000〜4.4.120" },
    });
    expect(readField(spec("position"), "9.9.000")!.clamped!.field).toBe("position");
  });

  it("音 ・ 長さ: 近いはし（音は G9 ・ C-1、2 つ以上なら高い音が G9 をこえない所）", () => {
    expect(readField(spec("key"), "C10")).toEqual({ value: 127, clamped: { field: "key", value: "G9", range: "C-1〜G9" } });
    expect(readField(spec("key"), "C-2")).toEqual({ value: 0, clamped: { field: "key", value: "C-1", range: "C-1〜G9" } });
    expect(readField(spec("key", [0, 1]), "G9")).toEqual({ value: 125, clamped: { field: "key", value: "F9", range: "C-1〜F9" } });
    expect(readField(spec("length"), "0.000")).toEqual({ value: 1, clamped: { field: "length", value: "0.001", range: "0.001〜8.000" } });
    expect(readField(spec("length"), "1.700")).toEqual({ value: 480 + 479, clamped: { field: "tick", value: "1.479", range: "0〜479" } });
  });

  it("丸めたことを言わせる意図", () => {
    expect(clampCommand({ field: "beat", value: "3.4.000", range: "1〜4" })).toEqual({ type: "clampValue", field: "beat", value: "3.4.000", range: "1〜4" });
  });
});

describe("右クリックの一覧（B-60）", () => {
  const facts: RollMenuFacts = { onNote: true, busy: false, clipboard: true, origin: false, pasteAt: 2 * TPQ, mac: false };
  const did: unknown[] = [];
  const act = { key: (k: unknown) => did.push(k), unquantize: () => did.push("unquantize"), pasteAt: (t: number) => did.push(t) };
  const rows = (f: RollMenuFacts) =>
    rollMenu(f, act).map((e) => (e.kind === "separator" ? "／" : `${e.label}${e.key ? ` [${e.key}]` : ""}${e.usable ? "" : " ×"}`));

  it("ノートの上: 切り取り ・ コピー ・ 貼りつけ ・ 複製 ・ 消す ／ クオンタイズ ・ もとの位置へ戻す（もとのテイクがあるときだけ）", () => {
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

  it("貼る物がなければ貼りつけは使えない。録音中は曲を変える行が使えない", () => {
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

  it("空いた所: 貼りつけ（押した所をグリッドに寄せた位置へ）・ ぜんぶ選ぶ", () => {
    const f = { ...facts, onNote: false };
    expect(rows(f)).toEqual(["menu.paste [Ctrl V]", "menu.selectAll [Ctrl A]"]);
    did.length = 0;
    for (const e of rollMenu(f, act)) if (e.kind === "row") e.run();
    expect(did).toEqual([2 * TPQ, { kind: "selectAll" }]);
  });
});
