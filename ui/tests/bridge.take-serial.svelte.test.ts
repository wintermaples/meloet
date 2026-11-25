// Take ids start from 1 in each song. When the song serial (song.serial) changes, fetch the new song's content even for the same id.
import { flushSync } from "svelte";
import { describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { encodeTake, fakeState } from "../src/bridge/fake";
import { decodeTake, TakeStore } from "../src/bridge/take";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

const song = vi.hoisted(() => ({ key: 60, asked: [] as number[], refused: new Set<number>() }));

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  takeNotes: async (id: number) => {
    song.asked.push(id);
    if (song.refused.has(id)) throw new Error("no such take");
    return decodeTake(bytes(id, song.key));
  },
}));

const { followShownTake, shown } = await import("../src/components/pianoroll/shown.svelte");

function bytes(id: number, key: number): ArrayBuffer {
  return encodeTake({ id, length: 4 * TPQ, notes: [{ start: 0, length: TPQ, key, velocity: 80 }] });
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function stateWith(serial: number, takeId: number): AppState {
  const s = fakeState();
  s.song = {
    ...s.song,
    serial,
    tracks: [
      {
        ...s.song.tracks[0]!,
        clips: [{ id: 9, name: "ピアノ", start: 0, end: 4 * TPQ, take: 1, takes: 1, takeId, takeOffset: 0, noteCount: 1, audio: null, comp: [] }],
      },
    ],
  };
  s.arrange = { ...s.arrange, pianoRollClip: 9 };
  return s;
}

describe("TakeStore.clear", () => {
  it("after clearing, fetch again even for the same id", async () => {
    let key = 60;
    const fetched: number[] = [];
    const s = new TakeStore(async (id) => {
      fetched.push(id);
      return bytes(id, key);
    });
    expect((await s.get(3)).keys[0]).toBe(60);
    key = 72;
    s.clear();
    expect(s.peek(3)).toBeUndefined();
    expect((await s.get(3)).keys[0]).toBe(72);
    expect(fetched).toEqual([3, 3]);
  });

  it("answers asked for before clearing are not kept; the new song's content is fetched again", async () => {
    let key = 60;
    const answers: (() => void)[] = [];
    const s = new TakeStore(
      (id) =>
        new Promise((resolve) => {
          const k = key;
          answers.push(() => resolve(bytes(id, k)));
        }),
    );
    const old = s.get(3);
    key = 72;
    s.clear();
    const fresh = s.get(3);
    answers[0]!();
    await settle();
    expect(s.peek(3)).toBeUndefined();
    answers[1]!();
    expect((await fresh).keys[0]).toBe(72);
    // The old request waits for the same thing as the new one and answers with the new song's content.
    expect((await old).keys[0]).toBe(72);
    expect(answers.length).toBe(2);
    expect(s.peek(3)?.keys[0]).toBe(72);
  });
});

describe("shown", () => {
  it("when the serial changes, fetch again even for a take with the same id. Not if it stays the same", async () => {
    song.asked.length = 0;
    song.key = 60;
    app.setState(stateWith(1, 3));
    const stop = $effect.root(() => followShownTake());
    flushSync();
    await settle();
    expect(shown.take?.keys[0]).toBe(60);

    app.setState(stateWith(1, 3));
    flushSync();
    await settle();
    expect(song.asked).toEqual([3]);

    song.key = 72;
    app.setState(stateWith(2, 3));
    flushSync();
    // Until the new song's content arrives, the old song's notes are not shown.
    expect(shown.take).toBeNull();
    await settle();
    expect(song.asked).toEqual([3, 3]);
    expect(shown.take?.keys[0]).toBe(72);
    stop();
  });

  it("a refused fetch is caught; the take shown before (of another take) does not stay", async () => {
    song.asked.length = 0;
    song.key = 60;
    song.refused.add(5);
    app.setState(stateWith(1, 3));
    const stop = $effect.root(() => followShownTake());
    flushSync();
    await settle();
    expect(shown.take?.id).toBe(3);

    app.setState(stateWith(1, 5));
    flushSync();
    await settle();
    expect(song.asked).toContain(5);
    expect(shown.take).toBeNull();
    expect(shown.origin).toBeNull();
    song.refused.clear();
    stop();
  });
});
