// Tempo and time signature changes (band, bar lines, transport values) and markers (row, names, moving, keys, piano roll ruler).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TimelineView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { cancelDrag, dragReading } from "../src/components/pianoroll/view.svelte";
import { timelineKeyCommand } from "../src/components/timeline/keys";
import { lastPick, pickClips, pickTimeline } from "../src/components/timeline/pick";
import * as T from "../src/components/timeline/tempoGeom";
import TempoBand from "../src/components/timeline/TempoBand.svelte";
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

/** 8 bars of 4/4, 4 bars of 3/4 from bar 9, 4/4 from bar 13 (the song of mock F6-Tempo). */
const tempoTimeline = (): TimelineView => scene("tempo").timeline;
const THREE_FOUR = 8 * BAR;
const FOUR_FOUR = THREE_FOUR + 4 * 3 * TPQ;

describe("bar lines (from the tempo map)", () => {
  const map = mapOf(tempoTimeline());

  it("the same map for the same section", () => {
    const tl = tempoTimeline();
    expect(mapOf(tl)).toBe(mapOf(tl));
  });

  it("bar starts line up by the 3/4 length from where the time signature changes", () => {
    const heads: number[] = [];
    eachLine(map, 7 * BAR, FOUR_FOUR + BAR, (_, bar) => bar, (tick, kind) => kind === 0 && heads.push(tick));
    expect(heads).toEqual([7 * BAR, THREE_FOUR, THREE_FOUR + 3 * TPQ, THREE_FOUR + 6 * TPQ, THREE_FOUR + 9 * TPQ, FOUR_FOUR]);
  });

  it("beat lines follow the beats of that bar's time signature", () => {
    const kinds: string[] = [];
    eachLine(map, THREE_FOUR, THREE_FOUR + 3 * TPQ, (beat) => beat, (tick, kind) => kinds.push(`${tick - THREE_FOUR}:${kind}`));
    expect(kinds).toEqual(["0:0", `${TPQ}:1`, `${2 * TPQ}:1`]);
  });

  it("snap: the nearest beat or bar start (the later one at the middle)", () => {
    expect(nearestBeatTick(map, THREE_FOUR + TPQ / 2 - 1)).toBe(THREE_FOUR);
    expect(nearestBeatTick(map, THREE_FOUR + TPQ / 2)).toBe(THREE_FOUR + TPQ);
    expect(nearestBarTick(map, THREE_FOUR + (35 * TPQ) / 24)).toBe(THREE_FOUR);
    expect(nearestBarTick(map, THREE_FOUR + (3 * TPQ) / 2)).toBe(THREE_FOUR + 3 * TPQ);
    expect(nearestBarTick(map, -5)).toBe(0);
    // A 1/16 grid starts from the bar start even in a 3/4 bar. Steps longer than 1 bar go to bar starts.
    expect(snapToStep(map, THREE_FOUR + (13 * TPQ) / 48, TPQ / 4)).toBe(THREE_FOUR + TPQ / 4);
    expect(snapToStep(map, THREE_FOUR + (25 * TPQ) / 12, Infinity)).toBe(THREE_FOUR + 3 * TPQ);
  });

  it("typed position「10.2.0960」counts in 3/4 bars, and the beat goes into 1–3", () => {
    expect(tickOfPosition(map, 10, 2, TPQ / 2)).toBe(THREE_FOUR + 3 * TPQ + TPQ + TPQ / 2);
    expect(readPosition("10.2.0960", map)).toEqual({ value: THREE_FOUR + 3 * TPQ + (3 * TPQ) / 2, clamp: null });
    const r = readPosition("10.4", map)!;
    expect(r.value).toBe(THREE_FOUR + 3 * TPQ + 2 * TPQ);
    expect(r.clamp).toEqual({ field: "beat", value: "10. 3. 0000", range: "1〜3" });
  });

  it("the clip's bar tag「9–12」", () => {
    expect(barsText(THREE_FOUR, FOUR_FOUR, map)).toBe("9–12");
    expect(barsText(0, 4 * BAR, { num: 4, den: 4 })).toBe("1–4");
  });
});

describe("note grid (counted from the start of the time signature section)", () => {
  // 1 bar of 4/4, 2 bars of 3/16 (BAR to BAR + 3 * TPQ / 2), then 4/4.
  const map = mapOf({
    ...tempoTimeline(),
    tempoPoints: [],
    meterChanges: [
      { bar: 1, meterNum: 3, meterDen: 16 },
      { bar: 3, meterNum: 4, meterDen: 4 },
    ],
  });

  it("1/8 triplets (TPQ / 3) count from BAR, and a step over the next flag goes to the flag's bar start (BAR + 3 * TPQ / 2)", () => {
    const t3 = TPQ / 3;
    expect(gridTick(map, BAR + (3 * TPQ) / 8, t3)).toBe(BAR + t3);
    expect(gridTick(map, BAR + (35 * TPQ) / 24, t3)).toBe(BAR + (3 * TPQ) / 2);
    expect(gridTick(map, BAR + (17 * TPQ) / 12, t3, true)).toBe(BAR + 4 * t3);
    expect(gridTick(map, BAR + (13 * TPQ) / 8, t3)).toBe(BAR + (3 * TPQ) / 2);
    expect(gridTick(map, -50, t3)).toBe(0);
  });

  it("a 1-bar grid is the bar start at that place", () => {
    expect(gridTick(map, BAR + (19 * TPQ) / 24, BAR, false, true)).toBe(BAR + (3 * TPQ) / 4);
    expect(gridTick(map, BAR + (19 * TPQ) / 24, BAR, true, true)).toBe(BAR + (3 * TPQ) / 4);
  });

  it("grid lines are at the same places as snapping", () => {
    const ticks: number[] = [];
    eachGridTick(map, BAR - TPQ / 24, BAR + (11 * TPQ) / 6, TPQ / 3, (tick) => ticks.push(tick));
    expect(ticks).toEqual([BAR, BAR + TPQ / 3, BAR + (2 * TPQ) / 3, BAR + TPQ, BAR + (4 * TPQ) / 3, BAR + (3 * TPQ) / 2]);
  });
});

describe("positions in the note list and selected notes (from the map)", () => {
  const map = mapOf(tempoTimeline());
  const take = { starts: [THREE_FOUR + 3 * TPQ + TPQ], lengths: [TPQ], keys: [60], velocities: [90] } as never;
  const clip = { start: 0, end: 20 * BAR, takeOffset: 0 };

  it("position counts in 3/4 bars, and typing does too", () => {
    const spec = noteFieldSpec("position", take, clip, [0], map)!;
    expect(spec.initial).toBe("10.2.0000");
    expect(spec.read("10.3")).toBe(THREE_FOUR + 3 * TPQ + 2 * TPQ);
    expect(spec.read("10.4")).toBeNull();
    expect(selectedSummary(take, clip, [{ start: 0, end: 1 }], map)!.position).toBe("10.2.0000");
  });
});

describe("tempo band shape", () => {
  const tl = tempoTimeline();
  const map = mapOf(tl);

  it("the count is the number of change places (ticks). Headings are「テンポ 96.00 ほか 3」「拍子 4/4 ほか 2」", () => {
    expect(T.changeCount(tl, map)).toBe(3);
    expect(T.summaries(tl)).toEqual({ tempo: "テンポ 96.00 ほか 3", meter: "拍子 4/4 ほか 2" });
    expect(T.summaries(scene("empty").timeline)).toEqual({ tempo: "テンポ 96.00", meter: "拍子 4/4" });
  });

  it("the highest tempo is pad from the top, the lowest pad from the bottom", () => {
    const s = T.tempoScale(T.tempoPoints(tl), 48, 12, 1);
    expect(T.bpmToY(s, 120)).toBe(12);
    expect(T.bpmToY(s, 96)).toBe(36);
    const flat = T.tempoScale([{ tick: 0, bpm: 96, ramp: false }], 48, 12, 1);
    expect(T.bpmToY(flat, 96)).toBe(24);
  });

  it("ramp sections and their lines (a slope is a row of step rectangles)", () => {
    const v = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
    const pts = T.tempoPoints(tl);
    expect(T.rampSpans(v, pts)).toEqual([{ tick: 4 * BAR, x0: 256, x1: 512 }]);
    const s = T.tempoScale(pts, 48, 12, 1);
    const rects = T.tempoRects(v, pts, s, { hair: 1, point: 6 }, (p) => ({ selected: p.tick === 4 * BAR, hovered: false }), null);
    expect(rects.filter((r) => r.color === "color.accent.ink")).toEqual([{ x: 253, y: 25, w: 6, h: 6, color: "color.accent.ink" }]);
    expect(rects.filter((r) => r.color === "color.gray.10")).toHaveLength(3);
  });

  it("drag: sideways to beat starts, up and down for tempo. The song start does not move sideways. Swaps with a point at the same place", () => {
    const pts = T.tempoPoints(tl);
    const s = T.tempoScale(pts, 48, 12, 1);
    const v = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
    const start: T.PointDrag = { tick: THREE_FOUR, fromBpm: 120, toTick: THREE_FOUR, bpm: 120, downX: 512, downY: 12, moved: false };
    const d = T.dragPoint(start, v, s, map, 512 + 17, 12 - 6, 4);
    expect(d).toMatchObject({ moved: true, toTick: THREE_FOUR + TPQ, bpm: 126 });
    const head = T.dragPoint({ ...start, tick: 0, toTick: 0, fromBpm: 96, bpm: 96 }, v, s, map, 600, 12, 4);
    expect(head.toTick).toBe(0);
    const onto = T.draggedPoints(pts, { ...start, moved: true, toTick: FOUR_FOUR, bpm: 110 });
    expect(onto.map((p) => [p.tick, p.bpm])).toEqual([
      [0, 96],
      [4 * BAR, 104],
      [FOUR_FOUR, 110],
    ]);
  });

  it("double press: the lower half gives that beat start and the tempo on the line; the upper half gives that bar", () => {
    const v = { pxPerTick: 64 / BAR, scrollX: 0, width: 1000 };
    expect(T.addAt(v, map, 6 * 64 + 1, "tempo")).toEqual({ tick: 6 * BAR, bpm: 112 });
    expect(T.addAt(v, map, 512 + 60, "meter")).toEqual({ bar: 9, num: 3, den: 4 });
  });
});

describe("keys (spec 4)", () => {
  const k = (key: string, more: Partial<{ alt: boolean; shift: boolean; ctrl: boolean }> = {}) => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...more });
  const tl = scene("markerMoving").timeline;
  const plain = { typing: false, busy: false, toArrange: true };
  const typing = { ...plain, typing: true };

  it("K gives「マーカー 4」. Not while typing text", () => {
    expect(timelineKeyCommand(k("k"), plain, tl)).toEqual({ type: "addMarker", name: "マーカー 4", tick: null });
    expect(timelineKeyCommand(k("k"), typing, tl)).toBeNull();
  });
  it("Alt →, Alt ← work even while typing", () => {
    expect(timelineKeyCommand(k("ArrowRight", { alt: true }), typing, tl)).toEqual({ type: "stepMarker", forward: true });
    expect(timelineKeyCommand(k("ArrowLeft", { alt: true }), plain, tl)).toEqual({ type: "stepMarker", forward: false });
  });
  it("F2 is the name of the chosen marker", () => {
    expect(timelineKeyCommand(k("F2"), plain, tl)).toEqual({ type: "openMarkerName", id: 2 });
    expect(timelineKeyCommand(k("F2"), plain, { ...tl, selected: null })).toBeNull();
  });
  it("Delete goes to the chosen item only when it was chosen after a clip (the song start takes it too, and it is not passed to the clip)", () => {
    pickTimeline();
    expect(timelineKeyCommand(k("Delete"), plain, tl)).toEqual({ type: "deleteTimeline", target: { kind: "marker", id: 2 } });
    const head = { kind: "tempoPoint", tick: 0 } as const;
    expect(timelineKeyCommand(k("Delete"), plain, { ...tl, selected: head })).toEqual({ type: "deleteTimeline", target: head });
    expect(timelineKeyCommand(k("Delete"), { ...plain, toArrange: false }, tl)).toBeNull();
    pickClips();
    expect(timelineKeyCommand(k("Delete"), plain, tl)).toBeNull();
  });
  it("while recording and counting in, nothing is sent", () => {
    pickTimeline();
    const busy = { ...plain, busy: true };
    for (const key of [k("k"), k("F2"), k("Delete"), k("ArrowRight", { alt: true })]) expect(timelineKeyCommand(key, busy, tl)).toBeNull();
  });
});

// ---- parts ----

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
  dragReading.text = null;
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

describe("the arrange row「テンポ ・ 拍子の変更」(mock F6-Tempo, F6-AutoRec)", () => {
  it("open row: heading, flags, values, ramps. The chosen point's value is accent", () => {
    show(scene("tempo"));
    expect(text(q('[data-test="tempo-toggle"]'))).toBe("▾ テンポ ・ 拍子の変更 3 件");
    expect(text(q('[data-test="tempo-summary"]'))).toBe("テンポ 96.00 ほか 3");
    expect(text(q('[data-test="meter-summary"]'))).toBe("拍子 4/4 ほか 2");
    expect(qa('[data-test="meter-flag"]').map(text)).toEqual(["4/4", "3/4", "4/4"]);
    expect(qa('[data-test="meter-flag"]').map((f) => f.style.left)).toEqual(["0px", "512px", "704px"]);
    expect(qa(".value").map(text)).toEqual(["96.00", "104.00", "120.00", "96.00"]);
    expect(qa(".value.selected").map(text)).toEqual(["104.00"]);
    expect(text(q(".ramp"))).toBe("だんだん");
    // The ruler numbers use 48 px for a 3/4 bar.
    const n = qa(".bar-number").find((b) => text(b) === "10")!;
    expect(n.style.left).toBe("560px");
  });

  it("lines and points are inside the arrangeLanes section (no more sections)", () => {
    show(scene("tempo"));
    const row = q('[data-test="tempo-row"]')!;
    expect(row.dataset.glSlot).toBe("rects");
    expect(row.closest("[data-gl-region]")!.getAttribute("data-gl-region")).toBe("arrangeLanes");
    expect(qa("[data-gl-region]").map((r) => r.dataset.glRegion).sort()).toEqual(["arrangeLanes", "recordingClip", "trackMeters"]);
  });

  it("folded row: the count, and the tempo and time signature at the play line. Pressing sends the open intent", () => {
    show(scene("tempoFolded"));
    expect(text(q('[data-test="tempo-toggle"]'))).toBe("▸ テンポ ・ 拍子の変更 1 件");
    expect(text(q('[data-test="tempo-now"]'))).toBe("96.00 ・ 4/4");
    q('[data-test="tempo-toggle"]')!.click();
    expect(app.sent.at(-1)).toEqual({ type: "setTimelineOpen", open: true });
  });

  it("holding and moving a point: only the screen moves, release sends one. Esc cancels", () => {
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

  it("double press adds a point. On a point, a value box; Enter sends one", () => {
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

  it("right click:「次の点まで、だんだん」(check) and「消す」", () => {
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

  it("moving a flag snaps to the bar start; a double press on empty space adds a flag", () => {
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

describe("marker row (mock R10-Markers)", () => {
  it("tags and the chosen tag. Pressing goes there, moving snaps to beats, the reading is the position", () => {
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

  it("name box: Enter sends RenameMarker, Esc CloseMarkerName", () => {
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

  it("right click:「ループに」, the rows of the section (round 12 A2), 「名前を変える」F2, 「消す」Delete", () => {
    show(scene("markerMoving"));
    const tag = qa('[data-test="marker-tag"]')[2]!;
    tag.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    flushSync();
    expect(app.sent.at(-1)).toEqual({ type: "selectTimeline", target: { kind: "marker", id: 3 } });
    const rows = contextMenu.open!.entries.filter((e): e is MenuRow => e.kind === "row");
    expect(rows.map((r) => [r.label, r.key ?? null])).toEqual([
      ["menu.loopFromMarker", null],
      ["menu.duplicateSection", null],
      ["menu.deleteSection", null],
      ["menu.insertSpace", null],
      ["menu.renameMarker", "F2"],
      ["menu.delete", "Delete"],
    ]);
    rows[0]!.run();
    expect(app.sent.at(-1)).toEqual({ type: "loopFromMarker", id: 3 });
  });
});

describe("dragging markers, tempo points and time signature flags", () => {
  /** The scene's timeline with one thing changed, as core's answer to a move. */
  const answered = (edit: (tl: TimelineView) => void): AppState => {
    const s = scene("tempo");
    edit(s.timeline);
    return s;
  };
  const tagLeft = (name: string) => qa('[data-test="marker-tag"]').find((tag) => text(tag) === name)!.style.left;

  it("with Shift, a tempo point and a marker still go to the start of a beat, as core puts them (spec 3 Y17, Y18; architecture 21-15)", () => {
    show(scene("tempo"));
    const row = q('[data-test="tempo-row"]')!;
    pointer(row, "pointerdown", 512, 12);
    pointer(row, "pointermove", 512 + 17, 6, { shiftKey: true });
    expect(text(q('[data-test="tempo-reading"]'))).toBe("9. 2. 0000");
    pointer(row, "pointerup", 512 + 17, 6, { shiftKey: true });
    expect(app.sent.at(-1)).toEqual({ type: "moveTempoPoint", tick: THREE_FOUR, toTick: THREE_FOUR + TPQ, bpm: 126 });

    const tag = qa('[data-test="marker-tag"]')[1]!;
    const markers = q('[data-test="marker-row"]')!;
    pointer(tag, "pointerdown", 256);
    pointer(markers, "pointermove", 256 + 66, 10, { shiftKey: true });
    expect(text(q('[data-test="marker-reading"]'))).toBe("6. 1. 0000");
    pointer(markers, "pointerup", 256 + 66, 10, { shiftKey: true });
    expect(app.sent.at(-1)).toEqual({ type: "moveMarker", id: 2, tick: 5 * BAR });
  });

  it("the reading in place is on the status line too; it goes on release and on Esc (spec 2 「その場の読み値」)", () => {
    show(scene("tempo"));
    const row = q('[data-test="tempo-row"]')!;
    pointer(row, "pointerdown", 512, 12);
    pointer(row, "pointermove", 512 + 17, 6);
    expect(dragReading.text).toBe("9. 2. 0000");
    pointer(row, "pointerup", 512 + 17, 6);
    expect(dragReading.text).toBeNull();

    const meters = q('[data-test="meter-row"]')!;
    pointer(qa('[data-test="meter-flag"]')[2]!, "pointerdown", 704);
    pointer(meters, "pointermove", 704 + 64);
    expect(dragReading.text).toBe(text(q('[data-test="tempo-reading"]')));
    expect(dragReading.text).toBe("14. 1. 0000");
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(dragReading.text).toBeNull();
    pointer(meters, "pointerup", 704 + 64);

    const markers = q('[data-test="marker-row"]')!;
    pointer(qa('[data-test="marker-tag"]')[1]!, "pointerdown", 256);
    pointer(markers, "pointermove", 256 + 66);
    expect(dragReading.text).toBe("6. 1. 0000");
    pointer(markers, "pointerup", 256 + 66);
    expect(dragReading.text).toBeNull();
  });

  it("after release a marker keeps its moved place until core's answer; the answer or a rev with no change ends it", async () => {
    show(scene("tempo"));
    const markers = q('[data-test="marker-row"]')!;
    pointer(qa('[data-test="marker-tag"]')[1]!, "pointerdown", 256);
    pointer(markers, "pointermove", 256 + 66);
    pointer(markers, "pointerup", 256 + 66);
    expect(tagLeft("Aメロ")).toBe("320px");
    expect(q(".from")).not.toBeNull();
    app.setState(answered((tl) => (tl.markers[1]!.tick = 5 * BAR)));
    flushSync();
    expect(tagLeft("Aメロ")).toBe("320px");
    expect(q(".from")).toBeNull();

    // An answer that changed nothing (the same rev): back to where the marker is.
    app.setState(scene("tempo"));
    flushSync();
    pointer(qa('[data-test="marker-tag"]')[1]!, "pointerdown", 256);
    pointer(markers, "pointermove", 256 + 66);
    pointer(markers, "pointerup", 256 + 66);
    expect(tagLeft("Aメロ")).toBe("320px");
    await Promise.resolve();
    await Promise.resolve();
    flushSync();
    expect(tagLeft("Aメロ")).toBe("256px");
  });

  it("after release a tempo point and a time signature flag keep their moved place until core's answer", () => {
    show(scene("tempo"));
    const row = q('[data-test="tempo-row"]')!;
    pointer(row, "pointerdown", 512, 12);
    pointer(row, "pointermove", 512 + 17, 6);
    pointer(row, "pointerup", 512 + 17, 6);
    expect(qa(".value").map(text)).toEqual(["96.00", "104.00", "126.00", "96.00"]);
    expect(qa(".value")[2]!.style.left).toBe(`${512 + 16 + 6}px`);
    app.setState(
      answered((tl) => {
        tl.tempoPoints[1] = { tick: THREE_FOUR + TPQ, tempoBpm: 126, ramp: false };
        tl.selected = { kind: "tempoPoint", tick: THREE_FOUR + TPQ };
      }),
    );
    flushSync();
    expect(qa(".value").map(text)).toEqual(["96.00", "104.00", "126.00", "96.00"]);
    expect(qa(".value.selected").map(text)).toEqual(["126.00"]);

    const meters = q('[data-test="meter-row"]')!;
    pointer(qa('[data-test="meter-flag"]')[1]!, "pointerdown", 512);
    pointer(meters, "pointermove", 512 + 50);
    pointer(meters, "pointerup", 512 + 50);
    expect(app.sent.at(-1)).toEqual({ type: "moveMeterChange", bar: 8, toBar: 9 });
    expect(qa('[data-test="meter-flag"]').map((f) => f.style.left)).toEqual(["0px", "560px", "704px"]);
    expect(q(".flag-from")).not.toBeNull();
    app.setState(answered((tl) => (tl.meterChanges[0]!.bar = 9)));
    flushSync();
    expect(qa('[data-test="meter-flag"]').map((f) => f.style.left)).toEqual(["0px", "576px", "720px"]);
    expect(q(".flag-from")).toBeNull();
  });

  it("an unreadable lower number typed on a flag is told as the lower number, as the transport's box does (spec 6)", () => {
    show(scene("tempo"));
    qa('[data-test="meter-flag"]')[1]!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true }));
    flushSync();
    const field = q('[data-test="meter-flag-field"]') as HTMLInputElement;
    for (const [typed, rejected] of [
      ["3/x", { field: "meterDen", kept: "4" }],
      ["x/4", { field: "meterNum", kept: "3" }],
    ] as const) {
      field.value = typed;
      field.dispatchEvent(new Event("input", { bubbles: true }));
      field.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      flushSync();
      expect(app.sent.at(-1)).toEqual({ type: "rejectValue", ...rejected, range: null });
    }
  });

  it("the folded row's tempo and time signature at the play line stay at the row's left edge when the arrange is scrolled", () => {
    const s = scene("tempoFolded");
    app.setState(s);
    const view = { pxPerTick: 64 / BAR, scrollX: 500, width: WIDTH };
    mounted = mount(TempoBand, { target, props: { view, map: mapOf(s.timeline), timeline: s.timeline, now: "96.00 ・ 4/4", busy: false } });
    flushSync();
    expect(text(q('[data-test="tempo-now"]'))).toBe("96.00 ・ 4/4");
    expect(q('[data-test="tempo-now"]')!.style.left).toBe("");
  });
});

describe("transport values (where the play line is)", () => {
  it("bar 7 is in the ramp at 112.00; time by the map is 00:00:14.446", () => {
    show(scene("tempo"), Transport);
    expect(text(q('[data-test="transport-tempo"]'))).toBe("112.00");
    expect(text(q('[data-test="transport-meter"]'))).toBe("4/4");
    expect(text(q('[data-test="transport-time"]'))).toBe("00:00:14.446");
  });
  it("at bar 10 the time signature is 3/4, and the position counts in 3/4 bars", () => {
    const s = scene("tempo");
    s.transport.position = THREE_FOUR + 3 * TPQ + 2 * TPQ;
    show(s, Transport);
    expect(text(q('[data-test="transport-meter"]'))).toBe("3/4");
    expect(text(q('[data-test="transport-tempo"]'))).toBe("120.00");
    expect(text(q('[data-test="transport-position"]'))).toBe("10. 3. 0000");
  });
});

describe("the last chosen side (where Delete goes), and recording", () => {
  it("pressing a tag or point picks the tempo and marker side; pressing the clip area picks the clip side", () => {
    show(scene("tempo"));
    pickClips();
    pointer(qa('[data-test="marker-tag"]')[0]!, "pointerdown", 0);
    expect(lastPick.kind).toBe("timeline");
    pointer(q(".lanes")!, "pointerdown", 900, 30);
    expect(lastPick.kind).toBe("clips");
    pointer(q('[data-test="tempo-row"]')!, "pointerdown", 512, 12);
    expect(lastPick.kind).toBe("timeline");
  });

  it("while recording, only choose and go (no moving, no adding, right click rows disabled)", () => {
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
