// トラックを変える行（仕様書 3 章「第 6 版で足した決まり」Y5）。右クリックの一覧（トラックの見出し）と、上のメニューの「トラック」が使う。
// 録音中とカウントイン中は、core が受けないので「使えない」。

import type { AppState, Command, TrackView } from "../bridge";
import { songBusy } from "../keys/attach";
import { t } from "../text/ja";
import type { MenuEntry, MenuRow } from "./menu/contextMenu.svelte";
import { startRename } from "./tracks.svelte";

export const TRACK_COLORS = [1, 2, 3, 4, 5] as const;

export interface TrackRows {
  rename: MenuRow;
  color: MenuRow;
  up: MenuRow;
  down: MenuRow;
  remove: MenuRow;
}

/** 名前を変える ・ 色 ▸ ・ 上へ動かす ・ 下へ動かす ・ トラックを消す。track が null（トラックがない）なら、どれも「使えない」。 */
export function trackRows(state: AppState, track: TrackView | null, send: (c: Command) => void): TrackRows {
  const can = track !== null && !songBusy(state);
  const at = track ? state.song.tracks.findIndex((x) => x.id === track.id) : -1;
  const last = state.song.tracks.length - 1;
  const row = (label: MenuRow["label"], usable: boolean, run: () => void, more: Partial<MenuRow> = {}): MenuRow => ({
    kind: "row",
    label,
    usable,
    run,
    ...more,
  });
  const id = track?.id ?? -1;
  return {
    rename: row("menu.renameTrack", can, () => startRename(id), { key: t("key.renameTrack") }),
    color: row("menu.trackColor", can, () => {}, {
      sub: TRACK_COLORS.map((color) =>
        row("menu.colorNumber", can, () => send({ type: "setTrackColor", track: id, color }), {
          text: t("menu.colorNumber", { number: color }),
          checked: track?.color === color,
          swatch: color,
        }),
      ),
    }),
    up: row("menu.moveTrackUp", can && at > 0, () => send({ type: "moveTrack", track: id, to: at - 1 })),
    down: row("menu.moveTrackDown", can && at >= 0 && at < last, () => send({ type: "moveTrack", track: id, to: at + 1 })),
    remove: row("menu.deleteTrack", can, () => send({ type: "deleteTrack", track: id })),
  };
}

/**
 * トラックの見出しの右クリック（モック R6-Tracks）: ミュート ・ ソロ ・ 録音待機 ／ 名前を変える ・ 色 ▸ ・ 上へ ・ 下へ ／ トラックを消す。
 * selected: 録音待機の印。右クリックはそのトラックを選んでからひらくので、選んだあとの形で渡す。
 */
export function trackContextMenu(
  state: AppState,
  track: TrackView,
  selected: boolean,
  send: (c: Command) => void,
  shiftKey: (letter: string) => string,
): MenuEntry[] {
  const rows = trackRows(state, track, send);
  // ミュートは録音中 ・ カウントイン中も受ける（A-14）。
  return [
    { kind: "row", label: "menu.mute", usable: true, run: () => send({ type: "toggleMute", track: track.id }), key: shiftKey(t("key.mute")), checked: track.muted },
    { kind: "row", label: "menu.solo", usable: true, run: () => send({ type: "toggleSolo", track: track.id }), key: shiftKey(t("key.solo")), checked: track.soloed },
    { kind: "row", label: "menu.arm", usable: true, run: () => send({ type: "selectTrack", track: track.id, fromArm: true }), checked: selected },
    { kind: "separator" },
    rows.rename,
    rows.color,
    rows.up,
    rows.down,
    { kind: "separator" },
    rows.remove,
  ];
}
