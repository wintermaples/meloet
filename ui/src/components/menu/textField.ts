// 文字の枠の右クリックの一覧（design-notes B-60）: 切り取り ・ コピー ・ 貼りつけ ・ ぜんぶ選ぶ。
// 貼りつけは「使えない」。WebKitGTK 2.52 は、殻（Tauri の enable_clipboard_access）が許していないと execCommand("paste") が
// 何もせず false を返す。queryCommandEnabled("paste") はそれでも true を返すので、それでは見分けられない。枠の中の Mod V は効く。

import { t } from "../../text/ja";
import type { MenuEntry } from "./contextMenu.svelte";

const NON_TEXT_INPUTS = new Set(["button", "checkbox", "color", "file", "hidden", "image", "radio", "range", "reset", "submit"]);

export type TextField = HTMLInputElement | HTMLTextAreaElement;

/** 右クリックされた物が文字の枠なら、その枠。 */
export function textFieldOf(target: EventTarget | null): TextField | null {
  if (target instanceof HTMLTextAreaElement) return target.disabled ? null : target;
  if (target instanceof HTMLInputElement) return NON_TEXT_INPUTS.has(target.type) || target.disabled ? null : target;
  return null;
}

function exec(doc: Document, name: string): void {
  try {
    doc.execCommand(name);
  } catch {
    // 何もしない（その WebView では、その操作ができない）。
  }
}

/** 一覧の行。mod は「Ctrl X」のようにキーを書く関数。行う前に、枠にフォーカスと、ひらいたときの選択を戻す。 */
export function textFieldRows(field: TextField, mod: (key: string) => string, doc: Document = document): MenuEntry[] {
  const start = field.selectionStart;
  const end = field.selectionEnd;
  const selected = start !== null && end !== null && start !== end;
  const writable = !field.readOnly;
  const back = () => {
    field.focus();
    if (start !== null && end !== null) {
      try {
        field.setSelectionRange(start, end);
      } catch {
        // 選択を持たない種類の枠（number など）。
      }
    }
  };
  return [
    { kind: "row", label: "menu.cut", key: mod(t("key.cut")), usable: selected && writable, run: () => (back(), exec(doc, "cut")) },
    { kind: "row", label: "menu.copy", key: mod(t("key.copy")), usable: selected, run: () => (back(), exec(doc, "copy")) },
    { kind: "row", label: "menu.paste", key: mod(t("key.paste")), usable: false, run: () => {} },
    { kind: "row", label: "menu.selectAll", key: mod(t("key.selectAll")), usable: field.value.length > 0, run: () => (field.focus(), field.select()) },
  ];
}
