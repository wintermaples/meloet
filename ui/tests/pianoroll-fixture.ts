// Takes and views used in piano roll tests.
import { NO_FROM, type TakeNotes } from "../src/bridge";
import type { RollGeom } from "../src/components/pianoroll/geom";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

/** Makes a take from a list of [start, length, key, velocity]. */
export function take(notes: [number, number, number, number?][], id = 7, extra: Partial<TakeNotes> = {}): TakeNotes {
  return {
    id,
    length: 4 * 4 * TPQ,
    origin: 0,
    starts: Uint32Array.from(notes.map((n) => n[0])),
    lengths: Uint32Array.from(notes.map((n) => n[1])),
    keys: Uint8Array.from(notes.map((n) => n[2])),
    velocities: Uint8Array.from(notes.map((n) => n[3] ?? 80)),
    froms: new Uint32Array(notes.length).fill(NO_FROM),
    controlTicks: new Uint32Array(0),
    controlKinds: new Uint8Array(0),
    controlNumbers: new Uint8Array(0),
    controlValues: new Int16Array(0),
    ...extra,
  };
}

/** 40 px per beat (40 / TPQ px per tick), top row C5, 12 px per row, 696 × 492 (the size of mock A-PianoRoll). */
export const GEOM: RollGeom = { scrollTick: 0, ppt: 40 / TPQ, topKey: 72, rowH: 12, width: 696, height: 492 };
