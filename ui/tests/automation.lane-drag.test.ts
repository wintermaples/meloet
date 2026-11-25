// Moving points of a mounted automation lane: the look kept after release stays on the points it was made on, while
// the answer gives the lane a new id whose points come a moment later (architecture 21-5: a new id whenever the points change).
import { flushSync, mount, unmount } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, LanePoints } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeLane } from "../src/bridge/lane";
import Arrange from "../src/components/Arrange.svelte";
import { cancelDrag, dragReading } from "../src/components/pianoroll/view.svelte";
import { watchSlots, type GlRect, type GlSlot } from "../src/gl/slot";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const POINTS = new Map<number, LanePoints>();
/** Lanes asked for whose points are not here yet: a test gives them by calling the resolver. */
const asked = new Map<number, (p: LanePoints) => void>();
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  peekLanePoints: (id: number) => POINTS.get(id),
  lanePoints: (id: number) => (POINTS.has(id) ? Promise.resolve(POINTS.get(id)!) : new Promise<LanePoints>((resolve) => asked.set(id, resolve))),
}));

const BAR = 4 * TPQ;
const OLD = 510;
const lanePoints = (id: number, list: [number, number][]): LanePoints => ({
  id,
  step: false,
  ticks: Float64Array.from(list.map((p) => p[0])),
  values: Float32Array.from(list.map((p) => p[1])),
});
const LIST: [number, number][] = [
  [0, 0.64],
  [2 * BAR, 0.73],
  [3 * BAR, 0.6],
  [5 * BAR, 0.45],
];

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => 1000 }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
const slots = new Map<HTMLElement, GlSlot>();
let stopWatch: () => void = () => {};
beforeEach(() => {
  app.reset();
  POINTS.clear();
  asked.clear();
  POINTS.set(OLD, lanePoints(OLD, LIST));
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
  stopWatch();
  target.remove();
  dragReading.text = null;
});

const settle = async () => {
  for (let i = 0; i < 4; i++) await Promise.resolve();
  flushSync();
};
const lane = () => target.querySelector<HTMLElement>("[data-test=auto-lane]")!;
const rects = (): readonly GlRect[] => {
  const s = slots.get(lane());
  return s?.kind === "rects" ? s.rects : [];
};
/** The middles of the drawn point squares (6×6), left to right. */
const squares = (): { x: number; y: number }[] =>
  rects()
    .filter((r) => r.w === 6 && r.h === 6)
    .map((r) => ({ x: r.x + 3, y: r.y + 3 }))
    .sort((a, b) => a.x - b.x);
/** The accent border at the original place of moved points (4 hair sides each). */
const fromFrames = () => rects().filter((r) => r.color === "color.accent.ink" && ((r.w === 6 && r.h === 1) || (r.w === 1 && r.h === 6))).length;
function pointer(type: string, x: number, y: number): void {
  lane().dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 }));
  flushSync();
}
/** The same state, with the lane under a new id (the patch that answers a change of its points). */
function withLaneId(s: AppState, id: number): AppState {
  const next = structuredClone(s);
  next.song.tracks[0]!.lanes[0]!.laneId = id;
  next.automation.selected = { laneId: id, points: [{ start: 1, end: 2 }] };
  return next;
}

describe("moving a point of an open lane (UI-ARRANGE-001)", () => {
  it("after release, the moved look stays on the old points until the new lane's points arrive; no error, and not moved twice", async () => {
    const state = fakeScene("autoMove")!.state;
    app.setState(state);
    mounted = mount(Arrange, { target });
    flushSync();
    await settle();
    const before = squares();
    expect(before).toHaveLength(LIST.length);
    const beat = (before[1]!.x - before[0]!.x) / 8;
    const grab = before[1]!;
    pointer("pointerdown", grab.x, grab.y);
    pointer("pointermove", grab.x + beat, grab.y);
    pointer("pointerup", grab.x + beat, grab.y);
    expect(app.sent.at(-1)).toMatchObject({ type: "moveAutoPoints", lane: OLD, points: [1], dTick: TPQ });
    const moved = squares().map((p) => p.x);
    expect(moved).toEqual([before[0]!.x, grab.x + beat, before[2]!.x, before[3]!.x]);
    expect(fromFrames()).toBe(4);

    // The answer: the lane under a new id whose points are not here yet.
    app.setState(withLaneId(state, 512));
    expect(() => flushSync()).not.toThrow();
    expect(squares().map((p) => p.x)).toEqual(moved);
    expect(fromFrames()).toBe(4);

    const answered: [number, number][] = LIST.map(([tick, value], i) => [i === 1 ? tick + TPQ : tick, value]);
    POINTS.set(512, lanePoints(512, answered));
    asked.get(512)?.(POINTS.get(512)!);
    await settle();
    expect(squares().map((p) => p.x)).toEqual(moved);
    expect(fromFrames()).toBe(0);
  });

  it("a point held while the lane gets a new id (undo, the AI link) lets go of the drag: no error, nothing sent", async () => {
    const state = fakeScene("autoMove")!.state;
    app.setState(state);
    mounted = mount(Arrange, { target });
    flushSync();
    await settle();
    const before = squares();
    const beat = (before[1]!.x - before[0]!.x) / 8;
    const grab = before[3]!;
    pointer("pointerdown", grab.x, grab.y);
    pointer("pointermove", grab.x + beat, grab.y);
    POINTS.set(514, lanePoints(514, LIST.slice(0, 2)));
    app.setState(withLaneId(state, 514));
    expect(() => flushSync()).not.toThrow();
    expect(() => pointer("pointermove", grab.x + 2 * beat, grab.y)).not.toThrow();
    pointer("pointerup", grab.x + 2 * beat, grab.y);
    expect(app.sent.some((c) => c.type === "moveAutoPoints")).toBe(false);
    expect(squares()).toHaveLength(2);
  });
});

describe("the fake bridge moves points as Rust does", () => {
  it("the moved points under a new lane id, chosen; values move together inside 0 to 1, and the first point stops at the song start", async () => {
    const f = createFakeTransport(fakeScene("autoMove")!.state);
    const before = (await f.snapshot()).state as AppState;
    const old = before.song.tracks[0]!.lanes[0]!.laneId;
    const was = decodeLane(await f.lane(old));
    await f.send({ type: "moveAutoPoints", lane: old, points: [0, 1], dTick: -TPQ, dValue: 0.5 });
    const s = (await f.snapshot()).state as AppState;
    const lane = s.song.tracks[0]!.lanes[0]!;
    expect(lane.laneId).not.toBe(old);
    expect(s.automation.selected).toEqual({ laneId: lane.laneId, points: [{ start: 0, end: 2 }] });
    expect(s.song.undoCount).toBe(before.song.undoCount + 1);
    const got = decodeLane(await f.lane(lane.laneId));
    expect([...got.ticks]).toEqual([...was.ticks]);
    const up = 1 - Math.max(was.values[0]!, was.values[1]!);
    expect([...got.values].map((v) => +v.toFixed(4))).toEqual([...was.values].map((v, i) => +(i < 2 ? v + up : v).toFixed(4)));
    expect(lane.points).toBe(got.ticks.length);
  });
});

describe("the reading of a lane is on the status line too (UI-PARITY-001)", () => {
  it("moving a point, and holding an empty place, put the in-place reading's text on the status line; release and Esc take it away", async () => {
    app.setState(fakeScene("autoMove")!.state);
    mounted = mount(Arrange, { target });
    flushSync();
    await settle();
    const before = squares();
    const beat = (before[1]!.x - before[0]!.x) / 8;
    const grab = before[1]!;
    const reading = () => target.querySelector("[data-test=auto-reading]")?.textContent?.trim() ?? null;
    pointer("pointerdown", grab.x, grab.y);
    pointer("pointermove", grab.x + beat, grab.y);
    expect(dragReading.text).toMatch(/^\+0\. 1\. 0000 ・ /);
    expect(dragReading.text).toBe(reading());
    pointer("pointerup", grab.x + beat, grab.y);
    expect(dragReading.text).toBeNull();

    pointer("pointerdown", grab.x, grab.y);
    pointer("pointermove", grab.x + 2 * beat, grab.y);
    expect(dragReading.text).toMatch(/^\+0\. 2\. 0000 ・ /);
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(dragReading.text).toBeNull();

    pointer("pointerdown", before[2]!.x + 2 * beat, 10);
    expect(dragReading.text).toMatch(/^4\. 3\. 0000 ・ /);
    expect(dragReading.text).toBe(reading());
    pointer("pointerup", before[2]!.x + 2 * beat, 10);
    expect(app.sent.at(-1)).toMatchObject({ type: "addAutoPoint", tick: 3 * BAR + 2 * TPQ });
    expect(dragReading.text).toBeNull();
  });
});
