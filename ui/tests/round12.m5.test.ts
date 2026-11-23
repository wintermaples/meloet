// Round 12 M5: the master's lane (of its volume) in the arrange and the mixer, 「編集 ▸ 曲の終わりにフェードアウトをつける…」
// with its box of bars, the status lines, and the fake's copy of the fade-out rule.
import { flushSync, mount, tick, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { FADE_OUT_BARS_DEFAULT, FADE_OUT_BARS_MAX, type AppState, type Command, type LanePoints, type LaneRef, type LaneView } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState, type FakeState } from "../src/bridge/fake";
import { fakeFadeOut, fakeFaderPlace } from "../src/bridge/fakeFade";
import { decodeLane, songLaneIds } from "../src/bridge/lane";
import Arrange from "../src/components/Arrange.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import TrackHeads from "../src/components/TrackHeads.svelte";
import { closeFadeBox, fadeBox, songEndTick } from "../src/components/arrange/fadeBox.svelte";
import { arrangeRows } from "../src/components/automation/autoGeom";
import { livePosition, refreshPositions } from "../src/components/automation/autoState.svelte";
import { forgetHand, handAt, noteHand } from "../src/components/automation/handValue";
import {
  MASTER_HOME,
  handPosition,
  hasLaneRef,
  isOnMaster,
  isOnTrack,
  laneMoves,
  masterHome,
  masterOwner,
  trackHome,
} from "../src/components/automation/laneValue";
import { historyRowText, stepPlaceText } from "../src/components/history/stepText";
import { closeContextMenu, contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { topMenus } from "../src/components/menu/menus";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { dragReading } from "../src/components/pianoroll/view.svelte";
import type { KeyDeps } from "../src/keys/attach";
import { t } from "../src/text";
import { TempoMap } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

// The points of the lanes come from the fake, as the screen would fetch them (`load`).
const POINTS = new Map<number, LanePoints>();
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  peekLanePoints: (id: number) => POINTS.get(id),
  lanePoints: (id: number) => (POINTS.has(id) ? Promise.resolve(POINTS.get(id)!) : Promise.reject(new Error("no lane"))),
}));
async function load(s: AppState): Promise<void> {
  const transport = createFakeTransport(s as FakeState);
  for (const id of songLaneIds(s.song)) POINTS.set(id, decodeLane(await transport.lane(id)));
}

const BAR = 4 * TPQ;
const VOLUME = { kind: "volume" } as const;
const MASTER: LaneRef = { home: { kind: "master" }, target: VOLUME };
const scene = (name: string): AppState => fakeScene(name)!.state;
const laneOfMaster = (s: AppState): LaneView => s.song.master.lanes[0]!;

const WIDTH = 1000;
const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientWidth");
beforeAll(() => Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => WIDTH }));
afterAll(() => had && Object.defineProperty(proto, "clientWidth", had));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  closeContextMenu();
  closeFadeBox();
  POINTS.clear();
});
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element) => el.textContent!.replace(/\s+/g, " ").trim();
const deps = (s: AppState): KeyDeps => ({ state: () => s, send: app.send, nav: { platform: "Linux x86_64" }, playTick: () => 0 });

describe("the master as the home of a lane", () => {
  it("stands under one number that no track has, and its home in the intents is the master's", () => {
    expect(trackHome(7)).toEqual({ kind: "track", track: 7 });
    expect(trackHome(MASTER_HOME)).toEqual({ kind: "master" });
    expect(masterHome).toEqual({ kind: "master" });
    const onTrack: LaneRef = { home: { kind: "track", track: 7 }, target: VOLUME };
    expect([isOnMaster(MASTER), isOnMaster(onTrack)]).toEqual([true, false]);
    expect([isOnTrack(MASTER, MASTER_HOME), isOnTrack(MASTER, 7), isOnTrack(onTrack, MASTER_HOME), isOnTrack(onTrack, 7)]).toEqual([true, false, false, true]);
    expect(hasLaneRef([MASTER], MASTER_HOME, VOLUME)).toBe(true);
    expect(hasLaneRef([onTrack], MASTER_HOME, VOLUME)).toBe(false);
    expect(hasLaneRef([MASTER], MASTER_HOME, { kind: "pan" })).toBe(false);
  });
  it("the hand's value of its lane is the master's fader; a held lane does not move", () => {
    const s = scene("mixerMasterRest");
    const owner = masterOwner(s.song.master);
    expect(owner.id).toBe(MASTER_HOME);
    expect(handPosition(owner, VOLUME, null)).toBeCloseTo(fakeFaderPlace(-3));
    const moving = scene("mixerMasterAuto");
    const lane = laneOfMaster(moving);
    expect(laneMoves(lane, [], MASTER_HOME)).toBe(true);
    expect(laneMoves(lane, [MASTER], MASTER_HOME)).toBe(false);
    expect(laneMoves(laneOfMaster(s), [], MASTER_HOME)).toBe(false);
  });
  it("the points of the master's lane are kept with the song's", () => {
    const s = scene("masterFade");
    expect(songLaneIds(s.song)).toContain(laneOfMaster(s).laneId);
    expect(songLaneIds({ tracks: [] })).toEqual([]);
  });
  it("a drag of the master's fader is the hand on its lane; a return's is none", () => {
    const drag: Command = { type: "setGain", strip: { kind: "master" }, db: -6, commit: false };
    noteHand(drag);
    expect(handAt(MASTER_HOME, VOLUME)).toBeCloseTo(fakeFaderPlace(-6));
    forgetHand(drag);
    expect(handAt(MASTER_HOME, VOLUME)).toBeNull();
    noteHand({ type: "setGain", strip: { kind: "return", bus: "a" }, db: -6, commit: false });
    expect(handAt(MASTER_HOME, VOLUME)).toBeNull();
  });
});

describe("the row 「マスター」 at the foot of the arrange", () => {
  const dims = { trackH: 64, laneH: 48, rowH: 24 };
  it("is there only while the master has a lane, below every track and its lanes", () => {
    const plain = scene("autoRec");
    const none = arrangeRows(plain.song.tracks, plain.automation.folded, dims, null);
    expect(none.master).toBeNull();
    expect(none.height).toBe(none.tracksHeight);
    const s = scene("masterFade");
    const owner = masterOwner(s.song.master);
    const rows = arrangeRows(s.song.tracks, [], dims, owner);
    expect(rows.master).toEqual({ top: rows.tracksHeight, height: 48, folded: false });
    expect(rows.height).toBe(rows.tracksHeight + 48);
    expect(rows.lanes.at(-1)).toEqual({ track: owner, lane: laneOfMaster(s), top: rows.tracksHeight });
    expect(rows.tops).toHaveLength(s.song.tracks.length);
    // Folded: the row stays, as high as a 「＋ 帯を足す ▾」 row, with no lane in it.
    const folded = arrangeRows(s.song.tracks, [MASTER], dims, owner);
    expect(folded.master).toEqual({ top: rows.tracksHeight, height: 24, folded: true });
    expect(folded.lanes).toHaveLength(0);
  });
  it("its head says 「マスター 音量」 and works as a track's lane head, with the master's home in the intents", async () => {
    const s = scene("masterFade");
    await load(s);
    show(TrackHeads, s);
    const heads = qa("[data-test=lane-head]");
    expect(heads).toHaveLength(1);
    const head = heads[0]!;
    expect(text(head.querySelector("[data-test=lane-fold]")!)).toBe("▾ マスター音量");
    expect(text(head.querySelector("[data-test=lane-state]")!)).toBe("いま 0.0 dB ・ 点 2");
    head.querySelector<HTMLElement>("[data-test=lane-write]")!.click();
    head.querySelector<HTMLElement>("[data-test=lane-fold]")!.click();
    head.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    flushSync();
    const rows = contextMenu.open!.entries.filter((e): e is MenuRow => e.kind === "row");
    expect(rows.map((r) => r.label)).toEqual(["lane.delete"]);
    rows[0]!.run();
    expect(app.sent).toEqual([
      { type: "setLaneWrite", home: { kind: "master" }, target: VOLUME, on: true },
      { type: "foldLane", home: { kind: "master" }, target: VOLUME, folded: true },
      { type: "deleteLane", home: { kind: "master" }, target: VOLUME },
    ]);
    // No 「＋ 帯を足す ▾」 row for the master, and the tracks' heads are as they were.
    expect(qa("[data-test=lane-add-row]")).toHaveLength(0);
  });
  it("a lane at rest shows 「もどす」; a folded one a line that opens it again", () => {
    const s = scene("masterFade");
    laneOfMaster(s).resting = true;
    s.song.master.gainDb = -3;
    show(TrackHeads, s);
    expect(text(q("[data-test=lane-state]")!)).toBe("休み ・ −3.0 dB");
    q("[data-test=lane-restore]")!.click();
    expect(app.sent).toEqual([{ type: "restoreLane", home: { kind: "master" }, target: VOLUME }]);
    app.sent.length = 0;
    show(TrackHeads, scene("masterFadeFolded"));
    expect(qa("[data-test=lane-head]")).toHaveLength(0);
    expect(text(q("[data-test=master-row-folded]")!)).toBe("▸マスター音量");
    q("[data-test=master-lane-unfold]")!.click();
    expect(app.sent).toEqual([{ type: "foldLane", home: { kind: "master" }, target: VOLUME, folded: false }]);
  });
  it("no row without a lane; on the time side the lane is drawn as a track's, a folded one as an empty row", () => {
    show(TrackHeads, scene("muted"));
    expect(qa("[data-test=lane-head], [data-test=master-row-folded]")).toHaveLength(0);
    show(Arrange, scene("muted"));
    expect(qa("[data-test=auto-lane]")).toHaveLength(0);
    show(Arrange, scene("masterFade"));
    expect(qa("[data-test=auto-lane]")).toHaveLength(1);
    expect(qa("[data-test=master-lane-folded]")).toHaveLength(0);
    show(Arrange, scene("masterFadeFolded"));
    expect(qa("[data-test=auto-lane]")).toHaveLength(0);
    expect(qa("[data-test=master-lane-folded]")).toHaveLength(1);
  });
  it("while 「● 書く」 writes, the head says so and the lane has the rec frame", () => {
    const s = scene("masterFadeWriting");
    show(TrackHeads, s);
    expect(text(q("[data-test=lane-state]")!)).toBe(t("lane.writing"));
    expect(q("[data-test=lane-write]")!.getAttribute("aria-pressed")).toBe("true");
    show(Arrange, s);
    expect(qa("[data-test=auto-writing]")).toHaveLength(1);
  });
});

describe("the mixer's master strip", () => {
  const strips = () => qa("[data-test=mixer-strip]");
  it("has the mark that makes the lane; no other strip has one", () => {
    const s = scene("mixer");
    show(MixerStrips, s);
    const marks = qa("[data-test=mixer-master-automation]");
    expect(marks).toHaveLength(1);
    expect(strips().at(-1)!.contains(marks[0]!)).toBe(true);
    const mark = marks[0]!.querySelector<HTMLElement>("[data-test=param-automation]")!;
    expect([mark.getAttribute("aria-pressed"), mark.getAttribute("aria-disabled"), mark.getAttribute("aria-label")]).toEqual(["false", null, "オートメーションに書く"]);
    mark.click();
    expect(app.sent).toEqual([{ type: "addLane", home: { kind: "master" }, target: VOLUME }]);
    // Without any lane the row under the values is not there.
    expect(qa("[data-test=mixer-auto]")).toHaveLength(0);
  });
  it("is disabled while recording", () => {
    const s = scene("mixer");
    s.transport.state = "recording";
    show(MixerStrips, s);
    const mark = q("[data-test=mixer-master-automation] [data-test=param-automation]")!;
    expect(mark.getAttribute("aria-disabled")).toBe("true");
    mark.click();
    expect(app.sent).toEqual([]);
  });
  it("the fader follows the line at the play line and looks moved by automation", async () => {
    const s = scene("mixerMasterAuto");
    await load(s);
    expect(Array.from(POINTS.get(laneOfMaster(s).laneId)!.ticks)).toEqual([0, 8 * BAR, 12 * BAR]);
    show(MixerStrips, s);
    const master = strips().at(-1)!;
    expect(master.querySelector("[data-test=mixer-fader]")!.classList.contains("auto")).toBe(true);
    expect(text(master.querySelector("[data-test=mixer-auto]")!)).toBe("オートメーション");
    expect(master.querySelector("[data-test=param-automation]")!.getAttribute("aria-pressed")).toBe("true");
    // Every strip has the row, to keep the heights the same.
    expect(qa("[data-test=mixer-auto]")).toHaveLength(strips().length);
    // The position on the line is kept under the master's number: half way down from 0.8 to 0.4 at 11.1.000.
    refreshPositions(s, s.transport.position);
    expect(livePosition(MASTER_HOME, VOLUME)).toBeCloseTo(0.6, 3);
    await tick();
    flushSync();
    expect(text(master.querySelector("[data-test=mixer-gain]")!)).toBe("−6.0");
    // A lane the hand holds does not move the fader.
    refreshPositions({ ...s, automation: { ...s.automation, held: [MASTER] } }, s.transport.position);
    expect(livePosition(MASTER_HOME, VOLUME)).toBeNull();
  });
  it("at rest: the hand's value, 「休み」 and 「もどす」", () => {
    const s = scene("mixerMasterRest");
    show(MixerStrips, s);
    const master = strips().at(-1)!;
    expect(master.querySelector("[data-test=mixer-fader]")!.classList.contains("auto")).toBe(false);
    expect(text(master.querySelector("[data-test=mixer-gain]")!)).toBe("−3.0");
    expect(text(master.querySelector("[data-test=mixer-auto]")!)).toBe("休み もどす");
    master.querySelector<HTMLElement>("[data-test=mixer-auto-restore]")!.click();
    expect(app.sent).toEqual([{ type: "restoreLane", home: { kind: "master" }, target: VOLUME }]);
  });
});

describe("「編集 ▸ 曲の終わりにフェードアウトをつける…」 and its box", () => {
  const row = (s: AppState) => topMenus(s, deps(s), "other")[1]!.entries.find((e): e is MenuRow => e.kind === "row" && e.label === "曲の終わりにフェードアウトをつける…")!;
  const field = () => target.querySelector<HTMLInputElement>('[data-test="fade-out-field"]');
  function type(value: string | null, key?: string, more: KeyboardEventInit = {}): void {
    const f = field()!;
    if (value !== null) {
      f.value = value;
      f.dispatchEvent(new Event("input", { bubbles: true }));
    }
    if (key) f.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...more }));
    flushSync();
  }
  function open(s: AppState): void {
    show(Arrange, s);
    row(s).run!();
    flushSync();
  }

  it("is the last row of 「編集」, after a separator; not while recording, counting in, or in the overview", () => {
    const s = scene("muted");
    const entries = topMenus(s, deps(s), "other")[1]!.entries;
    expect(entries.at(-2)).toEqual({ kind: "separator" });
    expect(entries.at(-1)).toMatchObject({ kind: "row", label: t("menu.fadeOut"), usable: true });
    expect(row(s).key).toBeUndefined();
    for (const state of ["recording", "countIn"] as const) {
      const busy = scene("muted");
      busy.transport.state = state;
      expect(row(busy).usable).toBe(false);
    }
    const overview = scene("muted");
    overview.workspace.layout.arrangeOverview = true;
    expect(row(overview).usable).toBe(false);
  });
  it("opens with 4, chosen, on the ruler at the song's end, and the status line says what Enter does", async () => {
    const s = scene("muted");
    open(s);
    await tick();
    expect(fadeBox.open).toBe(true);
    expect(field()!.value).toBe(String(FADE_OUT_BARS_DEFAULT));
    expect(document.activeElement).toBe(field());
    expect([field()!.selectionStart, field()!.selectionEnd]).toEqual([0, 1]);
    expect(field()!.getAttribute("aria-label")).toBe("フェードアウトの小節の数");
    expect(field()!.closest(".ruler")).not.toBeNull();
    expect(dragReading.text).toBe("曲の終わりの 4 小節をフェードアウトにします");
    expect(app.sent).toEqual([]);
  });
  it("Enter sends one addFadeOut and closes; Esc and a press outside close with nothing sent", () => {
    const s = scene("muted");
    open(s);
    type("8", "Enter");
    expect(app.sent).toEqual([{ type: "addFadeOut", bars: 8 }]);
    expect(field()).toBeNull();
    expect(dragReading.text).toBeNull();
    app.sent.length = 0;
    open(s);
    type("6", "Escape");
    expect(field()).toBeNull();
    open(s);
    field()!.dispatchEvent(new FocusEvent("blur"));
    flushSync();
    expect(field()).toBeNull();
    expect(app.sent).toEqual([]);
  });
  it("a number outside 1 to 16 goes to the nearer end and is told; what is no whole number stays open", () => {
    const s = scene("muted");
    open(s);
    type("99", "Enter");
    expect(app.sent).toEqual([
      { type: "addFadeOut", bars: FADE_OUT_BARS_MAX },
      { type: "clampValue", field: "bars", value: "16", range: "1〜16" },
    ]);
    app.sent.length = 0;
    open(s);
    type("0", "Enter");
    expect(app.sent).toEqual([
      { type: "addFadeOut", bars: 1 },
      { type: "clampValue", field: "bars", value: "1", range: "1〜16" },
    ]);
    app.sent.length = 0;
    open(s);
    for (const broken of ["", "2.5", "四"]) {
      type(broken, "Enter");
      expect(field()!.classList.contains("broken")).toBe(true);
      expect(dragReading.text).toBeNull();
    }
    expect(app.sent).toEqual([]);
  });
  it("↑ ↓ change it by 1, by 10 with Shift, inside 1 to 16", () => {
    open(scene("muted"));
    type(null, "ArrowUp");
    expect(field()!.value).toBe("5");
    type(null, "ArrowUp", { shiftKey: true });
    expect(field()!.value).toBe("15");
    type(null, "ArrowUp", { shiftKey: true });
    expect(field()!.value).toBe("16");
    type(null, "ArrowDown", { shiftKey: true });
    type(null, "ArrowDown", { shiftKey: true });
    expect(field()!.value).toBe("1");
    expect(dragReading.text).toBe("曲の終わりの 1 小節をフェードアウトにします");
  });
  it("recording starting closes the box", () => {
    const s = scene("muted");
    open(s);
    const busy = scene("muted");
    busy.transport.state = "countIn";
    app.setState(busy);
    flushSync();
    expect(field()).toBeNull();
    expect(fadeBox.open).toBe(false);
  });
  it("the song's end on the screen is the latest clip end, up to the head of a bar", () => {
    const s = fakeState();
    const map = new TempoMap(s.timeline);
    const tracks = (end: number) => [{ clips: [{ end: BAR }] }, { clips: [{ end }] }];
    expect(songEndTick([], map)).toBe(0);
    expect(songEndTick(tracks(4 * BAR), map)).toBe(4 * BAR);
    expect(songEndTick(tracks(4 * BAR + 1), map)).toBe(5 * BAR);
  });
});

describe("the fake's fade-out (Rust's rule)", () => {
  const song = (ends: number[], gainDb: number | null = 0) =>
    ({ tracks: ends.map((end) => ({ clips: [{ end }] })), master: { gainDb } }) as unknown as AppState["song"];
  const timeline = fakeState().timeline;

  it("ends at the song's end and starts bars before it, from the fader", () => {
    expect(fakeFadeOut(song([BAR, 6 * BAR + TPQ]), timeline, null, 4)).toEqual({ start: 3 * BAR, end: 7 * BAR, points: [[3 * BAR, 0.8], [7 * BAR, 0]] });
    expect(fakeFadeOut(song([2 * BAR], -6), timeline, null, 16)).toEqual({ start: 0, end: 2 * BAR, points: [[0, fakeFaderPlace(-6)], [2 * BAR, 0]] });
    expect(fakeFaderPlace(-6)).toBeCloseTo(0.6);
    expect([fakeFaderPlace(null), fakeFaderPlace(0), fakeFaderPlace(6)]).toEqual([0, 0.8, 1]);
  });
  it("no clip: nothing to fade; bars outside 1 to 16: nothing at all", () => {
    expect(fakeFadeOut(song([]), timeline, null, 4)).toBe("noClips");
    for (const bars of [0, 17, 2.5, -1]) expect(fakeFadeOut(song([4 * BAR]), timeline, null, bars)).toBeNull();
  });
  it("a lane that moves: the fade starts on its line and the points from the start on go", () => {
    const points: [number, number][] = [[0, 0.4], [2 * BAR, 0.4], [6 * BAR, 0.8], [7 * BAR, 0.2]];
    expect(fakeFadeOut(song([8 * BAR]), timeline, { points, resting: false }, 4)).toEqual({
      start: 4 * BAR,
      end: 8 * BAR,
      points: [[0, 0.4], [2 * BAR, 0.4], [4 * BAR, 0.6000000000000001], [8 * BAR, 0]],
    });
    // A lane at rest, or with no points: from the fader; the points before the start stay.
    expect(fakeFadeOut(song([8 * BAR]), timeline, { points, resting: true }, 4)!).toMatchObject({ points: [[0, 0.4], [2 * BAR, 0.4], [4 * BAR, 0.8], [8 * BAR, 0]] });
    expect(fakeFadeOut(song([8 * BAR], -6), timeline, { points: [], resting: false }, 2)!).toMatchObject({ points: [[6 * BAR, fakeFaderPlace(-6)], [8 * BAR, 0]] });
  });
  it("through the fake: a lane is made, replaced under a new id, woken; the same fade again is nothing", async () => {
    const s = scene("muted");
    const transport = createFakeTransport(s as FakeState);
    const now = async () => (await transport.snapshot()).state;
    const points = async () => {
      const lane = decodeLane(await transport.lane(laneOfMaster(await now()).laneId));
      return Array.from(lane.ticks).map((tick, i) => [tick, lane.values[i]]);
    };
    const end = songEndTick(s.song.tracks, new TempoMap(s.timeline));
    const undo = s.song.undoCount;
    expect(s.song.master.lanes).toEqual([]);
    await transport.send({ type: "addFadeOut", bars: 4 });
    const first = laneOfMaster(await now());
    expect(first).toMatchObject({ target: VOLUME, name: { kind: "volume" }, points: 2, step: false, resting: false });
    expect(await points()).toEqual([[end - 4 * BAR, expect.closeTo(0.8)], [end, 0]]);
    expect((await now()).song.undoCount).toBe(undo + 1);
    await transport.send({ type: "addFadeOut", bars: 4 });
    expect(laneOfMaster(await now()).laneId).toBe(first.laneId);
    expect((await now()).song.undoCount).toBe(undo + 1);
    for (const bars of [0, 17]) await transport.send({ type: "addFadeOut", bars });
    expect((await now()).song.undoCount).toBe(undo + 1);
    await transport.send({ type: "addFadeOut", bars: 8 });
    expect(laneOfMaster(await now()).laneId).not.toBe(first.laneId);
    expect(await points()).toEqual([[end - 8 * BAR, expect.closeTo(0.8)], [end, 0]]);
    // The lane's own intents take the master's home.
    await transport.send({ type: "foldLane", home: { kind: "master" }, target: VOLUME, folded: true });
    expect((await now()).automation.folded).toEqual([MASTER]);
    await transport.send({ type: "addLane", home: { kind: "master" }, target: VOLUME });
    expect((await now()).automation.folded).toEqual([]);
    await transport.send({ type: "setLaneWrite", home: { kind: "master" }, target: VOLUME, on: true });
    expect((await now()).automation.armed).toEqual([MASTER]);
    await transport.send({ type: "addLane", home: { kind: "master" }, target: { kind: "pan" } });
    expect((await now()).song.master.lanes).toHaveLength(1);
    await transport.send({ type: "deleteLane", home: { kind: "master" }, target: VOLUME });
    expect((await now()).song.master.lanes).toEqual([]);
    expect((await now()).automation.armed).toEqual([]);
    await transport.send({ type: "addLane", home: { kind: "master" }, target: VOLUME });
    expect(laneOfMaster(await now())).toMatchObject({ points: 0, resting: false });
  });
  it("through the fake: a lane at rest is woken and said to be back by 「もどす」; no clip is said", async () => {
    const s = scene("mixerMasterRest");
    const transport = createFakeTransport(s as FakeState);
    await transport.send({ type: "restoreLane", home: { kind: "master" }, target: VOLUME });
    const back = (await transport.snapshot()).state;
    expect(laneOfMaster(back).resting).toBe(false);
    expect(back.status.message).toEqual({ id: "masterAutomationRestored" });
    const rested = createFakeTransport(scene("mixerMasterRest") as FakeState);
    await rested.send({ type: "addFadeOut", bars: 2 });
    const woke = (await rested.snapshot()).state;
    expect(laneOfMaster(woke)).toMatchObject({ resting: false });
    const empty = createFakeTransport(fakeState());
    await empty.send({ type: "addFadeOut", bars: 4 });
    const none = (await empty.snapshot()).state;
    expect(none.status.message).toEqual({ id: "fadeOutNoClips" });
    expect(none.song.master.lanes).toEqual([]);
  });
});

describe("what the status line and the history band say", () => {
  function line(message: AppState["status"]["message"]): string {
    const s = scene("muted");
    s.status.message = message;
    show(StatusLine, s);
    return text(q("[data-test=status-message]") ?? target);
  }
  it("the master's lane in the sentences of a track's", () => {
    expect(line({ id: "masterAutomationWriting" })).toContain("マスター ・ 音量を書いています（● 書く ・ 止めるとオフに戻ります）");
    expect(line({ id: "masterAutomationRested" })).toContain("マスター の音量を手で動かしたので、オートメーションを休ませました。「もどす」で線のとおりに戻ります");
    expect(line({ id: "masterAutomationRestored" })).toContain("マスター の音量を、オートメーションのとおりに戻しました。");
    expect(line({ id: "fadeOutNoClips" })).toContain("クリップがないので、フェードアウトをつけられませんでした。");
  });
  it("the faders that move: the master after the tracks, and alone", () => {
    const piano = { number: 1, name: "ピアノ" };
    const pad = { number: 4, name: "パッド" };
    expect(line({ id: "automationMoving", tracks: [piano, pad], master: false })).toContain("1 ピアノ と 4 パッド の音量は、オートメーションで動いています");
    expect(line({ id: "automationMoving", tracks: [piano], master: true })).toContain("1 ピアノ と マスター の音量は、オートメーションで動いています");
    expect(line({ id: "automationMoving", tracks: [], master: true })).toContain("マスター の音量は、オートメーションで動いています");
  });
  it("a fade-out's row: the name, the bars, and 「マスター」 as its place", () => {
    const row = { index: 3, step: 9, name: { kind: "addFadeOut" as const }, what: { kind: "bars" as const, span: { from: 13, to: 17 } }, place: { kind: "master" as const } };
    const s = scene("muted");
    expect(historyRowText(row, s)).toBe("「フェードアウトをつける」・ 13〜17 小節");
    expect(stepPlaceText(row.place, s)).toBe("マスター");
  });
});
