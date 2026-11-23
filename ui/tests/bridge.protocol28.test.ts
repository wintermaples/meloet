// Protocol 28: clip names, split here, join, make an empty clip, a marker there, overdub, chord stamp.
// Shapes, fake intents and history rows. The core and the screen parts come in the second pass.
import { describe, expect, it } from "vitest";
import type { AppState, Command, HistoryRow } from "../src/bridge";
import { STEP_NAME_KINDS } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { stepNames } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const BAR = 4 * TPQ;

/** The band's song of six tracks (bands closed, no clip in the piano roll), the play line at bar 7. */
async function opened() {
  const t = createFakeTransport(fakeScene("tempoFolded")!.state);
  await t.subscribe(
    () => {},
    () => {},
  );
  const now = async () => (await t.snapshot()).state as AppState;
  const send = async (command: Command) => {
    await t.send(command);
    return now();
  };
  const track = async (name: string) => (await now()).song.tracks.find((x) => x.name === name)!;
  const newest = async (): Promise<HistoryRow> => {
    const s = await now();
    return (await t.historyRows(s.history.total, s.history.total))[0]!;
  };
  return { t, now, send, track, newest };
}

describe("shape of protocol 28", () => {
  it("in a new song「重ねる」is off", () => {
    expect(fakeState().transport.overdub).toBe(false);
  });

  it("three new step names. The text follows the owner's decision and the spec", () => {
    expect(STEP_NAME_KINDS).toEqual(expect.arrayContaining(["renameClip", "joinClips", "writeClip"]));
    expect(stepNames.renameClip).toEqual({ text: "クリップの名前を変える", source: "user:2026-11-23" });
    expect(stepNames.joinClips).toEqual({ text: "クリップをつなげる", source: "user:2026-11-23" });
    expect(stepNames.writeClip).toEqual({ text: "クリップを書く", source: "spec:3" });
  });
});

describe("fake intents", () => {
  it("renaming a clip: trims spaces around, 1 step. History is「前 → 後」. Empty or same name does nothing", async () => {
    const { now, send, track, newest } = await opened();
    const clip = (await track("ピアノ")).clips[0]!;
    const before = (await now()).song.undoCount;
    let s = await send({ type: "renameClip", clip: clip.id, name: "  はじまり " });
    expect(s.song.tracks[0]!.clips[0]!.name).toBe("はじまり");
    expect(s.song.undoCount).toBe(before + 1);
    expect(s.history.total).toBe(before + 1);
    expect(await newest()).toEqual({
      index: before + 1,
      step: before + 1,
      name: { kind: "renameClip" },
      what: { kind: "renamed", from: "イントロ", to: "はじまり" },
      place: { kind: "arrange" },
    });
    s = await send({ type: "renameClip", clip: clip.id, name: "   " });
    s = await send({ type: "renameClip", clip: clip.id, name: "はじまり" });
    expect(s.song.undoCount).toBe(before + 1);
  });

  it("split here: the right half is selected, and notes across the cut end at the cut on the left. The status line is the same as S. Nothing at the ends", async () => {
    const { t, send, track, newest } = await opened();
    const drums = await track("ドラム");
    const beat = drums.clips.find((c) => c.name === "8ビート")!;
    const at = 7 * BAR;
    expect(await send({ type: "splitClipAt", clip: beat.id, tick: beat.start })).toMatchObject({ song: { undoCount: 14 } });
    const s = await send({ type: "splitClipAt", clip: beat.id, tick: at });
    const clips = s.song.tracks.find((x) => x.id === drums.id)!.clips;
    expect(clips).toHaveLength(4);
    const [left, right] = [clips[1]!, clips[2]!];
    expect([left.id, left.start, left.end]).toEqual([beat.id, beat.start, at]);
    expect([right.start, right.end, right.name, right.takeId]).toEqual([at, beat.end, "8ビート", beat.takeId]);
    expect(right.takeOffset).toBe(beat.takeOffset - (at - beat.start));
    expect(s.arrange.selectedClips).toEqual([right.id]);
    expect(s.status.message).toEqual({ id: "clipSplit", name: "8ビート", tick: at, count: 1 });
    expect((await newest()).name).toEqual({ kind: "splitClips" });
    const cut = at - (left.start + left.takeOffset);
    const notes = decodeTake(await t.takeNotes(left.takeId));
    for (let i = 0; i < notes.starts.length; i++) {
      if (notes.starts[i]! < cut) expect(notes.starts[i]! + notes.lengths[i]!).toBeLessThanOrEqual(cut);
    }
  });

  it("join: into one with the first id and name. The gap becomes an empty part, with one take. Audio, different tracks, or only one do nothing", async () => {
    const { t, send, track, newest } = await opened();
    const piano = await track("ピアノ");
    const [intro, chorus] = piano.clips;
    const counts = intro!.noteCount + chorus!.noteCount;
    const bass = (await track("ベース")).clips[0]!;
    for (const clips of [[intro!.id], [intro!.id, bass.id]]) {
      expect((await send({ type: "joinClips", clips })).song.undoCount).toBe(14);
    }
    const s = await send({ type: "joinClips", clips: [chorus!.id, intro!.id] });
    const joined = s.song.tracks[0]!.clips;
    expect(joined).toHaveLength(1);
    expect(joined[0]).toMatchObject({ id: intro!.id, name: "イントロ", start: 0, end: 16 * BAR, take: 1, takes: 1, takeOffset: 0, noteCount: counts });
    expect(decodeTake(await t.takeNotes(joined[0]!.takeId)).starts).toHaveLength(counts);
    expect(s.arrange.selectedClips).toEqual([intro!.id]);
    expect(await newest()).toMatchObject({ name: { kind: "joinClips" }, what: { kind: "clips", count: 2 } });
  });

  it("make a clip here: 4 bars from that bar (up to before the next clip), named after the track. Selects it and opens the piano roll to show it", async () => {
    const { send, track, newest } = await opened();
    const pad = await track("パッド");
    expect((await send({ type: "createClip", track: pad.id, tick: 2 * BAR })).song.undoCount).toBe(14);
    let s = await send({ type: "createClip", track: pad.id, tick: 6 * BAR + 100 });
    let made = s.song.tracks.find((x) => x.id === pad.id)!.clips[1]!;
    expect([made.start, made.end, made.name, made.noteCount, made.audio]).toEqual([6 * BAR, 10 * BAR, "パッド", 0, null]);
    expect(s.arrange).toMatchObject({ selectedClips: [made.id], selectedClip: made.id, pianoRollClip: made.id });
    expect(s.pianoRoll.take).toBe(made.takeId);
    expect(s.workspace.layout.lower.map((l) => l.band)).toContain("pianoRoll");
    expect(await newest()).toMatchObject({ name: { kind: "writeClip" }, what: { kind: "name", name: "パッド" } });
    s = await send({ type: "createClip", track: pad.id, tick: 11 * BAR });
    made = s.song.tracks.find((x) => x.id === pad.id)!.clips[2]!;
    expect([made.start, made.end]).toEqual([11 * BAR, 12 * BAR]);
  });

  it("a marker there: snaps to the nearest beat start (half goes later). Without tick, at the play line. Selects it and opens the name field", async () => {
    const { send, newest } = await opened();
    let s = await send({ type: "addMarker", name: "マーカー 4", tick: 2 * BAR + TPQ / 2 });
    let m = s.timeline.markers.find((x) => x.name === "マーカー 4")!;
    expect(m.tick).toBe(2 * BAR + TPQ);
    expect(s.timeline).toMatchObject({ selected: { kind: "marker", id: m.id }, renaming: m.id });
    expect(s.status.message).toEqual({ id: "markerAdded", name: "マーカー 4" });
    expect(await newest()).toMatchObject({ name: { kind: "addMarker" }, what: { kind: "name", name: "マーカー 4" } });
    s = await send({ type: "addMarker", name: "マーカー 5", tick: null });
    m = s.timeline.markers.find((x) => x.name === "マーカー 5")!;
    expect(m.tick).toBe(s.transport.position);
    const ticks = s.timeline.markers.map((x) => x.tick);
    expect(ticks).toEqual([...ticks].sort((a, b) => a - b));
  });

  it("overdub: a toggle in the transport band. Not an undo step of the song", async () => {
    const { send } = await opened();
    let s = await send({ type: "setOverdub", on: true });
    expect(s.transport.overdub).toBe(true);
    expect(s.song.undoCount).toBe(14);
    s = await send({ type: "setOverdub", on: false });
    expect(s.transport.overdub).toBe(false);
  });

  it("chord stamp: many keys in the piano roll take as 1 step. The same key once. Writing outside makes the clip longer. Not written if it hits the next clip", async () => {
    const { t, send, track, newest } = await opened();
    const pad = await track("パッド");
    let s = await send({ type: "createClip", track: pad.id, tick: 6 * BAR });
    const made = s.song.tracks.find((x) => x.id === pad.id)!.clips[1]!;
    const before = s.song.undoCount;
    // Not the shown take: nothing.
    s = await send({ type: "addNotes", take: pad.clips[0]!.takeId, start: 0, length: TPQ, keys: [60], velocity: 90 });
    expect(s.song.undoCount).toBe(before);
    s = await send({ type: "addNotes", take: made.takeId, start: 10 * BAR - TPQ, length: 2 * TPQ, keys: [60, 64, 67, 64], velocity: 90 });
    const grown = s.song.tracks.find((x) => x.id === pad.id)!.clips[1]!;
    expect([grown.id, grown.start, grown.end, grown.noteCount]).toEqual([made.id, 6 * BAR, 11 * BAR, 3]);
    expect(s.song.undoCount).toBe(before + 1);
    expect(s.pianoRoll.take).toBe(grown.takeId);
    expect(s.pianoRoll.selectedNotes).toEqual([{ start: 0, end: 1 }, { start: 1, end: 2 }, { start: 2, end: 3 }]);
    const notes = decodeTake(await t.takeNotes(grown.takeId));
    expect([...notes.keys]).toEqual([60, 64, 67]);
    expect([...notes.starts]).toEqual([4 * BAR - TPQ, 4 * BAR - TPQ, 4 * BAR - TPQ]);
    expect(await newest()).toMatchObject({ name: { kind: "writeNote" }, what: { kind: "notes", count: 3 }, place: { kind: "pianoRoll" } });
    s = await send({ type: "addNotes", take: grown.takeId, start: 11 * BAR + TPQ, length: 4 * BAR, keys: [60, 64], velocity: 90 });
    expect(s.song.undoCount).toBe(before + 1);
    expect(s.status.message).toEqual({ id: "notesNotWritten" });
  });

  it("chord stamp: when the piano roll has no clip, make a clip on the keyboard track and write there", async () => {
    const { t, send } = await opened();
    const s = await send({ type: "addNotes", take: null, start: 6 * BAR, length: TPQ, keys: [57, 60, 64], velocity: 100 });
    const made = s.song.tracks[0]!.clips.find((c) => c.start === 6 * BAR)!;
    expect([made.end, made.name, made.noteCount]).toEqual([10 * BAR, "ピアノ", 3]);
    const notes = decodeTake(await t.takeNotes(made.takeId));
    expect([...notes.keys]).toEqual([57, 60, 64]);
    expect([...notes.starts]).toEqual([0, 0, 0]);
    expect(s.arrange.pianoRollClip).toBe(made.id);
  });
});
