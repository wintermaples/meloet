// 帯を足す一覧（仕様書 3 章 92-5 ・ 20-A、94-9 ・ 36-B）: 音量 ・ パン ・ ミュート ・ センド A ・ センド B ／ エフェクトのパラメータ…。
// 見出しの下の「＋ 帯を足す ▾」の行と、見出しの「帯」のボタン（モック F8-FirstLane）が同じ一覧をひらく。
// 足してある帯には選ぶ印。行を押すと AddLane（足してある帯がたたんであれば、core がひらく）。「エフェクトのパラメータ…」は
// 右のパラメータの欄の一覧をひらく（そこの行の右の印で帯を足す）。

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

/** パラメータの欄にひらく物: プラグインの音源、なければいちばん上のエフェクト。 */
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

/** 見出しの「帯」のボタンからひらいた一覧（ひらいている間、ボタンは accent、状態行は F8-FirstLane の文）。 */
const fromHead = $state({ track: null as number | null });
let headMenu: unknown = null;

/** el のすぐ下に一覧をひらく。head: 見出しの「帯」のボタンから。 */
export function openLaneAdd(el: HTMLElement, track: TrackView, busy: boolean, head: boolean): void {
  const r = el.getBoundingClientRect();
  openContextMenu(r.left, r.bottom, laneAddEntries(track, busy));
  headMenu = head ? contextMenu.open : null;
  fromHead.track = head && contextMenu.open ? track.id : null;
}

/** 見出しの「帯」のボタンからひらいた一覧がひらいているトラックの id。なければ null。 */
export function laneAddOpenFor(): number | null {
  const open = contextMenu.open;
  return open !== null && open === headMenu ? fromHead.track : null;
}
