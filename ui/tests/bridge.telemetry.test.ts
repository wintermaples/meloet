import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { decodeTelemetry, keyIsDown, positionAt } from "../src/bridge/telemetry";
// Both files are written by `cargo test -p meloet-protocol` from the Rust encoder.
import expected from "./fixtures/telemetry-v5.json";

function fixtureBytes(): Uint8Array {
  const here = decodeURIComponent(new URL(import.meta.url).pathname);
  const dir = here.slice(0, here.lastIndexOf("/"));
  return new Uint8Array(readFileSync(`${dir}/fixtures/telemetry-v5.bin`));
}

describe("telemetry binary, version 5", () => {
  it("(d) reads what Rust wrote", () => {
    const bytes = fixtureBytes();
    expect(bytes.length).toBe(124 + 8 * (expected.trackPeaks.length / 2) + 16 * expected.mics.length);
    expect(expected.mics.length).toBe(2);

    const t = decodeTelemetry(bytes.buffer as ArrayBuffer);
    expect({
      seq: t.seq,
      sentUs: t.sentUs,
      keys: Array.from(t.keys),
      keysDown: Array.from({ length: 128 }, (_, note) => note).filter((note) => keyIsDown(t, note)),
      lastVelocity: t.lastVelocity,
      sustain: t.sustain,
      modulation: t.modulation,
      running: t.running,
      masterClipped: t.masterClipped,
      pitchBend: t.pitchBend,
      loadPermille: t.loadPermille,
      masterPeak: t.masterPeak,
      position: t.position,
      returnPeaks: Array.from(t.returnPeaks),
      trackPeaks: Array.from(t.trackPeaks),
      mics: t.mics,
      masterReductionDb: t.masterReductionDb,
      spectrum: Array.from(t.spectrum),
      lufsShort: t.lufsShort,
      lufsIntegrated: t.lufsIntegrated,
      truePeakDb: t.truePeakDb,
    }).toEqual(expected);
    // The sample has a reading of each kind and one with none.
    expect([expected.lufsShort, expected.lufsIntegrated, expected.truePeakDb]).toEqual([-13.2, null, -1.4]);
  });

  it("reads the same from a view that does not start at byte 0", () => {
    const bytes = fixtureBytes();
    const shifted = new Uint8Array(bytes.length + 3);
    shifted.set(bytes, 3);
    const t = decodeTelemetry(shifted.subarray(3));
    expect(t.seq).toBe(expected.seq);
    expect(Array.from(t.trackPeaks)).toEqual(expected.trackPeaks);
  });

  it("refuses another kind, another version, or a wrong length", () => {
    const good = fixtureBytes();
    const kind = good.slice();
    kind[0] = 2;
    expect(() => decodeTelemetry(kind)).toThrow(/kind/);
    const version = good.slice();
    version[1] = 4;
    expect(() => decodeTelemetry(version)).toThrow(/version/);
    const state = good.slice();
    state[64] = 4;
    expect(() => decodeTelemetry(state)).toThrow(/transport state/);
    expect(() => decodeTelemetry(good.subarray(0, 123))).toThrow(/shorter/);
    expect(() => decodeTelemetry(good.subarray(0, good.length - 1))).toThrow(/expected 172/);
    const mics = good.slice();
    mics[88] = 3;
    expect(() => decodeTelemetry(mics)).toThrow(/expected 188/);
  });

  it("goes on from the heard position with ticksPerUs, and has none when the flag is off", () => {
    const t = decodeTelemetry(fixtureBytes());
    expect(positionAt(t, t.sentUs)).toBe(expected.position.tick);
    expect(positionAt(t, t.sentUs + 1000)).toBeCloseTo(expected.position.tick + 1000 * expected.position.ticksPerUs, 9);
    const none = fixtureBytes();
    none[35] = none[35]! & ~4;
    const n = decodeTelemetry(none);
    expect(n.position).toBeNull();
    expect(positionAt(n, n.sentUs)).toBeNull();
  });
});
