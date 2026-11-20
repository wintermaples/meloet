// The peaks of an audio file as a texture, for the wave shader (renderer.ts). No WebGL and no DOM here.
//
// A peak is the smallest and the largest sample of a channel (i16 each), so a texel is the peak as it
// comes: RG16I for one channel, RGBA16I for two. Nothing is converted and nothing is lost; a texel is read
// with texelFetch, so no filtering is needed (whole-number textures have none). The peaks are folded into
// rows of WAVE_TEX_WIDTH, which every WebGL2 has (MAX_TEXTURE_SIZE is at least 2048).

/** The peaks a wave reads: `LevelPeaks` of the bridge has this form. */
export interface WavePeaks {
  /** 1 or 2. */
  channels: number;
  count: number;
  /** A recording going on or a file being read: more come, and the last peak may still change. */
  growing: boolean;
  /** For each peak, for each channel: the smallest, then the largest sample. */
  values: Int16Array;
}

/**
 * One level of one audio file, for as long as it is drawn. `id` never names another file (`newWaveSourceId`);
 * while the file grows, `peaks` is replaced by a longer one whose earlier peaks are the same (but for the last),
 * and `stamp` goes up.
 */
export interface WaveSource {
  id: number;
  peaks: WavePeaks;
  stamp: number;
}

let lastId = 0;
export const newWaveSourceId = () => ++lastId;

export const WAVE_TEX_WIDTH = 2048;
/** With the finest level (256 frames to a peak) at 48 kHz: over six hours. Peaks past the last row are not drawn. */
export const WAVE_TEX_MAX_ROWS = 2048;
export const WAVE_TEX_MAX_PEAKS = WAVE_TEX_WIDTH * WAVE_TEX_MAX_ROWS;
/** Textures the last picture did not use are kept up to this many bytes for each canvas, the newest first. */
export const WAVE_TEX_KEEP_BYTES = 16 * 1024 * 1024;

/** A part of a row, or whole rows, to write: `w` × `h` texels at (`x`, `y`), from the peak `from` on. */
export interface TexWrite {
  x: number;
  y: number;
  w: number;
  h: number;
  from: number;
}

/** The writes that put the peaks `from .. count` into their rows: the rest of a row, whole rows, the start of a row. */
export function texWrites(from: number, count: number): TexWrite[] {
  const out: TexWrite[] = [];
  const W = WAVE_TEX_WIDTH;
  let i = from;
  if (i < count && i % W !== 0) {
    const w = Math.min(count - i, W - (i % W));
    out.push({ x: i % W, y: Math.floor(i / W), w, h: 1, from: i });
    i += w;
  }
  const whole = Math.floor((count - i) / W);
  if (whole > 0) {
    out.push({ x: 0, y: i / W, w: W, h: whole, from: i });
    i += whole * W;
  }
  if (i < count) out.push({ x: 0, y: i / W, w: count - i, h: 1, from: i });
  return out;
}

/**
 * How a texture that holds `had.count` peaks in `had.rows` rows takes `count` peaks: how many rows it must have
 * (another number than `had.rows`: allocate again) and what to write. A growing file gets rows to grow into
 * (twice as many each time), and only what came since is written, from the last peak it had (that one may have changed).
 */
export function texturePlan(had: { rows: number; count: number } | null, count: number, growing: boolean): { rows: number; count: number; writes: TexWrite[] } {
  count = Math.min(count, WAVE_TEX_MAX_PEAKS);
  const needed = Math.max(1, Math.ceil(count / WAVE_TEX_WIDTH));
  let rows = needed;
  if (had && needed <= had.rows) rows = had.rows;
  else if (growing) rows = Math.min(WAVE_TEX_MAX_ROWS, 2 ** Math.ceil(Math.log2(needed)));
  const from = had && rows === had.rows ? Math.max(0, Math.min(had.count, count) - 1) : 0;
  return { rows, count, writes: texWrites(from, count) };
}

/** What `WaveTextures` asks of WebGL (renderer.ts), or of a test. */
export interface TexOps<T> {
  make(): T;
  /** Gives the texture `rows` rows of WAVE_TEX_WIDTH, of one channel (two numbers a texel) or two (four). What it held is gone. */
  alloc(tex: T, channels: number, rows: number): void;
  /** `values` from the element `offset` on. */
  write(tex: T, channels: number, at: TexWrite, values: Int16Array, offset: number): void;
  free(tex: T): void;
}

interface Entry<T> {
  tex: T;
  channels: number;
  rows: number;
  count: number;
  peaks: WavePeaks;
  /** The picture that used it last. */
  used: number;
}

/** The textures of one canvas, by wave source. */
export class WaveTextures<T> {
  private readonly entries = new Map<number, Entry<T>>();
  private picture = 0;

  constructor(
    private readonly ops: TexOps<T>,
    private readonly keepBytes = WAVE_TEX_KEEP_BYTES,
  ) {}

  /** The texture of the source, with every peak the source has now. */
  use(source: WaveSource): T {
    const peaks = source.peaks;
    let e = this.entries.get(source.id);
    if (e && e.channels !== peaks.channels) {
      this.ops.free(e.tex);
      e = undefined;
    }
    if (e && e.peaks === peaks) {
      e.used = this.picture;
      return e.tex;
    }
    const plan = texturePlan(e ? e : null, peaks.count, peaks.growing);
    if (!e) {
      e = { tex: this.ops.make(), channels: peaks.channels, rows: 0, count: 0, peaks, used: this.picture };
      this.entries.set(source.id, e);
    }
    if (plan.rows !== e.rows) this.ops.alloc(e.tex, e.channels, plan.rows);
    for (const w of plan.writes) this.ops.write(e.tex, e.channels, w, peaks.values, w.from * e.channels * 2);
    e.rows = plan.rows;
    e.count = plan.count;
    e.peaks = peaks;
    e.used = this.picture;
    return e.tex;
  }

  /** After a picture: lets go of textures the picture did not use, the oldest first, until the rest is within `keepBytes`. */
  endPicture(): void {
    const now = this.picture++;
    let bytes = this.held().bytes;
    if (bytes <= this.keepBytes) return;
    const idle = [...this.entries].filter(([, e]) => e.used !== now).sort((a, b) => a[1].used - b[1].used);
    for (const [id, e] of idle) {
      if (bytes <= this.keepBytes) break;
      this.ops.free(e.tex);
      this.entries.delete(id);
      bytes -= bytesOf(e);
    }
  }

  /** The context was lost: its textures are gone with it. */
  forget(): void {
    this.entries.clear();
  }

  held(): { textures: number; bytes: number } {
    let bytes = 0;
    for (const e of this.entries.values()) bytes += bytesOf(e);
    return { textures: this.entries.size, bytes };
  }
}

const bytesOf = (e: { rows: number; channels: number }) => e.rows * WAVE_TEX_WIDTH * e.channels * 4;
