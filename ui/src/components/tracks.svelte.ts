// トラックの見出しと棚の、画面だけが持つ状態（名前の入力の枠、棚のさがす文字 ・ お気に入り ・ たたんだグループ、棚にキーが向いているか）。
// 曲や棚の中身は core が持つ。ここは見せかただけ。

import {
  INSTRUMENT_GROUPS,
  type AppState,
  type Command,
  type EffectKind,
  type InstrumentGroup,
  type InstrumentKind,
  type ShelfMode,
  type ShelfView,
} from "../bridge";
import { EFFECT_KINDS } from "../bridge/types";
import { t } from "../text/ja";

/** いま名前を打っているトラックの id（`TrackView::id`）。 */
export const trackEdit = $state({ renaming: null as number | null });

export function startRename(track: number): void {
  trackEdit.renaming = track;
}

export const shelfUi = $state({
  query: "",
  favoritesOnly: false,
  /** たたんだグループ（仕様書 3 章: たたんでも見出しと数はのこる）。画面の中だけで覚える。 */
  folded: [] as InstrumentGroup[],
  /** 棚の中の ↑ ↓ ・ Enter ・ F ・ Space を棚に向けるか。棚をひらいたとき、棚を押したときに向き、ほかの帯を押すと外れる。 */
  keys: false,
  /** 棚の一覧に出している物（見出しの「音源 N」「エフェクト 6」の切りかえ）。棚のモードが変わると、そのモードの物に戻す（Shelf.svelte）。 */
  view: "instruments" as ShelfList,
});

export type ShelfList = "instruments" | "effects";

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

export interface ShelfGroup {
  group: InstrumentGroup;
  /** さがす文字とお気に入りで残った行（たたんでいても数える）。 */
  rows: InstrumentKind[];
  folded: boolean;
}

/** 棚に出すグループと行。出す行が 1 つもないグループは、見出しごと出さない。 */
export function shelfGroups(
  shelf: ShelfView,
  ui: {
    query: string;
    favoritesOnly: boolean;
    folded: readonly InstrumentGroup[];
  },
): ShelfGroup[] {
  const query = ui.query.trim().toLowerCase();
  const hit = (text: string) => text.toLowerCase().includes(query);
  return INSTRUMENT_GROUPS.flatMap(({ group, instruments }) => {
    const groupHit = query === "" || hit(t(`instrumentGroup.${group}`));
    const rows = instruments.filter(
      (i) =>
        shelf.available.includes(i) &&
        (!ui.favoritesOnly || shelf.favorites.includes(i)) &&
        (groupHit || hit(t(`instrument.${i}`))),
    );
    return rows.length === 0
      ? []
      : [{ group, rows, folded: ui.folded.includes(group) }];
  });
}

/** ↑ ↓ でえらぶ次の行。見えている行（たたんだグループの行は見えない）の中で動き、はしでは止まる。 */
export function shelfStep(
  groups: readonly ShelfGroup[],
  selected: InstrumentKind | null,
  delta: 1 | -1,
): InstrumentKind | null {
  const rows = groups.flatMap((g) => (g.folded ? [] : g.rows));
  if (rows.length === 0) return null;
  const at = selected === null ? -1 : rows.indexOf(selected);
  if (at < 0) return delta > 0 ? rows[0]! : rows.at(-1)!;
  return rows[Math.max(0, Math.min(rows.length - 1, at + delta))]!;
}

/**
 * 状態行に出す棚のようす（モック R8-Shelf84「音色 31 ・ ピアノ ・ 鍵盤とギターをひらいています」）。
 * 棚が前に出ていて音源の一覧を見せ、ためし弾きもサンプルもしていないときだけ。count は棚に出ている行の数、groups はひらいている見出し。
 */
export function shelfStatus(state: AppState): { count: number; groups: InstrumentGroup[] } | null {
  const shelf = state.shelf;
  if (
    state.workspace.layout.rightFront !== "shelf" ||
    shelf.mode.kind === "insertEffect" ||
    shelfUi.view !== "instruments" ||
    shelf.selected !== null ||
    shelf.sample !== null
  )
    return null;
  const groups = shelfGroups(shelf, shelfUi);
  return {
    count: groups.reduce((n, g) => n + g.rows.length, 0),
    groups: groups.filter((g) => !g.folded).map((g) => g.group),
  };
}

/** 棚の数（「音源 N」「すべて N」「お気に入り N」）。お気に入りは、棚に出る物だけ。 */
export function shelfCounts(shelf: ShelfView): {
  all: number;
  favorites: number;
} {
  return {
    all: shelf.available.length,
    favorites: shelf.favorites.filter((i) => shelf.available.includes(i))
      .length,
  };
}

/** 最初から入っているエフェクトの数（仕様書 3 章 Y8: EQ 4 バンド、コンプレッサー、リバーブ、ディレイ、コーラス、リミッター）。 */
export const BUILTIN_EFFECTS = EFFECT_KINDS.length;

/** 棚の「エフェクト」の行（EFFECT_KINDS の順）。さがす文字は名前にふくまれる物（英字の大小は見ない）。 */
export function shelfEffects(query: string): EffectKind[] {
  const q = query.trim().toLowerCase();
  return EFFECT_KINDS.filter(
    (e) => q === "" || t(`effect.${e}`).toLowerCase().includes(q),
  );
}

/**
 * 棚の ↑ ↓ と Enter のうち、エフェクトにかかわる物。undefined は音源の棚の決まりのまま（音源のモードで音源を見ているとき）。
 * 音源のモードでエフェクトの一覧を見ているときは、見るだけで入れない（design-notes A-14）。
 * 「空き ＋」からひらいた棚（insertEffect）で音源の一覧を見ているときも、何もしない。
 */
export function effectShelfCommand(
  action: "up" | "down" | "confirm",
  state: AppState,
): Command | null | undefined {
  const shelf = state.shelf;
  const inserting = shelf.mode.kind === "insertEffect";
  if (!inserting && shelfUi.view === "instruments") return undefined;
  if (!inserting || shelfUi.view !== "effects") return null;
  if (action === "confirm")
    return shelf.selectedEffect === null ? null : { type: "shelfConfirm" };
  const rows = shelfEffects(shelfUi.query);
  if (rows.length === 0) return null;
  const at =
    shelf.selectedEffect === null ? -1 : rows.indexOf(shelf.selectedEffect);
  const next =
    at < 0
      ? action === "down"
        ? rows[0]!
        : rows.at(-1)!
      : rows[
          Math.max(
            0,
            Math.min(rows.length - 1, at + (action === "down" ? 1 : -1)),
          )
        ]!;
  return next === shelf.selectedEffect
    ? null
    : { type: "shelfSelectEffect", effect: next };
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
