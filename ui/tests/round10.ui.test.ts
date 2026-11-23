// Round 10 on the screen (spec 3 "Rules added in round 10", 4, 7 A-27; mocks P10-ClipName and P10-Overdub):
// the context menu rows, the clip name box, F2, Mod J, double presses, the marker row, 「重ねる」 and its texts.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, TrackView } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import { fakeAudioClip, fakeVocalClip } from "../src/bridge/fakeAudio";
import Arrange from "../src/components/Arrange.svelte";
import { joinable, joinTarget, overdubClipName, renameClipTarget, splitTick } from "../src/components/clipEdits";
import { clipEdit, renameCommand, stopClipRename } from "../src/components/clipName.svelte";
import { transportWithOverdub } from "../src/components/fit";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { trackEdit } from "../src/components/tracks.svelte";
import Transport from "../src/components/Transport.svelte";
import { handleKeydown, keyActionUsable, type KeyDeps } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { findTerm, t, TERM_SEPARATOR } from "../src/text/ja";
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

const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url; // Vite rewrites a direct new URL(…, import.meta.url) into an import
const readProposal = (board: string) => fs.readFileSync(new URL(`../../docs/design/proposals/${board}.dc.html`, here), "utf8");

const BAR = 4 * TPQ;
const WIDTH = 1000;
const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  stopClipRename();
  trackEdit.renaming = null;
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  contextMenu.open = null;
  stopClipRename();
  target.remove();
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const rows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");
const labels = () => (contextMenu.open?.entries ?? []).map((e) => (e.kind === "row" ? e.label : "-"));

function clip(id: number, name: string, fromBar: number, toBar: number, take = 1, takes = 1): ClipView {
  return { id, name, start: (fromBar - 1) * BAR, end: (toBar - 1) * BAR, take, takes, takeId: 100 + id, takeOffset: 0, noteCount: 0, audio: null };
}
const AUDIO = { id: 7, file: "歌.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 8 * 48000 };
const vocalClip = (id: number, name: string, fromBar: number, toBar: number) => fakeVocalClip(id, name, (fromBar - 1) * BAR, (toBar - 1) * BAR, fakeAudioClip(AUDIO));

/**
 * Row 1 ピアノ: イントロ 1–5, Aメロ 5–9, サビ 11–13 (a gap with nothing between Aメロ and サビ). Row 2 ベース: ベース A 5–9.
 * Row 3 歌 (vocal): 歌 1–3. Rows are 64 high, a bar is 64 wide.
 */
const song = (selected: number[] = [], clicked: number | null = null, edit: (s: AppState) => void = () => {}) =>
  app.stateWith((s) => {
    const base = s.song.tracks[0]!;
    const vocal: TrackView = {
      ...base,
      id: 3,
      number: 3,
      name: "歌",
      kind: { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false },
      clips: [vocalClip(61, "歌", 1, 3)],
    };
    s.song.tracks = [
      { ...base, clips: [clip(1, "イントロ", 1, 5), clip(3, "Aメロ", 5, 9), clip(5, "サビ", 11, 13)] },
      { ...base, id: 2, number: 2, name: "ベース", color: 2, clips: [clip(41, "ベース A", 5, 9)] },
      vocal,
    ];
    s.song.playTarget = 1;
    s.song.keysTarget = 1;
    s.arrange = { selectedClips: selected, selectedClip: clicked, pianoRollClip: clicked, clipboardClips: 0, tallTrack: null };
    edit(s);
  });

function mouse(el: Element, type: string, x: number, y: number, more: MouseEventInit = {}): MouseEvent {
  const ev = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: type === "contextmenu" ? 2 : 0, ...more });
  el.dispatchEvent(ev);
  flushSync();
  return ev;
}
const lanes = () => q(".lanes")!;

describe("つなげる ・ ここで切る ・ F2 の向き先の決まり（clipEdits.ts）", () => {
  const tracks = song().song.tracks;
  it("同じトラックの鍵盤のクリップを 2 つ以上、間に選んでいないクリップがないときだけ", () => {
    expect(joinable(tracks, [1, 3])).toBe(true);
    // あいだがあいていても、ほかのクリップがなければつなげる（あいた所は空の部分）。
    expect(joinable(tracks, [3, 5])).toBe(true);
    expect(joinable(tracks, [1, 5])).toBe(false);
    expect(joinable(tracks, [1, 3, 5])).toBe(true);
    expect(joinable(tracks, [3])).toBe(false);
    expect(joinable(tracks, [3, 41])).toBe(false);
    expect(joinable(tracks, [61, 3])).toBe(false);
    expect(joinable(tracks, [3, 999])).toBe(false);
  });
  it("ここで切るは、クリップのはしでは使えない", () => {
    const c = clip(3, "Aメロ", 5, 9);
    expect(splitTick(c, 4 * BAR)).toBeNull();
    expect(splitTick(c, 8 * BAR)).toBeNull();
    expect(splitTick(c, 5 * BAR)).toBe(5 * BAR);
  });
  it("F2 のクリップは最後に押したクリップ、なければ選んでいる最初のクリップ", () => {
    expect(renameClipTarget(song([1, 3], 3))).toBe(3);
    expect(renameClipTarget(song([1, 3], null))).toBe(1);
    expect(renameClipTarget(song([], null))).toBeNull();
    expect(joinTarget(song([1, 3], 3))).toEqual([1, 3]);
    expect(joinTarget(song([3, 41], 3))).toBeNull();
  });
  it("名前を決める: 空なら古い名前のまま、同じ名前は手にしない", () => {
    expect(renameCommand(3, "Aメロ", "  サビ ")).toEqual({ type: "renameClip", clip: 3, name: "サビ" });
    expect(renameCommand(3, "Aメロ", "  ")).toBeNull();
    expect(renameCommand(3, "Aメロ", "Aメロ")).toBeNull();
  });
});

describe("アレンジの右クリック（仕様書 3 章、第 10 回）", () => {
  it("歌のクリップ: ピアノロールでひらく ・ つなげるがない。逆にする ・ ノーマライズは使えない行のまま", () => {
    show(Arrange, song([61], 61));
    mouse(lanes(), "contextmenu", 64, 150);
    expect(labels()).toEqual([
      "menu.cut",
      "menu.copy",
      "menu.paste",
      "menu.duplicate",
      "menu.delete",
      "-",
      "menu.splitHere",
      "menu.renameClip",
      "-",
      "menu.nextTake",
      "menu.prevTake",
      "menu.deleteTake",
      "-",
      "menu.reverse",
      "menu.normalize",
    ]);
    const by = (label: string) => rows().find((r) => r.label === label)!;
    expect([by("menu.splitHere").usable, by("menu.reverse").usable, by("menu.normalize").usable]).toEqual([true, false, false]);
    by("menu.splitHere").run();
    expect(app.sent).toEqual([{ type: "splitClipAt", clip: 61, tick: BAR }]);
  });

  it("ここで切る: 寄せるとクリップのはしになる所では使えない", () => {
    show(Arrange, song([3], 3));
    // 4.3 小節目は 5 小節目の線（Aメロの頭）に寄る。
    mouse(lanes(), "contextmenu", 4.3 * 64, 30);
    expect(rows().find((r) => r.label === "menu.splitHere")!.usable).toBe(false);
  });

  it("つなげる: 同じトラックで 2 つ選んでいれば使える。名前を変えるは名前の箱をひらく", () => {
    show(Arrange, song([1, 3], 3));
    mouse(lanes(), "contextmenu", 300, 30);
    const join = rows().find((r) => r.label === "menu.join")!;
    expect([join.usable, join.key]).toEqual([true, "Ctrl J"]);
    join.run();
    expect(app.sent).toEqual([{ type: "joinClips", clips: [1, 3] }]);
    rows().find((r) => r.label === "menu.renameClip")!.run();
    flushSync();
    expect(clipEdit.renaming).toBe(3);
    expect(q<HTMLInputElement>('[data-test="clip-name-field"]')!.value).toBe("Aメロ");
  });

  it("あいた所: 鍵盤のトラックは「ここにクリップを作る」、歌のトラックは「音のファイルを入れる…」、トラックの外は貼りつけだけ", () => {
    show(Arrange, song());
    mouse(lanes(), "contextmenu", 600, 150);
    expect(rows().map((r) => [r.label, r.usable])).toEqual([
      ["menu.paste", false],
      ["menu.importAudio", true],
    ]);
    rows()[1]!.run();
    expect(app.sent).toEqual([{ type: "importAudio", track: 3 }]);
    mouse(lanes(), "contextmenu", 600, 400);
    expect(rows().map((r) => r.label)).toEqual(["menu.paste"]);
  });

  it("録音中は、ここで切る ・ つなげる ・ 名前を変える ・ ここにクリップを作るが使えない", () => {
    const s = song([1, 3], 3, (s) => (s.transport.state = "recording"));
    show(Arrange, s);
    mouse(lanes(), "contextmenu", 300, 30);
    expect(["menu.splitHere", "menu.join", "menu.renameClip"].map((l) => rows().find((r) => r.label === l)!.usable)).toEqual([false, false, false]);
    mouse(lanes(), "contextmenu", 100, 100);
    expect(rows().find((r) => r.label === "menu.createClipHere")!.usable).toBe(false);
  });
});

describe("2 回押す（アレンジ）", () => {
  it("鍵盤のトラックのあいた所: createClip（押した所のティック）。歌のトラックのあいた所: 何もしない", () => {
    show(Arrange, song());
    mouse(lanes(), "dblclick", 100, 100);
    mouse(lanes(), "dblclick", 600, 150);
    expect(app.sent).toEqual([{ type: "createClip", track: 2, tick: Math.round((100 * BAR) / 64) }]);
  });

  it("歌のクリップの波の所は、いままでどおり行を 160 に。名前の所は名前の箱", () => {
    show(Arrange, song());
    mouse(lanes(), "dblclick", 64, 128 + 40);
    expect(app.sent).toEqual([{ type: "setTallTrack", track: 3 }]);
    mouse(lanes(), "dblclick", 64, 128 + 10);
    expect(clipEdit.renaming).toBe(61);
    expect(app.sent).toHaveLength(1);
  });

  it("鍵盤のクリップの名前の所: 名前の箱。Enter で renameClip を 1 つ、Esc ・ 外を押すと送らずに閉じる、空なら古い名前のまま", () => {
    show(Arrange, song([3], 3));
    // 本体を 2 回押しても何もしない。
    mouse(lanes(), "dblclick", 300, 40);
    expect(clipEdit.renaming).toBeNull();
    const open = () => {
      mouse(lanes(), "dblclick", 300, 10);
      return q<HTMLInputElement>('[data-test="clip-name-field"]')!;
    };
    const key = (field: HTMLInputElement, k: string) => {
      field.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
      flushSync();
    };
    let field = open();
    // 箱を出している間、そのクリップの名前の札は出さない。
    expect([...target.querySelectorAll(".clip-label")].map(text)).not.toContain("Aメロ");
    field.value = "サビ 2";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    key(field, "Enter");
    expect(app.sent).toEqual([{ type: "renameClip", clip: 3, name: "サビ 2" }]);
    expect(q('[data-test="clip-name-field"]')).toBeNull();

    field = open();
    field.value = "ちがう";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    key(field, "Escape");
    field = open();
    field.value = "ちがう";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    field.dispatchEvent(new FocusEvent("blur"));
    flushSync();
    field = open();
    field.value = "";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    key(field, "Enter");
    expect(app.sent).toHaveLength(1);
  });

  it("箱はクリップの名前の所いっぱい（打った名前が長ければ、右へのびる）", () => {
    show(Arrange, song([3], 3));
    mouse(lanes(), "dblclick", 300, 10);
    const box = q(".clip-naming")!;
    expect(box.style.left).toBe("257px");
    expect(box.style.minWidth).toBe("254px");
  });
});

describe("キー: F2 と Mod J（仕様書 4 章）", () => {
  const deps = (state: AppState, doc: Document = document): KeyDeps => ({ state: () => state, send: app.send, nav: { platform: "Linux x86_64" }, playTick: () => 0, doc });
  const press = (state: AppState, init: KeyboardEventInit) => handleKeydown(new KeyboardEvent("keydown", { cancelable: true, ...init }), deps(state));
  const input = (k: Partial<KeyInput>): KeyInput => ({ key: "", ctrl: false, meta: false, alt: false, shift: false, ...k });

  it("Mod J は文字を打っている最中も「つなげる」", () => {
    expect(keyAction(input({ key: "j", ctrl: true }), false, "other")).toEqual({ kind: "joinClips" });
    expect(keyAction(input({ key: "j", ctrl: true }), true, "other")).toEqual({ kind: "joinClips" });
    expect(keyAction(input({ key: "j", meta: true }), false, "mac")).toEqual({ kind: "joinClips" });
    expect(keyAction(input({ key: "j" }), false, "other")).toBeNull();
  });

  it("Mod J: つなげられるときだけ joinClips を 1 つ。録音中は送らない", () => {
    press(song([1, 3], 3), { key: "j", ctrlKey: true });
    press(song([3, 41], 3), { key: "j", ctrlKey: true });
    press(song([1, 3], 3, (s) => (s.transport.state = "recording")), { key: "j", ctrlKey: true });
    expect(app.sent).toEqual([{ type: "joinClips", clips: [1, 3] }]);
    expect(keyActionUsable({ kind: "joinClips" }, deps(song([1, 3], 3)))).toBe(true);
    expect(keyActionUsable({ kind: "joinClips" }, deps(song([3], 3)))).toBe(false);
  });

  it("F2: クリップを選んでいればクリップの名前の箱、トラックだけならトラックの名前。録音中は何もしない", () => {
    press(song([3], 3), { key: "F2" });
    expect([clipEdit.renaming, trackEdit.renaming]).toEqual([3, null]);
    stopClipRename();
    press(song(), { key: "F2" });
    expect([clipEdit.renaming, trackEdit.renaming]).toEqual([null, 1]);
    trackEdit.renaming = null;
    press(song([3], 3, (s) => (s.transport.state = "recording")), { key: "F2" });
    expect([clipEdit.renaming, trackEdit.renaming]).toEqual([null, null]);
    expect(app.sent).toEqual([]);
  });
});

describe("マーカーの行のあいた所（仕様書 3 章 Y18、第 10 回）", () => {
  it("2 回押す ・ 右クリックの「ここにマーカーを足す」: K と同じ名前で、押した所のティック", () => {
    show(Arrange, song());
    const row = q('[data-test="marker-row"]')!;
    mouse(row, "dblclick", 128, 10);
    mouse(row, "contextmenu", 192, 10);
    expect(rows().map((r) => [r.label, r.usable])).toEqual([["menu.addMarkerHere", true]]);
    rows()[0]!.run();
    expect(app.sent).toEqual([
      { type: "addMarker", name: "マーカー 1", tick: 2 * BAR },
      { type: "addMarker", name: "マーカー 1", tick: 3 * BAR },
    ]);
  });

  it("録音中は 2 回押しても足さず、行は使えない", () => {
    show(Arrange, song([], null, (s) => (s.transport.state = "recording")));
    const row = q('[data-test="marker-row"]')!;
    mouse(row, "dblclick", 128, 10);
    mouse(row, "contextmenu", 192, 10);
    expect(rows()[0]!.usable).toBe(false);
    expect(app.sent).toEqual([]);
  });
});

describe("トラックの見出しの「ピアノロールでひらく」（第 10 回）", () => {
  const open = (s: AppState) => {
    show(Arrange, s);
    mouse(target.querySelectorAll("[data-test=track-head]")[0]!, "contextmenu", 10, 10);
    const row = rows()[0]!;
    expect([row.label, row.key, row.usable]).toEqual(["menu.openInPianoRoll", "E", true]);
    app.sent.length = 0;
    row.run();
  };
  it("意図を 1 つ送る（どのクリップを見せるかは core が決める）", () => {
    for (const at of [5 * BAR, 9.5 * BAR]) {
      open(song([], null, (s) => (s.transport.position = at)));
      expect(app.sent).toEqual([{ type: "openPianoRoll", track: 1 }]);
    }
  });
  it("歌のトラックには出さない", () => {
    show(Arrange, song());
    mouse(target.querySelectorAll("[data-test=track-head]")[2]!, "contextmenu", 10, 10);
    expect(rows().map((r) => r.label)).not.toContain("menu.openInPianoRoll");
  });
});

describe("右の欄「選んだクリップ」の名前（モック P10-ClipName (b)）", () => {
  const scene = () => fakeScene("audioEdit")!.state as FakeState;
  it("名前を押すとその場で打てる。打つ間は「選んだクリップ」を隠す。Enter で renameClip", () => {
    show(ParamsPanel, scene(), { onfold: () => {} });
    expect(text(q('[data-test="clip-params"]'))).toContain(t("clipParams.selected"));
    q('[data-test="params-name"]')!.click();
    flushSync();
    const field = q<HTMLInputElement>('[data-test="params-name-field"]')!;
    expect(field.value).toBe("歌 テイク 1");
    expect(text(q('[data-test="clip-params"]'))).not.toContain(t("clipParams.selected"));
    field.value = "歌 サビ";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    field.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    flushSync();
    expect(app.sent).toEqual([{ type: "renameClip", clip: expect.any(Number), name: "歌 サビ" }]);
    expect(q('[data-test="params-name-field"]')).toBeNull();
    // アレンジの箱は出していない（場所が別）。
    expect(clipEdit.renaming).toBeNull();
  });
});

describe("「重ねる」（モック P10-Overdub の A）", () => {
  const ready = (edit: (s: AppState) => void = () => {}) =>
    song([], null, (s) => {
      s.transport.canPlay = true;
      s.transport.canRecord = true;
      edit(s);
    });

  it("ループの右。オフ ・ オン（accent）・ オンで録音中も accent。押すと setOverdub", () => {
    show(Transport, ready());
    const button = q('[data-test="transport-overdub"]')!;
    expect(button.previousElementSibling?.getAttribute("data-test")).toBe("transport-loop");
    expect([text(button), button.classList.contains("on")]).toEqual(["重ねる", false]);
    button.click();
    expect(app.sent).toEqual([{ type: "setOverdub", on: true }]);
    show(Transport, ready((s) => (s.transport.overdub = true)));
    expect(q('[data-test="transport-overdub"]')!.classList.contains("on")).toBe(true);
    q('[data-test="transport-overdub"]')!.click();
    show(Transport, ready((s) => Object.assign(s.transport, { overdub: true, state: "recording" })));
    expect(q('[data-test="transport-overdub"]')!.classList.contains("on")).toBe(true);
    expect(app.sent.at(-1)).toEqual({ type: "setOverdub", on: false });
  });

  it("用語の説明と文は、モックの文のまま", () => {
    const mock = readProposal("P10-Overdub");
    const term = findTerm("重ねる")!;
    expect(mock).toContain(`${term.term}${TERM_SEPARATOR}${term.description}`);
    expect(mock).toContain(t("arrange.overdubClip", { clip: "イントロ" }));
    const sentence = [
      t("msg.overdubRecording", { track: "1 ピアノ", clip: "イントロ" }),
      t("msg.overdubLap", { from: 1, to: 4, lap: 2 }),
      t("msg.overdubKept", { clip: "イントロ" }),
    ].join(" ・ ");
    expect(mock).toContain(sentence);
  });

  it("窓がせまいとき、ほかの物を引っこめてから引っこむ（ループは引っこまない）", () => {
    const w = { base: 800, keys: 100, songName: 144, load: 60, countIn: 227, positionLabels: 20 };
    const stubs = { songName: 40, countIn: 20 };
    const all = w.base + w.keys + w.songName + w.load + w.countIn + w.positionLabels;
    expect(transportWithOverdub(all + 60, w, stubs, 60)).toEqual({ keys: true, songName: true, load: true, countIn: true, positionLabels: true, overdub: true });
    expect(transportWithOverdub(all + 59, w, stubs, 60).overdub).toBe(true);
    expect(transportWithOverdub(all + 59, w, stubs, 60).keys).toBe(false);
    const least = w.base + stubs.songName + stubs.countIn;
    expect(transportWithOverdub(least + 60, w, stubs, 60)).toEqual({ keys: false, songName: false, load: false, countIn: false, positionLabels: false, overdub: true });
    expect(transportWithOverdub(least + 59, w, stubs, 60).overdub).toBe(false);
  });

  const recording = (overdub: boolean, clipId: number | null, loop = false) =>
    ready((s) => {
      s.transport.state = "recording";
      s.transport.overdub = overdub;
      s.transport.recording = { id: 1, track: 1, from: 0, take: 2, clip: clipId, lap: loop ? 1 : 0, overdub };
      s.status.message = loop
        ? { id: "loopRecording", track: { number: 1, name: "ピアノ" }, fromBar: 1, toBar: 4, lap: 2 }
        : { id: "recording", track: { number: 1, name: "ピアノ" }, take: 2 };
    });

  it("重ねて録っている間の状態行（ループなら周の文も）。クリップがない所 ・ オフのときは、いままでの文", () => {
    const left = () => text(target.querySelector("footer.status"))!;
    show(StatusLine, recording(true, 1, true));
    expect(left()).toContain("1 ピアノ の イントロ に重ねて録音 ・ ループ 1–4 の 2 周目 ・ 止めても イントロ に足したまま（取り消しで戻せます）");
    show(StatusLine, recording(true, 1));
    expect(left()).toContain("1 ピアノ の イントロ に重ねて録音 ・ 止めても イントロ に足したまま（取り消しで戻せます）");
    show(StatusLine, recording(true, null));
    expect(left()).toContain("1 ピアノ へ録音 ・ 止めると テイク 2 として残ります");
    show(StatusLine, recording(false, 1));
    expect(left()).toContain("1 ピアノ へ録音 ・ 止めると テイク 2 として残ります");
  });

  it("アレンジの録っているクリップの上: 「● 重ねて録音中 イントロ」", () => {
    show(Arrange, recording(true, 1));
    expect(text(q(".clip-label.recording"))).toBe("● 重ねて録音中 イントロ");
    show(Arrange, recording(false, 1));
    expect(text(q(".clip-label.recording"))).toBe("● 録音中 テイク 2");
    expect(overdubClipName(song().song.tracks, { track: 3, clip: 61 })).toBeNull();
  });
});
