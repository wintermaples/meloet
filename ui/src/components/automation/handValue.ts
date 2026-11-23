// The position (0 to 1) where the hand is while writing with 「● 書く」. Written when mixer and params panel drags send a "sound only" intent
// (commit: false), and read every frame by the lane (AutoLane) to draw the line of the part being written. The core puts the written values in as
// one step on release (docs/architecture.md 21-9), so until then the screen draws the line from the hand's value. Values moved in a plugin window are not known.

import type { AutoTarget, Command } from "../../bridge";
import { EFFECT_PARAMS } from "../../bridge/types";
import { current } from "../../state/app.svelte";
import { dbToFrac } from "../mixer/scale";
import { laneKey, panPosition, paramPosition } from "./laneValue";

const hands = new Map<string, number>();

/** The lane the intent moves (track and what), and its position. Null for things that are not lanes (returns, master). */
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

/** Called when sending a "sound only" intent. */
export function noteHand(c: Command): void {
  const h = handOf(c);
  if (h) hands.set(laneKey(h.track, h.target), Math.min(1, Math.max(0, h.pos)));
}

/** Released or cancelled. */
export function forgetHand(c: Command): void {
  const h = handOf(c);
  if (h) hands.delete(laneKey(h.track, h.target));
}

/** The position where the hand is now on that lane. Null if there is no hand. */
export function handAt(track: number, target: AutoTarget): number | null {
  return hands.get(laneKey(track, target)) ?? null;
}
