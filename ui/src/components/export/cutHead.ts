// 1 行の欄に入りきらない字を、頭のほうで省いて終わりを見せる（道の終わりはファイルの名前）。
// 欄（node）の中に [data-cut-text]（字そのもの）を 1 つ置く。入りきらない間、欄に data-cut がつく。
// 「…」と右よせは CSS の側（data-cut を見る）。

/** 字が欄に入りきらないか。幅は左右の内がわのすきまを引いた数。 */
export function isCut(textWidth: number, boxWidth: number, padLeft: number, padRight: number): boolean {
  return textWidth > boxWidth - padLeft - padRight;
}

export function cutHead(node: HTMLElement): { destroy(): void } {
  const text = node.querySelector<HTMLElement>("[data-cut-text]");
  const measure = (): void => {
    if (!text) return;
    const style = getComputedStyle(node);
    const cut = isCut(text.offsetWidth, node.clientWidth, Number.parseFloat(style.paddingLeft) || 0, Number.parseFloat(style.paddingRight) || 0);
    node.toggleAttribute("data-cut", cut);
  };
  // 字が変わる、書体が届く、欄の幅が変わる、のどれでも字か欄の大きさが変わる。
  const observer = new ResizeObserver(measure);
  observer.observe(node);
  if (text) observer.observe(text);
  measure();
  return { destroy: () => observer.disconnect() };
}
