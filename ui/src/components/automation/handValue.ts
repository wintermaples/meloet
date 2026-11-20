// 「● 書く」で書いている間の、手が置いている位置（0〜1）。ミキサーとパラメータの欄のドラッグが「音だけ変える」意図
// （commit: false）を送るときに書き、帯（AutoLane）が書いている所の線を描くのに毎コマ読む。core は書いた値を離したときに
// 1 手で入れる（docs/architecture.md 21-9）ので、それまでの線は画面が手の値から描く。プラグインの窓で動かした値は分からない。

import type { AutoTarget, Command } from "../../bridge";
import { EFFECT_PARAMS } from "../../bridge/types";
import { current } from "../../state/app.svelte";
import { dbToFrac } from "../mixer/scale";
import { laneKey, panPosition, paramPosition } from "./laneValue";

const hands = new Map<string, number>();

/** 意図が動かす帯（トラックと何を）と、その位置。帯にならない物（戻り、マスター）は null。 */
function handOf(c: Command): { track: number; target: AutoTarget; pos: number } | null {
  switch (c.type) {
    case "setGain":
      return c.strip.kind === "track" ? { track: c.strip.id, target: { kind: "volume" }, pos: dbToFrac(c.db) } : null;
    case "setPan":
      return c.strip.kind === "track" ? { track: c.strip.id, target: { kind: "pan" }, pos: panPosition(c.pan) } : null;
    case "setSend":
      return { track: c.track, target: { kind: "send", bus: c.bus }, pos: dbToFrac(c.db) };
    case "setParam": {
      const strip = c.insert.strip;
      if (strip.kind !== "track") return null;
      const insert = current.state?.song.tracks.find((t) => t.id === strip.id)?.slots[c.insert.slot]?.id;
      const p = EFFECT_PARAMS[c.effect][c.param];
      return insert === undefined || !p ? null : { track: strip.id, target: { kind: "insert", insert, param: c.param }, pos: paramPosition(p, c.value) };
    }
    case "setPluginParam": {
      if (c.slot.kind === "instrument") return { track: c.slot.track, target: { kind: "instrument", param: c.param }, pos: c.value };
      const strip = c.slot.insert.strip;
      if (strip.kind !== "track") return null;
      const insert = current.state?.song.tracks.find((t) => t.id === strip.id)?.slots[c.slot.insert.slot]?.id;
      return insert === undefined ? null : { track: strip.id, target: { kind: "insert", insert, param: c.param }, pos: c.value };
    }
    default:
      return null;
  }
}

/** 「音だけ変える」意図を送るときに呼ぶ。 */
export function noteHand(c: Command): void {
  const h = handOf(c);
  if (h) hands.set(laneKey(h.track, h.target), Math.min(1, Math.max(0, h.pos)));
}

/** 離した ・ やめた。 */
export function forgetHand(c: Command): void {
  const h = handOf(c);
  if (h) hands.delete(laneKey(h.track, h.target));
}

/** その帯にいま手が置いている位置。手がなければ null。 */
export function handAt(track: number, target: AutoTarget): number | null {
  return hands.get(laneKey(track, target)) ?? null;
}
