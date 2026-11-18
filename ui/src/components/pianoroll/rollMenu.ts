// ノートの場所の右クリックの一覧（docs/design-notes.md B-60）の中身。DOM を知らない。
// ノートの上: 切り取り ・ コピー ・ 貼りつけ ・ 複製 ・ 消す ／ クオンタイズ ・ もとの位置へ戻す。
// 空いた所: 貼りつけ（押した所をグリッドに寄せた位置へ）・ ぜんぶ選ぶ。行の右のキーは帯札と同じ書きかた（Mod は Linux で Ctrl）。

import { t } from "../../text/ja";
import type { MenuEntry } from "../menu/contextMenu.svelte";
import type { PianoRollKey } from "./commands";

export interface RollMenuFacts {
  /** ノートの上を押した。 */
  onNote: boolean;
  /** 録音中かカウントイン中（曲を変える操作は受けない）。 */
  busy: boolean;
  /** 貼りつける物がある。 */
  clipboard: boolean;
  /** もとのテイクがある（クオンタイズしたテイク）。 */
  origin: boolean;
  /** 空いた所の貼りつけの位置（押した所をグリッドに寄せた、曲の中のティック）。 */
  pasteAt: number;
  mac: boolean;
}

export interface RollMenuActions {
  /** キーと同じ意図（pianoRollCommand）を送る。 */
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
