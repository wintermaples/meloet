// 約束 21（第 6 回 区切り2 ・ 3）: `automation` と `history` の区画、帯の見出しと点、履歴の行、手の名前、偽物の場面、状態行の文。
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
const volume = (number: number): LaneRef => ({ track: TRACK + number, target: { kind: "volume" } });

describe("約束 21 の形", () => {
  it("版は 21。新しい曲に帯はなく、履歴は 0 手。インサートは id を持つ（新しい曲は 1 ・ 2 ・ 3）", () => {
    expect(PROTOCOL_VERSION).toBe(21);
    const s = fakeState();
    expect(s.automation).toEqual({ folded: [], armed: [], writing: [], held: [], selected: null } satisfies AutomationView);
    expect(s.history).toEqual({ total: 0, redo: 0, current: 0, newest: 0 } satisfies HistoryView);
    expect(s.song.tracks.map((t) => t.lanes)).toEqual([[]]);
    expect([...s.song.returns, s.song.master].map((strip) => strip.slots[0]!.id)).toEqual([1, 2, 3]);
  });

  it("意図: 帯はトラックと動かす物で、点は laneId と番号で指す。履歴のとびは行の番号と手の番号", () => {
    const commands: Command[] = [
      { type: "addLane", track: 11, target: { kind: "pan" } },
      { type: "addLane", track: 11, target: { kind: "send", bus: "a" } },
      { type: "addLane", track: 11, target: { kind: "mute" } },
      { type: "addLane", track: 11, target: { kind: "instrument", param: 1000 } },
      { type: "addLane", track: 11, target: { kind: "insert", insert: 5, param: 0 } },
      { type: "deleteLane", track: 11, target: { kind: "volume" } },
      { type: "foldLane", track: 11, target: { kind: "volume" }, folded: true },
      { type: "addAutoPoint", lane: 501, tick: 4 * BAR + 2 * TPQ, value: null },
      { type: "addAutoPoint", lane: 501, tick: 4 * BAR + 2 * TPQ, value: 0.52 },
      { type: "moveAutoPoints", lane: 501, points: [1, 2], dTick: TPQ, dValue: -0.05 },
      { type: "deleteAutoPoints", lane: 501, points: [1] },
      { type: "selectAutoPoints", lane: 501, points: [1, 2, 3] },
      { type: "setLaneWrite", track: 11, target: { kind: "volume" }, on: true },
      { type: "restoreLane", track: 11, target: { kind: "volume" } },
      { type: "historyJump", index: 11, step: 11 },
    ];
    expect(new Set(commands.map((c) => c.type)).size).toBe(10);
  });
});

describe("手の名前（仕様書 3 章 Y19）", () => {
  const names = Object.entries(stepNames) as [string, Entry][];
  const filled = (e: Entry) => fill(e.text, e.example ?? {});

  it("約束のどの種類にも名前があり、約束にない名前はない（名前のない手が 1 つでもあると落ちる）", () => {
    expect(STEP_NAME_KINDS.length).toBe(66);
    expect(names.map(([kind]) => kind).sort()).toEqual([...STEP_NAME_KINDS].sort());
    for (const [kind, e] of names) {
      expect(e.text.trim(), kind).not.toBe("");
      expect(e.source, kind).toMatch(/^(spec:\d|mock:[\w-]+|(user|builder):\d{4}-\d{2}-\d{2})$/);
      expect(filled(e), kind).not.toMatch(/\{\w+\}/);
    }
  });

  /** 3 章の一覧を名前にひらく: 「CC の点を書く ・ 動かす ・ 消す」→「CC の点を書く」「CC の点を動かす」「CC の点を消す」、「…名前を変える」→「…の名前を変える」。 */
  function specNames(): string[] {
    const text = sectionText(specDocument(), "s3");
    const list = /手の名前は、(.+?)。ここにない手の名前は/.exec(text)![1]!;
    const out: string[] = [];
    for (const item of list.split("、")) {
      const quoted = /（「(.+)」）$/.exec(item);
      if (quoted) out.push(quoted[1]!);
      const plain = item.startsWith("（") ? item : item.replace(/（[^）]*）$/, "");
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

  it("出どころが spec:3 の名前は、3 章の一覧の言いかたのまま", () => {
    const spec = specNames();
    expect(spec).toContain("CC の点を動かす");
    expect(spec).toContain("トラックの名前を変える");
    expect(spec).toContain("テンポ ・ 拍子の変更を消す");
    expect(spec).toContain("コンプレッサーの「しきい値」を変える");
    for (const [kind, e] of names.filter(([, x]) => x.source === "spec:3")) expect(spec, kind).toContain(filled(e));
    // 一覧にあって、手にない名前: 「クリップを書く」（クリップだけを書く手はない）、「ソロ」（手にしない。A-19）、
    // 「パラメータを変える」（「コンプレッサーの「しきい値」を変える」の形で出す）、プラグインの窓（6 章の文で出す）。
    const used = new Set(names.map(([, e]) => filled(e)));
    expect(spec.filter((name) => !used.has(name))).toEqual(["クリップを書く", "パラメータを変える", "ソロ", "（プラグイン）の窓で変えた所"]);
  });

  it("出どころが spec:6 ・ mock の名前は、そこにそのまま書いてある", () => {
    expect(sectionText(specDocument(), "s6")).toContain(filled(stepNames.discardRecovered));
    expect(sectionText(specDocument(), "s6")).toContain(filled(stepNames.pluginWindow));
    expect(mockScreenText("F-Recover2")).toContain(filled(stepNames.recovered));
  });

  it("一覧にない手の仮の名前（builder。design-notes B-92 に戻してある物）", () => {
    expect(Object.fromEntries(names.filter(([, e]) => e.source.startsWith("builder:")).map(([kind, e]) => [kind, e.text]))).toEqual({
      cutNotes: "ノートを切り取る",
      pasteNotes: "ノートを貼りつける",
      unquantize: "もとの位置へ戻す",
      switchTake: "テイクを切りかえる",
      pasteClips: "クリップを貼りつける",
      removeInstrument: "音源を外す",
      removeMissing: "見つからない物を曲から外す",
      unmute: "ミュートを外す",
      addLane: "帯を足す",
      writeAutomation: "オートメーションを書く",
      restLane: "オートメーションを休ませる",
      restoreLane: "オートメーションをもどす",
      moveTimelineChange: "テンポ ・ 拍子の変更を動かす",
      setTempoRamp: "だんだんにする",
      clearTempoRamp: "だんだんをやめる",
    });
  });

  it("値をうめこむ名前", () => {
    expect(stepNameText({ kind: "setParam", effect: "compressor", param: 0 })).toBe("コンプレッサーの「おさえはじめ」を変える");
    expect(stepNameText({ kind: "setPluginParam", plugin: "Surge XT", param: "A Filter 1 Cutoff" })).toBe("Surge XT の「A Filter 1 Cutoff」を変える");
    expect(stepNameText({ kind: "setPluginParam", plugin: "Surge XT", param: null })).toBe("Surge XT のパラメータを変える");
    expect(stepNameText({ kind: "pluginWindow", plugin: "Surge XT" })).toBe("Surge XT の窓で変えた所");
    expect(stepNameText({ kind: "moveNotes" })).toBe("ノートを動かす");
  });
});

describe("履歴の行の文（モック R10-History ・ F-History）", () => {
  const ctx = scene("historyUndone3").state;
  const text = (row: HistoryRow) => [row.index, historyRowText(row, ctx), stepPlaceText(row.place, ctx)];

  it("historyUndone3（R10-History）: 16 手、3 手取り消してあり、いまここは 13 手目。行はモックの文", () => {
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
      [13, "「音量を変える」・ 4 パッド −10.5 → −12.0 dB", "ミキサー"],
      [12, "「エフェクトをとばす」・ MVerb", "4 パッド"],
      [11, "「マーカーを足す」・ サビ", "アレンジ"],
      [10, "「テンポを変える」・ 5 小節目 96 → 120", "アレンジ"],
    ]);
    const board = mockScreenText("R10-History");
    // モックの 14 手目は「1/16 ・ 12 ノート」（グリッドは、まだ手が持っていない）。ほかの行はモックの文のまま。
    for (const [index, row] of s.history.slice(9).map(text)) if (index !== 14) expect(board, String(index)).toContain(row as string);
  });

  it("historyList（F-History）: 14 手、上の 2 手はやりなおせる", () => {
    const s = scene("historyList");
    expect(s.state.history).toEqual({ total: 14, redo: 2, current: 12, newest: 14 });
    expect(s.state.status.message).toEqual({ id: "undoPending", count: 2 });
    expect(s.history.slice(8).reverse().map(text)).toEqual([
      [14, "「音量を変える」・ 4 パッド −10.5 → −12.0 dB", "ミキサー"],
      [13, "「クオンタイズ」・ 3 ノート", "ピアノロール"],
      [12, "「録音」・ テイク 2", "1 ピアノ"],
      [11, "「録音」・ テイク 1", "1 ピアノ"],
      [10, "「ループの範囲を変える」・ 5–9", "アレンジ"],
      [9, "「マーカーを足す」・ サビ", "アレンジ"],
    ]);
  });

  it("（何を）と「どこで」のほかの形", () => {
    const what = (name: HistoryRow["name"], w: HistoryRow["what"]) => stepWhatText(name, w, ctx);
    const pad = { number: 4, name: "パッド" };
    expect(what({ kind: "mute" }, { kind: "none" })).toBeNull();
    expect(historyRowText({ index: 1, step: 1, name: { kind: "recovered" }, what: { kind: "none" }, place: null }, ctx)).toBe("「なおすためのファイルから戻す」");
    expect(stepPlaceText(null, ctx)).toBeNull();
    expect(what({ kind: "setTempo" }, { kind: "tempo", bar: 1, beat: 1, fromBpm: 96, toBpm: 104.5 })).toBe("曲の頭 96 → 104.50");
    expect(what({ kind: "addTimelineChange" }, { kind: "tempo", bar: 5, beat: 3, fromBpm: null, toBpm: 120 })).toBe("5 小節目の 3 拍目 120");
    expect(what({ kind: "setMeter" }, { kind: "meter", bar: 9, from: { num: 4, den: 4 }, to: { num: 3, den: 4 } })).toBe("9 小節目 4/4 → 3/4");
    expect(what({ kind: "moveTimelineChange" }, { kind: "moved", bar: 5, beat: 1, toBar: 6, toBeat: 2 })).toBe("5 小節目 → 6 小節目の 2 拍目");
    expect(what({ kind: "setTempoRamp" }, { kind: "place", bar: 5, beat: 1 })).toBe("5 小節目");
    expect(what({ kind: "setGain" }, { kind: "gain", strip: { kind: "master" }, fromDb: 0, toDb: null })).toBe("マスター 0.0 → −∞ dB");
    expect(what({ kind: "setPan" }, { kind: "pan", strip: { kind: "track", track: pad }, from: 0, to: -20 })).toBe("4 パッド C → L 20");
    expect(what({ kind: "setSend" }, { kind: "send", track: pad, bus: "a", fromDb: -15, toDb: -12 })).toBe("4 パッド センド A −15.0 → −12.0 dB");
    expect(what({ kind: "setParam", effect: "compressor", param: 0 }, { kind: "param", from: -18, to: -24 })).toMatch(/^−18.* → −24/);
    expect(what({ kind: "replaceEffect" }, { kind: "replaced", from: { kind: "builtin", effect: "compressor" }, to: { kind: "builtin", effect: "eq4" } })).toBe("コンプレッサー → EQ 4 バンド");
    expect(what({ kind: "mute" }, { kind: "strip", strip: { kind: "return", bus: "a" } })).toBe("A リバーブ");
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

  it("場面にない行は、偽物が作る（番号と手の番号は同じ）", () => {
    expect(fakeHistoryRows([], 3, 0, 9).map((r) => [r.index, r.step])).toEqual([[1, 1], [2, 2], [3, 3]]);
    expect(fakeHistoryRows(scene("historyList").history, 14, 13, 99).map((r) => r.name.kind)).toEqual(["quantize", "setGain"]);
  });
});

describe("偽物の場面: オートメーション", () => {
  const lanesOf = (s: AppState, number: number) => s.song.tracks[number - 1]!.lanes;

  it("autoRec（F6-AutoRec）: 1 ピアノの音量の帯は「● 書く」がオンで、6.3.000 から書いている。パンとしきい値の帯はたたんである", () => {
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
      folded: [{ track: TRACK + 1, target: { kind: "pan" } }, { track: TRACK + 1, target: lanesOf(s, 1)[2]!.target }],
      armed: [volume(1)],
      writing: [{ lane: volume(1), fromTick: 5 * BAR + 2 * TPQ }],
      held: [],
      selected: null,
    } satisfies AutomationView);
    expect([s.transport.state, s.transport.position]).toEqual(["playing", 7 * BAR]);
    expect([s.song.undoCount, s.history.total]).toEqual([16, 16]);
    expect(s.status.message).toEqual({ id: "automationWriting", track: { number: 1, name: "ピアノ" }, target: { kind: "volume" } });
  });

  it("autoRest（F6-AutoRest）: フェーダーを手で −12 dB にしたので、音量の帯は休み", () => {
    const s = scene("autoRest").state;
    expect(lanesOf(s, 1).map((l) => l.resting)).toEqual([true, false, false]);
    expect(s.song.tracks[0]!.gainDb).toBe(-12);
    expect(s.automation.armed).toEqual([]);
    expect(s.status.message).toEqual({ id: "automationRested", track: { number: 1, name: "ピアノ" }, target: { kind: "volume" } });
  });

  it("mixerAuto ・ mixerRest（F6-MixerAuto ・ F6-MixerRest）: 1 ピアノと 4 パッドの音量の帯。休みは 4 パッド", async () => {
    const auto = scene("mixerAuto").state;
    expect(auto.workspace.layout.lower.map((b) => b.band)).toEqual(["mixer"]);
    expect([lanesOf(auto, 1)[0]!.laneId, lanesOf(auto, 4)[0]!.laneId]).toEqual([501, 504]);
    expect(auto.status.message).toEqual({ id: "automationMoving", tracks: [{ number: 1, name: "ピアノ" }, { number: 4, name: "パッド" }] });
    // 再生の線（9.2.000）の所で、1 ピアノの線は −2.4 dB（モックのフェーダーの数字）、4 パッドは −10.5 dB。
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

  it("autoAdd ・ autoMove ・ autoBox（R10-AutoEdit）: 足した点、動かす点、囲んだ 3 つの点がえらんである", async () => {
    const add = scene("autoAdd").state;
    expect(add.automation.selected).toEqual({ laneId: 511, points: [{ start: 3, end: 4 }] });
    const added = decodeLane(await createFakeTransport(add).lane(511));
    // 5.3.000 に −8.4 dB（モックの読み値）。
    expect([added.ticks[3], added.values[3]]).toEqual([4 * BAR + 2 * TPQ, Math.fround(dbToFrac(-8.4))]);
    expect(lanesOf(add, 1)).toHaveLength(1);
    expect(lanesOf(add, 1)[0]!.points).toBe(6);

    expect(scene("autoMove").state.automation.selected).toEqual({ laneId: 510, points: [{ start: 1, end: 2 }] });
    expect(scene("autoBox").state.automation.selected).toEqual({ laneId: 510, points: [{ start: 1, end: 4 }] });
    expect(lanesOf(scene("autoBox").state, 1)[0]!.points).toBe(5);
  });

  it("paramMany6（F6-ParamMany）: 「オートメーションに書いている」の 2 行は、たたんだ帯 2 本", () => {
    const s = scene("paramMany6").state;
    const selected = s.params.selected!;
    expect(selected.kind === "plugin" && selected.automated).toEqual([1000, 1012]);
    expect(lanesOf(s, 1).map((l) => [l.target, l.name, l.points])).toEqual([
      [{ kind: "volume" }, { kind: "volume" }, 5],
      [{ kind: "instrument", param: 1000 }, { kind: "plugin", plugin: "Surge XT", param: "A Filter 1 Cutoff" }, 3],
      [{ kind: "instrument", param: 1012 }, { kind: "plugin", plugin: "Surge XT", param: "A Osc 1 Pitch" }, 2],
    ]);
    expect(s.automation.folded.map((l) => l.target)).toEqual([{ kind: "instrument", param: 1000 }, { kind: "instrument", param: 1012 }]);
    // もとの場面（F-ParamMany）は、帯を持たないまま。
    expect(lanesOf(scene("paramMany").state, 1)).toEqual([]);
  });
});

describe("偽物の動き", () => {
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

  it("たたむ ・ 「● 書く」・ えらぶ ・ もどす ・ 帯を足す ・ 消す", async () => {
    const { t, state } = await connected("autoRest");
    const piano = TRACK + 1;
    await t.send({ type: "foldLane", track: piano, target: { kind: "volume" }, folded: true });
    expect(state().automation.folded.map((l) => l.target.kind)).toEqual(["pan", "insert", "volume"]);
    // 休みの帯は「書く」にならない。もどすと 1 手で、状態行が言う。
    await t.send({ type: "setLaneWrite", track: piano, target: { kind: "volume" }, on: true });
    expect(state().automation.armed).toEqual([]);
    await t.send({ type: "restoreLane", track: piano, target: { kind: "volume" } });
    expect(state().song.tracks[0]!.lanes[0]!.resting).toBe(false);
    expect([state().song.undoCount, state().history.total, state().history.current]).toEqual([18, 18, 18]);
    expect(state().status.message).toEqual({ id: "automationRestored", track: { number: 1, name: "ピアノ" }, target: { kind: "volume" } });
    await t.send({ type: "setLaneWrite", track: piano, target: { kind: "volume" }, on: true });
    expect(state().automation.armed).toEqual([volume(1)]);
    await t.send({ type: "selectAutoPoints", lane: 501, points: [3, 1, 2] });
    expect(state().automation.selected).toEqual({ laneId: 501, points: [{ start: 1, end: 4 }] });
    await t.send({ type: "selectAutoPoints", lane: 501, points: [] });
    expect(state().automation.selected).toBeNull();

    await t.send({ type: "addLane", track: piano, target: { kind: "mute" } });
    const mute = state().song.tracks[0]!.lanes.at(-1)!;
    expect([mute.name, mute.points, mute.step, mute.resting]).toEqual([{ kind: "mute" }, 0, true, false]);
    expect(decodeLane(await t.lane(mute.laneId)).ticks.length).toBe(0);
    // もうある帯は、ひらくだけ（手ではない）。
    const steps = state().song.undoCount;
    await t.send({ type: "addLane", track: piano, target: { kind: "pan" } });
    expect(state().automation.folded.some((l) => l.target.kind === "pan")).toBe(false);
    expect(state().song.undoCount).toBe(steps);
    await t.send({ type: "deleteLane", track: piano, target: { kind: "volume" } });
    expect(state().song.tracks[0]!.lanes.map((l) => l.name.kind)).toEqual(["pan", "builtin", "mute"]);
    expect(state().automation.armed).toEqual([]);
    // 音源がプラグインでないトラックに、音源のパラメータの帯は作れない。
    await t.send({ type: "addLane", track: piano, target: { kind: "instrument", param: 1 } });
    expect(state().song.tracks[0]!.lanes).toHaveLength(3);
  });

  it("履歴のとび: 行がいまここになり、状態行が言う。手の番号が古ければ何もしない", async () => {
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

describe("状態行の文", () => {
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

  it("仕様書 6 章の 7 つ（Mod は Ctrl）", () => {
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

  it("モック F6-MixerAuto の文と、仮の形", () => {
    expect(says(scene("mixerAuto").state.status.message!, scene("mixerAuto").state)).toBe("1 ピアノ と 4 パッド の音量は、オートメーションで動いています");
    expect(says({ id: "automationMoving", tracks: [piano] })).toBe("1 ピアノ の音量は、オートメーションで動いています");
    expect(says({ id: "automationMoving", tracks: [piano, { number: 2, name: "ベース" }, pad] })).toBe("1 ピアノ、2 ベース と 4 パッド の音量は、オートメーションで動いています");
    // （何を）のない手は、かっこを出さない。
    expect(says({ id: "undone", name: { kind: "recovered" }, what: { kind: "none" } })).toBe("「なおすためのファイルから戻す」を取り消しました。やりなおすには Ctrl Shift Z");
    expect(says({ id: "redone", name: { kind: "pluginWindow", plugin: "Surge XT" }, what: { kind: "none" } })).toBe("「Surge XT の窓で変えた所」をやりなおしました。");
    expect(says({ id: "historyJumped", index: 14, name: { kind: "quantize" }, steps: 2, forward: true })).toBe("14 手目「クオンタイズ」までやりなおしました（2 手）。");
    expect(says({ id: "automationWriting", track: piano, target: { kind: "builtin", effect: "compressor", param: 0 } })).toBe(
      "1 ピアノ ・ コンプレッサーの「おさえはじめ」を書いています（● 書く ・ 止めるとオフに戻ります）",
    );
  });
});
