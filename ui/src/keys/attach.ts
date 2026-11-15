// キーの決まり（keys.ts）を画面につなぐ。Esc の順番と、「音が出ました」の案内を消す所もここ。
import type { AppState, Command } from "../bridge";
import { closeMenu, menuVisible } from "../lib/menu";
import { hideTip, tipVisible } from "../lib/tip.svelte";
import { isTyping, keyAction, keyInputOf, platformOf, type KeyAction } from "./keys";

export interface KeyDeps {
  state(): AppState | null;
  send(command: Command): void;
  doc?: Document;
  nav?: { platform?: string; userAgent?: string };
}

const MODIFIERS = new Set(["Shift", "Control", "Alt", "Meta", "AltGraph", "CapsLock", "OS"]);

function command(action: KeyAction): Command | null {
  switch (action.kind) {
    case "arrange":
      return { type: "goArrange" };
    case "toggleBand":
      return { type: "toggleBand", band: action.band };
    case "audioSettings":
      return { type: "openSettings", advanced: false };
    case "scale":
      return { type: "setScale", percent: action.scale };
    case "chooserUp":
      return { type: "chooserMove", delta: -1 };
    case "chooserDown":
      return { type: "chooserMove", delta: 1 };
    case "chooserConfirm":
      return { type: "chooserConfirm" };
    default:
      return null;
  }
}

/** keydown をひとつ受ける。テストのために外に出してある。 */
export function handleKeydown(ev: KeyboardEvent, deps: KeyDeps): void {
  const doc = deps.doc ?? document;
  const state = deps.state();
  if (!state || ev.isComposing || MODIFIERS.has(ev.key)) return;

  // 鍵盤以外の操作で「音が出ました」の案内を消す（ヤチヨさんの決め）。
  if (state.workspace.hint === "soundCameOut") deps.send({ type: "dismissHint" });

  const typing = isTyping(doc);
  const context = state.workspace.chooser ? "outputChooser" : "main";
  const action = keyAction(keyInputOf(ev), typing, platformOf(deps.nav ?? navigator), context);

  if (!action) {
    const plainEnter = ev.key === "Enter" && !ev.shiftKey && !ev.altKey && !ev.ctrlKey && !ev.metaKey;
    if (plainEnter && state.workspace.settings) {
      ev.preventDefault();
      deps.send({ type: "settingsApply" });
    }
    return;
  }

  if (action.kind === "back") {
    ev.preventDefault();
    // 仕様書 3 章: 文字を打っている枠 → 用語の説明やメニュー → ダイアログ → …
    if (typing) (doc.activeElement as HTMLElement | null)?.blur();
    else if (tipVisible()) hideTip();
    else if (menuVisible()) closeMenu();
    else deps.send({ type: "back" });
    return;
  }

  const c = command(action);
  if (c) {
    ev.preventDefault();
    deps.send(c);
  }
}

/** マウスのボタンを押したら、案内を消す。 */
export function handlePointerdown(deps: KeyDeps): void {
  if (deps.state()?.workspace.hint === "soundCameOut") deps.send({ type: "dismissHint" });
}

/** document にキーとマウスをつなぐ。はずす関数を返す。 */
export function attachKeys(deps: KeyDeps): () => void {
  const doc = deps.doc ?? document;
  const onKey = (ev: KeyboardEvent) => handleKeydown(ev, deps);
  const onPointer = () => handlePointerdown(deps);
  doc.addEventListener("keydown", onKey);
  doc.addEventListener("pointerdown", onPointer, true);
  return () => {
    doc.removeEventListener("keydown", onKey);
    doc.removeEventListener("pointerdown", onPointer, true);
  };
}
