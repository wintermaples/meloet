// The content of the context menu of the note area (docs/history/design-notes.ja.md B-60). Does not know the DOM.
// On a note: 「切り取り」「コピー」「貼りつけ」「複製」「消す」 / 「クオンタイズ」「もとの位置へ戻す」.
// On an empty place: 「貼りつけ」 (to the pressed place snapped to the grid), 「ぜんぶ選ぶ」. The keys on the right of rows are written the same way as band tabs (Mod is Ctrl on Linux).

import { t } from "../../text";
import type { MenuEntry } from "../menu/contextMenu.svelte";
import type { PianoRollKey } from "./commands";

export interface RollMenuFacts {
  /** A note was pressed. */
  onNote: boolean;
  /** Recording or in count-in (operations that change the song are not accepted). */
  busy: boolean;
  /** There is something to paste. */
  clipboard: boolean;
  /** There is an original take (a quantized take). */
  origin: boolean;
  /** The paste position for an empty place (the pressed place snapped to the grid, a tick in the song). */
  pasteAt: number;
  mac: boolean;
}

export interface RollMenuActions {
  /** Sends the same intent as the key (pianoRollCommand). */
  key(k: PianoRollKey): void;
  unquantize(): void;
  pasteAt(tick: number): void;
}

export function rollMenu(f: RollMenuFacts, act: RollMenuActions): MenuEntry[] {
  const mod = (letter: string) => `${f.mac ? t("key.cmd") : t("key.ctrl")} ${letter}`;
  const edit = !f.busy;
  const paste = f.clipboard && edit;
  if (!f.onNote) {
    return [
      { kind: "row", label: "menu.paste", key: mod(t("key.paste")), usable: paste, run: () => act.pasteAt(f.pasteAt) },
      { kind: "row", label: "menu.selectAll", key: mod(t("key.selectAll")), usable: true, run: () => act.key({ kind: "selectAll" }) },
    ];
  }
  return [
    { kind: "row", label: "menu.cut", key: mod(t("key.cut")), usable: edit, run: () => act.key({ kind: "cut" }) },
    { kind: "row", label: "menu.copy", key: mod(t("key.copy")), usable: true, run: () => act.key({ kind: "copy" }) },
    { kind: "row", label: "menu.paste", key: mod(t("key.paste")), usable: paste, run: () => act.key({ kind: "paste" }) },
    { kind: "row", label: "menu.duplicate", key: mod(t("key.duplicate")), usable: edit, run: () => act.key({ kind: "duplicate" }) },
    { kind: "row", label: "menu.delete", key: t("key.delete"), usable: edit, run: () => act.key({ kind: "deleteNotes" }) },
    { kind: "separator" },
    { kind: "row", label: "menu.quantize", key: t("key.quantize"), usable: edit, run: () => act.key({ kind: "quantize" }) },
    { kind: "row", label: "menu.unquantize", usable: edit && f.origin, run: act.unquantize },
  ];
}
