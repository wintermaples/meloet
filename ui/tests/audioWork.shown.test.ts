// The % of making sound on screen: the shown % changes at most twice a second, and under the clip, in the selected clip's panel
// and in the status line it is the same value at the same time. The job table is looked up once, not in proportion to the number of clips (docs/architecture.md 22-19).
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

describe("WorkShown (the shown progress)", () => {
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
  const set = (jobs: AudioJob[]) => (work = { jobs, bounce: null });
  const later = (ms: number) => {
    now += ms;
    vi.advanceTimersByTime(ms);
  };

  it("when only the % changes, the old value stays until 500 ms after the last change", () => {
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

  it("even with a % 5 times a second, the shown value changes twice a second", () => {
    const seen: number[] = [];
    for (let i = 0; i < 25; i++) {
      set([rendering(7, i * 4)]);
      const p = shown.current.rendering.get(7)!.percent;
      if (seen.at(-1) !== p) seen.push(p);
      later(200);
    }
    // In 5 seconds (25 times): the first one and one every 500 ms, so 10 at most.
    expect(seen.length).toBeLessThanOrEqual(11);
    expect(seen.length).toBeGreaterThanOrEqual(9);
  });

  it("a job starting or ending (appear, disappear) is shown at once", () => {
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

  it("returns the same object when nothing changed (what was read does not move)", () => {
    set([rendering(7, 10)]);
    const first = shown.current;
    set([rendering(7, 10)]);
    later(SHOW_EVERY_MS * 2);
    expect(shown.current).toBe(first);
  });

  it("the table can be looked up by clip id and audio file number. The status line shows the job of the clip that started first", () => {
    set([{ kind: "loading", audio: 3, clip: null, percent: 40 }, rendering(9, 5, 4), rendering(7, 1, 5)]);
    const s = shown.current;
    expect(s.loading.get(3)?.percent).toBe(40);
    expect(s.rendering.get(7)?.audio).toBe(5);
    expect(s.firstRendering?.clip).toBe(9);
  });
});

describe("under the clip, the panel and the status line show the same % at the same time", () => {
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
    return { ...base, audioWork: { jobs: [{ ...job, percent }], bounce: null } };
  }

  it("even with a % every 200 ms, all 3 change together every 500 ms", () => {
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

  it("when the job ends, all 3 disappear at once", () => {
    const base = scene("stretchRendering");
    showAll(base);
    vi.advanceTimersByTime(100);
    app.setState({ ...base, audioWork: { jobs: [], bounce: null } });
    flushSync();
    expect(target.querySelector('[data-test="audio-clip-rendering"]')).toBeNull();
    expect(target.querySelector('[data-test="clip-rendering"]')).toBeNull();
    expect(target.querySelector(".message")?.textContent ?? "").not.toContain("音を作っています");
  });
});

describe("the number of job table lookups does not grow with the number of clips", () => {
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

  /** `clips` audio clips (1 bar each), `jobs` of them "making sound". Counts how many times the table is read. */
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
    app.setState({ ...base, song: { ...base.song, tracks: base.song.tracks.map((tr) => (tr.id === vocal.id ? track : tr)) }, audioWork: { jobs: watched, bounce: null } });
    mounted = mount(AudioClipNotes, { target, props: { view: { ...view, width: 1e7 }, dims, tracks: [track] } });
    flushSync();
    expect(target.querySelectorAll('[data-test="audio-clip-rendering"]').length).toBe(jobs);
    unmount(mounted);
    mounted = null;
    shownWork.forget();
    return reads;
  }

  it("192 clips with 190 jobs read the table only a few times the number of jobs (not clips × jobs)", () => {
    const reads192 = count(192, 190);
    expect(reads192).toBeLessThanOrEqual(190 * 4);
    expect(count(380, 190)).toBe(reads192);
  });
});
