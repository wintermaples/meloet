// Values of automation lanes (docs/architecture.md 21-5, 21-8). Pure functions that do not know the DOM.
// Every lane value is "the position of the knob the person sees", 0 to 1. The line is straight in position, and sideways straight in ticks.
// The screen works out the value on the line itself from the points and the play line position (values are not streamed).

import type { AutoTarget, LaneHome, LaneName, LanePoints, LaneRef, LaneView, MasterView, TrackView } from "../../bridge";
import { EFFECT_PARAMS, type EffectParam } from "../../bridge/types";
import { t } from "../../text/ja";
import { dbToFrac, faderText, fracToDb, panText } from "../mixer/scale";
import { fractionOf, unitText, valueAt as paramValueAt, valueText as paramValueText } from "../params/paramRows";

/** The value on the line. Before the first point, the first point; after the last point, the last point. Stepped: the nearest point at or before the position. Null if there are no points. */
export function valueAt(p: Pick<LanePoints, "ticks" | "values" | "step">, tick: number): number | null {
  const n = p.ticks.length;
  if (n === 0) return null;
  if (tick <= p.ticks[0]!) return p.values[0]!;
  if (tick >= p.ticks[n - 1]!) return p.values[n - 1]!;
  let lo = 0;
  let hi = n - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (p.ticks[mid]! <= tick) lo = mid;
    else hi = mid;
  }
  const a = p.values[lo]!;
  if (p.step) return a;
  const t0 = p.ticks[lo]!;
  const t1 = p.ticks[hi]!;
  return a + ((p.values[hi]! - a) * (tick - t0)) / (t1 - t0);
}

export const targetKey = (target: AutoTarget): string => {
  switch (target.kind) {
    case "send":
      return `send:${target.bus}`;
    case "instrument":
      return `instrument:${target.param}`;
    case "insert":
      return `insert:${target.insert}:${target.param}`;
    default:
      return target.kind;
  }
};
export const sameTarget = (a: AutoTarget, b: AutoTarget) => targetKey(a) === targetKey(b);
export const laneKey = (track: number, target: AutoTarget) => `${track}/${targetKey(target)}`;
/**
 * The number that stands for the master where the screen names the home of lanes by a track's id (round 12 M5: the master
 * has one lane, of its volume). No track has it: ids from Rust are not negative.
 */
export const MASTER_HOME = -1;
export const masterHome: LaneHome = { kind: "master" };
/** The home of a track's lanes in the intents (`LaneHome`); of the master's for `MASTER_HOME`. */
export const trackHome = (track: number): LaneHome => (track === MASTER_HOME ? masterHome : { kind: "track", track });
export const isOnMaster = (lane: LaneRef) => lane.home.kind === "master";
/** Whether the lane is a lane of that track (of the master for `MASTER_HOME`). */
export const isOnTrack = (lane: LaneRef, track: number) =>
  track === MASTER_HOME ? isOnMaster(lane) : lane.home.kind === "track" && lane.home.track === track;
/** What the lane parts read of the one whose lanes they show: a track, or the master standing in as one. */
export type LaneOwner = Pick<TrackView, "id" | "lanes" | "gainDb" | "pan" | "sends" | "muted" | "slots">;
/** The master as the owner of its lane: the hand's value of the lane is its fader. */
export const masterOwner = (master: MasterView): LaneOwner => ({
  id: MASTER_HOME,
  lanes: master.lanes,
  gainDb: master.gainDb,
  pan: master.pan,
  sends: [null, null],
  muted: false,
  slots: master.slots,
});
export const hasLaneRef = (list: readonly LaneRef[], track: number, target: AutoTarget) =>
  list.some((l) => isOnTrack(l, track) && sameTarget(l.target, target));

/** The track's lane for that thing. */
export const laneOf = (track: Pick<LaneOwner, "lanes">, target: AutoTarget): LaneView | undefined =>
  track.lanes.find((l) => sameTarget(l.target, target));

/** A parameter of a built-in effect (from the lane name). */
function builtinSpec(name: LaneName): EffectParam | null {
  return name.kind === "builtin" ? (EFFECT_PARAMS[name.effect][name.param] ?? null) : null;
}

/** The position of steps and on/off is n / (number of steps − 1) (21-5). */
function stepsOf(p: EffectParam): number {
  return p.shape === "switch" ? 2 : p.steps.length;
}

/** Position → value of a built-in effect (the bar position in the params panel; steps are the step index, on/off is 0 or 1). */
export function paramAt(p: EffectParam, pos: number): number {
  if (p.shape === "continuous") return paramValueAt(p, pos);
  const n = stepsOf(p);
  return Math.max(0, Math.min(n - 1, Math.round(pos * (n - 1))));
}

/** Value of a built-in effect → position. */
export function paramPosition(p: EffectParam, value: number): number {
  if (p.shape === "continuous") return fractionOf(p, value);
  const n = stepsOf(p);
  return n > 1 ? Math.max(0, Math.min(1, Math.round(value) / (n - 1))) : 0;
}

export const panAt = (pos: number) => Math.max(-100, Math.min(100, Math.round(pos * 200 - 100)));
export const panPosition = (pan: number) => (pan + 100) / 200;

/** The text of a position (「−4.8 dB」「L 20」「オン」「−18.0 dB」). Null for plugins, since the screen cannot make it (the plugin gives the value text). */
export function positionText(name: LaneName, pos: number): string | null {
  switch (name.kind) {
    case "volume":
    case "send": {
      const db = fracToDb(pos);
      return db === null ? t("common.dbSilent") : t("common.db", { value: faderText(db) });
    }
    case "pan":
      return panText(panAt(pos));
    case "mute":
      return t(pos >= 0.5 ? "param.on" : "param.off");
    case "builtin": {
      const p = builtinSpec(name);
      return p ? paramValueText(name.effect, p, paramAt(p, pos)) : null;
    }
    case "plugin":
      return null;
    case "knob":
      return t("param.value.percent", { value: String(Math.round(pos * 100)) });
  }
}

/** The text without a unit before the 「→」 of the reading (「−2.1」). For values without a unit, the same as positionText. */
export function bareText(name: LaneName, pos: number): string | null {
  switch (name.kind) {
    case "volume":
    case "send":
      return faderText(fracToDb(pos));
    case "builtin": {
      const p = builtinSpec(name);
      const text = positionText(name, pos);
      const unit = p ? unitText(p) : "";
      return text !== null && unit !== "" && text.endsWith(` ${unit}`) ? text.slice(0, -unit.length - 1) : text;
    }
    default:
      return positionText(name, pos);
  }
}

/**
 * The position of the value set by hand (when resting, the line of a lane with no points). A built-in effect's value is known only while the params panel shows
 * that slot (`values`). Null if not known.
 */
export function handPosition(track: LaneOwner, target: AutoTarget, values: readonly number[] | null): number | null {
  switch (target.kind) {
    case "volume":
      return dbToFrac(track.gainDb);
    case "pan":
      return panPosition(track.pan);
    case "send":
      return dbToFrac(track.sends[target.bus === "a" ? 0 : 1]);
    case "mute":
      return track.muted ? 1 : 0;
    case "insert": {
      const effect = track.slots.find((s) => s?.id === target.insert)?.effect;
      if (effect?.kind !== "builtin" || !values) return null;
      const p = EFFECT_PARAMS[effect.effect][target.param];
      const v = values[target.param];
      return p && v !== undefined ? paramPosition(p, v) : null;
    }
    case "instrument":
      return null;
  }
}

/** Whether the lane is moving a value now (not resting, not resting for now, and with points). */
export function laneMoves(lane: LaneView | undefined, held: readonly LaneRef[], track: number): boolean {
  return lane !== undefined && !lane.resting && lane.points > 0 && !hasLaneRef(held, track, lane.target);
}

/** Whether automation is moving it (moving) or it is resting (resting). Null for no lane (spec 2, "Handle moved by automation"). */
export type AutoLook = "moving" | "resting" | null;
export interface StripAuto {
  gain: AutoLook;
  pan: AutoLook;
  sends: readonly [AutoLook, AutoLook];
  /** Whether to show the line below the fader value (if any track has a gain lane, it is shown in every strip so the heights line up). */
  row: boolean;
  /** 「もどす」 for the gain lane. */
  restore: (() => void) | null;
  /** The master's strip: the mark by the fader's value that makes (or unfolds) the lane of its volume. */
  mark?: { on: boolean; usable: boolean; add: () => void };
}
