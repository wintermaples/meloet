// Mute sets on the screen (B-97-3, B-97-5): the list of 「ミュートのセット ▾」 at the head of the mixer band (and of
// 「トラック ▸ ミュートのセット ▸」), and the name box that saving and renaming open. Core keeps the sets; the screen keeps
// only whether the box is open. The screen does not send what core would refuse (no name, a name taken, 64 sets).

import { MUTE_SET_NAME_MAX, MUTE_SETS_MAX, type AppState, type Command } from "../../bridge";
import { songBusy } from "../../keys/attach";
import { t } from "../../text";
import type { MenuEntry } from "../menu/contextMenu.svelte";

/** `draft` as a set keeps it (the spaces at its ends taken off), or null when it is no name (Rust: `mute_sets::name_of`). */
export function muteSetName(draft: string): string | null {
  const name = draft.trim();
  const count = [...name].length;
  return count === 0 || count > MUTE_SET_NAME_MAX || /\p{Cc}/u.test(name) ? null : name;
}

export type MuteSetBoxKind = { kind: "save" } | { kind: "rename"; from: string };

/** Where the box opens: the top left of its frame (CSS px from the top left of the window). null: under the 「トラック」 menu. */
export type MuteSetBoxAt = { x: number; y: number } | null;

export const muteSetBox = $state({ open: null as (MuteSetBoxKind & { at: MuteSetBoxAt }) | null });

export function openMuteSetBox(kind: MuteSetBoxKind, at: MuteSetBoxAt): void {
  muteSetBox.open = { ...kind, at };
}

/** true if it closed (it was open). */
export function closeMuteSetBox(): boolean {
  if (!muteSetBox.open) return false;
  muteSetBox.open = null;
  return true;
}

/** What Enter does with `draft` in the box: the intent, or null with the reason (null: nothing to say). */
export function muteSetBoxResult(state: AppState, box: MuteSetBoxKind, draft: string): { command: Command | null; reading: string | null } {
  const sets = state.song.muteSets;
  const name = muteSetName(draft);
  if (name === null) return { command: null, reading: null };
  const taken = sets.some((s) => s.name === name);
  if (box.kind === "save") {
    if (taken) return { command: { type: "saveMuteSet", name, overwrite: true }, reading: t("mixer.muteSetReadOverwrite", { name }) };
    if (sets.length >= MUTE_SETS_MAX) return { command: null, reading: t("mixer.muteSetsFull", { max: MUTE_SETS_MAX }) };
    return { command: { type: "saveMuteSet", name, overwrite: false }, reading: t("mixer.muteSetReadSave", { name }) };
  }
  if (name === box.from) return { command: null, reading: null };
  if (taken) return { command: null, reading: t("mixer.muteSetTaken", { name }) };
  return { command: { type: "renameMuteSet", name: box.from, to: name }, reading: t("mixer.muteSetReadRename", { name }) };
}

/**
 * The rows: one per set (pressing recalls it; checked when a recall would change nothing), then 「いまのミュートを残す…」,
 * 「名前を変える ▸」 and 「消す ▸」 (these two only when there are sets). Every row is disabled while counting in or recording.
 */
export function muteSetEntries(state: AppState, send: (c: Command) => void, at: MuteSetBoxAt): MenuEntry[] {
  const busy = songBusy(state);
  const sets = state.song.muteSets;
  const full = sets.length >= MUTE_SETS_MAX;
  const save: MenuEntry = {
    kind: "row",
    label: "mixer.muteSetSave",
    usable: !busy && !full,
    hint: full ? t("mixer.muteSetsFull", { max: MUTE_SETS_MAX }) : undefined,
    run: () => openMuteSetBox({ kind: "save" }, at),
  };
  if (sets.length === 0) return [save];
  const each = (run: (name: string) => void): MenuEntry[] =>
    sets.map((s) => ({ kind: "row", label: "menu.muteSets", text: s.name, usable: !busy, run: () => run(s.name) }));
  const none = () => {};
  return [
    ...sets.map(
      (s): MenuEntry => ({
        kind: "row",
        label: "menu.muteSets",
        text: s.name,
        checked: s.asNow,
        usable: !busy,
        run: () => send({ type: "recallMuteSet", name: s.name }),
      }),
    ),
    { kind: "separator" },
    save,
    { kind: "row", label: "mixer.muteSetRename", usable: !busy, run: none, sub: each((name) => openMuteSetBox({ kind: "rename", from: name }, at)) },
    { kind: "row", label: "mixer.muteSetDelete", usable: !busy, run: none, sub: each((name) => send({ type: "deleteMuteSet", name })) },
  ];
}
