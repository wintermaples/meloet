// State of the params panel that only the screen has. The status line reads it too (「パラメータをさがしています…」「…を動かしています」; mocks F-ParamSearch, F-ParamFew).

export const paramUi = $state({
  /** The text in the search box. Emptied when the chosen plugin changes. */
  query: "",
  /** How many match the search box. */
  matches: 0,
  /** While a row of the panel is dragged: the plugin name and the parameter name. */
  moving: null as { name: string; param: string } | null,
  /** The row whose value is being typed (for built-in effects, the number in the table; for plugins, ParamRow.id). */
  typing: null as number | null,
});

/** Closes the row whose value is being typed. true if it closed one. */
export function stopTyping(): boolean {
  if (paramUi.typing === null) return false;
  paramUi.typing = null;
  return true;
}
