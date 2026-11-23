// The points of one automation lane, binary, version 1, little-endian: the reply of `lane`. The layout
// is fixed in crates/meloet-protocol/src/automation.rs (table: docs/architecture.md 21-15).
// A lane whose points change gets a new id, so the points are fetched once and kept by that id.

export const LANE_KIND = 3;
export const LANE_VERSION = 1;
export const LANE_HEADER_LEN = 16;
export const LANE_POINT_LEN = 16;
const FLAG_STEP = 1;

/** One array per field, so the WebGL side can upload them as they are. */
export interface LanePoints {
  /** `LaneView.laneId`. */
  id: number;
  /** The value stays at the point before until the next one. */
  step: boolean;
  /** Song positions, in order, each once. */
  ticks: Float64Array;
  /** The place of the knob, 0..1. */
  values: Float32Array;
}

export function decodeLane(data: ArrayBuffer | ArrayBufferView): LanePoints {
  const view = ArrayBuffer.isView(data)
    ? new DataView(data.buffer, data.byteOffset, data.byteLength)
    : new DataView(data);
  if (view.byteLength < LANE_HEADER_LEN) throw new Error(`lane is ${view.byteLength} bytes, shorter than the header`);
  if (view.getUint8(0) !== LANE_KIND) throw new Error(`lane kind ${view.getUint8(0)}, expected ${LANE_KIND}`);
  if (view.getUint8(1) !== LANE_VERSION) {
    throw new Error(`lane version ${view.getUint8(1)}, expected ${LANE_VERSION}`);
  }
  const n = view.getUint32(8, true);
  const expected = LANE_HEADER_LEN + LANE_POINT_LEN * n;
  if (view.byteLength !== expected) throw new Error(`lane is ${view.byteLength} bytes, expected ${expected}`);

  const lane: LanePoints = {
    id: view.getUint32(4, true),
    step: (view.getUint8(2) & FLAG_STEP) !== 0,
    ticks: new Float64Array(n),
    values: new Float32Array(n),
  };
  for (let i = 0, p = LANE_HEADER_LEN; i < n; i++, p += LANE_POINT_LEN) {
    // A u64; ticks stay far below 2^53.
    lane.ticks[i] = view.getUint32(p, true) + view.getUint32(p + 4, true) * 2 ** 32;
    lane.values[i] = view.getFloat32(p + 8, true);
  }
  return lane;
}

/** How many lanes the song no longer has are still kept (newest first): undo often brings them back. */
export const KEEP_DROPPED_LANES = 16;

/**
 * Fetches the points of each lane once. A fetch that fails is forgotten, so the next `get` tries again.
 * Every edit of points makes a new lane id, so `keepOnly` lets go of the ones the song no longer has.
 */
export class LaneStore {
  private readonly pending = new Map<number, Promise<LanePoints>>();
  private readonly ready = new Map<number, LanePoints>();
  /** Let go of, oldest first. Rust still has them, so dropping one too many only costs a fetch. */
  private readonly dropped = new Map<number, LanePoints>();
  /** Goes up on `clear`: an answer asked for before it may be of the other song. */
  private generation = 0;

  constructor(private readonly fetch: (id: number) => Promise<ArrayBuffer>) {}

  get(id: number): Promise<LanePoints> {
    const had = this.ready.get(id) ?? this.revive(id);
    if (had) return Promise.resolve(had);
    let p = this.pending.get(id);
    if (!p) {
      const generation = this.generation;
      const asked: Promise<LanePoints> = this.fetch(id)
        .then((bytes) => {
          if (generation !== this.generation) return this.get(id);
          const lane = decodeLane(bytes);
          if (lane.id !== id) throw new Error(`asked for lane ${id}, got ${lane.id}`);
          this.ready.set(id, lane);
          return lane;
        })
        .finally(() => {
          if (this.pending.get(id) === asked) this.pending.delete(id);
        });
      p = asked;
      this.pending.set(id, p);
    }
    return p;
  }

  /** Lets go of every lane, also the ones on the way: another song came, and lane ids start again per song. */
  clear(): void {
    this.generation += 1;
    this.pending.clear();
    this.ready.clear();
    this.dropped.clear();
  }

  /** The points if they are here already; for drawing a frame without waiting. */
  peek(id: number): LanePoints | undefined {
    return this.ready.get(id) ?? this.dropped.get(id);
  }

  /** Keeps the lanes `inUse`; puts every other one aside, keeping only the newest KEEP_DROPPED_LANES of those. */
  keepOnly(inUse: Iterable<number>): void {
    const keep = new Set(inUse);
    for (const id of keep) this.revive(id);
    for (const [id, lane] of this.ready) {
      if (keep.has(id)) continue;
      this.ready.delete(id);
      this.dropped.set(id, lane);
    }
    for (const id of this.dropped.keys()) {
      if (this.dropped.size <= KEEP_DROPPED_LANES) break;
      this.dropped.delete(id);
    }
  }

  /** How many lanes are kept: in use, and put aside. */
  held(): { ready: number; dropped: number } {
    return { ready: this.ready.size, dropped: this.dropped.size };
  }

  private revive(id: number): LanePoints | undefined {
    const lane = this.dropped.get(id);
    if (!lane) return undefined;
    this.dropped.delete(id);
    this.ready.set(id, lane);
    return lane;
  }
}

/** The id of every lane of the song: the tracks', and the master's. */
export function songLaneIds(song: {
  tracks: readonly { lanes: readonly { laneId: number }[] }[];
  master?: { lanes: readonly { laneId: number }[] };
}): number[] {
  return [...song.tracks.flatMap((t) => t.lanes), ...(song.master?.lanes ?? [])].map((l) => l.laneId);
}
