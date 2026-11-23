// The fake's side of round 12 M5 「フェードアウトをつける」, as Rust (`meloet_model::automation::add_fade_out`): on the
// master's volume lane a straight line down to the bottom at the last clip's end, over the length of the `bars` bars
// before the bar that end is in. Pure: the fake keeps the points and the lane's view (fake.ts).

import { meterScale } from "../design/tokens";
import { TempoMap } from "../time/tempoMap";
import { FADE_OUT_BARS_MAX } from "./types";
import type { SongView, TimelineView } from "./types";

/** The place (0 to 1) of a fader at `db` (null: −∞), on the scale of spec 2. */
export function fakeFaderPlace(db: number | null): number {
  const top = meterScale[0]![0];
  if (db === null || db < meterScale[meterScale.length - 1]![0]) return 0;
  if (db >= top) return 1;
  for (let i = 1; i < meterScale.length; i++) {
    const [hiDb, hiLen] = meterScale[i - 1]!;
    const [loDb, loLen] = meterScale[i]!;
    if (db >= loDb) return (loLen + ((db - loDb) / (hiDb - loDb)) * (hiLen - loLen)) / 100;
  }
  return 0;
}

/** The value of the straight line through `points` at `tick`: the first point's before it, the last's after. */
function lineAt(points: readonly [number, number][], tick: number): number | null {
  if (points.length === 0) return null;
  if (tick <= points[0]![0]) return points[0]![1];
  for (let i = 1; i < points.length; i++) {
    const [t0, v0] = points[i - 1]!;
    const [t1, v1] = points[i]!;
    if (tick < t1) return v0 + ((v1 - v0) * (tick - t0)) / (t1 - t0);
  }
  return points[points.length - 1]![1];
}

/**
 * The head of the fade-out that is there when a new one from `start` starts on it (Rust `old_fade`): the last line of
 * `points` when it goes down to the bottom and `start` is on it, after its head.
 */
function oldFade(points: readonly [number, number][], start: number): readonly [number, number] | null {
  const head = points[points.length - 2];
  const foot = points[points.length - 1];
  return head && foot && foot[1] === 0 && head[0] < start && start <= foot[0] ? head : null;
}

export interface FakeFade {
  start: number;
  end: number;
  points: [number, number][];
}

/**
 * The points of the master's lane with the fade-out. `lane`: its points now and whether it rests (null: the master has no
 * lane). "noClips": a song with no clip. Null: bars outside 1 to 16, which do nothing.
 */
export function fakeFadeOut(
  song: Pick<SongView, "tracks" | "master">,
  timeline: TimelineView,
  lane: { points: readonly [number, number][]; resting: boolean } | null,
  bars: number,
): FakeFade | "noClips" | null {
  if (!Number.isInteger(bars) || bars < 1 || bars > FADE_OUT_BARS_MAX) return null;
  const map = new TempoMap(timeline);
  const end = Math.max(0, ...song.tracks.flatMap((t) => t.clips.map((c) => c.end)));
  if (end === 0) return "noClips";
  const bar = map.meterAt(end).bar;
  const start = bar < bars ? 0 : end - (map.tickOfBar(bar) - map.tickOfBar(bar - bars));
  const moves = lane !== null && !lane.resting && lane.points.length > 0;
  const kept = (lane?.points ?? []).filter(([tick]) => tick < start).map((p): [number, number] => [p[0], p[1]]);
  const head = moves ? oldFade(lane.points, start) : null;
  // The old fade's head is the last point kept; it goes when the line comes to it flat (nothing heard changes).
  if (head && (kept.length < 2 || kept[kept.length - 2]![1] === head[1])) kept.pop();
  const from = head?.[1] ?? (moves ? lineAt(lane.points, start) : null) ?? fakeFaderPlace(song.master.gainDb);
  return { start, end, points: [...kept, [start, from], [end, 0]] };
}
