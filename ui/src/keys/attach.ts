// キーの決まり（keys.ts）を画面につなぐ。Esc の順番と、「音が出ました」の案内を消す所もここ。
// 上のメニューの行も、キーのある行はここの runKeyAction を通す（ピアノロールとアレンジのどちらに向けるかの決めを 1 か所に）。
import type { AppState, Command } from "../bridge";
import { aboutBack } from "../components/about/about.svelte";
import { closeContextMenu } from "../components/menu/contextMenu.svelte";
import { closeMenuBar, openMenuBar } from "../components/menu/menuBar.svelte";
import {
  pianoRollCommand,
  type PianoRollKey,
} from "../components/pianoroll/commands";
import type { Shown } from "../components/pianoroll/geom";
import { shown } from "../components/pianoroll/shown.svelte";
import { shownFor } from "../components/pianoroll/shownNotes";
import {
  cancelDrag,
  prView,
  zoomArrangeBy,
  zoomBy,
} from "../components/pianoroll/view.svelte";
import { selectedInsert } from "../components/params/selected";
import { songDialogEnter } from "../components/songDialog";
import {
  shelfConfirmCommand,
  shelfFavoriteCommand,
  shelfHasKeys,
  shelfHasPick,
  shelfSampleCommand,
  shelfStepCommand,
  shelfToggle,
  shelfUi,
  startRename,
} from "../components/tracks.svelte";
import { shownTick } from "../components/time";
import { laneDeleteCommand } from "../components/automation/laneKeys";
import { historyFocused, historyHome, historyJumpCommand, historyStep } from "../components/history/historyUi.svelte";
import { timelineKeyCommand } from "../components/timeline/keys";
import { pickTimeline } from "../components/timeline/pick";
import { closeMenu, menuVisible } from "../lib/menu";
import { hideTip, tipVisible } from "../lib/tip.svelte";
import { current } from "../state/app.svelte";
import {
  isTyping,
  keyAction,
  keyInputOf,
  platformOf,
  type KeyAction,
} from "./keys";

export interface KeyDeps {
  state(): AppState | null;
  send(command: Command): void;
  doc?: Document;
  nav?: { platform?: string; userAgent?: string };
  /** ピアノロールが見せているテイク（テスト用。ふだんは shown.svelte.ts から）。 */
  pianoRoll?: () => Shown | null;
  /** いまの再生の位置（テスト用。ふだんは計器から）。 */
  playTick?: () => number;
}

export const pianoRollOpen = (state: AppState) =>
  state.workspace.layout.lower.some((l) => l.band === "pianoRoll");

/** ピアノロールに向けないときは、アレンジのえらんだクリップへ向ける意図。 */
const ARRANGE_TOO = new Set<KeyAction["kind"]>([
  "deleteSelected",
  "duplicate",
  "selectAll",
  "cut",
  "copy",
  "paste",
]);

/**
 * ピアノロールに向けるキーの意図。ピアノロールがひらいていて、最後に押したのがピアノロールのとき（toPianoRoll なら、最後に押した所によらず）。
 * undefined はピアノロールの意図ではない（アレンジかほかへ）。
 */
function pianoRollKey(
  action: KeyAction,
  state: AppState,
  deps: KeyDeps,
  toPianoRoll: boolean,
): Command | null | undefined {
  let key: PianoRollKey;
  switch (action.kind) {
    case "nudge":
      key = { kind: "nudge", steps: action.steps, keys: action.keys };
      break;
    case "deleteSelected": {
      // 点とノートの両方を選んでいるときは、最後に押したほう。
      const pr = state.pianoRoll;
      const points = pr.selectedControls.length > 0;
      key =
        points &&
        (pr.selectedNotes.length === 0 || prView.picked === "controls")
          ? { kind: "deleteControls" }
          : { kind: "deleteNotes" };
      break;
    }
    case "quantize":
      key = { kind: "quantize" };
      break;
    case "selectAll":
    case "duplicate":
    case "cut":
    case "copy":
    case "paste":
      key = { kind: action.kind };
      break;
    default:
      return undefined;
  }
  if (!pianoRollOpen(state) || (!toPianoRoll && prView.focus !== "pianoRoll"))
    return ARRANGE_TOO.has(action.kind) ? undefined : null;
  const notes = deps.pianoRoll
    ? deps.pianoRoll()
    : shownFor(shown.take, shown.clip);
  const playTick = deps.playTick
    ? deps.playTick()
    : shownTick(state.transport, current.telemetry, null);
  return pianoRollCommand(key, { state, shown: notes, playTick });
}

const MODIFIERS = new Set([
  "Shift",
  "Control",
  "Alt",
  "Meta",
  "AltGraph",
  "CapsLock",
  "OS",
]);

/** 録音中とカウントイン中は、曲を新しくしたり、ひらいたり、保存したり、曲を変えたりできない（Rust も受けない）。 */
export function songBusy(state: AppState): boolean {
  return (
    state.transport.state === "recording" || state.transport.state === "countIn"
  );
}

/** 音の出口がまだのあいだ、Space と R は何もしない（ヤチヨさんの決め 2026-11-15）。止めるほうの Space は効く。 */
function command(action: KeyAction, state: AppState): Command | null {
  const transport = state.transport;
  const busy = songBusy(state);
  const clips = state.arrange.selectedClips;
  const track = state.song.playTarget;
  switch (action.kind) {
    case "arrange":
      return { type: "goArrange" };
    case "toggleBand":
      return action.band === "shelf"
        ? shelfToggle(state)
        : { type: "toggleBand", band: action.band };
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
      return transport.canPlay || transport.state !== "stopped"
        ? { type: "playStop" }
        : null;
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
    // どのクリップを切るかは Rust が決める（えらんだクリップで再生の線がかかる物、なければ選んだトラックの線の下の 1 つ。仕様書 4 章）。
    case "splitClips":
      return busy ? null : { type: "splitClips" };
    case "deleteSelected":
      return { type: "deleteSelected" };
    // アレンジにキーが向いているとき（B-71）。Mod D はえらんだクリップのすぐうしろに複製。
    case "duplicate":
      return clips.length === 0 || busy
        ? null
        : { type: "duplicateClips", clips };
    case "selectAll":
      return state.song.tracks.some((t) => t.clips.length > 0)
        ? { type: "selectAllClips" }
        : null;
    case "copy":
      return clips.length === 0 ? null : { type: "copyClips", clips };
    case "cut":
      return clips.length === 0 || busy ? null : { type: "cutClips", clips };
    // 貼りつけは、選んだトラックの再生の位置（Rust が小節の頭に寄せる）。
    case "paste":
      return state.arrange.clipboardClips === 0 || busy
        ? null
        : { type: "pasteClips", to: null };
    // 録音中とカウントイン中は、曲の名前の一覧と同じく使えない。
    case "newSong":
      return busy ? null : { type: "newSong" };
    case "openSong":
      return busy ? null : { type: "openSong" };
    case "saveSong":
      return busy ? null : { type: "saveSong" };
    case "saveSongAs":
      return busy ? null : { type: "saveSongAs" };
    // カウントイン中 ・ 録音中 ・ 書き出している間はひらかない（B-94-74）。
    case "export":
      return busy || state.export.dialog !== null || state.export.run?.state === "running" ? null : { type: "openExport" };
    case "quit":
      return { type: "quit" };
    case "undo":
      return { type: "undo" };
    case "redo":
      return { type: "redo" };
    // ミュートは曲を変える手、ソロは聞くときだけの切りかえ（B-70）。ミュートは録音中も受ける（A-14）。
    case "trackMute":
      return track === null ? null : { type: "toggleMute", track };
    case "trackSolo":
      return track === null ? null : { type: "toggleSolo", track };
    case "shelfStep":
      return shelfStepCommand(state, action.delta);
    case "shelfConfirm":
      return shelfConfirmCommand(state);
    case "shelfSample":
      return shelfSampleCommand(state);
    case "shelfFavorite":
      return shelfFavoriteCommand(state);
    // えらんでいるインサートがあり、ミキサーかパラメータの欄が見えているとき（仕様書 4 章「効かない（一文字 ・ ミキサー）」）。
    case "historyConfirm":
      return historyJumpCommand(state);
    case "bypass": {
      const insert = selectedInsert(state.params);
      const seen = state.workspace.layout.lower.some((l) => l.band === "mixer") || state.workspace.layout.rightFront === "params";
      return insert && seen ? { type: "toggleBypass", insert } : null;
    }
    default:
      return null;
  }
}

/** 意図から送るコマンド。null は何もしない（使えない）。toPianoRoll: メニューの「ノート」から（ピアノロールがひらいていれば、そこへ）。 */
export function keyCommand(
  action: KeyAction,
  state: AppState,
  deps: KeyDeps,
  toPianoRoll = false,
): Command | null {
  const forPianoRoll = pianoRollKey(action, state, deps, toPianoRoll);
  return forPianoRoll !== undefined ? forPianoRoll : command(action, state);
}

/** 画面の中だけで行う意図（横の拡大、道具、F10、名前の入力の枠）か。 */
function screenOnly(action: KeyAction): boolean {
  return (
    action.kind === "zoom" ||
    action.kind === "tool" ||
    action.kind === "menuBar" ||
    action.kind === "renameTrack" ||
    action.kind === "historyStep" ||
    action.kind === "historyHome"
  );
}

/** F2 で名前を変えるトラック（選んだトラック）。録音中とカウントイン中は変えられない。 */
function renameTarget(state: AppState): number | null {
  const track = state.song.playTarget;
  return track === null ||
    songBusy(state) ||
    !state.song.tracks.some((x) => x.id === track)
    ? null
    : track;
}

/** いま行えるか（メニューの行の「使えない」）。 */
export function keyActionUsable(
  action: KeyAction,
  deps: KeyDeps,
  toPianoRoll = false,
): boolean {
  const state = deps.state();
  if (!state) return false;
  if (action.kind === "tool") return pianoRollOpen(state);
  if (action.kind === "renameTrack") return renameTarget(state) !== null;
  if (screenOnly(action)) return true;
  return keyCommand(action, state, deps, toPianoRoll) !== null;
}

/** 意図を行う。何かしたら true。キーとメニューの行の両方がここを通る。 */
export function runKeyAction(
  action: KeyAction,
  deps: KeyDeps,
  toPianoRoll = false,
): boolean {
  const state = deps.state();
  if (!state) return false;
  switch (action.kind) {
    case "zoom":
      // 横の拡大は画面だけが持つ見た目なので、Rust には送らない。
      zoomHorizontal(action.delta, state);
      return true;
    case "tool":
      if (!pianoRollOpen(state)) return false;
      prView.tool = action.tool;
      return true;
    case "menuBar":
      if (!closeMenuBar()) openMenuBar(0, true);
      return true;
    case "renameTrack": {
      const track = renameTarget(state);
      if (track !== null) startRename(track);
      return track !== null;
    }
    case "historyStep":
      return historyStep(state, action.delta);
    case "historyHome":
      return historyHome(state);
  }
  const c = keyCommand(action, state, deps, toPianoRoll);
  if (c) deps.send(c);
  return c !== null;
}

const SHELF_KEYS = new Set<KeyAction["kind"]>(["shelfStep", "shelfConfirm"]);

/** Mod つきは、使えないときもブラウザの動き（Ctrl S の保存など）に渡さない。 */
const WITH_MOD = new Set<KeyAction["kind"]>([
  "newSong",
  "openSong",
  "saveSong",
  "saveSongAs",
  "export",
  "quit",
  "undo",
  "redo",
  "selectAll",
  "duplicate",
  "cut",
  "copy",
  "paste",
  "zoom",
]);

/**
 * Esc の「棚」の番で、ためし弾きをしていれば、とじる前にためし弾きをやめる（仕様書 3 章: Esc でやめて元のトラックの音に戻る。
 * 次の Esc で棚をとじる）。ダイアログがあれば、ダイアログが先。
 */
function auditionToCancel(state: AppState): boolean {
  const ws = state.workspace;
  return (
    ws.songDialog === null &&
    ws.settings === null &&
    state.export.dialog === null &&
    !state.plugins.stoppedOpen &&
    ws.layout.rightOpen.includes("shelf") &&
    shelfHasPick(state)
  );
}

/** 取り消しの履歴の帯にフォーカスがあり、ダイアログがないとき、↑ ↓ Home Enter Esc は帯のもの（仕様書 4 章）。 */
function historyKeys(state: AppState, doc: Document): boolean {
  const ws = state.workspace;
  return (
    ws.songDialog === null &&
    ws.settings === null &&
    state.export.dialog === null &&
    !state.plugins.stoppedOpen &&
    ws.layout.lower.some((l) => l.band === "history") &&
    historyFocused(doc)
  );
}

/** Esc の 3 つめの番（用語の説明やメニュー）。上のメニュー、右クリックの一覧、状態行などの一覧のどれかをとじたら true。 */
function closeMenus(): boolean {
  if (tipVisible()) {
    hideTip();
    return true;
  }
  if (closeMenuBar() || closeContextMenu()) return true;
  if (menuVisible()) {
    closeMenu();
    return true;
  }
  return false;
}

/** keydown をひとつ受ける。テストのために外に出してある。 */
export function handleKeydown(ev: KeyboardEvent, deps: KeyDeps): void {
  const doc = deps.doc ?? document;
  const state = deps.state();
  if (!state || ev.isComposing || MODIFIERS.has(ev.key)) return;

  // 鍵盤以外の操作で「音が出ました」の案内を消す（ヤチヨさんの決め）。
  if (state.workspace.hint === "soundCameOut")
    deps.send({ type: "dismissHint" });

  const typing = isTyping(doc);
  const context = state.workspace.chooser
    ? "outputChooser"
    : shelfHasKeys(state)
      ? "shelf"
      : historyKeys(state, doc)
        ? "history"
        : "main";

  // テンポ ・ 拍子の変更とマーカーのキー（K、Alt ← →、マーカーをえらんでいるときの F2、えらんだ物の Delete）。Esc はここを通さない。
  if (context === "main" && ev.key !== "Escape") {
    const lane = laneDeleteCommand(
      keyInputOf(ev),
      { typing, busy: songBusy(state), toArrange: prView.focus === "arrange" || !pianoRollOpen(state) },
      state.automation,
    );
    if (lane) {
      ev.preventDefault();
      if (lane !== "none") deps.send(lane);
      return;
    }
    const tl = timelineKeyCommand(
      keyInputOf(ev),
      {
        typing,
        busy: songBusy(state),
        toArrange: prView.focus === "arrange" || !pianoRollOpen(state),
      },
      state.timeline,
    );
    if (tl) {
      ev.preventDefault();
      deps.send(tl);
      if (tl.type === "addMarker" || tl.type === "stepMarker") pickTimeline();
      return;
    }
  }

  const action = keyAction(
    keyInputOf(ev),
    typing,
    platformOf(deps.nav ?? navigator),
    context,
  );

  if (!action) {
    const plainEnter =
      ev.key === "Enter" &&
      !ev.shiftKey &&
      !ev.altKey &&
      !ev.ctrlKey &&
      !ev.metaKey;
    if (plainEnter && state.plugins.stoppedOpen) {
      // 右はしの明るいボタン「閉じる」（3 章の保存まわりのダイアログと同じ決まり）。
      ev.preventDefault();
      deps.send({ type: "closeScanStopped" });
    } else if (plainEnter && state.workspace.songDialog) {
      ev.preventDefault();
      const d = state.workspace.songDialog;
      deps.send({
        type: "answerSongDialog",
        dialog: d.id,
        answer: songDialogEnter(d),
      });
    } else if (plainEnter && state.export.dialog) {
      // 右はしの明るいボタン「書き出す」（押せないときは何もしない）。
      ev.preventDefault();
      if (state.export.dialog.canExport) deps.send({ type: "startExport" });
    } else if (plainEnter && state.workspace.settings) {
      ev.preventDefault();
      deps.send({ type: "settingsApply" });
    }
    return;
  }

  if (action.kind === "back") {
    ev.preventDefault();
    // 仕様書 4 章: ドラッグ中の Esc は、動かす前に戻すだけ。そのあと 3 章: 文字を打っている枠 → 用語の説明やメニュー → ダイアログ → …
    if (cancelDrag()) return;
    // ファイルを持ってきている間の Esc は、持ってきた物を落とさないことにする（仕様書 6 章「やめるには、窓の外で離すか Esc」）。
    if (state.workspace.fileDrop) {
      deps.send({ type: "cancelFileDrop" });
      return;
    }
    if (typing) (doc.activeElement as HTMLElement | null)?.blur();
    else if (!closeMenus() && !aboutBack())
      deps.send(
        context === "history"
          ? { type: "toggleBand", band: "history" }
          : state.export.dialog
            ? { type: "closeExport" }
            : auditionToCancel(state)
              ? { type: "shelfCancel" }
              : { type: "back" },
      );
    return;
  }

  if (action.kind === "tool") {
    if (runKeyAction(action, deps)) ev.preventDefault();
    return;
  }

  // ブラウザの拡大（Ctrl ＋）や保存（Ctrl S）などに渡さない。
  if (WITH_MOD.has(action.kind) || action.kind === "menuBar")
    ev.preventDefault();
  // Space は、使えないときも、フォーカスのあるボタンを押さない（再生 ・ 停止か、棚のサンプルだけ）。
  if (action.kind === "playStop" || action.kind === "shelfSample")
    ev.preventDefault();
  // 打っている最中の Home は枠の中のカーソルも動かし、Shift M ・ Shift S は文字も入る。
  // 棚の ↑ ↓ ・ Enter は、さがす枠の中でも棚のもの（カーソルを動かさない）。
  if (runKeyAction(action, deps) && (!typing || SHELF_KEYS.has(action.kind)))
    ev.preventDefault();
}

/** Space の keyup。ボタンは keyup で押される（WebKit と Firefox）ので、文字を打っていなければ、ここでも止める。 */
export function handleKeyup(ev: KeyboardEvent, doc: Document = document): void {
  const space = ev.key === " " || ev.code === "Space";
  if (
    space &&
    !ev.shiftKey &&
    !ev.ctrlKey &&
    !ev.metaKey &&
    !ev.altKey &&
    !isTyping(doc)
  )
    ev.preventDefault();
}

/** Mod ＋ ・ Mod −: 最後に押したのがピアノロールで、ひらいていればピアノロール、そうでなければアレンジ。 */
export function zoomHorizontal(delta: 1 | -1, state: AppState): void {
  if (prView.focus === "pianoRoll" && pianoRollOpen(state))
    zoomBy(delta, state.workspace.scale);
  else zoomArrangeBy(delta);
}

/** マウスのボタンを押したら、案内を消す。棚を押せば棚にキーを向け、画面のほかの所を押せば外す（上にひらく一覧は .app の外）。 */
export function handlePointerdown(
  deps: KeyDeps,
  target?: EventTarget | null,
): void {
  if (deps.state()?.workspace.hint === "soundCameOut")
    deps.send({ type: "dismissHint" });
  const el = target instanceof Element ? target : null;
  if (el?.closest("[data-shelf]")) shelfUi.keys = true;
  else if (el?.closest(".app")) shelfUi.keys = false;
}

/** document にキーとマウスをつなぐ。はずす関数を返す。 */
export function attachKeys(deps: KeyDeps): () => void {
  const doc = deps.doc ?? document;
  const onKey = (ev: KeyboardEvent) => handleKeydown(ev, deps);
  const onKeyup = (ev: KeyboardEvent) => handleKeyup(ev, doc);
  const onPointer = (ev: PointerEvent) => handlePointerdown(deps, ev.target);
  doc.addEventListener("keydown", onKey);
  doc.addEventListener("keyup", onKeyup);
  doc.addEventListener("pointerdown", onPointer, true);
  return () => {
    doc.removeEventListener("keydown", onKey);
    doc.removeEventListener("keyup", onKeyup);
    doc.removeEventListener("pointerdown", onPointer, true);
  };
}
