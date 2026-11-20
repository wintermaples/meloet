// 数字の欄の、範囲の外の値（docs/design-notes.md B-68）。数字として読めれば近いはしにして決め、読めなければ null（「こわれた」の枠）。
// 丸めたときは、送る ClampValue の中身（field ・ value ・ range）もいっしょに返す。DOM を知らない純粋な関数。

import type { ValueField } from "../bridge";
import { tempoText } from "./format";
import { barAt, barsMap, perBeatOf, tickOfPosition, tickRangeText } from "../time/bars";
import { METER_DENS, METER_NUM_MAX, positionText, TEMPO_MAX, TEMPO_MIN, type Bars, type Meter } from "./time";

export interface ClampNote {
  field: ValueField;
  /** 決めた値（画面に出す書きかた）。 */
  value: string;
  range: string | null;
}

export interface Read<T> {
  value: T;
  /** 範囲の外だったので丸めた。範囲の中なら null。 */
  clamp: ClampNote | null;
}

/** 打ちこんだ文字を、全角の数字や記号、U+2212 のマイナスも読めるようにそろえる。 */
const tidy = (s: string) => s.normalize("NFKC").trim().replace(/−/g, "-");

const NUMBER = /^(-?)(\d*)(?:\.(\d*))?$/;

/** 小数を digits けたで四捨五入して、10^digits 倍の整数に（文字のまま数えるので、120.555 も 12056 になる）。読めなければ null。 */
export function readScaled(s: string, digits: number): number | null {
  const m = tidy(s).match(NUMBER);
  if (!m || (m[2] === "" && (m[3] ?? "") === "")) return null;
  const whole = Number(m[2] || "0");
  const frac = (m[3] ?? "").padEnd(digits + 1, "0");
  let scaled = whole * 10 ** digits + Number(frac.slice(0, digits) || "0");
  if (Number(frac[digits]) >= 5) scaled += 1;
  return m[1] === "-" ? -scaled : scaled;
}

const clampTo = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** テンポ（30.00〜300.00。小数第 2 位まで）。 */
export function readTempo(s: string): Read<number> | null {
  const centi = readScaled(s, 2);
  if (centi === null) return null;
  const kept = clampTo(centi, TEMPO_MIN * 100, TEMPO_MAX * 100);
  const value = kept / 100;
  return { value, clamp: kept === centi ? null : { field: "tempo", value: tempoText(value), range: null } };
}

/** 拍子の下: 2 ・ 4 ・ 8 ・ 16 のいちばん近い値。同じだけ離れていたら大きいほう。 */
export function nearestDen(n: number): number {
  let best = METER_DENS[0]!;
  for (const d of METER_DENS) if (Math.abs(d - n) <= Math.abs(best - n)) best = d;
  return best;
}

/** 拍子（「4/4」）。上は 1〜12、下は nearestDen。両方を丸めたときは上のことだけ言う。 */
export function readMeter(s: string): Read<Meter> | null {
  const m = tidy(s).match(/^([^/]*)\/([^/]*)$/);
  if (!m) return null;
  const numRaw = readScaled(m[1]!, 0);
  const denRaw = readScaled(m[2]!, 0);
  if (numRaw === null || denRaw === null) return null;
  const num = clampTo(numRaw, 1, METER_NUM_MAX);
  const den = nearestDen(denRaw);
  const clamp: ClampNote | null =
    num !== numRaw
      ? { field: "meterNum", value: String(num), range: null }
      : den !== denRaw
        ? { field: "meterDen", value: String(den), range: null }
        : null;
  return { value: { num, den }, clamp };
}

/** ベロシティ（1〜127）。 */
export function readVelocity(s: string): Read<number> | null {
  const raw = readScaled(s, 0);
  if (raw === null) return null;
  const value = clampTo(raw, 1, 127);
  return { value, clamp: value === raw ? null : { field: "velocity", value: String(value), range: null } };
}

/**
 * 再生の位置（「5」「5.2」「5.2.0960」。区切りの前後の空白はあってよい）を曲の中のティックに。
 * 小節は 1 から、拍は 1〜拍子の上、ティックは 0〜その拍子の 1 拍のティック − 1 に、それぞれ入れる。2 つ以上丸めたときは、いちばん大きい単位のことだけ言う。
 */
export function readPosition(s: string, bars: Bars): Read<number> | null {
  const parts = tidy(s).split(/\s*\.\s*/);
  if (parts.length > 3) return null;
  const nums = parts.map((p) => (/^-?\d+$/.test(p) ? Number(p) : null));
  if (nums.some((n) => n === null)) return null;
  const [barRaw, beatRaw = 1, tickRaw = 0] = nums as number[];
  const map = barsMap(bars);
  const bar = Math.max(1, barRaw!);
  const at = barAt(map, bar);
  const num = at.num;
  const perBeat = perBeatOf(at);
  const beat = clampTo(beatRaw, 1, num);
  const tick = clampTo(tickRaw, 0, perBeat - 1);
  const value = tickOfPosition(map, bar, beat, tick);
  const shown = positionText(value, map);
  const clamp: ClampNote | null =
    bar !== barRaw
      ? { field: "bar", value: shown, range: null }
      : beat !== beatRaw
        ? { field: "beat", value: shown, range: `1〜${num}` }
        : tick !== tickRaw
          ? { field: "tick", value: shown, range: tickRangeText(perBeat) }
          : null;
  return { value, clamp };
}
