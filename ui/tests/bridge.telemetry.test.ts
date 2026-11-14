import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { decodeTelemetry, keyIsDown } from "../src/bridge/telemetry";
// Both files are written by `cargo test -p meloet-protocol` from the Rust encoder.
import expected from "./fixtures/telemetry-v1.json";

function fixtureBytes(): Uint8Array {
  const here = decodeURIComponent(new URL(import.meta.url).pathname);
  const dir = here.slice(0, here.lastIndexOf("/"));
  return new Uint8Array(readFileSync(`${dir}/fixtures/telemetry-v1.bin`));
}

describe("telemetry binary, version 1", () => {
  it("(d) reads what Rust wrote", () => {
    const bytes = fixtureBytes();
    expect(bytes.length).toBe(48 + 8 * (expected.trackPeaks.length / 2));

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
      trackPeaks: Array.from(t.trackPeaks),
    }).toEqual(expected);
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
    version[1] = 2;
    expect(() => decodeTelemetry(version)).toThrow(/version/);
    expect(() => decodeTelemetry(good.subarray(0, 47))).toThrow(/shorter/);
    expect(() => decodeTelemetry(good.subarray(0, good.length - 1))).toThrow(/expected 64/);
  });
});
