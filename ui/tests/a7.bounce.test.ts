// Round 12, A7: 「音のクリップにする」 and 「このトラックを音にする」 on the screen: the rows of the keys clip's and the keys track
// head's right-click menus, what the fake does with them (one step, as Rust once the file is ready), and the status line.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, StatusMessage, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene, type FakeState } from "../src/bridge/fake";
import { fakeAudioClip, fakeVocalClip } from "../src/bridge/fakeAudio";
import Arrange from "../src/components/Arrange.svelte";
import { bounceable } from "../src/components/arrange/bounce";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { trackContextMenu } from "../src/components/trackMenu";
import { t } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  audioPeaks: () => new Promise(() => {}),
  moreAudioPeaks: () => new Promise(() => {}),
  peekAudioPeaks: () => null,
}));

const BAR = 4 * TPQ;
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

/** Row 1 「ピアノ」 with two keys clips (bars 1–4, 5–8); row 2 「ベース」 with one (bars 1–4); row 3 「歌」 with an audio clip. */
const song = (selected: number[], edit: (s: AppState) => void = () => {}) =>
  app.stateWith((s) => {
    const base = s.song.tracks[0]!;
    const keys = (id: number, name: string, from: number, to: number) => ({ ...fakeVocalClip(id, name, from, to, fakeAudioClip({ id: 1, file: "", sampleRate: 48000, bits: 24, channels: 1, frames: 1 })), audio: null });
    const audio = fakeAudioClip({ id: 7, file: "歌.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 8 * 48000 });
    const vocal: TrackView = {
      ...base,
      id: 3,
      number: 3,
      name: "歌",
      kind: { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false },
      clips: [fakeVocalClip(61, "歌", 0, 2 * BAR, audio)],
    };
    s.song.tracks = [
      { ...base, clips: [keys(1, "イントロ", 0, 4 * BAR), keys(3, "Aメロ", 4 * BAR, 8 * BAR)] },
      { ...base, id: 2, number: 2, name: "ベース", clips: [keys(41, "ベース A", 0, 4 * BAR)] },
      vocal,
    ];
    s.arrange = { selectedClips: selected, selectedClip: selected[0] ?? null, pianoRollClip: null, clipboardClips: 0, tallTrack: null };
    edit(s);
  });

function rightClick(x: number, y: number): void {
  target.querySelector(".lanes")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 2 }));
  flushSync();
}

describe("「音のクリップにする」 at the bottom of a keys clip's menu", () => {
  it("sends the selected clips; usable while the keys clips among them are on one track", () => {
    show(Arrange, song([1, 3, 61]));
    rightClick(20, 30);
    const labels = (contextMenu.open?.entries ?? []).slice(-2).map((e) => (e.kind === "row" ? e.label : "-"));
    expect(labels).toEqual(["-", "menu.bounceClips"]);
    expect(row("menu.bounceClips").usable).toBe(true);
    row("menu.bounceClips").run();
    expect(app.sent).toEqual([{ type: "bounceToAudio", clips: [1, 3, 61] }]);
    expect(t("menu.bounceClips")).toBe("音のクリップにする");
  });

  it("is disabled for keys clips of two tracks and while recording; an audio clip does not have it", () => {
    show(Arrange, song([1, 41]));
    rightClick(20, 30);
    expect(row("menu.bounceClips").usable).toBe(false);
    contextMenu.open = null;
    show(
      Arrange,
      song([1], (s) => {
        s.transport.state = "recording";
      }),
    );
    rightClick(20, 30);
    expect(row("menu.bounceClips").usable).toBe(false);
    contextMenu.open = null;
    show(Arrange, song([61]));
    rightClick(20, 2 * 64 + 30);
    expect(rows().map((r) => r.label)).not.toContain("menu.bounceClips");
  });

  it("bounceable: the keys clips among the ids, on one track", () => {
    const s = song([]);
    expect(bounceable(s.song.tracks, [1, 3])).toBe(true);
    expect(bounceable(s.song.tracks, [1, 61, 999])).toBe(true);
    expect(bounceable(s.song.tracks, [1, 41])).toBe(false);
    expect(bounceable(s.song.tracks, [61])).toBe(false);
    expect(bounceable(s.song.tracks, [])).toBe(false);
  });
});

describe("「このトラックを音にする」 in the right-click menu of a keys track's head", () => {
  const menu = (s: AppState, index: number, sent: Command[]) =>
    trackContextMenu(s, s.song.tracks[index]!, false, (c) => sent.push(c), (l) => l).filter((e): e is MenuRow => e.kind === "row");

  it("comes after the record arm, in the place a vocal track has 「音のファイルを入れる…」, and sends the track", () => {
    const s = song([]);
    const sent: Command[] = [];
    const list = menu(s, 1, sent);
    const at = list.findIndex((r) => r.label === "menu.bounceTrack");
    expect(list[at - 1]!.label).toBe("menu.arm");
    expect(list[at + 1]!.label).toBe("menu.renameTrack");
    expect(list[at]!.usable).toBe(true);
    list[at]!.run();
    expect(sent).toEqual([{ type: "bounceTrack", track: 2 }]);
    expect(t("menu.bounceTrack")).toBe("このトラックを音にする");
    expect(menu(s, 2, []).map((r) => r.label)).not.toContain("menu.bounceTrack");
  });

  it("is disabled while recording", () => {
    const s = song([], (x) => {
      x.transport.state = "countIn";
    });
    expect(menu(s, 0, []).find((r) => r.label === "menu.bounceTrack")!.usable).toBe(false);
  });
});

async function opened(state: FakeState) {
  const f = createFakeTransport(state);
  const send = async (c: Command) => {
    await f.send(c);
    return (await f.snapshot()).state as AppState;
  };
  return { send, now: async () => (await f.snapshot()).state as AppState };
}

describe("the fake (one step, at once)", () => {
  it("the track's audio on a new vocal track right below, 「（元の名前）（音）」 with the fader, pan and sends; the track muted", async () => {
    const { send, now } = await opened(fakeScene("muted")!.state);
    const before = await now();
    const from = before.song.tracks[0]!;
    const s = await send({ type: "bounceTrack", track: from.id });
    expect(s.song.undoCount).toBe(before.song.undoCount + 1);
    expect(s.song.tracks.length).toBe(before.song.tracks.length + 1);
    const [was, made] = [s.song.tracks[0]!, s.song.tracks[1]!];
    expect(was.muted).toBe(true);
    expect([made.number, made.name, made.kind.kind]).toEqual([2, `${from.name}（音）`, "vocal"]);
    expect([made.gainDb, made.pan, made.sends]).toEqual([from.gainDb, from.pan, from.sends]);
    expect(made.slots).toEqual([null, null, null, null]);
    const clip = made.clips[0]!;
    expect(clip.start).toBe(from.clips[0]!.start);
    expect(clip.audio!.stretch.follow).toBe(false);
    expect(s.song.tracks.slice(2).map((x) => x.number)).toEqual(before.song.tracks.slice(1).map((x) => x.number + 1));
    expect(s.status.message).toEqual({ id: "bounced", track: { number: 2, name: made.name } });
  });

  it("refuses as Rust does: no clip, a job running, the export's dialog, recording", async () => {
    const empty = fakeScene("muted")!.state;
    empty.song.tracks[0]!.clips = [];
    let s = await (await opened(empty)).send({ type: "bounceTrack", track: empty.song.tracks[0]!.id });
    expect(s.status.message).toEqual({ id: "nothingToBounce" });

    const running = fakeScene("bouncing")!.state;
    const { send } = await opened(running);
    s = await send({ type: "bounceTrack", track: running.song.tracks[1]!.id });
    expect(s.status.message).toEqual({ id: "bounceBusy" });
    s = await send({ type: "cancelBounce" });
    expect(s.audioWork.bounce).toBeNull();
    expect(s.status.message).toEqual({ id: "bounceCancelled" });

    const recording = fakeScene("muted")!.state;
    recording.transport.state = "recording";
    const r = await opened(recording);
    const before = await r.now();
    s = await r.send({ type: "bounceTrack", track: recording.song.tracks[0]!.id });
    expect(s.song).toEqual(before.song);
  });
});

describe("what the status line says", () => {
  const said = (state: AppState) => {
    show(StatusLine, state);
    return target.querySelector("footer.status")?.textContent?.replace(/\s+/g, " ").trim() ?? "";
  };
  const with_ = (message: StatusMessage) =>
    said(
      app.stateWith((s) => {
        s.status.message = message;
      }),
    );

  it("「音にしています… 40 %」 and 「やめる」 while it runs; 「やめる」 sends cancelBounce", () => {
    const text = said(fakeScene("bouncing")!.state);
    expect(text).toContain("音にしています… 40 %");
    const stop = [...target.querySelectorAll("button")].find((b) => b.textContent === t("button.cancel"))!;
    stop.click();
    expect(app.sent).toEqual([{ type: "cancelBounce" }]);
  });

  it("the messages", () => {
    expect(with_({ id: "bounced", track: { number: 2, name: "ピアノ（音）" } })).toContain("2 ピアノ（音） に音のクリップを置きました。もとのトラックはミュートしました。");
    expect(with_({ id: "bounceCancelled" })).toContain("音にするのをやめました。");
    expect(with_({ id: "bounceFailed", reason: { kind: "plugin", name: "Surge XT" } })).toContain("音にできませんでした。Surge XT が止まりました。");
    expect(with_({ id: "bounceFailed", reason: { kind: "cannotWrite" } })).toContain("音にできませんでした。作業の場所に書けません。");
    expect(with_({ id: "bounceGone" })).toContain("もとのトラックがなくなったので、できた音はすてました。");
    expect(with_({ id: "bounceBusy" })).toContain("書き出しか「音にする」の途中なので、終わってからもう一度ためしてください。");
    expect(with_({ id: "nothingToBounce" })).toContain("このトラックにはクリップがありません。");
    expect(with_({ id: "exportWaitsForBounce" })).toContain("音にしているあいだは書き出せません。終わってからひらいてください。");
  });

  it("the scene bounced: the piano muted, 「ピアノ（音）」 right below", () => {
    const s = fakeScene("bounced")!.state;
    expect(s.song.tracks[0]!.muted).toBe(true);
    expect(s.song.tracks[1]!.name).toBe(`${s.song.tracks[0]!.name}（音）`);
  });
});
