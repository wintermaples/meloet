// The peaks binary (architecture 22-17) and the screen's store (PeakStore).
// The sample is written by `cargo test -p meloet-protocol` (tests/fixtures/peaks-v1.{bin,json}).
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { createFakeTransport } from "../src/bridge/fake";
import { fakePeakCount, fakePeakValues, type FakeAudio } from "../src/bridge/fakeAudio";
import { KEEP_DROPPED_PEAK_BYTES, PeakStore, decodePeaks, encodePeaks, levelFor, songAudioIds } from "../src/bridge/peaks";
import { PEAK_LEVELS, PEAKS_MAX_COUNT } from "../src/bridge/types";
import expected from "./fixtures/peaks-v1.json";

function fixtureBytes(): ArrayBuffer {
  const here = decodeURIComponent(new URL(import.meta.url).pathname);
  const b = new Uint8Array(readFileSync(`${here.slice(0, here.lastIndexOf("/"))}/fixtures/peaks-v1.bin`));
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength) as ArrayBuffer;
}

describe("peaks binary (type 4, version 1)", () => {
  it("reads the sample Rust wrote. Writing it back gives the same bytes", () => {
    const bytes = fixtureBytes();
    const got = decodePeaks(bytes);
    expect({ ...got, values: [...got.values] }).toEqual(expected);
    expect(new Uint8Array(encodePeaks(got))).toEqual(new Uint8Array(bytes));
  });

  it("does not read broken data: too short, wrong type, version, level or channel, length not matching the header count", () => {
    const good = new Uint8Array(fixtureBytes());
    const withByte = (at: number, value: number) => {
      const b = good.slice();
      b[at] = value;
      return b.buffer;
    };
    expect(() => decodePeaks(good.slice(0, 10).buffer)).toThrow(/shorter than the header/);
    expect(() => decodePeaks(withByte(0, 2))).toThrow(/kind 2/);
    expect(() => decodePeaks(withByte(1, 9))).toThrow(/version 9/);
    expect(() => decodePeaks(withByte(2, 5))).toThrow(/level 5/);
    expect(() => decodePeaks(withByte(3, 3))).toThrow(/3 channels/);
    expect(() => decodePeaks(good.slice(0, good.length - 1).buffer)).toThrow(/the header says/);
  });

  it("levels: 5, from 256 by 4 times. The level to draw is the coarsest one still finer than the frames per 1 px", () => {
    expect(PEAK_LEVELS).toEqual([256, 1024, 4096, 16384, 65536]);
    expect([10, 256, 1023, 1024, 5000, 65536, 1e9].map(levelFor)).toEqual([0, 0, 0, 1, 2, 4, 4]);
  });
});

const VOICE: FakeAudio = { id: 7, file: "歌.wav", sampleRate: 48000, bits: 24, channels: 2, frames: 10 * 48000 };

/** A fetcher that answers for the fake audio `files`, and how many times it was called. */
function fetcher(files: FakeAudio[]) {
  const calls: [number, number, number, number][] = [];
  const fetch = async (audio: number, level: number, first: number, count: number) => {
    calls.push([audio, level, first, count]);
    const a = files.find((f) => f.id === audio);
    if (!a) throw new Error(`no audio ${audio}`);
    return encodePeaks({
      audio,
      level,
      channels: a.channels,
      growing: a.growingTo !== undefined,
      first,
      total: fakePeakCount(a, level),
      values: fakePeakValues(a, level, first, count),
    });
  };
  return { calls, fetch };
}

describe("fake wave (a plausible shape)", () => {
  it("the same audio always gives the same. A coarse level's peak covers four of the finer level", () => {
    expect(fakePeakCount(VOICE, 0)).toBe(1875);
    expect(fakePeakCount(VOICE, 4)).toBe(8);
    const fine = fakePeakValues(VOICE, 0, 0, 1875);
    expect(fine).toEqual(fakePeakValues(VOICE, 0, 0, 1875));
    const coarse = fakePeakValues(VOICE, 1, 0, 469);
    expect(coarse.length).toBe(469 * 2 * 2);
    for (const peak of [0, 100, 468]) {
      for (const channel of [0, 1]) {
        const lows = [0, 1, 2, 3].map((i) => fine[((peak * 4 + i) * 2 + channel) * 2]).filter((v) => v !== undefined);
        const highs = [0, 1, 2, 3].map((i) => fine[((peak * 4 + i) * 2 + channel) * 2 + 1]).filter((v) => v !== undefined);
        expect(coarse[(peak * 2 + channel) * 2]).toBe(Math.min(...lows));
        expect(coarse[(peak * 2 + channel) * 2 + 1]).toBe(Math.max(...highs));
      }
    }
    // Like a voice, with loud and quiet parts.
    const tops = Array.from({ length: 1875 }, (_, i) => fine[i * 4 + 1]!);
    expect(Math.max(...tops)).toBeGreaterThan(20000);
    expect(tops.filter((v) => v < 2000).length).toBeGreaterThan(100);
    expect(tops.every((v, i) => v >= 0 && fine[i * 4]! <= 0)).toBe(true);
  });

  it("peaks of the fake transport: answers for the scene's audio, refuses missing and not-found audio", async () => {
    const t = createFakeTransport();
    const got = decodePeaks(await t.peaks(60, 0, 10, 5));
    expect([got.audio, got.level, got.channels, got.first, got.count, got.total, got.growing]).toEqual([60, 0, 1, 10, 5, 1875, false]);
    expect(decodePeaks(await t.peaks(59, 0, 0, 9)).growing).toBe(true);
    await expect(t.peaks(999, 0, 0, 1)).rejects.toThrow(/no audio 999/);
    await expect(t.peaks(64, 0, 0, 1)).rejects.toThrow(/no peaks of audio 64/);
  });
});

describe("peak store (PeakStore)", () => {
  it("fetches a whole level once and not again. Two requests at the same time make one", async () => {
    const { calls, fetch } = fetcher([VOICE]);
    const store = new PeakStore(fetch);
    expect(store.peek(7, 0)).toBeUndefined();
    const [a, b] = await Promise.all([store.get(7, 0), store.get(7, 0)]);
    expect(a).toBe(b);
    expect(calls).toEqual([[7, 0, 0, PEAKS_MAX_COUNT]]);
    expect([a.count, a.channels, a.framesPerPeak, a.growing, a.values.length]).toEqual([1875, 2, 256, false, 1875 * 4]);
    expect(a.values).toEqual(fakePeakValues(VOICE, 0, 0, 1875));
    expect(await store.get(7, 0)).toBe(a);
    expect(store.peek(7, 0)).toBe(a);
    expect(calls.length).toBe(1);
    await store.get(7, 3);
    expect(calls.length).toBe(2);
    expect(store.held()).toMatchObject({ ready: 2, dropped: 0 });
  });

  it("long audio fetches the rest PEAKS_MAX_COUNT at a time", async () => {
    const long: FakeAudio = { ...VOICE, id: 8, channels: 1, frames: (PEAKS_MAX_COUNT + 100) * 256 };
    const { calls, fetch } = fetcher([long]);
    const got = await new PeakStore(fetch).get(8, 0);
    expect(calls.map((c) => c[2])).toEqual([0, PEAKS_MAX_COUNT]);
    expect(got.count).toBe(PEAKS_MAX_COUNT + 100);
  });

  it("audio being recorded: while growing, fetch the rest with more (the last peak again). After it ends, do not fetch", async () => {
    const rec: FakeAudio = { id: 9, file: "歌 9.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 0, growingTo: 1000 };
    const { calls, fetch } = fetcher([rec]);
    const store = new PeakStore(fetch);
    const first = await store.get(9, 0);
    expect([first.count, first.growing]).toEqual([4, true]);
    rec.growingTo = 2600;
    const second = await store.more(9, 0);
    expect(calls.at(-1)).toEqual([9, 0, 3, PEAKS_MAX_COUNT]);
    expect([second.count, second.growing]).toEqual([11, true]);
    expect(second.values).toEqual(fakePeakValues(rec, 0, 0, 11));
    expect(store.peek(9, 0)).toBe(second);
    Object.assign(rec, { frames: 2600, growingTo: undefined });
    const done = await store.more(9, 0);
    expect([done.count, done.growing]).toEqual([11, false]);
    const asked = calls.length;
    expect(await store.more(9, 0)).toBe(done);
    expect(calls.length).toBe(asked);
  });

  it("peaks of audio not in use go aside. Keep the newest up to the byte limit, and they come back when used again. A different song drops them all", async () => {
    const files = [1, 2, 3].map((id): FakeAudio => ({ ...VOICE, id, channels: 1, frames: 1000 * 256 }));
    const { calls, fetch } = fetcher(files);
    // One level is 1000 peaks × 4 bytes. Up to two aside.
    const store = new PeakStore(fetch, 8000);
    for (const f of files) await store.get(f.id, 0);
    store.keepOnly([3]);
    expect(store.held()).toEqual({ ready: 1, dropped: 2, bytes: 12000 });
    expect(store.peek(1, 0)).toBeDefined();
    store.keepOnly([]);
    expect(store.held()).toEqual({ ready: 0, dropped: 2, bytes: 8000 });
    expect(store.peek(1, 0)).toBeUndefined();
    // Audio back after undo: back from aside, not fetched.
    store.keepOnly([2]);
    expect(store.held()).toMatchObject({ ready: 1, dropped: 1 });
    await store.get(2, 0);
    expect(calls.length).toBe(3);
    store.clear();
    expect(store.held()).toEqual({ ready: 0, dropped: 0, bytes: 0 });
    expect(KEEP_DROPPED_PEAK_BYTES).toBe(4 * 1024 * 1024);
  });

  it("audio the song uses: audio of audio clips, and audio being recorded", () => {
    const song = { tracks: [{ clips: [{ audio: null }, { audio: { audio: 60 } }] }, { clips: [{ audio: { audio: 60 } }, { audio: { audio: 61 } }] }] };
    expect(songAudioIds(song)).toEqual([60, 61]);
    expect(songAudioIds(song, [{ audio: 59 }, { audio: 61 }])).toEqual([60, 61, 59]);
  });

  it("audio the song uses: every take of a comped clip, not only the shown one", () => {
    const song = { tracks: [{ clips: [{ audio: { audio: 60, takes: [{ audio: 60 }, { audio: 62 }, { audio: 63 }] } }] }] };
    expect(songAudioIds(song)).toEqual([60, 62, 63]);
  });
});
