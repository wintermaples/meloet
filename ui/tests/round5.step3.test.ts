// Screen: plugins in the params panel, typing values, windows, the rest of 86 in the mixer, status line texts, mixer meters at 30 Hz.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, PluginSlotRef, StatusMessage } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState, type FakeState } from "../src/bridge/fake";
import { FAKE_TABLES, FakeParamTables, fakeParamTable } from "../src/bridge/fakeParams";
import { ParamTableBook } from "../src/bridge/paramTable";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { stepDb } from "../src/components/mixer/control.svelte";
import { KEY_COMMIT_MS, KeySteps } from "../src/components/mixer/mixDrag";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { parseDb, parsePan } from "../src/components/mixer/scale";
import { paramUi } from "../src/components/params/paramUi.svelte";
import { FOLD_OVER, foldedAtFirst, matchCount, pluginEntries, pluginValueText, sameGroups, stepLabels } from "../src/components/params/pluginRows";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import RightRail from "../src/components/RightRail.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import TrackHeads from "../src/components/TrackHeads.svelte";
import { regionHz } from "../src/gl/budget";
import { handleKeydown } from "../src/keys/attach";
import { keyAction } from "../src/keys/keys";
import { closeMenu } from "../src/lib/menu";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

// the panel reads tables from the bridge's ParamTableBook. The tests pass a fake table as it is.
const book = { current: new ParamTableBook(async (table) => fakeParamTable(table)) };
vi.mock("../src/bridge", async (importOriginal) => ({ ...(await importOriginal<object>()), paramTable: () => book.current }));

const scene = (name: string): AppState => fakeScene(name)!.state;
const surge = () => fakeParamTable(FAKE_TABLES.surge)!;

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  paramUi.query = "";
  paramUi.typing = null;
  paramUi.moving = null;
  book.current = new ParamTableBook(async (table) => fakeParamTable(table));
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  contextMenu.open = null;
  closeMenu();
  vi.useRealTimers();
});

function show(state: AppState, component: Component<any>, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
/** Waits until the table arrives (ParamTableBook gets it with a Promise). */
async function settle(table: number | null): Promise<void> {
  book.current.show(table);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
function box(el: HTMLElement, left: number, top: number, width: number, height: number): void {
  el.getBoundingClientRect = () => ({ x: left, y: top, left, top, right: left + width, bottom: top + height, width, height, toJSON: () => ({}) });
}
function pointer(el: Element, type: string, x: number, y: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 }));
  flushSync();
}
function key(el: Element, k: string, more: KeyboardEventInit = {}): KeyboardEvent {
  const ev = new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...more });
  el.dispatchEvent(ev);
  flushSync();
  return ev;
}

describe("plugin parameter list (pluginRows)", () => {
  it("a plugin with more than 40 starts with groups folded. A group with no name is 「そのほか」 if others have names", () => {
    const t = surge();
    expect(FOLD_OVER).toBe(40);
    const folded = foldedAtFirst(t);
    const entries = pluginEntries(t, [], { query: "", folded, expanded: new Set() });
    expect(entries.map((e) => (e.kind === "heading" ? `${e.folded ? "▸" : "▾"} ${e.name} ${e.count}` : e.kind))).toEqual([
      "▸ Macros 8",
      "▸ Global 21",
      "▸ FX 120",
      "▸ Scene A 300",
      "▸ Scene B 300",
      "▸ そのほか 25",
    ]);
    // a plugin with up to 40 shows all of them open. A plugin that does not name groups is one list with no heading.
    const few = fakeParamTable(FAKE_TABLES.dragonfly)!;
    expect(foldedAtFirst(few).size).toBe(0);
    expect(pluginEntries(few, [], { query: "", folded: new Set(), expanded: new Set() }).every((e) => e.kind === "row")).toBe(true);
  });

  it("「オートメーションに書いている」 at the top (open, even when folded)", () => {
    const t = surge();
    const entries = pluginEntries(t, [1000, 1012], { query: "", folded: foldedAtFirst(t), expanded: new Set() });
    expect(entries.slice(0, 3).map((e) => (e.kind === "heading" ? e.name : e.kind === "row" ? e.row.name : e.kind))).toEqual([
      "オートメーションに書いている",
      "A Filter 1 Cutoff",
      "A Osc 1 Pitch",
    ]);
  });

  it("searching shows only matches, by group, up to 6 per group. The rest open with 「… ほか N」 (mock F-ParamSearch)", () => {
    const t = surge();
    const look = { query: "cutoff", folded: foldedAtFirst(t), expanded: new Set<string>() };
    const entries = pluginEntries(t, [1000], look);
    expect(matchCount(t, "cutoff")).toBe(24);
    const shape = entries.map((e) => (e.kind === "heading" ? `${e.name} ${e.count}` : e.kind === "more" ? `… ${e.count}` : "row"));
    expect(shape).toEqual(["Scene A 12", ...Array(6).fill("row"), "… 6", "Scene B 12", ...Array(6).fill("row"), "… 6"]);
    const more = pluginEntries(t, [], { ...look, expanded: new Set(["g:3"]) });
    expect(more.filter((e) => e.kind === "row")).toHaveLength(18);
    expect(pluginEntries(t, [], { ...look, query: "zzz" })).toEqual([]);
  });

  it("value text: as the plugin writes it. 「—」 for unknown values. The Enter list of a plugin that does not name its steps is 1, 2… (user:2026-11-19)", () => {
    expect(pluginValueText({ value: 0.5, text: "523.25 Hz", kind: { kind: "smooth" } })).toBe("523.25 Hz");
    expect(pluginValueText({ value: null, text: null, kind: { kind: "smooth" } })).toBe("—");
    expect(stepLabels({ kind: "steps", steps: 12, labels: [] })).toEqual(["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"]);
    expect(stepLabels({ kind: "steps", steps: 3, labels: ["Room", "Hall", "Plate"] })).toEqual(["Room", "Hall", "Plate"]);
  });
});

describe("plugins in the params panel (mock F-ParamFew, F-ParamMany, F-ParamNoWindow, F5-MixerBypass)", () => {
  it("name, format mark, open window, location. 「窓を前に出す」 when the window is open. Pressing sends OpenPluginWindow", async () => {
    show(scene("paramFew"), RightRail);
    await settle(FAKE_TABLES.dragonfly);
    expect(text(q("[data-test=params] .who"))).toBe("Dragonfly Reverb LV2 とばす B 窓をひらく A リバーブ ・ インサート 1");
    expect(text(q("[data-test=params-count]"))).toBe("18 / 18");
    expect(qa("[data-test=param-row]")).toHaveLength(18);
    q("[data-test=params-window]")!.click();
    expect(app.sent).toEqual([{ type: "openPluginWindow", slot: { kind: "insert", insert: { strip: { kind: "return", bus: "a" }, slot: 0 } } }]);
  });

  it("Surge XT (774, F6-ParamMany): headings start folded, only visible rows are made", async () => {
    const s = scene("paramMany");
    s.workspace.layout.rightFront = "params";
    show(s, RightRail);
    await settle(FAKE_TABLES.surge);
    expect(text(q("[data-test=params] .who"))).toBe("Surge XT VST3 窓を前に出す 1 ピアノ ・ 音源");
    expect(qa("[data-test=params-group]").map(text)).toEqual([
      "▾ オートメーションに書いている 2",
      "▸ Macros 8",
      "▸ Global 21",
      "▸ FX 120",
      "▸ Scene A 300",
      "▸ Scene B 300",
      "▸ そのほか 25",
    ]);
    expect(text(q("[data-test=params-hint]"))).toBe("40 個をこえる子は、グループをたたんで始めます");
    // parameters that only take keyboard knobs are not shown; they are mentioned below the list (spec 3 (91-3)).
    expect(text(q("[data-test=params-knobs]"))).toBe(
      "ほかに、鍵盤のつまみを受けるだけの 2080 個（MIDI CC 0|0 〜 15|129）は出していません。さがすと出ます。",
    );
    expect(text(q("[data-test=params-count]"))).toBe("774 / 774");
    // automated rows have an accent mark.
    expect(qa("[data-test=param-row] .automation.on")).toHaveLength(2);
    // even with 300 rows open, only rows around the visible part are made.
    q('[data-group="g:3"]')!.click();
    flushSync();
    expect(qa("[data-test=param-row]").length).toBeLessThan(100);
  });

  it("searching shows parameters that only take keyboard knobs in the last group 「鍵盤のつまみを受ける所」 (only when there are matches)", async () => {
    const s = scene("paramMany6");
    s.workspace.layout.rightFront = "params";
    show(s, RightRail);
    await settle(FAKE_TABLES.surge);
    const input = q("[data-test=params-search]") as HTMLInputElement;
    const search = (text: string) => {
      input.value = text;
      input.dispatchEvent(new Event("input", { bubbles: true }));
      flushSync();
    };
    search("cc 1|12");
    expect(qa("[data-test=params-group]").map(text)).toEqual(["▾ 鍵盤のつまみを受ける所 11"]);
    expect(qa("[data-test=param-row] .name").map(text).slice(0, 2)).toEqual(["MIDI CC 1|12", "MIDI CC 1|120"]);
    expect(text(q("[data-test=params-more]"))).toBe("… ほか 5");
    expect(text(q("[data-test=params-count]"))).toBe("11 / 774");
    expect(q("[data-test=params-knobs]")).toBeNull();
    search("cutoff");
    expect(qa("[data-test=params-group]").map(text)).toEqual(["▾ Scene A 12", "▾ Scene B 12"]);
  });

  it("search: matches / all, 「… ほか 6」, the no-match text and 「さがす枠を空にする」. Esc empties it", async () => {
    show(scene("paramFew"), RightRail);
    await settle(FAKE_TABLES.dragonfly);
    const input = q("[data-test=params-search]") as HTMLInputElement;
    input.value = "level";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    expect(text(q("[data-test=params-count]"))).toBe("3 / 18");
    input.value = "xyz";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    expect(text(q("[data-test=params-none] span"))).toBe("「xyz」に合うパラメータはありません。");
    expect(key(input, "Escape").defaultPrevented).toBe(true);
    expect(paramUi.query).toBe("");
  });

  it("even if undo changes the list number, the same plugin in the same slot keeps the search text, open groups and row. Choosing another plugin resets it", async () => {
    // returns the Surge XT table for any number (on each undo, core gives a new number to the table of the same plugin).
    book.current = new ParamTableBook(async (table) => ({ ...surge(), table }));
    const s = scene("paramMany");
    s.workspace.layout.rightFront = "params";
    show(s, RightRail);
    await settle(FAKE_TABLES.surge);
    q('[data-group="g:1"]')!.click();
    flushSync();
    const groups = () => qa("[data-test=params-group]").map(text);
    expect(groups()[2]).toBe("▾ Global 21");
    const rows = qa("[data-test=param-row]").length;
    expect(rows).toBeGreaterThan(20);

    const sel = s.params.selected!;
    if (sel.kind !== "plugin") throw new Error("the scene selects a plugin");
    const renumbered = (table: number): AppState => ({ ...s, params: { selected: { ...sel, table }, clips: null } });
    app.setState(renumbered(77));
    book.current.show(77);
    flushSync();
    // before the new table arrives: keeps showing the old table's rows (if the list became empty, the scroll would go back to the top).
    expect(qa("[data-test=param-row]")).toHaveLength(rows);
    await settle(77);
    expect(groups()[2]).toBe("▾ Global 21");
    expect(qa("[data-test=param-row]")).toHaveLength(rows);

    // the same while searching.
    const input = q("[data-test=params-search]") as HTMLInputElement;
    input.value = "cutoff";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
    app.setState(renumbered(78));
    await settle(78);
    expect(paramUi.query).toBe("cutoff");
    expect(input.value).toBe("cutoff");
    expect(text(q("[data-test=params-count]"))).toBe("24 / 774");

    // choosing a plugin in another slot: the search text is empty and the groups are folded.
    app.setState({ ...s, params: { selected: { ...sel, slot: { kind: "instrument", track: s.song.tracks[1]!.id }, table: 79 }, clips: null } });
    await settle(79);
    expect(paramUi.query).toBe("");
    expect(groups()[2]).toBe("▸ Global 21");
  });

  it("when a table with a different group order comes, the folding resets", () => {
    const a = surge();
    expect(sameGroups(a, { ...surge(), table: 9 })).toBe(true);
    expect(sameGroups(a, { ...a, groups: a.groups.slice(1) })).toBe(false);
    expect(sameGroups(a, fakeParamTable(FAKE_TABLES.dragonfly)!)).toBe(false);
  });

  it("a plugin whose window cannot open yet: a text above the list. No window button", async () => {
    show(scene("paramNoWindow"), RightRail);
    await settle(FAKE_TABLES.mossy);
    expect(text(q("[data-test=params-notice]"))).toBe(
      "この子の窓は、まだひらけません。パラメータはここで動かせます。 meloet が、この型の窓にまだ対応していないためです。音は、いつもどおり鳴ります。",
    );
    expect(q("[data-test=params-window]")).toBeNull();
    // 12 steps show as 「3 / 12」, up to 5 steps as squares.
    expect(qa("[data-test=param-steps]").map(text)).toEqual(["◂ ▸", "◂ 3 / 12 ▸"]);
  });

  it("a bypassed plugin: 「とばしている B」 in the heading is in the chosen form. Pressing sends ToggleBypass", async () => {
    show(scene("mixerBypass"), RightRail);
    await settle(FAKE_TABLES.mverb);
    const b = q("[data-test=params-bypass]")!;
    expect(text(b)).toBe("とばしている B");
    expect(b.classList.contains("on")).toBe(true);
    b.click();
    expect(app.sent.at(-1)).toMatchObject({ type: "toggleBypass", insert: { slot: 0 } });
  });

  it("dragging a row bar: the screen moves first with commit: false, commit: true on release. The status line meanwhile is 「… を動かしています」. Esc sends CancelMixPreview", async () => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
    const s = scene("paramFew");
    s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
    show(s, RightRail);
    await settle(FAKE_TABLES.dragonfly);
    const status = document.createElement("div");
    document.body.append(status);
    const line = mount(StatusLine, { target: status });
    try {
      const decay = qa("[data-test=param-row]").find((r) => r.querySelector(".name")?.textContent === "Decay")!;
      const bar = decay.querySelector<HTMLElement>("[data-test=param-slider]")!;
      box(bar, 0, 0, 100, 12);
      pointer(bar, "pointerdown", 50, 5);
      expect(text(status.querySelector(".message"))).toBe("Dragonfly Reverb の Decay を動かしています");
      vi.advanceTimersToNextFrame();
      pointer(bar, "pointermove", 60, 5);
      vi.advanceTimersToNextFrame();
      pointer(bar, "pointerup", 60, 5);
      const slot: PluginSlotRef = { kind: "insert", insert: { strip: { kind: "return", bus: "a" }, slot: 0 } };
      const base = { type: "setPluginParam", slot, table: FAKE_TABLES.dragonfly, param: 7 };
      expect(app.sent.filter((c) => c.type === "setPluginParam")).toEqual([
        { ...base, value: 0.5, commit: false },
        { ...base, value: 0.6, commit: false },
        { ...base, value: 0.6, commit: true },
      ]);
      app.sent.length = 0;
      pointer(bar, "pointerdown", 20, 5);
      expect(cancelDrag()).toBe(true);
      flushSync();
      expect(app.sent.at(-1)).toEqual({ type: "cancelMixPreview" });
      expect(paramUi.moving).toBeNull();
    } finally {
      unmount(line);
      status.remove();
    }
  });

  it("typing a value: Enter on the chosen row or pressing the value twice opens the input box. Enter sends TypeParam, Esc cancels. On/off toggles with Enter", async () => {
    const s = scene("paramNoWindow");
    const sel = s.params.selected!;
    if (sel.kind !== "plugin") throw new Error("the scene shows a plugin");
    sel.row = 2;
    show(s, RightRail);
    await settle(FAKE_TABLES.mossy);
    const list = q("[data-test=params-list] .list")!;
    key(list, "Enter");
    const input = q("[data-test=param-input]") as HTMLInputElement;
    expect(input.value).toBe("35 %");
    input.value = "40";
    key(input, "Enter");
    const slot = { kind: "insert", insert: { strip: { kind: "track", id: s.song.tracks[3]!.id }, slot: 0 } };
    expect(app.sent.at(-1)).toEqual({ type: "typeParam", param: { kind: "plugin", slot, table: FAKE_TABLES.mossy, param: 2 }, text: "40" });
    expect(q("[data-test=param-input]")).toBeNull();
    // press the value twice → cancel with Esc (sends nothing).
    const value = qa("[data-test=param-value]")[3]!;
    value.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    app.sent.length = 0;
    key(q("[data-test=param-input]")!, "Escape");
    expect(q("[data-test=param-input]")).toBeNull();
    expect(app.sent).toEqual([]);
    // on/off (Freeze) toggles with Enter. Steps (Type) open the step list with Enter.
    sel.row = 1;
    app.setState({ ...s, params: { selected: { ...sel }, clips: null } });
    flushSync();
    key(list, "Enter");
    expect(app.sent.at(-1)).toMatchObject({ type: "setPluginParam", param: 1, value: 1, commit: true });
    app.setState({ ...s, params: { selected: { ...sel, row: 0 }, clips: null } });
    flushSync();
    key(list, "Enter");
    const rows = contextMenu.open!.entries.filter((e): e is MenuRow => e.kind === "row");
    expect(rows.map((r) => [r.text, r.checked])).toEqual([
      ["Room", false],
      ["Hall", true],
      ["Plate", false],
      ["Chamber", false],
      ["Spring", false],
    ]);
    rows[2]!.run();
    expect(app.sent.at(-1)).toMatchObject({ type: "setPluginParam", param: 0, value: 0.5, commit: true });
  });

  it("UI-PARITY-011: with the focus on the list, ← → move the chosen plugin row and go no further", async () => {
    const s = scene("paramNoWindow");
    const sel = s.params.selected!;
    if (sel.kind !== "plugin") throw new Error("the scene shows a plugin");
    sel.row = 2;
    show(s, RightRail);
    await settle(FAKE_TABLES.mossy);
    const list = q("[data-test=params-list] .list")!;
    list.focus();
    const reached: string[] = [];
    const seen = (ev: KeyboardEvent) => reached.push(ev.key);
    document.addEventListener("keydown", seen);
    app.sent.length = 0;
    key(list, "ArrowRight");
    document.removeEventListener("keydown", seen);
    expect(app.sent).toHaveLength(1);
    expect(app.sent[0]).toMatchObject({ type: "setPluginParam", param: 2, commit: true });
    expect(reached).toEqual([]);
  });

  it("built-in effects also take typing after pressing the value twice (unit after the number)", () => {
    show(scene("paramClamped"), RightRail);
    const ratio = qa("[data-test=param-row]")[1]!;
    ratio.querySelector("[data-test=param-value]")!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    const input = q("[data-test=param-input]") as HTMLInputElement;
    expect(input.value).toBe("20.0");
    expect(text(ratio.querySelector(".unit"))).toBe(": 1");
    input.value = "4";
    key(input, "Enter");
    expect(app.sent.at(-1)).toMatchObject({ type: "typeParam", param: { kind: "builtin", effect: "compressor", param: 1 }, text: "4" });
  });

  it("two rows when nothing is chosen (86-5)", () => {
    show(scene("paramsNone"), RightRail);
    expect(text(q("[data-test=params-nothing]"))).toBe(
      "ミキサーでインサートの枠をえらぶと、ここにパラメータが出ます。 音源の枠をえらぶと、音源のパラメータが出ます。",
    );
  });
});

describe("fake answers: plugin values and typed values", () => {
  it("SetPluginParam returns the value and text in ParamValues (seq goes up by 1). TypeParam out of range gives ParamClamped, unreadable text gives ParamRejected", async () => {
    const tables = new FakeParamTables();
    expect(tables.set(FAKE_TABLES.dragonfly, 0, 0.5)).toEqual({ table: FAKE_TABLES.dragonfly, seq: 1, values: [{ id: 0, value: 0.5, text: "50 %" }] });
    expect(tables.table(FAKE_TABLES.dragonfly)!.seq).toBe(1);
    expect(tables.type(FAKE_TABLES.dragonfly, 0, "150").clamped).toEqual({ name: "Dry Level", value: "100 %", range: "0 %〜100 %" });
    expect(tables.type(FAKE_TABLES.dragonfly, 0, "あ")).toMatchObject({ values: null, rejected: "100 %" });

    const s = scene("paramFew");
    const transport = createFakeTransport(s as FakeState);
    const got: unknown[] = [];
    await transport.subscribe((m) => got.push(m), () => {});
    const slot = (s.params.selected as { slot: PluginSlotRef }).slot;
    await transport.send({ type: "typeParam", param: { kind: "plugin", slot, table: FAKE_TABLES.dragonfly, param: 0 }, text: "x" });
    expect(got).toContainEqual(expect.objectContaining({ kind: "patch", changes: [{ section: "status", value: { message: { id: "paramRejected", param: expect.anything(), kept: "80 %" } } }] }));
  });

  it("effect steps become status line text. Opening the mixer mentions bypassed and stopped slots together", async () => {
    const s = scene("mixerBypass");
    const transport = createFakeTransport(s as FakeState);
    let last: StatusMessage | null = null;
    await transport.subscribe((m) => {
      if (m.kind !== "patch") return;
      for (const c of m.changes) if (c.section === "status") last = c.value.message;
    }, () => {});
    const insert = { strip: { kind: "track", id: s.song.tracks[1]!.id }, slot: 0 } as const;
    await transport.send({ type: "toggleBypass", insert });
    expect(last).toEqual({ id: "effectBypassed", insert, effect: { kind: "builtin", effect: "compressor" }, bypassed: true });
    await transport.send({ type: "toggleBand", band: "mixer" });
    await transport.send({ type: "toggleBand", band: "mixer" });
    expect(last).toMatchObject({ id: "mixerPassesBy", bypassed: [{ effect: { kind: "builtin", effect: "compressor" } }, { effect: { kind: "plugin", name: "MVerb" } }], stopped: [{ effect: { kind: "plugin", name: "Brick EQ" } }] });
  });
});

describe("new status line texts (spec 6, mock)", () => {
  const says = (message: StatusMessage, base = scene("mixerBypass")) => {
    base.status.message = message;
    show(base, StatusLine);
    const out = text(q(".message"));
    unmount(mounted!);
    mounted = null;
    return out;
  };
  it("insert steps", () => {
    const s = scene("mixerBypass");
    const bass = { kind: "track", id: s.song.tracks[1]!.id } as const;
    const comp = { kind: "builtin", effect: "compressor" } as const;
    expect(says({ id: "effectBypassed", insert: { strip: bass, slot: 0 }, effect: comp, bypassed: true })).toBe("2 ベース の コンプレッサー をとばしました。");
    expect(says({ id: "effectBypassed", insert: { strip: bass, slot: 0 }, effect: comp, bypassed: false })).toBe("2 ベース の コンプレッサー をもどしました。");
    expect(says({ id: "effectInserted", insert: { strip: bass, slot: 1 }, effect: { kind: "builtin", effect: "reverb" } })).toBe(
      "2 ベース のインサート 2 に リバーブ を入れました。",
    );
    expect(says({ id: "effectRemoved", insert: { strip: bass, slot: 0 }, effect: comp, lanesRemoved: 0 })).toBe("2 ベース のインサート 1 の コンプレッサー を外しました。取り消せます（Ctrl Z）");
    expect(says({ id: "insertsReordered", strip: bass, order: [{ kind: "builtin", effect: "eq4" }, comp] })).toBe(
      "2 ベース のインサートを並べかえました（EQ 4 バンド → コンプレッサー）。",
    );
    expect(says({ id: "insertReplaced", insert: { strip: bass, slot: 0 }, effect: { kind: "builtin", effect: "eq4" }, lanesRemoved: 0 })).toBe(
      "2 ベース のインサート 1 を EQ 4 バンド に入れかえました。取り消せます（Ctrl Z）",
    );
    expect(
      says({
        id: "mixerPassesBy",
        bypassed: [{ insert: { strip: { kind: "track", id: s.song.tracks[3]!.id }, slot: 0 }, effect: { kind: "plugin", name: "MVerb" } }],
        stopped: [{ insert: { strip: { kind: "track", id: s.song.tracks[2]!.id }, slot: 1 }, effect: { kind: "plugin", name: "Brick EQ" } }],
      }),
    ).toBe("4 パッド の MVerb をとばしています ・ 3 ドラム の Brick EQ は止まっています（音は、その枠をとばして流しています）");
  });

  it("name holes (strip, plugin, parameter) use the same font as the text. Monospace only for number and value holes (the mock's status line)", () => {
    const mono = (message: StatusMessage) => {
      const base = scene("mixerBypass");
      base.status.message = message;
      show(base, StatusLine);
      const out = [...q(".message")!.querySelectorAll(".m-num")].map(text);
      unmount(mounted!);
      mounted = null;
      return out;
    };
    const s = scene("mixerBypass");
    const pad = { strip: { kind: "track", id: s.song.tracks[3]!.id }, slot: 0 } as const;
    expect(mono({ id: "mixerPassesBy", bypassed: [{ insert: pad, effect: { kind: "plugin", name: "MVerb" } }], stopped: [{ insert: pad, effect: { kind: "plugin", name: "Brick EQ" } }] })).toEqual([]);
    expect(mono({ id: "pluginWindowNotYet", loopBars: null, name: "Mossy Verb" })).toEqual([]);
    expect(mono({ id: "pluginStopped", name: "Surge XT", slot: { kind: "insert", insert: pad } })).toEqual([]);
    expect(mono({ id: "effectBypassed", insert: pad, effect: { kind: "plugin", name: "MVerb" }, bypassed: true })).toEqual([]);
    expect(mono({ id: "pluginWindowsOpen", count: 2 })).toEqual(["2"]);
    expect(mono({ id: "pluginWindowLarge", name: "Surge XT", slot: null, width: 1597, height: 995 })).toEqual(["1597", "995"]);
    expect(mono({ id: "effectInserted", insert: pad, effect: { kind: "plugin", name: "MVerb" } })).toEqual(["1"]);
    expect(mono({ id: "paramClamped", to: { kind: "plugin", name: "Mix", range: "0 %〜100 %", value: "100 %" } })).toEqual(["0 %〜100 %", "100 %"]);
  });

  it("stopped notice: a space after the plugin name (A-18)", () => {
    const s = scene("mixerBypass");
    expect(says({ id: "pluginStopped", name: "Dragonfly Hall Reverb", slot: { kind: "instrument", track: s.song.tracks[0]!.id } })).toBe(
      `Dragonfly Hall Reverb が止まりました ・ 1 ${s.song.tracks[0]!.name}。曲のほかの部分は動いています。音はこの枠をとばして流しています。パラメータの値は残っています。`,
    );
  });

  it("windows and parameters", () => {
    const s = scene("paramMany");
    const piano: PluginSlotRef = { kind: "instrument", track: s.song.tracks[0]!.id };
    expect(says({ id: "pluginWindowLarge", name: "Surge XT", slot: piano, width: 1597, height: 995 }, s)).toBe(
      "1 ピアノ ・ Surge XT の窓がひらいています（1597×995 ・ この画面より大きい窓です）。パラメータは右でも動かせます。",
    );
    expect(says({ id: "pluginWindowOpening", name: "Surge XT", slot: null }, scene("paramMany"))).toBe("Surge XT の窓をひらいています…");
    expect(says({ id: "pluginWindowsOpen", count: 2 }, scene("paramMany"))).toBe("プラグインの窓が 2 つひらいています");
    expect(says({ id: "pluginWindowNotYet", loopBars: { from: 5, to: 9 }, name: "Mossy Verb" }, scene("paramMany"))).toBe(
      "ループ 5–9 ・ Mossy Verb の窓はまだひらけません。パラメータは右で動かせます",
    );
    expect(says({ id: "pluginParamSelected", loopBars: null, slot: piano, plugin: "Surge XT", param: "Macro 1" }, scene("paramMany"))).toBe(
      "1 ピアノ ・ Surge XT の「Macro 1」をえらんでいます",
    );
    expect(says({ id: "paramClamped", to: { kind: "builtin", effect: "compressor", param: 1, value: 20 } }, scene("paramMany"))).toBe(
      "おさえる強さは 1〜20 なので、20.0 : 1 にしました。",
    );
    expect(
      says({ id: "paramRejected", param: { kind: "builtin", insert: { strip: { kind: "master" }, slot: 0 }, effect: "compressor", param: 0 }, kept: "-24" }, scene("paramMany")),
    ).toBe("おさえはじめは数字で打ちます。もとの −24.0 dB のままです。");
  });
});

describe("「窓 N ▾」 in the status line and its list (mock F-WindowOpen)", () => {
  it("not shown with no windows. Pressing gives the list (name and location, bring all to front). Pressing a row sends OpenPluginWindow. Esc (same order as menus) closes", () => {
    show(scene("paramFew"), StatusLine);
    expect(q("[data-test=status-windows]")).toBeNull();
    unmount(mounted!);
    const s = scene("windowOpen");
    s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: false, notes: null };
    show(s, StatusLine);
    const button = q("[data-test=status-windows]")!;
    expect(text(button)).toBe("窓 2▾");
    button.click();
    flushSync();
    const menu = document.querySelector<HTMLElement>("[data-test=windows-menu]")!;
    expect([...menu.querySelectorAll("[data-test=windows-row]")].map(text)).toEqual(["Surge XT 1 ピアノ ・ 音源", "Dragonfly Reverb A リバーブ ・ インサート 1"]);
    expect(text(menu.querySelector("[data-test=windows-raise-all]"))).toBe("ぜんぶ前に出す");
    // while open, 「プラグインの窓が 2 つひらいています」 in place of the usual summary.
    expect(text(q(".message"))).toBe("ループ 5–9 ・ プラグインの窓が 2 つひらいています");
    menu.querySelector<HTMLElement>("[data-test=windows-row]")!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "openPluginWindow", slot: { kind: "instrument", track: s.song.tracks[0]!.id } }]);
    expect(document.querySelector("[data-test=windows-menu]")).toBeNull();
    button.click();
    flushSync();
    closeMenu();
    flushSync();
    expect(document.querySelector("[data-test=windows-menu]")).toBeNull();
  });
});

describe("the rest of 86 in the mixer", () => {
  it("bypass mark: left of a filled slot. On is on; bypassed is empty with the name in gray.7 (bypassed). No mark on stopped or empty slots. Pressing sends ToggleBypass", () => {
    show(scene("mixerBypass"), MixerStrips);
    const pad = [...target.querySelectorAll<HTMLElement>('[data-strip^="track-"]')][3]!;
    const slots = [...pad.querySelectorAll<HTMLElement>("[data-test=mixer-insert]")];
    expect(slots[0]!.classList.contains("bypassed")).toBe(true);
    expect(slots[0]!.querySelector("[data-test=mixer-bypass]")!.classList.contains("on")).toBe(false);
    expect(slots[1]!.querySelector("[data-test=mixer-bypass]")!.classList.contains("on")).toBe(true);
    expect(slots[2]!.querySelector("[data-test=mixer-bypass]")).toBeNull();
    const drums = [...target.querySelectorAll<HTMLElement>('[data-strip^="track-"]')][2]!;
    expect(drums.querySelectorAll("[data-test=mixer-insert]")[1]!.querySelector("[data-test=mixer-bypass]")).toBeNull();
    slots[0]!.querySelector<HTMLElement>("[data-test=mixer-bypass]")!.click();
    // pressing the mark does not choose the slot.
    expect(app.sent).toEqual([{ type: "toggleBypass", insert: { strip: { kind: "track", id: Number(pad.dataset.strip!.slice(6)) }, slot: 0 } }]);
  });

  it("right-clicking a plugin slot that is not chosen only shows the list. The choose intent (which opens the window) is not sent", () => {
    show(scene("mixerBypass"), MixerStrips);
    const slots = qa("[data-test=mixer-insert].filled:not(.selected)").filter((el) => !el.classList.contains("stopped"));
    const plugin = qa("[data-test=mixer-insert].filled.stopped")[0]!;
    for (const el of [slots[0]!, plugin]) {
      contextMenu.open = null;
      const ev = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 5 });
      el.dispatchEvent(ev);
      expect(ev.defaultPrevented).toBe(true);
      expect(contextMenu.open).not.toBeNull();
    }
    expect(app.sent).toEqual([]);
  });

  it("a window mark on insert and instrument slots whose window is open", () => {
    show(scene("windowOpen"), MixerStrips);
    expect(target.querySelector('[data-strip="return-a"] [data-test=mixer-insert] [data-test=window-mark]')).not.toBeNull();
    expect(target.querySelector('[data-strip^="track-"] [data-test=mixer-frame] [data-test=window-mark]')).not.toBeNull();
    expect(target.querySelectorAll("[data-test=window-mark]")).toHaveLength(2);
  });

  it("plugin instrument slot: pressing sends SelectInstrument, right-click gives 「音源を入れかえる…」 (user:2026-11-19)", () => {
    const s = scene("windowOpen");
    show(s, MixerStrips);
    const frame = target.querySelector<HTMLElement>('[data-strip^="track-"] [data-test=mixer-frame]')!;
    frame.click();
    frame.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    const rows = contextMenu.open!.entries as MenuRow[];
    expect(rows.map((r) => r.label)).toEqual(["track.replaceSound"]);
    rows[0]!.run();
    const id = s.song.tracks[0]!.id;
    expect(app.sent).toEqual([{ type: "selectInstrument", track: id }, { type: "openShelf", mode: { kind: "replace", track: id } }]);
  });

  it("return name: when the top is a plugin, the kind name of 14-A (「A リバーブ」 in F-WindowOpen)", () => {
    const s = scene("windowOpen");
    show(s, MixerStrips);
    expect(text(target.querySelector('[data-strip="return-a"] .head'))).toBe("A リバーブ");
  });

  it("reorder drag: held and moved up or down, a line where it goes in, one MoveInsert on release. Esc cancels", () => {
    const s = scene("windowOpen");
    show(s, MixerStrips);
    const piano = target.querySelector<HTMLElement>('[data-strip^="track-"]')!;
    const slots = [...piano.querySelectorAll<HTMLElement>("[data-test=mixer-insert]")];
    slots.forEach((el, i) => box(el, 0, i * 26, 100, 24));
    pointer(slots[0]!, "pointerdown", 10, 10);
    pointer(slots[0]!, "pointermove", 10, 40);
    expect(slots[0]!.classList.contains("lifted")).toBe(true);
    expect(piano.querySelectorAll("[data-test=mixer-insert-line]")).toHaveLength(1);
    pointer(slots[0]!, "pointerup", 10, 40);
    expect(app.sent).toEqual([{ type: "moveInsert", insert: { strip: { kind: "track", id: s.song.tracks[0]!.id }, slot: 0 }, to: 1 }]);
    // a click after a drag does not count as pressing the slot.
    slots[0]!.querySelector<HTMLElement>(".press")!.click();
    expect(app.sent).toHaveLength(1);
    pointer(slots[1]!, "pointerdown", 10, 36);
    pointer(slots[1]!, "pointermove", 10, 70);
    expect(cancelDrag()).toBe(true);
    flushSync();
    pointer(slots[1]!, "pointerup", 10, 70);
    expect(app.sent).toHaveLength(1);
    expect(piano.querySelector(".lifted")).toBeNull();
  });

  it("master with no output: 「出口なし ▸」 (gray.8); pressing opens the sound settings (86-8)", () => {
    show(scene("noOutput"), MixerStrips);
    const frame = target.querySelector<HTMLElement>('[data-strip="master"] [data-test=mixer-frame]')!;
    expect(text(frame)).toBe("出口なし ▸");
    expect(frame.classList.contains("dim")).toBe(true);
    frame.click();
    expect(app.sent).toEqual([{ type: "openSettings", advanced: false }]);
  });

  it("fader, pan and send keys: ↑ ↓ 1 dB (Shift 0.1), ← → 1 (Shift 10), Home for the first value. commit: false for each key, commit: true 1 second after release", () => {
    vi.useFakeTimers();
    const s = scene("mixer");
    show(s, MixerStrips);
    const piano = target.querySelector<HTMLElement>('[data-strip^="track-"]')!;
    const fader = piano.querySelector<HTMLElement>("[data-test=mixer-fader]")!;
    const strip = { kind: "track", id: s.song.tracks[0]!.id } as const;
    const db0 = s.song.tracks[0]!.gainDb!;
    key(fader, "ArrowUp");
    fader.dispatchEvent(new KeyboardEvent("keyup", { key: "ArrowUp", bubbles: true }));
    key(fader, "ArrowUp", { shiftKey: true });
    fader.dispatchEvent(new KeyboardEvent("keyup", { key: "ArrowUp", bubbles: true }));
    expect(app.sent).toEqual([
      { type: "setGain", strip, db: db0 + 1, commit: false },
      { type: "setGain", strip, db: Math.round((db0 + 1.1) * 10) / 10, commit: false },
    ]);
    vi.advanceTimersByTime(KEY_COMMIT_MS);
    expect(app.sent.at(-1)).toEqual({ type: "setGain", strip, db: Math.round((db0 + 1.1) * 10) / 10, commit: true });
    app.sent.length = 0;
    // Home for the first value. When focus leaves, one step at once.
    key(fader, "Home");
    fader.dispatchEvent(new FocusEvent("blur"));
    expect(app.sent).toEqual([
      { type: "setGain", strip, db: 0, commit: false },
      { type: "setGain", strip, db: 0, commit: true },
    ]);
    app.sent.length = 0;
    const pan = piano.querySelector<HTMLElement>("[data-test=mixer-pan]")!;
    key(pan, "ArrowLeft", { shiftKey: true });
    pan.dispatchEvent(new FocusEvent("blur"));
    expect(app.sent.at(-1)).toEqual({ type: "setPan", strip, pan: s.song.tracks[0]!.pan - 10, commit: true });
    app.sent.length = 0;
    const send = piano.querySelector<HTMLElement>("[data-test=mixer-send]")!;
    // at the first amount Home sends nothing. Lower by 1 dB, then Home.
    key(send, "ArrowDown");
    key(send, "Home");
    send.dispatchEvent(new FocusEvent("blur"));
    expect(app.sent.at(-1)).toEqual({ type: "setSend", track: strip.id, bus: "a", db: s.song.tracks[0]!.sendDefaults[0], commit: true });
    // type with Enter.
    app.sent.length = 0;
    key(fader, "Enter");
    const input = piano.querySelector<HTMLInputElement>("[data-test=param-input]")!;
    input.value = "−3";
    key(input, "Enter");
    expect(app.sent).toEqual([{ type: "setGain", strip, db: -3, commit: true }]);
  });

  it("UI-PARITY-015: after Enter or Esc in the typing box, the fader or pan has the keys again", async () => {
    const s = scene("mixer");
    show(s, MixerStrips);
    const piano = target.querySelector<HTMLElement>('[data-strip^="track-"]')!;
    for (const [part, close] of [["mixer-fader", "Enter"], ["mixer-pan", "Escape"]] as const) {
      const el = piano.querySelector<HTMLElement>(`[data-test=${part}]`)!;
      el.focus();
      key(el, "Enter");
      const input = piano.querySelector<HTMLInputElement>("[data-test=param-input]")!;
      expect(document.activeElement).toBe(input);
      input.value = "−3";
      key(input, close);
      await Promise.resolve();
      await Promise.resolve();
      expect(piano.querySelector("[data-test=param-input]")).toBeNull();
      expect(document.activeElement, part).toBe(el);
    }
  });

  it("reading dB and pan, one ↑ ↓ step", () => {
    expect(stepDb(null, 1, false)).toBe(-60);
    expect(stepDb(-60, -1, false)).toBeNull();
    expect(stepDb(5.5, 1, false)).toBe(6);
    expect(stepDb(-3, -1, true)).toBe(-3.1);
    expect(parseDb("−3.0")).toBe(-3);
    expect(parseDb("+2 dB")).toBe(2);
    expect(parseDb("−∞")).toBeNull();
    expect(parseDb("あ")).toBeUndefined();
    expect(parsePan("C")).toBe(0);
    expect(parsePan("L 20")).toBe(-20);
    expect(parsePan("r150")).toBe(100);
    expect(parsePan("x")).toBeUndefined();
  });

  it("KeySteps: commit: false for each press, commit: true 1 second after release. A press in between restarts the count", () => {
    vi.useFakeTimers();
    const sent: Command[] = [];
    const k = new KeySteps<number>((v, commit) => ({ type: "setPan", strip: { kind: "master" }, pan: v, commit }), (c) => sent.push(c));
    k.step(1);
    k.release();
    vi.advanceTimersByTime(KEY_COMMIT_MS - 1);
    k.step(2);
    k.release();
    vi.advanceTimersByTime(KEY_COMMIT_MS - 1);
    expect(sent.map((c) => (c as { commit: boolean }).commit)).toEqual([false, false]);
    vi.advanceTimersByTime(1);
    expect(sent.at(-1)).toMatchObject({ pan: 2, commit: true });
    k.flush();
    expect(sent).toHaveLength(3);
  });
});

describe("instrument name in the arrange (plugin)", () => {
  it("pressing sends SelectInstrument, a window mark right of the name when the window is open, right-click gives 「音源を入れかえる…」", () => {
    const s = scene("paramMany");
    show(s, TrackHeads);
    const name = q("[data-test=track-instrument]")!;
    expect(text(name)).toBe("Surge XT");
    expect(name.querySelector("[data-test=window-mark]")).not.toBeNull();
    name.click();
    name.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    expect((contextMenu.open!.entries as MenuRow[]).map((r) => r.label)).toEqual(["track.replaceSound"]);
    expect(app.sent).toEqual([{ type: "selectInstrument", track: s.song.tracks[0]!.id }]);
  });
});

describe("key B (bypass)", () => {
  it("B is bypass. Does not work while typing", () => {
    expect(keyAction({ key: "b", ctrl: false, meta: false, alt: false, shift: false }, false, "other")).toEqual({ kind: "bypass" });
    expect(keyAction({ key: "b", ctrl: false, meta: false, alt: false, shift: false }, true, "other")).toBeNull();
  });
  it("ToggleBypass only when an insert is chosen and the mixer or the params panel is visible", () => {
    const sent: Command[] = [];
    const press = (state: AppState) =>
      handleKeydown(new KeyboardEvent("keydown", { key: "b", bubbles: true, cancelable: true }), { state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux" } });
    press(scene("mixerBypass"));
    expect(sent).toEqual([{ type: "toggleBypass", insert: { strip: { kind: "track", id: scene("mixerBypass").song.tracks[3]!.id }, slot: 0 } }]);
    press(fakeState());
    expect(sent).toHaveLength(1);
  });
});

// how 30 Hz is done (left and right one frame each) is in gl.surface.test.ts.
describe("mixer meters are always 30 Hz, whatever the number of tracks (user:2026-11-20)", () => {
  it("only mixerMeters. Other regions (keys, play line, master) stay at 60 Hz", () => {
    expect(regionHz("mixerMeters")).toBe(30);
    expect(regionHz("masterMeter")).toBe(60);
    expect(regionHz("pianoRollKeys")).toBe(60);
    expect(regionHz("trackMeters")).toBe(60);
  });
});

describe("replace shelf (ShelfMode::ReplaceEffect, mock R9-Inserts)", () => {
  it("heading, the current effect, and after choosing, the replace text and 「入れかえる Enter」「やめる Esc」", async () => {
    const { default: Shelf } = await import("../src/components/Shelf.svelte");
    const s = scene("insertReplace");
    show(s, Shelf);
    expect(text(q(".mode, [data-test=shelf-title]")) ?? text(q("[data-test=shelf]"))).toContain("2 ベース のインサート 1 を入れかえる");
    expect(text(q("[data-test=shelf-current-effect]"))).toBe("いまのエフェクト");
    expect(text(q("[data-test=shelf-insert]"))).toBe(
      "2 ベース のインサート 1 の コンプレッサー を、EQ 4 バンド に入れかえます。いまの設定は残りません（取り消せます）。 入れかえる Enter やめる Esc",
    );
  });
});
