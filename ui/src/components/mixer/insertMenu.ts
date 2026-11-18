// インサートの枠の右クリックの一覧（B-86: 「外す」だけ）。

import type { Command, InsertRef } from "../../bridge";
import type { MenuEntry } from "../menu/contextMenu.svelte";

export function insertMenu(insert: InsertRef, send: (c: Command) => void): MenuEntry[] {
  return [{ kind: "row", label: "mixer.removeEffect", usable: true, run: () => send({ type: "removeEffect", insert }) }];
}
