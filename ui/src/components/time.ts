// 曲の中の時間（docs/architecture.md 11 章）。crates/meloet-model/src/time.rs と同じ答えを出す。DOM を知らない純粋な関数。

import { positionAt, type Telemetry, type TransportView } from "../bridge";
import { barsMap, type Bars } from "../time/bars";

export type { Bars } from "../time/bars";

/** 4 分音符 1 つのティック。 */
export const TICKS_PER_QUARTER = 480;
/** 表示の「ティック」は、1 拍をこの数に分けた目盛り。 */
export const DISPLAY_TICKS_PER_BEAT = 480;

/** 使えるテンポ（ヤチヨさんの決め）。0.01 きざみ。 */
export const TEMPO_MIN = 30;
export const TEMPO_MAX = 300;
/** 使える拍子（ヤチヨさんの決め）。 */
export const METER_NUM_MAX = 12;
export const METER_DENS: readonly number[] = [2, 4, 8, 16];

export interface Meter {
  num: number;
  den: number;
}

export function ticksPerBeat(meter: Meter): number {
  return (TICKS_PER_QUARTER * 4) / meter.den;
}

export function ticksPerBar(meter: Meter): number {
  return meter.num * ticksPerBeat(meter);
}

export interface BarBeatTick {
  bar: number;
  beat: number;
  tick: number;
}

/**
 * 小節と拍は 1 から、ティックは 0〜479。小数のティックは切りすて、0 より前は 0 として読む。
 * bars は拍子 1 つか、テンポの地図（拍子の変更のある曲）。
 */
export function barBeatTick(tick: number, bars: Bars): BarBeatTick {
  return barsMap(bars).barBeatTick(tick, { bar: 0, beat: 0, tick: 0 });
}

/** 曲の頭のテンポと拍子で秒に。 */
export function ticksToSeconds(tick: number, bpm: number, meter: Meter): number {
  return ((tick / ticksPerBeat(meter)) * 60) / bpm;
}

/** 「7. 3. 240」（モック F-ArrangeRec）。 */
export function positionText(tick: number, bars: Bars): string {
  const b = barBeatTick(tick, bars);
  return `${b.bar}. ${b.beat}. ${String(b.tick).padStart(3, "0")}`;
}

/** 「00:00:16.562」（モック F-ArrangeRec）。ミリ秒は切りすて。 */
export function clockText(seconds: number): string {
  const ms = Math.max(0, Math.floor(seconds * 1000 + 1e-6));
  const two = (n: number) => String(n).padStart(2, "0");
  const h = Math.floor(ms / 3_600_000);
  const m = Math.floor(ms / 60_000) % 60;
  const s = Math.floor(ms / 1000) % 60;
  return `${two(h)}:${two(m)}:${two(s)}.${String(ms % 1000).padStart(3, "0")}`;
}

/** 打ちこんだ文字を、全角の数字や記号も読めるようにそろえる。 */
const tidy = (s: string) => s.normalize("NFKC").trim();

/** テンポの打ちこみ。使えない値や読めない文字は null。小数は 2 けたまで。 */
export function parseTempo(s: string): number | null {
  const m = tidy(s).match(/^(\d{1,3})(?:\.(\d{0,2}))?$/);
  if (!m) return null;
  const centi = Number(m[1]) * 100 + Number((m[2] ?? "").padEnd(2, "0"));
  if (centi < TEMPO_MIN * 100 || centi > TEMPO_MAX * 100) return null;
  return centi / 100;
}

/** 拍子の打ちこみ（「4/4」）。使えない値や読めない文字は null。 */
export function parseMeter(s: string): Meter | null {
  const m = tidy(s).match(/^(\d{1,2})\s*\/\s*(\d{1,2})$/);
  if (!m) return null;
  const num = Number(m[1]);
  const den = Number(m[2]);
  return num >= 1 && num <= METER_NUM_MAX && METER_DENS.includes(den) ? { num, den } : null;
}

/**
 * 使えない拍子の打ちこみで、だめなのは上と下のどちらか。上が 1〜12 の数なら下、そうでなければ上
 * （「/」がない、読めないときも上）。
 */
export function meterFieldOf(s: string): "meterNum" | "meterDen" {
  const m = tidy(s).match(/^(\d{1,2})\s*\/(.*)$/);
  const num = m ? Number(m[1]) : NaN;
  return num >= 1 && num <= METER_NUM_MAX ? "meterDen" : "meterNum";
}

export function meterText(meter: Meter): string {
  return `${meter.num}/${meter.den}`;
}

/** ↑ ↓ で動かした後のテンポ。範囲の外へは出ない。 */
export function stepTempo(bpm: number, delta: number): number {
  const centi = Math.round(bpm * 100) + delta * 100;
  return Math.min(TEMPO_MAX * 100, Math.max(TEMPO_MIN * 100, centi)) / 100;
}

/** ↑ ↓ で動かした後の拍子。上の数だけを動かす。 */
export function stepMeter(meter: Meter, delta: number): Meter {
  return { num: Math.min(METER_NUM_MAX, Math.max(1, meter.num + delta)), den: meter.den };
}

/**
 * 再生まわりの数字と再生の位置の線が出す位置（ティック）。止まっているときは transport.position（差分ですぐ届く）、
 * カウントイン中は録りはじめる所（モック F-Connect4）、動いているときは計器の位置を rustNowUs まで進めた所
 * （rustNowUs が null なら計器の位置のまま）。計器に位置がなければ transport.position。
 */
export function shownTick(transport: TransportView, telemetry: Telemetry | null, rustNowUs: number | null): number {
  if (transport.state === "stopped") return transport.position;
  if (transport.state === "countIn") return transport.recording?.from ?? transport.position;
  const p = telemetry?.position;
  if (!p) return transport.position;
  return rustNowUs === null ? p.tick : (positionAt(telemetry, rustNowUs) ?? transport.position);
}
