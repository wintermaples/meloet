// ノートの一覧と「選んだノート」の欄の打ちこみ（仕様書 3 章「ノートの一覧」）。DOM を知らない純粋な関数。
// 欄の文字の読み書き、使える範囲、↑ ↓ の 1 きざみ、送る意図（SetNotesField ・ RejectValue ・ ClampValue）。
// 数字として読めて範囲の外なら、近いはしに丸めて決める（design-notes B-68）。読めない物だけが「こわれた」。

import type { ClipView, Command, NoteField, TakeNotes, ValueField } from "../../bridge";
import { noteLengthText, notePositionText, rangeText } from "../format";
import { noteName } from "../meterFormat";
import { barAt, barsMap, meterAtOf, perBeatOf, tickOfPosition, tickRangeText } from "../../time/bars";
import { ticksPerBeat, type Bars, type Meter } from "../time";
import { drumKeyOf, keyLabel, type Drums } from "./drums";

const kitOf = (d: Drums | boolean): Drums => (d === true ? "builtin" : d);

export type FieldName = "position" | "key" | "length" | "velocity";
/** 左から右への順（一覧の欄の順、Tab の順）。 */
export const FIELD_ORDER: readonly FieldName[] = ["position", "key", "length", "velocity"];
/** ドラムのノートの一覧の欄（「長さ」の列を出さない。仕様書 3 章）。 */
export const DRUM_FIELD_ORDER: readonly FieldName[] = ["position", "key", "velocity"];

const NOTE_FIELDS: Record<FieldName, NoteField> = { position: "start", key: "key", length: "length", velocity: "velocity" };
const VALUE_FIELDS: Record<FieldName, ValueField> = { position: "position", key: "key", length: "length", velocity: "velocity" };

type Notes = Pick<TakeNotes, "starts" | "lengths" | "keys" | "velocities">;
type Placed = Pick<ClipView, "start" | "end" | "takeOffset">;

/** 打ちこんだ文字を、全角の数字や記号も読めるようにそろえる。 */
const tidy = (s: string) => s.normalize("NFKC").trim();

/** 位置「3.1.0000」（「3.1」「3」も）を曲の中のティックに。読めなければ null。範囲は見ない。 */
export function parseNotePosition(s: string, bars: Bars): number | null {
  const m = tidy(s).match(/^(\d{1,4})(?:\.(\d{1,2})(?:\.(\d{1,4}))?)?$/);
  if (!m) return null;
  const bar = Number(m[1]);
  const beat = m[2] === undefined ? 1 : Number(m[2]);
  const tick = m[3] === undefined ? 0 : Number(m[3]);
  const map = barsMap(bars);
  if (bar < 1 || beat < 1) return null;
  const at = barAt(map, bar);
  if (beat > at.num || tick >= perBeatOf(at)) return null;
  return tickOfPosition(map, bar, beat, tick);
}

/** 長さ「1.0960」（拍 . ティック。「2」も）をティックに。読めなければ null。範囲は見ない。 */
export function parseNoteLength(s: string, meter: Meter): number | null {
  const m = tidy(s).match(/^(\d{1,4})(?:\.(\d{1,4}))?$/);
  if (!m) return null;
  const tick = m[2] === undefined ? 0 : Number(m[2]);
  if (tick >= ticksPerBeat(meter)) return null;
  return Number(m[1]) * ticksPerBeat(meter) + tick;
}

const PITCH: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };

/** 音名「D#4」（C4 = 60。「Eb4」「d#4」も）を鍵の番号に。読めないか 0〜127 の外なら null。 */
export function parseKeyName(s: string): number | null {
  const m = tidy(s).match(/^([A-Ga-g])([#b]?)(-?\d{1,2})$/);
  if (!m) return null;
  const n = (Number(m[3]) + 1) * 12 + PITCH[m[1]!.toUpperCase()]! + (m[2] === "#" ? 1 : m[2] === "b" ? -1 : 0);
  return n >= 0 && n <= 127 ? n : null;
}

// ---- 範囲の外も読む（B-68）。丸めた所（小節 ・ 拍 ・ ティック）を返す ----

/** 丸めた所の言いかた（ClampValue の field ・ range）。 */
export interface ClampedUnit {
  field: ValueField;
  range: string | null;
}

export interface LooseValue {
  value: number;
  /** 小節 ・ 拍 ・ ティックのどれかを範囲に入れたなら、いちばん大きい単位。 */
  unit: ClampedUnit | null;
}

/** NFKC でそろえ、数学の − も - に。 */
const tidyMinus = (s: string) => tidy(s).replaceAll("\u2212", "-");
const clampTo = (v: number, low: number, high: number) => Math.min(high, Math.max(low, v));

/** 位置「3.5.0000」「0.1.0000」「3.1.9999」も読み、小節は 1 から、拍は 1〜拍子の上、ティックは 0〜1 拍のティック − 1 にそれぞれ入れる。 */
export function readNotePositionLoose(s: string, bars: Bars): LooseValue | null {
  const m = tidyMinus(s).match(/^(-?\d{1,4})(?:\.(\d{1,2})(?:\.(\d{1,4}))?)?$/);
  if (!m) return null;
  const rawBar = Number(m[1]);
  const rawBeat = m[2] === undefined ? 1 : Number(m[2]);
  const rawTick = m[3] === undefined ? 0 : Number(m[3]);
  const bar = Math.max(1, rawBar);
  const map = barsMap(bars);
  const at = barAt(map, bar);
  const num = at.num;
  const perBeat = perBeatOf(at);
  const beat = clampTo(rawBeat, 1, num);
  const tick = clampTo(rawTick, 0, perBeat - 1);
  let unit: ClampedUnit | null = null;
  if (bar !== rawBar) unit = { field: "bar", range: null };
  else if (beat !== rawBeat) unit = { field: "beat", range: rangeText("1", String(num)) };
  else if (tick !== rawTick) unit = { field: "tick", range: tickRangeText(perBeat) };
  return { value: tickOfPosition(map, bar, beat, tick), unit };
}

/** 長さ「1.9999」も読み、ティックを 0〜1 拍のティック − 1 に入れる。 */
export function readNoteLengthLoose(s: string, meter: Meter): LooseValue | null {
  const m = tidy(s).match(/^(\d{1,4})(?:\.(\d{1,4}))?$/);
  if (!m) return null;
  const perBeat = ticksPerBeat(meter);
  const rawTick = m[2] === undefined ? 0 : Number(m[2]);
  const tick = Math.min(rawTick, perBeat - 1);
  const unit: ClampedUnit | null = tick !== rawTick ? { field: "tick", range: tickRangeText(perBeat) } : null;
  return { value: Number(m[1]) * perBeat + tick, unit };
}

/** 音名を、0〜127 の外（「C10」「C-2」）も鍵の番号に。 */
export function readKeyNameLoose(s: string): LooseValue | null {
  const m = tidyMinus(s).match(/^([A-Ga-g])([#b]?)(-?\d{1,2})$/);
  if (!m) return null;
  const octave = Number(m[3]);
  return { value: (octave + 1) * 12 + PITCH[m[1]!.toUpperCase()]! + (m[2] === "#" ? 1 : m[2] === "b" ? -1 : 0), unit: null };
}

/** ベロシティ。負の数と小数も読み、小数は四捨五入。 */
export function readVelocityLoose(s: string): LooseValue | null {
  const m = tidyMinus(s).match(/^(-?)(\d{1,6}(?:\.\d+)?|\.\d+)$/);
  if (!m) return null;
  const v = Math.round(Number(m[2]));
  return { value: m[1] ? -v : v, unit: null };
}

/** ベロシティ（整数）。読めなければ null。範囲は見ない。 */
export function parseVelocity(s: string): number | null {
  const m = tidy(s).match(/^\d{1,3}$/);
  return m ? Number(m[0]) : null;
}

/** 入力の枠 1 つの決まり（ノートの欄と CC の一覧の欄）。 */
export interface FieldSpec {
  /** 状態行で言う欄（RejectValue ・ ClampValue の field）。 */
  field: ValueField;
  /** 枠をひらいたときに入れておく文字。 */
  initial: string;
  /** 使えない値を打ったときの「もとの {kept} のまま」。ばらばらなら null（欄に見えていた文字を使う）。 */
  kept: string | null;
  /** いまの値。ばらばら（どの値を決めても変わる）なら null。 */
  current: number | null;
  low: number;
  high: number;
  /** ↑ ↓ の 1 つぶん。 */
  unit: number;
  /** Shift ↑ ↓ で動く数（unit の何倍か）。音の欄は 12 半音、ほかは 10。 */
  shift: number;
  /** 状態行の範囲の文（位置 ・ 長さ ・ 音）。ベロシティは画面の文（value.velocityRange）なので null。 */
  range: string | null;
  format: (v: number) => string;
  read: (s: string) => number | null;
  /** 範囲の外も読む（B-68）。読めなければ null。 */
  loose: (s: string) => LooseValue | null;
}

export interface NoteFieldSpec extends FieldSpec {
  name: FieldName;
}

/** 決める値と、丸めたならその言いかた（ClampValue の中身）。 */
export interface FieldRead {
  value: number;
  clamped: { field: ValueField; value: string; range: string | null } | null;
}

/**
 * 打った文字を決める値に。数字として読めなければ null（「こわれた」）。範囲の外なら近いはしにして、丸めた所を返す。
 * 2 つ以上丸めたときは、いちばん大きい単位だけ（クリップの範囲 → 小節 → 拍 → ティック）。
 */
export function readField(spec: FieldSpec, s: string): FieldRead | null {
  const r = spec.loose(s);
  if (!r) return null;
  const value = clampTo(r.value, spec.low, spec.high);
  if (value !== r.value) return { value, clamped: { field: spec.field, value: spec.format(value), range: spec.range } };
  return { value, clamped: r.unit ? { field: r.unit.field, value: spec.format(value), range: r.unit.range } : null };
}

/** 丸めたことを状態行で言わせる意図（決める意図のあとに送る）。 */
export function clampCommand(clamped: NonNullable<FieldRead["clamped"]>): Command {
  return { type: "clampValue", field: clamped.field, value: clamped.value, range: clamped.range };
}

/** 範囲の中の値。読めないか範囲の外なら null。 */
export function parseField(spec: FieldSpec, s: string): number | null {
  const v = spec.read(s);
  return v !== null && v >= spec.low && v <= spec.high ? v : null;
}

/** ↑ ↓ で delta（1 か spec.shift、下は −）つぶん動かした値。範囲の外へは出ない。 */
export function stepField(spec: FieldSpec, v: number, delta: number): number {
  return Math.min(spec.high, Math.max(spec.low, v + delta * spec.unit));
}

/**
 * notes（テイクの中の番号、1 つ以上）の欄 name の打ちこみの決まり。2 つ以上なら SetNotesField の意味のとおり:
 * 位置はいちばん早いノートを、音はいちばん低いノートを動かし、ほかはその間を保つ。長さとベロシティはぜんぶを同じ値に。
 * 枠に入れておく値は、位置はいちばん早い位置、音はいちばん低い音、長さはそろっていればその値（ばらばらなら、いちばん早いノートの長さ）、
 * ベロシティはそろっていればその値（ばらばらなら、いちばん小さい値）。
 * drums なら、音は楽器の名前で見せ、楽器の名前でも音名でも読む。
 */
export function noteFieldSpec(
  name: FieldName,
  take: Notes,
  clip: Placed,
  notes: readonly number[],
  bars: Bars,
  drums: Drums | boolean = false,
): NoteFieldSpec | null {
  const picked = notes.filter((i) => i < take.starts.length);
  if (picked.length === 0) return null;
  const at = clip.start + clip.takeOffset;
  const starts = picked.map((i) => at + take.starts[i]!);
  const first = picked[starts.indexOf(Math.min(...starts))]!;
  // 長さとティックの目盛りは、いちばん早いノートの所の拍子で数える。
  const meter = meterAtOf(bars, Math.min(...starts));
  // ↑ ↓ と長さのいちばん下は 1 ティック（表示のいちばん下のけた）。
  const unit = 1;
  switch (name) {
    case "position": {
      const firstStart = Math.min(...starts);
      const lastEnd = Math.max(...picked.map((i) => at + take.starts[i]! + take.lengths[i]!));
      const low = clip.start;
      const high = Math.max(low, clip.end - (lastEnd - firstStart));
      const format = (v: number) => notePositionText(v, bars);
      return {
        name,
        field: VALUE_FIELDS[name],
        shift: 10,
        initial: format(firstStart),
        kept: format(firstStart),
        current: firstStart,
        low,
        high,
        unit,
        range: rangeText(format(low), format(high)),
        format,
        read: (s) => parseNotePosition(s, bars),
        loose: (s) => readNotePositionLoose(s, bars),
      };
    }
    case "key": {
      const keys = picked.map((i) => take.keys[i]!);
      const lowest = Math.min(...keys);
      const high = 127 - (Math.max(...keys) - lowest);
      const format = (k: number) => keyLabel(k, drums);
      return {
        name,
        field: VALUE_FIELDS[name],
        shift: 12,
        initial: format(lowest),
        kept: format(lowest),
        current: lowest,
        low: 0,
        high,
        unit: 1,
        range: rangeText(noteName(0), noteName(high)),
        format,
        read: drums ? (t) => drumKeyOf(t, kitOf(drums)) ?? parseKeyName(t) : parseKeyName,
        loose: drums
          ? (t) => {
              const k = drumKeyOf(t, kitOf(drums));
              return k === null ? readKeyNameLoose(t) : { value: k, unit: null };
            }
          : readKeyNameLoose,
      };
    }
    case "length": {
      const lengths = picked.map((i) => take.lengths[i]!);
      const same = lengths.every((l) => l === lengths[0]);
      const high = Math.max(unit, clip.end - Math.max(...starts));
      const format = (v: number) => noteLengthText(v, meter);
      const initial = format(same ? lengths[0]! : take.lengths[first]!);
      return {
        name,
        field: VALUE_FIELDS[name],
        shift: 10,
        initial,
        kept: same ? initial : null,
        current: same ? lengths[0]! : null,
        low: unit,
        high,
        unit,
        range: rangeText(format(unit), format(high)),
        format,
        read: (s) => parseNoteLength(s, meter),
        loose: (s) => readNoteLengthLoose(s, meter),
      };
    }
    case "velocity": {
      const velocities = picked.map((i) => take.velocities[i]!);
      const low = Math.min(...velocities);
      const same = velocities.every((v) => v === low);
      return {
        name,
        field: VALUE_FIELDS[name],
        shift: 10,
        initial: String(low),
        kept: same ? String(low) : null,
        current: same ? low : null,
        low: 1,
        high: 127,
        unit: 1,
        range: null,
        format: String,
        read: parseVelocity,
        loose: readVelocityLoose,
      };
    }
  }
}

/** 決めた値の意図。値が変わらなければ null（取り消しの手を作らない）。 */
export function commitCommand(spec: NoteFieldSpec, value: number, take: number, notes: readonly number[]): Command | null {
  if (spec.current !== null && value === spec.current) return null;
  return { type: "setNotesField", take, notes: [...notes], field: NOTE_FIELDS[spec.name], value };
}

/** 使えない値のまま Enter（Tab）を押した。shown は欄に見えていた文字（ばらばらのときの「ばらばら」「77 – 83」）。 */
export function rejectCommand(spec: FieldSpec, shown: string): Command {
  return { type: "rejectValue", field: spec.field, kept: spec.kept ?? shown, range: spec.range };
}

/** Tab（back なら Shift Tab）で次にひらく欄。いまの行の外へ出るなら row が ±1。order は欄の並び（ドラムの一覧は長さがない）。 */
export function nextField(name: FieldName, back: boolean, order: readonly FieldName[] = FIELD_ORDER): { name: FieldName; row: -1 | 0 | 1 } {
  const i = order.indexOf(name) + (back ? -1 : 1);
  if (i < 0) return { name: order[order.length - 1]!, row: -1 };
  if (i >= order.length) return { name: order[0]!, row: 1 };
  return { name: order[i]!, row: 0 };
}
