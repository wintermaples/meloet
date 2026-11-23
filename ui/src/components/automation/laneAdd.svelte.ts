// The list to add a lane (spec 3 (92-5, 20-A), (94-9, 36-B)): gain, pan, mute, send A, send B / effect parameters….
// The 「＋ 帯を足す ▾」 row below the heading and the 「帯」 button in the head (mock F8-FirstLane) open the same list.
// Lanes already added have a choice mark. Pressing a row sends AddLane (if the added lane is folded, the core opens it). 「エフェクトのパラメータ…」
// opens the list in the params panel on the right (a lane is added with the mark on the right of a row there).

import type { AutoTarget, Command, LaneName, TrackView } from "../../bridge";
import { send } from "../../state/app.svelte";
import { laneNameText } from "../history/stepText";
import { contextMenu, openContextMenu, type MenuEntry } from "../menu/contextMenu.svelte";
import { laneOf } from "./laneValue";

const TARGETS: readonly { target: AutoTarget; name: LaneName }[] = [
  { target: { kind: "volume" }, name: { kind: "volume" } },
  { target: { kind: "pan" }, name: { kind: "pan" } },
  { target: { kind: "mute" }, name: { kind: "mute" } },
  { target: { kind: "send", bus: "a" }, name: { kind: "send", bus: "a" } },
  { target: { kind: "send", bus: "b" }, name: { kind: "send", bus: "b" } },
];

/** What to open in the params panel: a plugin instrument, otherwise the top effect. */
function paramsCommand(track: TrackView): Command | null {
  if (track.sound.kind === "plugin") return { type: "selectInstrument", track: track.id };
  const slot = track.slots.findIndex((s) => s !== null);
  return slot < 0 ? null : { type: "selectInsert", insert: { strip: { kind: "track", id: track.id }, slot } };
}

export function laneAddEntries(track: TrackView, busy: boolean): MenuEntry[] {
  const id = track.id;
  const rows: MenuEntry[] = TARGETS.map(({ target, name }) => ({
    kind: "row",
    label: "lane.volume",
    text: laneNameText(name),
    usable: !busy,
    checked: laneOf(track, target) !== undefined,
    run: () => send({ type: "addLane", track: id, target }),
  }));
  const params = paramsCommand(track);
  rows.push({ kind: "separator" }, { kind: "row", label: "lane.addEffectParams", usable: params !== null, run: () => params && send(params) });
  return rows;
}

/** The list opened from the 「帯」 button in the head (while open, the button is accent and the status line shows the F8-FirstLane text). */
const fromHead = $state({ track: null as number | null });
let headMenu: unknown = null;

/** Opens the list just below el. head: from the 「帯」 button in the head. */
export function openLaneAdd(el: HTMLElement, track: TrackView, busy: boolean, head: boolean): void {
  const r = el.getBoundingClientRect();
  openContextMenu(r.left, r.bottom, laneAddEntries(track, busy));
  headMenu = head ? contextMenu.open : null;
  fromHead.track = head && contextMenu.open ? track.id : null;
}

/** The id of the track whose list opened from the 「帯」 button in the head is open. Null if none. */
export function laneAddOpenFor(): number | null {
  const open = contextMenu.open;
  return open !== null && open === headMenu ? fromHead.track : null;
}
