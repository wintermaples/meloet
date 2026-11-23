// Protocol 27: several selected audio clips, the check before export and solo, new notices in spec 6, sound packs.
// Shapes, fake scenes, fake intents and status line text. The screen parts come in the second pass.
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

/** The state with the changed sections put in. */
function applied(state: FakeState, changes: Change[] | null): FakeState {
  expect(changes).not.toBeNull();
  const s = structuredClone(state);
  for (const c of changes!) (s as unknown as Record<string, unknown>)[c.section] = c.value;
  return s;
}
const pack = (s: FakeState, command: Command) => applied(s, reducePack(s, command));

describe("shape of protocol 27", () => {
  it("version is 27. A new song has every sound pack, does not ask, and is not fetching. No panel for several selected clips", () => {
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

  it("fetch order is B-95-1: grand piano, studio kit, those of the Y2 urgent order that are in the pack, then the shelf order. The 5 computed sounds are not fetched", () => {
    expect(FETCH_ORDER.slice(0, 6)).toEqual(["grandPiano", "studioKit", "fingerBass", "strings", "electricPiano", "organ"]);
    expect(FETCH_ORDER).toHaveLength(26);
    for (const computed of ["synthBass", "electroKit", "warmPad", "synthLead", "sine"] as const) expect(FETCH_ORDER).not.toContain(computed);
  });
});

describe("fake scenes (mock F8-*)", () => {
  it("packAsk (F8-PackAsk): the band asks, 1 piano uses a sine stand-in, 26 in the shelf not yet", () => {
    const s = scene("packAsk");
    expect(s.sounds.ask).toBe(true);
    expect(s.sounds.fetch).toEqual({ state: "idle" });
    expect(s.sounds.have).toBe(5);
    expect(s.shelf.notYet).toHaveLength(26);
    expect(s.song.tracks[0]!.standIn).toBe("packMissing");
    expect(s.status.message).toEqual({ id: "packAsking" });
  });

  it("packGet (F8-PackGet): fetching the electric piano. 38 %, 92 / 240 MB, about 2 minutes left", () => {
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

  it("packSettings (F8-PackSettings): advanced settings open, 12 / 31 so far", () => {
    const s = scene("packSettings");
    expect(s.workspace.settings?.advancedOpen).toBe(true);
    expect([s.sounds.have, s.sounds.total]).toEqual([12, 31]);
  });

  it("packNew (spec 6「音の素材 ・ 新しい包み」)", () => {
    const s = scene("packNew");
    expect(s.sounds.newPackBytes).toBe(12 * MB);
    expect(s.status.message).toEqual({ id: "newPack", bytes: 12 * MB });
  });

  it("multiClip (F8-MultiClip): 3 selected, tracks 6 vocal and 7 vocal 2. Gains differ so「—」, the rest are the same", () => {
    const s = scene("multiClip");
    const clips = s.params.clips!;
    expect(s.params.selected).toBeNull();
    expect(clips.clips).toHaveLength(3);
    expect(clips.tracks.map((id) => s.song.tracks.find((t) => t.id === id)!.name)).toEqual(["歌", "歌 2"]);
    expect(clips.gainDb).toBeNull();
    expect([clips.follow, clips.semitones, clips.cents]).not.toContain(null);
  });

  it("exportCheck (F8-ExportCheck): in the middle of counting, with 2 bass on solo", () => {
    const d = scene("exportCheck").export.dialog!;
    expect(d.check).toEqual({ state: "counting", percent: 38, peakDb: -3.5, clips: false, over: 0 });
    expect(d.solo).toEqual({ first: { number: 2, name: "ベース" }, others: 0 });
    expect(d.canExport).toBe(true);
  });

  it("exportClip (F8-ExportClip): 3 places over 0 dB, the loudest +1.2 dB. No solo", () => {
    const d = scene("exportClip").export.dialog!;
    expect(d.check).toEqual({ state: "done", peakDb: 1.2, clips: true, over: 3 });
    expect(d.solo).toBeNull();
  });
});

describe("fake intents (sound packs)", () => {
  it("fetch → the progress moves, fetched sounds play → all fetched", () => {
    let s = pack(scene("packAsk"), { type: "fetchPack" });
    expect(s.sounds.ask).toBe(false);
    expect(s.sounds.fetch).toMatchObject({ state: "fetching", instrument: "grandPiano", remainingS: null });
    expect(s.status.message).toBeNull();
    // While the first one is not fully fetched, only bytes.
    s = applied(s, advancePack(s, 1));
    expect(s.sounds.fetch).toMatchObject({ state: "fetching", instrument: "grandPiano" });
    expect(s.sounds.fetch.state === "fetching" && s.sounds.fetch.remainingS).toBeGreaterThan(0);
    // Grand piano fetched: the shelf row and the sound of 1 piano come back, and the status line says so.
    s = applied(s, advancePack(s, 5));
    expect(s.shelf.available).toContain("grandPiano");
    expect(s.shelf.notYet).not.toContain("grandPiano");
    expect(s.song.tracks[0]!.standIn).toBeNull();
    expect(s.status.message).toEqual({ id: "soundReady", instrument: "grandPiano" });
    expect(s.sounds.fetch).toMatchObject({ instrument: "studioKit" });
    // To the end.
    s = applied(s, advancePack(s, 1000));
    expect(s.sounds).toMatchObject({ fetch: { state: "idle" }, have: 31 });
    expect(s.shelf.notYet).toEqual([]);
    expect(s.status.message).toEqual({ id: "packFetched", count: 31 });
    expect(advancePack(s, 1)).toEqual([]);
  });

  it("cancel, later, again. None is an undo step", () => {
    const fetching = scene("packGet");
    const stopped = pack(fetching, { type: "stopPackFetch" });
    expect(stopped.sounds.fetch).toEqual({ state: "idle" });
    expect(reducePack(stopped, { type: "stopPackFetch" })).toEqual([]);
    // 「取りにいく」while fetching does nothing.
    expect(reducePack(fetching, { type: "fetchPack" })).toEqual([]);
    const later = pack(scene("packAsk"), { type: "packLater" });
    expect([later.sounds.ask, later.status.message]).toEqual([false, null]);
    const failed = scene("packFailCutOff");
    expect(pack(failed, { type: "packLater" }).sounds.fetch).toEqual({ state: "idle" });
    // 「続きから」: from where it got to.
    const again = pack(failed, { type: "fetchPack" });
    expect(again.sounds.fetch).toMatchObject({ state: "fetching", instrument: "studioKit" });
    for (const command of ["fetchPack", "packLater", "stopPackFetch"] as const) {
      const changes = reducePack(scene("packAsk"), { type: command }) ?? [];
      expect(changes.some((c) => c.section === "song" || c.section === "history"), command).toBe(false);
    }
    // With everything there, do not fetch. With a new pack, fetch.
    expect(reducePack(fakeState(), { type: "fetchPack" })).toEqual([]);
    expect(pack(scene("packNew"), { type: "fetchPack" }).sounds.newPackBytes).toBeNull();
  });
});

describe("fake intents (several selected audio clips)", () => {
  const s = scene("multiClip");
  const ids = s.params.clips!.clips;
  const gains = (state: FakeState) => state.song.tracks.flatMap((t) => t.clips).filter((c) => ids.includes(c.id)).map((c) => c.audio!.gainDb);

  it("line drag: all move by the same dB (1 step, stops at the range)", () => {
    const before = gains(s);
    const moved = applied(s, reduceClips27(s, { type: "shiftClipGains", clips: ids, byDb: 2 }));
    expect(gains(moved)).toEqual(before.map((g) => g + 2));
    expect(moved.song.undoCount).toBe(s.song.undoCount + 1);
    const top = applied(s, reduceClips27(s, { type: "shiftClipGains", clips: ids, byDb: 100 }));
    expect(gains(top)).toEqual([12, 12, 12]);
    expect(reduceClips27(s, { type: "shiftClipGains", clips: ids, byDb: 0 })).toEqual([]);
  });

  it("typing in the field: all take that value (1 step). A field that became the same is no longer「—」", () => {
    const set = applied(s, reduceClips27(s, { type: "setClipGains", clips: ids, db: -6 }));
    expect(gains(set)).toEqual([-6, -6, -6]);
    expect(fakeClipsPanel(set)!.gainDb).toBe(-6);
    const pitched = applied(s, reduceClips27(s, { type: "setClipsStretch", clips: ids, change: { kind: "semitones", semitones: 2.4 } }));
    expect(fakeClipsPanel(pitched)!.semitones).toBe(2);
    expect(pitched.song.undoCount).toBe(s.song.undoCount + 1);
    // Already the same value: not an undo step.
    expect(reduceClips27(set, { type: "setClipGains", clips: ids, db: -6 })).toEqual([]);
  });

  it("with only one, or with a keyboard clip mixed in, there is no field for all", () => {
    expect(fakeClipsPanel({ ...s, arrange: { ...s.arrange, selectedClips: [ids[0]!] } })).toBeNull();
    const keys = s.song.tracks[0]!.clips[0]!.id;
    expect(fakeClipsPanel({ ...s, arrange: { ...s.arrange, selectedClips: [...ids, keys] } })).toBeNull();
  });
});

describe("status line text (spec 6, version 12)", () => {
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
