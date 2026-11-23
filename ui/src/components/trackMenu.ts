// The rows that change tracks (spec 3 "Rules added in version 6", Y5). Used by the context menu of the track head and the 「トラック」 menu.
// While recording and during count-in, core does not take them, so they are disabled.

import { VOCAL_INPUTS, type AppState, type Command, type TrackView, type VocalInput } from "../bridge";
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

/** Rename, color ▸, move up, move down, delete track. When track is null (no track), all are disabled. */
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
 * The context menu of a track head (mock R6-Tracks): mute, solo, arm / (vocal tracks: 「音のファイルを入れる…」 /) rename, color ▸, up, down / delete track.
 * selected: the arm mark (for a keyboard track, where playing sounds; for a vocal track, selected). The menu opens after selecting the track, so pass the state after selecting.
 */
export function trackContextMenu(
  state: AppState,
  track: TrackView,
  selected: boolean,
  send: (c: Command) => void,
  shiftKey: (letter: string) => string,
): MenuEntry[] {
  const rows = trackRows(state, track, send);
  const open = (): void => send({ type: "openPianoRoll", track: track.id });
  // Mute is taken while recording and during count-in too (A-14).
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
      ? // The ● of a vocal track is set and cleared on its own, even when not selected (spec 3 (24-C)). Selecting sets it.
        { kind: "row", label: "menu.arm", usable: true, run: () => send({ type: "toggleVocalArm", track: track.id }), checked: track.kind.armed || selected }
      : { kind: "row", label: "menu.arm", usable: true, run: () => send({ type: "selectTrack", track: track.id, fromArm: true }), checked: selected },
    { kind: "separator" },
    // Vocal tracks only: put a sound file at the playhead on that track (spec 3 (29-A)).
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
 * The input list of a vocal track (spec 2 "Vocal track head", mock F7-VocalTrack): 「入口 1 ・ モノ」 / 「入口 2 ・ モノ」 / 「入口 1 ・ 2 ・ ステレオ」 /
 * 「音の設定…」. Used by 「入口 1 ・ モノ ▾」 in the head and the instrument slot of the mixer. Changing the input is one undo step, so it is disabled while recording.
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
