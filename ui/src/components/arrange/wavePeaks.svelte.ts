// 波形を描く所が読む、ピークの帳面。橋の `audioPeaks` ・ `moreAudioPeaks` でためた物（音の id、段）に、
// WebGL の係が見分けるための番号（`WaveSource.id`）をつける。まだない段は頼み、来たら `version` を上げて描きなおさせる。
// ふえていく物（録っている歌、読みこんでいる途中のファイル）は、使われている間だけ 1 秒に 10 回ほど続きを取りにいく
// （docs/architecture.md 22-17: 波形は鍵のようにすぐ届かなくてよい）。

import { audioPeaks, moreAudioPeaks, peekAudioPeaks, type LevelPeaks } from "../../bridge";
import { PEAK_LEVELS } from "../../bridge/types";
import { newWaveSourceId, type WaveSource } from "../../gl/wave";

/** 続きを取りにいく間（ms）。 */
export const WAVE_POLL_MS = 100;
/** これだけ使われなかった物は、続きを取りにいかない（ms）。 */
const IDLE_MS = 500;
/** まだできていなかったピークを、もう一度頼むまでの間（ms）。頼むたびに倍、ここまで。 */
const RETRY_MS = [500, 1000, 2000, 4000, 8000];

interface Entry {
  serial: number;
  audio: number;
  level: number;
  source: WaveSource;
  usedAt: number;
  /** 曲のクリップが使った（ふえたら描きなおす）。録っているクリップの先は毎コマ読むので、知らせは要らない。 */
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
  /** 描く物が変わったかもしれない: 新しい段が来た、曲のクリップのピークがふえた。 */
  version = $state(0);
  private readonly entries = new Map<number, Entry>();
  private readonly asking = new Set<number>();
  /** できていなかった物を、次に頼んでよい時刻。 */
  private readonly failed = new Map<number, number>();
  /** できていなかった回数（待ちを倍にしていく）。 */
  private readonly tries = new Map<number, number>();
  private timer = 0;

  constructor(
    private readonly fetch: WaveFetch = bridge,
    private readonly now: () => number = () => performance.now(),
  ) {}

  /**
   * 曲 `serial` の音 `audio` を、段 `level` で描くための物。その段がまだなければ頼んで、いまある段のうち
   * いちばん近い物を返す（なければ null）。`quiet`: ふえても `version` を上げない（毎コマ読みなおす所）。
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
        // まだできていない（読みこみが始まったばかり、録りはじめ）。あとでもう一度。
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

  /** いま使われている、ふえていく物の続きを取りにいく。待っていた物の時刻が来たら、描く所にもう一度頼ませる。 */
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
      // 時刻が来た: 描く所を回す。まだ要るなら、そこがもう一度頼む。
      this.failed.delete(key);
      this.version += 1;
    }
    if (!busy) this.stop();
  }

  /** 曲が変わった ・ 帯がなくなった。 */
  stop(): void {
    clearInterval(this.timer);
    this.timer = 0;
  }

  /**
   * 曲が変わったときに: ほかの曲の物（番号は曲ごとに 0 から）と、橋がもう持っていないピーク（`PeakStore` の上限で
   * 手ばなした物）を手ばなす。ためる量は、橋の上限のまま。
   */
  trim(serial: number): void {
    for (const [key, e] of this.entries) if (e.serial !== serial || this.fetch.peek(e.audio, e.level) === undefined) this.entries.delete(key);
  }

  held(): number {
    return this.entries.size;
  }
}
