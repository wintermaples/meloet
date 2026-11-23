// 約束 27（第 8 回 区切り2 ・ 3 の 1 回目）: いくつも選んだ歌のクリップ、書き出す前のたしかめとソロ、6 章の新しい知らせ、音の素材。
// 形と、偽物の場面と、偽物の意図と、状態行の文。画面の部品は 2 回目。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Change, Command, SoundsView, StatusMessage } from "../src/bridge";
import { PROTOCOL_VERSION } from "../src/bridge";
import { fakeScene, fakeState, type FakeState } from "../src/bridge/fake";
import { FETCH_ORDER, advancePack, fakeClipsPanel, reduceClips27, reducePack } from "../src/bridge/fakeRound8";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state as FakeState;
const MB = 1_000_000;

/** 変わった区画を入れた状態。 */
function applied(state: FakeState, changes: Change[] | null): FakeState {
  expect(changes).not.toBeNull();
  const s = structuredClone(state);
  for (const c of changes!) (s as unknown as Record<string, unknown>)[c.section] = c.value;
  return s;
}
const pack = (s: FakeState, command: Command) => applied(s, reducePack(s, command));

describe("約束 27 の形", () => {
  it("版は 27。新しい曲では、音の素材はぜんぶあり、聞かず、取っていない。いくつも選んだクリップの欄もない", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(27);
    const s = fakeState();
    expect(s.sounds).toEqual({
      ask: false,
      fetch: { state: "idle" },
      have: 31,
      total: 31,
      packBytes: 240 * MB,
      newPackBytes: null,
      place: "~/.local/share/meloet/sounds",
    } satisfies SoundsView);
    expect(s.shelf.notYet).toEqual([]);
    expect(s.params.clips).toBeNull();
  });

  it("取る順は B-95-1: グランドピアノ、スタジオキット、Y2 の急ぐ順のうち包みにある物、そのあと棚の並び。計算で作る 5 つは取らない", () => {
    expect(FETCH_ORDER.slice(0, 6)).toEqual(["grandPiano", "studioKit", "fingerBass", "strings", "electricPiano", "organ"]);
    expect(FETCH_ORDER).toHaveLength(26);
    for (const computed of ["synthBass", "electroKit", "warmPad", "synthLead", "sine"] as const) expect(FETCH_ORDER).not.toContain(computed);
  });
});

describe("偽物の場面（モック F8-*）", () => {
  it("packAsk（F8-PackAsk）: 帯が聞き、1 ピアノ はサイン波で代わり、棚の 26 はまだ", () => {
    const s = scene("packAsk");
    expect(s.sounds.ask).toBe(true);
    expect(s.sounds.fetch).toEqual({ state: "idle" });
    expect(s.sounds.have).toBe(5);
    expect(s.shelf.notYet).toHaveLength(26);
    expect(s.song.tracks[0]!.standIn).toBe("packMissing");
    expect(s.status.message).toEqual({ id: "packAsking" });
  });

  it("packGet（F8-PackGet）: エレクトリックピアノを取っている。38 % ・ 92 / 240 MB ・ のこり 約 2 分", () => {
    const s = scene("packGet");
    expect(s.sounds.fetch).toEqual({ state: "fetching", instrument: "electricPiano", doneBytes: 92 * MB, totalBytes: 240 * MB, remainingS: 120 });
    expect(s.shelf.available).toContain("grandPiano");
    expect(s.shelf.notYet).toContain("electricPiano");
    expect(s.song.tracks[0]!.standIn).toBeNull();
    expect(s.status.message).toEqual({ id: "soundReady", instrument: "grandPiano" });
  });

  it.each([
    ["packFailOffline", { kind: "offline" }],
    ["packFailCutOff", { kind: "cutOff" }],
    ["packFailDiskFull", { kind: "diskFull", neededBytes: 160 * MB, freeBytes: 10 * MB }],
    ["packFailCorrupt", { kind: "corrupt" }],
  ] as const)("%s（F8-PackFail ・ 6 章）: 帯と状態行が同じわけを言う", (name, failure) => {
    const s = scene(name);
    expect(s.sounds.fetch).toEqual({ state: "failed", failure, doneBytes: 92 * MB, totalBytes: 240 * MB });
    expect(s.status.message).toEqual({ id: "packFailed", failure, doneBytes: 92 * MB, totalBytes: 240 * MB });
  });

  it("packSettings（F8-PackSettings）: くわしい設定がひらき、まだ 12 / 31", () => {
    const s = scene("packSettings");
    expect(s.workspace.settings?.advancedOpen).toBe(true);
    expect([s.sounds.have, s.sounds.total]).toEqual([12, 31]);
  });

  it("packNew（6 章「音の素材 ・ 新しい包み」）", () => {
    const s = scene("packNew");
    expect(s.sounds.newPackBytes).toBe(12 * MB);
    expect(s.status.message).toEqual({ id: "newPack", bytes: 12 * MB });
  });

  it("multiClip（F8-MultiClip）: 3 つ選び、トラックは 6 歌 と 7 歌 2。音量はちがうので「—」、ほかは同じ", () => {
    const s = scene("multiClip");
    const clips = s.params.clips!;
    expect(s.params.selected).toBeNull();
    expect(clips.clips).toHaveLength(3);
    expect(clips.tracks.map((id) => s.song.tracks.find((t) => t.id === id)!.name)).toEqual(["歌", "歌 2"]);
    expect(clips.gainDb).toBeNull();
    expect([clips.follow, clips.semitones, clips.cents]).not.toContain(null);
  });

  it("exportCheck（F8-ExportCheck）: 数えている途中で、2 ベース をソロにしている", () => {
    const d = scene("exportCheck").export.dialog!;
    expect(d.check).toEqual({ state: "counting", percent: 38, peakDb: -3.5, clips: false, over: 0 });
    expect(d.solo).toEqual({ first: { number: 2, name: "ベース" }, others: 0 });
    expect(d.canExport).toBe(true);
  });

  it("exportClip（F8-ExportClip）: 0 dB をこえる所が 3 か所、いちばん大きい所 +1.2 dB。ソロはない", () => {
    const d = scene("exportClip").export.dialog!;
    expect(d.check).toEqual({ state: "done", peakDb: 1.2, clips: true, over: 3 });
    expect(d.solo).toBeNull();
  });
});

describe("偽物の意図（音の素材）", () => {
  it("取りにいく → 進みが動き、取れた音色から鳴る → ぜんぶ取れた", () => {
    let s = pack(scene("packAsk"), { type: "fetchPack" });
    expect(s.sounds.ask).toBe(false);
    expect(s.sounds.fetch).toMatchObject({ state: "fetching", instrument: "grandPiano", remainingS: null });
    expect(s.status.message).toBeNull();
    // まだ 1 つ目が取れきらない間は、バイトだけ。
    s = applied(s, advancePack(s, 1));
    expect(s.sounds.fetch).toMatchObject({ state: "fetching", instrument: "grandPiano" });
    expect(s.sounds.fetch.state === "fetching" && s.sounds.fetch.remainingS).toBeGreaterThan(0);
    // グランドピアノが取れた: 棚の行と 1 ピアノ の音が戻り、状態行が言う。
    s = applied(s, advancePack(s, 5));
    expect(s.shelf.available).toContain("grandPiano");
    expect(s.shelf.notYet).not.toContain("grandPiano");
    expect(s.song.tracks[0]!.standIn).toBeNull();
    expect(s.status.message).toEqual({ id: "soundReady", instrument: "grandPiano" });
    expect(s.sounds.fetch).toMatchObject({ instrument: "studioKit" });
    // 最後まで。
    s = applied(s, advancePack(s, 1000));
    expect(s.sounds).toMatchObject({ fetch: { state: "idle" }, have: 31 });
    expect(s.shelf.notYet).toEqual([]);
    expect(s.status.message).toEqual({ id: "packFetched", count: 31 });
    expect(advancePack(s, 1)).toEqual([]);
  });

  it("やめる ・ あとで ・ もう一度。どれも手ではない", () => {
    const fetching = scene("packGet");
    const stopped = pack(fetching, { type: "stopPackFetch" });
    expect(stopped.sounds.fetch).toEqual({ state: "idle" });
    expect(reducePack(stopped, { type: "stopPackFetch" })).toEqual([]);
    // 取っている間の「取りにいく」は何もしない。
    expect(reducePack(fetching, { type: "fetchPack" })).toEqual([]);
    const later = pack(scene("packAsk"), { type: "packLater" });
    expect([later.sounds.ask, later.status.message]).toEqual([false, null]);
    const failed = scene("packFailCutOff");
    expect(pack(failed, { type: "packLater" }).sounds.fetch).toEqual({ state: "idle" });
    // 「続きから」: 取ってあった所から。
    const again = pack(failed, { type: "fetchPack" });
    expect(again.sounds.fetch).toMatchObject({ state: "fetching", instrument: "studioKit" });
    for (const command of ["fetchPack", "packLater", "stopPackFetch"] as const) {
      const changes = reducePack(scene("packAsk"), { type: command }) ?? [];
      expect(changes.some((c) => c.section === "song" || c.section === "history"), command).toBe(false);
    }
    // ぜんぶあれば、取りにいかない。新しい包みがあれば取る。
    expect(reducePack(fakeState(), { type: "fetchPack" })).toEqual([]);
    expect(pack(scene("packNew"), { type: "fetchPack" }).sounds.newPackBytes).toBeNull();
  });
});

describe("偽物の意図（いくつも選んだ歌のクリップ）", () => {
  const s = scene("multiClip");
  const ids = s.params.clips!.clips;
  const gains = (state: FakeState) => state.song.tracks.flatMap((t) => t.clips).filter((c) => ids.includes(c.id)).map((c) => c.audio!.gainDb);

  it("線のドラッグ: ぜんぶが同じ dB だけ動く（1 手、範囲で止まる）", () => {
    const before = gains(s);
    const moved = applied(s, reduceClips27(s, { type: "shiftClipGains", clips: ids, byDb: 2 }));
    expect(gains(moved)).toEqual(before.map((g) => g + 2));
    expect(moved.song.undoCount).toBe(s.song.undoCount + 1);
    const top = applied(s, reduceClips27(s, { type: "shiftClipGains", clips: ids, byDb: 100 }));
    expect(gains(top)).toEqual([12, 12, 12]);
    expect(reduceClips27(s, { type: "shiftClipGains", clips: ids, byDb: 0 })).toEqual([]);
  });

  it("欄に打つ: ぜんぶがその値（1 手）。同じになった欄は「—」でなくなる", () => {
    const set = applied(s, reduceClips27(s, { type: "setClipGains", clips: ids, db: -6 }));
    expect(gains(set)).toEqual([-6, -6, -6]);
    expect(fakeClipsPanel(set)!.gainDb).toBe(-6);
    const pitched = applied(s, reduceClips27(s, { type: "setClipsStretch", clips: ids, change: { kind: "semitones", semitones: 2.4 } }));
    expect(fakeClipsPanel(pitched)!.semitones).toBe(2);
    expect(pitched.song.undoCount).toBe(s.song.undoCount + 1);
    // もう同じ値なら手にならない。
    expect(reduceClips27(set, { type: "setClipGains", clips: ids, db: -6 })).toEqual([]);
  });

  it("1 つだけ、または鍵盤のクリップがまざると、まとめての欄はない", () => {
    expect(fakeClipsPanel({ ...s, arrange: { ...s.arrange, selectedClips: [ids[0]!] } })).toBeNull();
    const keys = s.song.tracks[0]!.clips[0]!.id;
    expect(fakeClipsPanel({ ...s, arrange: { ...s.arrange, selectedClips: [...ids, keys] } })).toBeNull();
  });
});

describe("状態行の文（6 章 第 12 版）", () => {
  let target: HTMLElement;
  let mounted: Record<string, unknown> | null = null;
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
  });
  function line(message: StatusMessage): string {
    const s: AppState = { ...fakeState(), status: { message } };
    app.setState(s);
    mounted = mount(StatusLine, { target });
    flushSync();
    const text = target.querySelector(".message")?.textContent?.replace(/\s+/g, " ").trim() ?? "";
    unmount(mounted);
    mounted = null;
    return text;
  }

  it.each<[StatusMessage, string]>([
    [{ id: "inputLost", input: "mono1", device: "USB マイク" }, "入口 1（USB マイク）が抜けました。録音を止めました。録れた所までは残しています。"],
    [
      { id: "recordingGaps", count: 3 },
      "録っている間に音が 3 回とぎれました（PC がいそがしかったため）。とぎれた所は無音で残しています。バッファを大きくすると、とぎれにくくなります。",
    ],
    [{ id: "audioFound", file: "歌 テイク 1.wav", clips: 2 }, "歌 テイク 1.wav を見つけました。2 つのクリップがまた鳴ります。"],
    [{ id: "audioFoundMismatch" }, "えらんだファイルは使えません（サンプルレートかチャンネルの数がちがいます）。"],
    [{ id: "audioBroken", file: "声.wav" }, "「声.wav」はこわれているので読めません。"],
    [{ id: "audioTooManyChannels", file: "8ch.wav" }, "「8ch.wav」は 3 チャンネル以上なので読めません（読めるのはモノとステレオ）。"],
    [{ id: "savingAudio", done: 3, total: 12, doneBytes: 80 * MB, totalBytes: 240 * MB }, "保存しています ・ 音のファイルを写しています 3 / 12（80 / 240 MB）"],
    [{ id: "diskTooSlow" }, "音のファイルを読むのが間に合わず、歌が少し切れました（ディスクがいそがしかったため）。"],
    [{ id: "sourceTempoFound", name: "ドラム", bpm: 120, bars: 2, same: true }, "この音のテンポは 120.00 くらいです（いまの数と同じです）。"],
    [{ id: "sourceTempoNotFound", name: "ドラム" }, "この音のテンポは見つかりませんでした。数を打ちこんでください。"],
    [{ id: "packAsking" }, "音の素材がまだありません ・ いまはサイン波で鳴ります"],
    [{ id: "soundReady", instrument: "grandPiano" }, "グランドピアノ が鳴るようになりました ・ のこりの音色を取っています"],
    [{ id: "packFetched", count: 31 }, "音の素材をぜんぶ取りました。31 の音色が鳴ります。"],
    [
      { id: "packFailed", failure: { kind: "offline" }, doneBytes: 92 * MB, totalBytes: 240 * MB },
      "音の素材を取りにいけませんでした（インターネットにつながっていません）。つながったら「もう一度」を押してください。",
    ],
    [{ id: "packFailed", failure: { kind: "cutOff" }, doneBytes: 92 * MB, totalBytes: 240 * MB }, "とちゅうで切れました。92 / 240 MB まで取ってあります。続きから取ります。"],
    [
      { id: "packFailed", failure: { kind: "diskFull", neededBytes: 160 * MB, freeBytes: 10 * MB }, doneBytes: 92 * MB, totalBytes: 240 * MB },
      "ディスクがいっぱいで、音の素材を置けません（あと 150 MB 要ります）。",
    ],
    [{ id: "packFailed", failure: { kind: "corrupt" }, doneBytes: 92 * MB, totalBytes: 240 * MB }, "取った音の素材がこわれていました。取りなおしても直りませんでした。"],
    [{ id: "newPack", bytes: 12 * MB }, "音の素材の新しい包みがあります（12 MB）。"],
  ])("%j", (message, want) => {
    expect(line(message)).toBe(want);
  });
});
