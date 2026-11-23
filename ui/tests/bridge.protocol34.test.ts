// Protocol 34 (round 12, the owner's answers of 2026-11-24): mute sets on the screen (B-97-3, B-97-5), their step names
// (B-97-2) and the step name of a batch of the AI link (B-97-1). The fake's copy of the four intents, by Rust's rules.
import { afterEach, describe, expect, it } from "vitest";
import type { AppState, Command, HistoryRow, MuteSetView } from "../src/bridge";
import { MUTE_SET_NAME_MAX, MUTE_SETS_MAX, PROTOCOL_VERSION, STEP_NAME_KINDS } from "../src/bridge";
import { createFakeTransport, fakeHistoryRows, fakeScene, fakeState, type FakeState } from "../src/bridge/fake";
import { historyRowText, stepNameText, stepPlaceText } from "../src/components/history/stepText";
import { useLanguage } from "../src/text";
import { stepNames } from "../src/text/ja";

afterEach(() => useLanguage("ja"));

async function opened(state: FakeState = fakeScene("mixerMuteSets")!.state) {
  const t = createFakeTransport(state);
  await t.subscribe(
    () => {},
    () => {},
  );
  const now = async () => (await t.snapshot()).state as AppState;
  const start = (await now()).song.undoCount;
  /** Steps made since it opened. */
  const steps = (s: AppState) => s.song.undoCount - start;
  const rows = async () => {
    const s = await now();
    return t.historyRows(1, s.history.total);
  };
  return { now, rows, steps, send: async (command: Command) => (await t.send(command), now()) };
}

const sets = (s: AppState): [string, boolean][] => s.song.muteSets.map((m: MuteSetView) => [m.name, m.asNow]);
const muted = (s: AppState) => s.song.tracks.map((t) => t.muted);
const soloed = (s: AppState) => s.song.tracks.map((t) => t.soloed);

describe("shape of protocol 34", () => {
  it("version 34; a new song has no mute sets; the numbers of Rust", () => {
    expect(PROTOCOL_VERSION).toBe(34);
    expect(fakeState().song.muteSets).toEqual([]);
    expect([MUTE_SETS_MAX, MUTE_SET_NAME_MAX]).toEqual([64, 64]);
    const commands: Command[] = [
      { type: "saveMuteSet", name: "サビ", overwrite: false },
      { type: "recallMuteSet", name: "サビ" },
      { type: "deleteMuteSet", name: "サビ" },
      { type: "renameMuteSet", name: "サビ", to: "大サビ" },
    ];
    expect(commands.map((c) => c.type)).toEqual(["saveMuteSet", "recallMuteSet", "deleteMuteSet", "renameMuteSet"]);
  });

  it("five more step names, each with its text, in Japanese and in English", () => {
    expect(STEP_NAME_KINDS).toEqual(expect.arrayContaining(["saveMuteSet", "recallMuteSet", "deleteMuteSet", "renameMuteSet", "batch"]));
    expect(stepNames.batch).toMatchObject({ text: "まとめて変える（{count} 個）", source: "user:2026-11-24" });
    const names: HistoryRow["name"][] = [
      { kind: "saveMuteSet" },
      { kind: "recallMuteSet" },
      { kind: "deleteMuteSet" },
      { kind: "renameMuteSet" },
      { kind: "batch", count: 15 },
    ];
    expect(names.map(stepNameText)).toEqual([
      "ミュートのセットを保存する",
      "ミュートのセットを呼び出す",
      "ミュートのセットを消す",
      "ミュートのセットの名前を変える",
      "まとめて変える（15 個）",
    ]);
    useLanguage("en");
    expect(names.map(stepNameText)).toEqual(["Save a mute set", "Recall a mute set", "Delete a mute set", "Rename a mute set", "Change together (15)"]);
  });

  it("historyMuteSets: the rows say the set's name (and both names for a rename) at 「ミキサー」; a batch has no place", () => {
    const s = fakeScene("historyMuteSets")!;
    const ctx = s.state;
    const rows = fakeHistoryRows(s.history, s.state.history.total, 9, 13);
    expect(rows.map((row) => [historyRowText(row, ctx), stepPlaceText(row.place, ctx)])).toEqual([
      ["「ミュートのセットを保存する」・ サビ", "ミキサー"],
      ["「ミュートのセットを呼び出す」・ サビ", "ミキサー"],
      ["「ミュートのセットの名前を変える」・ サビ → 大サビ", "ミキサー"],
      ["「ミュートのセットを消す」・ ドラムだけ", "ミキサー"],
      ["「まとめて変える（15 個）」", null],
    ]);
  });
});

describe("the fake's mute sets", () => {
  it("mixerMuteSets: three sets in order; 「シンセなし」 is as the song is now; mixerMuteSetsFull holds as many as a song can", () => {
    const s = fakeScene("mixerMuteSets")!.state;
    expect(s.workspace.layout.lower.map((b) => b.band)).toEqual(["mixer"]);
    expect(sets(s)).toEqual([
      ["ドラムとベースだけ", false],
      ["シンセなし", true],
      ["ピアノだけ", false],
    ]);
    expect(fakeScene("mixerMuteSetsFull")!.state.song.muteSets).toHaveLength(MUTE_SETS_MAX);
  });

  it("recall: the mutes as one step, the solo with no step; as it is already: nothing", async () => {
    const app = await opened();
    let s = await app.send({ type: "recallMuteSet", name: "ドラムとベースだけ" });
    expect(muted(s)).toEqual([true, false, false, true, true]);
    expect(app.steps(s)).toBe(1);
    expect(sets(s).map(([, now]) => now)).toEqual([true, false, false]);
    expect((await app.rows()).at(-1)).toMatchObject({ name: { kind: "recallMuteSet" }, what: { kind: "name", name: "ドラムとベースだけ" }, place: { kind: "mixer" } });

    s = await app.send({ type: "recallMuteSet", name: "ドラムとベースだけ" });
    expect(app.steps(s)).toBe(1);
    // 「ピアノだけ」 solos the piano and unmutes everything: one step.
    s = await app.send({ type: "recallMuteSet", name: "ピアノだけ" });
    expect([muted(s), soloed(s)]).toEqual([
      [false, false, false, false, false],
      [true, false, false, false, false],
    ]);
    expect(app.steps(s)).toBe(2);
    // Only the solo differs: no step.
    s = await app.send({ type: "toggleSolo", track: s.song.tracks[0]!.id });
    expect(sets(s).map(([, now]) => now)).toEqual([false, false, false]);
    s = await app.send({ type: "recallMuteSet", name: "ピアノだけ" });
    expect([app.steps(s), soloed(s)[0], sets(s)[2]![1]]).toEqual([2, true, true]);
  });

  it("save, rename and delete: one step each; what does not fit does nothing", async () => {
    const app = await opened();
    const piano = (await app.now()).song.tracks[0]!.id;
    let s = await app.send({ type: "toggleMute", track: piano });
    expect(sets(s)[1]).toEqual(["シンセなし", false]);
    s = await app.send({ type: "saveMuteSet", name: "  サビ ", overwrite: false });
    expect(sets(s).at(-1)).toEqual(["サビ", true]);
    expect(app.steps(s)).toBe(1);
    expect((await app.rows()).at(-1)).toMatchObject({ name: { kind: "saveMuteSet" }, what: { kind: "name", name: "サビ" } });

    // A name taken, no name, too long a name, the same set again: nothing.
    for (const command of [
      { type: "saveMuteSet", name: "シンセなし", overwrite: false },
      { type: "saveMuteSet", name: " ", overwrite: false },
      { type: "saveMuteSet", name: "あ".repeat(MUTE_SET_NAME_MAX + 1), overwrite: false },
      { type: "saveMuteSet", name: "サビ", overwrite: true },
    ] satisfies Command[]) {
      s = await app.send(command);
      expect(app.steps(s), JSON.stringify(command)).toBe(1);
    }
    // Written over, in its place.
    s = await app.send({ type: "saveMuteSet", name: "シンセなし", overwrite: true });
    expect([app.steps(s), sets(s)[1]]).toEqual([2, ["シンセなし", true]]);

    s = await app.send({ type: "renameMuteSet", name: "サビ", to: "大サビ" });
    expect(sets(s).map(([name]) => name)).toEqual(["ドラムとベースだけ", "シンセなし", "ピアノだけ", "大サビ"]);
    expect((await app.rows()).at(-1)).toMatchObject({ name: { kind: "renameMuteSet" }, what: { kind: "renamed", from: "サビ", to: "大サビ" } });
    for (const command of [
      { type: "renameMuteSet", name: "サビ", to: "x" },
      { type: "renameMuteSet", name: "大サビ", to: "ピアノだけ" },
      { type: "renameMuteSet", name: "大サビ", to: " 大サビ " },
      { type: "renameMuteSet", name: "大サビ", to: "" },
      { type: "deleteMuteSet", name: "サビ" },
      { type: "recallMuteSet", name: "サビ" },
    ] satisfies Command[]) {
      s = await app.send(command);
      expect(app.steps(s), JSON.stringify(command)).toBe(3);
    }

    s = await app.send({ type: "deleteMuteSet", name: "ドラムとベースだけ" });
    expect(sets(s).map(([name]) => name)).toEqual(["シンセなし", "ピアノだけ", "大サビ"]);
    expect((await app.rows()).at(-1)).toMatchObject({ name: { kind: "deleteMuteSet" }, what: { kind: "name", name: "ドラムとベースだけ" } });
    expect(s.history.total).toBe(s.song.undoCount);
    expect(app.steps(s)).toBe(4);
  });

  it("no new set past MUTE_SETS_MAX, and nothing while recording", async () => {
    const full = await opened(fakeScene("mixerMuteSetsFull")!.state);
    let s = await full.send({ type: "saveMuteSet", name: "もう一つ", overwrite: false });
    expect([s.song.muteSets.length, full.steps(s)]).toEqual([MUTE_SETS_MAX, 0]);

    const state = fakeScene("mixerMuteSets")!.state;
    state.transport.state = "recording";
    const app = await opened(state);
    for (const command of [
      { type: "saveMuteSet", name: "新しい", overwrite: false },
      { type: "recallMuteSet", name: "ピアノだけ" },
      { type: "renameMuteSet", name: "ピアノだけ", to: "x" },
      { type: "deleteMuteSet", name: "ピアノだけ" },
    ] satisfies Command[]) {
      s = await app.send(command);
      expect([sets(s).length, app.steps(s), soloed(s)[0]], JSON.stringify(command)).toEqual([3, 0, false]);
    }
  });
});
