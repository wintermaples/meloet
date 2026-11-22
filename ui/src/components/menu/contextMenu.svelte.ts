// 右クリックの一覧（docs/history/design-notes.ja.md B-60）。右クリックされた部品が openContextMenu を呼び、App にひとつだけ置く
// ContextMenu.svelte が描く。形は仕様書 2 章の「一覧」（行の右にキー）。WebKit の一覧は、どこでも出さない。
// 選ばれていない物を右クリックしたら、部品がそれだけを選んでから呼ぶ。どの道具でも右クリックは一覧。
// インサートと音源の枠は選ばない（選ぶとプラグインの窓がひらく）。行が、その枠をさして動く。

import { t, type TextKey } from "../../text/ja";
import type { ListEntry } from "./menus";

export interface MenuRow {
  kind: "row";
  label: TextKey;
  /** 穴を埋めた名前（「色 1」）。なければ label の文。 */
  text?: string;
  /** 行の右に書くキー（書いたままの字）。なければ出さない。 */
  key?: string;
  usable: boolean;
  run: () => void;
  /** オン ・ オフの行の選ぶ印（一覧の行と同じ）。 */
  checked?: boolean;
  /** ▸ の子の一覧。 */
  sub?: MenuEntry[];
  /** 色の一覧の行の、色の見本（トラックの色 1〜5）。 */
  swatch?: number;
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

/** 右クリックの一覧の行を、一覧（MenuList）の行にする。上のメニューでも同じ行を使うときに。 */
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
          checked: e.checked,
          sub: e.sub ? listEntries(e.sub) : undefined,
          swatch: e.swatch,
        },
  );
}
