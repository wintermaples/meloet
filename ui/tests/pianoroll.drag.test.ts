// ピアノロールのマウスの決まり（drag.ts）: 吸いつき、Shift、範囲で止まる、Esc で戻る、離して 1 つだけ送る、読み値の文。
import { beforeEach, describe, expect, it } from "vitest";
import type { Command, IndexRange, MusicalScale } from "../src/bridge";
import { Auditioner, type Timer } from "../src/components/pianoroll/audition";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import type { Tool } from "../src/components/pianoroll/view.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { GEOM, take } from "./pianoroll-fixture";

const NONE: Mods = { shift: false, alt: false, mod: false };
const SHIFT: Mods = { ...NONE, shift: true };
const ALT: Mods = { ...NONE, alt: true };

// 1 拍 40 px。ノート 0: 1.1 から 1 拍の C4、ノート 1: 1.2 から 1 拍の D4、ノート 2: 1.3 から半拍の E4。
const n = G.shownOf(
  take([
    [0, TPQ, 60, 80],
    [TPQ, TPQ, 62, 100],
    [2 * TPQ, TPQ / 2, 64, 120],
  ]),
  0,
);
const y = (key: number) => (GEOM.topKey - key) * GEOM.rowH + 5;
const x = (tick: number) => tick * GEOM.ppt;

let sent: Command[];
let tool: Tool;
let selected: IndexRange[];
let scale: MusicalScale;
let drag: RollDrag;
/** ノートを押した ・ 動かしたときに鳴らすか（止まっているとき）。 */
let auditionOn: boolean;
let drawVelocity: number;
/** 時間をはかる係の偽物: 止める時間（ms）を覚えるだけ。fire で止める。 */
let timers: { ms: number; run: () => void }[];
const clock: Timer = {
  set: (run, ms) => timers.push({ ms, run }),
  clear: () => (timers.length = 0),
};
const STOP: Command = { type: "audition", keys: [], velocity: 0 };

beforeEach(() => {
  sent = [];
  tool = "select";
  selected = [];
  scale = { root: 0, mode: "major", lock: false };
  auditionOn = false;
  drawVelocity = G.DRAW_VELOCITY;
  timers = [];
  drag = new RollDrag({
    shown: () => n,
    geom: () => GEOM,
    step: () => TPQ / 4,
    scale: () => scale,
    meter: () => ({ num: 4, den: 4 }),
    tool: () => tool,
    selected: () => selected,
    edgePx: () => 6,
    stillPx: () => 4,
    velocityFullPx: () => 64,
    velocityHeight: () => 71,
    send: (c) => void sent.push(c),
    drawVelocity: () => drawVelocity,
    mayAudition: () => auditionOn,
    ticksToMs: (ticks) => ticks,
    audition: new Auditioner((c) => void sent.push(c), undefined, clock),
  });
});

describe("選ぶ道具", () => {
  it("押すと選ぶ。押したまま動かすと、離したときに MoveNotes を 1 つだけ送る（グリッドに吸いつく）", () => {
    drag.down(x((5 * TPQ) / 24), y(60), NONE);
    expect(sent).toEqual([{ type: "selectNotes", take: 7, notes: [0], mode: "replace" }]);
    sent.length = 0;
    for (let dx = 1; dx <= 13; dx++) drag.move(x((5 * TPQ) / 24) + dx, y(60) - 12, NONE);
    expect(sent).toEqual([]);
    expect(drag.reading()).toBe("+0.0480 ・ +1 半音");
    const c = drag.up();
    expect(c).toEqual({ type: "moveNotes", take: 7, notes: [0], ticks: TPQ / 4, keys: 1, copy: false });
    expect(sent).toEqual([c]);
    expect(drag.drag).toBeNull();
  });

  it("Shift で吸いつかない", () => {
    drag.down(x((5 * TPQ) / 24), y(60), NONE);
    drag.move(x((5 * TPQ) / 24) + 5, y(60), SHIFT);
    expect(drag.up()).toMatchObject({ type: "moveNotes", ticks: TPQ / 8, keys: 0 });
  });

  it("選んだノートを押して動かすと、選んだぜんぶが動く。動かさずに離すと、そのノートだけを選ぶ", () => {
    selected = [{ start: 0, end: 2 }];
    drag.down(x((7 * TPQ) / 6), y(62), NONE);
    expect(sent).toEqual([]);
    drag.move(x((7 * TPQ) / 6) + 20, y(62), NONE);
    expect(drag.up()).toMatchObject({ type: "moveNotes", notes: [0, 1], ticks: TPQ / 2 });
    sent.length = 0;
    drag.down(x((7 * TPQ) / 6), y(62), NONE);
    expect(drag.up()).toEqual({ type: "selectNotes", take: 7, notes: [1], mode: "replace" });
  });

  it("曲の頭より前と、鍵の外へは出ない（範囲で止まる）", () => {
    drag.down(x((5 * TPQ) / 24), y(60), NONE);
    drag.move(x((5 * TPQ) / 24) - 500, y(60) + 12 * 100, NONE);
    expect(drag.up()).toMatchObject({ type: "moveNotes", ticks: 0, keys: -60 });
    // 動かしても、もとの所に戻して離したら送らない
    sent.length = 0;
    drag.down(x((5 * TPQ) / 24), y(60), NONE);
    drag.move(x((5 * TPQ) / 24) + 30, y(60), NONE);
    drag.move(x((5 * TPQ) / 24), y(60), NONE);
    expect(drag.up()).toBeNull();
    expect(sent.filter((c) => c.type === "moveNotes")).toEqual([]);
  });

  it("Alt ＋ ドラッグで複製", () => {
    drag.down(x((5 * TPQ) / 24), y(60), ALT);
    drag.move(x((5 * TPQ) / 24) + 40, y(60), ALT);
    expect(drag.cursor()).toBe("copy");
    expect(drag.up()).toMatchObject({ type: "moveNotes", ticks: TPQ, copy: true });
  });

  it("Esc（cancel）でドラッグの前に戻り、何も送らない", () => {
    drag.down(x((5 * TPQ) / 24), y(60), NONE);
    sent.length = 0;
    drag.move(x((5 * TPQ) / 24) + 40, y(60), NONE);
    expect(drag.cancel()).toBe(true);
    expect(drag.drag).toBeNull();
    expect(drag.up()).toBeNull();
    expect(sent).toEqual([]);
    expect(drag.cancel()).toBe(false);
  });

  it("右はし 6px は長さ（どの道具でも）。読み値は「長さ 1.0000 → 1.0960」", () => {
    for (const t of ["select", "draw", "erase"] as Tool[]) {
      tool = t;
      sent.length = 0;
      expect(drag.hover(x((239 * TPQ) / 240), y(60), NONE).cursor).toBe("ew-resize");
      drag.down(x((239 * TPQ) / 240), y(60), NONE);
      drag.move(x((239 * TPQ) / 240) + 20, y(60), NONE);
      expect(drag.reading()).toBe("長さ 1.0000 → 1.0960");
      expect(drag.up()).toEqual({ type: "resizeNotes", take: 7, notes: [0], edge: "end", ticks: TPQ / 2 });
    }
  });

  it("空いている所から囲んでえらぶ（Shift で足す）。押しただけなら選択を外す", () => {
    drag.down(x(TPQ / 48), y(66), NONE);
    drag.move(x((25 * TPQ) / 8), y(59), NONE);
    expect(drag.reading()).toBe("3 ノート");
    expect(drag.up()).toEqual({ type: "selectNotes", take: 7, notes: [0, 1, 2], mode: "replace" });
    drag.down(x(TPQ / 48), y(66), SHIFT);
    drag.move(x((25 * TPQ) / 24), y(59), SHIFT);
    expect(drag.up()).toEqual({ type: "selectNotes", take: 7, notes: [0, 1], mode: "add" });
    drag.down(x((25 * TPQ) / 6), y(66), NONE);
    expect(drag.up()).toEqual({ type: "clearNoteSelection" });
  });

  it("Mod ＋ 押す は切りかえ（ドラッグにならない）", () => {
    drag.down(x((5 * TPQ) / 24), y(60), { ...NONE, mod: true });
    expect(sent).toEqual([{ type: "selectNotes", take: 7, notes: [0], mode: "toggle" }]);
    expect(drag.drag).toBeNull();
  });
});

describe("書く道具", () => {
  beforeEach(() => (tool = "draw"));

  it("押した瞬間に Audition、離して音を止めて AddNote（グリッド 1 つぶん）", () => {
    drag.down(x((25 * TPQ) / 8), y(50), NONE);
    expect(sent).toEqual([{ type: "audition", keys: [50], velocity: G.DRAW_VELOCITY }]);
    expect(drag.reading()).toBe("D3 ・ 1.4.0000 ・ 長さ 0.0480");
    drag.up();
    expect(sent.slice(1)).toEqual([
      STOP,
      { type: "addNote", take: 7, start: 3 * TPQ, length: TPQ / 4, key: 50, velocity: G.DRAW_VELOCITY },
    ]);
  });

  it("押したまま右へ動かすと長さがのびる。Esc では書かずに音を止める", () => {
    drag.down(x((25 * TPQ) / 8), y(50), NONE);
    drag.move(x((25 * TPQ) / 8) + 30, y(50), NONE);
    expect(drag.reading()).toBe("D3 ・ 1.4.0000 ・ 長さ 1.0000");
    expect(drag.cancel()).toBe(true);
    expect(sent.at(-1)).toEqual(STOP);
    expect(sent.some((c) => c.type === "addNote")).toBe(false);
  });

  it("ノートの上では選ぶ道具と同じ", () => {
    drag.down(x((5 * TPQ) / 24), y(60), NONE);
    expect(sent).toEqual([{ type: "selectNotes", take: 7, notes: [0], mode: "replace" }]);
    expect(drag.drag?.kind).toBe("move");
  });

  it("スケール固定で音階の外の行は「できない」のカーソル。押すと書かずに Rust へ（状態行で言う）", () => {
    scale = { root: 0, mode: "major", lock: true };
    expect(drag.hover(x((25 * TPQ) / 8), y(61), NONE).cursor).toBe("not-allowed");
    expect(drag.hover(x((25 * TPQ) / 8), y(62), NONE).cursor).toBe("pencil");
    drag.down(x((25 * TPQ) / 8), y(61), NONE);
    expect(drag.drag).toBeNull();
    expect(sent).toEqual([{ type: "addNote", take: 7, start: 3 * TPQ, length: TPQ / 4, key: 61, velocity: G.DRAW_VELOCITY }]);
  });
});

describe("消す道具", () => {
  beforeEach(() => (tool = "erase"));

  it("乗せている間は、消えるノートを知らせる。なぞると通ったノートをぜんぶ、離して 1 つの DeleteNotes", () => {
    expect(drag.hover(x((5 * TPQ) / 24), y(60), NONE)).toEqual({ cursor: "eraser", changed: true });
    expect(drag.eraseHover).toBe(0);
    drag.down(x((5 * TPQ) / 24), y(60), NONE);
    drag.move(x((5 * TPQ) / 4), y(62), NONE);
    drag.move(x((25 * TPQ) / 12), y(64), NONE);
    expect(drag.up()).toEqual({ type: "deleteNotes", take: 7, notes: [0, 1, 2] });
    expect(sent).toHaveLength(1);
  });
});

describe("ベロシティの帯", () => {
  it("棒の上はしを押したまま上下。選んだノートがあればぜんぶ、離して ChangeVelocity 1 つ", () => {
    selected = [{ start: 0, end: 2 }];
    const top = 71 - (100 * 64) / 127;
    drag.downVelocity(x(TPQ), top);
    expect(drag.drag).toMatchObject({ kind: "velocity", notes: [0, 1], grab: 1 });
    drag.move(x(TPQ), top - 6, NONE);
    expect(drag.reading()).toBe("+12（2 ノート）");
    expect(drag.up()).toEqual({ type: "changeVelocity", take: 7, notes: [0, 1], delta: 12 });
  });

  it("選んでいなければ押した 1 本だけ。上はしから離れた所では何もしない", () => {
    drag.downVelocity(x(TPQ), 71 - 10);
    expect(drag.drag).toBeNull();
    const top = 71 - (80 * 64) / 127;
    drag.downVelocity(x(0), top);
    expect(drag.drag).toMatchObject({ kind: "velocity", notes: [0] });
  });
});
