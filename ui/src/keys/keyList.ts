// The table of the key list dialog (round 12 G1): docs/spec/4-keys.md as (group, wording, keys).
// Every KeyAction kind has an entry, so a key added to keys.ts without a row here fails the type check.
// Does not know the DOM.

import type { keyNames } from "../components/menu/menus";
import { t, type TextKey } from "../text/ja";
import type { KeyAction } from "./keys";

export type KeyNames = ReturnType<typeof keyNames>;

export const KEY_GROUPS = ["file", "play", "bands", "arrange", "pianoRoll", "mixer", "view"] as const;
export type KeyGroup = (typeof KEY_GROUPS)[number];

export interface KeyRow {
  group: KeyGroup;
  /** Several keys are shown joined with 「 ／ 」. */
  label: TextKey | readonly TextKey[];
  keys: (k: KeyNames) => string;
}

const or = (...parts: string[]) => parts.join(t("keyList.or"));
const both = (...keys: TextKey[]) => keys.map((key) => t(key)).join(" ");
const row = (group: KeyGroup, label: KeyRow["label"], keys: KeyRow["keys"]): KeyRow => ({ group, label, keys });
/** Cut, copy, paste and the like go to whichever of the arrange and the piano roll was pressed last: listed in both. */
const inBoth = (label: TextKey, keys: KeyRow["keys"]): KeyRow[] => [row("arrange", label, keys), row("pianoRoll", label, keys)];

/** The keys of keys.ts. An empty list is a key that is not shown (the output chooser's keys live in its own hint). */
export const ACTION_ROWS: Record<KeyAction["kind"], readonly KeyRow[]> = {
  newSong: [row("file", "menu.newSong", (k) => k.mod(t("key.newSong")))],
  openSong: [row("file", "menu.openSong", (k) => k.mod(t("key.openSong")))],
  saveSong: [row("file", "menu.save", (k) => k.mod(t("key.saveSong")))],
  saveSongAs: [row("file", "menu.saveAs", (k) => k.modShift(t("key.saveSong")))],
  export: [row("file", "menu.export", (k) => k.modShift(t("key.export")))],
  audioSettings: [row("file", "menu.audioSettings", (k) => k.mod(t("key.audioSettings")))],
  undo: [row("file", "menu.undo", (k) => k.mod(t("key.undo")))],
  redo: [row("file", "menu.redo", (k) => k.modShift(t("key.undo")))],
  quit: [row("file", "menu.quit", (k) => k.mod(t("key.quit")))],

  playStop: [row("play", "keyList.playStop", () => t("key.playStop"))],
  record: [row("play", "menu.record", () => t("key.record"))],
  keepPlayed: [row("play", "keyList.keepLast", (k) => k.shift(t("key.keepLast")))],
  toStart: [row("play", "menu.toStart", () => t("key.toStart"))],
  loop: [row("play", "menu.loop", () => t("key.loop"))],
  metronome: [row("play", "menu.metronome", () => t("key.metronome"))],

  arrange: [row("bands", "menu.goArrange", () => t("key.arrange"))],
  toggleBand: [
    row("bands", "menu.pianoRoll", () => t("key.pianoRoll")),
    row("bands", "menu.mixer", () => t("key.mixer")),
    row("bands", "menu.history", () => t("key.history")),
    row("bands", "menu.shelf", () => t("key.shelf")),
    row("bands", "menu.params", () => t("key.params")),
  ],
  back: [row("bands", "keyList.back", () => t("key.esc"))],
  shelfStep: [row("bands", "keyList.shelfStep", () => both("key.up", "key.down"))],
  shelfConfirm: [row("bands", "keyList.shelfConfirm", () => t("key.enter"))],
  shelfSample: [row("bands", "keyList.shelfSample", () => t("key.playStop"))],
  shelfFavorite: [row("bands", "keyList.shelfFavorite", () => t("key.favorite"))],
  historyStep: [row("bands", "keyList.historyStep", () => both("key.up", "key.down"))],
  historyHome: [row("bands", "keyList.historyHome", () => t("key.toStart"))],
  historyConfirm: [row("bands", "keyList.historyConfirm", () => t("key.enter"))],
  chooserUp: [],
  chooserDown: [],
  chooserConfirm: [],

  splitClips: [row("arrange", "menu.splitClip", () => t("key.split"))],
  joinClips: [row("arrange", "menu.joinClips", (k) => k.mod(t("key.join")))],
  stepTake: [row("arrange", ["menu.nextTake", "menu.prevTake"], (k) => or(t("key.take"), k.shift(t("key.take"))))],
  trackMute: [row("arrange", "menu.mute", (k) => k.shift(t("key.mute")))],
  trackSolo: [row("arrange", "menu.solo", (k) => k.shift(t("key.solo")))],
  renameTrack: [row("arrange", "keyList.renameTrack", () => t("key.renameTrack"))],

  tool: [row("pianoRoll", "keyList.tools", () => or(t("key.toolSelect"), t("key.toolDraw"), t("key.toolErase")))],
  nudge: [
    row("pianoRoll", "keyList.nudgeKeys", (k) => or(both("key.up", "key.down"), k.shift(both("key.up", "key.down")))),
    row("pianoRoll", "keyList.nudgeSteps", () => both("key.left", "key.right")),
  ],
  quantize: [row("pianoRoll", "keyList.quantize", () => t("key.quantize"))],

  cut: inBoth("menu.cut", (k) => k.mod(t("key.cut"))),
  copy: inBoth("menu.copy", (k) => k.mod(t("key.copy"))),
  paste: inBoth("menu.paste", (k) => k.mod(t("key.paste"))),
  duplicate: inBoth("menu.duplicate", (k) => k.mod(t("key.duplicate"))),
  selectAll: inBoth("menu.selectAll", (k) => k.mod(t("key.selectAll"))),
  deleteSelected: inBoth("menu.delete", () => t("key.delete")),

  bypass: [row("mixer", "keyList.bypass", () => t("key.bypass"))],

  zoom: [row("view", ["menu.zoomIn", "menu.zoomOut"], (k) => or(k.mod(t("key.zoomIn")), k.mod(t("key.zoomOut"))))],
  scale: [row("view", "menu.scale", (k) => or(k.modAlt("1"), "2", "3"))],
  menuBar: [row("view", "keyList.menuBar", () => t("key.f10"))],
};

/** Keys spec 4 lists that are taken outside keys.ts (the timeline, the mixer, the parameters panel, drags, boxes). */
export const OTHER_ROWS: readonly KeyRow[] = [
  row("arrange", "keyList.addMarker", () => t("key.marker")),
  row("arrange", "keyList.stepMarker", (k) => or(k.alt(t("key.right")), k.alt(t("key.left")))),
  row("arrange", "keyList.renameMarker", () => t("key.renameTrack")),
  row("arrange", "keyList.deletePoints", () => t("key.delete")),
  row("arrange", "keyList.tempoBox", (k) => or(both("key.up", "key.down"), k.shift(both("key.up", "key.down")))),
  row("arrange", "keyList.dragCopy", (k) => k.alt(both("key.plus", "key.drag"))),
  row("arrange", "keyList.dragFree", (k) => k.shift(both("key.plus", "key.drag"))),
  row("arrange", "keyList.dragCancel", () => t("key.esc")),
  row("pianoRoll", "keyList.noteList", () => or(t("key.enter"), t("key.tab"))),
  row("mixer", "keyList.removeInsert", () => t("key.delete")),
  row("mixer", "keyList.fader", (k) => or(both("key.up", "key.down"), k.shift(both("key.up", "key.down")))),
  row("mixer", "keyList.pan", (k) => or(both("key.left", "key.right"), k.shift(both("key.left", "key.right")))),
  row("mixer", "keyList.mixHome", () => t("key.toStart")),
  row("mixer", "keyList.mixType", () => t("key.enter")),
  row("mixer", "keyList.paramStep", () => both("key.up", "key.down")),
  row("mixer", "keyList.paramMove", (k) => or(both("key.left", "key.right"), k.shift(both("key.left", "key.right")))),
  row("mixer", "keyList.paramType", () => t("key.enter")),
  row("mixer", "keyList.paramFind", (k) => k.mod(t("key.find"))),
  row("view", ["menu.zoomRowsIn", "menu.zoomRowsOut"], (k) => k.modShift(both("key.plus", "key.wheel"))),
  row("view", "keyList.focus", (k) => or(t("key.tab"), k.shift(t("key.tab")))),
];

/**
 * Coming in round 12 wave 2 (R2, A1). When their KeyAction kinds are added to keys.ts, move each row into
 * ACTION_ROWS under its kind (the type check asks for it) and take it out of here.
 */
export const WAVE2_ROWS: readonly KeyRow[] = [
  row("play", "keyList.playByKeys", (k) => k.mod(t("key.playByKeys"))),
  row("arrange", "keyList.repeatClips", (k) => k.mod(t("key.repeatClips"))),
];

export interface ShownRow {
  label: string;
  keys: string;
}
export interface ShownGroup {
  group: KeyGroup;
  label: string;
  rows: ShownRow[];
}

export function labelOf(r: KeyRow): string {
  return typeof r.label === "string" ? t(r.label) : or(...r.label.map((key) => t(key)));
}

/**
 * The groups in KEY_GROUPS order, rows in table order, with the words written out. query filters by the label, the keys
 * and the group name (case does not matter); a group whose name matches shows all its rows. Groups left empty are dropped.
 */
export function keyListGroups(names: KeyNames, query = ""): ShownGroup[] {
  const all = [...Object.values(ACTION_ROWS).flat(), ...OTHER_ROWS, ...WAVE2_ROWS];
  const q = query.trim().toLowerCase();
  const has = (text: string) => text.toLowerCase().includes(q);
  return KEY_GROUPS.map((group) => {
    const label = t(`keyList.group.${group}`);
    const rows = all.filter((r) => r.group === group).map((r) => ({ label: labelOf(r), keys: r.keys(names) }));
    return { group, label, rows: q === "" || has(label) ? rows : rows.filter((r) => has(r.label) || has(r.keys)) };
  }).filter((g) => g.rows.length > 0);
}
