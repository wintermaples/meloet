// Text and buttons of the song dialog (spec 6 "Not saved", "Cannot save", "Song file is broken"). Pure functions that do not know the DOM.
import type { SongAnswer, SongDialogView } from "../bridge";
import { t, type TextKey } from "../text";

export interface SongDialogButton {
  answer: SongAnswer;
  label: string;
  /** The bright-bordered button that Enter presses. */
  primary: boolean;
}

export interface SongDialogLook {
  body: string;
  /** From the left. Same order as the sound settings dialog: cancel on the left, the deciding button at the right end. */
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
    case "deleteRecovery":
      return {
        body: t("msg.deleteRecoveryFile"),
        buttons: [button("keep", "button.keepRecoveryFile"), button("delete", "button.deleteRecoveryFile", true)],
      };
  }
}

/** The answer sent on Enter. */
export function songDialogEnter(d: SongDialogView): SongAnswer {
  return songDialogLook(d).buttons.find((b) => b.primary)!.answer;
}
