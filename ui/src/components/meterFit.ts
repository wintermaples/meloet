// 入力計の横の入れかた（仕様書 1 章「大きさ」の size.meter.name・size.meter.name.slim、「拡大率の決まり」、
// 3 章「鍵盤の名前も本物をそのまま出す…」）。幅はどれも、いまの率での CSS px。
import { partAt, size, type Scale } from "../design/tokens";

/** 名前の欄の幅に使う数値の名前。CSS の変数は `--` ＋ 点をハイフンにした物。 */
export type NameWidth =
  | "size.meter.name"
  | "part.meter.name.long"
  | "size.meter.name.slim"
  | "part.meter.name.slim-narrow";

/** 太いとき、名前がこの文字数をこえたら名前の欄を広げる。 */
export const LONG_NAME_CHARS = 8;

export interface MeterFitInput {
  meter: "full" | "slim";
  scale: Scale;
  /** 鍵盤の名前の文字数。 */
  nameChars: number;
  /** 行の中で使える幅。 */
  available: number;
  /** 名前、ピッチベンド、モジュレーションをのぞいた、全部の物とすきまの幅。 */
  rest: number;
  /** 細いとき: つながりの印と名前を、ちぢめずに置いた幅。 */
  name: number;
  /** ピッチベンド、モジュレーションの幅（前のすきまを入れて）。 */
  pitchBend: number;
  modulation: number;
}

export interface MeterFit {
  nameWidth: NameWidth;
  pitchBend: boolean;
  modulation: boolean;
}

export function nameWidthPx(name: NameWidth, scale: Scale): number {
  const i = scale === 100 ? 0 : scale === 125 ? 1 : 2;
  return name === "size.meter.name" || name === "size.meter.name.slim" ? size[name][i] : partAt(name, scale);
}

/**
 * どの読み値を出すかと、名前の欄の幅。
 * 横が足りなければ、モジュレーション、ピッチベンドの順に引っこめ、それでも足りなければ
 * 細いときの名前の上限を 176 から 128 にする。それでも入らない名前は「…」でちぢむ。
 */
export function meterFit(input: MeterFitInput): MeterFit {
  const { meter, scale, available, rest, pitchBend, modulation } = input;
  const nameUsed = (w: NameWidth) =>
    meter === "full" ? nameWidthPx(w, scale) : Math.min(input.name, nameWidthPx(w, scale));
  const fits = (w: NameWidth, bend: boolean, mod: boolean) =>
    rest + nameUsed(w) + (bend ? pitchBend : 0) + (mod ? modulation : 0) <= available;

  const first: NameWidth =
    meter === "slim"
      ? "size.meter.name.slim"
      : input.nameChars > LONG_NAME_CHARS
        ? "part.meter.name.long"
        : "size.meter.name";

  if (fits(first, true, true)) return { nameWidth: first, pitchBend: true, modulation: true };
  if (fits(first, true, false)) return { nameWidth: first, pitchBend: true, modulation: false };
  if (meter === "full" || fits(first, false, false)) return { nameWidth: first, pitchBend: false, modulation: false };
  return { nameWidth: "part.meter.name.slim-narrow", pitchBend: false, modulation: false };
}

/** 文字数（サロゲートペアも 1 文字）。 */
export function charCount(text: string): number {
  return [...text].length;
}
