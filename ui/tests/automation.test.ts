// The automation band screen (spec 2 (Automation point, Automation band heading, write, resting, Handle moved by automation),
// spec 3 (Y16), mocks F6-AutoRec / F6-AutoRest / F6-MixerAuto / F6-MixerRest / R10-AutoEdit).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, AutomationView, LaneView, TrackView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import * as A from "../src/components/automation/autoGeom";
import { laneDeleteCommand } from "../src/components/automation/laneKeys";
import { bareText, handPosition, paramAt, paramPosition, positionText, valueAt } from "../src/components/automation/laneValue";
import { blockAt, clipBox, rowTop, trackRowAt, type LaneDims as ClipDims } from "../src/components/arrangeGeom";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { closeContextMenu, contextMenu } from "../src/components/menu/contextMenu.svelte";
import { pickClips, pickLane } from "../src/components/timeline/pick";
import { moveSlotAt } from "../src/components/tracks.svelte";
import { EFFECT_PARAMS } from "../src/bridge/types";
import { mapOf } from "../src/time/bars";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import { t } from "../src/text";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 4 * TPQ;
const scene = (name: string) => fakeScene(name)!.state;
const lane = (ticks: number[], values: number[], step = false) => ({ id: 1, step, ticks: Float64Array.from(ticks), values: Float32Array.from(values) });

describe("value on the line (21-5)", () => {
  const p = lane([BAR, 3 * BAR, 5 * BAR], [0.2, 0.6, 0.4]);
  it("straight; at the ends, the first and last points; null with no points", () => {
    expect(valueAt(p, 0)).toBeCloseTo(0.2);
    expect(valueAt(p, 2 * BAR)).toBeCloseTo(0.4);
    expect(valueAt(p, 4 * BAR)).toBeCloseTo(0.5);
    expect(valueAt(p, 9 * BAR)).toBeCloseTo(0.4);
    expect(valueAt(lane([], []), 0)).toBeNull();
  });
  it("a staircase uses the nearest point at or before the position", () => {
    const s = lane([BAR, 3 * BAR], [0, 1], true);
    expect([valueAt(s, 3 * BAR - 1), valueAt(s, 3 * BAR), valueAt(s, 0)]).toEqual([0, 1, 0]);
  });
});

describe("position text", () => {
  it("gain and sends use the spec 2 scale marks, pan is (pan + 100) / 200, mute is on / off", () => {
    expect(positionText({ kind: "volume" }, 0.8)).toBe("0.0 dB");
    expect(positionText({ kind: "volume" }, 0.64)).toBe("−4.8 dB");
    expect(positionText({ kind: "send", bus: "a" }, 0)).toBe("−∞ dB");
    expect(positionText({ kind: "pan" }, 0.4)).toBe("L 20");
    expect(positionText({ kind: "mute" }, 1)).toBe("オン");
    expect(positionText({ kind: "plugin", plugin: "Surge XT", param: "A" }, 0.5)).toBeNull();
    expect(bareText({ kind: "volume" }, 0.64)).toBe("−4.8");
  });
  it("built-in effects use the bar position in the params panel. Steps are n / (steps − 1)", () => {
    const threshold = EFFECT_PARAMS.compressor[0]!;
    expect(paramPosition(threshold, paramAt(threshold, 0.25))).toBeCloseTo(0.25, 2);
    const steps = EFFECT_PARAMS.delay.find((x) => x.shape === "steps")!;
    expect(paramAt(steps, 1)).toBe(steps.steps.length - 1);
    expect(paramPosition(steps, 1)).toBeCloseTo(1 / (steps.steps.length - 1));
    const text = positionText({ kind: "builtin", effect: "compressor", param: 0 }, 0.5)!;
    expect(bareText({ kind: "builtin", effect: "compressor", param: 0 }, 0.5)).toBe(text.replace(/ dB$/, ""));
  });
  it("position of a value set by hand", () => {
    const t = { gainDb: 0, pan: -20, sends: [null, 0], muted: true, slots: [null, null, null, null] } as unknown as TrackView;
    expect(handPosition(t, { kind: "volume" }, null)).toBeCloseTo(0.8);
    expect(handPosition(t, { kind: "pan" }, null)).toBeCloseTo(0.4);
    expect(handPosition(t, { kind: "send", bus: "a" }, null)).toBe(0);
    expect(handPosition(t, { kind: "mute" }, null)).toBe(1);
    expect(handPosition(t, { kind: "instrument", param: 3 }, null)).toBeNull();
  });
});

describe("track rows and band order", () => {
  const view = (target: LaneView["target"]): LaneView => ({ target, name: { kind: "volume" }, laneId: 1, points: 0, step: false, resting: false });
  const tracks = [
    { id: 1, lanes: [view({ kind: "volume" }), view({ kind: "pan" })] },
    { id: 2, lanes: [] },
  ] as unknown as TrackView[];
  const d = { trackH: 64, laneH: 48, rowH: 24 };
  it("an open band is 48 each, folded bands share the 「▸」 row (24). A track with no bands has no row", () => {
    const r = A.arrangeRows(tracks, [{ home: { kind: "track", track: 1 }, target: { kind: "pan" } }], d);
    expect(r.tops).toEqual([0, 64 + 48 + 24]);
    expect(r.lanes.map((l) => l.top)).toEqual([64]);
    expect(r.rows.map((x) => x.top)).toEqual([112]);
    expect(r.height).toBe(136 + 64);
  });
  it("clip boxes, the row under the press, and groups", () => {
    const dims: ClipDims = { tops: [0, 136], trackH: 64, inset: 2, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
    expect(rowTop(dims, 1)).toBe(136);
    expect(clipBox({ pxPerTick: 1, scrollX: 0, width: 100 }, dims, 1, 0, 10).y0).toBe(138);
    expect([trackRowAt(dims, 10), trackRowAt(dims, 80), trackRowAt(dims, 140), trackRowAt(dims, 300)]).toEqual([0, -1, 1, -1]);
    expect([blockAt(dims, 80), blockAt(dims, 140), blockAt(dims, 900)]).toEqual([0, 1, 1]);
  });
  it("track reorder goes to the nearest row border", () => {
    expect(moveSlotAt(0, [0, 136, 200], 64 / 2 + 150)).toEqual({ slot: 2, to: 1 });
    expect(moveSlotAt(1, [0, 136, 200], 168 - 150)).toEqual({ slot: 0, to: 0 });
  });
});

describe("points in the band", () => {
  const d: A.LaneDims = { h: 48, pad: 4, hair: 1, point: 6 };
  const v = { pxPerTick: 48 / BAR, scrollX: 0, width: 400 };
  const map = mapOf({ tempoBpm: 120, meterNum: 4, meterDen: 4, headRamp: false, tempoPoints: [], meterChanges: [] } as never);
  const pts: A.Pt[] = [
    { tick: 0, value: 0.5 },
    { tick: 2 * BAR, value: 0.8 },
    { tick: 4 * BAR, value: 0.2 },
  ];
  it("value and height: 1 is top, 0 is bottom (pad above and below)", () => {
    expect([A.valueToY(d, 1), A.valueToY(d, 0)]).toEqual([4, 43]);
    expect(A.yToValue(d, A.valueToY(d, 0.5))).toBeCloseTo(0.5, 1);
  });
  it("moving a point: sideways to beat starts, up and down for value. Shift gives ticks. The first point stops at the song start", () => {
    const start: A.PointDrag = { points: [1], grab: 1, downX: 96, downY: 10, moved: false, dTick: 0, dValue: 0 };
    const moved = A.dragPoints(start, pts, v, d, map, 96 + 13, 10 + 10, 4, false);
    expect(moved.dTick).toBe(TPQ);
    expect(moved.dValue).toBeCloseTo(-10 / 39);
    expect(A.dragPoints(start, pts, v, d, map, 97, 11, 4, false)).toBe(start);
    const first: A.PointDrag = { ...start, points: [0, 1], grab: 1 };
    expect(A.dragPoints(first, pts, v, d, map, 0, 10, 4, false).dTick).toBe(0);
  });
  it("points moved past the top or the bottom stop together and keep their distances (as Rust's move_points)", () => {
    const up = A.dragPoints({ points: [0, 1], grab: 1, downX: 96, downY: 40, moved: false, dTick: 0, dValue: 0 }, pts, v, d, map, 96, 40 - 30, 4, true);
    expect(up.dValue).toBeCloseTo(0.2);
    expect(A.draggedPoints(pts, up).pts.map((p) => +p.value.toFixed(3))).toEqual([0.7, 1, 0.2]);
    const down = A.dragPoints({ points: [1, 2], grab: 1, downX: 96, downY: 10, moved: false, dTick: 0, dValue: 0 }, pts, v, d, map, 96, 10 + 30, 4, true);
    expect(down.dValue).toBeCloseTo(-0.2);
    expect(A.draggedPoints(pts, down).pts.map((p) => +p.value.toFixed(3))).toEqual([0.5, 0.6, 0]);
  });
  it("a point moved onto the tick of another point swaps with it", () => {
    const drag: A.PointDrag = { points: [1], grab: 1, downX: 0, downY: 0, moved: true, dTick: 2 * BAR, dValue: 0 };
    const shown = A.draggedPoints(pts, drag);
    expect(shown.pts.map((p) => p.tick)).toEqual([0, 4 * BAR]);
    expect(shown.moving).toEqual([false, true]);
  });
  it("the line is gray.9, resting is gray.7. Points before writing have only an accent border, chosen points are accent", () => {
    const look: A.LaneLook = { resting: false, armed: false, selected: (i) => i === 2, hovered: -1, from: [], flat: null };
    const rects = A.laneRects(v, pts, false, d, look);
    expect(rects.some((r) => r.color === "color.gray.9")).toBe(true);
    expect(rects.filter((r) => r.w === 6 && r.h === 6).map((r) => r.color)).toEqual(["color.gray.10", "color.gray.10", "color.accent.ink"]);
    expect(A.laneRects(v, pts, false, d, { ...look, resting: true }).some((r) => r.color === "color.gray.9")).toBe(false);
    expect(A.laneRects(v, pts, false, d, { ...look, armed: true, selected: () => false }).some((r) => r.w === 6 && r.h === 6)).toBe(false);
    expect(A.laneRects(v, [], false, d, { ...look, flat: 0.5 })).toHaveLength(1);
  });
  it("pressed point, boxed points, place to add", () => {
    expect(A.pointAt(v, pts, d, 96 + 2, A.valueToY(d, 0.8) + 1, 6)).toBe(1);
    expect(A.pointAt(v, pts, d, 50, 20, 6)).toBe(-1);
    expect(A.pointsInBox(v, pts, d, 90, 0, 200, 48)).toEqual([1, 2]);
    expect(A.addTick(v, map, 48 * 4 + 25, false)).toBe(4 * BAR + 2 * TPQ);
  });
  it("position difference in the reading (the 「+0. 1. 000」 form of mock R10-AutoEdit; ticks have 4 digits)", () => {
    expect(A.deltaText(TPQ, BAR, TPQ)).toBe("+0. 1. 0000");
    expect(A.deltaText(-(BAR + TPQ / 2), BAR, TPQ)).toBe("−1. 0. 0960");
  });
});

describe("Delete (when the last thing chosen is an automation point)", () => {
  const auto: AutomationView = { folded: [], armed: [], writing: [], held: [], selected: { laneId: 510, points: [{ start: 1, end: 4 }] } };
  const key = { key: "Delete", ctrl: false, meta: false, alt: false, shift: false };
  const c = { typing: false, busy: false, toArrange: true };
  it("deletes the chosen points. If a clip was chosen last, it goes to the clip", () => {
    pickLane();
    expect(laneDeleteCommand(key, c, auto)).toEqual({ type: "deleteAutoPoints", lane: 510, points: [1, 2, 3] });
    expect(laneDeleteCommand(key, { ...c, typing: true }, auto)).toBeNull();
    expect(laneDeleteCommand(key, c, { ...auto, selected: null })).toBe("none");
    pickClips();
    expect(laneDeleteCommand(key, c, auto)).toBeNull();
  });
});

// ---- parts ----

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});
function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

describe("arrange bands (mocks F6-AutoRec / F6-AutoRest)", () => {
  it("the heading has 2 lines. While writing, 「● 書く」 is on with 「書いています」. Below, 「＋ 帯を足す ▾」 and the folded count", () => {
    show(Arrange, scene("autoRec"));
    expect(qa("[data-test=lane-head]")).toHaveLength(1);
    expect(text(q("[data-test=lane-fold]"))).toBe("▾ オートメーション ・ 音量");
    expect(q("[data-test=lane-write]")!.getAttribute("aria-pressed")).toBe("true");
    expect(text(q("[data-test=lane-state]"))).toBe("書いています");
    expect(text(q("[data-test=lane-add-row]"))).toBe("＋ 帯を足す ▾ 1");
    expect(qa("[data-test=auto-lane]")).toHaveLength(1);
    expect(q("[data-test=auto-writing]")).not.toBeNull();
  });
  it("the rec border is not written again for every meter message (only its frame callback reads the play line)", () => {
    show(Arrange, scene("autoRec"));
    const frame = q("[data-test=auto-writing]")!;
    let width = frame.style.width;
    let writes = 0;
    Object.defineProperty(frame.style, "width", {
      configurable: true,
      get: () => width,
      set: (w: string) => {
        writes += 1;
        width = w;
      },
    });
    const at = scene("autoRec").transport.position;
    for (let i = 0; i < 5; i++) {
      app.setTelemetry(app.telemetryWith({ seq: i + 1, position: { tick: at, ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 } }));
      flushSync();
    }
    expect(writes).toBe(0);
  });
  it("resting shows 「もどす」 and 「休み ・ （いまの値）」. Pressing sends RestoreLane. ▾ folds, pan in the 「＋ 帯を足す ▾」 list adds a band", () => {
    show(Arrange, scene("autoRest"));
    expect(text(q("[data-test=lane-state]"))).toBe("休み ・ −12.0 dB");
    q("[data-test=lane-restore]")!.click();
    q("[data-test=lane-fold]")!.click();
    q("[data-test=lane-add]")!.click();
    const menu = contextMenu.open!.entries;
    // spec 3 (92-5): gain / pan / mute / send A / send B, then effect parameters…. The added band (gain) has the choice mark.
    expect(menu.map((e) => (e.kind === "row" ? [e.text ?? t(e.label), e.checked ?? false] : "—"))).toEqual([
      ["音量", true],
      ["パン", true],
      ["ミュート", false],
      ["センド A", false],
      ["センド B", false],
      "—",
      ["エフェクトのパラメータ…", false],
    ]);
    (menu[1] as { run: () => void }).run();
    closeContextMenu();
    const track = scene("autoRest").song.tracks[0]!.id;
    expect(app.sent).toEqual([
      { type: "restoreLane", home: { kind: "track", track }, target: { kind: "volume" } },
      { type: "foldLane", home: { kind: "track", track }, target: { kind: "volume" }, folded: true },
      { type: "addLane", home: { kind: "track", track }, target: { kind: "pan" } },
    ]);
  });
  it("under a track with bands, the rows move down (clip name positions too)", () => {
    show(Arrange, scene("autoRec"));
    const tops = qa(".clip-label").map((el) => parseFloat(el.style.top));
    // The clip of track 2 is below 64 + 48 + 24.
    expect(tops.some((y) => y >= 136)).toBe(true);
  });
  it("the band context menu has 「帯を消す」", () => {
    show(Arrange, scene("autoRec"));
    q("[data-test=auto-lane]")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 5 }));
    flushSync();
    expect(contextMenu.open?.entries.map((e) => (e.kind === "row" ? e.label : null))).toEqual(["lane.delete"]);
    closeContextMenu();
  });
});

describe("mixer (mocks F6-MixerAuto / F6-MixerRest)", () => {
  it("a fader being moved has an accent border and 「オートメーション」 below. Resting shows 「休み」 and 「もどす」. Other strips keep the same row height", () => {
    show(MixerStrips, scene("mixerRest"));
    const rows = qa("[data-test=mixer-auto]");
    expect(rows.length).toBe(qa("[data-test=mixer-strip]").length);
    expect(text(rows[0]!)).toBe("オートメーション");
    expect(qa("[data-test=mixer-fader]")[0]!.classList.contains("auto")).toBe(true);
    expect(text(rows[3]!)).toBe("休み もどす");
    expect(qa("[data-test=mixer-fader]")[3]!.classList.contains("auto")).toBe(false);
    q("[data-test=mixer-auto-restore]")!.click();
    expect(app.sent).toEqual([{ type: "restoreLane", home: { kind: "track", track: scene("mixerRest").song.tracks[3]!.id }, target: { kind: "volume" } }]);
  });
  it("with no bands, the row under the value is not shown", () => {
    show(MixerStrips, scene("mixer"));
    expect(qa("[data-test=mixer-auto]")).toHaveLength(0);
  });
});
