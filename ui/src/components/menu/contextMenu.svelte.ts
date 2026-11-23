// Context menu (docs/history/design-notes.ja.md B-60). The right-clicked part calls openContextMenu, and the single
// ContextMenu.svelte in App draws it. The form is the 「一覧」 (List) of spec 2 (keys on the right of rows). The WebKit menu is never shown.
// When something not selected is right-clicked, the part selects only it before calling. Right click opens the list with every tool.
// Insert and instrument slots are not selected (selecting opens the plugin window). The rows act on that slot.

import { t, type TextKey } from "../../text";
import type { ListEntry } from "./menus";

export interface MenuRow {
  kind: "row";
  label: TextKey;
  /** The name with the holes filled (「色 1」). Otherwise the label text. */
  text?: string;
  /** The key written on the right of the row (as written). Not shown if missing. */
  key?: string;
  usable: boolean;
  run: () => void;
  /** The reason it is disabled (shown inside the row only on hover, as in list rows). */
  hint?: string;
  /** The choice mark of an on/off row (the same as list rows). */
  checked?: boolean;
  /** The ▸ child list. */
  sub?: MenuEntry[];
  /** The color sample of a color list row (track colors 1–5). */
  swatch?: number;
}

export interface MenuSeparator {
  kind: "separator";
}

export type MenuEntry = MenuRow | MenuSeparator;

export const contextMenu = $state({ open: null as { x: number; y: number; entries: MenuEntry[] } | null });

/** x, y is where it was pressed (CSS px from the top left of the window). Shows nothing if there are no rows. */
export function openContextMenu(x: number, y: number, entries: MenuEntry[]): void {
  contextMenu.open = entries.some((e) => e.kind === "row") ? { x, y, entries } : null;
}

/** true if it closed (it was open). It takes the 「用語の説明やメニュー」 turn in the Esc order. */
export function closeContextMenu(): boolean {
  if (!contextMenu.open) return false;
  contextMenu.open = null;
  return true;
}

/** Turns context menu rows into list (MenuList) rows. For when the menu bar uses the same rows. */
export function listEntries(entries: readonly MenuEntry[]): ListEntry[] {
  return entries.map((e) =>
    e.kind === "separator"
      ? e
      : {
          kind: "row",
          label: e.text ?? t(e.label),
          key: e.key,
          usable: e.usable,
          run: e.run,
          hint: e.hint,
          checked: e.checked,
          sub: e.sub ? listEntries(e.sub) : undefined,
          swatch: e.swatch,
        },
  );
}
