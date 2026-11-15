// ひらいているメニュー（状態行の「拡大率」の一覧など）。画面に 1 つだけ。Esc の順番を決めるために、ここで覚える。

let closer: (() => void) | null = null;

/** メニューがひらいた。前のメニューはとじる。とじたときに呼ぶ関数を返す。 */
export function menuOpened(close: () => void): () => void {
  if (closer && closer !== close) closer();
  closer = close;
  return () => {
    if (closer === close) closer = null;
  };
}

/** いまメニューがひらいているか。 */
export function menuVisible(): boolean {
  return closer !== null;
}

/** ひらいているメニューをとじる。 */
export function closeMenu(): void {
  const close = closer;
  closer = null;
  close?.();
}
