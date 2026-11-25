// The M of a track head shows what the M of its mixer strip shows: while a mute lane moves, the lane's value at the
// play line; otherwise the mute set by hand (docs/spec/7-readings.md A-21).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, LanePoints, LaneView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import TrackHeads from "../src/components/TrackHeads.svelte";
import { refreshPositions } from "../src/components/automation/autoState.svelte";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const POINTS = new Map<number, LanePoints>();
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  peekLanePoints: (id: number) => POINTS.get(id),
  lanePoints: (id: number) => Promise.resolve(POINTS.get(id)!),
}));

const MUTE = { kind: "mute" } as const;
/** A mute lane with one point at `value`. */
function muteLane(laneId: number, value: number): LaneView {
  POINTS.set(laneId, { id: laneId, step: true, ticks: Float64Array.from([0]), values: Float32Array.from([value]) });
  return { target: MUTE, name: MUTE, laneId, points: 1, step: true, resting: false };
}

/** Track 1: not muted by hand, its lane mutes. Track 2: muted by hand, its lane plays. Track 3: muted by hand, no lane. */
function laned(): AppState {
  const s = fakeScene("mixer")!.state;
  const [one, two, three] = s.song.tracks;
  one!.muted = false;
  one!.lanes = [muteLane(901, 1)];
  two!.muted = true;
  two!.lanes = [muteLane(902, 0)];
  three!.muted = true;
  three!.lanes = [];
  return s;
}

let mounted: Record<string, unknown>[] = [];
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  for (const m of mounted) unmount(m);
  mounted = [];
  target.remove();
  POINTS.clear();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>): void {
  mounted.push(mount(component, { target }));
  flushSync();
}
const pressed = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)].map((b) => b.getAttribute("aria-pressed") === "true" && b.classList.contains("on"));

describe("the M of a track head", () => {
  it("shows the moving mute lane's value as the mixer's M does, and the mute set by hand without a lane", () => {
    const s = laned();
    app.setState(s);
    refreshPositions(s, 0);
    show(TrackHeads);
    show(MixerStrips);
    const heads = pressed("[data-test=track-mute]").slice(0, 3);
    const strips = [1, 2, 3].map((n) => pressed(`[data-strip=track-${s.song.tracks[n - 1]!.id}] [data-test=mixer-mute]`)[0]);
    expect(heads).toEqual([true, false, true]);
    expect(strips).toEqual(heads);
  });

  it("shows the mute set by hand while the lane rests or a hand holds it", () => {
    const s = laned();
    s.song.tracks[0]!.lanes[0]!.resting = true;
    s.automation.held = [{ home: { kind: "track", track: s.song.tracks[1]!.id }, target: MUTE }];
    app.setState(s);
    refreshPositions(s, 0);
    show(TrackHeads);
    expect(pressed("[data-test=track-mute]").slice(0, 3)).toEqual([false, true, true]);
  });
});
