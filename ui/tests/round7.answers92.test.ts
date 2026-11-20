// 第 7 回 区切り0 の 0-2: 仕様書 第 11 版の 92 の答え（3 章「第 10 版で足した決まり」「オーナーさんがえらんだ形」、モック F7-Answers92 ・ R11-Answers92）。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, LanePoints } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import * as A from "../src/components/automation/autoGeom";
import { handAt, noteHand } from "../src/components/automation/handValue";
import HistoryHead from "../src/components/history/HistoryHead.svelte";
import { closeContextMenu, contextMenu } from "../src/components/menu/contextMenu.svelte";
import { labelsBesideMarkers } from "../src/components/pianoroll/rulerMarks";
import StatusLine from "../src/components/StatusLine.svelte";
import TempoBand from "../src/components/timeline/TempoBand.svelte";
import { runFrameCallbacks, watchSlots, type GlRect, type GlSlot } from "../src/gl/slot";
import { mapOf } from "../src/time/bars";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const POINTS = new Map<number, LanePoints>();
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  peekLanePoints: (id: number) => POINTS.get(id),
  lanePoints: (id: number) => Promise.resolve(POINTS.get(id)!),
}));
let tick = 0;
vi.mock("../src/components/automation/autoState.svelte", async (original) => ({
  ...(await original<typeof import("../src/components/automation/autoState.svelte")>()),
  playTick: () => tick,
}));

const BAR = 4 * TPQ;
const scene = (name: string) => fakeScene(name)!.state;
const points = (id: number, list: [number, number][]): LanePoints => ({
  id,
  step: false,
  ticks: Float64Array.from(list.map((p) => p[0])),
  values: Float32Array.from(list.map((p) => p[1])),
});

describe("帯の縦（仕様書 3 章 92-6 ・ 21-B）", () => {
  const pt = (value: number) => ({ tick: 0, value });
  it("点のいちばん下からいちばん上まで、上下に 1 割のゆとり", () => {
    const r = A.laneRange([pt(0.5), pt(0.7), pt(0.6)], false);
    expect(r.lo).toBeCloseTo(0.48);
    expect(r.hi).toBeCloseTo(0.72);
  });
  it("点が 1 つか、どれも同じ値なら、その値のまわりにパラメータの幅の 1 割。はみ出す分は反対へずらす", () => {
    const one = A.laneRange([pt(0.5)], false);
    expect([one.lo, one.hi].map((v) => +v.toFixed(3))).toEqual([0.45, 0.55]);
    const same = A.laneRange([pt(0.98), pt(0.98)], false);
    expect([same.lo, same.hi].map((v) => +v.toFixed(3))).toEqual([0.9, 1]);
  });
  it("オン ・ オフとミュート（階段の帯）は、いつも下〜上。点がない帯も", () => {
    expect(A.laneRange([pt(0.2), pt(0.3)], true)).toBe(A.FULL_RANGE);
    expect(A.laneRange([], false)).toBe(A.FULL_RANGE);
  });
  it("範囲の下が帯の下、上が帯の上。範囲の外は帯のはしで止まる。上下の動きは範囲の幅で", () => {
    const d: A.LaneDims = { h: 48, pad: 4, hair: 1, point: 6, range: { lo: 0.4, hi: 0.6 } };
    expect([A.valueToY(d, 0.6), A.valueToY(d, 0.4), A.valueToY(d, 0.9), A.valueToY(d, 0.1)]).toEqual([4, 43, 4, 43]);
    expect(A.yToValue(d, A.valueToY(d, 0.5))).toBeCloseTo(0.5, 2);
    expect(A.dyToValue(d, -39)).toBeCloseTo(0.2);
  });
  it("右上と右下の字は、点のいちばん上と下（点が 1 つなら範囲のはし）", () => {
    expect(A.rangeEnds([pt(0.5), pt(0.7)], { lo: 0.48, hi: 0.72 })).toEqual({ lo: 0.5, hi: 0.7 });
    expect(A.rangeEnds([pt(0.5)], { lo: 0.45, hi: 0.55 })).toEqual({ lo: 0.45, hi: 0.55 });
  });
});

// ---- 部品 ----

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
const slots = new Map<HTMLElement, GlSlot>();
let stopWatch: () => void = () => {};
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
  slots.clear();
  stopWatch = watchSlots({
    attach: (node, slot) => slots.set(node, slot),
    update: (node, slot) => slots.set(node, slot),
    detach: (node) => slots.delete(node),
  });
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  stopWatch();
  POINTS.clear();
  closeContextMenu();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const rectsOf = (el: HTMLElement | null): readonly GlRect[] => {
  const s = el ? slots.get(el) : undefined;
  return s?.kind === "rects" ? s.rects : [];
};
const settle = async () => {
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
};

describe("F7-Answers92: 帯の縦の字と「＋ 帯を足す ▾」", () => {
  it("音量が −9.0〜−3.0 dB だけ動く帯は、右上に「上 −3.0 dB」、右下に「下 −9.0 dB」", async () => {
    POINTS.set(520, points(520, [[0, 0.64], [4 * BAR, 0.64], [5 * BAR, 0.7], [6 * BAR + 2 * TPQ, 0.66], [8 * BAR, 0.5]]));
    show(Arrange, scene("laneScale"));
    await settle();
    expect(text(q("[data-test=auto-range-top]"))).toBe("上 −3.0 dB");
    expect(text(q("[data-test=auto-range-bottom]"))).toBe("下 −9.0 dB");
    expect(text(q("[data-test=lane-add-row]"))).toBe("＋ 帯を足す ▾");
  });
});

describe("書いている間の線（F6-AutoRec。core は離したときに 1 手、画面が手の値から枠の中を描く）", () => {
  it("手の値をためて rec の枠の中に描き、core の手（新しい laneId）の点が届いたら捨てる", async () => {
    POINTS.set(501, points(501, [[0, 0.8], [4 * BAR, 0.64], [7 * BAR, 0.64], [8 * BAR + TPQ, 0.72], [12 * BAR, 0.6]]));
    const state = scene("autoRec");
    const piano = state.song.tracks[0]!;
    const from = state.automation.writing[0]!.fromTick;
    show(Arrange, state);
    await settle();
    expect(q("[data-test=auto-write-line]")).toBeNull();

    // フェーダーを動かした（ミキサーの「音だけ変える」意図）。
    noteHand({ type: "setGain", strip: { kind: "track", id: piano.id }, db: -8.4, commit: false });
    expect(handAt(piano.id, { kind: "volume" })).toBeCloseTo(0.52);
    tick = from + TPQ;
    runFrameCallbacks();
    flushSync();
    for (let i = 2; i <= 8; i++) {
      tick = from + i * TPQ;
      runFrameCallbacks();
      flushSync();
    }
    const line = q("[data-test=auto-write-line]");
    expect(line).not.toBeNull();
    const rects = rectsOf(line);
    // 枠の中の帯の線をかくす地と、線（gray.9）と点（gray.10）。
    expect(rects[0]!.color).toBe("color.gray.2");
    expect(rects.some((r) => r.color === "color.gray.9")).toBe(true);
    expect(rects.some((r) => r.color === "color.gray.10" && r.w === r.h)).toBe(true);

    // 離した: core の手で帯が新しい laneId になり、書いている帯がなくなる。点が届くまでは残す。
    app.setState({
      ...state,
      automation: { ...state.automation, writing: [], armed: [] },
      song: { ...state.song, tracks: [{ ...piano, lanes: [{ ...piano.lanes[0]!, laneId: 601 }, ...piano.lanes.slice(1)] }, ...state.song.tracks.slice(1)] },
    });
    flushSync();
    expect(q("[data-test=auto-write-line]")).not.toBeNull();
    POINTS.set(601, points(601, [[0, 0.8], [4 * BAR, 0.64], [from, 0.52], [12 * BAR, 0.6]]));
    await settle();
    expect(q("[data-test=auto-write-line]")).toBeNull();
  });

  it("やめた（同じ laneId のまま書きおわった）ら、すぐ捨てる", async () => {
    POINTS.set(501, points(501, [[0, 0.8], [12 * BAR, 0.6]]));
    const state = scene("autoRec");
    const piano = state.song.tracks[0]!;
    show(Arrange, state);
    await settle();
    noteHand({ type: "setGain", strip: { kind: "track", id: piano.id }, db: -8.4, commit: false });
    tick = state.automation.writing[0]!.fromTick + TPQ;
    runFrameCallbacks();
    flushSync();
    expect(q("[data-test=auto-write-line]")).not.toBeNull();
    app.setState({ ...state, automation: { ...state.automation, writing: [] } });
    flushSync();
    expect(q("[data-test=auto-write-line]")).toBeNull();
  });
});

describe("履歴の帯の見出し（仕様書 6 章 ・ 3 章 92-9、R11-Answers92）", () => {
  it.each([
    ["historyLatest", "14 手 ・ いまは 14 手目（いちばん新しい手）"],
    ["historyAllUndone", "14 手 ・ ぜんぶ取り消してあります ・ やりなおすには Ctrl Shift Z"],
    ["historyEmpty", "まだ手がありません ・ 何かすると、ここに 1 手ずつならびます"],
    ["historyList", "14 手 ・ いまは 12 手目。上の 2 手は取り消してあり、やりなおせます"],
  ])("%s", (name, head) => {
    show(HistoryHead, scene(name));
    expect(text(q("[data-test=history-head]"))).toBe(head);
  });
  it("やりなおす手がないときの Mod Shift Z は「やりなおせる手がありません。」", () => {
    const s = scene("historyLatest");
    s.status.message = { id: "nothingToRedo" };
    show(StatusLine, s);
    expect(text(q(".status"))).toContain("やりなおせる手がありません。");
  });
});

describe("曲の頭のテンポ ・ 拍子（仕様書 3 章 92-2）", () => {
  it("曲の頭の点と旗は「変更」と呼ばない", () => {
    const s = scene("tempo");
    s.status.message = { id: "tempoPointSelected", bar: 1, beat: 1, tempoBpm: 96 };
    show(StatusLine, s);
    expect(target.textContent).toContain("曲の頭のテンポをえらんでいます ・ 96.00");
    unmount(mounted!);
    s.status.message = { id: "meterChangeSelected", bar: 1, meterNum: 4, meterDen: 4 };
    mounted = mount(StatusLine, { target });
    flushSync();
    expect(target.textContent).toContain("曲の頭の拍子をえらんでいます ・ 4/4");
    unmount(mounted!);
    s.status.message = { id: "meterChangeSelected", bar: 9, meterNum: 3, meterDen: 4 };
    mounted = mount(StatusLine, { target });
    flushSync();
    expect(target.textContent).toContain("9 小節目の変更をえらんでいます ・ 3/4");
  });
});

describe("拍子の旗の右クリック（仕様書 3 章 92-17、R11-Answers92）", () => {
  const view = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
  function band(): void {
    const s = scene("tempo");
    show(TempoBand, s, { view, map: mapOf(s.timeline), timeline: s.timeline, now: 0, busy: false });
  }
  const labels = () => contextMenu.open!.entries.map((e) => (e.kind === "row" ? e.label : "—"));
  it("「拍子を打ちこむ…」「消す Delete」。曲の頭の旗には「消す」がない", () => {
    band();
    const flags = qa("[data-test=meter-flag]");
    flags[1]!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    flushSync();
    expect(labels()).toEqual(["menu.typeMeter", "—", "menu.delete"]);
    (contextMenu.open!.entries[0] as { run: () => void }).run();
    flushSync();
    expect(q("[data-test=meter-flag-field]")).not.toBeNull();
    closeContextMenu();
    flags[0]!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    flushSync();
    expect(labels()).toEqual(["menu.typeMeter"]);
  });
});

describe("ピアノロールの目盛のマーカー（R11-Answers92）", () => {
  it("名前は拍の数字の上に重ね、名前がかかる拍の数字は出さない（小節の頭の数字にかかるときも）", () => {
    // モックの「いま」: 32 px ごとの拍の数字、6.1 の所に「サビ」（幅 40）。6.1 と 6.2 が消える。
    const labels = ["5.4", "6.1", "6.2", "6.3"].map((text, i) => ({ tick: i, x: 99 + 32 * i, text }));
    const widths = { 0: 22, 1: 22, 2: 22, 3: 22 };
    const shown = labelsBesideMarkers(labels, [{ id: 1, x: 128 }], widths, { 1: 40 });
    expect(shown.map((l) => l.text)).toEqual(["5.4", "6.3"]);
    expect(labelsBesideMarkers(labels, [{ id: 1, x: 128 }], widths, {})).toHaveLength(4);
  });
});
