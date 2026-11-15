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
  /**
   * 太いときだけ: 鍵盤の絵を細い鍵にして浮く幅、鳴る先の音源の名前をはぶいて浮く幅、
   * 「弾くと鳴る先」のラベルをはぶいて浮く幅、サステインの幅（前のすきまを入れて）。
   */
  narrowKeys?: number;
  instrument?: number;
  targetLabel?: number;
  sustain?: number;
  /** 右はしに案内の文が出ているときの、文ごとの幅。rest には全部の文が入っている。 */
  sentences?: readonly number[];
}

export interface MeterFit {
  nameWidth: NameWidth;
  pitchBend: boolean;
  modulation: boolean;
  /** 鍵盤の絵を細い鍵（白鍵の幅 8）にする。 */
  narrowKeys: boolean;
  /** 鳴る先の値に音源の名前を出す（トラックはいつも出す）。 */
  instrument: boolean;
  /** 「弾くと鳴る先」のラベルを出す。 */
  targetLabel: boolean;
  sustain: boolean;
  /** 案内の文を前からいくつ出すか。1 文目は残し、入らなければ画面がうしろを「…」でちぢめる。 */
  sentences: number;
}

type Step = "modulation" | "pitchBend" | "narrowKeys" | "instrument" | "targetLabel" | "sentence" | "sustain";

/** 太いときに引っこめる順（仕様書 1 章「拡大率の決まり」）。sentence は 2 文目から先を、うしろから 1 文ずつ。 */
const TARGET_ORDER: readonly Step[] = ["modulation", "pitchBend", "narrowKeys", "instrument", "targetLabel", "sustain"];
const MESSAGE_ORDER: readonly Step[] = ["modulation", "pitchBend", "narrowKeys", "sentence", "sustain"];

export function nameWidthPx(name: NameWidth, scale: Scale): number {
  const i = scale === 100 ? 0 : scale === 125 ? 1 : 2;
  return name === "size.meter.name" || name === "size.meter.name.slim" ? size[name][i] : partAt(name, scale);
}

/**
 * どの読み値を出すかと、名前の欄の幅。
 * 細いとき: モジュレーション、ピッチベンドの順に引っこめ、それでも足りなければ名前の上限を 176 から 128 にする。
 * 案内の文は、そのあとで 2 文目から先を引っこめる。
 * 太いとき、右はしが「弾くと鳴る先」: モジュレーション → ピッチベンド → 鍵盤の絵を細い鍵に → 音源の名前 →
 * 「弾くと鳴る先」のラベル → サステイン。
 * 太いとき、右はしが案内の文: モジュレーション → ピッチベンド → 鍵盤の絵を細い鍵に → 案内の 2 文目から先 → サステイン。
 * それでも入らない名前や案内の文は「…」でちぢむ。
 */
export function meterFit(input: MeterFitInput): MeterFit {
  const { meter, scale, available, rest, pitchBend, modulation } = input;
  const sentenceWidths = input.sentences ?? [];
  let sentences = sentenceWidths.length;
  const dropSentence = (need: number) => need - sentenceWidths[--sentences]!;
  if (meter === "full") {
    const first: NameWidth = input.nameChars > LONG_NAME_CHARS ? "part.meter.name.long" : "size.meter.name";
    const saved: Record<Exclude<Step, "sentence">, number> = {
      modulation,
      pitchBend,
      narrowKeys: input.narrowKeys ?? 0,
      instrument: input.instrument ?? 0,
      targetLabel: input.targetLabel ?? 0,
      sustain: input.sustain ?? 0,
    };
    const fit: MeterFit = {
      nameWidth: first,
      pitchBend: true,
      modulation: true,
      narrowKeys: false,
      instrument: true,
      targetLabel: true,
      sustain: true,
      sentences,
    };
    let need = rest + nameWidthPx(first, scale) + pitchBend + modulation;
    for (const step of input.sentences ? MESSAGE_ORDER : TARGET_ORDER) {
      if (step === "sentence") {
        while (need > available && sentences > 1) need = dropSentence(need);
        continue;
      }
      if (need <= available) break;
      if (step === "narrowKeys") fit.narrowKeys = true;
      else fit[step] = false;
      need -= saved[step];
    }
    fit.sentences = sentences;
    return fit;
  }
  const width = (w: NameWidth, bend: boolean, mod: boolean) =>
    rest + Math.min(input.name, nameWidthPx(w, scale)) + (bend ? pitchBend : 0) + (mod ? modulation : 0);
  const slim = (nameWidth: NameWidth, pitchBend: boolean, modulation: boolean): MeterFit => {
    let need = width(nameWidth, pitchBend, modulation);
    while (need > available && sentences > 1) need = dropSentence(need);
    return {
      nameWidth,
      pitchBend,
      modulation,
      narrowKeys: false,
      instrument: true,
      targetLabel: true,
      sustain: true,
      sentences,
    };
  };
  const fits = (w: NameWidth, bend: boolean, mod: boolean) => width(w, bend, mod) <= available;
  const first: NameWidth = "size.meter.name.slim";
  if (fits(first, true, true)) return slim(first, true, true);
  if (fits(first, true, false)) return slim(first, true, false);
  if (fits(first, false, false)) return slim(first, false, false);
  return slim("part.meter.name.slim-narrow", false, false);
}

/** 文を「。」のあとで分ける。 */
export function sentences(text: string): string[] {
  return text.match(/[^。]+。?/g) ?? [text];
}

/** 文字数（サロゲートペアも 1 文字）。 */
export function charCount(text: string): number {
  return [...text].length;
}
