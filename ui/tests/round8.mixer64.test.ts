// The mixer with 64 strips and 4 bands each at 60 frames. The fader value text changes every frame (spec 3),
// but strips scrolled out of view (content-visibility: auto skips their inside) are not written (whenSeen of coverIf).
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LanePoints } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { refreshPositions } from "../src/components/automation/autoState.svelte";
import { faderText, fracToDb } from "../src/components/mixer/scale";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const POINTS = new Map<number, LanePoints>();
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  peekLanePoints: (id: number) => POINTS.get(id),
  lanePoints: (id: number) => Promise.resolve(POINTS.get(id)!),
}));

/** A line from 0 → 1 over 0 to 1000 ticks. */
const ramp = (id: number): LanePoints => ({ id, step: false, ticks: Float64Array.from([0, 1000]), values: Float32Array.from([0, 1]) });

/** An IntersectionObserver where the test decides what is visible. */
class FakeObserver {
  static all: FakeObserver[] = [];
  readonly seen = new Set<Element>();
  constructor(readonly tell: (entries: { target: Element; isIntersecting: boolean }[]) => void) {
    FakeObserver.all.push(this);
  }
  observe(node: Element): void {
    this.seen.add(node);
  }
  unobserve(node: Element): void {
    this.seen.delete(node);
  }
  disconnect(): void {
    this.seen.clear();
  }
}
function setSeen(node: Element, isIntersecting: boolean): void {
  const io = FakeObserver.all.find((o) => o.seen.has(node));
  if (!io) throw new Error("not observed");
  io.tell([{ target: node, isIntersecting }]);
  flushSync();
}

describe("mixer value text is written every frame only for visible strips", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  beforeEach(() => vi.stubGlobal("IntersectionObserver", FakeObserver));
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    target.remove();
    POINTS.clear();
    vi.unstubAllGlobals();
  });

  it("while visible, the text changes on each frame where the newest position changes. Out of view, no writing and no rectangle requests", async () => {
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
    const value = strip.querySelector<HTMLElement>(".gain-box > .value")!;
    const gain = () => strip.querySelector<HTMLElement>("[data-test=mixer-gain]")!.textContent;
    const asked = () => value.querySelectorAll("[data-test=ask-whole]").length;
    const frame = async (tick: number) => {
      refreshPositions(state, tick, true);
      flushSync();
      await Promise.resolve();
      flushSync();
    };
    const atStart = gain();

    // Not visible yet (no notice has come): the text stays at the knob value.
    await frame(100);
    expect(gain()).toBe(atStart);
    expect(asked()).toBe(0);

    setSeen(value, true);
    expect(asked()).toBe(1);
    const texts: string[] = [];
    for (const tick of [200, 300, 400, 500, 600]) {
      await frame(tick);
      expect(gain(), `tick ${tick}`).toBe(faderText(fracToDb(tick / 1000)));
      texts.push(gain()!);
    }
    expect(new Set(texts).size).toBe(texts.length);

    setSeen(value, false);
    expect(asked()).toBe(0);
    const out = gain();
    await frame(700);
    await frame(800);
    expect(gain()).toBe(out);
    expect(gain()).not.toBe(faderText(fracToDb(0.8)));

    // Back in view, every frame again.
    setSeen(value, true);
    await frame(900);
    expect(gain()).toBe(faderText(fracToDb(0.9)));
    refreshPositions(state, 0);
  });
});
