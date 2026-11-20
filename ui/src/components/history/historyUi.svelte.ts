// 取り消しの履歴の帯の、画面だけが持つ物（仕様書 3 章 Y19、4 章）: えらんだ行、それがキーでえらんだ物か。
// 行をえらぶのは画面だけ。「ここまで戻す」か Enter で HistoryJump を 1 つ送る。

import { historyRowBook, type AppState, type Command, type HistoryRowStore } from "../../bridge";

export const historyUi = $state({
  /** えらんだ行の番号（1 から）。 */
  selected: null as number | null,
  /** キーでえらんだ（まわりに gray.10 の 1px のフォーカス）。押してえらんだときは false。 */
  byKeys: false,
});

/** いまの手の数で、えらんだ行がまだあるか。なければ null。 */
export function selectedRow(history: AppState["history"]): number | null {
  const s = historyUi.selected;
  return s !== null && s >= 1 && s <= history.total ? s : null;
}

export function pickRow(index: number): void {
  historyUi.selected = index;
  historyUi.byKeys = false;
}

/** ↑（1）は新しい手、↓（−1）は古い手。えらんでいなければ、いまの手から。 */
export function historyStep(state: AppState, delta: 1 | -1): boolean {
  const { total, current } = state.history;
  if (total === 0) return false;
  const from = selectedRow(state.history) ?? current;
  historyUi.selected = from === 0 && delta === 1 ? 1 : Math.max(1, Math.min(total, from + delta));
  historyUi.byKeys = true;
  return true;
}

/** Home: いまの手へ。ぜんぶ取り消してあれば、えらぶ行はない。 */
export function historyHome(state: AppState): boolean {
  historyUi.selected = state.history.current >= 1 ? state.history.current : null;
  historyUi.byKeys = true;
  return true;
}

/** えらんだ行へとぶ意図。いまの手、行がまだ届いていない、えらんでいないときは null。 */
export function historyJumpCommand(state: AppState, index = selectedRow(state.history), book: HistoryRowStore = historyRowBook()): Command | null {
  if (index === null || index === state.history.current) return null;
  const row = book.row(index);
  return row ? { type: "historyJump", index, step: row.step } : null;
}

/** 帯にフォーカスがあるか（キーを帯に向けるか）。 */
export function historyFocused(doc: Document): boolean {
  return doc.activeElement?.closest("[data-history-keys]") != null;
}
