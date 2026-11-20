// テイクの id は曲ごとに 1 から。曲の通し番号（song.serial）が変わったら、同じ id でも新しい曲の中身を取りにいく。
import { flushSync } from "svelte";
import { describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { encodeTake, fakeState } from "../src/bridge/fake";
import { decodeTake, TakeStore } from "../src/bridge/take";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

const song = vi.hoisted(() => ({ key: 60, asked: [] as number[] }));

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  takeNotes: async (id: number) => {
    song.asked.push(id);
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
        clips: [{ id: 9, name: "ピアノ", start: 0, end: 4 * TPQ, take: 1, takes: 1, takeId, takeOffset: 0, noteCount: 1, vocal: false }],
      },
    ],
  };
  s.arrange = { ...s.arrange, pianoRollClip: 9 };
  return s;
}

describe("TakeStore.clear", () => {
  it("空にしたあとは、同じ id でも取りなおす", async () => {
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

  it("空にする前に頼んだ答えはためず、新しい曲の中身を取りなおす", async () => {
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
    // 古い頼みも、新しい頼みと同じ物を待ち、新しい曲の中身で答える。
    expect((await old).keys[0]).toBe(72);
    expect(answers.length).toBe(2);
    expect(s.peek(3)?.keys[0]).toBe(72);
  });
});

describe("shown", () => {
  it("通し番号が変わると、同じ id のテイクでも取りなおす。変わらなければ取りなおさない", async () => {
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
    // 新しい曲の中身が届くまで、前の曲のノートは見せない。
    expect(shown.take).toBeNull();
    await settle();
    expect(song.asked).toEqual([3, 3]);
    expect(shown.take?.keys[0]).toBe(72);
    stop();
  });
});
