// クリップの外に書く ・ 動かす（design-notes B-82）: 画面はクリップの外でも止めずに意図を送り、のばすのは Rust。
import { beforeEach, describe, expect, it } from "vitest";
import type { Command, IndexRange } from "../src/bridge";
import { fakeScene, fakeState } from "../src/bridge/fake";
import { Auditioner } from "../src/components/pianoroll/audition";
import { pianoRollCommand } from "../src/components/pianoroll/commands";
import { RollDrag, type Mods } from "../src/components/pianoroll/drag";
import * as G from "../src/components/pianoroll/geom";
import type { Tool } from "../src/components/pianoroll/view.svelte";
import { t } from "../src/text/ja";
import { GEOM, take } from "./pianoroll-fixture";

const NONE: Mods = { shift: false, alt: false, mod: false };
const BAR = 1920;

// クリップは 2 小節め [1920, 3840)。ノート 0 は 2.1 から 1 拍の C4、ノート 1 は 2.4 から 1 拍の D4（クリップの終わりまで）。
const n = G.shownOf(
  take([
    [0, 480, 60],
    [1440, 480, 62],
  ]),
  BAR,
  { from: BAR, to: 2 * BAR },
);
// 1 小節めの頭から 3 小節めの終わりまでが見えている。
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
    step: () => 120,
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

describe("書く道具", () => {
  it("クリップのうしろを押すと、そのテイクに AddNote を送る（丸めない）", () => {
    tool = "draw";
    drag.down(x(2 * BAR + 500), y(64), NONE);
    expect(drag.up()).toEqual({ type: "addNote", take: 7, start: 2 * BAR + 480, length: 120, key: 64, velocity: G.DRAW_VELOCITY });
  });

  it("クリップの前（1 小節め）も同じ", () => {
    tool = "draw";
    drag.down(x(250), y(64), NONE);
    expect(drag.up()).toEqual({ type: "addNote", take: 7, start: 240, length: 120, key: 64, velocity: G.DRAW_VELOCITY });
  });

  it("クリップの中から外まで押したまま右へのばすと、のびた長さで送る", () => {
    tool = "draw";
    drag.down(x(2 * BAR - 240), y(64), NONE);
    drag.move(x(2 * BAR + 600), y(64), NONE);
    expect(drag.up()).toMatchObject({ type: "addNote", start: 2 * BAR - 240, length: 840 });
  });
});

describe("選ぶ道具", () => {
  it("ドラッグでクリップのうしろへ出せる", () => {
    drag.down(x(BAR + 1500), y(62), NONE);
    drag.move(x(BAR + 1500 + 960), y(62), NONE);
    expect(drag.up()).toEqual({ type: "moveNotes", take: 7, notes: [1], ticks: 960, keys: 0, copy: false });
  });

  it("ドラッグでクリップの前へ出せる。曲の頭で止まる", () => {
    drag.down(x(BAR + 100), y(60), NONE);
    drag.move(x(0) - 200, y(60), NONE);
    expect(drag.up()).toEqual({ type: "moveNotes", take: 7, notes: [0], ticks: -BAR, keys: 0, copy: false });
  });

  it("右はしをクリップの外までのばせる", () => {
    drag.down(x(2 * BAR) - 2, y(62), NONE);
    drag.move(x(2 * BAR + 960) - 2, y(62), NONE);
    expect(drag.up()).toEqual({ type: "resizeNotes", take: 7, notes: [1], edge: "end", ticks: 960 });
  });
});

describe("← →", () => {
  it("クリップの終わりにあるノートも、グリッド 1 つ外へ動かす意図になる", () => {
    const state = fakeState();
    state.pianoRoll = { ...state.pianoRoll, take: 7, selectedNotes: [{ start: 1, end: 2 }], grid: { division: 16, triplet: false } };
    expect(pianoRollCommand({ kind: "nudge", steps: 1, keys: 0 }, { state, shown: n, playTick: 0 })).toEqual({
      type: "moveNotes",
      take: 7,
      notes: [1],
      ticks: 120,
      keys: 0,
      copy: false,
    });
    state.pianoRoll = { ...state.pianoRoll, selectedNotes: [{ start: 0, end: 1 }] };
    expect(pianoRollCommand({ kind: "nudge", steps: -1, keys: 0 }, { state, shown: n, playTick: 0 })).toMatchObject({ notes: [0], ticks: -120 });
  });
});

describe("知らせ", () => {
  it("のばせなかったときの文（書いた ・ 動かした）", () => {
    expect(fakeScene("notesNotWritten")!.state.status.message).toEqual({ id: "notesNotWritten" });
    expect(t("msg.notesNotWritten")).toBe("ほかのクリップと重なるので、書けませんでした。");
    expect(t("msg.notesNotResized")).toBe("ほかのクリップと重なるので、長さを変えられませんでした。");
    expect(t("msg.clipsNotMoved")).toBe("ほかのクリップと重なるので、動かしませんでした。");
  });

  it("のびたクリップの場面: 1–5 小節、書いたノートは見えている", () => {
    const s = fakeScene("pianoRollGrown")!.state;
    const clip = s.song.tracks[0]!.clips[0]!;
    expect([clip.start, clip.end]).toEqual([0, 5 * BAR]);
    expect(G.barsText(clip.start, clip.end, { num: 4, den: 4 })).toBe("1–5");
  });
});
