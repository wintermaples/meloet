// Connects the key rules (keys.ts) to the screen. The Esc order and clearing the 「音が出ました」 hint are here too.
// Menu bar rows that have a key also go through runKeyAction here (so the choice between the piano roll and the arrange is made in one place).
import { clockOffsetUs, type AppState, type Command } from "../bridge";
import { aboutBack } from "../components/about/about.svelte";
import { keyListBack } from "../components/keylist/keyList.svelte";
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
import { joinTarget, pasteableOnSelected, renameClipTarget, repeatTarget } from "../components/clipEdits";
import { openRepeatBox } from "../components/arrange/repeatBox.svelte";
import { startClipRename } from "../components/clipName.svelte";
import { historyFocused, historyHome, historyJumpCommand, historyStep } from "../components/history/historyUi.svelte";
import { timelineKeyCommand } from "../components/timeline/keys";
import { pickTimeline } from "../components/timeline/pick";
import { closeMenu, menuVisible } from "../lib/menu";
import { hideTip, tipVisible } from "../lib/tip.svelte";
import { current } from "../state/app.svelte";
import { copied } from "../state/copied";
import {
  isTyping,
  keyAction,
  keyInputOf,
  platformOf,
  type KeyAction,
} from "./keys";
import { dialogInFront, latencyCancel } from "./front";
import { forgetTyped, letGoTyped, switchTypedKeys, typedKeyDown, typedKeyUp, type TypedDeps } from "./typed.svelte";

export interface KeyDeps {
  state(): AppState | null;
  send(command: Command): void;
  /** Sends without waiting for the rev (the keys of the PC keyboard). send when not given. */
  post?(command: Command): void;
  /** For the times of the PC keyboard's keys (for tests; normally the bridge's). */
  clockOffsetUs?(): Promise<number>;
  doc?: Document;
  nav?: { platform?: string; userAgent?: string };
  /** The take the piano roll shows (for tests; normally from shown.svelte.ts). */
  pianoRoll?: () => Shown | null;
  /** The current play position (for tests; normally from the telemetry). */
  playTick?: () => number;
}

export const pianoRollOpen = (state: AppState) =>
  state.workspace.layout.lower.some((l) => l.band === "pianoRoll");

/** Intents that go to the chosen clip in the arrange when they do not go to the piano roll. */
const ARRANGE_TOO = new Set<KeyAction["kind"]>([
  "deleteSelected",
  "duplicate",
  "selectAll",
  "cut",
  "copy",
  "paste",
]);

/**
 * The intent of a key for the piano roll: when the piano roll is open and was pressed last (with toPianoRoll, whatever was pressed last).
 * undefined means it is not a piano roll intent (it goes to the arrange or elsewhere).
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
      // When both points and notes are selected, whichever was pressed last.
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
  // Paste follows what was copied last, wherever the keys point.
  if (action.kind === "paste" && copied.last !== null) {
    if (copied.last === "clips" || !pianoRollOpen(state)) return undefined;
    toPianoRoll = true;
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

/** While recording or counting in, a song cannot be made new, opened, saved or changed (Rust does not accept it either). */
export function songBusy(state: AppState): boolean {
  return (
    state.transport.state === "recording" || state.transport.state === "countIn"
  );
}

const PLAIN_DELETE = { key: "Delete", code: "Delete", ctrl: false, meta: false, alt: false, shift: false };

/**
 * Delete in the arrange when automation points or a tempo point, time signature flag or marker was pressed last (7-readings A-21):
 * the same rules as the key (handleKeydown asks them first). null: the chosen clips. "none": taken, but nothing to delete.
 */
function pickedDelete(state: AppState): Command | "none" | null {
  const c = { typing: false, busy: songBusy(state), toArrange: prView.focus === "arrange" || !pianoRollOpen(state) };
  return laneDeleteCommand(PLAIN_DELETE, c, state.automation) ?? timelineKeyCommand(PLAIN_DELETE, c, state.timeline);
}

/** Whether 「消す」 of the menu bar deletes something when keys go to the arrange (the start of the song is not deleted). */
export function arrangeDeleteUsable(state: AppState): boolean {
  const picked = pickedDelete(state);
  if (picked === "none") return false;
  if (picked?.type === "deleteTimeline") {
    const at = picked.target;
    return !(at.kind === "tempoPoint" && at.tick === 0) && !(at.kind === "meterChange" && at.bar === 0);
  }
  return picked !== null || (state.arrange.selectedClips.length > 0 && !songBusy(state));
}

/** While there is no output yet, Space and R do nothing (the owner's decision). Space to stop still works. */
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
    // Sent with nothing to keep too: the status line then says so.
    case "keepPlayed":
      return { type: "keepPlayed" };
    case "typedKeys":
      return { type: "typedKeys", on: !state.keyboards.typedKeys };
    case "toStart":
      return { type: "goToStart" };
    case "loop":
      return { type: "toggleLoop" };
    case "metronome":
      return { type: "toggleMetronome" };
    case "stepTake":
      return { type: "stepTake", delta: action.delta };
    // Rust decides which clip to cut (a chosen clip under the play line, else the one under the line on the selected track; spec 4).
    case "splitClips":
      return busy ? null : { type: "splitClips" };
    case "joinClips": {
      const ids = joinTarget(state);
      return ids === null || busy ? null : { type: "joinClips", clips: ids };
    }
    case "deleteSelected": {
      const picked = pickedDelete(state);
      if (picked === "none") return null;
      return picked ?? { type: "deleteSelected" };
    }
    // When keys go to the arrange (B-71). Mod D duplicates right after the chosen clips.
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
    // Paste goes at the play position on the selected track (Rust moves it to the start of the bar).
    case "paste":
      return !pasteableOnSelected(state) || busy
        ? null
        : { type: "pasteClips", to: null };
    // Disabled while recording or counting in, like the song name list.
    case "newSong":
      return busy ? null : { type: "newSong" };
    case "openSong":
      return busy ? null : { type: "openSong" };
    case "saveSong":
      return busy ? null : { type: "saveSong" };
    case "saveSongAs":
      return busy ? null : { type: "saveSongAs" };
    // Does not open during count-in, recording or export (B-94-74).
    case "export":
      return busy || state.export.dialog !== null || state.export.run?.state === "running" ? null : { type: "openExport" };
    case "quit":
      return { type: "quit" };
    case "undo":
      return { type: "undo" };
    case "redo":
      return { type: "redo" };
    // Mute is an undo step that changes the song; solo is a toggle only for listening (B-70). Mute is accepted while recording too (A-14).
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
    // When an insert is chosen and the mixer or the params panel is shown (spec 4: "Does not work (one letter; mixer)").
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

/** The command to send for an intent. null does nothing (disabled). toPianoRoll: from the 「ノート」 menu (to the piano roll when it is open). */
export function keyCommand(
  action: KeyAction,
  state: AppState,
  deps: KeyDeps,
  toPianoRoll = false,
): Command | null {
  const forPianoRoll = pianoRollKey(action, state, deps, toPianoRoll);
  return forPianoRoll !== undefined ? forPianoRoll : command(action, state);
}

/** Whether the intent is handled only in the screen (zoom across, tools, F10, the name input box). */
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

/** The track F2 renames (the selected track). Not while recording or counting in. */
function renameTrackTarget(state: AppState): number | null {
  const track = state.song.playTarget;
  return track === null ||
    songBusy(state) ||
    !state.song.tracks.some((x) => x.id === track)
    ? null
    : track;
}

/** F2: with a clip chosen, that clip's name; with only a track chosen, the track's name (spec 4, round 10). */
function renameTarget(state: AppState): { kind: "clip" | "track"; id: number } | null {
  if (songBusy(state)) return null;
  const clip = renameClipTarget(state);
  if (clip !== null) return { kind: "clip", id: clip };
  const track = renameTrackTarget(state);
  return track === null ? null : { kind: "track", id: track };
}

/** Whether it can be done now (for the disabled state of menu rows). */
export function keyActionUsable(
  action: KeyAction,
  deps: KeyDeps,
  toPianoRoll = false,
): boolean {
  const state = deps.state();
  if (!state) return false;
  if (action.kind === "tool") return pianoRollOpen(state);
  if (action.kind === "renameTrack") return renameTarget(state) !== null;
  if (action.kind === "repeatClips") return repeatTarget(state) !== null;
  if (screenOnly(action)) return true;
  return keyCommand(action, state, deps, toPianoRoll) !== null;
}

/** Carries out an intent. true when something was done. Both keys and menu rows go through here. */
export function runKeyAction(
  action: KeyAction,
  deps: KeyDeps,
  toPianoRoll = false,
): boolean {
  const state = deps.state();
  if (!state) return false;
  switch (action.kind) {
    case "zoom":
      // Zoom across is a view only the screen holds, so it is not sent to Rust.
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
      const target = renameTarget(state);
      if (target?.kind === "clip") startClipRename(target.id);
      else if (target?.kind === "track") startRename(target.id);
      return target !== null;
    }
    case "historyStep":
      return historyStep(state, action.delta);
    case "historyHome":
      return historyHome(state);
    case "repeatClips": {
      const clips = repeatTarget(state);
      if (clips) openRepeatBox(clips);
      return clips !== null;
    }
    case "typedKeys":
      switchTypedKeys(!state.keyboards.typedKeys, { ...typedDeps(deps), send: (c) => deps.send(c) });
      return true;
  }
  const c = keyCommand(action, state, deps, toPianoRoll);
  if (c) deps.send(c);
  return c !== null;
}

function typedDeps(deps: KeyDeps): TypedDeps {
  return {
    post: (c) => (deps.post ?? deps.send)(c),
    clockOffsetUs: deps.clockOffsetUs ?? clockOffsetUs,
  };
}

/** Esc while 「キーで弾く」 is on turns it off (after a drag and the menus, before going back; spec R2). */
function typedKeysOff(state: AppState, deps: KeyDeps): boolean {
  if (!state.keyboards.typedKeys) return false;
  runKeyAction({ kind: "typedKeys" }, deps);
  return true;
}

const SHELF_KEYS = new Set<KeyAction["kind"]>(["shelfStep", "shelfConfirm"]);

/** Mod keys are not passed on to the browser (Ctrl S to save, and so on) even when disabled. */
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
  "joinClips",
  "repeatClips",
  "typedKeys",
]);

/**
 * At the shelf's turn of Esc, if an audition is on, stop it before closing (spec 3: Esc stops it and goes back to the track's sound;
 * the next Esc closes the shelf). A dialog comes first if there is one.
 */
function auditionToCancel(state: AppState): boolean {
  return dialogInFront(state) === null && state.workspace.layout.rightOpen.includes("shelf") && shelfHasPick(state);
}

/** When the undo history band has focus and there is no dialog, ↑ ↓ Home Enter Esc belong to the band (spec 4). */
function historyKeys(state: AppState, doc: Document): boolean {
  return dialogInFront(state) === null && state.workspace.layout.lower.some((l) => l.band === "history") && historyFocused(doc);
}

/** The third turn of Esc (term tips and menus). true when it closed the menu bar menu, a context menu, or a list such as the status line's. */
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

/** Takes one keydown. Exported for tests. */
export function handleKeydown(ev: KeyboardEvent, deps: KeyDeps): void {
  const doc = deps.doc ?? document;
  const state = deps.state();
  if (!state || ev.isComposing || MODIFIERS.has(ev.key)) return;
  const typing = isTyping(doc);

  // While 「キーで弾く」 is on, letters and digits are the PC keyboard's, before any shortcut (spec R2). They are a
  // keyboard, so they leave the 「音が出ました」 hint.
  if (!state.keyboards.typedKeys) forgetTyped();
  else if (!typing && typedKeyDown(ev, typedDeps(deps))) return;

  // Any action other than the keyboard clears the 「音が出ました」 hint (the owner's decision).
  if (state.workspace.hint === "soundCameOut")
    deps.send({ type: "dismissHint" });

  // A part that took the key says so (lists moved to body are heard on document too, before this handler).
  if (ev.defaultPrevented) return;

  const front = dialogInFront(state);
  const context = front
    ? "main"
    : state.workspace.chooser
      ? "outputChooser"
      : shelfHasKeys(state)
        ? "shelf"
        : historyKeys(state, doc)
          ? "history"
          : "main";

  // Keys for tempo and time signature changes and markers (K, Alt ← →, F2 with a marker chosen, Delete of the chosen ones). Esc does not go through here.
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
    // About, the key list and the latency dialog have no Enter of their own here (their focused button or box takes it).
    if (plainEnter && front === "scanStopped") {
      // The bright button on the right, 「閉じる」 (the same rule as the save dialogs of spec 3).
      ev.preventDefault();
      deps.send({ type: "closeScanStopped" });
    } else if (plainEnter && front === "song" && state.workspace.songDialog) {
      ev.preventDefault();
      const d = state.workspace.songDialog;
      deps.send({
        type: "answerSongDialog",
        dialog: d.id,
        answer: songDialogEnter(d),
      });
    } else if (plainEnter && front === "export" && state.export.dialog) {
      // The bright button on the right, 「書き出す」 (does nothing when it cannot be pressed).
      ev.preventDefault();
      if (state.export.dialog.canExport) deps.send({ type: "startExport" });
    } else if (plainEnter && front === "exported") {
      // 「書き出しました」: the bright button 「閉じる」 (round 12 E9).
      ev.preventDefault();
      deps.send({ type: "closeExport" });
    } else if (plainEnter && front === "settings") {
      ev.preventDefault();
      deps.send({ type: "settingsApply" });
    }
    return;
  }

  if (action.kind === "back") {
    ev.preventDefault();
    // Spec 4: Esc during a drag only goes back to before the move. Then spec 3: the box being typed in → term tips and menus → dialog → …
    if (cancelDrag()) return;
    // Esc while dragging in a file means the file is not dropped (spec 6: 「やめるには、窓の外で離すか Esc」).
    if (state.workspace.fileDrop) {
      deps.send({ type: "cancelFileDrop" });
      return;
    }
    if (typing) (doc.activeElement as HTMLElement | null)?.blur();
    else if (!closeMenus() && !typedKeysOff(state, deps) && !aboutBack() && !keyListBack())
      deps.send(
        context === "history"
          ? { type: "toggleBand", band: "history" }
          : front === "export" || front === "exported"
            ? { type: "closeExport" }
            : front === "latency"
              ? latencyCancel(state.workspace.latencyDialog?.measure ?? null)
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

  // Not passed on to the browser's zoom (Ctrl +), save (Ctrl S) and so on.
  if (WITH_MOD.has(action.kind) || action.kind === "menuBar")
    ev.preventDefault();
  // Space never presses the focused button, even when disabled (it is only play / stop, or the shelf sample).
  if (action.kind === "playStop" || action.kind === "shelfSample")
    ev.preventDefault();
  // While typing, Home also moves the cursor in the box, and Shift M, Shift S also type the letter.
  // ↑ ↓ and Enter in the shelf belong to the shelf even in the search box (they do not move the cursor).
  if (runKeyAction(action, deps) && (!typing || SHELF_KEYS.has(action.kind)))
    ev.preventDefault();
}

/**
 * The keyup of Space. Buttons are pressed on keyup (WebKit and Firefox), so it is stopped here too when not typing.
 * With deps, a key of the PC keyboard is let go (R2).
 */
export function handleKeyup(ev: KeyboardEvent, doc: Document = document, deps?: KeyDeps): void {
  if (deps) typedKeyUp(ev, typedDeps(deps));
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

/** Mod + and Mod −: the piano roll when it is open and was pressed last; otherwise the arrange. */
export function zoomHorizontal(delta: 1 | -1, state: AppState): void {
  if (prView.focus === "pianoRoll" && pianoRollOpen(state))
    zoomBy(delta, state.workspace.scale);
  else zoomArrangeBy(delta);
}

/** A mouse button press clears the hint. Pressing the shelf sends keys to the shelf; pressing elsewhere stops that (lists that open on top are outside .app). */
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

/** Connects keys and the mouse to document. Returns a function that disconnects them. */
export function attachKeys(deps: KeyDeps): () => void {
  const doc = deps.doc ?? document;
  const win = doc.defaultView;
  const onKey = (ev: KeyboardEvent) => handleKeydown(ev, deps);
  const onKeyup = (ev: KeyboardEvent) => handleKeyup(ev, doc, deps);
  const onPointer = (ev: PointerEvent) => handlePointerdown(deps, ev.target);
  // The keyup of a held key may never come once the window is not in front (R2): its notes are let go now.
  const onLeave = () => letGoTyped(typedDeps(deps));
  const onVisibility = () => {
    if (doc.visibilityState === "hidden") onLeave();
  };
  doc.addEventListener("keydown", onKey);
  doc.addEventListener("keyup", onKeyup);
  doc.addEventListener("pointerdown", onPointer, true);
  doc.addEventListener("visibilitychange", onVisibility);
  win?.addEventListener("blur", onLeave);
  return () => {
    doc.removeEventListener("keydown", onKey);
    doc.removeEventListener("keyup", onKeyup);
    doc.removeEventListener("pointerdown", onPointer, true);
    doc.removeEventListener("visibilitychange", onVisibility);
    win?.removeEventListener("blur", onLeave);
  };
}
