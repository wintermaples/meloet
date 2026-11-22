// 第 6 回の直し: オートメーションで動く値があっても、ミキサーとアレンジを 60 コマに（docs/architecture.md 7 章「レイアウトでためる Region」）。
// 毎コマ変わる物はレイアウトを走らせない（transform、流れの外の字の箱）。つまみと字は、上の canvas を描くコマに見せる。
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

/** 0 から 1000 ティックで 0 → 1 の線。 */
const ramp = (id: number): LanePoints => ({ id, step: false, ticks: Float64Array.from([0, 1000]), values: Float32Array.from([0, 1]) });
const VOLUME = { kind: "volume" } as const;
/** トラック 1 と 2 に、音量の帯が 1 本ずつ。 */
const twoTracks = {
  automation: { held: [] },
  song: { tracks: [1, 2].map((id) => ({ id, lanes: [{ laneId: 10 + id, target: VOLUME, resting: false, points: 2 }] })) },
} as unknown as AppState;
/** ミキサーのストリップに見せている位置。 */
const shown = () => [1, 2].map((track) => shownPosition(track, VOLUME));
/** 線の上のいちばん新しい位置（帯の見出し、パラメータの欄）。 */
const live = () => [1, 2].map((track) => livePosition(track, VOLUME));

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
    expect(live()).toEqual([0.5, 0.5]);
  });

  it("待つトラックは、showTrack（その箱の上の canvas を描くとき）まで前の位置のまま。見せたら、箱に知らせる。いちばん新しい位置は待たない", () => {
    const told = vi.fn();
    const open = gateTrack(1, "gain", told);
    refreshPositions(twoTracks, 500, true);
    expect(shown()).toEqual([0, 0.5]);
    expect(live()).toEqual([0.5, 0.5]);
    expect(told).not.toHaveBeenCalled();
    showTrack(1);
    expect(shown()).toEqual([0.5, 0.5]);
    expect(told).toHaveBeenCalledTimes(1);
    // もう見せた位置は、もう一度呼んでも知らせない。
    showTrack(1);
    expect(told).toHaveBeenCalledTimes(1);
    open();
  });

  it("どの canvas も描かれないコマは次の番を待ち、MAX_WAIT_FRAMES コマ待ったら見せる", () => {
    const told = vi.fn();
    const open = gateTrack(1, "gain", told);
    // 前のテストで描いたコマを、遠い昔にする。
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

  it("ほかのトラックの canvas は描かれているのに番が来ないトラック（見えていないストリップ）は、見えるまで待つ", () => {
    const open1 = gateTrack(1, "gain", () => {});
    const open2 = gateTrack(2, "gain", () => {});
    for (let i = 0; i < 4 * MAX_WAIT_FRAMES; i++) {
      refreshPositions(twoTracks, 10 * i, true);
      showTrack(2);
    }
    expect(shown()[0]).toBe(0);
    expect(shown()[1]).toBeGreaterThan(0);
    expect(live()[0]).toBeGreaterThan(0);
    // 見えた（canvas を描くときに呼ばれた）。
    showTrack(1);
    expect(shown()[0]).toBe(live()[0]);
    open1();
    open2();
  });

  it("コマの外（止まっているとき）は待たない。待たせるのをやめたら、すぐ見せる", () => {
    const open = gateTrack(1, "gain", () => {});
    refreshPositions(twoTracks, 750);
    expect(shown()).toEqual([0.75, 0.75]);
    refreshPositions(twoTracks, 1000, true);
    expect(shown()).toEqual([0.75, 1]);
    open();
    expect(shown()).toEqual([1, 1]);
  });

  it("箱が 2 つあるトラックは、どちらもやめるまで待つ", () => {
    const a = gateTrack(1, "gain", () => {});
    const b = gateTrack(1, "gain", () => {});
    a();
    refreshPositions(twoTracks, 500, true);
    expect(shown()[0]).toBe(0);
    b();
    expect(shown()[0]).toBe(0.5);
  });

  it("待つトラックのパンとセンドは、1 回に片方ずつ見せる（音量は毎回）。どのトラックも同じ番で、コマ 2 つごとに入れかわる", () => {
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
    // 8 コマ（左右の半分の番を 4 回ずつ）。毎コマ canvas を描いたとして、音量は毎回、パンとセンドは 2 コマずつ交互に追いつく。
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
    // 同じコマに、パンとセンドの両方は変わらない。
    expect(seen.some((s, f) => f > 0 && s[1] !== seen[f - 1]![1] && s[2] !== seen[f - 1]![2])).toBe(false);
    expect(told.gain).toHaveBeenCalledTimes(8);
    expect(told.pan.mock.calls.length + told.sends.mock.calls.length).toBe(8);
    // コマの外（止まった時）は、ぜんぶ見せる。
    refreshPositions(three, 1000);
    expect(now()).toEqual([1, 1, 1]);
    for (const stop of open) stop();
    refreshPositions(twoTracks, 0);
  });

  it("帯がなくなったトラックの位置は、どちらも null に戻る", () => {
    refreshPositions(twoTracks, 500);
    const one = { ...twoTracks, song: { tracks: [twoTracks.song.tracks[1]!] } } as unknown as AppState;
    refreshPositions(one, 500);
    expect(shown()).toEqual([null, 0.5]);
    expect(live()).toEqual([null, 0.5]);
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

  it("askWhole は、見た目の同じ 2 つの値を入れかえる。同じコマ（同じ仕事の中）に何回呼んでも 1 回だけ", async () => {
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

  it("トラックがあるあいだ、箱いっぱいの空の箱が、絵を描くときに呼ばれる印（paint）になり、そこで位置を見せて、自分の四角を頼む", () => {
    const node = document.createElement("div");
    const action = coverIf(node, 1);
    // canvas の下に入るのも、四角を頼むのも、同じ空の箱（中の物のはみ出しを入れず、canvas とぴったり重なる）。
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

  it("帯が動かしているストリップだけ: フェーダーと値の箱が canvas の下に入り、動くパンの行は canvas の外で四角を頼む。値の字の行も四角を頼む", () => {
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
    // センドの 2 行は 1 つの箱（動いていないので、頼む箱はない）。
    expect(qa(".sends-box").every((b) => b.querySelectorAll("[data-test=mixer-send]").length === 2 || b.querySelectorAll(".empty-row").length === 2)).toBe(true);
  });
});

describe("値の字は毎コマ（仕様書 3 章「オートメーションで動くつまみ」: つまみは 1 秒 30 回 ・ 15 回、値の字は間引かない）", () => {
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

  it("askLive の箱は、待つトラックでも、いちばん新しい位置が変わるコマごとに四角を頼む", async () => {
    const open = gateTrack(1, "gain", () => {});
    const node = document.createElement("div");
    const action = askLive(node, 1);
    const whole = node.querySelector<HTMLElement>("[data-test=ask-whole]")!;
    const seen = new Set<string>();
    for (let frame = 1; frame <= 4; frame++) {
      await nextTask();
      const before = whole.style.backgroundPosition;
      refreshPositions(twoTracks, 100 * frame, true);
      // 帯の見出しを書いたあとの、次の小さな仕事で。
      expect(whole.style.backgroundPosition).toBe(before);
      await nextTask();
      expect(whole.style.backgroundPosition, `frame ${frame}`).not.toBe(before);
      seen.add(whole.style.backgroundPosition);
    }
    // つまみ（見せる位置）は、まだ待っている。
    expect(shown()[0]).toBe(0);
    expect(live()[0]).toBe(Math.round(0.4 * 4096) / 4096);
    expect(seen.size).toBe(2);
    action.destroy();
    open();
  });

  it("値の字の箱（askLive）とパンの行（askIf）は、同じコマにそれぞれ四角を頼む", async () => {
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

describe("ミキサーの値の字は、つまみを待たない", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  afterEach(() => {
    if (mounted) unmount(mounted);
    target.remove();
    POINTS.clear();
  });

  it("帯が音量を動かしているトラック: 字はいちばん新しい位置、フェーダーのつまみは canvas を描くコマまで前の位置", async () => {
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
