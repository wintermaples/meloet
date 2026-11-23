// Round 12 A2, whole sections: the three rows of a marker's right-click menu (and why they are disabled), the box of
// 「この前に空きを入れる…」, the row of the history band, and the fake's copy of the splice.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, HistoryRow, MarkerView } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import { fakeSection, reduceSections } from "../src/bridge/fakeSections";
import Arrange from "../src/components/Arrange.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { historyRowText } from "../src/components/history/stepText";
import { contextMenu, listEntries, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { dragReading } from "../src/components/pianoroll/view.svelte";
import { closeSpaceBox, spaceBox } from "../src/components/timeline/spaceBox.svelte";
import { readBars } from "../src/components/valueClamp";
import { t } from "../src/text";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const BAR = 4 * TPQ;
const scene = (name = "sections"): AppState => fakeScene(name)!.state;
const fake = (s: AppState) => s as unknown as FakeState;
const FAKE_TAKES = new Map();

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
  contextMenu.open = null;
  closeSpaceBox();
});

function show(state: AppState, component: Component<any> = Arrange): void {
  if (mounted) unmount(mounted);
  closeSpaceBox();
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const tag = (name: string) => qa('[data-test="marker-tag"]').find((el) => el.textContent?.trim() === name)!;
function menuOf(name: string): MenuRow[] {
  tag(name).dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  flushSync();
  return contextMenu.open!.entries.filter((e): e is MenuRow => e.kind === "row");
}
const sectionRows = (rows: MenuRow[]) => rows.filter((r) => ["menu.duplicateSection", "menu.deleteSection", "menu.insertSpace"].includes(r.label));
const field = () => target.querySelector<HTMLInputElement>('[data-test="insert-space-field"]');
function type(text: string | null, key?: string, more: KeyboardEventInit = {}): void {
  const f = field()!;
  if (text !== null) {
    f.value = text;
    f.dispatchEvent(new Event("input", { bubbles: true }));
  }
  if (key) f.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...more }));
  flushSync();
}
const sentAfter = (from: number) => app.sent.slice(from).filter((c: Command) => c.type !== "selectTimeline");

describe("the scene's sections (the fake's rule is Rust's)", () => {
  it("a marker to the next later one; the last one to the song's end", () => {
    const s = scene();
    expect(s.timeline.markers.map((m: MarkerView) => [m.name, m.section])).toEqual([
      ["イントロ", "usable"],
      ["Aメロ", "notOnBars"],
      ["ブリッジ", "notOnBars"],
      ["サビ", "usable"],
      ["おわり", "empty"],
    ]);
    expect(fakeSection(s.song, s.timeline, 0)).toEqual({ use: "usable", to: 4 * BAR, bars: 4 });
    expect(fakeSection(s.song, s.timeline, 9 * BAR)).toEqual({ use: "usable", to: 16 * BAR, bars: 7 });
  });
});

describe("the marker's right-click menu", () => {
  it("three rows after 「ループに」, then a separator; each sends one command", () => {
    show(scene());
    const rows = menuOf("イントロ");
    expect(contextMenu.open!.entries.map((e) => (e.kind === "row" ? e.label : "—"))).toEqual([
      "menu.loopFromMarker",
      "menu.duplicateSection",
      "menu.deleteSection",
      "menu.insertSpace",
      "—",
      "menu.renameMarker",
      "—",
      "menu.delete",
    ]);
    expect(sectionRows(rows).map((r) => [t(r.label), r.usable, r.hint])).toEqual([
      ["この区間をすぐ後ろに複製", true, undefined],
      ["この区間を消して詰める", true, undefined],
      ["この前に空きを入れる…", true, undefined],
    ]);
    const from = app.sent.length;
    rows[1]!.run();
    rows[2]!.run();
    expect(sentAfter(from)).toEqual([
      { type: "duplicateSection", marker: 1 },
      { type: "deleteSection", marker: 1 },
    ]);
  });

  it("a section that is not on bar heads, or has nothing in it: disabled, with the reason in the row", () => {
    // 「おわり」 (bar 17) is past the right end of the test's arrange: 「サビ」 stands in for a marker with nothing after it.
    const s = scene();
    s.timeline.markers = s.timeline.markers.map((m: MarkerView) => (m.name === "サビ" ? { ...m, section: "empty" } : m));
    show(s);
    for (const [name, why] of [
      ["Aメロ", "区間の両端が小節の頭のときに使えます"],
      ["ブリッジ", "区間の両端が小節の頭のときに使えます"],
      ["サビ", "このマーカーより後ろにクリップがないので、区間がありません"],
    ] as const) {
      const rows = menuOf(name);
      expect(sectionRows(rows).map((r) => [r.usable, r.hint])).toEqual([
        [false, why],
        [false, why],
        [false, why],
      ]);
      // The other rows stay usable, and the list rows carry the reason.
      expect(rows.filter((r) => !sectionRows(rows).includes(r)).every((r) => r.usable && r.hint === undefined)).toBe(true);
      const list = listEntries(contextMenu.open!.entries);
      expect(list.filter((e) => e.kind === "row" && e.hint === why)).toHaveLength(3);
    }
  });

  it("while recording: disabled without a reason of the section", () => {
    const s = scene();
    s.transport.state = "recording";
    show(s);
    expect(sectionRows(menuOf("イントロ")).map((r) => [r.usable, r.hint])).toEqual([
      [false, undefined],
      [false, undefined],
      [false, undefined],
    ]);
  });
});

describe("the box of 「この前に空きを入れる…」", () => {
  function open(name = "サビ"): number {
    show(scene());
    const rows = menuOf(name);
    const from = app.sent.length;
    sectionRows(rows)[2]!.run();
    flushSync();
    return from;
  }

  it("opens at the marker with 4 chosen; the status line says what Enter does; Enter sends one command", async () => {
    const from = open();
    expect(spaceBox.marker).toBe(3);
    await Promise.resolve();
    flushSync();
    const f = field()!;
    expect(f.value).toBe("4");
    expect(f.getAttribute("aria-label")).toBe("入れる小節の数");
    expect(f.parentElement!.classList.contains("space-box")).toBe(true);
    expect(dragReading.text).toBe("4 小節の空きを入れます");
    type("8");
    expect(dragReading.text).toBe("8 小節の空きを入れます");
    expect(sentAfter(from)).toEqual([]);
    type(null, "Enter");
    expect(sentAfter(from)).toEqual([{ type: "insertSpace", marker: 3, bars: 8 }]);
    expect(field()).toBeNull();
    expect(dragReading.text).toBeNull();
  });

  it("↑ ↓ step by 1 (10 with Shift) inside 1–64; full-width digits are read", () => {
    open();
    type(null, "ArrowUp");
    expect(field()!.value).toBe("5");
    type(null, "ArrowUp", { shiftKey: true });
    expect(field()!.value).toBe("15");
    type("60", "ArrowUp", { shiftKey: true });
    expect(field()!.value).toBe("64");
    type("3", "ArrowDown", { shiftKey: true });
    expect(field()!.value).toBe("1");
    type("１２");
    expect(dragReading.text).toBe("12 小節の空きを入れます");
  });

  it("out of 1–64: the nearer end is sent, and the status line is told; not a whole number: the box stays", () => {
    const from = open();
    type("200", "Enter");
    expect(sentAfter(from)).toEqual([
      { type: "insertSpace", marker: 3, bars: 64 },
      { type: "clampValue", field: "bars", value: "64", range: "1〜64" },
    ]);
    const again = open();
    type("0", "Enter");
    expect(sentAfter(again)[0]).toEqual({ type: "insertSpace", marker: 3, bars: 1 });
    const broken = open();
    for (const text of ["", "abc", "2.5", "4 小節"]) {
      type(text, "Enter");
      expect(field()!.classList.contains("broken")).toBe(true);
      expect(field()!.getAttribute("aria-invalid")).toBe("true");
      expect(dragReading.text).toBeNull();
    }
    expect(sentAfter(broken)).toEqual([]);
    expect(readBars("7", 64)).toEqual({ value: 7, clamp: null });
    expect(readBars("-3", 64)).toEqual({ value: 1, clamp: { field: "bars", value: "1", range: "1〜64" } });
    expect(readBars("1.5", 64)).toBeNull();
  });

  it("Esc and a press outside close it without sending; so do recording and the marker going away", () => {
    let from = open();
    type(null, "Escape");
    expect([field(), sentAfter(from)]).toEqual([null, []]);
    from = open();
    field()!.dispatchEvent(new FocusEvent("blur"));
    flushSync();
    expect([field(), sentAfter(from)]).toEqual([null, []]);
    open();
    const s = scene();
    s.transport.state = "countIn";
    app.setState(s);
    flushSync();
    expect(spaceBox.marker).toBeNull();
    open();
    const gone = scene();
    gone.timeline.markers = gone.timeline.markers.filter((m: MarkerView) => m.id !== 3);
    app.setState(gone);
    flushSync();
    expect(spaceBox.marker).toBeNull();
  });
});

describe("the status line", () => {
  it("a song that would get too long: 「曲が長くなりすぎるので、できませんでした。」", () => {
    show(scene("songTooLong"), StatusLine);
    expect(target.querySelector(".message")?.textContent?.trim()).toBe("曲が長くなりすぎるので、できませんでした。");
  });
});

describe("the row of the history band", () => {
  it("「区間を複製 ・ A メロ ・ 8 小節」", () => {
    const s = scene();
    const ctx = { song: s.song, library: s.library };
    const row = (kind: "duplicateSection" | "deleteSection" | "insertSpace", bars: number): HistoryRow => ({
      index: 1,
      step: 1,
      name: { kind },
      what: { kind: "section", name: "A メロ", bars },
      place: { kind: "arrange" },
    });
    expect(historyRowText(row("duplicateSection", 8), ctx)).toContain("区間を複製");
    expect(historyRowText(row("duplicateSection", 8), ctx)).toContain("A メロ ・ 8 小節");
    expect(historyRowText(row("deleteSection", 8), ctx)).toContain("区間を消して詰める");
    expect(historyRowText(row("insertSpace", 4), ctx)).toContain("空きを入れる");
    expect(historyRowText(row("insertSpace", 4), ctx)).toContain("A メロ ・ 4 小節");
  });
});

describe("the fake's copy of the splice", () => {
  const clips = (s: FakeState, track: number) => s.song.tracks[track]!.clips.map((c) => [c.name, c.start / BAR, c.end / BAR]);
  const marks = (s: FakeState) => s.timeline.markers.map((m) => [m.name, m.tick / BAR, m.section]);
  function run(s: FakeState, command: Command): { state: FakeState; row: HistoryRow | null; changed: number } {
    const out = reduceSections(s, command, FAKE_TAKES)!;
    const state = { ...s } as Record<string, unknown>;
    for (const c of out.changes) state[c.section] = c.value;
    return { state: state as unknown as FakeState, row: out.row, changed: out.changes.length };
  }

  it("a copy of 「イントロ」 right after it: new clips and a marker of the same name, the rest four bars later", () => {
    const s = fake(scene());
    s.arrange.selectedClips = [s.song.tracks[0]!.clips[0]!.id];
    s.song.loopRange = { start: 0, end: 4 * BAR };
    const { state, row } = run(s, { type: "duplicateSection", marker: 1 });
    expect(clips(state, 0)).toEqual([
      ["イントロ", 0, 4],
      ["イントロ", 4, 8],
      ["サビ", 16, 20],
    ]);
    // 「フィル」 (bars 3–4) is in the section; the clips from bar 5 on only move.
    expect(clips(state, 2)).toEqual([
      ["フィル", 2, 4],
      ["フィル", 6, 8],
      ["8ビート", 8, 16],
      ["サビ ビート", 16, 20],
    ]);
    const ids = state.song.tracks.flatMap((t) => t.clips.map((c) => c.id));
    expect(new Set(ids).size).toBe(ids.length);
    expect(marks(state).slice(0, 3)).toEqual([
      ["イントロ", 0, "usable"],
      ["イントロ", 4, "usable"],
      ["Aメロ", 8, "notOnBars"],
    ]);
    expect(new Set(state.timeline.markers.map((m) => m.id)).size).toBe(6);
    expect(state.song.loopRange).toEqual({ start: 0, end: 4 * BAR });
    expect(state.arrange.selectedClips).toEqual([]);
    expect(state.transport.position).toBe(s.transport.position);
    expect(row).toMatchObject({ name: { kind: "duplicateSection" }, what: { kind: "section", name: "イントロ", bars: 4 } });
    expect(state.song.undoCount).toBe(s.song.undoCount + 1);
  });

  it("「サビ」 taken out: its clips and its marker go, and the marker at the song's end comes up", () => {
    const s = fake(scene());
    const { state, row } = run(s, { type: "deleteSection", marker: 3 });
    expect(clips(state, 0)).toEqual([["イントロ", 0, 4]]);
    // 「8ビート」 (bars 5–12) is cut at bar 10.
    expect(clips(state, 2)).toEqual([
      ["フィル", 2, 4],
      ["8ビート", 4, 9],
    ]);
    expect(marks(state).slice(-2)).toEqual([
      ["ブリッジ", 6.25, "notOnBars"],
      ["おわり", 9, "empty"],
    ]);
    expect(row).toMatchObject({ name: { kind: "deleteSection" }, what: { kind: "section", name: "サビ", bars: 7 } });
  });

  it("empty bars before 「サビ」: a clip over the cut is cut there, tempo points and meter changes after it move", () => {
    const s = fake(scene());
    s.timeline.tempoPoints = [
      { tick: 4 * BAR, tempoBpm: 104, ramp: false },
      { tick: 12 * BAR, tempoBpm: 120, ramp: false },
    ];
    s.timeline.meterChanges = [{ bar: 12, meterNum: 2, meterDen: 4 }];
    s.song.loopRange = { start: 8 * BAR, end: 10 * BAR };
    const { state, row } = run(s, { type: "insertSpace", marker: 3, bars: 2 });
    expect(clips(state, 2)).toEqual([
      ["フィル", 2, 4],
      ["8ビート", 4, 9],
      ["8ビート", 11, 14],
      ["サビ ビート", 14, 18],
    ]);
    const [left, right] = [state.song.tracks[2]!.clips[1]!, state.song.tracks[2]!.clips[2]!];
    expect(right.takeId).toBe(left.takeId);
    expect(right.takeOffset).toBe(left.takeOffset - 5 * BAR);
    expect(state.timeline.tempoPoints.map((p) => p.tick / BAR)).toEqual([4, 14]);
    expect(state.timeline.meterChanges).toEqual([{ bar: 14, meterNum: 2, meterDen: 4 }]);
    expect(state.song.loopRange).toEqual({ start: 8 * BAR, end: 12 * BAR });
    expect(marks(state)[3]).toEqual(["サビ", 11, "usable"]);
    expect(row).toMatchObject({ name: { kind: "insertSpace" }, what: { kind: "section", name: "サビ", bars: 2 } });
  });

  it("nothing for a marker that is not there, a section that can not be used, bars out of 1–64, or while recording", () => {
    const s = fake(scene());
    const none = (command: Command, state = s) => expect(run(state, command)).toMatchObject({ row: null, changed: 0 });
    none({ type: "duplicateSection", marker: 99 });
    none({ type: "deleteSection", marker: 2 });
    none({ type: "insertSpace", marker: 5, bars: 4 });
    none({ type: "insertSpace", marker: 1, bars: 0 });
    none({ type: "insertSpace", marker: 1, bars: 65 });
    const busy = fake(scene());
    busy.transport.state = "recording";
    none({ type: "duplicateSection", marker: 1 }, busy);
    expect(reduceSections(s, { type: "undo" }, FAKE_TAKES)).toBeNull();
  });
});
