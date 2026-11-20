// 第 6 回の直し: オートメーションで動く値があっても、ミキサーとアレンジを 60 コマに（docs/architecture.md 7 章「レイアウトでためる Region」）。
// 毎コマ変わる物はレイアウトを走らせない（transform、流れの外の字の箱）。つまみと字は、上の canvas を描くコマに見せる。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, LanePoints } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { MAX_WAIT_FRAMES, gateTrack, livePosition, refreshPositions, showTrack } from "../src/components/automation/autoState.svelte";
import { askWhole, coverIf } from "../src/components/automation/coverIf";
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

/** 0 から 1000 ティックで 0 → 1 の線。 */
const ramp = (id: number): LanePoints => ({ id, step: false, ticks: Float64Array.from([0, 1000]), values: Float32Array.from([0, 1]) });
const VOLUME = { kind: "volume" } as const;
/** トラック 1 と 2 に、音量の帯が 1 本ずつ。 */
const twoTracks = {
  automation: { held: [] },
  song: { tracks: [1, 2].map((id) => ({ id, lanes: [{ laneId: 10 + id, target: VOLUME, resting: false, points: 2 }] })) },
} as unknown as AppState;
const shown = () => [1, 2].map((track) => livePosition(track, VOLUME));

describe("待つトラック（autoState: canvas を描くコマに位置を見せる）", () => {
  beforeEach(() => {
    POINTS.set(11, ramp(11));
    POINTS.set(12, ramp(12));
    refreshPositions(twoTracks, 0);
  });
  afterEach(() => POINTS.clear());

  it("待たせていなければ、コマの中でもすぐ見せる", () => {
    refreshPositions(twoTracks, 500, true);
    expect(shown()).toEqual([0.5, 0.5]);
  });

  it("待つトラックは、showTrack（その箱の上の canvas を描くとき）まで前の位置のまま。見せたら、箱に知らせる", () => {
    const told = vi.fn();
    const open = gateTrack(1, told);
    refreshPositions(twoTracks, 500, true);
    expect(shown()).toEqual([0, 0.5]);
    expect(told).not.toHaveBeenCalled();
    showTrack(1);
    expect(shown()).toEqual([0.5, 0.5]);
    expect(told).toHaveBeenCalledTimes(1);
    // もう見せた位置は、もう一度呼んでも知らせない。
    showTrack(1);
    expect(told).toHaveBeenCalledTimes(1);
    open();
  });

  it("canvas が描かれないコマは次の番を待ち、MAX_WAIT_FRAMES コマ待ったら見せる", () => {
    const told = vi.fn();
    const open = gateTrack(1, told);
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

  it("コマの外（止まっているとき）は待たない。待たせるのをやめたら、すぐ見せる", () => {
    const open = gateTrack(1, () => {});
    refreshPositions(twoTracks, 750);
    expect(shown()).toEqual([0.75, 0.75]);
    refreshPositions(twoTracks, 1000, true);
    expect(shown()).toEqual([0.75, 1]);
    open();
    expect(shown()).toEqual([1, 1]);
  });

  it("箱が 2 つあるトラックは、どちらもやめるまで待つ", () => {
    const a = gateTrack(1, () => {});
    const b = gateTrack(1, () => {});
    a();
    refreshPositions(twoTracks, 500, true);
    expect(shown()[0]).toBe(0);
    b();
    expect(shown()[0]).toBe(0.5);
  });
});

describe("canvas の下に入れる箱（coverIf）", () => {
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

  it("askWhole は、見た目の同じ 2 つの値を入れかえる", () => {
    const node = document.createElement("div");
    askWhole(node);
    const first = node.style.backgroundPosition;
    askWhole(node);
    const second = node.style.backgroundPosition;
    askWhole(node);
    expect(first).not.toBe("");
    expect(second).not.toBe(first);
    expect(node.style.backgroundPosition).toBe(first);
  });

  it("トラックがあるあいだ、絵を描くときに呼ばれる印（paint）になり、そこで位置を見せて、箱まるごとの四角を頼む", () => {
    const node = document.createElement("div");
    const action = coverIf(node, 1);
    const slot = slots.get(node)!;
    expect(slot.kind).toBe("paint");
    refreshPositions(twoTracks, 500, true);
    expect(shown()[0]).toBe(0);
    // 頼むのは、箱いっぱいに置いた空の箱（中の物のはみ出しを入れないため）。
    const whole = node.querySelector<HTMLElement>("[data-test=ask-whole]")!;
    expect([whole.style.position, whole.style.pointerEvents]).toEqual(["absolute", "none"]);
    expect(whole.style.inset).toContain("--ask-left");
    expect(whole.style.backgroundPosition).toBe("");
    if (slot.kind === "paint") slot.paint(null as never, null as never, null as never);
    expect(shown()[0]).toBe(0.5);
    expect(whole.style.backgroundPosition).not.toBe("");

    action.update(null);
    expect(slots.has(node)).toBe(false);
    expect(node.querySelector("[data-test=ask-whole]")).toBeNull();
    refreshPositions(twoTracks, 750, true);
    expect(shown()[0]).toBe(0.75);
    action.destroy();
  });
});

describe("ミキサーの動く物は、レイアウトを走らせない", () => {
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

  it("フェーダーのつまみは transform（top は書かない）、画素に合わせた px", () => {
    const knobs = qa("[data-test=mixer-fader-knob]");
    expect(knobs.length).toBe(qa("[data-test=mixer-strip]").length);
    for (const knob of knobs) {
      expect(knob.style.top).toBe("");
      expect(knob.style.transform).toMatch(/^translate\(-50%, -?\d+px\)$/);
    }
  });

  it("センドの棒は scaleX（幅は書かない）", () => {
    const fills = qa("[data-test=mixer-send] .fill");
    expect(fills.length).toBeGreaterThan(0);
    for (const fill of fills) {
      expect(fill.style.width).toBe("");
      expect(fill.style.transform).toMatch(/^scaleX\([\d.]+\)$/);
    }
  });

  it("値の字（フェーダー ・ パン ・ センド）は、流れの外の箱（.live > .in）の中", () => {
    const texts = [...qa("[data-test=mixer-gain]"), ...qa("[data-test=mixer-pan-text]"), ...qa("[data-test=mixer-send] + .value > .in")];
    expect(texts.length).toBeGreaterThan(qa("[data-test=mixer-strip]").length * 2);
    for (const el of texts) {
      expect(el.classList.contains("in")).toBe(true);
      expect(el.parentElement!.classList.contains("live")).toBe(true);
      expect(el.textContent).not.toBe("");
    }
  });

  it("帯が動かしているストリップだけ、動く物の箱が canvas の下に入る（パンも動くなら、パンから値までの箱）", () => {
    const boxes = qa(".moving-box, .pan-box, .gain-box").filter((b) => b.dataset.glSlot !== undefined);
    expect(boxes.map((b) => [b.classList[0], b.dataset.glSlot])).toEqual([["pan-box", "paint"]]);
    const box = boxes[0]!;
    expect(box.closest("[data-test=mixer-strip]")).toBe(qa("[data-test=mixer-strip]")[0]);
    expect(box.querySelector("[data-test=mixer-send]")).toBeNull();
    for (const part of ["mixer-pan", "mixer-fader", "mixer-gain"]) expect(box.querySelector(`[data-test=${part}]`), part).not.toBeNull();
  });
});
