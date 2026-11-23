// トラックを変える行（仕様書 3 章「第 6 版で足した決まり」Y5）。右クリックの一覧（トラックの見出し）と、上のメニューの「トラック」が使う。
// 録音中とカウントイン中は、core が受けないので「使えない」。

import { VOCAL_INPUTS, type AppState, type Command, type TrackView, type VocalInput } from "../bridge";
import { pianoRollOpen, songBusy } from "../keys/attach";
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
 * トラックの見出しの右クリック（モック R6-Tracks）: ミュート ・ ソロ ・ 録音待機 ／（歌のトラックは「音のファイルを入れる…」／）名前を変える ・ 色 ▸ ・ 上へ ・ 下へ ／ トラックを消す。
 * selected: 録音待機の印（鍵盤のトラックは弾くと鳴る先、歌のトラックは選んだ）。右クリックはそのトラックを選んでからひらくので、選んだあとの形で渡す。
 */
export function trackContextMenu(
  state: AppState,
  track: TrackView,
  selected: boolean,
  send: (c: Command) => void,
  shiftKey: (letter: string) => string,
  playTick = state.transport.position,
): MenuEntry[] {
  const rows = trackRows(state, track, send);
  // Keyboard tracks (round 10): show the clip at the play line; with no clip there, only open the band (the track is
  // already selected when the menu opens). The hint 「ここにはクリップがありません。」 then needs core to stop showing
  // the clip of another track (arrange.pianoRollClip).
  const open = (): void => {
    const clip = track.clips.find((c) => c.start <= playTick && playTick < c.end);
    if (clip) send({ type: "selectClip", clip: clip.id });
    if (!pianoRollOpen(state)) send({ type: "toggleBand", band: "pianoRoll" });
  };
  // ミュートは録音中 ・ カウントイン中も受ける（A-14）。
  return [
    ...(track.kind.kind === "keys"
      ? ([
          { kind: "row", label: "menu.openInPianoRoll", usable: true, run: open, key: t("key.pianoRoll") },
          { kind: "separator" },
        ] satisfies MenuEntry[])
      : []),
    { kind: "row", label: "menu.mute", usable: true, run: () => send({ type: "toggleMute", track: track.id }), key: shiftKey(t("key.mute")), checked: track.muted },
    { kind: "row", label: "menu.solo", usable: true, run: () => send({ type: "toggleSolo", track: track.id }), key: shiftKey(t("key.solo")), checked: track.soloed },
    track.kind.kind === "vocal"
      ? // 歌のトラックの ● は、選んでいなくても自分でつける ・ 外す（仕様書 3 章 24-C）。選ぶと、つく。
        { kind: "row", label: "menu.arm", usable: true, run: () => send({ type: "toggleVocalArm", track: track.id }), checked: track.kind.armed || selected }
      : { kind: "row", label: "menu.arm", usable: true, run: () => send({ type: "selectTrack", track: track.id, fromArm: true }), checked: selected },
    { kind: "separator" },
    // 歌のトラックだけ: そのトラックの再生の線の所へ音のファイルを入れる（仕様書 3 章 29-A）。
    ...(track.kind.kind === "vocal"
      ? ([
          { kind: "row", label: "menu.importAudio", usable: !songBusy(state), run: () => send({ type: "importAudio", track: track.id }) },
          { kind: "separator" },
        ] satisfies MenuEntry[])
      : []),
    rows.rename,
    rows.color,
    rows.up,
    rows.down,
    { kind: "separator" },
    rows.remove,
  ];
}

/**
 * 歌のトラックの入口の一覧（仕様書 3 章「歌のトラックの見出し」、モック F7-VocalTrack）: 入口 1 ・ モノ ／ 入口 2 ・ モノ ／ 入口 1 ・ 2 ・ ステレオ ／
 * 音の設定…。見出しの「入口 1 ・ モノ ▾」とミキサーの音源の枠が使う。入口を変えるのは取り消せる 1 手なので、録音中は「使えない」。
 */
export function vocalInputRows(track: number, input: VocalInput, busy: boolean, send: (c: Command) => void): MenuEntry[] {
  return [
    ...VOCAL_INPUTS.map(
      (choice): MenuEntry => ({
        kind: "row",
        label: `vocal.input.${choice}`,
        usable: !busy,
        checked: choice === input,
        run: () => {
          if (choice !== input) send({ type: "setVocalInput", track, input: choice });
        },
      }),
    ),
    { kind: "separator" },
    { kind: "row", label: "button.audioSettingsEllipsis", usable: true, run: () => send({ type: "openSettings", advanced: true }) },
  ];
}
