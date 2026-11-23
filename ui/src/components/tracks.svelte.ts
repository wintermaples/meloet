// State that only the screen keeps for the track heads and the shelf (the name input box; the shelf query, favorites and folded groups; whether keys go to the shelf).
// core keeps the song and the shelf contents. This is only how they are shown.

import type { AppState, Command, LibraryView, ShelfMode, SoundRef } from "../bridge";
import { headingOpen, rowIs, shelfModel, stepRow, type ShelfList, type ShelfModel, type ShelfRow } from "./shelfList";
import { sameSound } from "./sounds";

/** The id of the track whose name is being typed (`TrackView::id`). */
export const trackEdit = $state({ renaming: null as number | null });

export function startRename(track: number): void {
  trackEdit.renaming = track;
}

export const shelfUi = $state({
  query: "",
  favoritesOnly: false,
  /** Headings opened or folded (only the ones changed from the first form; spec 3: a folded heading keeps its name and count). Kept only in the screen. */
  open: {} as Record<string, boolean>,
  /** Headings opened or folded while searching (only while the query is searchFor). When the query changes, they go back to the first form (open). */
  searchOpen: {} as Record<string, boolean>,
  searchFor: "",
  /** Whether ↑ ↓, Enter, F and Space in the shelf go to the shelf. Set when the shelf opens or is clicked; cleared when another band is clicked. */
  keys: false,
  /** What the shelf list shows (the 「音源 N」 / 「エフェクト 6」 toggle in the heading). When the shelf mode changes, it goes back to that mode's list (Shelf.svelte). */
  view: "instruments" as ShelfList,
});

/** Open or fold a heading. */
export function toggleHeading(id: string, library: LibraryView): void {
  if (shelfUi.query.trim() !== "") {
    if (shelfUi.searchFor !== shelfUi.query) {
      shelfUi.searchOpen = {};
      shelfUi.searchFor = shelfUi.query;
    }
    shelfUi.searchOpen = { ...shelfUi.searchOpen, [id]: !headingOpen(id, shelfUi, library) };
    return;
  }
  shelfUi.open = { ...shelfUi.open, [id]: !headingOpen(id, shelfUi, library) };
}

export type { ShelfList };

/** The intent to open the shelf. Shelf.svelte sends keys to the opened shelf (when it opens and when the mode changes). */
export function openShelf(mode: ShelfMode): Command {
  return { type: "openShelf", mode };
}

/** P, 「棚」 in the view menu, the vertical heading: if closed, open it as 「新しいトラック」. If open, the usual band toggle (bring to front or close). */
export function shelfToggle(state: AppState): Command {
  if (!state.workspace.layout.rightOpen.includes("shelf"))
    return openShelf({ kind: "newTrack" });
  return { type: "toggleBand", band: "shelf" };
}

/** Whether keys go to the shelf (the shelf is in front and there is no dialog). */
export function shelfHasKeys(state: AppState): boolean {
  const ws = state.workspace;
  return (
    shelfUi.keys &&
    ws.layout.rightFront === "shelf" &&
    ws.songDialog === null &&
    ws.settings === null &&
    !state.plugins.stoppedOpen
  );
}

/** The current shelf list (effects in the instrument mode and instruments in the effect mode are view only). */
export function currentShelf(state: AppState): ShelfModel {
  return shelfModel(shelfUi.view, state.shelf, state.library, shelfUi);
}

/** Whether a row of the shelf list can be chosen (choosing shows the audition or the 「入れる」 box). */
export function shelfPicks(state: AppState): boolean {
  const inserting = state.shelf.mode.kind === "insertEffect" || state.shelf.mode.kind === "replaceEffect";
  return inserting === (shelfUi.view === "effects");
}

/** The chosen row of the shelf list (audition, loading, cannot load, effect to put in). */
export function pickedRow(state: AppState, rows: readonly ShelfRow[]): ShelfRow | null {
  const shelf = state.shelf;
  if (!shelfPicks(state)) return null;
  if (shelfUi.view === "effects") return rows.find((r) => rowIs(r, null, shelf.chosenEffect)) ?? null;
  const sound = shelf.selectedSound ?? shelf.loadingSound ?? shelf.unreadable;
  return rows.find((r) => rowIs(r, sound)) ?? null;
}

/** The intent to choose a row (an instrument auditions; an effect shows the 「入れる」 box). null in a view-only list. */
export function pickCommand(state: AppState, row: ShelfRow): Command | null {
  if (!shelfPicks(state)) return null;
  if (row.effect) return rowIs(row, null, state.shelf.chosenEffect) ? null : { type: "shelfChooseEffect", effect: row.effect };
  if (!row.sound) return null;
  const shelf = state.shelf;
  if (rowIs(row, shelf.selectedSound ?? shelf.loadingSound)) return null;
  return { type: "shelfSelectSound", sound: row.sound };
}

/** ↑ ↓: choose the next row among the visible rows. Stops at the ends. */
export function shelfStepCommand(state: AppState, delta: 1 | -1): Command | null {
  if (!shelfPicks(state)) return null;
  const { rows } = currentShelf(state);
  const picked = pickedRow(state, rows);
  const next = stepRow(rows, picked ? rows.indexOf(picked) : -1, delta);
  return next ? pickCommand(state, next) : null;
}

/** Enter: put in the chosen instrument or the chosen effect. */
export function shelfConfirmCommand(state: AppState): Command | null {
  const shelf = state.shelf;
  if (!shelfPicks(state)) return null;
  if (shelfUi.view === "effects") return shelf.chosenEffect === null ? null : { type: "shelfConfirm" };
  return shelf.selectedSound === null || shelf.loadingSound !== null ? null : { type: "shelfConfirm" };
}

/** Space: play or stop the sample of the chosen instrument. */
export function sampleCommand(state: AppState, sound: SoundRef): Command {
  const playing = state.shelf.soundSample?.sound;
  return playing && sameSound(playing, sound) ? { type: "stopSample" } : { type: "playSampleOf", sound };
}

export function shelfSampleCommand(state: AppState): Command | null {
  if (shelfUi.view !== "instruments" || !shelfPicks(state)) return null;
  const sound = state.shelf.selectedSound;
  return sound === null ? null : sampleCommand(state, sound);
}

/** F: the favorite mark of the chosen row (in the effect list too; spec 3 (86-4)). */
export function shelfFavoriteCommand(state: AppState): Command | null {
  if (!shelfPicks(state)) return null;
  const shelf = state.shelf;
  if (shelfUi.view === "effects") return shelf.chosenEffect === null ? null : { type: "toggleFavoriteEffect", effect: shelf.chosenEffect };
  return shelf.selectedSound === null ? null : { type: "toggleFavoriteSound", sound: shelf.selectedSound };
}

/** In the shelf, before Esc: stop the audition, the loading, the effect to put in, or the notice that it could not load. */
export function shelfHasPick(state: AppState): boolean {
  const shelf = state.shelf;
  return shelf.selectedSound !== null || shelf.loadingSound !== null || shelf.chosenEffect !== null || shelf.unreadable !== null;
}

export type ShelfStatus =
  | { kind: "search"; query: string; count: number }
  | { kind: "instruments"; count: number; groups: string[] }
  | { kind: "effects"; count: number; groups: string[] };

/**
 * The shelf state shown in the status line. Only when the shelf is in front and nothing is auditioning or playing a sample.
 * While searching: 「さがす「Dragonfly」・ 合う物 4 個」 (mock F5-ShelfSearch).
 * The instrument list: 「音色 31 ・ ピアノ ・ 鍵盤 と ギター をひらいています」; the effect list: 「エフェクト 512 ・ リバーブ をひらいています」
 * (spec 6). count is the number of rows shown; groups are the open headings.
 */
export function shelfStatus(state: AppState): ShelfStatus | null {
  const shelf = state.shelf;
  if (state.workspace.layout.rightFront !== "shelf" || shelf.selectedSound !== null || shelf.loadingSound !== null || shelf.soundSample !== null)
    return null;
  const model = currentShelf(state);
  if (shelfUi.query.trim() !== "") return { kind: "search", query: shelfUi.query.trim(), count: model.shown };
  if (shelfUi.view === "effects") return { kind: "effects", count: model.shown, groups: model.open.map((o) => o.label) };
  return { kind: "instruments", count: model.shown, groups: model.open.map((o) => o.label) };
}

/**
 * Where the track lands when a heading drag is released (`MoveTrack::to`). from: the order number of the moving track (from 0),
 * center: the distance from the top edge of the top heading to the center of the moving heading; h: heading height.
 * slot is the line "before the heading of that number" (0 to count).
 */
export function moveSlot(
  from: number,
  count: number,
  center: number,
  h: number,
): { slot: number; to: number } {
  const slot = Math.max(0, Math.min(count, Math.round(center / h)));
  return { slot, to: slot > from ? slot - 1 : slot };
}

/** moveSlot when the row heights differ (an automation band under a track). bounds are the y of the row borders (from the top, including the bottom). */
export function moveSlotAt(from: number, bounds: readonly number[], center: number): { slot: number; to: number } {
  let slot = 0;
  bounds.forEach((b, i) => {
    if (Math.abs(center - b) <= Math.abs(center - bounds[slot]!)) slot = i;
  });
  return { slot, to: slot > from ? slot - 1 : slot };
}
