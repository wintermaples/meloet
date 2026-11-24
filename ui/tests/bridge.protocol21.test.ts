// Protocol 21: the `automation` and `history` sections, band headings and points, history rows, step names, fake scenes, status line text.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, AutomationView, Command, HistoryRow, HistoryView, LaneRef, StatusMessage } from "../src/bridge";
import { PROTOCOL_VERSION, STEP_NAME_KINDS } from "../src/bridge";
import { createFakeTransport, fakeHistoryRows, fakeScene, fakeState } from "../src/bridge/fake";
import { decodeLane } from "../src/bridge/lane";
import { historyRowText, laneNameText, stepNameText, stepPlaceText, stepWhatText } from "../src/components/history/stepText";
import { dbToFrac } from "../src/components/mixer/scale";
import StatusLine from "../src/components/StatusLine.svelte";
import { fill, stepNames, type Entry } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { mockScreenText, sectionText, specDocument } from "./design-sources";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!;
const BAR = 4 * TPQ;
const TRACK = 10;
const volume = (number: number): LaneRef => ({ home: { kind: "track", track: TRACK + number }, target: { kind: "volume" } });

describe("protocol 21 shape", () => {
  it("version is 23 (after 22). A new song has no bands and 0 steps in history. Inserts have ids (1, 2, 3 in a new song)", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(26);
    const s = fakeState();
    expect(s.automation).toEqual({ folded: [], armed: [], writing: [], held: [], selected: null } satisfies AutomationView);
    expect(s.history).toEqual({ total: 0, redo: 0, current: 0, newest: 0 } satisfies HistoryView);
    expect(s.song.tracks.map((t) => t.lanes)).toEqual([[]]);
    expect([...s.song.returns, s.song.master].map((strip) => strip.slots[0]!.id)).toEqual([1, 2, 3]);
  });

  it("intents: a band is pointed at by track and target, a point by laneId and index. A history jump uses the row index and the step number", () => {
    const commands: Command[] = [
      { type: "addLane", home: { kind: "track", track: 11 }, target: { kind: "pan" } },
      { type: "addLane", home: { kind: "track", track: 11 }, target: { kind: "send", bus: "a" } },
      { type: "addLane", home: { kind: "track", track: 11 }, target: { kind: "mute" } },
      { type: "addLane", home: { kind: "track", track: 11 }, target: { kind: "instrument", param: 1000 } },
      { type: "addLane", home: { kind: "track", track: 11 }, target: { kind: "insert", insert: 5, param: 0 } },
      { type: "deleteLane", home: { kind: "track", track: 11 }, target: { kind: "volume" } },
      { type: "foldLane", home: { kind: "track", track: 11 }, target: { kind: "volume" }, folded: true },
      { type: "addAutoPoint", lane: 501, tick: 4 * BAR + 2 * TPQ, value: null },
      { type: "addAutoPoint", lane: 501, tick: 4 * BAR + 2 * TPQ, value: 0.52 },
      { type: "moveAutoPoints", lane: 501, points: [1, 2], dTick: TPQ, dValue: -0.05 },
      { type: "deleteAutoPoints", lane: 501, points: [1] },
      { type: "selectAutoPoints", lane: 501, points: [1, 2, 3] },
      { type: "setLaneWrite", home: { kind: "track", track: 11 }, target: { kind: "volume" }, on: true },
      { type: "restoreLane", home: { kind: "track", track: 11 }, target: { kind: "volume" } },
      { type: "historyJump", index: 11, step: 11 },
    ];
    expect(new Set(commands.map((c) => c.type)).size).toBe(10);
  });
});

describe("step names (spec 3 (Y19))", () => {
  const names = Object.entries(stepNames) as [string, Entry][];
  const filled = (e: Entry) => fill(e.text, e.example ?? {});

  it("every kind in the protocol has a name, and no name is outside the protocol (fails if even one step has no name)", () => {
    expect(STEP_NAME_KINDS.length).toBe(105);
    expect(names.map(([kind]) => kind).sort()).toEqual([...STEP_NAME_KINDS].sort());
    for (const [kind, e] of names) {
      expect(e.text.trim(), kind).not.toBe("");
      expect(e.source, kind).toMatch(/^(spec:\d|mock:[\w-]+|(user|builder):\d{4}-\d{2}-\d{2})$/);
      expect(filled(e), kind).not.toMatch(/\{\w+\}/);
    }
  });

  /** Opens the chapter 3 list into names: 「CC の点を書く ・ 動かす ・ 消す」→「CC の点を書く」「CC の点を動かす」「CC の点を消す」, 「…名前を変える」→「…の名前を変える」. */
  function specNames(): string[] {
    const text = sectionText(specDocument(), "s3");
    const list = /The step names are (.+?)\. For a step typed into a field/.exec(text)![1]!;
    const out: string[] = [];
    for (const entry of list.split(", ")) {
      // After the name, a note in half-width parentheses (example names in 「」).
      const [, item, note] = /^「([^「」]+)」(?: \((.*)\))?$/.exec(entry)!;
      // 「だんだんにする ・ だんだんをやめる」: both are names as they are.
      if (item === "だんだんにする ・ だんだんをやめる") {
        out.push(...item.split(" ・ "));
        continue;
      }
      const quoted = note === undefined ? null : /^「(.+)」$/.exec(note);
      if (quoted) out.push(quoted[1]!);
      const plain = item!;
      const at = plain.indexOf("を");
      if (at < 0 || plain.startsWith("（")) {
        out.push(plain);
        continue;
      }
      const noun = plain.slice(0, at);
      for (const verb of plain.slice(at + 1).split(" ・ ")) out.push(verb.includes("を") ? `${noun}の${verb}` : `${noun}を${verb}`);
    }
    return out;
  }

  it("names from spec:3 keep the wording of the chapter 3 list", () => {
    const spec = specNames();
    expect(spec).toContain("CC の点を動かす");
    expect(spec).toContain("トラックの名前を変える");
    expect(spec).toContain("テンポ ・ 拍子の変更を消す");
    expect(spec).toContain("コンプレッサーの「しきい値」を変える");
    for (const [kind, e] of names.filter(([, x]) => x.source === "spec:3")) expect(spec, kind).toContain(filled(e));
    // Names in the list but not in the steps: 「パラメータを変える」 (shown in the form 「コンプレッサーの「しきい値」を変える」), and plugin windows (shown with the chapter 6 text).
    // 「戻した変更をすてる」 is 「戻した変更をすてる（最後に保存した形にする）」 in chapter 6.
    // 「まとめて変える（N 個）」 is written with N for the count.
    const used = new Set(names.flatMap(([, e]) => [filled(e).replace(/（[^）]*）$/, ""), e.text.replace("{count}", "N")]));
    expect(spec.filter((name) => !used.has(name))).toEqual(["パラメータを変える", "（プラグイン）の窓で変えた所"]);
  });

  it("names from spec:6 and mock are written there as they are", () => {
    expect(sectionText(specDocument(), "s6")).toContain(filled(stepNames.discardRecovered));
    expect(sectionText(specDocument(), "s6")).toContain(filled(stepNames.pluginWindow));
    expect(mockScreenText("F-Recover2")).toContain(filled(stepNames.recovered));
  });

  it("temporary names of steps not in the list (builder; returned to the design in docs/history/design-notes.ja.md B-92). The other temporary names went into the chapter 3 list in version 11 (92-7)", () => {
    expect(Object.fromEntries(names.filter(([, e]) => e.source.startsWith("builder:")).map(([kind, e]) => [kind, e.text]))).toEqual({
      clearComp: "いいとこ取りを外す",
      cutClips: "クリップを切り取る",
      cutNotes: "ノートを切り取る",
      importAudio: "音のファイルを入れる",
      setClipFade: "フェードを変える",
      setClipFollow: "のび縮みを変える",
      setClipGain: "クリップの音量を変える",
      setClipPitch: "音の高さを変える",
      setSourceTempo: "この音のテンポを決める",
      setVocalInput: "歌の入口を変える",
      unmute: "ミュートを外す",
    });
  });

  it("names with values filled in", () => {
    expect(stepNameText({ kind: "setParam", effect: "compressor", param: 0 })).toBe("コンプレッサーの「おさえはじめ」を変える");
    expect(stepNameText({ kind: "setPluginParam", plugin: "Surge XT", param: "A Filter 1 Cutoff" })).toBe("Surge XT の「A Filter 1 Cutoff」を変える");
    expect(stepNameText({ kind: "setPluginParam", plugin: "Surge XT", param: null })).toBe("Surge XT のパラメータを変える");
    expect(stepNameText({ kind: "pluginWindow", plugin: "Surge XT" })).toBe("Surge XT の窓で変えた所");
    expect(stepNameText({ kind: "moveNotes" })).toBe("ノートを動かす");
  });
});

describe("history row text (mocks R10-History / F-History)", () => {
  const ctx = scene("historyUndone3").state;
  const text = (row: HistoryRow) => [row.index, historyRowText(row, ctx), stepPlaceText(row.place, ctx)];

  it("historyUndone3 (R10-History): 16 steps, 3 undone, now at step 13. Rows use the mock text", () => {
    const s = scene("historyUndone3");
    expect(s.state.history).toEqual({ total: 16, redo: 3, current: 13, newest: 16 });
    expect([s.state.song.undoCount, s.state.song.redoCount]).toEqual([13, 3]);
    expect(s.state.workspace.layout.lower.map((b) => b.band)).toEqual(["history"]);
    expect(s.state.status.message).toEqual({ id: "undoPending", count: 3 });
    expect(s.history.map((r) => [r.index, r.step])).toEqual(Array.from({ length: 16 }, (_, i) => [i + 1, i + 1]));
    expect(s.history.slice(9).reverse().map(text)).toEqual([
      [16, "「ノートを動かす」・ 3 ノート", "ピアノロール"],
      [15, "「ノートの長さを変える」・ 2 ノート", "ピアノロール"],
      [14, "「クオンタイズ」・ 12 ノート", "ピアノロール"],
      [13, "「音量を変える」・ 4 パッド ・ −10.5 → −12.0 dB", "ミキサー"],
      [12, "「エフェクトをとばす」・ MVerb", "4 パッド"],
      [11, "「マーカーを足す」・ サビ", "アレンジ"],
      [10, "「テンポを変える」・ 5 小節目 96.00 → 120.00", "アレンジ"],
    ]);
    const board = mockScreenText("R10-History");
    // In spec 3 (92-3), gain is 「4 パッド ・ −10.5 → −12.0 dB」 and tempo is 「5 小節目 96.00 → 120.00」 (newer than the mock).
    // Step 14 of the mock is 「1/16 ・ 12 ノート」 (steps do not hold the grid yet). Other rows keep the mock text.
    for (const [index, row] of s.history.slice(9).map(text)) if (![14, 13, 10].includes(index as number)) expect(board, String(index)).toContain(row as string);
  });

  it("historyList (F-History): 14 steps, the top 2 can be redone", () => {
    const s = scene("historyList");
    expect(s.state.history).toEqual({ total: 14, redo: 2, current: 12, newest: 14 });
    expect(s.state.status.message).toEqual({ id: "undoPending", count: 2 });
    expect(s.history.slice(8).reverse().map(text)).toEqual([
      [14, "「音量を変える」・ 4 パッド ・ −10.5 → −12.0 dB", "ミキサー"],
      [13, "「クオンタイズ」・ 3 ノート", "ピアノロール"],
      [12, "「録音」・ テイク 2", "1 ピアノ"],
      [11, "「録音」・ テイク 1", "1 ピアノ"],
      [10, "「ループの範囲を変える」・ 5〜9 小節", "アレンジ"],
      [9, "「マーカーを足す」・ サビ", "アレンジ"],
    ]);
  });

  it("other forms of (what) and 「どこで」", () => {
    const what = (name: HistoryRow["name"], w: HistoryRow["what"]) => stepWhatText(name, w, ctx);
    const pad = { number: 4, name: "パッド" };
    expect(what({ kind: "mute" }, { kind: "none" })).toBeNull();
    expect(historyRowText({ index: 1, step: 1, name: { kind: "recovered" }, what: { kind: "none" }, place: null }, ctx)).toBe("「なおすためのファイルから戻す」");
    expect(stepPlaceText(null, ctx)).toBeNull();
    expect(what({ kind: "setTempo" }, { kind: "tempo", bar: 1, beat: 1, fromBpm: 96, toBpm: 104.5 })).toBe("曲の頭 96.00 → 104.50");
    expect(what({ kind: "addTimelineChange" }, { kind: "tempo", bar: 5, beat: 3, fromBpm: null, toBpm: 120 })).toBe("5 小節目の 3 拍目 120.00");
    expect(what({ kind: "setMeter" }, { kind: "meter", bar: 9, from: { num: 4, den: 4 }, to: { num: 3, den: 4 } })).toBe("9 小節目 4/4 → 3/4");
    expect(what({ kind: "moveTimelineChange" }, { kind: "moved", bar: 5, beat: 1, toBar: 6, toBeat: 2 })).toBe("5 小節目 → 6 小節目の 2 拍目");
    expect(what({ kind: "setTempoRamp" }, { kind: "place", bar: 5, beat: 1 })).toBe("5 小節目");
    expect(what({ kind: "setGain" }, { kind: "gain", strip: { kind: "master" }, fromDb: 0, toDb: null })).toBe("マスター ・ 0.0 → −∞ dB");
    expect(what({ kind: "setPan" }, { kind: "pan", strip: { kind: "track", track: pad }, from: 0, to: -20 })).toBe("4 パッド ・ C → L 20");
    expect(what({ kind: "setSend" }, { kind: "send", track: pad, bus: "a", fromDb: -15, toDb: -12 })).toBe("4 パッド ・ センド A −15.0 → −12.0 dB");
    expect(what({ kind: "setParam", effect: "compressor", param: 0 }, { kind: "param", from: -18, to: -24 })).toMatch(/^−18.* → −24/);
    expect(what({ kind: "replaceEffect" }, { kind: "replaced", from: { kind: "builtin", effect: "compressor" }, to: { kind: "builtin", effect: "eq4" } })).toBe("コンプレッサー → EQ 4 バンド");
    expect(what({ kind: "mute" }, { kind: "strip", strip: { kind: "return", bus: "a" } })).toBe("A リバーブ ・ オン");
    expect(what({ kind: "moveAutoPoints" }, { kind: "lane", target: { kind: "volume" }, points: 3 })).toBe("音量 ・ 3 点");
    expect(what({ kind: "deleteLane" }, { kind: "lane", target: { kind: "builtin", effect: "compressor", param: 0 }, points: null })).toBe("コンプレッサーの「おさえはじめ」");
    expect(what({ kind: "switchTake" }, { kind: "takeSwitch", from: 1, to: 2 })).toBe("テイク 1 → 2");
    expect(what({ kind: "renameTrack" }, { kind: "renamed", from: "ピアノ", to: "エレピ" })).toBe("ピアノ → エレピ");
    expect(what({ kind: "setInstrument" }, { kind: "sound", sound: { kind: "plugin", name: "Surge XT" } })).toBe("Surge XT");
    expect(stepPlaceText({ kind: "pluginWindow", strip: { kind: "track", track: { number: 5, name: "Surge XT" } } }, ctx)).toBe("5 Surge XT ・ プラグインの窓");
    expect(laneNameText({ kind: "send", bus: "b" })).toBe("センド B");
    expect(laneNameText({ kind: "plugin", plugin: "Surge XT", param: null })).toBe("Surge XT のパラメータ");
    expect(laneNameText({ kind: "plugin", plugin: "Surge XT", param: "A Osc 1 Pitch" })).toBe("Surge XT の「A Osc 1 Pitch」");
    expect([laneNameText({ kind: "pan" }), laneNameText({ kind: "mute" })]).toEqual(["パン", "ミュート"]);
  });

  it("(what) in a row: steps whose track name shows in the right column do not write the name. Windows are 「プラグインの窓」 (spec 3 (94-6))", () => {
    const piano = { number: 1, name: "ピアノ" };
    const surge = { number: 5, name: "Surge XT" };
    const row = (name: HistoryRow["name"], what: HistoryRow["what"], place: HistoryRow["place"]): HistoryRow => ({ index: 1, step: 1, name, what, place });
    const replaced: HistoryRow["what"] = {
      kind: "soundReplaced",
      track: piano,
      from: { kind: "builtin", instrument: "grandPiano" },
      to: { kind: "builtin", instrument: "organDrawbar" },
    };
    expect(text(row({ kind: "setInstrument" }, replaced, { kind: "track", track: piano }))).toEqual([1, "「音源を入れかえる」・ グランドピアノ → ドローバーオルガン", "1 ピアノ"]);
    // The status line (undone / redone) has no right column, so the name stays.
    expect(stepWhatText({ kind: "setInstrument" }, replaced, ctx)).toBe("1 ピアノ ・ グランドピアノ → ドローバーオルガン");
    expect(text(row({ kind: "pluginWindow", plugin: "Surge XT" }, { kind: "none" }, { kind: "pluginWindow", strip: { kind: "track", track: surge } }))).toEqual([
      1,
      "「Surge XT の窓で変えた所」・ プラグインの窓",
      "5 Surge XT ・ プラグインの窓",
    ]);
    expect(text(row({ kind: "record" }, { kind: "take", number: 2 }, { kind: "track", track: piano }))).toEqual([1, "「録音」・ テイク 2", "1 ピアノ"]);
    // Arrange and mixer steps keep the name.
    expect(text(row({ kind: "setTrackColor" }, { kind: "color", track: { number: 4, name: "パッド" }, color: 3 }, { kind: "arrange" }))).toEqual([1, "「トラックの色を変える」・ 4 パッド ・ 色 3", "アレンジ"]);
    expect(text(row({ kind: "mute" }, { kind: "strip", strip: { kind: "track", track: { number: 4, name: "パッド" } } }, { kind: "mixer" }))).toEqual([1, "「ミュート」・ 4 パッド ・ オン", "ミキサー"]);
  });

  it("rows not in the scene are made by the fake (index and step number are the same)", () => {
    expect(fakeHistoryRows([], 3, 0, 9).map((r) => [r.index, r.step])).toEqual([[1, 1], [2, 2], [3, 3]]);
    expect(fakeHistoryRows(scene("historyList").history, 14, 13, 99).map((r) => r.name.kind)).toEqual(["quantize", "setGain"]);
  });
});

describe("fake scenes: automation", () => {
  const lanesOf = (s: AppState, number: number) => s.song.tracks[number - 1]!.lanes;

  it("autoRec (F6-AutoRec): the gain band of 「1 ピアノ」 has 「● 書く」 on, writing from 6.3.000. Pan and threshold bands are folded", () => {
    const s = scene("autoRec").state;
    const piano = s.song.tracks[0]!;
    const compressor = piano.slots[1]!;
    expect(compressor.effect).toEqual({ kind: "builtin", effect: "compressor" });
    expect(lanesOf(s, 1).map((l) => [l.name.kind, l.laneId, l.points, l.step, l.resting])).toEqual([
      ["volume", 501, 5, false, false],
      ["pan", 502, 2, false, false],
      ["builtin", 503, 3, false, false],
    ]);
    expect(lanesOf(s, 1)[2]!.target).toEqual({ kind: "insert", insert: compressor.id, param: 0 });
    expect(new Set(s.song.tracks.flatMap((t) => t.slots).concat(s.song.returns.flatMap((r) => r.slots), s.song.master.slots).filter((i) => i !== null).map((i) => i.id)).has(0)).toBe(false);
    expect(s.automation).toEqual({
      folded: [
        { home: { kind: "track", track: TRACK + 1 }, target: { kind: "pan" } },
        { home: { kind: "track", track: TRACK + 1 }, target: lanesOf(s, 1)[2]!.target },
      ],
      armed: [volume(1)],
      writing: [{ lane: volume(1), fromTick: 5 * BAR + 2 * TPQ }],
      held: [],
      selected: null,
    } satisfies AutomationView);
    expect([s.transport.state, s.transport.position]).toEqual(["playing", 7 * BAR]);
    expect([s.song.undoCount, s.history.total]).toEqual([16, 16]);
    expect(s.status.message).toEqual({ id: "automationWriting", track: { number: 1, name: "ピアノ" }, target: { kind: "volume" } });
  });

  it("autoRest (F6-AutoRest): the fader was set to −12 dB by hand, so the gain band rests", () => {
    const s = scene("autoRest").state;
    expect(lanesOf(s, 1).map((l) => l.resting)).toEqual([true, false, false]);
    expect(s.song.tracks[0]!.gainDb).toBe(-12);
    expect(s.automation.armed).toEqual([]);
    expect(s.status.message).toEqual({ id: "automationRested", track: { number: 1, name: "ピアノ" }, target: { kind: "volume" } });
  });

  it("mixerAuto / mixerRest (F6-MixerAuto / F6-MixerRest): gain bands of 「1 ピアノ」 and 「4 パッド」. 「4 パッド」 rests", async () => {
    const auto = scene("mixerAuto").state;
    expect(auto.workspace.layout.lower.map((b) => b.band)).toEqual(["mixer"]);
    expect([lanesOf(auto, 1)[0]!.laneId, lanesOf(auto, 4)[0]!.laneId]).toEqual([501, 504]);
    expect(auto.status.message).toEqual({ id: "automationMoving", tracks: [{ number: 1, name: "ピアノ" }, { number: 4, name: "パッド" }], master: false });
    // At the play line (9.2.000), the 「1 ピアノ」 line is −2.4 dB (the mock's fader number), 「4 パッド」 is −10.5 dB.
    const t = createFakeTransport(auto);
    const piano = decodeLane(await t.lane(501));
    const pad = decodeLane(await t.lane(504));
    expect(auto.transport.position).toBe(8 * BAR + TPQ);
    expect(piano.values[Array.from(piano.ticks).indexOf(auto.transport.position)]).toBeCloseTo(dbToFrac(-2.4), 6);
    expect(pad.values[1]).toBeCloseTo(dbToFrac(-10.5), 6);
    expect(piano.ticks.length).toBe(lanesOf(auto, 1)[0]!.points);
    await expect(t.lane(9999)).rejects.toThrow(/no lane 9999/);

    const rest = scene("mixerRest").state;
    expect([lanesOf(rest, 1)[0]!.resting, lanesOf(rest, 4)[0]!.resting]).toEqual([false, true]);
    expect(rest.song.tracks[3]!.gainDb).toBe(-12);
    expect(rest.status.message).toEqual({ id: "automationRested", track: { number: 4, name: "パッド" }, target: { kind: "volume" } });
  });

  it("autoAdd / autoMove / autoBox (R10-AutoEdit): the added point, the moved point, and 3 boxed points are chosen", async () => {
    const add = scene("autoAdd").state;
    expect(add.automation.selected).toEqual({ laneId: 511, points: [{ start: 3, end: 4 }] });
    const added = decodeLane(await createFakeTransport(add).lane(511));
    // −8.4 dB at 5.3.000 (the mock's reading).
    expect([added.ticks[3], added.values[3]]).toEqual([4 * BAR + 2 * TPQ, Math.fround(dbToFrac(-8.4))]);
    expect(lanesOf(add, 1)).toHaveLength(1);
    expect(lanesOf(add, 1)[0]!.points).toBe(6);

    expect(scene("autoMove").state.automation.selected).toEqual({ laneId: 510, points: [{ start: 1, end: 2 }] });
    expect(scene("autoBox").state.automation.selected).toEqual({ laneId: 510, points: [{ start: 1, end: 4 }] });
    expect(lanesOf(scene("autoBox").state, 1)[0]!.points).toBe(5);
  });

  it("paramMany6 (F6-ParamMany): the 2 rows 「オートメーションに書いている」 are 2 folded bands", () => {
    const s = scene("paramMany6").state;
    const selected = s.params.selected!;
    expect(selected.kind === "plugin" && selected.automated).toEqual([1000, 1012]);
    expect(lanesOf(s, 1).map((l) => [l.target, l.name, l.points])).toEqual([
      [{ kind: "volume" }, { kind: "volume" }, 5],
      [{ kind: "instrument", param: 1000 }, { kind: "plugin", plugin: "Surge XT", param: "A Filter 1 Cutoff" }, 3],
      [{ kind: "instrument", param: 1012 }, { kind: "plugin", plugin: "Surge XT", param: "A Osc 1 Pitch" }, 2],
    ]);
    expect(s.automation.folded.map((l) => l.target)).toEqual([{ kind: "instrument", param: 1000 }, { kind: "instrument", param: 1012 }]);
    // The original scene (F-ParamMany) still has no bands.
    expect(lanesOf(scene("paramMany").state, 1)).toEqual([]);
  });
});

describe("fake behavior", () => {
  async function connected(name: string) {
    // The scene's rows are the fake's own when it starts from the URL; a state handed in has made-up rows.
    const t = createFakeTransport(scene(name).state);
    let state = (await t.snapshot()).state;
    await t.subscribe((m) => {
      if (m.kind !== "patch") return;
      state = { ...state };
      for (const c of m.changes) (state as Record<string, unknown>)[c.section] = c.value;
    }, () => {});
    return { t, state: () => state };
  }

  it("fold / 「● 書く」 / choose / restore / add band / delete", async () => {
    const { t, state } = await connected("autoRest");
    const piano = TRACK + 1;
    await t.send({ type: "foldLane", home: { kind: "track", track: piano }, target: { kind: "volume" }, folded: true });
    expect(state().automation.folded.map((l) => l.target.kind)).toEqual(["pan", "insert", "volume"]);
    // A resting band does not turn to 「書く」. Restoring is 1 step, and the status line says so.
    await t.send({ type: "setLaneWrite", home: { kind: "track", track: piano }, target: { kind: "volume" }, on: true });
    expect(state().automation.armed).toEqual([]);
    await t.send({ type: "restoreLane", home: { kind: "track", track: piano }, target: { kind: "volume" } });
    expect(state().song.tracks[0]!.lanes[0]!.resting).toBe(false);
    expect([state().song.undoCount, state().history.total, state().history.current]).toEqual([18, 18, 18]);
    expect(state().status.message).toEqual({ id: "automationRestored", track: { number: 1, name: "ピアノ" }, target: { kind: "volume" } });
    await t.send({ type: "setLaneWrite", home: { kind: "track", track: piano }, target: { kind: "volume" }, on: true });
    expect(state().automation.armed).toEqual([volume(1)]);
    await t.send({ type: "selectAutoPoints", lane: 501, points: [3, 1, 2] });
    expect(state().automation.selected).toEqual({ laneId: 501, points: [{ start: 1, end: 4 }] });
    await t.send({ type: "selectAutoPoints", lane: 501, points: [] });
    expect(state().automation.selected).toBeNull();

    await t.send({ type: "addLane", home: { kind: "track", track: piano }, target: { kind: "mute" } });
    const mute = state().song.tracks[0]!.lanes.at(-1)!;
    expect([mute.name, mute.points, mute.step, mute.resting]).toEqual([{ kind: "mute" }, 0, true, false]);
    expect(decodeLane(await t.lane(mute.laneId)).ticks.length).toBe(0);
    // A band that exists only opens (not a step).
    const steps = state().song.undoCount;
    await t.send({ type: "addLane", home: { kind: "track", track: piano }, target: { kind: "pan" } });
    expect(state().automation.folded.some((l) => l.target.kind === "pan")).toBe(false);
    expect(state().song.undoCount).toBe(steps);
    await t.send({ type: "deleteLane", home: { kind: "track", track: piano }, target: { kind: "volume" } });
    expect(state().song.tracks[0]!.lanes.map((l) => l.name.kind)).toEqual(["pan", "builtin", "mute"]);
    expect(state().automation.armed).toEqual([]);
    // A track whose instrument is not a plugin cannot get an instrument parameter band.
    await t.send({ type: "addLane", home: { kind: "track", track: piano }, target: { kind: "instrument", param: 1 } });
    expect(state().song.tracks[0]!.lanes).toHaveLength(3);
  });

  it("history jump: the row becomes the current one and the status line says so. An old step number does nothing", async () => {
    const { t, state } = await connected("historyUndone3");
    const rows = await t.historyRows(1, 16);
    expect(rows).toHaveLength(16);
    await t.send({ type: "historyJump", index: 11, step: 999 });
    expect(state().history.current).toBe(13);
    await t.send({ type: "historyJump", index: 11, step: rows[10]!.step });
    expect(state().history).toEqual({ total: 16, redo: 5, current: 11, newest: 16 });
    expect([state().song.undoCount, state().song.redoCount]).toEqual([11, 5]);
    expect(state().status.message).toMatchObject({ id: "historyJumped", index: 11, steps: 2, forward: false });
    await t.send({ type: "historyJump", index: 16, step: rows[15]!.step });
    expect(state().history.current).toBe(16);
    expect(state().status.message).toMatchObject({ id: "historyJumped", index: 16, steps: 5, forward: true });
  });
});

describe("status line text", () => {
  let target: HTMLElement;
  beforeEach(() => {
    app.reset();
    target = document.createElement("div");
    target.className = "app";
    document.body.append(target);
  });
  afterEach(() => target.remove());

  const says = (message: StatusMessage, base = scene("tracks6").state) => {
    base.status.message = message;
    app.setState(base);
    const mounted = mount(StatusLine, { target });
    flushSync();
    const out = (target.querySelector(".message")?.textContent ?? "").replace(/\s+/g, " ").trim();
    unmount(mounted);
    return out;
  };
  const piano = { number: 1, name: "ピアノ" };
  const pad = { number: 4, name: "パッド" };

  it("the 7 in spec 6 (Mod is Ctrl)", () => {
    expect(says({ id: "undone", name: { kind: "moveNotes" }, what: { kind: "notes", count: 3 } })).toBe(
      "「ノートを動かす」を取り消しました（3 ノート）。やりなおすには Ctrl Shift Z",
    );
    expect(says({ id: "redone", name: { kind: "moveNotes" }, what: { kind: "notes", count: 3 } })).toBe("「ノートを動かす」をやりなおしました（3 ノート）。");
    expect(says({ id: "historyJumped", index: 11, name: { kind: "addMarker" }, steps: 5, forward: false })).toBe("11 手目「マーカーを足す」まで戻しました（5 手）。");
    expect(says({ id: "nothingToUndo" })).toBe("取り消せる手がありません。");
    expect(says(scene("autoRec").state.status.message!, scene("autoRec").state)).toBe("1 ピアノ ・ 音量を書いています（● 書く ・ 止めるとオフに戻ります）");
    expect(says({ id: "automationRested", track: pad, target: { kind: "volume" } })).toBe(
      "4 パッド の音量を手で動かしたので、オートメーションを休ませました。「もどす」で線のとおりに戻ります",
    );
    expect(says({ id: "automationRestored", track: pad, target: { kind: "volume" } })).toBe("4 パッド の音量を、オートメーションのとおりに戻しました。");
  });

  it("mock F6-MixerAuto text, and temporary forms", () => {
    expect(says(scene("mixerAuto").state.status.message!, scene("mixerAuto").state)).toBe("1 ピアノ と 4 パッド の音量は、オートメーションで動いています");
    expect(says({ id: "automationMoving", tracks: [piano], master: false })).toBe("1 ピアノ の音量は、オートメーションで動いています");
    expect(says({ id: "automationMoving", tracks: [piano, { number: 2, name: "ベース" }, pad], master: false })).toBe("1 ピアノ、2 ベース と 4 パッド の音量は、オートメーションで動いています");
    // A step with no (what) shows no parentheses.
    expect(says({ id: "undone", name: { kind: "recovered" }, what: { kind: "none" } })).toBe("「なおすためのファイルから戻す」を取り消しました。やりなおすには Ctrl Shift Z");
    expect(says({ id: "redone", name: { kind: "pluginWindow", plugin: "Surge XT" }, what: { kind: "none" } })).toBe("「Surge XT の窓で変えた所」をやりなおしました。");
    expect(says({ id: "historyJumped", index: 14, name: { kind: "quantize" }, steps: 2, forward: true })).toBe("14 手目「クオンタイズ」までやりなおしました（2 手）。");
    expect(says({ id: "automationWriting", track: piano, target: { kind: "builtin", effect: "compressor", param: 0 } })).toBe(
      "1 ピアノ ・ コンプレッサーの「おさえはじめ」を書いています（● 書く ・ 止めるとオフに戻ります）",
    );
  });
});
