import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { encodeTake } from "../src/bridge/fake";
import { CONTROL_CC, NO_FROM, TakeStore, decodeTake } from "../src/bridge/take";
// Both files are written by `cargo test -p meloet-protocol` from the Rust encoder.
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import expected from "./fixtures/take-v2.json";

function fixtureBytes(): Uint8Array {
  const here = decodeURIComponent(new URL(import.meta.url).pathname);
  const dir = here.slice(0, here.lastIndexOf("/"));
  return new Uint8Array(readFileSync(`${dir}/fixtures/take-v2.bin`));
}

describe("take binary, version 2", () => {
  it("reads what Rust wrote", () => {
    const t = decodeTake(fixtureBytes());
    expect({
      id: t.id,
      length: t.length,
      origin: t.origin,
      notes: Array.from(t.starts, (start, i) => ({
        start,
        length: t.lengths[i],
        key: t.keys[i],
        velocity: t.velocities[i],
        from: t.froms[i],
      })),
      controls: Array.from(t.controlTicks, (tick, i) => ({
        tick,
        kind: t.controlKinds[i] === CONTROL_CC ? "cc" : "pitchBend",
        number: t.controlNumbers[i],
        value: t.controlValues[i],
      })),
    }).toEqual(expected);
  });

  it("refuses another kind, another version, a wrong length or a wrong control kind", () => {
    const good = fixtureBytes();
    const kind = good.slice();
    kind[0] = 1;
    expect(() => decodeTake(kind)).toThrow(/kind 1/);
    const version = good.slice();
    version[1] = 1;
    expect(() => decodeTake(version)).toThrow(/version 1/);
    expect(() => decodeTake(good.subarray(0, 23))).toThrow(/shorter/);
    expect(() => decodeTake(good.subarray(0, good.length - 1))).toThrow(/expected 72/);
    const control = good.slice();
    control[60] = 7;
    expect(() => decodeTake(control)).toThrow(/control kind 7/);
  });

  it("the fake writes what the reader reads", () => {
    const notes = [
      { start: 10, length: 20, key: 60, velocity: 90, from: 3 },
      { start: 30, length: 20, key: 62, velocity: 91 },
    ];
    const t = decodeTake(encodeTake({ id: 9, length: 4 * TPQ, origin: 4, notes }));
    expect([t.id, t.length, t.origin, t.starts[0], t.lengths[0], t.keys[0], t.velocities[0], t.controlTicks.length]).toEqual([
      9, 4 * TPQ, 4, 10, 20, 60, 90, 0,
    ]);
    expect(Array.from(t.froms)).toEqual([3, NO_FROM]);
    expect(decodeTake(encodeTake({ id: 9, length: 4 * TPQ, notes })).origin).toBe(0);
  });
});

describe("TakeStore", () => {
  it("fetches a take once, and again after a failure", async () => {
    const asked: number[] = [];
    let fail = true;
    const store = new TakeStore(async (id) => {
      asked.push(id);
      if (fail) throw new Error("not yet");
      return encodeTake({ id, length: TPQ, notes: [] });
    });
    await expect(store.get(5)).rejects.toThrow("not yet");
    expect(store.peek(5)).toBeUndefined();
    fail = false;
    const [a, b] = await Promise.all([store.get(5), store.get(5)]);
    expect(a).toBe(b);
    expect(await store.get(5)).toBe(a);
    expect(store.peek(5)).toBe(a);
    expect(asked).toEqual([5, 5]);
  });

  it("refuses a reply for another take", async () => {
    const store = new TakeStore(async () => encodeTake({ id: 1, length: TPQ, notes: [] }));
    await expect(store.get(2)).rejects.toThrow(/asked for take 2, got 1/);
    expect(store.peek(1)).toBeUndefined();
  });
});
