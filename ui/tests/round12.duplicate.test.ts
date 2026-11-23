// A3 「トラックを複製」: the row of the track head's right-click menu, the fake's answer, and the status line at 64 tracks.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, fakeScene, FakeBands, reduceTracks, type FakeState } from "../src/bridge/fake";
import type { MenuRow } from "../src/components/menu/contextMenu.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { trackContextMenu } from "../src/components/trackMenu";
import { t } from "../src/text";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const run = (state: FakeState, c: Command) => reduceTracks(state, new FakeBands(800), c);

describe("「トラックを複製」 in the right-click menu of a track head", () => {
  const rows = (s: AppState, index: number, sent: Command[]) =>
    trackContextMenu(s, s.song.tracks[index]!, false, (c) => sent.push(c), (l) => l).filter((e): e is MenuRow => e.kind === "row");

  it("is right below 「名前を変える」, has no key, and sends the track's id", () => {
    const s = scene("tracks6");
    const sent: Command[] = [];
    const list = rows(s, 1, sent);
    const at = list.findIndex((r) => r.label === "menu.duplicateTrack");
    expect(list[at - 1]!.label).toBe("menu.renameTrack");
    expect(list[at + 1]!.label).toBe("menu.trackColor");
    expect(list[at]!.key).toBeUndefined();
    expect(list[at]!.usable).toBe(true);
    list[at]!.run();
    expect(sent).toEqual([{ type: "duplicateTrack", track: s.song.tracks[1]!.id }]);
    expect(t("menu.duplicateTrack")).toBe("トラックを複製");
  });

  it("is there for a vocal track too, and is disabled while recording", () => {
    const s = scene("audioEdit");
    const vocal = s.song.tracks.findIndex((x) => x.kind.kind === "vocal");
    expect(rows(s, vocal, []).some((r) => r.label === "menu.duplicateTrack" && r.usable)).toBe(true);
    s.transport.state = "recording";
    expect(rows(s, vocal, []).find((r) => r.label === "menu.duplicateTrack")!.usable).toBe(false);
  });
});

describe("the fake's 「トラックを複製」", () => {
  it("puts a copy right below with the next free number, the same color, new ids and the same takes, and selects it", () => {
    const s = scene("tracks6");
    const from = s.song.tracks[1]!;
    const r = run(s, { type: "duplicateTrack", track: from.id })!;
    expect(r.song.tracks.map((x) => [x.number, x.name])).toEqual([
      [1, "ピアノ"],
      [2, "ベース"],
      [3, "ベース 3"],
      [4, "ドラム"],
      [5, "パッド"],
      [6, "シンセ"],
      [7, "ベース 2"],
    ]);
    const copy = r.song.tracks[2]!;
    expect(r.song.playTarget).toBe(copy.id);
    expect(copy.color).toBe(from.color);
    expect([copy.gainDb, copy.instrument, copy.sends]).toEqual([from.gainDb, from.instrument, from.sends]);
    const ids = r.song.tracks.flatMap((x) => [x.id, ...x.clips.map((c) => c.id)]);
    expect(new Set(ids).size).toBe(ids.length);
    expect(copy.clips.map((c) => c.takeId)).toEqual(from.clips.map((c) => c.takeId));
    expect(r.notice).toBeUndefined();
  });

  it("aims the copy's lanes at its own inserts, and copies neither the solo nor the monitor", () => {
    const s = scene("autoRec");
    const piano = s.song.tracks[0]!;
    piano.soloed = true;
    const r = run(s, { type: "duplicateTrack", track: piano.id })!;
    const copy = r.song.tracks[1]!;
    expect(copy.name).toBe("ピアノ 2");
    expect(copy.soloed).toBe(false);
    const insertIds = (x: typeof copy) => x.slots.map((i) => i?.id ?? null);
    expect(insertIds(copy).filter((id) => id !== null)).toHaveLength(insertIds(piano).filter((id) => id !== null).length);
    for (const id of insertIds(copy)) if (id !== null) expect(insertIds(piano)).not.toContain(id);
    const threshold = copy.lanes.find((l) => l.target.kind === "insert")!;
    expect(threshold.target).toEqual({ kind: "insert", insert: copy.slots[1]!.id, param: 0 });
    expect(copy.lanes.map((l) => l.laneId)).not.toContain(piano.lanes[0]!.laneId);
    expect(copy.lanes.map((l) => l.points)).toEqual(piano.lanes.map((l) => l.points));
  });

  it("a vocal copy has its input and its ●, without the monitor", () => {
    const s = scene("audioEdit");
    const vocal = s.song.tracks.find((x) => x.kind.kind === "vocal")!;
    if (vocal.kind.kind === "vocal") vocal.kind.monitor = true;
    const r = run(s, { type: "duplicateTrack", track: vocal.id })!;
    const copy = r.song.tracks.find((x) => x.id === r.song.playTarget)!;
    expect(copy.kind).toEqual({ ...vocal.kind, armed: true, monitor: false });
  });

  it("says so at 64 tracks, and does nothing while recording or for a track that is not there", () => {
    const full = scene("tooManyTracks");
    expect(full.song.tracks).toHaveLength(64);
    expect(full.status.message).toEqual({ id: "tooManyTracks" });
    const r = run(full, { type: "duplicateTrack", track: full.song.tracks[0]!.id })!;
    expect(r.song.tracks).toHaveLength(64);
    expect(r.notice).toEqual({ id: "tooManyTracks" });
    const s = scene("tracks6");
    expect(run(s, { type: "duplicateTrack", track: 999 })).toBeNull();
    s.transport.state = "recording";
    expect(run(s, { type: "duplicateTrack", track: s.song.tracks[0]!.id })).toBeNull();
  });

  it("through the fake transport: the copy comes in the song, and at 64 tracks the status line says so", async () => {
    const changes = async (start: FakeState, c: Command) => {
      const transport = createFakeTransport(start);
      let state = (await transport.snapshot()).state;
      await transport.subscribe(
        (m) => {
          if (m.kind !== "patch") return;
          state = { ...state };
          for (const x of m.changes) (state as unknown as Record<string, unknown>)[x.section] = x.value;
        },
        () => {},
      );
      await transport.send(c);
      return state;
    };
    const s = scene("tracks6");
    const after = await changes(s, { type: "duplicateTrack", track: s.song.tracks[0]!.id });
    expect(after.song.tracks.map((x) => x.name).slice(0, 3)).toEqual(["ピアノ", "ピアノ 2", "ベース"]);
    expect(after.song.playTarget).toBe(after.song.tracks[1]!.id);
    const full = scene("tooManyTracks");
    full.status.message = null;
    const said = await changes(full, { type: "duplicateTrack", track: full.song.tracks[0]!.id });
    expect(said.status.message).toEqual({ id: "tooManyTracks" });
    expect(said.song.tracks).toHaveLength(64);
  });

  it("at 64 tracks 「＋ 歌のトラック」 and the shelf's new track say so too, and add nothing", async () => {
    const full = scene("tooManyTracks");
    full.status.message = null;
    const transport = createFakeTransport(full);
    let state = (await transport.snapshot()).state;
    await transport.subscribe(
      (m) => {
        if (m.kind !== "patch") return;
        state = { ...state };
        for (const x of m.changes) (state as unknown as Record<string, unknown>)[x.section] = x.value;
      },
      () => {},
    );
    await transport.send({ type: "addVocalTrack" });
    expect(state.status.message).toEqual({ id: "tooManyTracks" });
    expect(state.song.tracks).toHaveLength(64);
    state.status.message = null;
    await transport.send({ type: "openShelf", mode: { kind: "newTrack" } });
    await transport.send({ type: "shelfSelect", instrument: "organ" });
    await transport.send({ type: "shelfConfirm" });
    expect(state.status.message).toEqual({ id: "tooManyTracks" });
    expect(state.song.tracks).toHaveLength(64);
    expect(state.shelf.selectedSound).toEqual({ kind: "builtin", instrument: "organ" });
  });

  it("the scene of a duplicated track has the copy selected below 「2 ベース」", () => {
    const s = scene("trackDuplicated");
    expect(s.song.tracks.map((x) => x.name)).toEqual(["ピアノ", "ベース", "ベース 3", "ドラム", "パッド", "シンセ", "ベース 2"]);
    expect(s.song.playTarget).toBe(s.song.tracks[2]!.id);
  });
});

describe("the status line at 64 tracks", () => {
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
  });

  it("「トラックは 64 本までなので、これ以上足せません。」", () => {
    app.setState(scene("tooManyTracks"));
    mounted = mount(StatusLine, { target });
    flushSync();
    expect(target.querySelector(".message")?.textContent?.trim()).toBe("トラックは 64 本までなので、これ以上足せません。");
  });
});
