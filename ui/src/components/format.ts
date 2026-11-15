// 数の見せかた。DOM を知らない純粋な関数。
import type { ScaleMode } from "../bridge";
import { t, texts, type TextKey } from "../text/ja";
import { barBeatTick, DISPLAY_TICKS_PER_BEAT, ticksPerBeat, type Meter } from "./time";

/** マイナスは仕様書と同じ「−」（U+2212）。 */
const MINUS = "−";

/** これより小さい dB は「−∞」。 */
export const SILENT_DB = -90;

/** 小数 1 けたの dB の数字（"0.0"、"−4.8"）。 */
export function dbNumber(db: number): string {
  const s = (Math.round(db * 10) / 10).toFixed(1);
  return s === "-0.0" ? "0.0" : s.replace("-", MINUS);
}

/** 1.0 = 0 dBFS の値を「−8.4 dB」の文に。小さすぎれば「−∞ dB」。 */
export function peakText(peak: number): string {
  const db = peak > 0 ? 20 * Math.log10(peak) : -Infinity;
  return db < SILENT_DB ? t("common.dbSilent") : t("common.db", { value: dbNumber(db) });
}

/** テンポを「96.00」の形に。 */
export function tempoText(bpm: number): string {
  return bpm.toFixed(2);
}

/** 遅れの ms。20 ms 未満は小数 1 けた（「13.5」「13.0」）、20 ms 以上は整数（「53」）。仕様書 6 章の書きかた。 */
export function msText(ms: number): string {
  const r = Math.round(ms * 10) / 10;
  return r < 20 ? r.toFixed(1) : String(Math.round(ms));
}

/** 直近 windowMs のいちばん大きい値。 */
export class PeakHold {
  private samples: { at: number; value: number }[] = [];

  constructor(readonly windowMs = 1000) {}

  push(value: number, nowMs: number): void {
    this.samples.push({ at: nowMs, value });
    this.drop(nowMs);
  }

  max(nowMs: number): number {
    this.drop(nowMs);
    return this.samples.reduce((m, s) => Math.max(m, s.value), 0);
  }

  private drop(nowMs: number): void {
    const from = nowMs - this.windowMs;
    let i = 0;
    while (i < this.samples.length && this.samples[i]!.at < from) i++;
    if (i > 0) this.samples.splice(0, i);
  }
}

export interface TextPart {
  text: string;
  /** うめこんだ値の穴の名前。文のほかの所は null。 */
  hole: string | null;
}

/** 文を、ふつうの所とうめこんだ値に分ける（値だけ書体や色を変えるため）。 */
export function textParts(key: TextKey, values: Readonly<Record<string, string | number>>): TextPart[] {
  const out: TextPart[] = [];
  const text = texts[key].text;
  const re = /\{(\w+)\}/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > last) out.push({ text: text.slice(last, m.index), hole: null });
    const name = m[1]!;
    out.push(name in values ? { text: String(values[name]), hole: name } : { text: m[0], hole: null });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ text: text.slice(last), hole: null });
  return out;
}

// ---- ピアノロールのまわり（ノートの一覧、選んだノート、状態行） ----

const PITCH_CLASSES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;

/** オクターブのない音名（「D#」）。0 は C。 */
export function pitchClassName(keyOrRoot: number): string {
  return PITCH_CLASSES[((keyOrRoot % 12) + 12) % 12]!;
}

/** 音階の名前（「C メジャー」「A マイナー」。ヤチヨさんの決め 2026-11-15）。 */
export function scaleName(root: number, mode: ScaleMode): string {
  return t("scale.name", { root: pitchClassName(root), mode: t(mode === "major" ? "scale.major" : "scale.minor") });
}

/** グリッドの名前（「1 小節」「1/16」。三連符なら「1/8 三連符」）。division 0 は 1 小節。 */
export function gridText(division: number, triplet: boolean): string {
  const base = division === 0 ? t("grid.bar") : t("grid.division", { division });
  return triplet ? `${base} ${t("grid.triplet")}` : base;
}

/** ノートの一覧の位置（「3.1.000」。モック A-PianoRoll）。曲の中のティック。 */
export function notePositionText(tick: number, meter: Meter): string {
  const b = barBeatTick(tick, meter);
  return `${b.bar}.${b.beat}.${String(b.tick).padStart(3, "0")}`;
}

/** ノートの長さ（「拍 . ティック」。「4.000」「0.360」。モック A-PianoRoll）。 */
export function noteLengthText(ticks: number, meter: Meter): string {
  const perBeat = ticksPerBeat(meter);
  const whole = Math.max(0, Math.floor(ticks));
  const rest = Math.floor(((whole % perBeat) * DISPLAY_TICKS_PER_BEAT) / perBeat);
  return `${Math.floor(whole / perBeat)}.${String(rest).padStart(3, "0")}`;
}
