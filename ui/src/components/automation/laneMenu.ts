// Right-click on an automation lane (spec 3 (Y16) 「右クリックの「帯を消す」で、点ごと消す（取り消せる 1 手）」).

import type { LaneView } from "../../bridge";
import { send } from "../../state/app.svelte";
import { openContextMenu } from "../menu/contextMenu.svelte";
import { trackHome, type LaneOwner } from "./laneValue";

export function laneMenu(e: MouseEvent, track: LaneOwner, lane: LaneView, busy: boolean): void {
  openContextMenu(e.clientX, e.clientY, [
    { kind: "row", label: "lane.delete", usable: !busy, run: () => send({ type: "deleteLane", home: trackHome(track.id), target: lane.target }) },
  ]);
}
