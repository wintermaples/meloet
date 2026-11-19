// インサートの枠の右クリックの一覧（仕様書 3 章 86-1 ・ 86-2 ・ 86-3、モック R9-Inserts: 入れかえる… ・ 上へ動かす ・ 下へ動かす ／ 外す）。
// とばす（13-A）は、モックの「ここにも入る」のとおり、下へ動かすの下（選ぶ印つき、キー B）。
// 止まったプラグインは「もう一度読みこむ」「外す」（仕様書 6 章「プラグインが止まった」）。

import type { Command, InsertRef, InsertView } from "../../bridge";
import { INSERT_SLOTS } from "../../bridge/types";
import { t } from "../../text/ja";
import type { MenuEntry } from "../menu/contextMenu.svelte";
import { openShelf } from "../tracks.svelte";

export function insertMenu(insert: InsertRef, view: InsertView | null, send: (c: Command) => void): MenuEntry[] {
  if (view?.effect.kind === "plugin" && view.effect.state === "stopped") {
    const slot = { kind: "insert", insert } as const;
    return [
      { kind: "row", label: "button.reload", usable: true, run: () => send({ type: "reloadPlugin", slot }) },
      { kind: "row", label: "button.remove", usable: true, run: () => send({ type: "removePlugin", slot }) },
    ];
  }
  const filled = view !== null;
  return [
    { kind: "row", label: "insert.replace", usable: filled, run: () => send(openShelf({ kind: "replaceEffect", strip: insert.strip, slot: insert.slot })) },
    { kind: "row", label: "insert.moveUp", usable: filled && insert.slot > 0, run: () => send({ type: "moveInsert", insert, to: insert.slot - 1 }) },
    {
      kind: "row",
      label: "insert.moveDown",
      usable: filled && insert.slot < INSERT_SLOTS - 1,
      run: () => send({ type: "moveInsert", insert, to: insert.slot + 1 }),
    },
    { kind: "row", label: "params.bypass", key: t("key.bypass"), checked: view?.bypassed === true, usable: filled, run: () => send({ type: "toggleBypass", insert }) },
    { kind: "separator" },
    { kind: "row", label: "mixer.removeEffect", key: t("key.delete"), usable: filled, run: () => send({ type: "removeEffect", insert }) },
  ];
}
