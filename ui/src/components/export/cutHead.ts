// Cuts text that does not fit a one-line field at the head and shows the end (the end of a path is the file name).
// Put one [data-cut-text] (the text itself) inside the field (node). While it does not fit, the field gets data-cut.
// The 「…」 and right alignment are on the CSS side (it looks at data-cut).

/** Whether the text does not fit the field. The width is minus the inner padding on left and right. */
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
  // A text change, a font arriving, or a field width change all change the size of the text or the field.
  const observer = new ResizeObserver(measure);
  observer.observe(node);
  if (text) observer.observe(text);
  measure();
  return { destroy: () => observer.disconnect() };
}
