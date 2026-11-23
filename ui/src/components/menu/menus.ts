// Content of the menu bar (docs/history/design-notes.ja.md B-59). Does not know the DOM. Rows with a key go the same way as the key (runKeyAction in keys/attach.ts).

import { SCALE_MODES, type AppState, type Band, type Command } from "../../bridge";
import { THEME_IDS } from "../../design/themes";
import { SCALES } from "../../design/tokens";
import { keyActionUsable, pianoRollOpen, runKeyAction, songBusy, type KeyDeps } from "../../keys/attach";
import type { KeyAction, Platform } from "../../keys/keys";
import { t, type TextKey } from "../../text/ja";
import { gridText, pitchClassName, scaleModeName } from "../format";
import { GRID_DIVISIONS, indexesOf } from "../pianoroll/geom";
import { shown } from "../pianoroll/shown.svelte";
import { shownFor } from "../pianoroll/shownNotes";
import { prView, zoomRowsBy } from "../pianoroll/view.svelte";
import { about, openAbout } from "../about/about.svelte";
import { keyList, openKeyList } from "../keylist/keyList.svelte";
import { trackRows } from "../trackMenu";
import { openShelf } from "../tracks.svelte";
import { listEntries } from "./contextMenu.svelte";

export interface ListRow {
  kind: "row";
  label: string;
  /** The key written on the right of the row (as written). */
  key?: string;
  usable: boolean;
  /** The choice mark of on/off rows and choice rows. undefined is an action row (the mark stays empty). */
  checked?: boolean;
  run?: () => void;
  /** The reason it is disabled. Shown inside the row only on hover. */
  hint?: string;
  /** The ▸ child list (opens to the right). */
  sub?: ListEntry[];
  /** Thin text after the name (the place of a recent song). */
  detail?: string;
  /** Color list row (spec 2): a 24×12 color sample after the choice mark. Track colors 1–5. */
  swatch?: number;
}
export interface ListHeading {
  kind: "heading";
  label: string;
}
export interface ListSeparator {
  kind: "separator";
}
export type ListEntry = ListRow | ListHeading | ListSeparator;

export interface TopMenu {
  label: string;
  entries: ListEntry[];
}

export const TOP_MENUS: readonly TextKey[] = ["menu.file", "menu.edit", "menu.view", "menu.play", "menu.notes", "menu.track", "menu.help"];

/** How keys are written (「Ctrl Shift S」「Cmd ,」「Shift M」). Mac uses Cmd and Option (docs/history/design-notes.ja.md A-2). */
export function keyNames(platform: Platform) {
  const mod = platform === "mac" ? t("key.cmd") : t("key.ctrl");
  const alt = platform === "mac" ? t("key.option") : t("key.alt");
  return {
    mod: (k: string) => `${mod} ${k}`,
    modShift: (k: string) => `${mod} ${t("key.shift")} ${k}`,
    modAlt: (k: string) => `${mod} ${alt} ${k}`,
    shift: (k: string) => `${t("key.shift")} ${k}`,
    alt: (k: string) => `${alt} ${k}`,
  };
}

/** The id of the selected track if it is a vocal track. */
function vocalTarget(state: AppState): number | null {
  const track = state.song.tracks.find((x) => x.id === state.song.playTarget);
  return track?.kind.kind === "vocal" ? track.id : null;
}

/** 「最近ひらいた曲 ▸」: newest first, then 「一覧を消す」; only a disabled 「まだありません」 when there are none. */
function recentRows(state: AppState, canOpen: boolean, send: (c: Command) => () => void): ListEntry[] {
  if (state.recent.length === 0) return [{ kind: "row", label: t("menu.noRecent"), usable: false }];
  return [
    ...state.recent.map((r): ListRow => ({ kind: "row", label: r.name, detail: r.place, usable: canOpen, run: send({ type: "openRecent", id: r.id }) })),
    { kind: "separator" },
    { kind: "row", label: t("menu.clearRecent"), usable: true, run: send({ type: "clearRecent" }) },
  ];
}

/** Builds the menus. Sends to deps.send. scale and unfit are for the scale child list. */
export function topMenus(state: AppState, deps: KeyDeps, platform: Platform): TopMenu[] {
  const k = keyNames(platform);
  const busy = songBusy(state);
  const prOpen = pianoRollOpen(state);
  // When checking what can be used, do not read the play position (so it is not rebuilt at 60 Hz; the position does not change what can be used).
  const look: KeyDeps = { ...deps, playTick: () => 0 };
  const send = (c: Command) => () => deps.send(c);

  const act = (label: TextKey, action: KeyAction, key?: string, more: Partial<ListRow> = {}, toPianoRoll = false): ListRow => ({
    kind: "row",
    label: t(label),
    key,
    usable: keyActionUsable(action, look, toPianoRoll),
    run: () => void runKeyAction(action, deps, toPianoRoll),
    ...more,
  });
  const plain = (label: TextKey, usable: boolean, run?: () => void, more: Partial<ListRow> = {}): ListRow => ({
    kind: "row",
    label: t(label),
    usable,
    run,
    ...more,
  });
  const choice = (label: string, checked: boolean, usable: boolean, run: () => void, key?: string): ListRow => ({
    kind: "row",
    label,
    key,
    usable,
    checked,
    run,
  });

  const file: ListEntry[] = [
    act("menu.newSong", { kind: "newSong" }, k.mod(t("key.newSong"))),
    act("menu.openSong", { kind: "openSong" }, k.mod(t("key.openSong"))),
    plain("menu.recentSongs", true, undefined, { sub: recentRows(state, keyActionUsable({ kind: "openSong" }, look), send) }),
    act("menu.save", { kind: "saveSong" }, k.mod(t("key.saveSong"))),
    act("menu.saveAs", { kind: "saveSongAs" }, k.modShift(t("key.saveSong"))),
    { kind: "separator" },
    // At the play line of the selected vocal track, or a new vocal track if none (spec 3 29-A; Rust decides).
    plain("menu.importAudio", !busy, () => deps.send({ type: "importAudio", track: vocalTarget(state) })),
    act("menu.export", { kind: "export" }, k.modShift(t("key.export"))),
    act("menu.audioSettings", { kind: "audioSettings" }, k.mod(t("key.audioSettings"))),
    { kind: "separator" },
    act("menu.quit", { kind: "quit" }, k.mod(t("key.quit"))),
  ];

  // Like the key, to whichever of the piano roll and the arrange the keys go to.
  const toArrange = !(prOpen && prView.focus === "pianoRoll");
  const edit: ListEntry[] = [
    act("menu.undo", { kind: "undo" }, k.mod(t("key.undo")), { usable: state.song.undoCount > 0 && !busy }),
    act("menu.redo", { kind: "redo" }, k.modShift(t("key.undo")), { usable: state.song.redoCount > 0 && !busy }),
    { kind: "separator" },
    act("menu.cut", { kind: "cut" }, k.mod(t("key.cut"))),
    act("menu.copy", { kind: "copy" }, k.mod(t("key.copy"))),
    act("menu.paste", { kind: "paste" }, k.mod(t("key.paste"))),
    act("menu.duplicate", { kind: "duplicate" }, k.mod(t("key.duplicate"))),
    act("menu.delete", { kind: "deleteSelected" }, t("key.delete"), toArrange ? { usable: state.arrange.selectedClips.length > 0 && !busy } : {}),
    { kind: "separator" },
    act("menu.selectAll", { kind: "selectAll" }, k.mod(t("key.selectAll"))),
    act("menu.splitClip", { kind: "splitClips" }, t("key.split")),
    act("menu.joinClips", { kind: "joinClips" }, k.mod(t("key.join"))),
  ];

  const scale = state.workspace.scale;
  const unfit = state.workspace.unfitScales;
  // Band rows of the View menu show whether the band is open with the mark on the left (spec 3).
  const layout = state.workspace.layout;
  const bandOpen = (band: Band) => layout.lower.some((l) => l.band === band) || layout.rightOpen.includes(band);
  const view: ListEntry[] = [
    act("menu.goArrange", { kind: "arrange" }, t("key.arrange")),
    act("menu.pianoRoll", { kind: "toggleBand", band: "pianoRoll" }, t("key.pianoRoll"), { checked: bandOpen("pianoRoll") }),
    act("menu.mixer", { kind: "toggleBand", band: "mixer" }, t("key.mixer"), { checked: bandOpen("mixer") }),
    act("menu.history", { kind: "toggleBand", band: "history" }, t("key.history"), { checked: bandOpen("history") }),
    act("menu.shelf", { kind: "toggleBand", band: "shelf" }, t("key.shelf"), { checked: bandOpen("shelf") }),
    act("menu.params", { kind: "toggleBand", band: "params" }, t("key.params"), { checked: bandOpen("params") }),
    { kind: "separator" },
    act("menu.zoomIn", { kind: "zoom", delta: 1 }, k.mod(t("key.zoomIn"))),
    act("menu.zoomOut", { kind: "zoom", delta: -1 }, k.mod(t("key.zoomOut"))),
    plain("menu.zoomRowsIn", prOpen, () => void zoomRowsBy(1)),
    plain("menu.zoomRowsOut", prOpen, () => void zoomRowsBy(-1)),
    { kind: "separator" },
    plain("menu.scale", true, undefined, {
      sub: SCALES.map((s, i) => {
        const fits = !unfit.includes(s);
        const row = choice(t("status.scale", { scale: s }), s === scale, fits, send({ type: "setScale", percent: s }), k.modAlt(String(i + 1)));
        return fits ? row : { ...row, hint: t("msg.scaleDoesNotFit") };
      }),
    }),
    plain("menu.theme", true, undefined, {
      sub: THEME_IDS.map((id) => choice(t(`theme.${id}`), id === state.workspace.theme, true, send({ type: "setTheme", theme: id }))),
    }),
  ];

  const tr = state.transport;
  const counting = tr.state === "countIn";
  const play: ListEntry[] = [
    plain("menu.playStart", tr.state === "stopped" && tr.canPlay, send({ type: "playStop" }), { key: t("key.playStop") }),
    plain("menu.stop", tr.state !== "stopped", send({ type: "playStop" }), { key: t("key.playStop") }),
    act("menu.record", { kind: "record" }, t("key.record")),
    act("menu.keepPlayed", { kind: "keepPlayed" }, k.shift(t("key.keepLast")), { usable: tr.canKeepPlayed }),
    act("menu.toStart", { kind: "toStart" }, t("key.toStart")),
    act("menu.loop", { kind: "loop" }, t("key.loop"), { checked: state.song.loopOn, usable: !busy }),
    act("menu.metronome", { kind: "metronome" }, t("key.metronome"), { checked: tr.metronome }),
    plain("menu.countIn", !counting, undefined, {
      sub: [0, 1, 2].map((bars) =>
        choice(
          bars === 0 ? t("transport.countInNone") : t("transport.countInBars", { bars }),
          bars === tr.countInBars,
          !counting,
          send({ type: "setCountIn", bars }),
        ),
      ),
    }),
  ];

  const pr = state.pianoRoll;
  const notesShown = deps.pianoRoll ? deps.pianoRoll() : shownFor(shown.take, shown.clip);
  const take = notesShown?.take ?? null;
  const live = prOpen && take !== null && pr.take === take.id;
  const selected = live ? indexesOf(pr.selectedNotes).filter((i) => i < take!.starts.length) : [];
  const canUnquantize = live && take!.origin !== 0 && !busy;
  const tools = [
    { tool: "select", label: "pr.toolSelect", key: "key.toolSelect" },
    { tool: "draw", label: "pr.toolDraw", key: "key.toolDraw" },
    { tool: "erase", label: "pr.toolErase", key: "key.toolErase" },
  ] as const;
  const roots = Array.from({ length: 12 }, (_, root) => root);
  const scaleOf = pr.scale;
  const notes: ListEntry[] = [
    plain("menu.tools", prOpen, undefined, {
      sub: tools.map((x) =>
        choice(t(x.label), prView.tool === x.tool, prOpen, () => void runKeyAction({ kind: "tool", tool: x.tool }, deps), t(x.key)),
      ),
    }),
    act("menu.quantize", { kind: "quantize" }, t("key.quantize"), {}, true),
    plain("menu.unquantize", canUnquantize, () => take && deps.send({ type: "unquantize", take: take.id, notes: selected })),
    plain("menu.grid", prOpen, undefined, {
      sub: GRID_DIVISIONS.map((d) =>
        choice(gridText(d, false), pr.grid.division === d, prOpen, send({ type: "setGrid", division: d, triplet: pr.grid.triplet })),
      ),
    }),
    plain("menu.triplet", prOpen, send({ type: "setGrid", division: pr.grid.division, triplet: !pr.grid.triplet }), {
      checked: pr.grid.triplet,
    }),
    plain("menu.scaleLock", prOpen, undefined, {
      checked: scaleOf.lock,
      sub: [
        choice(t("inspector.on"), scaleOf.lock, prOpen, send({ type: "setMusicalScale", root: scaleOf.root, mode: scaleOf.mode, lock: true, highlight: scaleOf.highlight })),
        choice(t("inspector.off"), !scaleOf.lock, prOpen, send({ type: "setMusicalScale", root: scaleOf.root, mode: scaleOf.mode, lock: false, highlight: scaleOf.highlight })),
        { kind: "heading", label: t("scale.kind") },
        // Choosing a scale turns it on (B-65, the same as the ▾ in the tool row).
        ...SCALE_MODES.map((mode) =>
          choice(
            scaleModeName(mode),
            scaleOf.mode === mode,
            prOpen,
            send({ type: "setMusicalScale", root: scaleOf.root, mode, lock: true, highlight: scaleOf.highlight }),
          ),
        ),
        { kind: "heading", label: t("scale.root") },
        ...roots.map((root) =>
          choice(pitchClassName(root), scaleOf.root === root, prOpen, send({ type: "setMusicalScale", root, mode: scaleOf.mode, lock: true, highlight: scaleOf.highlight })),
        ),
      ],
    }),
  ];

  const clipId = state.arrange.selectedClip;
  const clip = clipId === null ? null : (state.song.tracks.flatMap((x) => x.clips).find((c) => c.id === clipId) ?? null);
  const canStepTake = clip !== null && clip.takes >= 2 && !busy;
  const target = state.song.tracks.find((x) => x.id === state.song.playTarget) ?? null;
  const edits = trackRows(state, target, deps.send);
  const track: ListEntry[] = [
    act("menu.mute", { kind: "trackMute" }, k.shift(t("key.mute")), { checked: target?.muted ?? false }),
    act("menu.solo", { kind: "trackSolo" }, k.shift(t("key.solo")), { checked: target?.soloed ?? false }),
    // Record arm is always on the selected track (spec 3). The ● of a vocal track toggles on and off (24-C).
    target?.kind.kind === "vocal"
      ? plain("menu.arm", !busy, () => deps.send({ type: "toggleVocalArm", track: target.id }), { checked: target.kind.armed })
      : plain("menu.arm", target !== null && !busy, () => target && deps.send({ type: "selectTrack", track: target.id, fromArm: true }), {
          checked: target !== null,
        }),
    { kind: "separator" },
    act("menu.nextTake", { kind: "stepTake", delta: 1 }, t("key.take"), { usable: canStepTake }),
    act("menu.prevTake", { kind: "stepTake", delta: -1 }, k.shift(t("key.take")), { usable: canStepTake }),
    plain("menu.deleteTake", canStepTake, () => clip && deps.send({ type: "deleteTake", clip: clip.id })),
    { kind: "separator" },
    plain("menu.addKeysTrack", !busy, () => deps.send(openShelf({ kind: "newTrack" }))),
    plain("menu.addVocalTrack", !busy, () => deps.send({ type: "addVocalTrack" })),
    ...listEntries([edits.rename, edits.color, edits.up, edits.down, { kind: "separator" }, edits.remove]),
  ];

  // Not opened while another dialog (song, sound settings) is open (so the Esc order stays single).
  const noDialog = !state.workspace.songDialog && !state.workspace.settings && !state.plugins.stoppedOpen;
  const help: ListEntry[] = [
    plain("menu.keyList", noDialog && !about.open, openKeyList),
    plain("menu.userGuide", true, send({ type: "openUserGuide" })),
    { kind: "separator" },
    plain("menu.about", noDialog && !keyList.open, openAbout),
  ];

  return [file, edit, view, play, notes, track, help].map((entries, i) => ({ label: t(TOP_MENUS[i]!), entries }));
}
