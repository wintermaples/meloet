// The peaks of an audio file: the reply of the `peaks` invoke (binary, version 1, little-endian;
// docs/architecture.md 22-17), and the store that keeps them. An audio file never changes, so its
// peaks are kept by (audio id, level) until another song comes.

import { PEAK_LEVELS, PEAKS_HEADER_LEN, PEAKS_KIND, PEAKS_MAX_COUNT, PEAKS_VERSION } from "./generated/audio";

const FLAG_GROWING = 1;

/** One reply: the peaks `first .. first + count` of a level. */
export interface PeaksPiece {
  audio: number;
  /** An index into PEAK_LEVELS. */
  level: number;
  /** 1 or 2. */
  channels: number;
  /** The file is being recorded or read: `total` goes up, and the last peak may still change. */
  growing: boolean;
  first: number;
  count: number;
  /** How many peaks of this level the file has now. */
  total: number;
  /** For each peak, for each channel: the smallest, then the largest sample (−32768..32767). */
  values: Int16Array;
}

export function decodePeaks(bytes: ArrayBuffer): PeaksPiece {
  if (bytes.byteLength < PEAKS_HEADER_LEN) throw new Error(`peaks: ${bytes.byteLength} bytes, shorter than the header`);
  const view = new DataView(bytes);
  if (view.getUint8(0) !== PEAKS_KIND) throw new Error(`peaks: kind ${view.getUint8(0)}, expected ${PEAKS_KIND}`);
  if (view.getUint8(1) !== PEAKS_VERSION) throw new Error(`peaks: version ${view.getUint8(1)}, expected ${PEAKS_VERSION}`);
  const level = view.getUint8(2);
  const channels = view.getUint8(3);
  if (level >= PEAK_LEVELS.length) throw new Error(`peaks: level ${level}`);
  if (channels !== 1 && channels !== 2) throw new Error(`peaks: ${channels} channels`);
  const count = view.getUint32(16, true);
  const expected = PEAKS_HEADER_LEN + count * channels * 4;
  if (bytes.byteLength !== expected) throw new Error(`peaks: ${bytes.byteLength} bytes, the header says ${expected}`);
  const values = new Int16Array(count * channels * 2);
  for (let i = 0; i < values.length; i++) values[i] = view.getInt16(PEAKS_HEADER_LEN + 2 * i, true);
  return {
    audio: view.getUint32(8, true),
    level,
    channels,
    growing: (view.getUint8(4) & FLAG_GROWING) !== 0,
    first: view.getUint32(12, true),
    count,
    total: view.getUint32(20, true),
    values,
  };
}

/** Writes the peaks binary (the fake and the tests). */
export function encodePeaks(p: Omit<PeaksPiece, "count">): ArrayBuffer {
  const count = p.values.length / (p.channels * 2);
  const bytes = new ArrayBuffer(PEAKS_HEADER_LEN + p.values.length * 2);
  const view = new DataView(bytes);
  view.setUint8(0, PEAKS_KIND);
  view.setUint8(1, PEAKS_VERSION);
  view.setUint8(2, p.level);
  view.setUint8(3, p.channels);
  view.setUint8(4, p.growing ? FLAG_GROWING : 0);
  view.setUint32(8, p.audio, true);
  view.setUint32(12, p.first, true);
  view.setUint32(16, count, true);
  view.setUint32(20, p.total, true);
  p.values.forEach((v, i) => view.setInt16(PEAKS_HEADER_LEN + 2 * i, v, true));
  return bytes;
}

/** Every peak of one level of an audio file that is here. */
export interface LevelPeaks {
  audio: number;
  level: number;
  channels: number;
  /** Frames of the file to a peak. */
  framesPerPeak: number;
  /** More may come: ask again with `PeakStore.more`. */
  growing: boolean;
  /** How many peaks `values` holds. */
  count: number;
  values: Int16Array;
}

/** The level to draw with: the coarsest whose peaks are no wider than `framesPerPx` frames; the finest when even that is wider. */
export function levelFor(framesPerPx: number): number {
  let level = 0;
  for (let i = 1; i < PEAK_LEVELS.length; i++) if (PEAK_LEVELS[i]! <= framesPerPx) level = i;
  return level;
}

/** Peaks of files no clip uses any more are kept up to this many bytes, the newest first (as takes: `KEEP_DROPPED_TAKES`). */
export const KEEP_DROPPED_PEAK_BYTES = 4 * 1024 * 1024;

type Fetch = (audio: number, level: number, first: number, count: number) => Promise<ArrayBuffer>;

const keyOf = (audio: number, level: number) => audio * PEAK_LEVELS.length + level;

/** Fetches a level of a file once, in pieces of PEAKS_MAX_COUNT, and keeps it. */
export class PeakStore {
  private readonly ready = new Map<number, LevelPeaks>();
  private readonly pending = new Map<number, Promise<LevelPeaks>>();
  /** Levels of files that are in no clip now, oldest first. */
  private readonly dropped = new Map<number, LevelPeaks>();
  /** Goes up on `clear`: an answer asked for before it may be of the other song. */
  private generation = 0;

  constructor(
    private readonly fetch: Fetch,
    private readonly keepDroppedBytes = KEEP_DROPPED_PEAK_BYTES,
  ) {}

  /** The peaks if they are here already; for drawing a frame without waiting. */
  peek(audio: number, level: number): LevelPeaks | undefined {
    const key = keyOf(audio, level);
    return this.ready.get(key) ?? this.dropped.get(key);
  }

  /** Every peak of the level that there is now. Rejects while the peaks are not made; ask again later. */
  get(audio: number, level: number): Promise<LevelPeaks> {
    const key = keyOf(audio, level);
    const had = this.ready.get(key) ?? this.revive(key);
    if (had) return Promise.resolve(had);
    return this.load(audio, level, undefined);
  }

  /** For a level that is `growing`: fetches what came since, from its last peak on (that one may have changed). */
  more(audio: number, level: number): Promise<LevelPeaks> {
    const had = this.peek(audio, level);
    if (had && !had.growing) return Promise.resolve(had);
    return this.load(audio, level, had);
  }

  private load(audio: number, level: number, had: LevelPeaks | undefined): Promise<LevelPeaks> {
    const key = keyOf(audio, level);
    const waiting = this.pending.get(key);
    if (waiting) return waiting;
    const generation = this.generation;
    const asked: Promise<LevelPeaks> = this.fetchFrom(audio, level, had)
      .then((peaks) => {
        if (generation !== this.generation) return this.get(audio, level);
        this.dropped.delete(key);
        this.ready.set(key, peaks);
        return peaks;
      })
      .finally(() => {
        if (this.pending.get(key) === asked) this.pending.delete(key);
      });
    this.pending.set(key, asked);
    return asked;
  }

  private async fetchFrom(audio: number, level: number, had: LevelPeaks | undefined): Promise<LevelPeaks> {
    let channels = had?.channels ?? 0;
    // The last peak of a growing level is fetched again.
    let have = had ? Math.max(0, had.count - 1) : 0;
    const pieces: Int16Array[] = had ? [had.values.subarray(0, have * had.channels * 2)] : [];
    let growing = false;
    for (;;) {
      const piece = decodePeaks(await this.fetch(audio, level, have, PEAKS_MAX_COUNT));
      if (piece.audio !== audio || piece.level !== level || piece.first !== have) throw new Error(`peaks: asked for ${audio}/${level} from ${have}`);
      if (channels !== 0 && piece.channels !== channels) throw new Error("peaks: the channels changed");
      channels = piece.channels;
      pieces.push(piece.values);
      have += piece.count;
      growing = piece.growing;
      if (piece.count === 0 || have >= piece.total) break;
    }
    const values = new Int16Array(have * channels * 2);
    let at = 0;
    for (const piece of pieces) {
      values.set(piece, at);
      at += piece.length;
    }
    return { audio, level, channels, framesPerPeak: PEAK_LEVELS[level]!, growing, count: have, values };
  }

  /** Lets go of everything, also what is on the way: another song came, and audio ids start again per song. */
  clear(): void {
    this.generation += 1;
    this.pending.clear();
    this.ready.clear();
    this.dropped.clear();
  }

  /** Keeps every level of the files `inUse`; puts the others aside, keeping the newest up to `keepDroppedBytes`. */
  keepOnly(inUse: Iterable<number>): void {
    const keep = new Set(inUse);
    for (const key of [...this.dropped.keys()]) if (keep.has(Math.floor(key / PEAK_LEVELS.length))) this.revive(key);
    for (const [key, peaks] of this.ready) {
      if (keep.has(peaks.audio)) continue;
      this.ready.delete(key);
      this.dropped.set(key, peaks);
    }
    let bytes = 0;
    for (const peaks of this.dropped.values()) bytes += peaks.values.byteLength;
    for (const [key, peaks] of this.dropped) {
      if (bytes <= this.keepDroppedBytes) break;
      this.dropped.delete(key);
      bytes -= peaks.values.byteLength;
    }
  }

  /** How much is kept: levels in use and put aside, and their bytes. */
  held(): { ready: number; dropped: number; bytes: number } {
    let bytes = 0;
    for (const peaks of this.ready.values()) bytes += peaks.values.byteLength;
    for (const peaks of this.dropped.values()) bytes += peaks.values.byteLength;
    return { ready: this.ready.size, dropped: this.dropped.size, bytes };
  }

  private revive(key: number): LevelPeaks | undefined {
    const peaks = this.dropped.get(key);
    if (!peaks) return undefined;
    this.dropped.delete(key);
    this.ready.set(key, peaks);
    return peaks;
  }
}

/** Every audio file the song's clips play (the shown take of each clip), and the files being recorded. */
export function songAudioIds(
  song: { tracks: readonly { clips: readonly { audio: { audio: number } | null }[] }[] },
  recording: readonly { audio: number }[] = [],
): number[] {
  const ids = song.tracks.flatMap((t) => t.clips.flatMap((c) => (c.audio ? [c.audio.audio] : [])));
  return [...new Set([...ids, ...recording.map((r) => r.audio)])];
}
