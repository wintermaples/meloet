import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import type { AppState, Change, Command, ServerMessage } from "../src/bridge/types";
import { t } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const BAR = 4 * TPQ;

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

describe("protocol 8 fake scenes", () => {
  it("3 tracks: the bass is the chosen track and muted, the drums solo, 2 clips across 2 tracks are selected", () => {
    const s = fakeScene("arrangeSelect")!.state;
    expect(s.song.tracks.map((t) => [t.name, t.muted, t.soloed])).toEqual([
      ["ピアノ", false, false],
      ["ベース", true, false],
      ["ドラム", false, true],
    ]);
    expect(s.song.playTarget).toBe(2);
    expect(s.arrange).toEqual({ selectedClips: [3, 41], selectedClip: 41, pianoRollClip: 41, clipboardClips: 1, tallTrack: null });
  });

  it("fixed velocity is on (100)", () => {
    expect(fakeScene("velocityFixed")!.state.keyboards.velocityFixed).toEqual({ on: true, value: 100 });
  });

  it("the rounded text and the aligned text show in the status line (wording as the owner chose)", () => {
    const fill = (m: AppState["status"]["message"]) => {
      if (m?.id === "valueClamped") return t("msg.valueClamped", { field: t(m.field === "velocity" ? "value.velocity" : "value.meterDen"), range: t(m.field === "velocity" ? "value.velocityRange" : "value.meterDenRange"), value: m.value });
      if (m?.id === "velocityMatched") return t("msg.velocityMatched", { count: m.count, value: m.value });
      return null;
    };
    expect(fill(fakeScene("valueClamped")!.state.status.message)).toBe("ベロシティは 1〜127 なので、127 にしました。");
    expect(fill(fakeScene("meterClamped")!.state.status.message)).toBe("拍子の下は 2 ・ 4 ・ 8 ・ 16 なので、4 にしました。");
    expect(fill(fakeScene("velocityMatched")!.state.status.message)).toBe("3 ノートのベロシティを 100 にそろえました。");
  });

  it("text for rounding bar / beat / tick positions (only for bars, no space after 「1 から」)", () => {
    const pos = fakeScene("positionClamped")!.state.status.message;
    expect(pos).toEqual({ id: "valueClamped", field: "beat", value: "3. 4. 000", range: "1〜4" });
    if (pos?.id !== "valueClamped") throw new Error();
    expect(t("msg.valueClamped", { field: t("value.beat"), range: pos.range!, value: pos.value })).toBe("拍は 1〜4 なので、3. 4. 000 にしました。");
    expect(t("msg.valueClamped", { field: t("value.tick"), range: "0〜479", value: "3. 4. 479" })).toBe("ティックは 0〜479 なので、3. 4. 479 にしました。");
    const bar = fakeScene("barClamped")!.state.status.message;
    if (bar?.id !== "valueClamped") throw new Error();
    expect(t("msg.valueClampedBar", { value: bar.value })).toBe("小節は 1 からなので、1. 1. 000 にしました。");
  });

  it("text when overlapping clips could not be placed (wording as the owner chose)", () => {
    expect(fakeScene("clipsNotMoved")!.state.status.message).toEqual({ id: "clipsNotMoved" });
    expect(t("msg.clipsNotMoved")).toBe("ほかのクリップと重なるので、動かしませんでした。");
    expect(t("msg.clipsNotPasted")).toBe("ほかのクリップと重なるので、貼りつけませんでした。");
    expect(t("msg.clipsNotDuplicated")).toBe("ほかのクリップと重なるので、複製できませんでした。");
  });
});

describe("fake Rust: protocol 8 intents", () => {
  it("choose a track, toggle mute and solo", async () => {
    const f = await opened("arrangeSelect");
    let s = await f.send({ type: "selectTrack", track: 3, fromArm: false }, { type: "toggleMute", track: 2 }, { type: "toggleSolo", track: 1 });
    expect(s.song.playTarget).toBe(3);
    expect(s.song.tracks.map((t) => [t.muted, t.soloed])).toEqual([
      [false, true],
      [false, false],
      [false, true],
    ]);
    s = await f.send({ type: "selectTrack", track: 9, fromArm: false });
    expect(s.song.playTarget).toBe(3);
  });

  it("pressing a clip also chooses its track. Shift adds, a box replaces, Mod A selects all", async () => {
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

  it("Shift-pressing an already selected clip makes it the pressed clip (same as core)", async () => {
    const f = await opened("arrangeSelect");
    let s = await f.send({ type: "selectClip", clip: 1 }, { type: "selectClips", clips: [43], mode: "add" });
    expect([s.arrange.selectedClips, s.arrange.selectedClip]).toEqual([[1, 43], 43]);
    s = await f.send({ type: "selectClips", clips: [1], mode: "add" });
    expect([s.arrange.selectedClips, s.arrange.selectedClip, s.arrange.pianoRollClip, s.song.playTarget]).toEqual([[1, 43], 1, 1, 1]);
  });

  it("if clips overlap and cannot be placed, the status line says so. Going outside the tracks says nothing", async () => {
    const f = await opened("arrangeSelect");
    const before = clipsOf(f.now());
    let s = await f.send({ type: "moveClips", clips: [1], ticks: 4 * BAR, tracks: 0, copy: false });
    expect([clipsOf(s), s.status.message]).toEqual([before, { id: "clipsNotMoved" }]);
    s = await f.send({ type: "selectClip", clip: null });
    expect(s.status.message?.id).not.toBe("clipsNotMoved");
    s = await f.send({ type: "moveClips", clips: [1], ticks: 4 * BAR, tracks: 0, copy: true });
    expect([clipsOf(s), s.status.message]).toEqual([before, { id: "clipsNotDuplicated" }]);
    s = await f.send({ type: "duplicateClips", clips: [1] });
    expect([clipsOf(s), s.status.message]).toEqual([before, { id: "clipsNotDuplicated" }]);
    s = await f.send({ type: "copyClips", clips: [3] }, { type: "pasteClips", to: { track: 1, tick: 0 } });
    expect([clipsOf(s), s.status.message]).toEqual([before, { id: "clipsNotPasted" }]);
    s = await f.send({ type: "selectClip", clip: null }, { type: "moveClips", clips: [43], ticks: 40 * BAR, tracks: 1, copy: false });
    expect([clipsOf(s), s.status.message?.id]).toEqual([before, "summary"]);
  });

  it("move together (by the track row difference). Nothing happens if it goes outside or overlaps", async () => {
    const f = await opened("arrangeSelect");
    // The piano's 「Aメロ」 (5–9) and the bass's 「ベース A」 (5–9) go 1 row down and 4 bars later.
    let s = await f.send({ type: "moveClips", clips: [3, 41], ticks: 4 * BAR, tracks: 1, copy: false });
    expect(clipsOf(s)).toEqual([[[1, 1]], [[3, 9]], [[43, 1], [41, 9]]]);
    expect([s.arrange.selectedClips, s.arrange.selectedClip]).toEqual([[3, 41], 41]);
    const before = clipsOf(s);
    s = await f.send({ type: "moveClips", clips: [3, 41], ticks: 0, tracks: 2, copy: false });
    expect(clipsOf(s)).toEqual(before);
    s = await f.send({ type: "moveClips", clips: [3], ticks: -8 * BAR, tracks: 1, copy: false });
    expect(clipsOf(s)).toEqual(before);
  });

  it("copying many goes right after the group. The copies are selected", async () => {
    const f = await opened("arrangeSelect");
    const s = await f.send({ type: "duplicateClips", clips: [3, 41] });
    expect(clipsOf(s)[0]!.map((c) => c[1])).toEqual([1, 5, 9]);
    expect(clipsOf(s)[1]!.map((c) => c[1])).toEqual([5, 9]);
    expect(s.arrange.selectedClips).toHaveLength(2);
    expect(s.arrange.selectedClips).not.toContain(3);
  });

  it("copy / cut / paste (Mod V at the bar start of the play position on the chosen track, right-click at the pressed place)", async () => {
    const f = await opened("arrangeSelect");
    let s = await f.send({ type: "copyClips", clips: [3, 41] });
    expect(s.arrange.clipboardClips).toBe(2);
    // The chosen track is the bass (2), the play position is bar 5: overlaps.
    s = await f.send({ type: "pasteClips", to: null });
    expect(clipsOf(s)[1]).toEqual([[41, 5]]);
    s = await f.send({ type: "locate", tick: 12 * BAR + 300 }, { type: "selectTrack", track: 1, fromArm: false }, { type: "pasteClips", to: null });
    expect(clipsOf(s)[0]!.map((c) => c[1])).toEqual([1, 5, 13]);
    expect(clipsOf(s)[1]!.map((c) => c[1])).toEqual([5, 13]);
    s = await f.send({ type: "pasteClips", to: { track: 1, tick: 20 * BAR + 10 } });
    expect(clipsOf(s)[0]!.at(-1)![1]).toBe(21 + 10 / BAR);
    // No paste where it goes below the 3rd track.
    const before = clipsOf(s);
    s = await f.send({ type: "pasteClips", to: { track: 3, tick: 40 * BAR } });
    expect(clipsOf(s)).toEqual(before);
    s = await f.send({ type: "cutClips", clips: [1] });
    expect([s.arrange.clipboardClips, clipsOf(s)[0]![0]![0]]).toEqual([1, 3]);
  });

  it("Delete deletes all selected clips", async () => {
    const f = await opened("arrangeSelect");
    const s = await f.send({ type: "deleteSelected" });
    expect(clipsOf(s)).toEqual([[[1, 1]], [], [[43, 1]]]);
    expect(s.arrange.selectedClips).toEqual([]);
  });

  it("fixed velocity, rounded value, aligned text", async () => {
    const f = await opened("pianoRoll");
    let s = await f.send({ type: "setVelocityFixed", on: true, value: 200 });
    expect(s.keyboards.velocityFixed).toEqual({ on: true, value: 127 });
    s = await f.send({ type: "matchVelocity", take: s.pianoRoll.take!, notes: [14, 15, 16], value: 100 });
    expect(s.status.message).toEqual({ id: "velocityMatched", count: 3, value: 100 });
    s = await f.send({ type: "matchLength", take: s.pianoRoll.take!, notes: [14, 15, 16], value: TPQ / 2 });
    expect(s.status.message).toEqual({ id: "lengthMatched", count: 3, value: TPQ / 2 });
    s = await f.send({ type: "clampValue", field: "velocity", value: "127", range: null });
    expect(s.status.message).toEqual({ id: "valueClamped", field: "velocity", value: "127", range: null });
    s = await f.send({ type: "selectAllNotes" });
    expect(s.status.message?.id).toBe("summary");
  });

  it("the play position is the pressed place as it is", async () => {
    const f = await opened("arrangeSelect");
    const s = await f.send({ type: "locate", tick: 4 * BAR + 123 });
    expect(s.transport.position).toBe(4 * BAR + 123);
  });
});
