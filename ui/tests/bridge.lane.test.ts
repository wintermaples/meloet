import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { encodeLane } from "../src/bridge/fake";
import { KEEP_DROPPED_LANES, LaneStore, decodeLane, songLaneIds } from "../src/bridge/lane";
// Both files are written by `cargo test -p meloet-protocol` from the Rust encoder.
import expected from "./fixtures/lane-v1.json";

function fixtureBytes(): Uint8Array {
  const here = decodeURIComponent(new URL(import.meta.url).pathname);
  const dir = here.slice(0, here.lastIndexOf("/"));
  return new Uint8Array(readFileSync(`${dir}/fixtures/lane-v1.bin`));
}

describe("lane binary, version 1", () => {
  it("reads what Rust wrote", () => {
    const l = decodeLane(fixtureBytes());
    expect({
      id: l.id,
      step: l.step,
      points: Array.from(l.ticks, (tick, i) => ({ tick, value: l.values[i] })),
    }).toEqual(expected);
    // A tick above 2^32, and the largest whole number a JS number holds.
    expect(Array.from(l.ticks)).toEqual([0, 2 ** 32 + 7680, Number.MAX_SAFE_INTEGER]);
  });

  it("refuses another kind, another version and a wrong length", () => {
    const good = fixtureBytes();
    const kind = good.slice();
    kind[0] = 2;
    expect(() => decodeLane(kind)).toThrow(/kind 2/);
    const version = good.slice();
    version[1] = 2;
    expect(() => decodeLane(version)).toThrow(/version 2/);
    expect(() => decodeLane(good.subarray(0, 15))).toThrow(/shorter/);
    expect(() => decodeLane(good.subarray(0, good.length - 1))).toThrow(/expected 64/);
  });

  it("the fake writes what the reader reads", () => {
    const l = decodeLane(encodeLane({ id: 9, step: false, points: [[0, 0.5], [2 ** 32 + 1, 0.25]] }));
    expect([l.id, l.step, Array.from(l.ticks), Array.from(l.values)]).toEqual([9, false, [0, 2 ** 32 + 1], [0.5, 0.25]]);
    expect(decodeLane(encodeLane({ id: 9, step: true, points: [] })).step).toBe(true);
  });
});

describe("LaneStore", () => {
  const bytes = (id: number) => encodeLane({ id, step: false, points: [[id * 10, 0.5]] });

  it("fetches a lane once, and again after a failure", async () => {
    let calls = 0;
    let fail = true;
    const store = new LaneStore(async (id) => {
      calls += 1;
      if (fail) throw new Error("not yet");
      return bytes(id);
    });
    await expect(store.get(1)).rejects.toThrow(/not yet/);
    fail = false;
    const [a, b] = await Promise.all([store.get(1), store.get(1)]);
    expect(a).toBe(b);
    expect(calls).toBe(2);
    expect(store.peek(1)).toBe(a);
    expect(await store.get(1)).toBe(a);
    expect(calls).toBe(2);
  });

  it("refuses a reply for another lane", async () => {
    const store = new LaneStore(async () => bytes(1));
    await expect(store.get(2)).rejects.toThrow(/asked for lane 2, got 1/);
  });

  it("keeps the lanes of the song and the newest of the others; another song lets go of all", async () => {
    const store = new LaneStore(async (id) => bytes(id));
    const song = (ids: number[]) => ({ tracks: [{ lanes: ids.map((laneId) => ({ laneId })) }] });
    expect(songLaneIds({ tracks: [{ lanes: [{ laneId: 3 }] }, { lanes: [] }, { lanes: [{ laneId: 5 }, { laneId: 9 }] }] })).toEqual([3, 5, 9]);
    // Every edit of points is a new lane id: a thousand of them leave a fixed number behind.
    for (let id = 1; id <= 1000; id++) {
      await store.get(id);
      store.keepOnly(songLaneIds(song([id])));
    }
    expect(store.held()).toEqual({ ready: 1, dropped: KEEP_DROPPED_LANES });
    // Undo brings the lane before back without a fetch.
    const before = store.peek(999);
    expect(before).toBeDefined();
    store.keepOnly(songLaneIds(song([999])));
    expect(await store.get(999)).toBe(before);
    expect(store.peek(1)).toBeUndefined();
    store.clear();
    expect(store.held()).toEqual({ ready: 0, dropped: 0 });
  });
});
