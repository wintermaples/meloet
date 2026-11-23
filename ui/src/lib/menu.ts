// The open menu (such as the 「拡大率」 list of the status line). Only one on the screen. Kept here to decide the Esc order.

let closer: (() => void) | null = null;

/** A menu opened; the one before is closed. Returns the function to call when it closes. */
export function menuOpened(close: () => void): () => void {
  if (closer && closer !== close) closer();
  closer = close;
  return () => {
    if (closer === close) closer = null;
  };
}

/** Whether a menu is open now. */
export function menuVisible(): boolean {
  return closer !== null;
}

/** Closes the open menu. */
export function closeMenu(): void {
  const close = closer;
  closer = null;
  close?.();
}
