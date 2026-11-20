// ドラムのピアノロールの決まり（仕様書 3 章「ドラムの音源（Y4）」「ドラムのピアノロール」、2 章「ドラムの叩いた所の四角」）:
// 行と名前、行の高さ、叩いた所の四角（当たり、囲む、絵）、行をまたいで動かす、書く道具の長さ、↑ ↓。
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DRUM_ROWS, type Command, type IndexRange } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { Auditioner } from "../src/components/pianoroll/audition";
import { pianoRollCommand } from "../src/components/pianoroll/commands";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import { DRUM_KEYS, drumKeyOf, drumName, drumTerm, isDrumKit, keyLabel, keysDrums, keysLabel, pianoRollDrums } from "../src/components/pianoroll/drums";
import * as G from "../src/components/pianoroll/geom";
import { DRUM_FIELD_ORDER, nextField, noteFieldSpec, readField } from "../src/components/pianoroll/noteFields";
import { noteMarks, paintRoll, paintRollKeys, type RollPicture } from "../src/components/pianoroll/paint";
import { drumFitRowPx, drumView, prView, zoomRowsBy } from "../src/components/pianoroll/view.svelte";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { findTerm, t, texts } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { sectionText, specDocument } from "./design-sources";
import { take } from "./pianoroll-fixture";

const KICK = 36;
const SNARE = 38;
const CLOSED_HH = 42;
const CRASH = 49;
const CLAP = 39;
const SIDE_STICK = 37;

// 1 拍 40 px（1/16 は 10 px）、行 36 px、四角の幅 8（10 − 2）。
const DRUM: G.DrumGeom = { keys: DRUM_KEYS, top: 0, hitW: 8 };
const GEOM: G.RollGeom = { scrollTick: 0, ppt: 40 / TPQ, topKey: 0, rowH: 36, width: 696, height: 440, drum: DRUM };
const rowY = (key: number) => DRUM_KEYS.indexOf(key) * 36;

describe("行と名前（仕様書 3 章）", () => {
  it("12 の音を、上から クラッシュ … キック の順に。名前は仕様書の文のまま", () => {
    const names = DRUM_ROWS.map((r) => drumName(r.key));
    expect(names).toEqual([
      "クラッシュ",
      "ライド",
      "オープンハイハット",
      "ペダルハイハット",
      "クローズハイハット",
      "ハイタム",
      "ミッドタム",
      "ロータム",
      "クラップ",
      "スネア",
      "サイドスティック",
      "キック",
    ]);
    expect(sectionText(specDocument(), "s3")).toContain(`上から ${names.join("、")} の順に`);
    for (const r of DRUM_ROWS) expect(texts[`drum.${r.id}` as keyof typeof texts].source).toBe("spec:3");
  });

  it("ドラムのグループの音源だけがドラム", () => {
    expect(isDrumKit("studioKit")).toBe(true);
    expect(isDrumKit("electroKit")).toBe(true);
    expect(isDrumKit("brushKit")).toBe(true);
    expect(isDrumKit("grandPiano")).toBe(false);
    expect(isDrumKit(null)).toBe(false);
  });

  it("名前の中の用語（5 章に説明がある物）。どの行の名前にも 1 つある", () => {
    expect(drumTerm("ハイタム")).toBe("タム");
    expect(drumTerm("オープンハイハット")).toBe("ハイハット");
    expect(drumTerm("サイドスティック")).toBe("サイドスティック");
    for (const r of DRUM_ROWS) {
      const word = drumTerm(drumName(r.key)!);
      expect(word, r.id).not.toBeNull();
      expect(findTerm(word!), r.id).toBeDefined();
    }
  });

  it("音の名前: ドラムなら楽器の名前、キットにない鍵は音名", () => {
    expect(keyLabel(SNARE, true)).toBe("スネア");
    expect(keyLabel(60, true)).toBe("C4");
    expect(keyLabel(SNARE, false)).toBe("D2");
    expect(keysLabel([SNARE, KICK], true)).toBe("キック スネア");
    expect(keysLabel([SNARE, 60], true)).toBe("スネア C4");
    expect(drumKeyOf(" スネア ")).toBe(SNARE);
    expect(drumKeyOf("D2")).toBeNull();
  });

  it("鍵を弾くと鳴る先: ためし弾きの音源、録っているトラック、選んだトラックの順", () => {
    const s = fakeScene("drumRoll")!.state;
    expect(keysDrums(s)).toBe("builtin");
    expect(pianoRollDrums(s)).toBe("builtin");
    s.shelf.selectedSound = { kind: "builtin", instrument: "grandPiano" };
    expect(keysDrums(s)).toBe(false);
    s.shelf.selectedSound = null;
    s.song.playTarget = s.song.tracks[0]!.id;
    expect(keysDrums(s)).toBe(false);
    // ピアノロールは見せているクリップのトラックで決まる。
    expect(pianoRollDrums(s)).toBe("builtin");
  });
});

describe("行の高さ（仕様書 3 章）", () => {
  beforeEach(() => {
    drumView.rowPx = null;
    drumView.topRow = 0;
    prView.drums = true;
    prView.areaHeight = 440;
  });
  afterEach(() => {
    prView.drums = false;
    drumView.rowPx = null;
  });

  it("はじめは全部の行がちょうど入る高さ: 高さ ÷ 12 を 4 の倍数に切りすて、12〜48。モックの 440 は 36", () => {
    expect(drumFitRowPx(440, 12, 100)).toBe(36);
    expect(drumFitRowPx(100, 12, 100)).toBe(12);
    expect(drumFitRowPx(2000, 12, 100)).toBe(48);
    // 150 % では、100 % の px で数える（660 ÷ 1.5 = 440）。
    expect(drumFitRowPx(660, 12, 150)).toBe(36);
  });

  it("縦の − ＋ は 4 ずつ、12〜48 で止まる。ふつうの行の高さは変えない", () => {
    const rowPx = prView.rowPx;
    expect(zoomRowsBy(1)).toBe(true);
    expect(drumView.rowPx).toBe(40);
    zoomRowsBy(1);
    zoomRowsBy(1);
    expect(drumView.rowPx).toBe(48);
    expect(zoomRowsBy(1)).toBe(false);
    for (let i = 0; i < 10; i++) zoomRowsBy(-1);
    expect(drumView.rowPx).toBe(12);
    expect(prView.rowPx).toBe(rowPx);
  });
});

describe("叩いた所の四角（geom.ts）", () => {
  // 0: 1.1 のキック（長さ 1 拍）、1: 1.2 のスネア、2: 1.1 のクローズハイハット、3: 1.1 の C4（キットにない）。
  const n = G.shownOf(
    take([
      [0, TPQ, KICK, 104],
      [0, TPQ / 4, CLOSED_HH, 88],
      [0, TPQ / 4, 60, 80],
      [TPQ, TPQ / 4, SNARE, 96],
    ]),
    0,
  );

  it("行の位置: 上から DRUM_KEYS の順。キットにない鍵は NaN、いちばん下の行より下は -1", () => {
    expect(G.keyToY(GEOM, CRASH)).toBe(0);
    expect(G.keyToY(GEOM, KICK)).toBe(11 * 36);
    expect(G.keyToY(GEOM, 60)).toBeNaN();
    expect(G.yToKey(GEOM, 11 * 36 + 5)).toBe(KICK);
    expect(G.yToKey(GEOM, 12 * 36 + 1)).toBe(-1);
    expect(G.yToKey({ ...GEOM, drum: { ...DRUM, top: 2 } }, 1)).toBe(DRUM_KEYS[2]);
  });

  it("当たるのは四角だけ（長さの所は当たらない）。はしはない", () => {
    expect(G.hitNote(n, GEOM, 4, rowY(KICK) + 10, 6, true)).toEqual({ note: 0, edge: false });
    expect(G.hitNote(n, GEOM, 20, rowY(KICK) + 10, 6, true)).toBeNull();
    expect(G.hitNote(n, GEOM, 41, rowY(SNARE) + 10, 6, true)).toEqual({ note: 3, edge: false });
  });

  it("囲む: 四角にかかる物だけ。キットにない鍵は入らない", () => {
    expect(G.notesInBox(n, GEOM, 20, 0, 600, 440)).toEqual([3]);
    expect(G.notesInBox(n, GEOM, 0, 0, 30, 440).sort()).toEqual([0, 1]);
  });

  it("行をまたいで動かす: 動かすノートがみな同じ行数だけ動ける鍵の差。動けなければ 0 へ近づける", () => {
    // スネア 1 つを 1 行上へ: クラップ（39）。
    expect(G.drumMoveKeys(DRUM, [SNARE], [0], 0, -1)).toBe(CLAP - SNARE);
    // 1 行下へ: サイドスティック（37）。
    expect(G.drumMoveKeys(DRUM, [SNARE], [0], 0, 1)).toBe(SIDE_STICK - SNARE);
    // いちばん下の行より下へは出ない。
    expect(G.drumMoveKeys(DRUM, [KICK], [0], 0, 1)).toBe(0);
    // キックとスネア（2 行はなれている）を 1 行上へ: 36 → 37、38 → 39 で、どちらも 1 行上。
    expect(G.drumMoveKeys(DRUM, [KICK, SNARE], [0, 1], 0, -1)).toBe(1);
    // クローズハイハットとスネアは、同じ鍵の差で同じ行数だけ動ける所がない。
    expect(G.drumMoveKeys(DRUM, [CLOSED_HH, SNARE], [0, 1], 0, -1)).toBe(0);
    // キットにない鍵がまざっていたら、縦には動かさない。
    expect(G.drumMoveKeys(DRUM, [SNARE, 60], [0, 1], 0, -1)).toBe(0);
  });
});

describe("マウス（drag.ts）", () => {
  const NONE: Mods = { shift: false, alt: false, mod: false };
  const n = G.shownOf(take([[TPQ, TPQ / 4, SNARE, 96]]), 0);
  let sent: Command[];
  let selected: IndexRange[];
  let drag: RollDrag;
  let tool: "select" | "draw" | "erase";
  beforeEach(() => {
    sent = [];
    selected = [];
    tool = "select";
    drag = new RollDrag({
      shown: () => n,
      geom: () => GEOM,
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
      drawVelocity: () => 100,
      mayAudition: () => false,
      ticksToMs: (ticks) => ticks,
      audition: new Auditioner((c) => void sent.push(c)),
    });
  });

  it("四角を下の行へ動かすと、その行の鍵へ（スネア → サイドスティック）", () => {
    drag.down(42, rowY(SNARE) + 10, NONE);
    sent.length = 0;
    drag.move(42, rowY(SNARE) + 10 + 36, NONE);
    expect(drag.up()).toEqual({ type: "moveNotes", take: 7, notes: [0], ticks: 0, keys: SIDE_STICK - SNARE, copy: false });
  });

  it("書く道具: 押した所にグリッドの 1 マス。右へ動かしても長さは変わらない。読み値は楽器の名前", () => {
    tool = "draw";
    drag.down(85, rowY(KICK) + 5, NONE);
    expect(drag.move(200, rowY(KICK) + 5, NONE)).toBe(false);
    expect(drag.reading()).toBe("キック ・ 1.3.0000 ・ 長さ 0.0480");
    const c = drag.up();
    expect(c).toEqual({ type: "addNote", take: 7, start: 2 * TPQ, length: TPQ / 4, key: KICK, velocity: 100 });
  });

  it("書く道具: いちばん下の行より下には書かない", () => {
    tool = "draw";
    drag.down(85, 12 * 36 + 2, NONE);
    expect(drag.drag).toBeNull();
    expect(sent).toEqual([]);
    expect(drag.hover(85, 12 * 36 + 2, NONE).cursor).toBe("not-allowed");
  });

  it("はしの形のカーソルは出ない（長さを変えるはしはない）", () => {
    expect(drag.hover(41, rowY(SNARE) + 10, NONE).cursor).toBe("move");
    expect(drag.hover(48, rowY(SNARE) + 10, NONE).cursor).toBe("move");
  });
});

describe("絵（paint.ts）", () => {
  const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8 };
  const BOX = { x0: 0, y0: 0, x1: 696, y1: 440 };
  const colorOf = (r: number[]) => [r[4], r[5], r[6]].join(",");
  const is = (name: Parameters<typeof rgb>[0]) => Array.from(Float32Array.from(rgb(name))).join(",");
  const n = G.shownOf(
    take([
      [0, 2 * TPQ, KICK, 104],
      [0, TPQ / 4, 60, 80],
      [TPQ, TPQ / 4, SNARE, 96],
    ]),
    0,
  );
  const picture = (over: Partial<RollPicture> = {}): RollPicture => ({
    g: GEOM,
    sizes: SIZES,
    scale: { root: 0, mode: "major", lock: false },
    meter: { num: 4, den: 4 },
    step: TPQ / 4,
    shown: n,
    origin: null,
    selected: Uint8Array.from([0, 0, 1]),
    live: null,
    drag: null,
    eraseHover: -1,
    ...over,
  });
  const draw = (p: RollPicture, marks: Uint8Array | null = null) => {
    const out = new Batch();
    out.clip(BOX.x0, BOX.y0, BOX.x1, BOX.y1);
    paintRoll(out, BOX, p, marks, 1);
    return out.rects();
  };

  it("四角: 線の 1px 右から幅 8（1/16 の 10 − 2）、行 36 のまん中に高さ 28（36 − 8）。長さは見せない", () => {
    const rects = draw(picture());
    const kick = rects.filter((r) => r[1] === rowY(KICK) + 4 && r[3] === rowY(KICK) + 32);
    expect(kick.map((r) => r.slice(0, 4))).toEqual([[1, rowY(KICK) + 4, 9, rowY(KICK) + 32]]);
    expect(colorOf(kick[0]!)).toBe(is("color.gray.10"));
    const snare = rects.find((r) => r[0] === 41 && r[1] === rowY(SNARE) + 4)!;
    expect(snare[2]).toBe(49);
    expect(colorOf(snare)).toBe(is("color.accent"));
  });

  it("キットにない鍵のノートは描かない", () => {
    const rects = draw(picture());
    const squares = rects.filter((r) => r[3] - r[1] === 28);
    expect(squares).toHaveLength(2);
  });

  it("行の地は gray.2 と gray.3 を交互に。12 行より下は gray.1 のまま", () => {
    const rects = draw(picture());
    const ground = (r: number) => rects.find((x) => x[0] === 0 && x[1] === r * 36 && x[2] === 696 && x[3] === r * 36 + 35)!;
    expect(colorOf(ground(0))).toBe(is("color.gray.2"));
    expect(colorOf(ground(1))).toBe(is("color.gray.3"));
    expect(rects.some((x) => x[1] === 12 * 36 && x[2] === 696)).toBe(false);
  });

  it("消す道具を乗せた四角は gray.7", () => {
    const rects = draw(picture(), noteMarks(3, null, 0));
    const kick = rects.find((r) => r[0] === 1 && r[1] === rowY(KICK) + 4)!;
    expect(colorOf(kick)).toBe(is("color.gray.7"));
  });

  it("鍵の列の地は gray.5（名前は HTML）", () => {
    const out = new Batch();
    out.clip(0, 0, 132, 440);
    paintRollKeys(out, { x0: 0, y0: 0, x1: 132, y1: 440 }, GEOM, () => false, 1);
    const rows = out.rects().filter((r) => r[3] - r[1] === 35);
    expect(rows).toHaveLength(12);
    for (const r of rows) expect(colorOf(r)).toBe(is("color.gray.5"));
  });
});

describe("ノートの一覧の欄と ↑ ↓", () => {
  it("ドラムの一覧は 位置 ・ 音 ・ Vel（Tab は長さを飛ばす）", () => {
    expect(DRUM_FIELD_ORDER).toEqual(["position", "key", "velocity"]);
    expect(nextField("key", false, DRUM_FIELD_ORDER)).toEqual({ name: "velocity", row: 0 });
    expect(nextField("velocity", false, DRUM_FIELD_ORDER)).toEqual({ name: "position", row: 1 });
  });

  it("音の欄は楽器の名前で見せ、名前でも音名でも読む", () => {
    const tk = take([[0, TPQ / 4, SNARE, 96]]);
    const clip = { start: 0, end: 16 * TPQ, takeOffset: 0 };
    const spec = noteFieldSpec("key", tk, clip, [0], { num: 4, den: 4 }, true)!;
    expect(spec.initial).toBe("スネア");
    expect(readField(spec, "キック")?.value).toBe(KICK);
    expect(readField(spec, "C4")?.value).toBe(60);
    expect(readField(spec, "たいこ")).toBeNull();
  });

  it("↑ ↓ はドラムでは 1 行ずつ。Shift ↑ ↓ は使わない", async () => {
    const s = fakeScene("drumRoll")!.state;
    const clip = s.song.tracks[2]!.clips[0]!;
    const tk = take([[0, TPQ / 4, SNARE, 96]], clip.takeId);
    const shownTake = G.shownOf(tk, clip.start + clip.takeOffset);
    s.pianoRoll = { ...s.pianoRoll, take: clip.takeId, selectedNotes: [{ start: 0, end: 1 }] };
    const up = pianoRollCommand({ kind: "nudge", steps: 0, keys: 1 }, { state: s, shown: shownTake, playTick: 0 });
    expect(up).toEqual({ type: "moveNotes", take: clip.takeId, notes: [0], ticks: 0, keys: CLAP - SNARE, copy: false });
    expect(pianoRollCommand({ kind: "nudge", steps: 0, keys: 12 }, { state: s, shown: shownTake, playTick: 0 })).toBeNull();
    const right = pianoRollCommand({ kind: "nudge", steps: 1, keys: 0 }, { state: s, shown: shownTake, playTick: 0 });
    expect(right).toMatchObject({ type: "moveNotes", ticks: TPQ / 4, keys: 0 });
  });
});

describe("文面", () => {
  it("スケール固定に乗せたときの文（6 章）と、右の欄の「—」「使えない」（3 章）", () => {
    expect(t("msg.drumsNoScaleLock")).toBe("ドラムには音階がないので、スケール固定は使えません。");
    expect(t("inspector.scaleNone")).toBe("—");
    expect(t("inspector.lockUnusable")).toBe("使えない");
  });
});
