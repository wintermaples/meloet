// The peak book that waveform drawing reads. To what the bridge's `audioPeaks` / `moreAudioPeaks` kept (audio id, level) it adds
// a number for the WebGL side to tell them apart (`WaveSource.id`). Missing levels are asked for; when they come, `version` goes up to redraw.
// Growing ones (audio being recorded, a file still loading) fetch more about 10 times a second, only while used
// (docs/architecture.md 22-17: waveforms need not arrive as fast as keys).

import { audioPeaks, moreAudioPeaks, peekAudioPeaks, type LevelPeaks } from "../../bridge";
import { PEAK_LEVELS } from "../../bridge/types";
import { newWaveSourceId, type WaveSource } from "../../gl/wave";

/** Interval (ms) for fetching more. */
export const WAVE_POLL_MS = 100;
/** Ones not used for this long stop fetching more (ms). */
const IDLE_MS = 500;
/** Wait (ms) before asking again for peaks that were not ready. Doubles each time, up to this. */
const RETRY_MS = [500, 1000, 2000, 4000, 8000];

interface Entry {
  serial: number;
  audio: number;
  level: number;
  source: WaveSource;
  usedAt: number;
  /** A song clip used it (redraw when it grows). The growing end of a clip being recorded is read every frame, so it needs no notice. */
  loud: boolean;
}

export interface WaveFetch {
  get(audio: number, level: number): Promise<LevelPeaks>;
  more(audio: number, level: number): Promise<LevelPeaks>;
  peek(audio: number, level: number): LevelPeaks | undefined;
}

const bridge: WaveFetch = { get: audioPeaks, more: moreAudioPeaks, peek: peekAudioPeaks };
const keyOf = (audio: number, level: number) => audio * PEAK_LEVELS.length + level;

export class WaveBook {
  /** What to draw may have changed: a new level came, or a song clip's peaks grew. */
  version = $state(0);
  private readonly entries = new Map<number, Entry>();
  private readonly asking = new Set<number>();
  /** The time from which a not-ready one may be asked for again. */
  private readonly failed = new Map<number, number>();
  /** How many times it was not ready (the wait doubles). */
  private readonly tries = new Map<number, number>();
  private timer = 0;

  constructor(
    private readonly fetch: WaveFetch = bridge,
    private readonly now: () => number = () => performance.now(),
  ) {}

  /**
   * What is needed to draw audio `audio` of song `serial` at level `level`. If that level is missing, ask for it and return
   * the nearest level there is (null if none). `quiet`: do not raise `version` when it grows (for places that read again every frame).
   */
  sourceFor(serial: number, audio: number, level: number, quiet = false): { source: WaveSource; framesPerPeak: number } | null {
    let peaks = this.fetch.peek(audio, level);
    if (!peaks) {
      this.ask(audio, level);
      for (let away = 1; away < PEAK_LEVELS.length && !peaks; away++) {
        peaks = (level - away >= 0 ? this.fetch.peek(audio, level - away) : undefined) ?? (level + away < PEAK_LEVELS.length ? this.fetch.peek(audio, level + away) : undefined);
      }
      if (!peaks) return null;
    }
    const key = keyOf(audio, peaks.level);
    let e = this.entries.get(key);
    if (!e || e.serial !== serial || peaks.count < e.source.peaks.count || peaks.channels !== e.source.peaks.channels) {
      e = { serial, audio, level: peaks.level, source: { id: newWaveSourceId(), peaks, stamp: 0 }, usedAt: 0, loud: false };
      this.entries.set(key, e);
    } else if (e.source.peaks !== peaks) {
      e.source.peaks = peaks;
      e.source.stamp += 1;
    }
    e.usedAt = this.now();
    if (!quiet) e.loud = true;
    if (peaks.growing) this.start();
    return { source: e.source, framesPerPeak: peaks.framesPerPeak };
  }

  private ask(audio: number, level: number): void {
    const key = keyOf(audio, level);
    if (this.asking.has(key)) return;
    if (this.failed.has(key)) return this.start();
    this.asking.add(key);
    this.fetch.get(audio, level).then(
      () => {
        this.asking.delete(key);
        this.tries.delete(key);
        this.version += 1;
      },
      () => {
        // Not ready yet (loading just started, or recording just started). Try again later.
        this.asking.delete(key);
        const tries = (this.tries.get(key) ?? -1) + 1;
        this.tries.set(key, tries);
        this.failed.set(key, this.now() + RETRY_MS[Math.min(tries, RETRY_MS.length - 1)]!);
        this.start();
      },
    );
  }

  private start(): void {
    if (this.timer === 0) this.timer = window.setInterval(() => this.poll(), WAVE_POLL_MS);
  }

  /** Fetch more of the growing ones in use now. When a waiting one's time comes, make the drawing side ask again. */
  poll(): void {
    const now = this.now();
    let busy = false;
    for (const e of this.entries.values()) {
      if (!e.source.peaks.growing || now - e.usedAt > IDLE_MS) continue;
      busy = true;
      const loud = e.loud;
      e.loud = false;
      const had = e.source.peaks;
      this.fetch.more(e.audio, e.level).then(
        (peaks) => {
          if (peaks !== had && loud) this.version += 1;
        },
        () => {},
      );
    }
    for (const [key, at] of this.failed) {
      if (now < at) {
        busy = true;
        continue;
      }
      // Its time came: run the drawing side. If it is still needed, that side asks again.
      this.failed.delete(key);
      this.version += 1;
    }
    if (!busy) this.stop();
  }

  /** The song changed, or the band is gone. */
  stop(): void {
    clearInterval(this.timer);
    this.timer = 0;
  }

  /**
   * When the song changes: let go of other songs' entries (numbers start at 0 per song) and of peaks the bridge no longer holds
   * (let go at the `PeakStore` limit). The amount kept stays the bridge's limit.
   */
  trim(serial: number): void {
    for (const [key, e] of this.entries) if (e.serial !== serial || this.fetch.peek(e.audio, e.level) === undefined) this.entries.delete(key);
  }

  held(): number {
    return this.entries.size;
  }
}
