// 右クリックの一覧（design-notes B-60）。右クリックされた部品が openContextMenu を呼び、App にひとつだけ置く
// ContextMenu.svelte が描く。形は仕様書 2 章の「一覧」（行の右にキー）。WebKit の一覧は、どこでも出さない。
// 選ばれていない物を右クリックしたら、部品がそれだけを選んでから呼ぶ。どの道具でも右クリックは一覧。

import type { TextKey } from "../../text/ja";

export interface MenuRow {
  kind: "row";
  label: TextKey;
  /** 行の右に書くキー（書いたままの字）。なければ出さない。 */
  key?: string;
  usable: boolean;
  run: () => void;
}

export interface MenuSeparator {
  kind: "separator";
}

export type MenuEntry = MenuRow | MenuSeparator;

export const contextMenu = $state({ open: null as { x: number; y: number; entries: MenuEntry[] } | null });

/** x, y は押した所（CSS px、窓の左上から）。行がなければ何も出さない。 */
export function openContextMenu(x: number, y: number, entries: MenuEntry[]): void {
  contextMenu.open = entries.some((e) => e.kind === "row") ? { x, y, entries } : null;
}

/** とじた（ひらいていた）なら true。Esc の順では「用語の説明やメニュー」の番。 */
export function closeContextMenu(): boolean {
  if (!contextMenu.open) return false;
  contextMenu.open = null;
  return true;
}
