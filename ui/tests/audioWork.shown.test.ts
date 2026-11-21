// 区切り4 の直し（画面: 音を作っている %）: 見せる % は 1 秒に 2 回まで、クリップの下 ・ 選んだクリップの欄 ・ 状態行の 3 か所が同じ値で
// 同じときに変わる。仕事の表は 1 回だけ引き、クリップの数に比例して引かない（docs/architecture.md 22-19）。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, AudioJob, AudioWorkView } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import AudioClipNotes from "../src/components/arrange/AudioClipNotes.svelte";
import type { LaneDims, View } from "../src/components/arrangeGeom";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { SHOW_EVERY_MS, shownWork, WorkShown } from "../src/state/audioWork.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 4 * 1920;
const dims: LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
const view: View = { pxPerTick: 64 / BAR, scrollX: 0, width: 2000 };
const scene = (name: string) => fakeScene(name)!.state as FakeState;
const rendering = (clip: number, percent: number, audio = 1): AudioJob => ({ kind: "rendering", audio, clip, percent });

describe("WorkShown（見せる進み）", () => {
  let now = 0;
  let work: AudioWorkView | null = null;
  let shown: WorkShown;
  beforeEach(() => {
    vi.useFakeTimers();
    now = 1000;
    shown = new WorkShown(
      () => work,
      () => now,
    );
  });
  afterEach(() => {
    shown.forget();
    vi.useRealTimers();
  });
  const set = (jobs: AudioJob[]) => (work = { jobs });
  const later = (ms: number) => {
    now += ms;
    vi.advanceTimersByTime(ms);
  };

  it("% だけが変わったときは、前に見せかえてから 500 ms たつまで前の値のまま", () => {
    set([rendering(7, 10)]);
    const first = shown.current;
    expect(first.rendering.get(7)?.percent).toBe(10);
    later(200);
    set([rendering(7, 22)]);
    expect(shown.current).toBe(first);
    later(200);
    set([rendering(7, 44)]);
    expect(shown.current).toBe(first);
    later(SHOW_EVERY_MS - 400);
    expect(shown.current.rendering.get(7)?.percent).toBe(44);
  });

  it("1 秒に 5 回の % でも、見せかえるのは 1 秒に 2 回", () => {
    const seen: number[] = [];
    for (let i = 0; i < 25; i++) {
      set([rendering(7, i * 4)]);
      const p = shown.current.rendering.get(7)!.percent;
      if (seen.at(-1) !== p) seen.push(p);
      later(200);
    }
    // 5 秒（25 回）で、はじめの 1 回と 500 ms ごとの 10 回まで。
    expect(seen.length).toBeLessThanOrEqual(11);
    expect(seen.length).toBeGreaterThanOrEqual(9);
  });

  it("仕事が始まった ・ 終わった（出る ・ 消える）はすぐ", () => {
    set([rendering(7, 10)]);
    void shown.current;
    later(100);
    set([rendering(7, 10), rendering(8, 0)]);
    expect(shown.current.rendering.has(8)).toBe(true);
    later(100);
    set([rendering(8, 3)]);
    expect([...shown.current.rendering.keys()]).toEqual([8]);
    later(100);
    set([]);
    expect(shown.current.jobs).toEqual([]);
  });

  it("何も変わらなければ同じ物を返す（読んだ所は動かない）", () => {
    set([rendering(7, 10)]);
    const first = shown.current;
    set([rendering(7, 10)]);
    later(SHOW_EVERY_MS * 2);
    expect(shown.current).toBe(first);
  });

  it("表はクリップの id と音のファイルの番号で引ける。状態行には、いちばん先に始めたクリップのある仕事", () => {
    set([{ kind: "loading", audio: 3, clip: null, percent: 40 }, rendering(9, 5, 4), rendering(7, 1, 5)]);
    const s = shown.current;
    expect(s.loading.get(3)?.percent).toBe(40);
    expect(s.rendering.get(7)?.audio).toBe(5);
    expect(s.firstRendering?.clip).toBe(9);
  });
});

describe("クリップの下 ・ 欄 ・ 状態行は、同じ % を同じときに出す", () => {
  let mounted: Record<string, unknown>[] = [];
  let target: HTMLElement;
  beforeEach(() => {
    vi.useFakeTimers();
    app.reset();
    shownWork.forget();
    target = document.createElement("div");
    target.className = "app";
    document.body.append(target);
  });
  afterEach(() => {
    for (const m of mounted) unmount(m);
    mounted = [];
    target.remove();
    shownWork.forget();
    vi.useRealTimers();
  });

  function showAll(state: AppState): void {
    app.setState(state);
    const vocal = state.song.tracks.find((tr) => tr.kind.kind === "vocal")!;
    mounted.push(mount(AudioClipNotes, { target, props: { view, dims, tracks: [vocal] } }));
    mounted.push(mount(ParamsPanel, { target, props: { onfold: () => {} } }));
    mounted.push(mount(StatusLine, { target }));
    flushSync();
  }
  const percents = () => {
    const under = target.querySelector('[data-test="audio-clip-rendering"]')?.textContent?.match(/(\d+) %/)?.[1];
    const panel = target.querySelector('[data-test="clip-rendering"] .in')?.textContent?.match(/(\d+) %/)?.[1];
    const status = target.querySelector(".message")?.textContent?.match(/(\d+) %/)?.[1];
    return [under, panel, status].map(Number);
  };
  function withPercent(base: FakeState, percent: number): FakeState {
    const job = base.audioWork.jobs[0]!;
    return { ...base, audioWork: { jobs: [{ ...job, percent }] } };
  }

  it("% が 200 ms ごとに来ても、3 つは 500 ms ごとにそろって変わる", () => {
    const base = scene("stretchRendering");
    showAll(base);
    expect(percents()).toEqual([34, 34, 34]);
    const fill = () => target.querySelector<HTMLElement>('[data-test="clip-rendering"] .fill')!.style.transform;
    expect(fill()).toBe("scaleX(0.34)");

    vi.advanceTimersByTime(200);
    app.setState(withPercent(base, 40));
    flushSync();
    expect(percents()).toEqual([34, 34, 34]);

    vi.advanceTimersByTime(200);
    app.setState(withPercent(base, 47));
    flushSync();
    expect(percents()).toEqual([34, 34, 34]);

    vi.advanceTimersByTime(SHOW_EVERY_MS - 400);
    flushSync();
    expect(percents()).toEqual([47, 47, 47]);
    expect(fill()).toBe("scaleX(0.47)");
  });

  it("仕事が終わったら、3 つともすぐ消える", () => {
    const base = scene("stretchRendering");
    showAll(base);
    vi.advanceTimersByTime(100);
    app.setState({ ...base, audioWork: { jobs: [] } });
    flushSync();
    expect(target.querySelector('[data-test="audio-clip-rendering"]')).toBeNull();
    expect(target.querySelector('[data-test="clip-rendering"]')).toBeNull();
    expect(target.querySelector(".message")?.textContent ?? "").not.toContain("音を作っています");
  });
});

describe("仕事の表を引く回数は、クリップの数に比例しない", () => {
  let mounted: Record<string, unknown> | null = null;
  let target: HTMLElement;
  beforeEach(() => {
    app.reset();
    shownWork.forget();
    target = document.createElement("div");
    document.body.append(target);
  });
  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = null;
    target.remove();
    shownWork.forget();
  });

  /** clips 個の歌のクリップ（1 小節ずつ）と、そのうち jobs 個の「音を作っている」。表を読んだ回数を数える。 */
  function count(clips: number, jobs: number): number {
    const base = scene("stretchRendering");
    const vocal = base.song.tracks.find((tr) => tr.kind.kind === "vocal")!;
    const one = vocal.clips[0]!;
    const many = Array.from({ length: clips }, (_, i) => ({ ...one, id: 1000 + i, start: i * BAR, end: (i + 1) * BAR }));
    const track = { ...vocal, clips: many };
    let reads = 0;
    const list = Array.from({ length: jobs }, (_, i) => rendering(1000 + i, i % 100));
    const watched = new Proxy(list, {
      get(target, key, receiver) {
        if (typeof key === "string" && /^\d+$/.test(key)) reads++;
        return Reflect.get(target, key, receiver);
      },
    });
    app.setState({ ...base, song: { ...base.song, tracks: base.song.tracks.map((tr) => (tr.id === vocal.id ? track : tr)) }, audioWork: { jobs: watched } });
    mounted = mount(AudioClipNotes, { target, props: { view: { ...view, width: 1e7 }, dims, tracks: [track] } });
    flushSync();
    expect(target.querySelectorAll('[data-test="audio-clip-rendering"]').length).toBe(jobs);
    unmount(mounted);
    mounted = null;
    shownWork.forget();
    return reads;
  }

  it("192 個のクリップに 190 の仕事でも、表を読むのは仕事の数の数倍まで（クリップ × 仕事ではない）", () => {
    const reads192 = count(192, 190);
    expect(reads192).toBeLessThanOrEqual(190 * 4);
    expect(count(380, 190)).toBe(reads192);
  });
});
