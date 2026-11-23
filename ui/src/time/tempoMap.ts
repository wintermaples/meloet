// The map of the song's tempo and time signatures (architecture 21-1). Uses the same formulas as crates/meloet-model/src/tempo_map.rs
// and gives the same answers (matching tempoMap.table.json to a relative 1e-9; ui/tests/tempoMap.test.ts). Knows neither the DOM nor Tauri.
//
// Tempo is always quarter notes per minute, whatever the time signature (22-2). A ramp is linear in ticks.
// Before 0, the tempo and time signature of the song start go on backwards.
//
// Not changed once made. No function makes an array on each call (they are used at 60 Hz). Functions that return several values
// write into the caller's holder (out).

import type { TimelineView } from "../bridge/generated/TimelineView";
import { TICKS_PER_QUARTER } from "./ticks";

/** What is needed to make a map (the `timeline` section can be passed as it is). */
export type TimelineShape = Pick<TimelineView, "tempoBpm" | "meterNum" | "meterDen" | "headRamp" | "tempoPoints" | "meterChanges">;

export interface MeterAt {
  num: number;
  den: number;
  /** The bar number, from 0. */
  bar: number;
  /** The start tick of that bar. */
  barTick: number;
}

/** The shown form: bar and beat from 1, the tick as the tick within its beat. */
export interface BarBeatTick {
  bar: number;
  beat: number;
  tick: number;
}

export interface Beat {
  /** The beat start. Negative before the song start. */
  tick: number;
  /** The first beat of a bar. */
  downbeat: boolean;
}

/** `ln(1 + x) / x`. Goes smoothly to 1 at 0. */
const ln1pOver = (x: number) => (x === 0 ? 1 : Math.log1p(x) / x);
/** `(exp(y) − 1) / y`. Goes smoothly to 1 at 0. */
const expm1Over = (y: number) => (y === 0 ? 1 : Math.expm1(y) / y);

const Q = TICKS_PER_QUARTER;

const secondsOf = (u: number, a: number, s: number) => ((u * 60) / (Q * a)) * ln1pOver((s * u) / a);

function ticksOf(seconds: number, a: number, s: number): number {
  const minutes = seconds / 60;
  return a * Q * minutes * expm1Over(s * Q * minutes);
}

/** Ticks between metronome clicks: a dotted quarter in 6/8, 9/8 and 12/8, one beat otherwise (`Meter::ticks_per_click` in crates/meloet-model). */
export function ticksPerClick(num: number, den: number): number {
  const perBeat = (Q * 4) / den;
  return den === 8 && (num === 6 || num === 9 || num === 12) ? 3 * perBeat : perBeat;
}

/** Metronome clicks in one bar (2 in 6/8, 5 in 5/8, 4 in 4/4). */
export function clicksPerBar(num: number, den: number): number {
  return (num * ((Q * 4) / den)) / ticksPerClick(num, den);
}

/** The last i with `xs[i] <= x` (0 if none). xs is sorted from small and not empty. */
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
  // Pieces (where one time signature and one tempo line go on). u is ticks from tick0.
  private readonly pTick0: Float64Array;
  private readonly pSec0: Float64Array;
  /** The tempo at tick0. */
  private readonly pA: Float64Array;
  /** The tempo rise per tick. 0 when it is not a ramp. */
  private readonly pS: Float64Array;
  // Runs of bars with the same time signature.
  private readonly rTick0: Float64Array;
  private readonly rBar0: Float64Array;
  private readonly rNum: Float64Array;
  private readonly rDen: Float64Array;
  private readonly rPerBeat: Float64Array;
  private readonly rPerBar: Float64Array;

  /**
   * Even badly formed input makes the same single map as Rust: sort, keep the later one when two are at the same place,
   * and leave out the point at tick 0 and the flag at bar 0 (the song start is held apart).
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

  /** How many pieces the map is split into (for measuring). */
  get pieceCount(): number {
    return this.pTick0.length;
  }

  /** Seconds from the song start to tick. Negative before the start. */
  secondsAt(tick: number): number {
    if (tick < 0) return secondsOf(tick, this.pA[0]!, 0);
    const i = lastAtOrBefore(this.pTick0, tick);
    return this.pSec0[i]! + secondsOf(tick - this.pTick0[i]!, this.pA[i]!, this.pS[i]!);
  }

  /** The tick (fractional) at seconds from the song start. */
  tickAt(seconds: number): number {
    if (seconds < 0) return ticksOf(seconds, this.pA[0]!, 0);
    const i = lastAtOrBefore(this.pSec0, seconds);
    return this.pTick0[i]! + ticksOf(seconds - this.pSec0[i]!, this.pA[i]!, this.pS[i]!);
  }

  /** The tempo shown in the transport: quarter notes per minute. */
  tempoAt(tick: number): number {
    if (tick < 0) return this.pA[0]!;
    const i = lastAtOrBefore(this.pTick0, tick);
    return this.pA[i]! + this.pS[i]! * (tick - this.pTick0[i]!);
  }

  /** The time signature and the bar there. A fractional tick is cut down; before 0 is read as 0. */
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

  /** The start tick of bar (from 0). */
  tickOfBar(bar: number): number {
    const r = lastAtOrBefore(this.rBar0, bar);
    return (bar - this.rBar0[r]!) * this.rPerBar[r]! + this.rTick0[r]!;
  }

  /** The shown 「小節 . 拍 . ティック」. A fractional tick is cut down; before 0 is read as 0. */
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

  /** The first beat start at or after tick. Before the song start, the beats of the first time signature go on backwards. */
  nextBeat(tick: number, out: Beat = { tick: 0, downbeat: false }): Beat {
    const r = lastAtOrBefore(this.rTick0, tick);
    return this.nextStep(tick, r, this.rPerBeat[r]!, out);
  }

  /** The first metronome click at or after tick (`ticksPerClick`). downbeat is the strong click. Before 0, the same as `nextBeat`. */
  nextClick(tick: number, out: Beat = { tick: 0, downbeat: false }): Beat {
    const r = lastAtOrBefore(this.rTick0, tick);
    return this.nextStep(tick, r, ticksPerClick(this.rNum[r]!, this.rDen[r]!), out);
  }

  /** Metronome clicks per bar in the time signature there. A fractional tick is cut down; before 0 is read as 0. */
  clicksPerBarAt(tick: number): number {
    const r = lastAtOrBefore(this.rTick0, Math.max(0, Math.floor(tick)));
    return clicksPerBar(this.rNum[r]!, this.rDen[r]!);
  }

  /** step divides one bar of section r into a whole number of parts. */
  private nextStep(tick: number, r: number, step: number, out: Beat): Beat {
    const steps = this.rPerBar[r]! / step;
    // A section is a whole number of bars, so a step found past its end is the first step of the next section.
    const n = Math.ceil((tick - this.rTick0[r]!) / step);
    out.tick = this.rTick0[r]! + n * step;
    out.downbeat = ((n % steps) + steps) % steps === 0;
    return out;
  }
}
