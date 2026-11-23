// What only the screen keeps for the undo history band (spec 3 (Y19), spec 4): the chosen row, and whether it was chosen with keys.
// Only the screen chooses rows. 「ここまで戻す」 or Enter sends one HistoryJump.

import { historyRowBook, type AppState, type Command, type HistoryRowStore } from "../../bridge";

export const historyUi = $state({
  /** The chosen row number (from 1). */
  selected: null as number | null,
  /** Chosen with keys (a 1px gray.10 focus around it). False when chosen by pressing. */
  byKeys: false,
});

/** Whether the chosen row still exists for the current step count. Null if not. */
export function selectedRow(history: AppState["history"]): number | null {
  const s = historyUi.selected;
  return s !== null && s >= 1 && s <= history.total ? s : null;
}

export function pickRow(index: number): void {
  historyUi.selected = index;
  historyUi.byKeys = false;
}

/** ↑ (1) is a newer step, ↓ (−1) an older one. With nothing chosen, starts from the current step. */
export function historyStep(state: AppState, delta: 1 | -1): boolean {
  const { total, current } = state.history;
  if (total === 0) return false;
  const from = selectedRow(state.history) ?? current;
  historyUi.selected = from === 0 && delta === 1 ? 1 : Math.max(1, Math.min(total, from + delta));
  historyUi.byKeys = true;
  return true;
}

/** Home: to the current step. If everything is undone, there is no row to choose. */
export function historyHome(state: AppState): boolean {
  historyUi.selected = state.history.current >= 1 ? state.history.current : null;
  historyUi.byKeys = true;
  return true;
}

/** The intent to jump to the chosen row. Null for the current step, for a row that has not arrived yet, or when nothing is chosen. */
export function historyJumpCommand(state: AppState, index = selectedRow(state.history), book: HistoryRowStore = historyRowBook()): Command | null {
  if (index === null || index === state.history.current) return null;
  const row = book.row(index);
  return row ? { type: "historyJump", index, step: row.step } : null;
}

/** Whether the band has focus (whether keys go to the band). */
export function historyFocused(doc: Document): boolean {
  return doc.activeElement?.closest("[data-history-keys]") != null;
}
