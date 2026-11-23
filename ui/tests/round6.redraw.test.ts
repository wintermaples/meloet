// Keeps the mixer and arrange at 60 frames even with values moved by automation (docs/architecture.md 7, "The Region collected during layout").
// Things that change every frame do not run layout (transform, text boxes out of flow). Knobs and text are shown on the frame that draws the canvas above them.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, LanePoints } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { MAX_WAIT_FRAMES, gateTrack, livePosition, refreshPositions, showTrack, shownPosition } from "../src/components/automation/autoState.svelte";
import { askIf, askLive, askWhole, coverIf } from "../src/components/automation/coverIf";
import { faderText, fracToDb } from "../src/components/mixer/scale";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { watchSlots, type GlSlot } from "../src/gl/slot";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const POINTS = new Map<number, LanePoints>();
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  peekLanePoints: (id: number) => POINTS.get(id),
  lanePoints: (id: number) => Promise.resolve(POINTS.get(id)!),
}));

/** A line from 0 to 1 over ticks 0 to 1000. */
const ramp = (id: number): LanePoints => ({ id, step: false, ticks: Float64Array.from([0, 1000]), values: Float32Array.from([0, 1]) });
const VOLUME = { kind: "volume" } as const;
/** One volume band each on tracks 1 and 2. */
const twoTracks = {
  automation: { held: [] },
  song: { tracks: [1, 2].map((id) => ({ id, lanes: [{ laneId: 10 + id, target: VOLUME, resting: false, points: 2 }] })) },
} as unknown as AppState;
/** The position shown on the mixer strip. */
const shown = () => [1, 2].map((track) => shownPosition(track, VOLUME));
/** The newest position on the line (band heading, params panel). */
const live = () => [1, 2].map((track) => livePosition(track, VOLUME));

describe("waiting tracks (autoState: show the position on the frame that draws the canvas)", () => {
  beforeEach(() => {
    POINTS.set(11, ramp(11));
    POINTS.set(12, ramp(12));
    refreshPositions(twoTracks, 0);
  });
  afterEach(() => POINTS.clear());

  it("if not made to wait, shown at once even inside a frame", () => {
    refreshPositions(twoTracks, 500, true);
    expect(shown()).toEqual([0.5, 0.5]);
    expect(live()).toEqual([0.5, 0.5]);
  });

  it("a waiting track keeps the old position until showTrack (when the canvas above its box is drawn). Once shown, the box is told. The newest position does not wait", () => {
    const told = vi.fn();
    const open = gateTrack(1, "gain", told);
    refreshPositions(twoTracks, 500, true);
    expect(shown()).toEqual([0, 0.5]);
    expect(live()).toEqual([0.5, 0.5]);
    expect(told).not.toHaveBeenCalled();
    showTrack(1);
    expect(shown()).toEqual([0.5, 0.5]);
    expect(told).toHaveBeenCalledTimes(1);
    // A position already shown is not told again on another call.
    showTrack(1);
    expect(told).toHaveBeenCalledTimes(1);
    open();
  });

  it("on a frame where no canvas is drawn, wait for the next turn, and show after MAX_WAIT_FRAMES frames", () => {
    const told = vi.fn();
    const open = gateTrack(1, "gain", told);
    // Make the frame drawn in the earlier test long ago.
    for (let i = 0; i <= MAX_WAIT_FRAMES; i++) refreshPositions(twoTracks, 0, true);
    refreshPositions(twoTracks, 250, true);
    for (let i = 0; i < MAX_WAIT_FRAMES; i++) {
      refreshPositions(twoTracks, 250, true);
      expect(shown()[0], `コマ ${i + 1}`).toBe(0);
    }
    refreshPositions(twoTracks, 250, true);
    expect(shown()[0]).toBe(0.25);
    expect(told).toHaveBeenCalledTimes(1);
    open();
  });

  it("a track whose turn does not come while other tracks' canvases are drawn (a strip not in view) waits until it is visible", () => {
    const open1 = gateTrack(1, "gain", () => {});
    const open2 = gateTrack(2, "gain", () => {});
    for (let i = 0; i < 4 * MAX_WAIT_FRAMES; i++) {
      refreshPositions(twoTracks, 10 * i, true);
      showTrack(2);
    }
    expect(shown()[0]).toBe(0);
    expect(shown()[1]).toBeGreaterThan(0);
    expect(live()[0]).toBeGreaterThan(0);
    // Visible (called when its canvas was drawn).
    showTrack(1);
    expect(shown()[0]).toBe(live()[0]);
    open1();
    open2();
  });

  it("no wait outside a frame (when stopped). Stopping the wait shows it at once", () => {
    const open = gateTrack(1, "gain", () => {});
    refreshPositions(twoTracks, 750);
    expect(shown()).toEqual([0.75, 0.75]);
    refreshPositions(twoTracks, 1000, true);
    expect(shown()).toEqual([0.75, 1]);
    open();
    expect(shown()).toEqual([1, 1]);
  });

  it("a track with 2 boxes waits until both stop", () => {
    const a = gateTrack(1, "gain", () => {});
    const b = gateTrack(1, "gain", () => {});
    a();
    refreshPositions(twoTracks, 500, true);
    expect(shown()[0]).toBe(0);
    b();
    expect(shown()[0]).toBe(0.5);
  });

  it("pan and send of a waiting track are shown one at a time (volume every time). All tracks take the same turn, switching every 2 frames", () => {
    const targets = [VOLUME, { kind: "pan" }, { kind: "send", bus: "a" }] as const;
    const three = {
      automation: { held: [] },
      song: { tracks: [{ id: 1, lanes: targets.map((target, i) => ({ laneId: 21 + i, target, resting: false, points: 2 })) }] },
    } as unknown as AppState;
    for (const id of [21, 22, 23]) POINTS.set(id, ramp(id));
    refreshPositions(three, 0);
    const told = { gain: vi.fn(), pan: vi.fn(), sends: vi.fn() };
    const open = (["gain", "pan", "sends"] as const).map((part) => gateTrack(1, part, told[part]));
    const now = () => targets.map((target) => shownPosition(1, target));
    // 8 frames (4 turns each for the left and right halves). Drawing the canvas every frame, volume catches up every time, pan and send by turns every 2 frames.
    const seen: (number | null)[][] = [];
    for (let frame = 1; frame <= 8; frame++) {
      refreshPositions(three, 100 * frame, true);
      showTrack(1);
      seen.push(now());
    }
    expect(seen.map((s) => s[0])).toEqual([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8].map((v) => Math.round(v * 4096) / 4096));
    const steps = (i: number) => seen.filter((s, f) => f > 0 && s[i] !== seen[f - 1]![i]).length;
    expect(steps(1)).toBeGreaterThanOrEqual(3);
    expect(steps(1)).toBeLessThanOrEqual(4);
    expect(steps(2)).toBeGreaterThanOrEqual(3);
    expect(steps(2)).toBeLessThanOrEqual(4);
    // Pan and send never both change in the same frame.
    expect(seen.some((s, f) => f > 0 && s[1] !== seen[f - 1]![1] && s[2] !== seen[f - 1]![2])).toBe(false);
    expect(told.gain).toHaveBeenCalledTimes(8);
    expect(told.pan.mock.calls.length + told.sends.mock.calls.length).toBe(8);
    // Outside a frame (when stopped), everything is shown.
    refreshPositions(three, 1000);
    expect(now()).toEqual([1, 1, 1]);
    for (const stop of open) stop();
    refreshPositions(twoTracks, 0);
  });

  it("positions of a track whose band is gone go back to null", () => {
    refreshPositions(twoTracks, 500);
    const one = { ...twoTracks, song: { tracks: [twoTracks.song.tracks[1]!] } } as unknown as AppState;
    refreshPositions(one, 500);
    expect(shown()).toEqual([null, 0.5]);
    expect(live()).toEqual([null, 0.5]);
  });
});

describe("box placed under the canvas (coverIf)", () => {
  let slots: Map<HTMLElement, GlSlot>;
  let stop: () => void;
  beforeEach(() => {
    slots = new Map();
    stop = watchSlots({ attach: (n, s) => slots.set(n, s), update: (n, s) => slots.set(n, s), detach: (n) => slots.delete(n) });
    POINTS.set(11, ramp(11));
    POINTS.set(12, ramp(12));
    refreshPositions(twoTracks, 0);
  });
  afterEach(() => {
    stop();
    POINTS.clear();
  });

  it("askWhole swaps two values that look the same. Any number of calls in the same frame (the same task) count as one", async () => {
    const node = document.createElement("div");
    askWhole(node);
    const first = node.style.backgroundPosition;
    askWhole(node);
    expect(node.style.backgroundPosition).toBe(first);
    await Promise.resolve();
    askWhole(node);
    const second = node.style.backgroundPosition;
    await Promise.resolve();
    askWhole(node);
    expect(first).not.toBe("");
    expect(second).not.toBe(first);
    expect(node.style.backgroundPosition).toBe(first);
  });

  it("while there are tracks, an empty box filling the box becomes the mark called when painting (paint); there it shows the position and asks for its own rectangle", () => {
    const node = document.createElement("div");
    const action = coverIf(node, 1);
    // Going under the canvas and asking for a rectangle use the same empty box (no overflow of the content, it lies exactly on the canvas).
    const whole = node.querySelector<HTMLElement>("[data-test=ask-whole]")!;
    expect([...slots.keys()]).toEqual([whole]);
    const slot = slots.get(whole)!;
    expect(slot.kind).toBe("paint");
    expect([whole.style.position, whole.style.pointerEvents]).toEqual(["absolute", "none"]);
    expect(whole.style.inset).toContain("--ask-left");
    refreshPositions(twoTracks, 500, true);
    expect(shown()[0]).toBe(0);
    expect(whole.style.backgroundPosition).toBe("");
    if (slot.kind === "paint") slot.paint(null as never, null as never, null as never);
    expect(shown()[0]).toBe(0.5);
    expect(whole.style.backgroundPosition).not.toBe("");

    action.update(null);
    expect(slots.size).toBe(0);
    expect(node.querySelector("[data-test=ask-whole]")).toBeNull();
    refreshPositions(twoTracks, 750, true);
    expect(shown()[0]).toBe(0.75);
    action.destroy();
  });
});

describe("moving things in the mixer do not run layout", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  beforeEach(() => {
    app.reset();
    target = document.createElement("div");
    document.body.append(target);
    app.setState(fakeScene("mixerRest")!.state);
    mounted = mount(MixerStrips, { target });
    flushSync();
  });
  afterEach(() => {
    if (mounted) unmount(mounted);
    target.remove();
  });
  const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];

  it("the fader knob uses transform (top is not written), px aligned to device pixels", () => {
    const knobs = qa("[data-test=mixer-fader-knob]");
    expect(knobs.length).toBe(qa("[data-test=mixer-strip]").length);
    for (const knob of knobs) {
      expect(knob.style.top).toBe("");
      expect(knob.style.transform).toMatch(/^translate\(-50%, -?\d+px\)$/);
    }
  });

  it("the send bar uses scaleX (width is not written)", () => {
    const fills = qa("[data-test=mixer-send] .fill");
    expect(fills.length).toBeGreaterThan(0);
    for (const fill of fills) {
      expect(fill.style.width).toBe("");
      expect(fill.style.transform).toMatch(/^scaleX\([\d.]+\)$/);
    }
  });

  it("value text (fader, pan, send) is in a box out of flow (.live > .in)", () => {
    const texts = [...qa("[data-test=mixer-gain]"), ...qa("[data-test=mixer-pan-text]"), ...qa("[data-test=mixer-send] + .value > .in")];
    expect(texts.length).toBeGreaterThan(qa("[data-test=mixer-strip]").length * 2);
    for (const el of texts) {
      expect(el.classList.contains("in")).toBe(true);
      expect(el.parentElement!.classList.contains("live")).toBe(true);
      expect(el.textContent).not.toBe("");
    }
  });

  it("only strips moved by a band: the fader and value box go under the canvas, the moving pan row asks for a rectangle outside the canvas. The value text row also asks", () => {
    const wholes = qa("[data-test=ask-whole]");
    expect(wholes.map((w) => [w.parentElement!.classList[0], w.dataset.glSlot ?? null])).toEqual([
      ["pan-box", null],
      ["gain-box", "paint"],
      ["value", null],
    ]);
    for (const whole of wholes) expect(whole.closest("[data-test=mixer-strip]")).toBe(qa("[data-test=mixer-strip]")[0]);
    expect(wholes[2]!.parentElement!.querySelector("[data-test=mixer-gain]")).not.toBeNull();
    const [panBox, gainBox] = wholes.map((w) => w.parentElement!);
    expect(panBox!.querySelector("[data-test=mixer-pan]")).not.toBeNull();
    for (const part of ["mixer-fader", "mixer-gain"]) expect(gainBox!.querySelector(`[data-test=${part}]`), part).not.toBeNull();
    // The 2 send rows are one box (not moving, so no box asks).
    expect(qa(".sends-box").every((b) => b.querySelectorAll("[data-test=mixer-send]").length === 2 || b.querySelectorAll(".empty-row").length === 2)).toBe(true);
  });
});

describe("value text every frame (spec 2 (Handle moved by automation): knobs 30 and 15 times a second, value text is not thinned)", () => {
  beforeEach(() => {
    POINTS.set(11, ramp(11));
    POINTS.set(12, ramp(12));
    refreshPositions(twoTracks, 0);
  });
  afterEach(() => {
    POINTS.clear();
    refreshPositions(twoTracks, 0);
  });
  const nextTask = () => Promise.resolve();

  it("an askLive box asks for a rectangle on every frame the newest position changes, even on a waiting track", async () => {
    const open = gateTrack(1, "gain", () => {});
    const node = document.createElement("div");
    const action = askLive(node, 1);
    const whole = node.querySelector<HTMLElement>("[data-test=ask-whole]")!;
    const seen = new Set<string>();
    for (let frame = 1; frame <= 4; frame++) {
      await nextTask();
      const before = whole.style.backgroundPosition;
      refreshPositions(twoTracks, 100 * frame, true);
      // In the next small task after the band heading is written.
      expect(whole.style.backgroundPosition).toBe(before);
      await nextTask();
      expect(whole.style.backgroundPosition, `frame ${frame}`).not.toBe(before);
      seen.add(whole.style.backgroundPosition);
    }
    // The knob (shown position) is still waiting.
    expect(shown()[0]).toBe(0);
    expect(live()[0]).toBe(Math.round(0.4 * 4096) / 4096);
    expect(seen.size).toBe(2);
    action.destroy();
    open();
  });

  it("the value text box (askLive) and the pan row (askIf) each ask for a rectangle in the same frame", async () => {
    const open = gateTrack(1, "gain", () => {});
    const node = document.createElement("div");
    const live = askLive(node, 1);
    const gate = askIf(node, { track: 1, part: "gain" });
    const wholes = [...node.querySelectorAll<HTMLElement>("[data-test=ask-whole]")];
    expect(wholes).toHaveLength(2);
    await nextTask();
    refreshPositions(twoTracks, 500, true);
    showTrack(1);
    await nextTask();
    for (const whole of wholes) expect(whole.style.backgroundPosition).not.toBe("");
    live.destroy();
    gate.destroy();
    open();
  });
});

describe("mixer value text does not wait for the knob", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  afterEach(() => {
    if (mounted) unmount(mounted);
    target.remove();
    POINTS.clear();
  });

  it("a track whose volume is moved by a band: text at the newest position, the fader knob at the old position until the frame that draws the canvas", async () => {
    app.reset();
    const state = fakeScene("mixerRest")!.state;
    const track = state.song.tracks.find((x) => x.lanes.some((l) => l.target.kind === "volume" && !l.resting))!;
    const lane = track.lanes.find((l) => l.target.kind === "volume")!;
    POINTS.set(lane.laneId, ramp(lane.laneId));
    target = document.createElement("div");
    document.body.append(target);
    app.setState(state);
    refreshPositions(state, 0);
    mounted = mount(MixerStrips, { target });
    flushSync();
    const strip = target.querySelector<HTMLElement>(`[data-strip=track-${track.id}]`)!;
    const knob = () => strip.querySelector<HTMLElement>("[data-test=mixer-fader]")!.dataset.db;
    const gain = () => strip.querySelector<HTMLElement>("[data-test=mixer-gain]")!.textContent;
    const knobBefore = knob();
    refreshPositions(state, 500, true);
    flushSync();
    expect(gain()).toBe(faderText(fracToDb(0)));
    await Promise.resolve();
    flushSync();
    expect(livePosition(track.id, VOLUME)).toBe(0.5);
    expect(shownPosition(track.id, VOLUME)).toBe(0);
    expect(gain()).toBe(faderText(fracToDb(0.5)));
    expect(knob()).toBe(knobBefore);
    showTrack(track.id);
    flushSync();
    expect(knob()).not.toBe(knobBefore);
    expect(gain()).toBe(faderText(fracToDb(0.5)));
    refreshPositions(state, 0);
  });
});
