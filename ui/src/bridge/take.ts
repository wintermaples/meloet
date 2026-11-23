// The notes of one take, binary, version 2, little-endian: the reply of `take_notes`. The layout
// is fixed in crates/meloet-protocol/src/take.rs (table: docs/architecture.md 3-4).
// A take never changes once made, so it is fetched once and kept by its id.

export const TAKE_KIND = 2;
export const TAKE_VERSION = 2;
export const TAKE_HEADER_LEN = 24;
export const TAKE_NOTE_LEN = 16;
const CONTROL_LEN = 8;

export const CONTROL_CC = 0;
export const CONTROL_PITCH_BEND = 1;
/** `froms[i]` of a note that did not come from the origin take. */
export const NO_FROM = 0xffffffff;

/** One array per field, so the WebGL side can upload them as they are. Ticks count from the take's tick 0. */
export interface TakeNotes {
  id: number;
  length: number;
  /** The recorded take a quantized take was made from (draw its notes as 「そろえる前の位置」); 0 for none. */
  origin: number;
  /** In order of start. */
  starts: Uint32Array;
  lengths: Uint32Array;
  keys: Uint8Array;
  velocities: Uint8Array;
  /** The index of the note of the origin take each note came from; NO_FROM for none. */
  froms: Uint32Array;
  /** In order of tick. */
  controlTicks: Uint32Array;
  /** CONTROL_CC or CONTROL_PITCH_BEND. */
  controlKinds: Uint8Array;
  /** The CC number; 0 for pitch bend. */
  controlNumbers: Uint8Array;
  /** CC 0..127, pitch bend -8192..8191. */
  controlValues: Int16Array;
}

export function decodeTake(data: ArrayBuffer | ArrayBufferView): TakeNotes {
  const view = ArrayBuffer.isView(data)
    ? new DataView(data.buffer, data.byteOffset, data.byteLength)
    : new DataView(data);
  if (view.byteLength < TAKE_HEADER_LEN) throw new Error(`take is ${view.byteLength} bytes, shorter than the header`);
  if (view.getUint8(0) !== TAKE_KIND) throw new Error(`take kind ${view.getUint8(0)}, expected ${TAKE_KIND}`);
  if (view.getUint8(1) !== TAKE_VERSION) {
    throw new Error(`take version ${view.getUint8(1)}, expected ${TAKE_VERSION}`);
  }
  const n = view.getUint32(12, true);
  const m = view.getUint32(16, true);
  const expected = TAKE_HEADER_LEN + TAKE_NOTE_LEN * n + CONTROL_LEN * m;
  if (view.byteLength !== expected) throw new Error(`take is ${view.byteLength} bytes, expected ${expected}`);

  const take: TakeNotes = {
    id: view.getUint32(4, true),
    length: view.getUint32(8, true),
    origin: view.getUint32(20, true),
    starts: new Uint32Array(n),
    lengths: new Uint32Array(n),
    keys: new Uint8Array(n),
    velocities: new Uint8Array(n),
    froms: new Uint32Array(n),
    controlTicks: new Uint32Array(m),
    controlKinds: new Uint8Array(m),
    controlNumbers: new Uint8Array(m),
    controlValues: new Int16Array(m),
  };
  for (let i = 0, p = TAKE_HEADER_LEN; i < n; i++, p += TAKE_NOTE_LEN) {
    take.starts[i] = view.getUint32(p, true);
    take.lengths[i] = view.getUint32(p + 4, true);
    take.keys[i] = view.getUint8(p + 8);
    take.velocities[i] = view.getUint8(p + 9);
    take.froms[i] = view.getUint32(p + 12, true);
  }
  for (let i = 0, p = TAKE_HEADER_LEN + TAKE_NOTE_LEN * n; i < m; i++, p += CONTROL_LEN) {
    const kind = view.getUint8(p + 4);
    if (kind !== CONTROL_CC && kind !== CONTROL_PITCH_BEND) throw new Error(`take control kind ${kind}`);
    take.controlTicks[i] = view.getUint32(p, true);
    take.controlKinds[i] = kind;
    take.controlNumbers[i] = view.getUint8(p + 5);
    take.controlValues[i] = view.getInt16(p + 6, true);
  }
  return take;
}

/** How many takes no clip points at any more are still kept (newest first): undo often brings them back. */
export const KEEP_DROPPED_TAKES = 16;

/**
 * Fetches each take once. A fetch that fails is forgotten, so the next `get` tries again.
 * Every edit of notes makes a new take, so `keepOnly` lets go of the ones the song no longer points at.
 */
export class TakeStore {
  private readonly pending = new Map<number, Promise<TakeNotes>>();
  private readonly ready = new Map<number, TakeNotes>();
  /** Let go of, oldest first. Rust still has them, so dropping one too many only costs a fetch. */
  private readonly dropped = new Map<number, TakeNotes>();

  /** Goes up on `clear`: an answer asked for before it may be of the other song. */
  private generation = 0;

  constructor(private readonly fetch: (id: number) => Promise<ArrayBuffer>) {}

  get(id: number): Promise<TakeNotes> {
    const had = this.ready.get(id) ?? this.revive(id);
    if (had) return Promise.resolve(had);
    let p = this.pending.get(id);
    if (!p) {
      const generation = this.generation;
      const asked: Promise<TakeNotes> = this.fetch(id)
        .then((bytes) => {
          if (generation !== this.generation) return this.get(id);
          const take = decodeTake(bytes);
          if (take.id !== id) throw new Error(`asked for take ${id}, got ${take.id}`);
          this.ready.set(id, take);
          return take;
        })
        .finally(() => {
          if (this.pending.get(id) === asked) this.pending.delete(id);
        });
      p = asked;
      this.pending.set(id, p);
    }
    return p;
  }

  /** Lets go of every take, also the ones on the way: another song came, and take ids start again per song. */
  clear(): void {
    this.generation += 1;
    this.pending.clear();
    this.ready.clear();
    this.dropped.clear();
  }

  /** The take if it is here already; for drawing a frame without waiting. */
  peek(id: number): TakeNotes | undefined {
    return this.ready.get(id) ?? this.dropped.get(id);
  }

  /**
   * Keeps the takes `inUse` and their origin takes (the quantized take's recorded one); puts every other
   * take aside, keeping only the newest KEEP_DROPPED_TAKES of those.
   */
  keepOnly(inUse: Iterable<number>): void {
    const keep = new Set<number>();
    for (const id of inUse) {
      keep.add(id);
      const take = this.ready.get(id) ?? this.dropped.get(id);
      if (take && take.origin !== 0) keep.add(take.origin);
    }
    for (const id of keep) this.revive(id);
    for (const [id, take] of this.ready) {
      if (keep.has(id)) continue;
      this.ready.delete(id);
      this.dropped.set(id, take);
    }
    for (const id of this.dropped.keys()) {
      if (this.dropped.size <= KEEP_DROPPED_TAKES) break;
      this.dropped.delete(id);
    }
  }

  /** How many takes are kept: in use, and put aside. */
  held(): { ready: number; dropped: number } {
    return { ready: this.ready.size, dropped: this.dropped.size };
  }

  private revive(id: number): TakeNotes | undefined {
    const take = this.dropped.get(id);
    if (!take) return undefined;
    this.dropped.delete(id);
    this.ready.set(id, take);
    return take;
  }
}

/** Every take id the song's clips point at (the shown take of each clip). */
export function songTakeIds(song: { tracks: readonly { clips: readonly { takeId: number }[] }[] }): number[] {
  return song.tracks.flatMap((t) => t.clips.map((c) => c.takeId));
}
