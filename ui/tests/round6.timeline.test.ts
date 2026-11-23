// 第 6 回 区切り1 の画面: テンポと拍子の変更（帯、小節の線、再生まわりの値）とマーカー（行、名前、動かす、キー、ピアノロールの目盛）。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TimelineView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import { timelineKeyCommand } from "../src/components/timeline/keys";
import { lastPick, pickClips, pickTimeline } from "../src/components/timeline/pick";
import * as T from "../src/components/timeline/tempoGeom";
import Transport from "../src/components/Transport.svelte";
import { readPosition } from "../src/components/valueClamp";
import { barsText } from "../src/components/pianoroll/geom";
import { eachGridTick, eachLine, gridTick, mapOf, nearestBarTick, nearestBeatTick, snapToStep, tickOfPosition } from "../src/time/bars";
import { noteFieldSpec } from "../src/components/pianoroll/noteFields";
import { selectedSummary } from "../src/components/pianoroll/notes";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 4 * TPQ;
const scene = (name: string): AppState => fakeScene(name)!.state;

/** 4/4 で 8 小節、9 小節目から 3/4 を 4 小節、13 小節目から 4/4（モック F6-Tempo の曲）。 */
const tempoTimeline = (): TimelineView => scene("tempo").timeline;
const THREE_FOUR = 8 * BAR;
const FOUR_FOUR = THREE_FOUR + 4 * 3 * TPQ;

describe("小節の線（テンポの地図から）", () => {
  const map = mapOf(tempoTimeline());

  it("同じ区画には同じ地図", () => {
    const tl = tempoTimeline();
    expect(mapOf(tl)).toBe(mapOf(tl));
  });

  it("小節の頭は、拍子の変わった所から 3/4 の長さでならぶ", () => {
    const heads: number[] = [];
    eachLine(map, 7 * BAR, FOUR_FOUR + BAR, (_, bar) => bar, (tick, kind) => kind === 0 && heads.push(tick));
    expect(heads).toEqual([7 * BAR, THREE_FOUR, THREE_FOUR + 3 * TPQ, THREE_FOUR + 6 * TPQ, THREE_FOUR + 9 * TPQ, FOUR_FOUR]);
  });

  it("拍の線は、その小節の拍子の拍", () => {
    const kinds: string[] = [];
    eachLine(map, THREE_FOUR, THREE_FOUR + 3 * TPQ, (beat) => beat, (tick, kind) => kinds.push(`${tick - THREE_FOUR}:${kind}`));
    expect(kinds).toEqual(["0:0", `${TPQ}:1`, `${2 * TPQ}:1`]);
  });

  it("吸いつき: いちばん近い拍 ・ 小節の頭（まん中なら後ろ）", () => {
    expect(nearestBeatTick(map, THREE_FOUR + TPQ / 2 - 1)).toBe(THREE_FOUR);
    expect(nearestBeatTick(map, THREE_FOUR + TPQ / 2)).toBe(THREE_FOUR + TPQ);
    expect(nearestBarTick(map, THREE_FOUR + (35 * TPQ) / 24)).toBe(THREE_FOUR);
    expect(nearestBarTick(map, THREE_FOUR + (3 * TPQ) / 2)).toBe(THREE_FOUR + 3 * TPQ);
    expect(nearestBarTick(map, -5)).toBe(0);
    // 1/16 のグリッドは、3/4 の小節でも小節の頭から。1 小節より長いきざみは小節の頭へ。
    expect(snapToStep(map, THREE_FOUR + (13 * TPQ) / 48, TPQ / 4)).toBe(THREE_FOUR + TPQ / 4);
    expect(snapToStep(map, THREE_FOUR + (25 * TPQ) / 12, Infinity)).toBe(THREE_FOUR + 3 * TPQ);
  });

  it("位置の打ちこみ「10.2.0960」は 3/4 の小節で数え、拍は 1〜3 に入れる", () => {
    expect(tickOfPosition(map, 10, 2, TPQ / 2)).toBe(THREE_FOUR + 3 * TPQ + TPQ + TPQ / 2);
    expect(readPosition("10.2.0960", map)).toEqual({ value: THREE_FOUR + 3 * TPQ + (3 * TPQ) / 2, clamp: null });
    const r = readPosition("10.4", map)!;
    expect(r.value).toBe(THREE_FOUR + 3 * TPQ + 2 * TPQ);
    expect(r.clamp).toEqual({ field: "beat", value: "10. 3. 0000", range: "1〜3" });
  });

  it("クリップの小節の札「9–12」", () => {
    expect(barsText(THREE_FOUR, FOUR_FOUR, map)).toBe("9–12");
    expect(barsText(0, 4 * BAR, { num: 4, den: 4 })).toBe("1–4");
  });
});

describe("ノートのグリッド（拍子の区間の頭から数える）", () => {
  // 4/4 を 1 小節、3/16 を 2 小節（BAR〜BAR + 3 * TPQ / 2）、そのあと 4/4。
  const map = mapOf({
    ...tempoTimeline(),
    tempoPoints: [],
    meterChanges: [
      { bar: 1, meterNum: 3, meterDen: 16 },
      { bar: 3, meterNum: 4, meterDen: 4 },
    ],
  });

  it("1/8 三連符（TPQ / 3）は BAR から数え、次の旗をこえる段は旗の小節の頭（BAR + 3 * TPQ / 2）", () => {
    const t3 = TPQ / 3;
    expect(gridTick(map, BAR + (3 * TPQ) / 8, t3)).toBe(BAR + t3);
    expect(gridTick(map, BAR + (35 * TPQ) / 24, t3)).toBe(BAR + (3 * TPQ) / 2);
    expect(gridTick(map, BAR + (17 * TPQ) / 12, t3, true)).toBe(BAR + 4 * t3);
    expect(gridTick(map, BAR + (13 * TPQ) / 8, t3)).toBe(BAR + (3 * TPQ) / 2);
    expect(gridTick(map, -50, t3)).toBe(0);
  });

  it("1 小節のグリッドは、その所の小節の頭", () => {
    expect(gridTick(map, BAR + (19 * TPQ) / 24, BAR, false, true)).toBe(BAR + (3 * TPQ) / 4);
    expect(gridTick(map, BAR + (19 * TPQ) / 24, BAR, true, true)).toBe(BAR + (3 * TPQ) / 4);
  });

  it("方眼の線は吸いつきと同じ所", () => {
    const ticks: number[] = [];
    eachGridTick(map, BAR - TPQ / 24, BAR + (11 * TPQ) / 6, TPQ / 3, (tick) => ticks.push(tick));
    expect(ticks).toEqual([BAR, BAR + TPQ / 3, BAR + (2 * TPQ) / 3, BAR + TPQ, BAR + (4 * TPQ) / 3, BAR + (3 * TPQ) / 2]);
  });
});

describe("ノートの一覧 ・ 選んだノートの位置（地図から）", () => {
  const map = mapOf(tempoTimeline());
  const take = { starts: [THREE_FOUR + 3 * TPQ + TPQ], lengths: [TPQ], keys: [60], velocities: [90] } as never;
  const clip = { start: 0, end: 20 * BAR, takeOffset: 0 };

  it("位置は 3/4 の小節で数え、打ちこみも同じ", () => {
    const spec = noteFieldSpec("position", take, clip, [0], map)!;
    expect(spec.initial).toBe("10.2.0000");
    expect(spec.read("10.3")).toBe(THREE_FOUR + 3 * TPQ + 2 * TPQ);
    expect(spec.read("10.4")).toBeNull();
    expect(selectedSummary(take, clip, [{ start: 0, end: 1 }], map)!.position).toBe("10.2.0000");
  });
});

describe("テンポの帯の形", () => {
  const tl = tempoTimeline();
  const map = mapOf(tl);

  it("件数は変わる所（ティック）の数。見出しは「テンポ 96.00 ほか 3」「拍子 4/4 ほか 2」", () => {
    expect(T.changeCount(tl, map)).toBe(3);
    expect(T.summaries(tl)).toEqual({ tempo: "テンポ 96.00 ほか 3", meter: "拍子 4/4 ほか 2" });
    expect(T.summaries(scene("empty").timeline)).toEqual({ tempo: "テンポ 96.00", meter: "拍子 4/4" });
  });

  it("いちばん高いテンポが上から pad、いちばん低いテンポが下から pad", () => {
    const s = T.tempoScale(T.tempoPoints(tl), 48, 12, 1);
    expect(T.bpmToY(s, 120)).toBe(12);
    expect(T.bpmToY(s, 96)).toBe(36);
    const flat = T.tempoScale([{ tick: 0, bpm: 96, ramp: false }], 48, 12, 1);
    expect(T.bpmToY(flat, 96)).toBe(24);
  });

  it("だんだんの区間と、その線（斜めは段の四角の列）", () => {
    const v = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
    const pts = T.tempoPoints(tl);
    expect(T.rampSpans(v, pts)).toEqual([{ tick: 4 * BAR, x0: 256, x1: 512 }]);
    const s = T.tempoScale(pts, 48, 12, 1);
    const rects = T.tempoRects(v, pts, s, { hair: 1, point: 6 }, (p) => ({ selected: p.tick === 4 * BAR, hovered: false }), null);
    expect(rects.filter((r) => r.color === "color.accent")).toEqual([{ x: 253, y: 25, w: 6, h: 6, color: "color.accent" }]);
    expect(rects.filter((r) => r.color === "color.gray.10")).toHaveLength(3);
  });

  it("ドラッグ: 横は拍の頭、上下はテンポ。曲の頭は横に動かない。同じ所の点と入れかわる", () => {
    const pts = T.tempoPoints(tl);
    const s = T.tempoScale(pts, 48, 12, 1);
    const v = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
    const start: T.PointDrag = { tick: THREE_FOUR, fromBpm: 120, toTick: THREE_FOUR, bpm: 120, downX: 512, downY: 12, moved: false };
    const d = T.dragPoint(start, v, s, map, 512 + 17, 12 - 6, 4, false);
    expect(d).toMatchObject({ moved: true, toTick: THREE_FOUR + TPQ, bpm: 126 });
    const head = T.dragPoint({ ...start, tick: 0, toTick: 0, fromBpm: 96, bpm: 96 }, v, s, map, 600, 12, 4, false);
    expect(head.toTick).toBe(0);
    const onto = T.draggedPoints(pts, { ...start, moved: true, toTick: FOUR_FOUR, bpm: 110 });
    expect(onto.map((p) => [p.tick, p.bpm])).toEqual([
      [0, 96],
      [4 * BAR, 104],
      [FOUR_FOUR, 110],
    ]);
  });

  it("2 回押した所: 下の段はその拍の頭と線の上のテンポ、上の段はその小節", () => {
    const v = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
    expect(T.addAt(v, map, 6 * 64 + 1, "tempo")).toEqual({ tick: 6 * BAR, bpm: 112 });
    expect(T.addAt(v, map, 512 + 60, "meter")).toEqual({ bar: 9, num: 3, den: 4 });
  });
});

describe("キー（仕様書 4 章）", () => {
  const k = (key: string, more: Partial<{ alt: boolean; shift: boolean; ctrl: boolean }> = {}) => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...more });
  const tl = scene("markerMoving").timeline;
  const plain = { typing: false, busy: false, toArrange: true };
  const typing = { ...plain, typing: true };

  it("K で「マーカー 4」。文字を打っている最中は効かない", () => {
    expect(timelineKeyCommand(k("k"), plain, tl)).toEqual({ type: "addMarker", name: "マーカー 4", tick: null });
    expect(timelineKeyCommand(k("k"), typing, tl)).toBeNull();
  });
  it("Alt → ・ Alt ← は打っている最中も効く", () => {
    expect(timelineKeyCommand(k("ArrowRight", { alt: true }), typing, tl)).toEqual({ type: "stepMarker", forward: true });
    expect(timelineKeyCommand(k("ArrowLeft", { alt: true }), plain, tl)).toEqual({ type: "stepMarker", forward: false });
  });
  it("F2 はえらんだマーカーの名前", () => {
    expect(timelineKeyCommand(k("F2"), plain, tl)).toEqual({ type: "openMarkerName", id: 2 });
    expect(timelineKeyCommand(k("F2"), plain, { ...tl, selected: null })).toBeNull();
  });
  it("Delete は、クリップより後にえらんだときだけ、えらんだ物へ（曲の頭も受けて、クリップへ回さない）", () => {
    pickTimeline();
    expect(timelineKeyCommand(k("Delete"), plain, tl)).toEqual({ type: "deleteTimeline", target: { kind: "marker", id: 2 } });
    const head = { kind: "tempoPoint", tick: 0 } as const;
    expect(timelineKeyCommand(k("Delete"), plain, { ...tl, selected: head })).toEqual({ type: "deleteTimeline", target: head });
    expect(timelineKeyCommand(k("Delete"), { ...plain, toArrange: false }, tl)).toBeNull();
    pickClips();
    expect(timelineKeyCommand(k("Delete"), plain, tl)).toBeNull();
  });
  it("録音中とカウントイン中は、どれも送らない", () => {
    pickTimeline();
    const busy = { ...plain, busy: true };
    for (const key of [k("k"), k("F2"), k("Delete"), k("ArrowRight", { alt: true })]) expect(timelineKeyCommand(key, busy, tl)).toBeNull();
  });
});

// ---- 部品 ----

const WIDTH = 1000;
const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  contextMenu.open = null;
});

function show(state: AppState, component: Component<any> = Arrange): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
function pointer(el: HTMLElement, type: string, clientX: number, clientY = 10, more: MouseEventInit = {}): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX, clientY, button: 0, ...more }));
  flushSync();
}

describe("アレンジの「テンポ ・ 拍子の変更」の行（モック F6-Tempo ・ F6-AutoRec）", () => {
  it("ひらいた行: 見出し、旗、値、だんだん。えらんだ点の値は accent", () => {
    show(scene("tempo"));
    expect(text(q('[data-test="tempo-toggle"]'))).toBe("▾ テンポ ・ 拍子の変更 3 件");
    expect(text(q('[data-test="tempo-summary"]'))).toBe("テンポ 96.00 ほか 3");
    expect(text(q('[data-test="meter-summary"]'))).toBe("拍子 4/4 ほか 2");
    expect(qa('[data-test="meter-flag"]').map(text)).toEqual(["4/4", "3/4", "4/4"]);
    expect(qa('[data-test="meter-flag"]').map((f) => f.style.left)).toEqual(["0px", "512px", "704px"]);
    expect(qa(".value").map(text)).toEqual(["96.00", "104.00", "120.00", "96.00"]);
    expect(qa(".value.selected").map(text)).toEqual(["104.00"]);
    expect(text(q(".ramp"))).toBe("だんだん");
    // 目盛の数字は 3/4 の小節を 48 px で。
    const n = qa(".bar-number").find((b) => text(b) === "10")!;
    expect(n.style.left).toBe("560px");
  });

  it("線と点は arrangeLanes の区画の中（区画はふえない）", () => {
    show(scene("tempo"));
    const row = q('[data-test="tempo-row"]')!;
    expect(row.dataset.glSlot).toBe("rects");
    expect(row.closest("[data-gl-region]")!.getAttribute("data-gl-region")).toBe("arrangeLanes");
    expect(qa("[data-gl-region]").map((r) => r.dataset.glRegion).sort()).toEqual(["arrangeLanes", "recordingClip", "trackMeters"]);
  });

  it("たたんだ行: 件数と、再生の線がいる所のテンポと拍子。押すとひらく意図", () => {
    show(scene("tempoFolded"));
    expect(text(q('[data-test="tempo-toggle"]'))).toBe("▸ テンポ ・ 拍子の変更 1 件");
    expect(text(q('[data-test="tempo-now"]'))).toBe("96.00 ・ 4/4");
    q('[data-test="tempo-toggle"]')!.click();
    expect(app.sent.at(-1)).toEqual({ type: "setTimelineOpen", open: true });
  });

  it("点を押したまま動かす: 画面だけが動き、離して 1 つ送る。Esc でやめる", () => {
    show(scene("tempo"));
    const row = q('[data-test="tempo-row"]')!;
    pointer(row, "pointerdown", 512, 12);
    pointer(row, "pointermove", 512 + 17, 6);
    expect(text(q('[data-test="tempo-reading"]'))).toBe("9. 2. 0000");
    expect(app.sent.some((c) => c.type === "moveTempoPoint")).toBe(false);
    pointer(row, "pointerup", 512 + 17, 6);
    expect(app.sent.at(-1)).toEqual({ type: "moveTempoPoint", tick: THREE_FOUR, toTick: THREE_FOUR + TPQ, bpm: 126 });

    pointer(row, "pointerdown", 512, 12);
    pointer(row, "pointermove", 600, 12);
    expect(cancelDrag()).toBe(true);
    flushSync();
    pointer(row, "pointerup", 600, 12);
    expect(app.sent.filter((c) => c.type === "moveTempoPoint")).toHaveLength(1);
  });

  it("2 回押すと点を足す。点の上なら値の枠で、Enter で 1 つ送る", () => {
    show(scene("tempo"));
    const row = q('[data-test="tempo-row"]')!;
    pointer(row, "dblclick", 6 * 64 + 2, 30);
    expect(app.sent.at(-1)).toEqual({ type: "addTempoPoint", tick: 6 * BAR, bpm: 112 });
    pointer(row, "dblclick", 512, 12);
    const field = q('[data-test="tempo-point-field"]') as HTMLInputElement;
    field.value = "130";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    field.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    flushSync();
    expect(app.sent.at(-1)).toEqual({ type: "moveTempoPoint", tick: THREE_FOUR, toTick: THREE_FOUR, bpm: 130 });
  });

  it("右クリック: 「次の点まで、だんだん」（チェック）と「消す」", () => {
    show(scene("tempo"));
    const row = q('[data-test="tempo-row"]')!;
    row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 256, clientY: 32 }));
    flushSync();
    const rows = contextMenu.open!.entries.filter((e): e is MenuRow => e.kind === "row");
    expect(rows.map((r) => [r.label, r.checked ?? null])).toEqual([
      ["menu.tempoRamp", true],
      ["menu.delete", null],
    ]);
    rows[0]!.run();
    expect(app.sent.at(-1)).toEqual({ type: "setTempoRamp", tick: 4 * BAR, on: false });
  });

  it("旗を動かすと小節の頭に吸いつき、2 回押した空いた所に旗を足す", () => {
    show(scene("tempo"));
    const flag = qa('[data-test="meter-flag"]')[1]!;
    pointer(flag, "pointerdown", 512);
    const row = q('[data-test="meter-row"]')!;
    pointer(row, "pointermove", 512 + 50);
    pointer(row, "pointerup", 512 + 50);
    expect(app.sent.at(-1)).toEqual({ type: "moveMeterChange", bar: 8, toBar: 9 });
    pointer(row, "dblclick", 100);
    expect(app.sent.at(-1)).toEqual({ type: "addMeterChange", bar: 1, num: 4, den: 4 });
  });
});

describe("マーカーの行（モック R10-Markers）", () => {
  it("名札と、えらんだ名札。押すとそこへ、動かすと拍に吸いつき、読み値は位置", () => {
    show(scene("markerMoving"));
    expect(qa('[data-test="marker-tag"]').map(text)).toEqual(["イントロ", "Aメロ", "サビ"]);
    const tag = qa('[data-test="marker-tag"]')[1]!;
    expect(tag.classList.contains("selected")).toBe(true);
    pointer(tag, "pointerdown", 256);
    const row = q('[data-test="marker-row"]')!;
    pointer(row, "pointerup", 256);
    expect(app.sent.at(-1)).toEqual({ type: "goToMarker", id: 2 });
    pointer(tag, "pointerdown", 256);
    pointer(row, "pointermove", 256 + 66);
    expect(text(q('[data-test="marker-reading"]'))).toBe("6. 1. 0000");
    expect(q(".from")).not.toBeNull();
    pointer(row, "pointerup", 256 + 66);
    expect(app.sent.at(-1)).toEqual({ type: "moveMarker", id: 2, tick: 5 * BAR });
  });

  it("名前の枠: Enter で RenameMarker、Esc で CloseMarkerName", () => {
    show(scene("markerRenaming"));
    const field = q('[data-test="marker-name-field"]') as HTMLInputElement;
    expect(field.value).toBe("Aメロ");
    field.value = "Aメロ ・ 1 回目";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    field.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(app.sent.at(-1)).toEqual({ type: "renameMarker", id: 2, name: "Aメロ ・ 1 回目" });
    field.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(app.sent.at(-1)).toEqual({ type: "closeMarkerName" });
  });

  it("右クリック: ループに ・ 名前を変える F2 ・ 消す Delete", () => {
    show(scene("markerMoving"));
    const tag = qa('[data-test="marker-tag"]')[2]!;
    tag.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    flushSync();
    expect(app.sent.at(-1)).toEqual({ type: "selectTimeline", target: { kind: "marker", id: 3 } });
    const rows = contextMenu.open!.entries.filter((e): e is MenuRow => e.kind === "row");
    expect(rows.map((r) => [r.label, r.key ?? null])).toEqual([
      ["menu.loopFromMarker", null],
      ["menu.renameMarker", "F2"],
      ["menu.delete", "Delete"],
    ]);
    rows[0]!.run();
    expect(app.sent.at(-1)).toEqual({ type: "loopFromMarker", id: 3 });
  });
});

describe("再生まわりの値（再生の線がいる所）", () => {
  it("7 小節目はだんだんの中で 112.00、時間は地図で 00:00:14.446", () => {
    show(scene("tempo"), Transport);
    expect(text(q('[data-test="transport-tempo"]'))).toBe("112.00");
    expect(text(q('[data-test="transport-meter"]'))).toBe("4/4");
    expect(text(q('[data-test="transport-time"]'))).toBe("00:00:14.446");
  });
  it("10 小節目では拍子 3/4、位置は 3/4 の小節で数える", () => {
    const s = scene("tempo");
    s.transport.position = THREE_FOUR + 3 * TPQ + 2 * TPQ;
    show(s, Transport);
    expect(text(q('[data-test="transport-meter"]'))).toBe("3/4");
    expect(text(q('[data-test="transport-tempo"]'))).toBe("120.00");
    expect(text(q('[data-test="transport-position"]'))).toBe("10. 3. 0000");
  });
});

describe("最後にえらんだほう（Delete の向き）と、録音中", () => {
  it("名札や点を押すとテンポ ・ マーカーの側、クリップの場所を押すとクリップの側", () => {
    show(scene("tempo"));
    pickClips();
    pointer(qa('[data-test="marker-tag"]')[0]!, "pointerdown", 0);
    expect(lastPick.kind).toBe("timeline");
    pointer(q(".lanes")!, "pointerdown", 900, 30);
    expect(lastPick.kind).toBe("clips");
    pointer(q('[data-test="tempo-row"]')!, "pointerdown", 512, 12);
    expect(lastPick.kind).toBe("timeline");
  });

  it("録音中は、えらぶ ・ 行くだけ（動かさない、足さない、右クリックの行は使えない）", () => {
    const s = scene("tempo");
    s.transport.state = "recording";
    show(s);
    const row = q('[data-test="tempo-row"]')!;
    pointer(row, "pointerdown", 512, 12);
    pointer(row, "pointermove", 600, 0);
    pointer(row, "pointerup", 600, 0);
    pointer(row, "dblclick", 6 * 64 + 2, 30);
    expect(app.sent.map((c) => c.type)).toEqual(["selectTimeline"]);
    row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 256, clientY: 32 }));
    flushSync();
    const rows = contextMenu.open!.entries.filter((e): e is MenuRow => e.kind === "row");
    expect(rows.every((r) => !r.usable)).toBe(true);
    const tag = qa('[data-test="marker-tag"]')[1]!;
    const markers = q('[data-test="marker-row"]')!;
    pointer(tag, "pointerdown", 256);
    pointer(markers, "pointermove", 400);
    pointer(markers, "pointerup", 400);
    expect(app.sent.at(-1)).toEqual({ type: "goToMarker", id: 2 });
  });
});
