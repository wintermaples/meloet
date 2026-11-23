// Context menu of text boxes (docs/history/design-notes.ja.md B-60): cut, copy, paste, select all.
// Paste is disabled. In WebKitGTK 2.52, execCommand("paste") does nothing and returns false unless the shell (Tauri's enable_clipboard_access)
// allows it. queryCommandEnabled("paste") still returns true, so it cannot tell. Mod V inside the box works.

import { t } from "../../text";
import type { MenuEntry } from "./contextMenu.svelte";

const NON_TEXT_INPUTS = new Set(["button", "checkbox", "color", "file", "hidden", "image", "radio", "range", "reset", "submit"]);

export type TextField = HTMLInputElement | HTMLTextAreaElement;

/** The text box that was right-clicked, if it is one. */
export function textFieldOf(target: EventTarget | null): TextField | null {
  if (target instanceof HTMLTextAreaElement) return target.disabled ? null : target;
  if (target instanceof HTMLInputElement) return NON_TEXT_INPUTS.has(target.type) || target.disabled ? null : target;
  return null;
}

function exec(doc: Document, name: string): void {
  try {
    doc.execCommand(name);
  } catch {
    // Do nothing (that WebView cannot do this action).
  }
}

/** The list rows. mod is a function that writes a key like 「Ctrl X」. Before acting, give the box back its focus and the selection it had on open. */
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
        // A kind of box that has no selection (number and others).
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
