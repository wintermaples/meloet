// CC の帯（cc.ts、paint.ts の paintLane）: 値と高さ、点の拾いかた、帯をたたむ決まり、マウス（書く ・ 選ぶ ・ 動かす ・ 囲む ・ 消す ・ Esc）、
// オン ・ オフの吸いつき、読み値、絵（点 6×6、線、ピッチベンドの 0 の線）。Q と、点の Delete。偽物の場面の CC の数と、そろえる前の位置の枠。
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
/** 1 本 48、上下のあき 9（モック R3-Lanes）。 */
const LANE: LaneBox = { height: 48, pad: 9 };

/** サステイン 127 ・ 0（ティック 0 と TPQ）、ピッチベンド 0 ・ 4096（TPQ / 2 と 2 * TPQ）、モジュレーション 64（(3 * TPQ) / 2）。tick の順。 */
const withControls = (id = 7) =>
  take([[0, TPQ, 60]], id, {
    controlTicks: Uint32Array.from([0, TPQ / 2, TPQ, (3 * TPQ) / 2, 2 * TPQ]),
    controlKinds: Uint8Array.from([0, 1, 0, 0, 1]),
    controlNumbers: Uint8Array.from([64, 0, 64, 1, 0]),
    controlValues: Int16Array.from([127, 0, 0, 64, 4096]),
  });
const shown = (base = 0) => G.shownOf(withControls(), base);

describe("値と高さ", () => {
  it("CC は 127 が上のあき 9、0 が下のあき 9。ピッチベンドの 0 はまん中", () => {
    expect(valueToY(MOD, LANE, 127)).toBe(9);
    expect(valueToY(MOD, LANE, 0)).toBe(39);
    expect(valueToY(PB, LANE, 0)).toBe(24);
    expect(valueToY(PB, LANE, -8192)).toBe(39);
    expect(yToValue(MOD, LANE, valueToY(MOD, LANE, 100))).toBe(100);
    expect(yToValue(PB, LANE, valueToY(PB, LANE, 512))).toBe(512);
    expect(yToValue(PB, LANE, -100)).toBe(8191);
  });

  it("オン ・ オフの CC（64〜69）は 0 か 127 にだけ止まる。ほかは範囲で止まる", () => {
    expect(settle(SUSTAIN, 63)).toBe(0);
    expect(settle(SUSTAIN, 64)).toBe(127);
    expect(settle({ kind: "cc", number: 69 }, 100)).toBe(127);
    expect(settle({ kind: "cc", number: 70 }, 100)).toBe(100);
    expect(settle(MOD, 300)).toBe(127);
    expect(yToValue(SUSTAIN, LANE, 20)).toBe(127);
    expect(yToValue(SUSTAIN, LANE, 30)).toBe(0);
  });
});

describe("帯の点", () => {
  it("帯ごとに、その種類の点だけ（tick の順）", () => {
    const t = withControls();
    expect([...lanePoints(t, SUSTAIN)]).toEqual([0, 2]);
    expect([...lanePoints(t, PB)]).toEqual([1, 4]);
    expect([...lanePoints(t, MOD)]).toEqual([3]);
    expect([...lanePoints(t, { kind: "cc", number: 7 })]).toEqual([]);
  });

  it("点に乗る（縦も横も 4 以内）。囲むと、かかった点", () => {
    const n = shown();
    // 1 拍 40 px: TPQ は 40 px。
    expect(hitPoint(n, SUSTAIN, GEOM, LANE, 40, 39, 4)).toBe(2);
    expect(hitPoint(n, SUSTAIN, GEOM, LANE, 45, 39, 4)).toBe(-1);
    expect(hitPoint(n, SUSTAIN, GEOM, LANE, 40, 9, 4)).toBe(-1);
    expect(pointsInBox(n, PB, GEOM, LANE, 0, 0, 100, 48)).toEqual([1, 4]);
    expect(pointsInBox(n, PB, GEOM, LANE, 0, 20, 100, 48)).toEqual([1]);
  });
});

describe("帯の高さ（仕様書 3 章: ノートが 10 行を下まわるなら、古いほうの帯をたたむ）", () => {
  it("古くひらいたほうからたたむ。いちばん新しい帯は残す", () => {
    expect(fitLanes(["pitchBend", "sustain"], 120, 48, 12)).toEqual(["pitchBend", "sustain"]);
    expect(fitLanes(["pitchBend", "sustain"], 119, 48, 12)).toEqual(["sustain"]);
    expect(fitLanes(["modulation", "pitchBend", "sustain"], 80, 48, 12)).toEqual(["pitchBend", "sustain"]);
    expect(fitLanes(["sustain"], 10, 48, 12)).toEqual(["sustain"]);
  });
});

describe("マウス（LaneMouse）", () => {
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

  it("書く道具で押すと点がふえる。横はグリッドに吸いつき（Shift で吸いつかない）、値は押した高さ", () => {
    tool = "draw";
    const m = make();
    m.down(33, 24, none); // (33 * TPQ) / 40 ティック → 1/16（TPQ / 4）の (3 * TPQ) / 4
    expect(m.up()).toEqual({ type: "addControl", take: 7, tick: (3 * TPQ) / 4, control: PB, value: 0 });
    m.down(33, 9, shift);
    expect(m.up()).toEqual({ type: "addControl", take: 7, tick: (33 * TPQ) / 40, control: PB, value: 8191 });
  });

  it("オン ・ オフの CC は、書いても 0 か 127", () => {
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

  it("選ぶ道具で点を押すと選ぶ（Shift で足す、Mod で入れかえ）", () => {
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

  it("押したまま動かすと、横はグリッドに吸いつき、縦は値。離して MoveControls を 1 つ。読み値は「+0512 ・ 1.3.000」の形", () => {
    selected = [{ start: 1, end: 2 }, { start: 4, end: 5 }];
    const m = make(4 * TPQ);
    // 点 1 は 4 * TPQ + TPQ / 2 ティック → x = 4.5 拍 × 40 px = 180。値 0 は y 24。
    m.down(180, 24, none);
    expect(sent).toEqual([]);
    const up = valueToY(PB, LANE, 512) - 24;
    expect(m.move(180 + 23, 24 + up, none)).toBe(true); // (23 * TPQ) / 40 ティック → TPQ / 2 に吸いつく
    expect(m.reading()).toBe("+0512 ・ 2.2.000");
    expect(m.up()).toEqual({ type: "moveControls", take: 7, points: [1, 4], ticks: TPQ / 2, value: 512 });
  });

  it("選んだ点を押して動かさずに離すと、その点だけを選ぶ", () => {
    selected = [{ start: 1, end: 2 }, { start: 4, end: 5 }];
    const m = make();
    m.down(20, 24, none);
    expect(m.up()).toEqual({ type: "selectControls", take: 7, points: [1], mode: "replace" });
  });

  it("値は、動かす点がどれも範囲に入る所で止まる。曲の頭より前へは出ない", () => {
    selected = [{ start: 1, end: 2 }, { start: 4, end: 5 }];
    const m = make();
    m.down(20, 24, none);
    m.move(-200, -500, none);
    expect(m.up()).toEqual({ type: "moveControls", take: 7, points: [1, 4], ticks: -TPQ / 2, value: 8191 - 4096 });
  });

  it("オン ・ オフの CC は、動かしても 0 か 127 に吸いつく", () => {
    control = SUSTAIN;
    const m = make();
    m.down(0, 9, none); // 点 0（127）
    m.move(0, 30, shift);
    expect(m.reading()).toBe("OFF（0）");
    expect(m.up()).toEqual({ type: "moveControls", take: 7, points: [0], ticks: 0, value: -127 });
  });

  it("空いている所から囲んでえらぶ。押しただけなら選択を外す", () => {
    const m = make();
    m.down(5, 1, none);
    m.move(100, 47, none);
    expect(m.reading()).toBe("点 2");
    expect(m.up()).toEqual({ type: "selectControls", take: 7, points: [1, 4], mode: "replace" });
    m.down(5, 1, none);
    expect(m.up()).toEqual({ type: "selectControls", take: 7, points: [], mode: "replace" });
  });

  it("消す道具は、押す ・ なぞると消す（DeleteControls）。乗せている点を覚える", () => {
    tool = "erase";
    const m = make();
    expect(m.hover(80, 15)).toBe(true);
    expect(m.eraseHover).toBe(4);
    m.down(20, 24, none);
    m.move(80, 15, none);
    expect(m.up()).toEqual({ type: "deleteControls", take: 7, points: [1, 4] });
  });

  it("Esc（cancel）で、動かす前に戻す（意図を送らない）", () => {
    const m = make();
    m.down(20, 24, none);
    m.move(60, 10, none);
    expect(m.cancel()).toBe(true);
    expect(m.up()).toBeNull();
    expect(sent).toEqual([{ type: "selectControls", take: 7, points: [1], mode: "replace" }]);
  });

  it("CC の差は 3 けた、ピッチベンドは 4 けた", () => {
    expect(deltaText(MOD, 12)).toBe("+012");
    expect(deltaText(PB, -512)).toBe("−0512");
  });
});

describe("帯の絵（paintLane）", () => {
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

  it("点は 6×6 の gray.10、選ぶと accent。ピッチベンドは 0 の所に gray.5 の線", () => {
    const rects = draw(picture());
    const dots = squares(rects);
    expect(dots.map((r) => [r[0], r[1], colorOf(r)])).toEqual([
      [17, 21, is("color.gray.10")],
      [77, Math.round(valueToY(PB, LANE, 4096) - 3), is("color.accent")],
    ]);
    expect(rects.some((r) => r[1] === 24 && r[3] === 25 && r[2]! - r[0]! === 696 && colorOf(r) === is("color.gray.5"))).toBe(true);
  });

  it("消す道具で乗せている点は gray.7", () => {
    const dots = squares(draw(picture({ eraseHover: 1 })));
    expect(colorOf(dots[0]!)).toBe(is("color.gray.7"));
  });

  it("サステインは段の線（次の点まで値をたもつ）: 127 の高さの横線が、点 0 から点 2 まで", () => {
    const rects = draw(picture({ control: SUSTAIN, selected: null }));
    const line = rects.filter((r) => colorOf(r) === is("color.gray.9"));
    expect(line.some((r) => r[0] === 0 && r[2] === 41 && r[1] === 9)).toBe(true);
    // 点 2 で、127 から 0 へ縦に下りる。
    expect(line.some((r) => r[0] === 40 && r[2] === 41 && r[1] === 9 && r[3] === 40)).toBe(true);
  });

  it("動かしている間は、もとの位置に gray.7 の枠だけ（線と点は動く面）", () => {
    const drag = { kind: "move" as const, press: { x: 0, y: 0, moved: true }, points: [1], grab: 1, ticks: TPQ / 4, value: 0, selectOnClick: false };
    const rects = draw(picture({ drag }));
    expect(rects.some((r) => colorOf(r) === is("color.gray.9"))).toBe(false);
    expect(rects.filter((r) => colorOf(r) === is("color.gray.7"))).toHaveLength(4);
  });
});

describe("Q と、点の Delete（仕様書 4 章）", () => {
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

  it("Q はクオンタイズ。文字を打っている最中は効かない", () => {
    expect(keyAction(k("q"), false, "other")).toEqual({ kind: "quantize" });
    expect(keyAction(k("q"), true, "other")).toBeNull();
    expect(keyAction(k("q", { shift: true }), false, "other")).toBeNull();
  });

  it("選んだノートに、なければ全部（notes が空）に Quantize。ピアノロールにフォーカスがなければ何もしない", () => {
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

  it("Delete: 点だけを選んでいれば点を消す。両方を選んでいるときは、最後に押したほう", () => {
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

describe("偽物の場面 pianoRoll", () => {
  it("CC の札の数はモック A-PianoRoll と同じ（踏む 5 ・ 点 6 ・ 点 0）。そのほかに CC 7", async () => {
    const t = fakeScene("pianoRoll")!.state;
    const transport = createFakeTransport(t);
    const shownTake = decodeTake(await transport.takeNotes(t.pianoRoll.take!));
    const cc = G.ccCounts(shownTake);
    expect([cc.sustain, cc.pitchBend, cc.modulation, cc.others]).toEqual([5, 6, 0, [7]]);
  });

  it("そろえる前の位置の枠は、もとと位置か長さがちがうノート 3 つにだけ", async () => {
    const t = fakeScene("pianoRoll")!.state;
    const transport = createFakeTransport(t);
    const q = decodeTake(await transport.takeNotes(t.pianoRoll.take!));
    const o = decodeTake(await transport.takeNotes(q.origin));
    const moved = Array.from({ length: q.starts.length }, (_, i) => G.movedFrom(q, o, i)).filter((j) => j >= 0);
    expect(moved).toEqual([14, 15, 16]);
  });

  it("偽物は SelectControls に、選んだ点を返す", async () => {
    const t = fakeScene("pianoRoll")!.state;
    const transport = createFakeTransport(t);
    await transport.send({ type: "selectControls", take: t.pianoRoll.take!, points: [3, 1, 999], mode: "replace" });
    const snap = await transport.snapshot();
    expect(snap.state.pianoRoll.selectedControls).toEqual([{ start: 1, end: 2 }, { start: 3, end: 4 }]);
  });
});
