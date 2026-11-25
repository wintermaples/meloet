// Rules of the drum piano roll (spec 3 (drum instruments (Y4), the drum piano roll), spec 2 (Drum hit square)):
// rows and names, row height, hit squares (hit test, box, picture), moving across rows, the draw tool length, ↑ ↓.
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DRUM_ROWS, type Command, type IndexRange } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { Auditioner } from "../src/components/pianoroll/audition";
import { pianoRollCommand } from "../src/components/pianoroll/commands";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import { DRUM_KEYS, drumKeyOf, drumName, drumTerm, isDrumKit, keyLabel, keysDrums, keysLabel, pianoRollDrums } from "../src/components/pianoroll/drums";
import * as G from "../src/components/pianoroll/geom";
import { DRUM_FIELD_ORDER, nextField, noteFieldSpec, readField } from "../src/components/pianoroll/noteFields";
import { noteMarks, paintRoll, paintRollKeys, paintVelocityLane, type RollPicture } from "../src/components/pianoroll/paint";
import { drumFitRowPx, drumView, prView, zoomRowsBy } from "../src/components/pianoroll/view.svelte";
import { THEME_IDS, palettes } from "../src/design/themes";
import { Batch } from "../src/gl/batch";
import { hexToRgb, rgb } from "../src/gl/color";
import { findTerm, t } from "../src/text";
import { texts } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { sectionText, specDocument } from "./design-sources";
import { take } from "./pianoroll-fixture";

const KICK = 36;
const SNARE = 38;
const CLOSED_HH = 42;
const CRASH = 49;
const CLAP = 39;
const SIDE_STICK = 37;

// 1 beat is 40 px (1/16 is 10 px), row 36 px, square width 8 (10 − 2).
const DRUM: G.DrumGeom = { keys: DRUM_KEYS, top: 0, hitW: 8 };
const GEOM: G.RollGeom = { scrollTick: 0, ppt: 40 / TPQ, topKey: 0, rowH: 36, width: 696, height: 440, drum: DRUM };
const rowY = (key: number) => DRUM_KEYS.indexOf(key) * 36;

describe("rows and names (spec 3)", () => {
  it("12 sounds, from the top in order 「クラッシュ」 … 「キック」. Names are the spec text", () => {
    const names = DRUM_ROWS.map((r) => drumName(r.key));
    expect(names).toEqual([
      "クラッシュ",
      "ライド",
      "オープンハイハット",
      "ペダルハイハット",
      "クローズハイハット",
      "ハイタム",
      "ミッドタム 2",
      "ロータム",
      "クラップ",
      "スネア",
      "サイドスティック",
      "キック",
    ]);
    expect(sectionText(specDocument(), "s3")).toContain(`in this order from the top: ${names.map((n) => `「${n}」`).join(", ")}.`);
    for (const r of DRUM_ROWS) expect(texts[`drum.${r.id}` as keyof typeof texts].source).toBe("spec:3");
  });

  it("only instruments in the drum group are drums", () => {
    expect(isDrumKit("studioKit")).toBe(true);
    expect(isDrumKit("electroKit")).toBe(true);
    expect(isDrumKit("brushKit")).toBe(true);
    expect(isDrumKit("grandPiano")).toBe(false);
    expect(isDrumKit(null)).toBe(false);
  });

  it("terms inside names (those explained in chapter 5). Every row name has one", () => {
    expect(drumTerm("ハイタム")).toBe("タム");
    expect(drumTerm("オープンハイハット")).toBe("ハイハット");
    expect(drumTerm("サイドスティック")).toBe("サイドスティック");
    for (const r of DRUM_ROWS) {
      const word = drumTerm(drumName(r.key)!);
      expect(word, r.id).not.toBeNull();
      expect(findTerm(word!), r.id).toBeDefined();
    }
  });

  it("sound names: for drums the instrument name, keys not in the kit get the note name", () => {
    expect(keyLabel(SNARE, true)).toBe("スネア");
    expect(keyLabel(60, true)).toBe("C4");
    expect(keyLabel(SNARE, false)).toBe("D2");
    expect(keysLabel([SNARE, KICK], true)).toBe("キック スネア");
    expect(keysLabel([SNARE, 60], true)).toBe("スネア C4");
    expect(drumKeyOf(" スネア ")).toBe(SNARE);
    expect(drumKeyOf("D2")).toBeNull();
  });

  it("play target of keys: the audition instrument, the recording track, then the chosen track", () => {
    const s = fakeScene("drumRoll")!.state;
    expect(keysDrums(s)).toBe("builtin");
    expect(pianoRollDrums(s)).toBe("builtin");
    s.shelf.selectedSound = { kind: "builtin", instrument: "grandPiano" };
    expect(keysDrums(s)).toBe(false);
    s.shelf.selectedSound = null;
    s.song.playTarget = s.song.keysTarget = s.song.tracks[0]!.id;
    expect(keysDrums(s)).toBe(false);
    // The piano roll is decided by the track of the shown clip.
    expect(pianoRollDrums(s)).toBe("builtin");
  });
});

describe("row height (spec 3)", () => {
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

  it("at first, all rows just fit: height ÷ 12 rounded down to a multiple of 4, 12 to 48. The mock's 440 gives 36", () => {
    expect(drumFitRowPx(440, 12, 100)).toBe(36);
    expect(drumFitRowPx(100, 12, 100)).toBe(12);
    expect(drumFitRowPx(2000, 12, 100)).toBe(48);
    // At 150 %, count in 100 % px (660 ÷ 1.5 = 440).
    expect(drumFitRowPx(660, 12, 150)).toBe(36);
  });

  it("vertical − ＋ goes by 4 and stops at 12 to 48. The normal row height does not change", () => {
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

describe("hit squares (geom.ts)", () => {
  // 0: kick at 1.1 (1 beat long), 1: snare at 1.2, 2: closed hi-hat at 1.1, 3: C4 at 1.1 (not in the kit).
  const n = G.shownOf(
    take([
      [0, TPQ, KICK, 104],
      [0, TPQ / 4, CLOSED_HH, 88],
      [0, TPQ / 4, 60, 80],
      [TPQ, TPQ / 4, SNARE, 96],
    ]),
    0,
  );

  it("row position: from the top in DRUM_KEYS order. Keys not in the kit are NaN, below the bottom row is -1", () => {
    expect(G.keyToY(GEOM, CRASH)).toBe(0);
    expect(G.keyToY(GEOM, KICK)).toBe(11 * 36);
    expect(G.keyToY(GEOM, 60)).toBeNaN();
    expect(G.yToKey(GEOM, 11 * 36 + 5)).toBe(KICK);
    expect(G.yToKey(GEOM, 12 * 36 + 1)).toBe(-1);
    expect(G.yToKey({ ...GEOM, drum: { ...DRUM, top: 2 } }, 1)).toBe(DRUM_KEYS[2]);
  });

  it("only the square hits (not the length). There are no edges", () => {
    expect(G.hitNote(n, GEOM, 4, rowY(KICK) + 10, 6, true)).toEqual({ note: 0, edge: false });
    expect(G.hitNote(n, GEOM, 20, rowY(KICK) + 10, 6, true)).toBeNull();
    expect(G.hitNote(n, GEOM, 41, rowY(SNARE) + 10, 6, true)).toEqual({ note: 3, edge: false });
  });

  it("box: only those touching the box. Keys not in the kit are not included", () => {
    expect(G.notesInBox(n, GEOM, 20, 0, 600, 440)).toEqual([3]);
    expect(G.notesInBox(n, GEOM, 0, 0, 30, 440).sort()).toEqual([0, 1]);
  });

  it("moving across rows: the key difference that lets every moved note move the same number of rows. If none, nearer to 0", () => {
    // One snare 1 row up: clap (39).
    expect(G.drumMoveKeys(DRUM, [SNARE], [0], 0, -1)).toBe(CLAP - SNARE);
    // 1 row down: side stick (37).
    expect(G.drumMoveKeys(DRUM, [SNARE], [0], 0, 1)).toBe(SIDE_STICK - SNARE);
    // Does not go below the bottom row.
    expect(G.drumMoveKeys(DRUM, [KICK], [0], 0, 1)).toBe(0);
    // Kick and snare (2 rows apart) 1 row up: 36 → 37, 38 → 39, both 1 row up.
    expect(G.drumMoveKeys(DRUM, [KICK, SNARE], [0, 1], 0, -1)).toBe(1);
    // Closed hi-hat and snare have no key difference that moves both the same number of rows.
    expect(G.drumMoveKeys(DRUM, [CLOSED_HH, SNARE], [0, 1], 0, -1)).toBe(0);
    // If a key not in the kit is mixed in, no vertical move.
    expect(G.drumMoveKeys(DRUM, [SNARE, 60], [0, 1], 0, -1)).toBe(0);
  });
});

describe("mouse (drag.ts)", () => {
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
      scale: () => ({ root: 0, mode: "major", lock: false, highlight: true }),
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

  it("moving a square to the row below moves it to that row's key (snare → side stick)", () => {
    drag.down(42, rowY(SNARE) + 10, NONE);
    sent.length = 0;
    drag.move(42, rowY(SNARE) + 10 + 36, NONE);
    expect(drag.up()).toEqual({ type: "moveNotes", take: 7, notes: [0], ticks: 0, keys: SIDE_STICK - SNARE, copy: false });
  });

  it("draw tool: one grid cell at the pressed place. Moving right does not change the length. The reading is the instrument name", () => {
    tool = "draw";
    drag.down(85, rowY(KICK) + 5, NONE);
    expect(drag.move(200, rowY(KICK) + 5, NONE)).toBe(false);
    expect(drag.reading()).toBe("キック ・ 1.3.0000 ・ 長さ 0.0480");
    const c = drag.up();
    expect(c).toEqual({ type: "addNote", take: 7, start: 2 * TPQ, length: TPQ / 4, key: KICK, velocity: 100 });
  });

  it("draw tool: does not draw below the bottom row", () => {
    tool = "draw";
    drag.down(85, 12 * 36 + 2, NONE);
    expect(drag.drag).toBeNull();
    expect(sent).toEqual([]);
    expect(drag.hover(85, 12 * 36 + 2, NONE).cursor).toBe("not-allowed");
  });

  it("no edge cursor (there is no edge to change the length)", () => {
    expect(drag.hover(41, rowY(SNARE) + 10, NONE).cursor).toBe("move");
    expect(drag.hover(48, rowY(SNARE) + 10, NONE).cursor).toBe("move");
  });
});

describe("velocity lane: notes on keys not in the kit are not shown there either (spec 3)", () => {
  // 0: C4 at 1.1 (not in the kit), 1: kick at 1.1, 2: C4 at 1.2 alone.
  const n = G.shownOf(
    take([
      [0, TPQ / 4, 60, 100],
      [0, TPQ / 4, KICK, 50],
      [TPQ, TPQ / 4, 60, 100],
    ]),
    0,
  );
  const SIZES = { noteH: 11, edge: 2, velBar: 3, velCap: 7, velFull: 64, velMid: 36, endMark: 16, drumGap: 8 };
  const topOf = (v: number) => 71 - Math.round((v * 64) / 127);

  it("no bar is drawn for them", () => {
    const out = new Batch();
    out.clip(0, 0, 696, 71);
    const p: RollPicture = {
      g: GEOM,
      sizes: SIZES,
      scale: { root: 0, mode: "major", lock: false, highlight: true },
      meter: { num: 4, den: 4 },
      step: TPQ / 4,
      shown: n,
      origin: null,
      selected: null,
      live: [{ start: 2 * TPQ, length: TPQ / 4, key: 60, velocity: 100, lap: 0, index: 0 }],
      drag: null,
      eraseHover: -1,
    };
    paintVelocityLane(out, { x0: 0, y0: 0, x1: 696, y1: 71 }, p, 1);
    const bars = out.rects().filter((r) => r[2]! - r[0]! === 3);
    expect(bars.map((r) => [r[0], r[1]])).toEqual([[1, topOf(50)]]);
  });

  it("they cannot be grabbed, and a drag of the selection leaves them out", () => {
    let selected: IndexRange[] = [];
    const drag = new RollDrag({
      shown: () => n,
      geom: () => GEOM,
      step: () => TPQ / 4,
      scale: () => ({ root: 0, mode: "major", lock: false, highlight: true }),
      meter: () => ({ num: 4, den: 4 }),
      tool: () => "select",
      selected: () => selected,
      edgePx: () => 6,
      stillPx: () => 4,
      velocityFullPx: () => 64,
      velocityHeight: () => 71,
      send: () => {},
      drawVelocity: () => 100,
      mayAudition: () => false,
      ticksToMs: (ticks) => ticks,
      audition: new Auditioner(() => {}),
    });
    const top = 71 - (100 * 64) / 127;
    drag.downVelocity(40, top);
    expect(drag.drag).toBeNull();
    expect(drag.overVelocityBar(40, top)).toBe(false);
    drag.downVelocity(0, top);
    expect(drag.drag).toBeNull();
    selected = [{ start: 0, end: 3 }];
    drag.downVelocity(0, 71 - (50 * 64) / 127);
    expect(drag.drag).toMatchObject({ kind: "velocity", notes: [1], grab: 1 });
  });
});

describe("picture (paint.ts)", () => {
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
    scale: { root: 0, mode: "major", lock: false, highlight: true },
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

  it("square: width 8 from 1px right of the line (1/16 is 10 − 2), height 28 (36 − 8) centered in the 36 row. Length is not shown", () => {
    const rects = draw(picture());
    const kick = rects.filter((r) => r[1] === rowY(KICK) + 4 && r[3] === rowY(KICK) + 32 && r[2] - r[0] > 1);
    expect(kick.map((r) => r.slice(0, 4))).toEqual([[1, rowY(KICK) + 4, 9, rowY(KICK) + 32]]);
    // An ordinary square is accent (velocity 90 and up: all of it) with a 1px accent.ink frame; a selected one is gray.10.
    expect(colorOf(kick[0]!)).toBe(is("color.accent"));
    const inKick = (r: number[]) => r[0]! >= 1 && r[2]! <= 9 && r[1]! >= rowY(KICK) + 4 && r[3]! <= rowY(KICK) + 32;
    const thin = (r: number[]) => r[2]! - r[0]! === 1 || r[3]! - r[1]! === 1;
    expect(rects.filter((r) => inKick(r) && thin(r) && colorOf(r) === is("color.accent.ink"))).toHaveLength(4);
    const snare = rects.find((r) => r[0] === 41 && r[1] === rowY(SNARE) + 4)!;
    expect(snare[2]).toBe(49);
    expect(colorOf(snare)).toBe(is("color.gray.10"));
  });

  it("notes on keys not in the kit are not drawn", () => {
    const rects = draw(picture());
    const squares = rects.filter((r) => r[3] - r[1] === 28 && r[2] - r[0] > 1);
    expect(squares).toHaveLength(2);
  });

  it("row background alternates row.white and row.black. Below 12 rows it stays gray.1", () => {
    const rects = draw(picture());
    const ground = (r: number) => rects.find((x) => x[0] === 0 && x[1] === r * 36 && x[2] === 696 && x[3] === r * 36 + 35)!;
    expect(colorOf(ground(0))).toBe(is("color.row.white"));
    expect(colorOf(ground(1))).toBe(is("color.row.black"));
    expect(colorOf(ground(2))).toBe(is("color.row.white"));
    expect(rects.some((x) => x[1] === 12 * 36 && x[2] === 696)).toBe(false);
  });

  it("the fine grid lines (line.grid) are seen on both drum rows in every theme (lines are not checked by kind any more, user:2026-11-24); no row.out, even with scale lock on", () => {
    const lum = (hex: string) =>
      hexToRgb(hex)
        .map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
        .reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i]!, 0);
    const ratio = (a: string, b: string) => (Math.max(lum(a), lum(b)) + 0.05) / (Math.min(lum(a), lum(b)) + 0.05);
    for (const id of THEME_IDS) {
      const p = palettes[id];
      for (const row of ["color.row.white", "color.row.black"] as const) expect(ratio(p["color.line.grid"], p[row]), `${id} ${row}`).toBeGreaterThanOrEqual(1.1);
    }
    const out = new Batch();
    out.clip(BOX.x0, BOX.y0, BOX.x1, BOX.y1);
    paintRoll(out, BOX, picture({ scale: { root: 0, mode: "major", lock: true, highlight: true } }), null, 1);
    expect(out.rects().some((r) => colorOf(r) === is("color.row.out"))).toBe(false);
  });

  it("a square under the erase tool is gray.7", () => {
    const rects = draw(picture(), noteMarks(3, null, 0));
    const kick = rects.find((r) => r[0] === 1 && r[1] === rowY(KICK) + 4)!;
    expect(colorOf(kick)).toBe(is("color.gray.7"));
  });

  it("the key column background is gray.5 (names are HTML)", () => {
    const out = new Batch();
    out.clip(0, 0, 132, 440);
    paintRollKeys(out, { x0: 0, y0: 0, x1: 132, y1: 440 }, GEOM, () => false, 1);
    const rows = out.rects().filter((r) => r[3] - r[1] === 35);
    expect(rows).toHaveLength(12);
    for (const r of rows) expect(colorOf(r)).toBe(is("color.gray.5"));
  });
});

describe("note list panel and ↑ ↓", () => {
  it("the drum list is position, sound, Vel (Tab skips the length)", () => {
    expect(DRUM_FIELD_ORDER).toEqual(["position", "key", "velocity"]);
    expect(nextField("key", false, DRUM_FIELD_ORDER)).toEqual({ name: "velocity", row: 0 });
    expect(nextField("velocity", false, DRUM_FIELD_ORDER)).toEqual({ name: "position", row: 1 });
  });

  it("the sound field shows the instrument name and reads both names and note names", () => {
    const tk = take([[0, TPQ / 4, SNARE, 96]]);
    const clip = { start: 0, end: 16 * TPQ, takeOffset: 0 };
    const spec = noteFieldSpec("key", tk, clip, [0], { num: 4, den: 4 }, true)!;
    expect(spec.initial).toBe("スネア");
    expect(readField(spec, "キック")?.value).toBe(KICK);
    expect(readField(spec, "C4")?.value).toBe(60);
    expect(readField(spec, "たいこ")).toBeNull();
  });

  it("↑ ↓ go by 1 row for drums. Shift ↑ ↓ is not used", async () => {
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

describe("wording", () => {
  it("the text when hovering scale lock (chapter 6), and 「—」「使えない」 in the right panel (chapter 3)", () => {
    expect(t("msg.drumsNoScaleLock")).toBe("ドラムには音階がないので、スケール固定は使えません。");
    expect(t("inspector.scaleNone")).toBe("—");
    expect(t("inspector.lockUnusable")).toBe("使えない");
  });
});
