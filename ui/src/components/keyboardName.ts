// The name a keyboard is shown with. 「パソコンのキーボード」 of 「キーで弾く」 (R2) is named by the screen: Rust gives it
// the id TYPED_KEYBOARD_ID and no name.
import { TYPED_KEYBOARD_ID, type KeyboardView } from "../bridge";
import { t } from "../text";

export function keyboardName(k: KeyboardView): string {
  return k.id === TYPED_KEYBOARD_ID ? t("input.typedKeyboard") : k.name;
}

/** A keyboard named by the status line (`keyboardFound`): no name is the PC keyboard. */
export function namedKeyboard(name: string): string {
  return name === "" ? t("input.typedKeyboard") : name;
}
