// 選んだクリップの欄の字（仕様書 3 章 93-4、モック F7-AudioEdit）。DOM を知らない純粋な関数。
// 位置は「7. 1. 0000」、長さは「2. 0. 0000」（小節 . 拍 . ティック、どれも 0 から）、フェードは「0.0480」（拍 . ティック。B-94-28）。

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
