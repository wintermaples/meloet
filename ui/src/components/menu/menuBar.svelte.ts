// The open menu of the menu bar (docs/history/design-notes.ja.md B-59). Only the screen holds it. Read by the key handler (F10) and MenuBar.svelte.

export const menuBar = $state({
  /** Index of the open menu (File 0 to Track 5). null means closed. */
  open: null as number | null,
  /** Whether it was opened by keyboard (F10, ← →). If so, focus goes to the first usable row. */
  byKeys: false,
});

export function openMenuBar(index: number, byKeys: boolean): void {
  menuBar.byKeys = byKeys;
  menuBar.open = index;
}

/** true if it closed (it was open). */
export function closeMenuBar(): boolean {
  if (menuBar.open === null) return false;
  menuBar.open = null;
  return true;
}
