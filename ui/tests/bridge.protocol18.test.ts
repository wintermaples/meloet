// Protocol 18: plugin windows, plugins in the params panel, the rest of 86 in the mixer. Fake scenes and fake answers.
import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene, fakeState, reduceMixer, type FakeState } from "../src/bridge/fake";
import { FAKE_TABLES, SURGE_AUTOMATED, fakeParamTable } from "../src/bridge/fakeParams";
import { ParamTableBook } from "../src/bridge/paramTable";
import { PROTOCOL_VERSION, type Command, type InsertRef, type ParamTable, type ParamValues, type PluginSlotRef } from "../src/bridge/types";
import { builtinOf, pluginAt, pluginOf, selectedInsert, selectedInstrument, windowOf } from "../src/components/params/selected";
import { mockScreenText as mockText } from "./design-sources";

const scene = (name: string) => fakeScene(name)!.state;
const run = (s: FakeState, command: Command) => reduceMixer(s, s.shelf, s.song, command)!;
const rows = (table: ParamTable) => table.groups.flatMap((g) => g.params);

describe("shape of protocol 18", () => {
  it("version is 23 (after 22). A new song has no windows and nothing chosen. The old panel is no longer in the protocol", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(26);
    const s = fakeState();
    expect(s.windows).toEqual({ list: [] });
    expect(s.params).toEqual({ selected: null, clips: null });
    expect(s.shelf.currentEffect).toBeNull();
    expect(s.song.tracks[0]!.sendDefaults).toEqual([-15, null]);
  });

  it("every scene loads", () => {
    for (const name of ["windowOpen", "paramFew", "paramMany", "paramSearch", "paramNoWindow", "mixerBypass", "insertReplace", "mixerFx", "paramsNone", "noOutput", "paramClamped", "windowUndo"]) {
      expect(fakeScene(name), name).not.toBeNull();
    }
  });
});

describe("fake scenes (the mock content)", () => {
  it("F-WindowOpen: two windows (Surge XT as the instrument of 1 piano, Dragonfly Reverb on return A). The folded panel is「Surge XT 774」(the reading of 91-3)", () => {
    const s = scene("windowOpen");
    expect(s.windows.list.map((w) => w.name)).toEqual(["Surge XT", "Dragonfly Reverb"]);
    const surge: PluginSlotRef = { kind: "instrument", track: s.song.tracks[0]!.id };
    expect(windowOf(s.windows, surge)?.largerThanScreen).toBe(true);
    expect(windowOf(s.windows, { kind: "insert", insert: { strip: { kind: "return", bus: "a" }, slot: 0 } })?.name).toBe("Dragonfly Reverb");
    expect(windowOf(s.windows, { kind: "instrument", track: s.song.tracks[1]!.id })).toBeNull();
    expect(pluginAt(s.song, surge)).toMatchObject({ name: "Surge XT", format: "vst3", state: "ready" });
    expect(pluginOf(s.params)).toMatchObject({ slot: surge, count: 774, window: "opens" });
    expect(selectedInstrument(s.params)).toBe(surge.track);
    expect(s.workspace.layout.rightOpen).not.toContain("params");
    expect(s.song.tracks.map((t) => [t.name, t.gainDb, t.pan, t.sends])).toEqual([
      ["ピアノ", -4.8, -12, [-12, null]],
      ["ベース", -7.5, 0, [null, null]],
      ["ドラム", -6, 0, [-18, null]],
      ["パッド", -12, 24, [-6, -15]],
      ["ボーカル", -3.2, 0, [-9, -18]],
    ]);
    expect(s.song.tracks[2]!.slots[1]?.effect).toMatchObject({ kind: "plugin", name: "Brick EQ", state: "stopped" });
  });

  it("F-ParamFew: Dragonfly Reverb has 18, no groups. No window open", () => {
    const s = scene("paramFew");
    const sel = pluginOf(s.params)!;
    expect(selectedInsert(s.params)).toEqual({ strip: { kind: "return", bus: "a" }, slot: 0 });
    expect(pluginAt(s.song, sel.slot)).toMatchObject({ name: "Dragonfly Reverb", format: "lv2" });
    expect(windowOf(s.windows, sel.slot)).toBeNull();
    const table = fakeParamTable(sel.table!)!;
    expect(table.groups.map((g) => g.name)).toEqual([null]);
    expect(rows(table).map((r) => [r.name, r.text]).slice(6, 9)).toEqual([["Predelay", "4 ms"], ["Decay", "1.3 s"], ["Diffuse", "90 %"]]);
    expect([rows(table).length, sel.count]).toEqual([18, 18]);
    expect(s.workspace.layout.rightFront).toBe("params");
  });

  it("F6-ParamMany, F-ParamSearch: Surge XT has 774, 6 groups, and 2080 that only take keyboard controls.「cutoff」has 12 each in Scene A and Scene B", () => {
    const s = scene("paramMany6");
    const sel = pluginOf(s.params)!;
    const table = fakeParamTable(sel.table!)!;
    expect(table.groups.map((g) => [g.name, g.params.length])).toEqual([["Macros", 8], ["Global", 21], ["FX", 120], ["Scene A", 300], ["Scene B", 300], [null, 25]]);
    const squash = (x: string) => x.replace(/\s+/g, "");
    expect(squash(mockText("F6-ParamMany"))).toContain(squash("Global 21 ▸ FX 120 ▸ Scene A 300 ▸ Scene B 300 ▸ そのほか 25"));
    expect(rows(table).length).toBe(774);
    expect(sel.count).toBe(774);
    expect(table.keyboardKnobs.length).toBe(2080);
    expect([table.keyboardKnobs[0]!.name, table.keyboardKnobs.at(-1)!.name]).toEqual(["MIDI CC 0|0", "MIDI CC 15|129"]);
    expect(new Set([...rows(table), ...table.keyboardKnobs].map((r) => r.id)).size).toBe(774 + 2080);
    expect(table.groups.map((g) => g.params.filter((r) => r.name.toLowerCase().includes("cutoff")).length)).toEqual([0, 0, 0, 12, 12, 0]);
    expect(table.groups[0]!.params.map((r) => r.text)).toEqual(["35.00 %", "0.00 %", "62.50 %", "0.00 %", "0.00 %", "100.00 %", "0.00 %", "12.00 %"]);
    expect(sel.automated.map((id) => rows(table).find((r) => r.id === id)!.name)).toEqual(["A Filter 1 Cutoff", "A Osc 1 Pitch"]);
    expect(sel.automated).toEqual(SURGE_AUTOMATED);
    expect(s.status.message).toEqual({ id: "pluginWindowLarge", name: "Surge XT", slot: sel.slot, width: 1597, height: 995 });
    expect(s.windows.list).toHaveLength(1);
    expect(scene("paramSearch").status.message).toMatchObject({ id: "summary" });
  });

  it("F-ParamNoWindow: Mossy Verb cannot open a window yet. It has stepped and on/off parameters", () => {
    const s = scene("paramNoWindow");
    const sel = pluginOf(s.params)!;
    expect(sel.window).toBe("notYet");
    expect(s.status.message).toEqual({ id: "pluginWindowNotYet", loopBars: { from: 5, to: 9 }, name: "Mossy Verb" });
    const all = rows(fakeParamTable(sel.table!)!);
    expect(all).toHaveLength(12);
    expect(all.map((r) => r.kind.kind)).toEqual(["steps", "toggle", "smooth", "smooth", "smooth", "smooth", "smooth", "smooth", "steps", "smooth", "smooth", "toggle"]);
    const shape = all[8]!;
    expect(shape.kind).toEqual({ kind: "steps", steps: 12, labels: [] });
    expect(Math.round(shape.value! * 11) + 1).toBe(3);
  });

  it("F5-MixerBypass: MVerb on 4 pads is bypassed and chosen. Brick EQ on 3 drums is stopped", () => {
    const s = scene("mixerBypass");
    const insert = selectedInsert(s.params)!;
    expect(insert).toEqual({ strip: { kind: "track", id: s.song.tracks[3]!.id }, slot: 0 });
    expect(s.song.tracks[3]!.slots[0]).toMatchObject({ bypassed: true, effect: { name: "MVerb" } });
    expect(fakeParamTable(FAKE_TABLES.mverb)!.groups[0]!.params).toHaveLength(9);
  });

  it("R9-Inserts, R8-MixerFx, R9-Params, Opt5-Undo", () => {
    const replace = scene("insertReplace");
    const bass = { kind: "track", id: replace.song.tracks[1]!.id } as const;
    expect(replace.shelf).toMatchObject({
      mode: { kind: "replaceEffect", strip: bass, slot: 0 },
      chosenEffect: { kind: "builtin", effect: "eq4" },
      currentEffect: { kind: "builtin", effect: "compressor" },
    });
    expect(replace.song.tracks[1]!.slots[0]?.effect).toEqual({ kind: "builtin", effect: "compressor" });
    expect(scene("mixerFx").shelf).toMatchObject({ mode: { kind: "insertEffect", strip: bass, slot: 1 }, chosenEffect: { kind: "builtin", effect: "reverb" } });
    expect(scene("paramsNone").params.selected).toBeNull();
    expect(scene("paramsNone").workspace.layout.rightFront).toBe("params");
    expect(scene("noOutput").outputs.chosen).toBeNull();
    const clamped = scene("paramClamped");
    expect(clamped.status.message).toEqual({ id: "paramClamped", to: { kind: "builtin", effect: "compressor", param: 1, value: 20 } });
    expect(builtinOf(clamped.params)).toMatchObject({ insert: { strip: bass, slot: 0 }, effect: "compressor", row: 1 });
    expect(builtinOf(clamped.params)!.values[1]).toBe(20);
    expect(scene("windowUndo").song).toMatchObject({ undoCount: 15, redoCount: 0, file: "changed" });
  });
});

describe("fake answers (protocol 18 intents)", () => {
  it("toggleBypass: bypass and back. A stopped plugin and an empty slot do nothing", () => {
    const s = scene("mixerBypass");
    const at = (track: number, slot: number): InsertRef => ({ strip: { kind: "track", id: s.song.tracks[track]!.id }, slot });
    expect(run(s, { type: "toggleBypass", insert: at(3, 0) }).song.tracks[3]!.slots[0]!.bypassed).toBe(false);
    expect(run(s, { type: "toggleBypass", insert: at(0, 0) }).song.tracks[0]!.slots[0]!.bypassed).toBe(true);
    expect(run(s, { type: "toggleBypass", insert: at(2, 1) }).song).toEqual(s.song);
    expect(run(s, { type: "toggleBypass", insert: at(0, 3) }).song).toEqual(s.song);
  });

  it("moveInsert: the slots between move by one. The chosen slot and the window follow their content", () => {
    const s = scene("windowOpen");
    const piano = { kind: "track", id: s.song.tracks[0]!.id } as const;
    const down = run(s, { type: "moveInsert", insert: { strip: piano, slot: 0 }, to: 3 });
    expect(down.song.tracks[0]!.slots.map((x) => (x?.effect.kind === "builtin" ? x.effect.effect : null))).toEqual(["compressor", null, null, "eq4"]);
    expect(run(s, { type: "moveInsert", insert: { strip: piano, slot: 2 }, to: 0 }).song).toEqual(s.song);
    const a = { kind: "return", bus: "a" } as const;
    const picked = run(s, { type: "selectInsert", insert: { strip: a, slot: 0 } });
    const moved = run({ ...s, params: picked.params }, { type: "moveInsert", insert: { strip: a, slot: 0 }, to: 2 });
    expect(selectedInsert(moved.params)).toEqual({ strip: a, slot: 2 });
    expect(moved.windows.list[1]!.slot).toEqual({ kind: "insert", insert: { strip: a, slot: 2 } });
    expect(moved.song.returns[0].slots[2]?.effect).toMatchObject({ name: "Dragonfly Reverb" });
  });

  it("selectInsert, selectInstrument: a plugin that can open a window opens it and the panel is not shown. One that cannot yet brings the panel forward", () => {
    const s = scene("paramFew");
    const a: InsertRef = { strip: { kind: "return", bus: "a" }, slot: 0 };
    const none = { ...s, params: { selected: null, clips: null } };
    const opened = run(none, { type: "selectInsert", insert: a });
    expect(pluginOf(opened.params)).toMatchObject({ slot: { kind: "insert", insert: a }, table: FAKE_TABLES.dragonfly, count: 18 });
    expect([opened.windows.list.map((w) => w.name), opened.showParams]).toEqual([["Dragonfly Reverb"], false]);
    expect(run({ ...s, windows: opened.windows }, { type: "closePluginWindow", slot: { kind: "insert", insert: a } }).windows.list).toEqual([]);
    const mossy = scene("paramNoWindow");
    const pad: InsertRef = { strip: { kind: "track", id: mossy.song.tracks[3]!.id }, slot: 0 };
    const shown = run({ ...mossy, params: { selected: null, clips: null } }, { type: "selectInsert", insert: pad });
    expect([pluginOf(shown.params)?.window, shown.windows.list, shown.showParams]).toEqual(["notYet", [], true]);
    const surge = scene("paramMany");
    const track = surge.song.tracks[0]!.id;
    expect(selectedInstrument(run({ ...surge, params: { selected: null, clips: null } }, { type: "selectInstrument", track }).params)).toBe(track);
    // A built-in sound says in the panel that it has no parameters (19-A).
    expect(run(surge, { type: "selectInstrument", track: surge.song.tracks[1]!.id }).params.selected).toMatchObject({ kind: "sound", track: surge.song.tracks[1]!.id });
    expect(pluginOf(run(surge, { type: "selectParam", param: 1012 }).params)?.row).toBe(1012);
  });

  it("replace: the shelf opens with replaceEffect and Enter replaces. The same one as now does nothing", async () => {
    const start = scene("mixer");
    const transport = createFakeTransport(structuredClone(start));
    let state = (await transport.snapshot()).state;
    await transport.subscribe(
      (m) => {
        if (m.kind !== "patch") return;
        state = { ...state };
        for (const c of m.changes) (state as unknown as Record<string, unknown>)[c.section] = c.value;
      },
      () => {},
    );
    const strip = { kind: "track", id: start.song.tracks[1]!.id } as const;
    await transport.send({ type: "openShelf", mode: { kind: "replaceEffect", strip, slot: 0 } });
    expect(state.shelf).toMatchObject({ mode: { kind: "replaceEffect", strip, slot: 0 }, currentEffect: { kind: "builtin", effect: "compressor" } });
    await transport.send({ type: "shelfChooseEffect", effect: { kind: "builtin", effect: "compressor" } });
    await transport.send({ type: "shelfConfirm" });
    expect(state.shelf.mode.kind).toBe("replaceEffect");
    await transport.send({ type: "shelfChooseEffect", effect: { kind: "builtin", effect: "eq4" } });
    await transport.send({ type: "shelfConfirm" });
    expect(state.song.tracks[1]!.slots[0]?.effect).toEqual({ kind: "builtin", effect: "eq4" });
    expect(state.shelf).toMatchObject({ mode: { kind: "newTrack" }, currentEffect: null, chosenEffect: null });
    expect(builtinOf(state.params)).toMatchObject({ insert: { strip, slot: 0 }, effect: "eq4" });
    expect(await transport.pluginParams(FAKE_TABLES.mossy)).toMatchObject({ table: FAKE_TABLES.mossy, seq: 0 });
    expect(await transport.pluginParams(99)).toBeNull();
  });
});

describe("parameter list store (ParamTableBook)", () => {
  const table = (id: number, seq = 0): ParamTable => ({
    table: id,
    seq,
    groups: [{ name: null, params: [1, 2].map((n) => ({ id: n, name: `P${n}`, kind: { kind: "smooth" }, default: 0, value: 0, text: "0" })) }],
    keyboardKnobs: [],
  });
  const values = (id: number, seq: number, value: number, param = 1): ParamValues => ({ table: id, seq, values: [{ id: param, value, text: `${value}` }] });
  const settle = () => new Promise((r) => setTimeout(r, 0));

  it("fetches the list only once when the table changes. Values are applied in seq order; old ones and ones for other lists are dropped", async () => {
    const fetched: number[] = [];
    const told: (readonly number[] | null)[] = [];
    const book = new ParamTableBook(async (id) => (fetched.push(id), table(id)));
    book.listen((changed) => told.push(changed));
    book.show(7);
    book.show(7);
    expect(book.table()).toBeNull();
    await settle();
    expect([fetched, told]).toEqual([[7], [null]]);
    book.receive(values(7, 1, 0.5));
    book.receive(values(7, 1, 0.9));
    book.receive(values(8, 2, 0.9));
    expect([book.row(1)!.value, book.row(1)!.text, book.table()!.seq]).toEqual([0.5, "0.5", 1]);
    expect(told).toEqual([null, [1]]);
    book.show(null);
    expect([book.table(), told.at(-1)]).toEqual([null, null]);
    book.receive(values(7, 2, 0.7));
    await settle();
    expect(fetched).toEqual([7]);
  });

  it("a gap in the numbers fetches again. Values that arrive while fetching are applied to the list they belong to", async () => {
    let seq = 0;
    let fetches = 0;
    const book = new ParamTableBook(async (id) => (fetches++, table(id, seq)));
    book.show(3);
    book.receive(values(3, 1, 0.25, 2));
    await settle();
    expect([fetches, book.row(2)!.value, book.table()!.seq]).toEqual([1, 0.25, 1]);
    seq = 5;
    book.receive(values(3, 5, 0.75));
    await settle();
    expect([fetches, book.table()!.seq, book.row(1)!.value]).toEqual([2, 5, 0]);
    book.receive(values(3, 6, 1));
    expect(book.row(1)!.value).toBe(1);
  });

  it("when the chosen plugin changes, answers for the old list are not used. When Rust says「もうない」, wait", async () => {
    const waiting = new Map<number, (t: ParamTable | null) => void>();
    const book = new ParamTableBook((id) => new Promise((done) => waiting.set(id, done)));
    book.show(1);
    book.show(2);
    waiting.get(1)!(table(1));
    await settle();
    expect(book.table()).toBeNull();
    waiting.get(2)!(null);
    await settle();
    expect(book.table()).toBeNull();
    book.show(4);
    await settle();
    waiting.get(4)!(table(4));
    await settle();
    expect(book.table()!.table).toBe(4);
  });
});
