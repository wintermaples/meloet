// トラックの見出しと棚の、画面だけが持つ状態（名前の入力の枠、棚のさがす文字 ・ お気に入り ・ たたんだグループ、棚にキーが向いているか）。
// 曲や棚の中身は core が持つ。ここは見せかただけ。

import type { AppState, Command, LibraryView, ShelfMode, SoundRef } from "../bridge";
import { headingOpen, rowIs, shelfModel, stepRow, type ShelfList, type ShelfModel, type ShelfRow } from "./shelfList";
import { sameSound } from "./sounds";

/** いま名前を打っているトラックの id（`TrackView::id`）。 */
export const trackEdit = $state({ renaming: null as number | null });

export function startRename(track: number): void {
  trackEdit.renaming = track;
}

export const shelfUi = $state({
  query: "",
  favoritesOnly: false,
  /** 見出しをひらいた ・ たたんだ（はじめの形からかえた物だけ。仕様書 3 章: たたんでも見出しと数はのこる）。画面の中だけで覚える。 */
  open: {} as Record<string, boolean>,
  /** さがしている間にひらいた ・ たたんだ見出し（searchFor の文字のときだけ）。さがす文字が変わると、はじめの形（ひらく）に戻る。 */
  searchOpen: {} as Record<string, boolean>,
  searchFor: "",
  /** 棚の中の ↑ ↓ ・ Enter ・ F ・ Space を棚に向けるか。棚をひらいたとき、棚を押したときに向き、ほかの帯を押すと外れる。 */
  keys: false,
  /** 棚の一覧に出している物（見出しの「音源 N」「エフェクト 6」の切りかえ）。棚のモードが変わると、そのモードの物に戻す（Shelf.svelte）。 */
  view: "instruments" as ShelfList,
});

/** 見出しをひらく ・ たたむ。 */
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

/** 棚をひらく意図。ひらいた棚には、Shelf.svelte がキーを向ける（ひらいたとき、モードが変わったとき）。 */
export function openShelf(mode: ShelfMode): Command {
  return { type: "openShelf", mode };
}

/** P ・ 表示の「棚」・ 縦の見出し: とじていれば「新しいトラック」でひらく。ひらいていれば、いままでの帯の切りかえ（前に出す ・ とじる）。 */
export function shelfToggle(state: AppState): Command {
  if (!state.workspace.layout.rightOpen.includes("shelf"))
    return openShelf({ kind: "newTrack" });
  return { type: "toggleBand", band: "shelf" };
}

/** 棚にキーが向いているか（棚が前に出ていて、ダイアログがないとき）。 */
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

/** いまの棚の一覧（音源のモードのエフェクト、エフェクトのモードの音源は見るだけ）。 */
export function currentShelf(state: AppState): ShelfModel {
  return shelfModel(shelfUi.view, state.shelf, state.library, shelfUi);
}

/** 棚の一覧で、えらべる（えらぶと、ためし弾きか「入れる」の箱が出る）か。 */
export function shelfPicks(state: AppState): boolean {
  const inserting = state.shelf.mode.kind === "insertEffect" || state.shelf.mode.kind === "replaceEffect";
  return inserting === (shelfUi.view === "effects");
}

/** 棚の一覧でえらばれている行（ためし弾き ・ 読みこみ中 ・ 読みこめない ・ 入れるエフェクト）。 */
export function pickedRow(state: AppState, rows: readonly ShelfRow[]): ShelfRow | null {
  const shelf = state.shelf;
  if (!shelfPicks(state)) return null;
  if (shelfUi.view === "effects") return rows.find((r) => rowIs(r, null, shelf.chosenEffect)) ?? null;
  const sound = shelf.selectedSound ?? shelf.loadingSound ?? shelf.unreadable;
  return rows.find((r) => rowIs(r, sound)) ?? null;
}

/** 行をえらぶ意図（音源はためし弾き、エフェクトは「入れる」の箱）。見るだけの一覧では null。 */
export function pickCommand(state: AppState, row: ShelfRow): Command | null {
  if (!shelfPicks(state)) return null;
  if (row.effect) return rowIs(row, null, state.shelf.chosenEffect) ? null : { type: "shelfChooseEffect", effect: row.effect };
  if (!row.sound) return null;
  const shelf = state.shelf;
  if (rowIs(row, shelf.selectedSound ?? shelf.loadingSound)) return null;
  return { type: "shelfSelectSound", sound: row.sound };
}

/** ↑ ↓: 見えている行の中で次の行をえらぶ。はしでは止まる。 */
export function shelfStepCommand(state: AppState, delta: 1 | -1): Command | null {
  if (!shelfPicks(state)) return null;
  const { rows } = currentShelf(state);
  const picked = pickedRow(state, rows);
  const next = stepRow(rows, picked ? rows.indexOf(picked) : -1, delta);
  return next ? pickCommand(state, next) : null;
}

/** Enter: えらんだ音源を入れる ・ えらんだエフェクトを入れる。 */
export function shelfConfirmCommand(state: AppState): Command | null {
  const shelf = state.shelf;
  if (!shelfPicks(state)) return null;
  if (shelfUi.view === "effects") return shelf.chosenEffect === null ? null : { type: "shelfConfirm" };
  return shelf.selectedSound === null || shelf.loadingSound !== null ? null : { type: "shelfConfirm" };
}

/** Space: えらんだ音源のサンプルを鳴らす ・ 止める。 */
export function sampleCommand(state: AppState, sound: SoundRef): Command {
  const playing = state.shelf.soundSample?.sound;
  return playing && sameSound(playing, sound) ? { type: "stopSample" } : { type: "playSampleOf", sound };
}

export function shelfSampleCommand(state: AppState): Command | null {
  if (shelfUi.view !== "instruments" || !shelfPicks(state)) return null;
  const sound = state.shelf.selectedSound;
  return sound === null ? null : sampleCommand(state, sound);
}

/** F: えらんだ行のお気に入りの印（エフェクトの一覧でも。仕様書 3 章 86-4）。 */
export function shelfFavoriteCommand(state: AppState): Command | null {
  if (!shelfPicks(state)) return null;
  const shelf = state.shelf;
  if (shelfUi.view === "effects") return shelf.chosenEffect === null ? null : { type: "toggleFavoriteEffect", effect: shelf.chosenEffect };
  return shelf.selectedSound === null ? null : { type: "toggleFavoriteSound", sound: shelf.selectedSound };
}

/** 棚の中の Esc の前の番: ためし弾き ・ 読みこみ ・ 入れるエフェクト ・ 読みこめなかった知らせをやめる。 */
export function shelfHasPick(state: AppState): boolean {
  const shelf = state.shelf;
  return shelf.selectedSound !== null || shelf.loadingSound !== null || shelf.chosenEffect !== null || shelf.unreadable !== null;
}

export type ShelfStatus =
  | { kind: "search"; query: string; count: number }
  | { kind: "instruments"; count: number; groups: string[] }
  | { kind: "effects"; count: number; groups: string[] };

/**
 * 状態行に出す棚のようす。棚が前に出ていて、ためし弾きもサンプルもしていないときだけ。
 * さがしているとき「さがす「Dragonfly」・ 合う物 4 個」（モック F5-ShelfSearch）。
 * 音源の一覧は「音色 31 ・ ピアノ ・ 鍵盤 と ギター をひらいています」、エフェクトの一覧は「エフェクト 512 ・ リバーブ をひらいています」
 * （仕様書 6 章）。count は出ている行の数、groups はひらいている見出し。
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
 * 見出しのドラッグで、離したときの並びの場所（`MoveTrack::to`）。from: 動かすトラックの並びの番号（0 から）、
 * center: 動かしている見出しのまん中の、いちばん上の見出しの上はしからの距離、h: 見出しの高さ。
 * slot は「その番号の見出しの前」の線（0〜count）。
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

/** moveSlot の、行の高さがそろっていない形（トラックの下にオートメーションの帯がある）。bounds は行の境の y（上から、いちばん下もふくむ）。 */
export function moveSlotAt(from: number, bounds: readonly number[], center: number): { slot: number; to: number } {
  let slot = 0;
  bounds.forEach((b, i) => {
    if (Math.abs(center - b) <= Math.abs(center - bounds[slot]!)) slot = i;
  });
  return { slot, to: slot > from ? slot - 1 : slot };
}
