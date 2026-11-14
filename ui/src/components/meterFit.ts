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
  /** 太いときだけ: 鍵盤の絵を細い鍵にして浮く幅、「弾くと鳴る先」のラベルをはぶいて浮く幅、サステインの幅（前のすきまを入れて）。 */
  narrowKeys?: number;
  targetLabel?: number;
  sustain?: number;
}

export interface MeterFit {
  nameWidth: NameWidth;
  pitchBend: boolean;
  modulation: boolean;
  /** 鍵盤の絵を細い鍵（白鍵の幅 8）にする。 */
  narrowKeys: boolean;
  /** 「弾くと鳴る先」のラベルを出す（値はいつも出す）。 */
  targetLabel: boolean;
  sustain: boolean;
}

/** 太いときに引っこめる順（仕様書 1 章「拡大率の決まり」）。 */
const FULL_ORDER = ["modulation", "pitchBend", "narrowKeys", "targetLabel", "sustain"] as const;

export function nameWidthPx(name: NameWidth, scale: Scale): number {
  const i = scale === 100 ? 0 : scale === 125 ? 1 : 2;
  return name === "size.meter.name" || name === "size.meter.name.slim" ? size[name][i] : partAt(name, scale);
}

/**
 * どの読み値を出すかと、名前の欄の幅。
 * 細いとき: モジュレーション、ピッチベンドの順に引っこめ、それでも足りなければ名前の上限を 176 から 128 にする。
 * 太いとき: モジュレーション → ピッチベンド → 鍵盤の絵を細い鍵に → 「弾くと鳴る先」のラベル → サステイン。
 * それでも入らない名前は「…」でちぢむ。
 */
export function meterFit(input: MeterFitInput): MeterFit {
  const { meter, scale, available, rest, pitchBend, modulation } = input;
  if (meter === "full") {
    const first: NameWidth = input.nameChars > LONG_NAME_CHARS ? "part.meter.name.long" : "size.meter.name";
    const saved = {
      modulation,
      pitchBend,
      narrowKeys: input.narrowKeys ?? 0,
      targetLabel: input.targetLabel ?? 0,
      sustain: input.sustain ?? 0,
    };
    const fit: MeterFit = {
      nameWidth: first,
      pitchBend: true,
      modulation: true,
      narrowKeys: false,
      targetLabel: true,
      sustain: true,
    };
    let need = rest + nameWidthPx(first, scale) + pitchBend + modulation;
    for (const step of FULL_ORDER) {
      if (need <= available) break;
      if (step === "narrowKeys") fit.narrowKeys = true;
      else fit[step] = false;
      need -= saved[step];
    }
    return fit;
  }
  const fits = (w: NameWidth, bend: boolean, mod: boolean) =>
    rest + Math.min(input.name, nameWidthPx(w, scale)) + (bend ? pitchBend : 0) + (mod ? modulation : 0) <= available;
  const slim = (nameWidth: NameWidth, pitchBend: boolean, modulation: boolean): MeterFit => ({
    nameWidth,
    pitchBend,
    modulation,
    narrowKeys: false,
    targetLabel: true,
    sustain: true,
  });
  const first: NameWidth = "size.meter.name.slim";
  if (fits(first, true, true)) return slim(first, true, true);
  if (fits(first, true, false)) return slim(first, true, false);
  if (fits(first, false, false)) return slim(first, false, false);
  return slim("part.meter.name.slim-narrow", false, false);
}

/**
 * 入力計の右はしの案内の文を、いくつ出すか（仕様書 1 章「拡大率の決まり」）。うしろの文から引っこめ、1 文目は残す。
 * それでも入らなければ、画面がうしろを「…」でちぢめる。widths は文ごとの幅、room は文に使える幅。
 */
export function sentencesShown(widths: readonly number[], room: number): number {
  let n = widths.length;
  let need = widths.reduce((a, b) => a + b, 0);
  while (n > 1 && need > room) need -= widths[--n]!;
  return n;
}

/** 文を「。」のあとで分ける。 */
export function sentences(text: string): string[] {
  return text.match(/[^。]+。?/g) ?? [text];
}

/** 文字数（サロゲートペアも 1 文字）。 */
export function charCount(text: string): number {
  return [...text].length;
}
