// 曲のテンポと拍子の地図（docs/architecture.md 21-1）。crates/meloet-model/src/tempo_map.rs と同じ式で、
// 同じ答えを出す（tempoMap.table.json と相対 1e-9 まで合う。ui/tests/tempoMap.test.ts）。DOM も Tauri も知らない。
//
// テンポは、拍子によらず「1 分間の 4 分音符の数」（22-2）。だんだんはティックに対してまっすぐ。
// 0 より前は、曲の頭のテンポと拍子がそのまま前へ続く。
//
// 作ったあとは変えない。どの関数も、呼ぶたびに配列を作らない（60 Hz で使う）。いくつも値を返す関数は、
// 呼ぶ側の入れ物（out）に書く。

import type { TimelineView } from "../bridge/generated/TimelineView";
import { TICKS_PER_QUARTER } from "./ticks";

/** 地図を作るのに要る所（`timeline` の区画をそのまま渡せる）。 */
export type TimelineShape = Pick<TimelineView, "tempoBpm" | "meterNum" | "meterDen" | "headRamp" | "tempoPoints" | "meterChanges">;

export interface MeterAt {
  num: number;
  den: number;
  /** 小節の番号。0 から。 */
  bar: number;
  /** その小節の頭のティック。 */
  barTick: number;
}

/** 表示の形: 小節と拍は 1 から、ティックはその拍の中のティックそのまま。 */
export interface BarBeatTick {
  bar: number;
  beat: number;
  tick: number;
}

export interface Beat {
  /** 拍の頭。曲の頭より前は負。 */
  tick: number;
  /** 小節の 1 拍目。 */
  downbeat: boolean;
}

/** `ln(1 + x) / x`。0 で段なく 1 になる。 */
const ln1pOver = (x: number) => (x === 0 ? 1 : Math.log1p(x) / x);
/** `(exp(y) − 1) / y`。0 で段なく 1 になる。 */
const expm1Over = (y: number) => (y === 0 ? 1 : Math.expm1(y) / y);

const Q = TICKS_PER_QUARTER;

const secondsOf = (u: number, a: number, s: number) => ((u * 60) / (Q * a)) * ln1pOver((s * u) / a);

function ticksOf(seconds: number, a: number, s: number): number {
  const minutes = seconds / 60;
  return a * Q * minutes * expm1Over(s * Q * minutes);
}

/** メトロノームが鳴る間のティック: 6/8 ・ 9/8 ・ 12/8 は付点 4 分、そのほかは 1 拍（crates/meloet-model の `Meter::ticks_per_click`）。 */
export function ticksPerClick(num: number, den: number): number {
  const perBeat = (Q * 4) / den;
  return den === 8 && (num === 6 || num === 9 || num === 12) ? 3 * perBeat : perBeat;
}

/** 1 小節でメトロノームが鳴る数（6/8 は 2、5/8 は 5、4/4 は 4）。 */
export function clicksPerBar(num: number, den: number): number {
  return (num * ((Q * 4) / den)) / ticksPerClick(num, den);
}

/** `xs[i] <= x` である最後の i（なければ 0）。xs は小さい順で、空でない。 */
function lastAtOrBefore(xs: Float64Array, x: number): number {
  let lo = 0;
  let hi = xs.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (xs[mid]! <= x) lo = mid + 1;
    else hi = mid;
  }
  return lo > 0 ? lo - 1 : 0;
}

export class TempoMap {
  // かけら（1 つの拍子と 1 本のテンポの線が続く所）。u は tick0 からのティック。
  private readonly pTick0: Float64Array;
  private readonly pSec0: Float64Array;
  /** tick0 でのテンポ。 */
  private readonly pA: Float64Array;
  /** 1 ティックあたりのテンポの増え。だんだんでなければ 0。 */
  private readonly pS: Float64Array;
  // 拍子が同じ小節の続き。
  private readonly rTick0: Float64Array;
  private readonly rBar0: Float64Array;
  private readonly rNum: Float64Array;
  private readonly rDen: Float64Array;
  private readonly rPerBeat: Float64Array;
  private readonly rPerBar: Float64Array;

  /**
   * 形のくずれた入力でも、Rust と同じ 1 つの地図になる: 順にならべ、同じ所に 2 つあれば後ろの物を残し、
   * ティック 0 の点と小節 0 の旗は入れない（曲の頭は別に持っている）。
   */
  constructor(timeline: TimelineShape) {
    const changes = timeline.meterChanges
      .filter((c) => c.bar > 0)
      .map((c, i) => ({ c, i }))
      .sort((x, y) => x.c.bar - y.c.bar || x.i - y.i)
      .map((x) => x.c)
      .filter((c, i, all) => all[i + 1]?.bar !== c.bar);
    const rn = changes.length + 1;
    this.rTick0 = new Float64Array(rn);
    this.rBar0 = new Float64Array(rn);
    this.rNum = new Float64Array(rn);
    this.rDen = new Float64Array(rn);
    this.rPerBeat = new Float64Array(rn);
    this.rPerBar = new Float64Array(rn);
    for (let i = 0; i < rn; i++) {
      const num = i === 0 ? timeline.meterNum : changes[i - 1]!.meterNum;
      const den = i === 0 ? timeline.meterDen : changes[i - 1]!.meterDen;
      const bar0 = i === 0 ? 0 : changes[i - 1]!.bar;
      this.rNum[i] = num;
      this.rDen[i] = den;
      this.rPerBeat[i] = (TICKS_PER_QUARTER * 4) / den;
      this.rPerBar[i] = num * this.rPerBeat[i]!;
      this.rBar0[i] = bar0;
      this.rTick0[i] = i === 0 ? 0 : this.rTick0[i - 1]! + (bar0 - this.rBar0[i - 1]!) * this.rPerBar[i - 1]!;
    }

    const points = [
      { tick: 0, tempoBpm: timeline.tempoBpm, ramp: timeline.headRamp },
      ...timeline.tempoPoints
        .filter((p) => p.tick > 0)
        .map((p, i) => ({ p, i }))
        .sort((x, y) => x.p.tick - y.p.tick || x.i - y.i)
        .map((x) => x.p)
        .filter((p, i, all) => all[i + 1]?.tick !== p.tick),
    ];

    const cuts = [...new Set([...points.map((p) => p.tick), ...this.rTick0])].sort((x, y) => x - y);
    const n = cuts.length;
    this.pTick0 = new Float64Array(n);
    this.pSec0 = new Float64Array(n);
    this.pA = new Float64Array(n);
    this.pS = new Float64Array(n);
    let pi = 0;
    let sec0 = 0;
    for (let i = 0; i < n; i++) {
      const cut = cuts[i]!;
      while (pi + 1 < points.length && points[pi + 1]!.tick <= cut) pi++;
      const point = points[pi]!;
      const next = points[pi + 1];
      const s = next && point.ramp ? (next.tempoBpm - point.tempoBpm) / (next.tick - point.tick) : 0;
      const a = point.tempoBpm + s * (cut - point.tick);
      this.pTick0[i] = cut;
      this.pSec0[i] = sec0;
      this.pA[i] = a;
      this.pS[i] = s;
      if (i + 1 < n) sec0 += secondsOf(cuts[i + 1]! - cut, a, s);
    }
  }

  /** 地図がいくつのかけらに分かれているか（測るため）。 */
  get pieceCount(): number {
    return this.pTick0.length;
  }

  /** 曲の頭から tick までの秒。頭より前は負。 */
  secondsAt(tick: number): number {
    if (tick < 0) return secondsOf(tick, this.pA[0]!, 0);
    const i = lastAtOrBefore(this.pTick0, tick);
    return this.pSec0[i]! + secondsOf(tick - this.pTick0[i]!, this.pA[i]!, this.pS[i]!);
  }

  /** 曲の頭から seconds 秒の所のティック（小数）。 */
  tickAt(seconds: number): number {
    if (seconds < 0) return ticksOf(seconds, this.pA[0]!, 0);
    const i = lastAtOrBefore(this.pSec0, seconds);
    return this.pTick0[i]! + ticksOf(seconds - this.pSec0[i]!, this.pA[i]!, this.pS[i]!);
  }

  /** 運びに出すテンポ: 1 分間の 4 分音符の数。 */
  tempoAt(tick: number): number {
    if (tick < 0) return this.pA[0]!;
    const i = lastAtOrBefore(this.pTick0, tick);
    return this.pA[i]! + this.pS[i]! * (tick - this.pTick0[i]!);
  }

  /** その所の拍子と、その小節。小数のティックは切りすて、0 より前は 0 として読む。 */
  meterAt(tick: number, out: MeterAt = { num: 0, den: 0, bar: 0, barTick: 0 }): MeterAt {
    const t = Math.max(0, Math.floor(tick));
    const r = lastAtOrBefore(this.rTick0, t);
    const perBar = this.rPerBar[r]!;
    const bars = Math.floor((t - this.rTick0[r]!) / perBar);
    out.num = this.rNum[r]!;
    out.den = this.rDen[r]!;
    out.bar = this.rBar0[r]! + bars;
    out.barTick = this.rTick0[r]! + bars * perBar;
    return out;
  }

  /** 小節 bar（0 から）の頭のティック。 */
  tickOfBar(bar: number): number {
    const r = lastAtOrBefore(this.rBar0, bar);
    return (bar - this.rBar0[r]!) * this.rPerBar[r]! + this.rTick0[r]!;
  }

  /** 表示の「小節 . 拍 . ティック」。小数のティックは切りすて、0 より前は 0 として読む。 */
  barBeatTick(tick: number, out: BarBeatTick = { bar: 0, beat: 0, tick: 0 }): BarBeatTick {
    const t = Math.max(0, Math.floor(tick));
    const r = lastAtOrBefore(this.rTick0, t);
    const perBeat = this.rPerBeat[r]!;
    const perBar = this.rPerBar[r]!;
    const from = t - this.rTick0[r]!;
    const inBar = from % perBar;
    out.bar = this.rBar0[r]! + Math.floor(from / perBar) + 1;
    out.beat = Math.floor(inBar / perBeat) + 1;
    out.tick = inBar % perBeat;
    return out;
  }

  /** tick か、その後ろの、最初の拍の頭。曲の頭より前は、頭の拍子の拍がそのまま前へ続く。 */
  nextBeat(tick: number, out: Beat = { tick: 0, downbeat: false }): Beat {
    const r = lastAtOrBefore(this.rTick0, tick);
    return this.nextStep(tick, r, this.rPerBeat[r]!, out);
  }

  /** tick か、その後ろの、メトロノームが最初に鳴る所（`ticksPerClick`）。downbeat は強い音。0 より前は `nextBeat` と同じ。 */
  nextClick(tick: number, out: Beat = { tick: 0, downbeat: false }): Beat {
    const r = lastAtOrBefore(this.rTick0, tick);
    return this.nextStep(tick, r, ticksPerClick(this.rNum[r]!, this.rDen[r]!), out);
  }

  /** その所の拍子で、1 小節にメトロノームが鳴る数。小数のティックは切りすて、0 より前は 0 として読む。 */
  clicksPerBarAt(tick: number): number {
    const r = lastAtOrBefore(this.rTick0, Math.max(0, Math.floor(tick)));
    return clicksPerBar(this.rNum[r]!, this.rDen[r]!);
  }

  /** step は区間 r の 1 小節を整数個に分ける。 */
  private nextStep(tick: number, r: number, step: number, out: Beat): Beat {
    const steps = this.rPerBar[r]! / step;
    // 区間は小節の整数個なので、終わりをこえて見つけた段は、次の区間の最初の段。
    const n = Math.ceil((tick - this.rTick0[r]!) / step);
    out.tick = this.rTick0[r]! + n * step;
    out.downbeat = ((n % steps) + steps) % steps === 0;
    return out;
  }
}
