// オートメーションの帯の値（docs/architecture.md 21-5 ・ 21-8）。DOM を知らない純粋な関数。
// 帯の値は、どれも「人が見ているつまみの位置」0〜1。線は位置でまっすぐ、横はティックでまっすぐ。
// 画面は、点と再生の線の位置から、自分で線の上の値を出す（値は流れてこない）。

import type { AutoTarget, LaneName, LanePoints, LaneRef, LaneView, TrackView } from "../../bridge";
import { EFFECT_PARAMS, type EffectParam } from "../../bridge/types";
import { t } from "../../text/ja";
import { dbToFrac, faderText, fracToDb, panText } from "../mixer/scale";
import { fractionOf, unitText, valueAt as paramValueAt, valueText as paramValueText } from "../params/paramRows";

/** 線の上の値。最初の点より前は最初の点、最後の点より後は最後の点。階段は、その位置か前のいちばん近い点。点がなければ null。 */
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
export const hasLaneRef = (list: readonly LaneRef[], track: number, target: AutoTarget) =>
  list.some((l) => l.track === track && sameTarget(l.target, target));

/** トラックの、その物の帯。 */
export const laneOf = (track: Pick<TrackView, "lanes">, target: AutoTarget): LaneView | undefined =>
  track.lanes.find((l) => sameTarget(l.target, target));

/** 最初から入っているエフェクトのパラメータ（帯の名前から）。 */
function builtinSpec(name: LaneName): EffectParam | null {
  return name.kind === "builtin" ? (EFFECT_PARAMS[name.effect][name.param] ?? null) : null;
}

/** 段階 ・ オン ・ オフの位置は n /（段の数 − 1）（21-5）。 */
function stepsOf(p: EffectParam): number {
  return p.shape === "switch" ? 2 : p.steps.length;
}

/** 位置 → 最初から入っているエフェクトの値（パラメータの欄の棒の位置。段階は段の番号、オン ・ オフは 0 か 1）。 */
export function paramAt(p: EffectParam, pos: number): number {
  if (p.shape === "continuous") return paramValueAt(p, pos);
  const n = stepsOf(p);
  return Math.max(0, Math.min(n - 1, Math.round(pos * (n - 1))));
}

/** 最初から入っているエフェクトの値 → 位置。 */
export function paramPosition(p: EffectParam, value: number): number {
  if (p.shape === "continuous") return fractionOf(p, value);
  const n = stepsOf(p);
  return n > 1 ? Math.max(0, Math.min(1, Math.round(value) / (n - 1))) : 0;
}

export const panAt = (pos: number) => Math.max(-100, Math.min(100, Math.round(pos * 200 - 100)));
export const panPosition = (pan: number) => (pan + 100) / 200;

/** 位置の文（「−4.8 dB」「L 20」「オン」「−18.0 dB」）。プラグインは画面で出せないので null（値の文字はプラグインが言う）。 */
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
  }
}

/** 読み値の「→」の前に書く、単位のない文（「−2.1」）。単位のない値は positionText と同じ。 */
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
 * 手で置いた値の位置（休みのとき、点のない帯の線）。最初から入っているエフェクトの値は、パラメータの欄で見せている枠のときだけ
 * 分かる（`values`）。分からなければ null。
 */
export function handPosition(track: TrackView, target: AutoTarget, values: readonly number[] | null): number | null {
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

/** その帯が、いま値を動かしているか（休みでも、仮の休みでもなく、点がある）。 */
export function laneMoves(lane: LaneView | undefined, held: readonly LaneRef[], track: number): boolean {
  return lane !== undefined && !lane.resting && lane.points > 0 && !hasLaneRef(held, track, lane.target);
}

/** オートメーションが動かしているか（moving）、休みか（resting）。null は帯なし（仕様書 2 章「オートメーションで動くつまみ」）。 */
export type AutoLook = "moving" | "resting" | null;
export interface StripAuto {
  gain: AutoLook;
  pan: AutoLook;
  sends: readonly [AutoLook, AutoLook];
  /** フェーダーの値の下の行を出すか（どこかのトラックに音量の帯があると、どのストリップにも出して、高さをそろえる）。 */
  row: boolean;
  /** 音量の帯の「もどす」。 */
  restore: (() => void) | null;
}
