// Round 12, A9: 「逆にする」 and 「ノーマライズ」 of audio clips on the screen: the two rows at the bottom of an audio clip's
// menu, what the fake does with them (one step each, as Rust once its audio thread answers), and what the status line says.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, AudioClipView, Command, StatusMessage, TrackView } from "../src/bridge";
import { createFakeTransport, FAKE_AUDIO, fakeScene } from "../src/bridge/fake";
import { fakeAudioClip, fakePeakValues, fakeVocalClip, steadyTempo } from "../src/bridge/fakeAudio";
import Arrange from "../src/components/Arrange.svelte";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
// Audio clips ask the bridge for peaks; these tests do not draw waves.
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  audioPeaks: () => new Promise(() => {}),
  moreAudioPeaks: () => new Promise(() => {}),
  peekAudioPeaks: () => null,
}));

const BAR = 4 * TPQ;
const VOCAL = 16;
const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => 1000 }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

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
  contextMenu.open = null;
  target.remove();
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target, props: {} });
  flushSync();
}

const rows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");
const row = (label: string) => rows().find((r) => r.label === label)!;

/** Row 1 「ピアノ」 with one clip; row 2 「歌」 with two audio clips over bars 1–2 and 3–4. Rows are 64 high, a bar 64 wide. */
const song = (selected: number[], edit: (s: AppState) => void = () => {}) =>
  app.stateWith((s) => {
    const base = s.song.tracks[0]!;
    const audio = fakeAudioClip({ id: 7, file: "歌.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 8 * 48000 });
    const vocal: TrackView = {
      ...base,
      id: 3,
      number: 2,
      name: "歌",
      kind: { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false },
      clips: [fakeVocalClip(61, "歌", 0, 2 * BAR, audio), fakeVocalClip(63, "歌 2", 2 * BAR, 4 * BAR, audio)],
    };
    s.song.tracks = [{ ...base, clips: [{ ...fakeVocalClip(1, "イントロ", 0, 4 * BAR, audio), audio: null }] }, vocal];
    s.arrange = { selectedClips: selected, selectedClip: selected[0] ?? null, pianoRollClip: null, clipboardClips: 0, clipboardKind: null, tallTrack: null };
    edit(s);
  });

function rightClick(x: number, y: number): void {
  target.querySelector(".lanes")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 2 }));
  flushSync();
}

describe("the rows at the bottom of an audio clip's menu (spec 3)", () => {
  it("「逆にする」 and 「ノーマライズ」 send the selected clips; Rust takes the audio clips among them", () => {
    show(Arrange, song([61, 63, 1]));
    rightClick(20, 64 + 30);
    const labels = (contextMenu.open?.entries ?? []).slice(-3).map((e) => (e.kind === "row" ? e.label : "-"));
    expect(labels).toEqual(["-", "menu.reverse", "menu.normalize"]);
    expect([row("menu.reverse").usable, row("menu.normalize").usable]).toEqual([true, true]);
    row("menu.reverse").run();
    row("menu.normalize").run();
    expect(app.sent).toEqual([
      { type: "reverseClips", clips: [61, 63, 1] },
      { type: "normalizeClips", clips: [61, 63, 1] },
    ]);
  });

  it("a clip that is not selected is selected first, and the rows go to it alone", () => {
    show(Arrange, song([]));
    rightClick(64 * 2 + 20, 64 + 30);
    expect(app.sent).toEqual([{ type: "selectClip", clip: 63 }]);
    // The selection Rust sends back is what the rows take.
    app.setState(song([63]));
    flushSync();
    row("menu.normalize").run();
    expect(app.sent.at(-1)).toEqual({ type: "normalizeClips", clips: [63] });
  });

  it("while recording or counting in they cannot be used; a keys clip has neither", () => {
    show(
      Arrange,
      song([61], (s) => {
        s.transport.state = "recording";
      }),
    );
    rightClick(20, 64 + 30);
    expect([row("menu.reverse").usable, row("menu.normalize").usable]).toEqual([false, false]);
    contextMenu.open = null;
    show(Arrange, song([1]));
    rightClick(20, 30);
    expect(rows().map((r) => r.label)).not.toContain("menu.reverse");
    expect(rows().map((r) => r.label)).not.toContain("menu.normalize");
  });
});

const vocalOf = (s: AppState) => s.song.tracks.find((t) => t.id === VOCAL)!;
const audioOf = (s: AppState, clip: number): AudioClipView => vocalOf(s).clips.find((c) => c.id === clip)!.audio!;

async function opened(name: string) {
  const f = createFakeTransport(fakeScene(name)!.state);
  const send = async (c: Command) => {
    await f.send(c);
    return (await f.snapshot()).state as AppState;
  };
  return { f, send, now: async () => (await f.snapshot()).state as AppState };
}

describe("the fake (one step each)", () => {
  it("normalize: the loudest of the shown take at −1.0 dB, within −60..+12; told when it stops there", async () => {
    const { send, now } = await opened("audioEdit");
    const before = await now();
    const file = FAKE_AUDIO.get(audioOf(before, 72).audio)!;
    let peak = 0;
    for (const v of fakePeakValues(file, 0, 0, 10_000)) peak = Math.max(peak, Math.abs(v));
    const want = Math.round((-1 - 20 * Math.log10(peak / 32767)) * 100) / 100;
    let s = await send({ type: "normalizeClips", clips: [72, 999] });
    expect(audioOf(s, 72).gainDb).toBe(Math.min(12, Math.max(-60, want)));
    expect([s.song.undoCount, s.song.file]).toEqual([before.song.undoCount + 1, "changed"]);
    // Again: nothing changes, no step.
    s = await send({ type: "normalizeClips", clips: [72] });
    expect(s.song.undoCount).toBe(before.song.undoCount + 1);
    // An id of nothing, or a keys clip: nothing.
    s = await send({ type: "normalizeClips", clips: [999, s.song.tracks[0]!.clips[0]?.id ?? 998] });
    expect(s.song.undoCount).toBe(before.song.undoCount + 1);
  });

  it("reverse: a new file and a new take in the same place, its tick 0 at the clip's start; the waves come backwards", async () => {
    const { f, send, now } = await opened("audioEdit");
    const before = await now();
    const old = vocalOf(before).clips.find((c) => c.id === 72)!;
    const s = await send({ type: "reverseClips", clips: [72] });
    const clip = vocalOf(s).clips.find((c) => c.id === 72)!;
    expect([clip.start, clip.end, clip.takeOffset]).toEqual([old.start, old.end, 0]);
    expect(clip.takeId).not.toBe(old.takeId);
    const audio = clip.audio!;
    expect(audio.audio).not.toBe(old.audio!.audio);
    expect(audio.file).toBe(`歌 ${audio.audio}.wav`);
    expect([audio.firstFrame, audio.frames, audio.gainDb, audio.fadeIn]).toEqual([0, old.audio!.frames, old.audio!.gainDb, old.audio!.fadeIn]);
    expect(s.song.undoCount).toBe(before.song.undoCount + 1);
    // The other clip of the same file is as it was.
    expect(vocalOf(s).clips.find((c) => c.id === 70)).toEqual(vocalOf(before).clips.find((c) => c.id === 70));
    // The new file's peaks are the old ones the other way round.
    const made = FAKE_AUDIO.get(audio.audio)!;
    const first = fakePeakValues(made, 0, 0, 1);
    const source = FAKE_AUDIO.get(old.audio!.audio)!;
    const last = Math.floor((old.audio!.frames - 1) / 256);
    expect(first).toEqual(fakePeakValues(source, 0, last, 1));
    await expect(f.peaks(audio.audio, 0, 0, 10)).resolves.toBeInstanceOf(ArrayBuffer);
  });

  it("reverse of a clip whose audio changes tempo: it does not follow any more, and the status line says so", async () => {
    const changing = fakeScene("audioEdit")!.state;
    const clip = vocalOf(changing).clips.find((c) => c.id === 72)!;
    clip.audio!.sourceTempo = { ...steadyTempo(96), tempoPoints: [{ tick: BAR, tempoBpm: 120, ramp: false }] };
    const f = createFakeTransport(changing);
    await f.send({ type: "reverseClips", clips: [72] });
    const s = (await f.snapshot()).state as AppState;
    expect(audioOf(s, 72).stretch.follow).toBe(false);
    expect(audioOf(s, 72).sourceTempo).toBeNull();
    expect(s.status.message).toEqual({ id: "reverseFollowOff", count: 1 });
  });

  it("while recording nothing changes", async () => {
    const state = fakeScene("audioEdit")!.state;
    state.transport.state = "recording";
    const f = createFakeTransport(state);
    const before = (await f.snapshot()).state as AppState;
    await f.send({ type: "reverseClips", clips: [72] });
    await f.send({ type: "normalizeClips", clips: [72] });
    expect(((await f.snapshot()).state as AppState).song).toEqual(before.song);
  });
});

describe("what the status line says", () => {
  const said = (message: StatusMessage) => {
    show(
      StatusLine,
      app.stateWith((s) => {
        s.status.message = message;
      }),
    );
    return target.querySelector("footer.status")?.textContent?.replace(/\s+/g, " ").trim() ?? "";
  };
  it("normalize that stopped at an end, normalize of silence, reverse that stopped following", () => {
    expect(said({ id: "normalizeLimited", count: 2 })).toContain("2 つのクリップは、いちばん大きい音を −1.0 dB にできませんでした（クリップの音量は −60 〜 +12 dB まで）。");
    expect(said({ id: "normalizeSilent", count: 1 })).toContain("1 つのクリップは音がないので、そのままにしました。");
    expect(said({ id: "reverseFollowOff", count: 1 })).toContain("1 つのクリップは、もとの音のテンポが途中で変わるので、「曲のテンポについていく」をオフにしました。");
  });

  it("a file that could not be read or written", () => {
    expect(said({ id: "clipWorkFailed", count: 2 })).toContain("2 つのクリップは、音のファイルを読めないか書けないので、そのままにしました。");
  });

  it("the scene audioNormalized shows the right clip at +12.0 dB and the text", () => {
    const s = fakeScene("audioNormalized")!.state;
    expect(audioOf(s, 72).gainDb).toBe(12);
  });
});
