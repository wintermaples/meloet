// 曲のダイアログ（仕様書 6 章「保存していない」「保存できない」「曲のファイルがこわれている」）の文とボタン。DOM を知らない純粋な関数。
import type { SongAnswer, SongDialogView } from "../bridge";
import { t, type TextKey } from "../text/ja";

export interface SongDialogButton {
  answer: SongAnswer;
  label: string;
  /** 明るい枠の、Enter で押されるボタン。 */
  primary: boolean;
}

export interface SongDialogLook {
  body: string;
  /** 左から。並びは音の設定のダイアログと同じく、やめるが左、決めるボタンが右はし。 */
  buttons: SongDialogButton[];
}

const button = (answer: SongAnswer, key: TextKey, primary = false): SongDialogButton => ({ answer, label: t(key), primary });

export function songDialogLook(d: SongDialogView): SongDialogLook {
  switch (d.kind) {
    case "unsaved":
      return {
        body: t("msg.unsaved", { song: d.title }),
        buttons: [button("cancel", "button.cancel"), button("dontSave", "button.dontSave"), button("save", "button.save", true)],
      };
    case "cannotSave":
      return {
        body: t("msg.cannotSave"),
        buttons: [button("cancel", "button.cancel"), button("chooseLocation", "button.chooseLocation", true)],
      };
    case "broken":
      return d.previous
        ? {
            body: t("msg.songBroken"),
            buttons: [button("cancel", "button.cancel"), button("openPrevious", "button.openPrevious", true)],
          }
        : { body: t("msg.songBrokenFirst"), buttons: [button("cancel", "button.cancel", true)] };
  }
}

/** Enter で送る答え。 */
export function songDialogEnter(d: SongDialogView): SongAnswer {
  return songDialogLook(d).buttons.find((b) => b.primary)!.answer;
}
