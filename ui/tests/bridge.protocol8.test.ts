import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import type { AppState, Change, Command, ServerMessage } from "../src/bridge/types";
import { t } from "../src/text/ja";

const BAR = 1920;

/** The fake behind a scene, and the state it holds after the commands, built from the patches. */
async function opened(name: string) {
  const scene = fakeScene(name)!;
  const transport = createFakeTransport(structuredClone(scene.state));
  let state: AppState = structuredClone(scene.state);
  const got: Change[] = [];
  await transport.subscribe(
    (m: ServerMessage) => {
      if (m.kind !== "patch") return;
      got.push(...m.changes);
      state = { ...state };
      for (const c of m.changes) (state as Record<string, unknown>)[c.section] = c.value;
    },
    () => {},
  );
  const send = async (...commands: Command[]) => {
    for (const c of commands) await transport.send(c);
    return state;
  };
  return { scene, send, got, now: () => state };
}

const clipsOf = (s: AppState) => s.song.tracks.map((t) => t.clips.map((c) => [c.id, c.start / BAR + 1]));

describe("約束 8 の偽物の場面", () => {
  it("トラック 3 本: ベースが選んだトラックでミュート、ドラムがソロ、2 本のトラックにまたがるクリップ 2 つを選んでいる", () => {
    const s = fakeScene("arrangeSelect")!.state;
    expect(s.song.tracks.map((t) => [t.name, t.muted, t.soloed])).toEqual([
      ["ピアノ", false, false],
      ["ベース", true, false],
      ["ドラム", false, true],
    ]);
    expect(s.song.playTarget).toBe(2);
    expect(s.arrange).toEqual({ selectedClips: [3, 41], selectedClip: 41, pianoRollClip: 41, clipboardClips: 1 });
  });

  it("ベロシティ固定がオン（100）", () => {
    expect(fakeScene("velocityFixed")!.state.keyboards.velocityFixed).toEqual({ on: true, value: 100 });
  });

  it("丸めた文と、そろえた文が状態行に出る（文はヤチヨさんの決めのまま）", () => {
    const fill = (m: AppState["status"]["message"]) => {
      if (m?.id === "valueClamped") return t("msg.valueClamped", { field: t(m.field === "velocity" ? "value.velocity" : "value.meterDen"), range: t(m.field === "velocity" ? "value.velocityRange" : "value.meterDenRange"), value: m.value });
      if (m?.id === "velocityMatched") return t("msg.velocityMatched", { count: m.count, value: m.value });
      return null;
    };
    expect(fill(fakeScene("valueClamped")!.state.status.message)).toBe("ベロシティは 1〜127 なので、127 にしました。");
    expect(fill(fakeScene("meterClamped")!.state.status.message)).toBe("拍子の下は 2 ・ 4 ・ 8 ・ 16 なので、4 にしました。");
    expect(fill(fakeScene("velocityMatched")!.state.status.message)).toBe("3 ノートのベロシティを 100 にそろえました。");
  });
});

describe("偽物の Rust: 約束 8 の意図", () => {
  it("トラックを選ぶ、ミュートとソロを切りかえる", async () => {
    const f = await opened("arrangeSelect");
    let s = await f.send({ type: "selectTrack", track: 3 }, { type: "toggleMute", track: 2 }, { type: "toggleSolo", track: 1 });
    expect(s.song.playTarget).toBe(3);
    expect(s.song.tracks.map((t) => [t.muted, t.soloed])).toEqual([
      [false, true],
      [false, false],
      [false, true],
    ]);
    s = await f.send({ type: "selectTrack", track: 9 });
    expect(s.song.playTarget).toBe(3);
  });

  it("クリップを押すと、そのトラックも選ぶ。Shift で足し、囲みで入れかえ、Mod A でぜんぶ", async () => {
    const f = await opened("arrangeSelect");
    let s = await f.send({ type: "selectClip", clip: 1 });
    expect([s.arrange.selectedClips, s.arrange.selectedClip, s.song.playTarget]).toEqual([[1], 1, 1]);
    s = await f.send({ type: "selectClips", clips: [43], mode: "add" });
    expect([s.arrange.selectedClips, s.arrange.selectedClip, s.arrange.pianoRollClip, s.song.playTarget]).toEqual([[1, 43], 43, 43, 3]);
    s = await f.send({ type: "selectClips", clips: [3, 41], mode: "replace" });
    expect([s.arrange.selectedClips, s.arrange.selectedClip, s.arrange.pianoRollClip, s.song.playTarget]).toEqual([[3, 41], null, 43, 3]);
    s = await f.send({ type: "selectAllClips" });
    expect(s.arrange.selectedClips).toEqual([1, 3, 41, 43]);
    s = await f.send({ type: "selectClip", clip: null });
    expect([s.arrange.selectedClips, s.arrange.selectedClip]).toEqual([[], null]);
  });

  it("まとめて動かす（トラックの行の差で）。はみだす・重なるときは何もしない", async () => {
    const f = await opened("arrangeSelect");
    // ピアノの「Aメロ」（5–9）とベースの「ベース A」（5–9）を 1 行下、4 小節うしろへ。
    let s = await f.send({ type: "moveClips", clips: [3, 41], ticks: 4 * BAR, tracks: 1, copy: false });
    expect(clipsOf(s)).toEqual([[[1, 1]], [[3, 9]], [[43, 1], [41, 9]]]);
    expect([s.arrange.selectedClips, s.arrange.selectedClip]).toEqual([[3, 41], 41]);
    const before = clipsOf(s);
    s = await f.send({ type: "moveClips", clips: [3, 41], ticks: 0, tracks: 2, copy: false });
    expect(clipsOf(s)).toEqual(before);
    s = await f.send({ type: "moveClips", clips: [3], ticks: -8 * BAR, tracks: 1, copy: false });
    expect(clipsOf(s)).toEqual(before);
  });

  it("まとめて複製はまとまりのすぐうしろ。写しを選ぶ", async () => {
    const f = await opened("arrangeSelect");
    const s = await f.send({ type: "duplicateClips", clips: [3, 41] });
    expect(clipsOf(s)[0]!.map((c) => c[1])).toEqual([1, 5, 9]);
    expect(clipsOf(s)[1]!.map((c) => c[1])).toEqual([5, 9]);
    expect(s.arrange.selectedClips).toHaveLength(2);
    expect(s.arrange.selectedClips).not.toContain(3);
  });

  it("コピー・切り取り・貼りつけ（Mod V は選んだトラックの再生の位置の小節の頭、右クリックは押した所）", async () => {
    const f = await opened("arrangeSelect");
    let s = await f.send({ type: "copyClips", clips: [3, 41] });
    expect(s.arrange.clipboardClips).toBe(2);
    // 選んだトラックはベース（2）、再生の位置は 5 小節め: 重なる。
    s = await f.send({ type: "pasteClips", to: null });
    expect(clipsOf(s)[1]).toEqual([[41, 5]]);
    s = await f.send({ type: "locate", tick: 12 * BAR + 300 }, { type: "selectTrack", track: 1 }, { type: "pasteClips", to: null });
    expect(clipsOf(s)[0]!.map((c) => c[1])).toEqual([1, 5, 13]);
    expect(clipsOf(s)[1]!.map((c) => c[1])).toEqual([5, 13]);
    s = await f.send({ type: "pasteClips", to: { track: 1, tick: 20 * BAR + 10 } });
    expect(clipsOf(s)[0]!.at(-1)![1]).toBe(21 + 10 / BAR);
    // 3 本目の下にはみだす所へは貼らない。
    const before = clipsOf(s);
    s = await f.send({ type: "pasteClips", to: { track: 3, tick: 40 * BAR } });
    expect(clipsOf(s)).toEqual(before);
    s = await f.send({ type: "cutClips", clips: [1] });
    expect([s.arrange.clipboardClips, clipsOf(s)[0]![0]![0]]).toEqual([1, 3]);
  });

  it("Delete は選んだクリップぜんぶ", async () => {
    const f = await opened("arrangeSelect");
    const s = await f.send({ type: "deleteSelected" });
    expect(clipsOf(s)).toEqual([[[1, 1]], [], [[43, 1]]]);
    expect(s.arrange.selectedClips).toEqual([]);
  });

  it("ベロシティ固定、丸めた値、そろえた文", async () => {
    const f = await opened("pianoRoll");
    let s = await f.send({ type: "setVelocityFixed", on: true, value: 200 });
    expect(s.keyboards.velocityFixed).toEqual({ on: true, value: 127 });
    s = await f.send({ type: "matchVelocity", take: s.pianoRoll.take!, notes: [14, 15, 16], value: 100 });
    expect(s.status.message).toEqual({ id: "velocityMatched", count: 3, value: 100 });
    s = await f.send({ type: "clampValue", field: "velocity", value: "127", range: null });
    expect(s.status.message).toEqual({ id: "valueClamped", field: "velocity", value: "127", range: null });
    s = await f.send({ type: "selectAllNotes" });
    expect(s.status.message?.id).toBe("summary");
  });

  it("再生の位置は押した所そのまま", async () => {
    const f = await opened("arrangeSelect");
    const s = await f.send({ type: "locate", tick: 4 * BAR + 123 });
    expect(s.transport.position).toBe(4 * BAR + 123);
  });
});
