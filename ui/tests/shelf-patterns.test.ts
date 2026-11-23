// P4 (round 12): the shelf's 「パターン」: the rows, ▶, 「ここに置く」, the keys, and the fake's answers.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { FAKE_PATTERNS } from "../src/bridge/fakeP4";
import RightRail from "../src/components/RightRail.svelte";
import { patternModel, patternName, placeText } from "../src/components/shelfPatterns";
import {
  shelfConfirmCommand,
  shelfFavoriteCommand,
  shelfSampleCommand,
  shelfStepCommand,
  shelfUi,
} from "../src/components/tracks.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { t } from "../src/text";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const BAR = 4 * 1920;

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
  target.remove();
  Object.assign(shelfUi, { query: "", favoritesOnly: false, open: {}, searchOpen: {}, searchFor: "", keys: false, view: "instruments" });
});

function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

describe("the patterns (tools/patterns/drums.json)", () => {
  it("the fake has Rust's rows, in their order, and every one has a name", () => {
    // tools/ is outside ui/; the tests run in ui/.
    const node = (globalThis as unknown as { process: { cwd(): string; getBuiltinModule(id: "node:fs"): { readFileSync(p: string, e: "utf8"): string } } })
      .process;
    const read = node.getBuiltinModule("node:fs").readFileSync(`${node.cwd()}/../tools/patterns/drums.json`, "utf8");
    const file = JSON.parse(read) as {
      patterns: { id: string; group: string; bars: number }[];
    };
    expect(FAKE_PATTERNS).toEqual(file.patterns.map(({ id, group, bars }) => ({ id, group, bars })));
    expect(FAKE_PATTERNS.map((p) => patternName(p.id))).toEqual([
      "8 ビート",
      "8 ビート（ハイハット開き）",
      "16 ビート",
      "4 つ打ち",
      "シャッフル",
      "ハーフタイム",
      "ライドで 8 ビート",
      "バラード",
      "スネアの連打",
      "タムまわし",
      "クラッシュで止める",
      "小さなフィル",
    ]);
  });

  it("two headings that fold; ↑ ↓ moves through the rows shown", () => {
    const all = patternModel(FAKE_PATTERNS, {});
    expect(all.entries.filter((e) => e.kind === "heading").map((e) => e.label)).toEqual(["基本", "フィルイン"]);
    expect(all.rows.length).toBe(12);
    const folded = patternModel(FAKE_PATTERNS, { "pt:basic": false });
    expect(folded.rows.map((p) => p.id)).toEqual(["snareRoll", "tomFill", "crashStop", "smallFill"]);
  });

  it("where 「ここに置く」 puts it: the selected drum track, or a studio kit track made below the selected one", () => {
    const s = scene("shelfPatterns");
    expect(placeText(s)).toBe("3 ドラム ・ 17 小節めに置きます");
    const n = scene("shelfPatternsNewTrack");
    expect(placeText(n)).toBe("トラック 2（スタジオキット）を作って、9 小節めに置きます");
  });
});

describe("the keys in 「パターン」", () => {
  it("↓ chooses the next row, Enter places it, Space plays or stops it, F does nothing", () => {
    const s = scene("shelfPatterns");
    shelfUi.view = "patterns";
    expect(shelfStepCommand(s, 1)).toEqual({ type: "shelfSelectPattern", id: "fourOnTheFloor" });
    expect(shelfStepCommand(s, -1)).toEqual({ type: "shelfSelectPattern", id: "eightBeatOpen" });
    expect(shelfConfirmCommand(s)).toEqual({ type: "placePattern", id: "sixteenBeat" });
    expect(shelfSampleCommand(s)).toEqual({ type: "stopSample" });
    expect(shelfFavoriteCommand(s)).toBeNull();
    const n = scene("shelfPatternsNewTrack");
    expect(shelfSampleCommand(n)).toEqual({ type: "playPattern", id: "tomFill" });
    const none = { ...n, shelf: { ...n.shelf, selectedPattern: null } };
    expect(shelfConfirmCommand(none)).toBeNull();
    expect(shelfStepCommand(none, 1)).toEqual({ type: "shelfSelectPattern", id: "eightBeat" });

    const sent: Command[] = [];
    const d: KeyDeps = { state: () => s, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
    shelfUi.keys = true;
    handleKeydown(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }), d);
    expect(sent).toEqual([{ type: "placePattern", id: "sixteenBeat" }]);
  });
});

describe("the shelf with 「パターン」 (spec P4 mock)", () => {
  it("the switch is there only for a new track; the rows, ■ on the one playing, and 「ここに置く Enter」", () => {
    show(RightRail, scene("shelfPatterns"));
    expect(text(q(".head"))).toBe("▾ 棚 音源 31 エフェクト 9 パターン P");
    q("[data-test=shelf-patterns]")!.click();
    flushSync();
    expect(q("[data-test=shelf-mode]")).toBeNull();
    expect(qa("[data-group]").map(text)).toEqual(["▾ 基本", "▾ フィルイン"]);
    const rows = qa("[data-test=shelf-pattern]");
    expect(rows.length).toBe(12);
    expect(text(rows[2]!)).toBe("16 ビート 1 小節");
    expect(rows[2]!.classList.contains("selected")).toBe(true);
    expect(rows[2]!.querySelector(".sample")!.classList.contains("playing")).toBe(true);
    expect(text(q("[data-test=shelf-place]"))).toBe("3 ドラム ・ 17 小節めに置きます ここに置く Enter");

    rows[9]!.click();
    rows[9]!.querySelector<HTMLElement>(".sample")!.click();
    rows[2]!.querySelector<HTMLElement>(".sample")!.click();
    q("[data-test=shelf-place-pattern]")!.click();
    expect(app.sent).toEqual([
      { type: "shelfSelectPattern", id: "tomFill" },
      { type: "playPattern", id: "tomFill" },
      { type: "stopSample" },
      { type: "placePattern", id: "sixteenBeat" },
    ]);

    q("[data-group='pt:basic']")!.click();
    flushSync();
    expect(qa("[data-test=shelf-pattern]").length).toBe(4);
  });

  it("not for replacing a track's sound; 「ここに置く」 is off with no row", () => {
    const s = scene("shelfPatternsNewTrack");
    show(RightRail, { ...s, shelf: { ...s.shelf, mode: { kind: "replace", track: s.song.tracks[0]!.id } } });
    expect(q("[data-test=shelf-patterns]")).toBeNull();
    unmount(mounted!);
    mounted = null;
    target.replaceChildren();
    show(RightRail, { ...s, shelf: { ...s.shelf, selectedPattern: null } });
    q("[data-test=shelf-patterns]")!.click();
    flushSync();
    expect((q("[data-test=shelf-place-pattern]") as HTMLButtonElement).disabled).toBe(true);
    expect(t("msg.patternNeedsFourFour")).toBe("4/4 のところにだけ置けます");
  });
});

describe("the fake", () => {
  async function opened(state = fakeState()) {
    const tr = createFakeTransport(state);
    await tr.subscribe(
      () => {},
      () => {},
    );
    const now = async () => (await tr.snapshot()).state as AppState;
    return { now, send: (c: Command) => tr.send(c) };
  }

  it("places a pattern on a new studio kit track, then on that track after it, as one step each", async () => {
    const { now, send } = await opened();
    await send({ type: "openShelf", mode: { kind: "newTrack" } });
    await send({ type: "shelfSelectPattern", id: "eightBeat" });
    expect((await now()).shelf.selectedPattern).toBe("eightBeat");
    const before = await now();
    await send({ type: "placePattern", id: "eightBeat" });
    let s = await now();
    expect(s.song.tracks.length).toBe(before.song.tracks.length + 1);
    const kit = s.song.tracks.find((x) => x.id === s.song.playTarget)!;
    expect(kit.sound).toEqual({ kind: "builtin", instrument: "studioKit" });
    expect(kit.clips.map((c) => [c.start, c.end])).toEqual([[0, BAR]]);
    expect(s.transport.position).toBe(BAR);
    expect(s.song.undoCount).toBe(before.song.undoCount + 1);
    await send({ type: "placePattern", id: "tomFill" });
    s = await now();
    expect(s.song.tracks.length).toBe(before.song.tracks.length + 1);
    expect(s.song.tracks.find((x) => x.id === kit.id)!.clips.map((c) => [c.start, c.end])).toEqual([
      [0, BAR],
      [BAR, 2 * BAR],
    ]);
    // Where it would overlap, nothing.
    await send({ type: "locate", tick: BAR / 2 });
    await send({ type: "placePattern", id: "tomFill" });
    expect((await now()).status.message).toEqual({ id: "notesNotWritten" });
    expect((await now()).song.undoCount).toBe(s.song.undoCount);
  });

  it("▶ shows the pattern playing until StopSample; nothing with the shelf closed", async () => {
    const state = fakeState();
    state.transport.canPlay = true;
    const { now, send } = await opened(state);
    await send({ type: "playPattern", id: "shuffle" });
    expect((await now()).shelf.patternSample).toBeNull();
    await send({ type: "openShelf", mode: { kind: "newTrack" } });
    await send({ type: "playPattern", id: "shuffle" });
    expect((await now()).shelf.patternSample?.id).toBe("shuffle");
    await send({ type: "stopSample" });
    expect((await now()).shelf.patternSample).toBeNull();
  });
});
