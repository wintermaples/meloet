// 選んだクリップの欄の字（仕様書 3 章 93-4、モック F7-AudioEdit）。DOM を知らない純粋な関数。
// 位置は「7. 1. 0000」、長さは「2. 0. 0000」（小節 . 拍 . ティック、どれも 0 から）、フェードは「0.0480」（拍 . ティック。B-94-28）。

import type { StretchView } from "../../bridge";
import { t } from "../../text/ja";
import { signed } from "../pianoroll/geom";
import { parseNoteLength, parseNotePosition } from "../pianoroll/noteFields";
import { noteLengthText } from "../format";
import { tickText, ticksPerBar, ticksPerBeat, type Bars, type Meter } from "../time";

/** 「2. 0. 0000」: クリップの長さ。 */
export function clipLengthLabel(ticks: number, meter: Meter): string {
  const perBar = ticksPerBar(meter);
  const perBeat = ticksPerBeat(meter);
  const t = Math.max(0, Math.round(ticks));
  return `${Math.floor(t / perBar)}. ${Math.floor((t % perBar) / perBeat)}. ${tickText(t % perBeat)}`;
}

/** 「0.0480」: フェードの長さ。 */
export const fadeLengthText = (ticks: number, meter: Meter) => noteLengthText(ticks, meter);

/** 打った字の空白を外し、全角をそろえる（「7. 1. 0000」も読む）。 */
const tidy = (s: string) => s.normalize("NFKC").replace(/\s+/g, "");

/** 位置「7. 1. 0000」（「7.1」「7」も）を曲の中のティックに。読めなければ null。 */
export function parseClipPosition(s: string, bars: Bars): number | null {
  return parseNotePosition(tidy(s), bars);
}

/**
 * 長さ「2. 0. 0000」（「2.2」「2」も。小節 . 拍 . ティック）をティックに。`beats` なら「拍 . ティック」（フェード）。
 * 読めなければ null。範囲は見ない（Rust が中に入れる）。
 */
export function parseClipLength(s: string, meter: Meter, beats = false): number | null {
  const text = tidy(s);
  if (beats) return parseNoteLength(text, meter);
  const m = text.match(/^(\d{1,4})(?:\.(\d{1,2})(?:\.(\d{1,4}))?)?$/);
  if (!m) return null;
  const beat = m[2] === undefined ? 0 : Number(m[2]);
  const tick = m[3] === undefined ? 0 : Number(m[3]);
  if (beat >= meter.num || tick >= ticksPerBeat(meter)) return null;
  return Number(m[1]) * ticksPerBar(meter) + beat * ticksPerBeat(meter) + tick;
}

// ---- のび縮み ・ 音の高さ（仕様書 2 章「歌のクリップ」の印、3 章「のび縮み」「音の高さ」） ----

/** 上へ `dy` px ドラッグしたときの段の数（`stepPx` ごとに 1。下へはマイナス）。 */
export const dragSteps = (dy: number, stepPx: number) => (stepPx > 0 ? Math.trunc(dy / stepPx) : 0);

/** 半音 ・ セントの打ちこみ（「+2」「−1」「3」）。小数はいちばん近い整数。読めなければ null。範囲は見ない。 */
export function parseWhole(s: string): number | null {
  const text = tidy(s).replace(/−/g, "-");
  if (!/^[+-]?(\d+(\.\d*)?|\.\d+)$/.test(text)) return null;
  return Math.round(Number(text));
}

/** 「この音のテンポ」の打ちこみ（「120」「96.5」）。読めなければ null。範囲とまるめは Rust。 */
export function parseSourceBpm(s: string): number | null {
  const text = tidy(s);
  if (!/^(\d+(\.\d*)?|\.\d+)$/.test(text)) return null;
  const bpm = Number(text);
  return bpm > 0 ? bpm : null;
}

/** 印「×1.25」の数（小数 2 けた）。ついていかない ・ 1.00 になる物は null（B-94-55）。 */
export function ratioMarkValue(ratio: number | null): string | null {
  if (ratio === null || !Number.isFinite(ratio)) return null;
  const text = ratio.toFixed(2);
  return text === "1.00" ? null : text;
}

/** セントの字（マイナスは U+2212、+ は書かない）。 */
export const centsText = (v: number) => (v < 0 ? `−${-v}` : String(v));

/**
 * クリップの名前の右の印: 「×1.25」と「+2」「−1 ・ 30」（半音 ・ セント。B-94-59 の仮の読み）。どちらもなければ空。
 * 音の高さの印は、半音もセントも 0 なら出さない。
 */
export function clipMarks(stretch: StretchView): readonly string[] {
  if (stretch.ratio === null && stretch.semitones === 0 && stretch.cents === 0) return NO_MARKS;
  const out: string[] = [];
  const ratio = ratioMarkValue(stretch.ratio);
  if (ratio !== null) out.push(t("clip.ratioMark", { ratio }));
  if (stretch.cents !== 0) out.push(t("clip.pitchMarkCents", { semitones: signed(stretch.semitones), cents: centsText(stretch.cents) }));
  else if (stretch.semitones !== 0) out.push(signed(stretch.semitones));
  return out.length > 0 ? out : NO_MARKS;
}

export const NO_MARKS: readonly string[] = [];
