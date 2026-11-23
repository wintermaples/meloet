// Cleaning the take store: takes no longer pointed to from anywhere in the song are let go, and only the newest 16 are kept.
import { describe, expect, it } from "vitest";
import { encodeTake } from "../src/bridge/fake";
import { KEEP_DROPPED_TAKES, songTakeIds, TakeStore } from "../src/bridge/take";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

function store() {
  const fetched: number[] = [];
  const s = new TakeStore(async (id) => {
    fetched.push(id);
    // A take with an even id has the id before it as its original take (before quantize).
    return encodeTake({ id, length: 4 * TPQ, origin: id % 2 === 0 ? id - 1 : undefined, notes: [{ start: 0, length: TPQ, key: 60, velocity: 80 }] });
  });
  return { s, fetched };
}

describe("TakeStore.keepOnly", () => {
  it("after swapping ids 1000 times, the number kept stays the same", async () => {
    const { s } = store();
    for (let id = 1; id <= 1000; id += 2) {
      await s.get(id);
      s.keepOnly([id]);
    }
    expect(s.held()).toEqual({ ready: 1, dropped: KEEP_DROPPED_TAKES });
  });

  it("takes in use and their original takes are never let go", async () => {
    const { s } = store();
    await s.get(3);
    await s.get(4);
    await s.get(5);
    s.keepOnly([4]);
    expect(s.held()).toEqual({ ready: 2, dropped: 1 });
    expect(s.peek(3)).toBeDefined();
  });

  it("a take brought back by undo comes from the store without fetching again. One not in the store is fetched from Rust again", async () => {
    const { s, fetched } = store();
    for (let id = 1; id <= 41; id += 2) {
      await s.get(id);
      s.keepOnly([id]);
    }
    fetched.length = 0;
    s.keepOnly([39]);
    await s.get(39);
    expect(fetched).toEqual([]);
    await s.get(1);
    expect(fetched).toEqual([1]);
  });

  it("take ids pointed to by the song's clips", () => {
    const clip = (takeId: number) => ({ takeId });
    expect(songTakeIds({ tracks: [{ clips: [clip(3), clip(7)] }, { clips: [] }, { clips: [clip(9)] }] })).toEqual([3, 7, 9]);
  });
});
