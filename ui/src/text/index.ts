// The screen's wording in the language of the page (docs/architecture.md 25, D4). ja.ts is the source, with sources;
// en.ts follows it key by key. The language is set once, before the first component is made (main.ts), and stays until
// the page loads again, so t() is a plain lookup.

import type { Language } from "../bridge/generated/Language";
import type { StepName } from "../bridge/generated/StepName";
import { enJoins, enStepNames, enTerms, enTexts, type EnText } from "./en";
import * as ja from "./ja";
import { fill, findTerm as findJaTerm, stepNames, texts, type TextKey } from "./ja";

export { fill, type TextKey };

/** A term and its tip as the page shows them. */
export interface ShownTerm {
  readonly term: string;
  readonly description: string;
}

let language: Language = "ja";

/** The mark that joins a term and its tip (spec 5 「用語 — 説明」). */
export let TERM_SEPARATOR = ja.TERM_SEPARATOR;
/** The joint between parts of the status line left text (spec 6 「止まっているときの状態行の左」). */
export let STATUS_JOIN = ja.STATUS_JOIN;
/** The joint before the last 「と」 when listing three or more names (spec 6 「棚をひらいている（見出し）」). */
export let LIST_COMMA = ja.LIST_COMMA;

/** The mark between the two ends of a range (「1〜127」). */
export let RANGE_MARK = ja.RANGE_MARK;
/** The brackets around a short note after a name (「6 歌（入口 1）」). */
export let BRACKET_OPEN = ja.BRACKET_OPEN;
export let BRACKET_CLOSE = ja.BRACKET_CLOSE;

/** Between two sentences put one after the other: nothing after 「。」, a space after ". ". */
export let SENTENCE_JOIN = "";
/** The mark that ends a sentence (where a note in brackets goes before it). */
export let SENTENCE_END = "。";

/** Sets the language of the page. Only main.ts calls it, before anything is drawn (and the tests). */
export function useLanguage(next: Language): void {
  language = next;
  const joins =
    next === "en"
      ? enJoins
      : {
          term: ja.TERM_SEPARATOR,
          status: ja.STATUS_JOIN,
          list: ja.LIST_COMMA,
          sentence: "",
          end: "。",
          range: ja.RANGE_MARK,
          open: ja.BRACKET_OPEN,
          close: ja.BRACKET_CLOSE,
        };
  TERM_SEPARATOR = joins.term;
  STATUS_JOIN = joins.status;
  LIST_COMMA = joins.list;
  SENTENCE_JOIN = joins.sentence;
  SENTENCE_END = joins.end;
  RANGE_MARK = joins.range;
  BRACKET_OPEN = joins.open;
  BRACKET_CLOSE = joins.close;
}

export function textLanguage(): Language {
  return language;
}

type Values = Readonly<Record<string, string | number>>;

/** An English text that changes with a number takes the form of that number (1, or any other). */
function english(entry: EnText, values?: Values): string {
  if (typeof entry === "string") return entry;
  return Math.abs(Number(String(values?.[entry.by]).replace("−", "-"))) === 1 ? entry.one : entry.other;
}

/** The text of `key` with its holes as they are (to split it around the values). */
export function rawText(key: TextKey, values?: Values): string {
  return language === "en" ? english(enTexts[key], values) : texts[key].text;
}

export function t(key: TextKey, values?: Values): string {
  return fill(rawText(key, values), values);
}

/** Whether `key` is a key of the table (keys made from data, as the names of presets). */
export function hasText(key: string): key is TextKey {
  return key in texts;
}

/** The step name with its holes (spec 3 (Y19)). */
export function stepNameText(kind: StepName["kind"]): string {
  return language === "en" ? enStepNames[kind] : stepNames[kind].text;
}

/**
 * The term and its tip. `term` is the Japanese word (the key, as in `use:term={"バッファ"}`); in English the English word of
 * a term is found too, for places that look up a name as it is shown.
 */
export function findTerm(term: string): ShownTerm | undefined {
  if (language === "ja") return findJaTerm(term);
  const known = findJaTerm(term) ? enTerms[term] : undefined;
  return known ?? Object.values(enTerms).find((e) => e.term === term);
}
