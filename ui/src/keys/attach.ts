// キーの決まり（keys.ts）を画面につなぐ。Esc の順番と、「音が出ました」の案内を消す所もここ。
import type { AppState, Command } from "../bridge";
import { prView, zoomArrangeBy, zoomBy } from "../components/pianoroll/view.svelte";
import { songDialogEnter } from "../components/songDialog";
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

/** 録音中とカウントイン中は、曲を新しくしたり、ひらいたり、保存したりできない。 */
export function songBusy(state: AppState): boolean {
  return state.transport.state === "recording" || state.transport.state === "countIn";
}

/** 音の出口がまだのあいだ、Space と R は何もしない（ヤチヨさんの決め 2026-11-15）。止めるほうの Space は効く。 */
function command(action: KeyAction, state: AppState): Command | null {
  const transport = state.transport;
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
    case "playStop":
      return transport.canPlay || transport.state !== "stopped" ? { type: "playStop" } : null;
    case "record":
      return transport.canPlay ? { type: "record" } : null;
    case "toStart":
      return { type: "goToStart" };
    case "loop":
      return { type: "toggleLoop" };
    case "metronome":
      return { type: "toggleMetronome" };
    case "stepTake":
      return { type: "stepTake", delta: action.delta };
    case "deleteSelected":
      return { type: "deleteSelected" };
    // 録音中とカウントイン中は、曲の名前の一覧と同じく使えない。
    case "newSong":
      return songBusy(state) ? null : { type: "newSong" };
    case "openSong":
      return songBusy(state) ? null : { type: "openSong" };
    case "saveSong":
      return songBusy(state) ? null : { type: "saveSong" };
    case "undo":
      return { type: "undo" };
    case "redo":
      return { type: "redo" };
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
    if (plainEnter && state.workspace.songDialog) {
      ev.preventDefault();
      const d = state.workspace.songDialog;
      deps.send({ type: "answerSongDialog", dialog: d.id, answer: songDialogEnter(d) });
    } else if (plainEnter && state.workspace.settings) {
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

  if (action.kind === "zoom") {
    // ブラウザの拡大（Ctrl ＋）に渡さない。横の拡大は画面だけが持つ見た目なので、Rust には送らない。
    ev.preventDefault();
    zoomHorizontal(action.delta, state);
    return;
  }

  const c = command(action, state);
  // Mod つきは、使えないときもブラウザの動き（Ctrl S の保存など）に渡さない。
  const withMod = ["newSong", "openSong", "saveSong", "undo", "redo"].includes(action.kind);
  if (withMod) ev.preventDefault();
  if (c) {
    // 打っている最中の Home は、枠の中のカーソルも動かす。
    if (!typing) ev.preventDefault();
    deps.send(c);
  }
}

/** Mod ＋ ・ Mod −: 最後に押したのがピアノロールで、ひらいていればピアノロール、そうでなければアレンジ。 */
export function zoomHorizontal(delta: 1 | -1, state: AppState): void {
  const pianoRollOpen = state.workspace.layout.lower.some((b) => b.band === "pianoRoll");
  if (prView.focus === "pianoRoll" && pianoRollOpen) zoomBy(delta, state.workspace.scale);
  else zoomArrangeBy(delta);
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
