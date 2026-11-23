import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene, toRanges } from "../src/bridge/fake";
import { NO_FROM, decodeTake } from "../src/bridge/take";
import type { Change, ServerMessage } from "../src/bridge/types";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const keyName = (k: number) => `${NAMES[k % 12]}${Math.floor(k / 12) - 1}`;
/** The mocks write the tick of a position or a length as 480 to a beat. */
const MOCK_TICKS_PER_BEAT = 480;
const mockTick = (t: number) => String(((t % TPQ) * MOCK_TICKS_PER_BEAT) / TPQ).padStart(3, "0");
const pos = (t: number) => `${Math.floor(t / (4 * TPQ)) + 1}.${Math.floor((t % (4 * TPQ)) / TPQ) + 1}.${mockTick(t)}`;
const len = (t: number) => `${Math.floor(t / TPQ)}.${mockTick(t)}`;

// The note list of F-WidePR, as written there.
const LIST = `1.1.000 C3 4.000 70|1.1.000 G3 4.000 64|1.1.000 E4 1.000 87|1.2.000 G4 1.000 92|1.3.000 C5 1.240 101|
1.4.240 G4 0.240 74|2.1.000 F3 4.000 66|2.1.000 A3 4.000 62|2.1.000 A4 1.000 84|2.2.000 G4 0.240 70|2.2.240 F4 0.240 72|
2.3.000 E4 2.000 90|3.1.000 G3 4.000 71|3.1.000 B3 4.000 60|3.1.000 D4 0.360 80|3.1.360 E4 0.360 83|3.2.240 F4 0.240 77|
3.3.000 G4 2.000 96|4.1.000 C3 4.000 73|4.1.000 E3 4.000 61|4.1.000 G3 4.000 63|4.1.000 C4 3.000 88|4.4.000 E4 0.360 58`
  .split("|")
  .map((r) => r.trim());

async function opened(name: string) {
  const scene = fakeScene(name)!;
  const transport = createFakeTransport(scene.state);
  const got: Change[] = [];
  await transport.subscribe(
    (m: ServerMessage) => {
      if (m.kind === "patch") got.push(...m.changes);
    },
    () => {},
  );
  return { scene, transport, got };
}

describe("fake piano roll", () => {
  it("the take matches the mock's note list. The 3 aligned notes come from the same indices of the original take and differ only in position", async () => {
    const { scene, transport } = await opened("pianoRoll");
    const pr = scene.state.pianoRoll;
    const take = decodeTake(await transport.takeNotes(pr.take!));
    const rows = Array.from(take.starts, (s, i) => `${pos(s)} ${keyName(take.keys[i]!)} ${len(take.lengths[i]!)} ${take.velocities[i]}`);
    expect(rows).toEqual(LIST);
    expect(Array.from(take.froms)).toEqual(LIST.map((_, i) => i));
    expect(take.froms).not.toContain(NO_FROM);

    const played = decodeTake(await transport.takeNotes(take.origin));
    expect(played.origin).toBe(0);
    const moved = Array.from(take.starts).flatMap((s, i) => (played.starts[take.froms[i]!] === s ? [] : [i]));
    expect(moved).toEqual([14, 15, 16]);
    expect(pr.selectedNotes).toEqual([{ start: 14, end: 17 }]);
  });

  it("a clip with 3 takes (take 2 / 3), and the notes held while recording", () => {
    const play = fakeScene("pianoRollPlay")!.state;
    const clip = play.song.tracks[0]!.clips[0]!;
    expect([clip.name, clip.take, clip.takes, clip.takeId]).toEqual(["Aメロ", 2, 3, play.pianoRoll.take]);
    const rec = fakeScene("pianoRollRec")!;
    expect(rec.state.transport.recording?.take).toBe(3);
    expect(rec.live.notes.filter((n) => n.length === null).map((n) => keyName(n.key))).toEqual(["C3", "E3", "G3", "C4"]);
    expect(fakeScene("pianoRoll125")!.state.workspace.scale).toBe(125);
    expect(fakeScene("pianoRollWide")!.state.song.tracks.map((t) => t.name)).toEqual(["ピアノ", "ベース", "ドラム", "パッド", "ボーカル"]);
  });

  it("select, clear selection and grid come back in the state", async () => {
    const { scene, transport, got } = await opened("pianoRoll");
    const take = scene.state.pianoRoll.take!;
    await transport.send({ type: "selectNotes", take, notes: [0, 1], mode: "add" });
    expect(got.at(-2)).toMatchObject({ section: "pianoRoll", value: { selectedNotes: [{ start: 0, end: 2 }, { start: 14, end: 17 }] } });
    expect(got.at(-1)).toMatchObject({ section: "status", value: { message: { id: "summary", notes: { count: 5, keys: [48, 55, 62, 64] } } } });
    await transport.send({ type: "selectNotes", take, notes: [1, 15], mode: "toggle" });
    expect(got.at(-2)).toMatchObject({ value: { selectedNotes: [{ start: 0, end: 1 }, { start: 14, end: 15 }, { start: 16, end: 17 }] } });
    const before = got.length;
    await transport.send({ type: "selectNotes", take: take + 100, notes: [3], mode: "replace" });
    expect(got.length).toBe(before);
    await transport.send({ type: "clearNoteSelection" });
    expect(got.at(-2)).toMatchObject({ value: { selectedNotes: [] } });
    expect(got.at(-1)).toMatchObject({ value: { message: { id: "summary", notes: null } } });
    await transport.send({ type: "setGrid", division: 8, triplet: true });
    expect(got.at(-1)).toMatchObject({ section: "pianoRoll", value: { grid: { division: 8, triplet: true } } });
  });

  it("index ranges are sorted and joined where they touch", () => {
    expect(toRanges([5, 1, 2, 3, 3, 9])).toEqual([
      { start: 1, end: 4 },
      { start: 5, end: 6 },
      { start: 9, end: 10 },
    ]);
  });
});
