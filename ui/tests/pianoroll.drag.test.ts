// ピアノロールのマウスの決まり（drag.ts）: 吸いつき、Shift、範囲で止まる、Esc で戻る、離して 1 つだけ送る、読み値の文。
import { beforeEach, describe, expect, it } from "vitest";
import type { Command, IndexRange, MusicalScale } from "../src/bridge";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import type { Tool } from "../src/components/pianoroll/view.svelte";
import { GEOM, take } from "./pianoroll-fixture";

const NONE: Mods = { shift: false, alt: false, mod: false };
const SHIFT: Mods = { ...NONE, shift: true };
const ALT: Mods = { ...NONE, alt: true };

// 1 拍 40 px。ノート 0: 1.1 から 1 拍の C4、ノート 1: 1.2 から 1 拍の D4、ノート 2: 1.3 から半拍の E4。
const n = G.shownOf(
  take([
    [0, 480, 60, 80],
    [480, 480, 62, 100],
    [960, 240, 64, 120],
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

beforeEach(() => {
  sent = [];
  tool = "select";
  selected = [];
  scale = { root: 0, mode: "major", lock: false };
  drag = new RollDrag({
    shown: () => n,
    geom: () => GEOM,
    step: () => 120,
    scale: () => scale,
    meter: () => ({ num: 4, den: 4 }),
    tool: () => tool,
    selected: () => selected,
    edgePx: () => 6,
    stillPx: () => 4,
    velocityFullPx: () => 64,
    velocityHeight: () => 71,
    send: (c) => void sent.push(c),
  });
});

describe("選ぶ道具", () => {
  it("押すと選ぶ。押したまま動かすと、離したときに MoveNotes を 1 つだけ送る（グリッドに吸いつく）", () => {
    drag.down(x(100), y(60), NONE);
    expect(sent).toEqual([{ type: "selectNotes", take: 7, notes: [0], mode: "replace" }]);
    sent.length = 0;
    for (let dx = 1; dx <= 13; dx++) drag.move(x(100) + dx, y(60) - 12, NONE);
    expect(sent).toEqual([]);
    expect(drag.reading()).toBe("+0.120 ・ +1 半音");
    const c = drag.up();
    expect(c).toEqual({ type: "moveNotes", take: 7, notes: [0], ticks: 120, keys: 1, copy: false });
    expect(sent).toEqual([c]);
    expect(drag.drag).toBeNull();
  });

  it("Shift で吸いつかない", () => {
    drag.down(x(100), y(60), NONE);
    drag.move(x(100) + 5, y(60), SHIFT);
    expect(drag.up()).toMatchObject({ type: "moveNotes", ticks: 60, keys: 0 });
  });

  it("選んだノートを押して動かすと、選んだぜんぶが動く。動かさずに離すと、そのノートだけを選ぶ", () => {
    selected = [{ start: 0, end: 2 }];
    drag.down(x(500), y(62), NONE);
    expect(sent).toEqual([]);
    drag.move(x(500) + 20, y(62), NONE);
    expect(drag.up()).toMatchObject({ type: "moveNotes", notes: [0, 1], ticks: 240 });
    sent.length = 0;
    drag.down(x(500), y(62), NONE);
    expect(drag.up()).toEqual({ type: "selectNotes", take: 7, notes: [1], mode: "replace" });
  });

  it("曲の頭より前と、鍵の外へは出ない（範囲で止まる）", () => {
    drag.down(x(100), y(60), NONE);
    drag.move(x(100) - 500, y(60) + 12 * 100, NONE);
    expect(drag.up()).toMatchObject({ type: "moveNotes", ticks: 0, keys: -60 });
    // 動かしても、もとの所に戻して離したら送らない
    sent.length = 0;
    drag.down(x(100), y(60), NONE);
    drag.move(x(100) + 30, y(60), NONE);
    drag.move(x(100), y(60), NONE);
    expect(drag.up()).toBeNull();
    expect(sent.filter((c) => c.type === "moveNotes")).toEqual([]);
  });

  it("Alt ＋ ドラッグで複製", () => {
    drag.down(x(100), y(60), ALT);
    drag.move(x(100) + 40, y(60), ALT);
    expect(drag.cursor()).toBe("copy");
    expect(drag.up()).toMatchObject({ type: "moveNotes", ticks: 480, copy: true });
  });

  it("Esc（cancel）でドラッグの前に戻り、何も送らない", () => {
    drag.down(x(100), y(60), NONE);
    sent.length = 0;
    drag.move(x(100) + 40, y(60), NONE);
    expect(drag.cancel()).toBe(true);
    expect(drag.drag).toBeNull();
    expect(drag.up()).toBeNull();
    expect(sent).toEqual([]);
    expect(drag.cancel()).toBe(false);
  });

  it("右はし 6px は長さ（どの道具でも）。読み値は「長さ 1.000 → 1.240」", () => {
    for (const t of ["select", "draw", "erase"] as Tool[]) {
      tool = t;
      sent.length = 0;
      expect(drag.hover(x(478), y(60), NONE).cursor).toBe("ew-resize");
      drag.down(x(478), y(60), NONE);
      drag.move(x(478) + 20, y(60), NONE);
      expect(drag.reading()).toBe("長さ 1.000 → 1.240");
      expect(drag.up()).toEqual({ type: "resizeNotes", take: 7, notes: [0], ticks: 240 });
    }
  });

  it("空いている所から囲んでえらぶ（Shift で足す）。押しただけなら選択を外す", () => {
    drag.down(x(10), y(66), NONE);
    drag.move(x(1500), y(59), NONE);
    expect(drag.reading()).toBe("3 ノート");
    expect(drag.up()).toEqual({ type: "selectNotes", take: 7, notes: [0, 1, 2], mode: "replace" });
    drag.down(x(10), y(66), SHIFT);
    drag.move(x(500), y(59), SHIFT);
    expect(drag.up()).toEqual({ type: "selectNotes", take: 7, notes: [0, 1], mode: "add" });
    drag.down(x(2000), y(66), NONE);
    expect(drag.up()).toEqual({ type: "clearNoteSelection" });
  });

  it("Mod ＋ 押す は切りかえ（ドラッグにならない）", () => {
    drag.down(x(100), y(60), { ...NONE, mod: true });
    expect(sent).toEqual([{ type: "selectNotes", take: 7, notes: [0], mode: "toggle" }]);
    expect(drag.drag).toBeNull();
  });
});

describe("書く道具", () => {
  beforeEach(() => (tool = "draw"));

  it("押した瞬間に Audition、離して AuditionOff と AddNote（グリッド 1 つぶん）", () => {
    drag.down(x(1500), y(50), NONE);
    expect(sent).toEqual([{ type: "audition", key: 50, velocity: G.DRAW_VELOCITY }]);
    expect(drag.reading()).toBe("D3 ・ 1.4.000 ・ 長さ 0.120");
    drag.up();
    expect(sent.slice(1)).toEqual([
      { type: "auditionOff" },
      { type: "addNote", take: 7, start: 1440, length: 120, key: 50, velocity: G.DRAW_VELOCITY },
    ]);
  });

  it("押したまま右へ動かすと長さがのびる。Esc では書かずに音を止める", () => {
    drag.down(x(1500), y(50), NONE);
    drag.move(x(1500) + 30, y(50), NONE);
    expect(drag.reading()).toBe("D3 ・ 1.4.000 ・ 長さ 1.000");
    expect(drag.cancel()).toBe(true);
    expect(sent.at(-1)).toEqual({ type: "auditionOff" });
    expect(sent.some((c) => c.type === "addNote")).toBe(false);
  });

  it("ノートの上では選ぶ道具と同じ", () => {
    drag.down(x(100), y(60), NONE);
    expect(sent).toEqual([{ type: "selectNotes", take: 7, notes: [0], mode: "replace" }]);
    expect(drag.drag?.kind).toBe("move");
  });

  it("スケール固定で音階の外の行は「できない」のカーソル。押すと書かずに Rust へ（状態行で言う）", () => {
    scale = { root: 0, mode: "major", lock: true };
    expect(drag.hover(x(1500), y(61), NONE).cursor).toBe("not-allowed");
    expect(drag.hover(x(1500), y(62), NONE).cursor).toBe("pencil");
    drag.down(x(1500), y(61), NONE);
    expect(drag.drag).toBeNull();
    expect(sent).toEqual([{ type: "addNote", take: 7, start: 1440, length: 120, key: 61, velocity: G.DRAW_VELOCITY }]);
  });
});

describe("消す道具", () => {
  beforeEach(() => (tool = "erase"));

  it("乗せている間は、消えるノートを知らせる。なぞると通ったノートをぜんぶ、離して 1 つの DeleteNotes", () => {
    expect(drag.hover(x(100), y(60), NONE)).toEqual({ cursor: "eraser", changed: true });
    expect(drag.eraseHover).toBe(0);
    drag.down(x(100), y(60), NONE);
    drag.move(x(600), y(62), NONE);
    drag.move(x(1000), y(64), NONE);
    expect(drag.up()).toEqual({ type: "deleteNotes", take: 7, notes: [0, 1, 2] });
    expect(sent).toHaveLength(1);
  });
});

describe("ベロシティの帯", () => {
  it("棒の上はしを押したまま上下。選んだノートがあればぜんぶ、離して ChangeVelocity 1 つ", () => {
    selected = [{ start: 0, end: 2 }];
    const top = 71 - (100 * 64) / 127;
    drag.downVelocity(x(480), top);
    expect(drag.drag).toMatchObject({ kind: "velocity", notes: [0, 1], grab: 1 });
    drag.move(x(480), top - 6, NONE);
    expect(drag.reading()).toBe("+12（2 ノート）");
    expect(drag.up()).toEqual({ type: "changeVelocity", take: 7, notes: [0, 1], delta: 12 });
  });

  it("選んでいなければ押した 1 本だけ。上はしから離れた所では何もしない", () => {
    drag.downVelocity(x(480), 71 - 10);
    expect(drag.drag).toBeNull();
    const top = 71 - (80 * 64) / 127;
    drag.downVelocity(x(0), top);
    expect(drag.drag).toMatchObject({ kind: "velocity", notes: [0] });
  });
});
