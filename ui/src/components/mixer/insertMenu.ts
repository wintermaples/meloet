// インサートの枠の右クリックの一覧（B-86: 「外す」。止まったプラグインは「もう一度読みこむ」「外す」: 仕様書 6 章「プラグインが止まった」）。

import type { Command, InsertRef, InsertView } from "../../bridge";
import type { MenuEntry } from "../menu/contextMenu.svelte";

export function insertMenu(insert: InsertRef, view: InsertView | null, send: (c: Command) => void): MenuEntry[] {
  if (view?.effect.kind === "plugin" && view.effect.state === "stopped") {
    const slot = { kind: "insert", insert } as const;
    return [
      { kind: "row", label: "button.reload", usable: true, run: () => send({ type: "reloadPlugin", slot }) },
      { kind: "row", label: "button.remove", usable: true, run: () => send({ type: "removePlugin", slot }) },
    ];
  }
  return [{ kind: "row", label: "mixer.removeEffect", usable: true, run: () => send({ type: "removeEffect", insert }) }];
}
