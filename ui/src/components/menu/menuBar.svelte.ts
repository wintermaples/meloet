// 上のメニューの行（docs/history/design-notes.ja.md B-59）の、ひらいているメニュー。画面だけが持つ。キーの係（F10）と MenuBar.svelte が読む。

export const menuBar = $state({
  /** ひらいているメニューの番号（ファイル 0 〜 トラック 5）。null はとじている。 */
  open: null as number | null,
  /** キーボードでひらいた（F10、← →）か。そのときは最初の使える行にフォーカスを置く。 */
  byKeys: false,
});

export function openMenuBar(index: number, byKeys: boolean): void {
  menuBar.byKeys = byKeys;
  menuBar.open = index;
}

/** とじた（ひらいていた）なら true。 */
export function closeMenuBar(): boolean {
  if (menuBar.open === null) return false;
  menuBar.open = null;
  return true;
}
