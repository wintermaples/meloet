// オートメーションの帯の点と、線の上のいまの値（docs/architecture.md 21-8）。
// 点は LaneStore（ui/src/bridge/lane.ts）が laneId ごとに 1 回だけ取る。ここは、届いたら描きなおすための数と、
// 帯が動かしている物（音量、パン、センド、ミュート、最初から入っているエフェクトのパラメータ）のいまの位置を持つ。
// 位置は AutoClock が、止まっているときは状態が変わったときに、動いているときは WebGL の係のコマごとに書く。
// 書くのは値が変わったときだけで、読んでいる所（ミキサー、帯の見出し、パラメータの欄）だけが動く。

import { clockOffsetUs, lanePoints, peekLanePoints, type AppState, type AutoTarget, type LanePoints, type TrackView } from "../../bridge";
import { current } from "../../state/app.svelte";
import { shownTick } from "../time";
import { laneKey, laneMoves, valueAt } from "./laneValue";

let version = $state(0);
const RETRY_MS = 100;
const asking = new Set<number>();

/** 帯の点。まだなければ取りにいき、届いたら、これを読んでいた所が描きなおす。 */
export function pointsOf(laneId: number): LanePoints | undefined {
  void version;
  const had = peekLanePoints(laneId);
  if (had || asking.has(laneId)) return had;
  asking.add(laneId);
  try {
    lanePoints(laneId).then(
      () => {
        asking.delete(laneId);
        version += 1;
      },
      () => asking.delete(laneId),
    );
  } catch {
    // つながる前（最初の状態は、つながりきる少し前に来る）。少しあとで、読んでいる所にもう一度取りにいかせる。
    asking.delete(laneId);
    setTimeout(() => (version += 1), RETRY_MS);
  }
  return undefined;
}

// ---- いまの位置 ----

// 位置はふつうの Map に持ち、変わったら数を 1 つ進める（読む所はこの数で読みなおす）。位置ごとに $state を作らないのは、
// 読む所（$derived の中）で初めて作った $state は、その $derived の依存にならず、あとで変わっても読みなおされないため。
const positions = new Map<string, number>();
let moved = $state(0);

/** 帯が動かしている物の、線の上のいまの位置（0〜1）。動かしていなければ null（手で置いた値のまま）。 */
export function livePosition(track: number, target: AutoTarget): number | null {
  void moved;
  return positions.get(laneKey(track, target)) ?? null;
}

/** 位置を書く細かさ（つまみの長さの 1/4096）。同じなら書かない。 */
const STEP = 1 / 4096;

const seen = new Set<string>();

/** tick の所の線の上の位置を書く。動かしていない帯と、なくなった帯は null に。 */
export function refreshPositions(state: AppState, tick: number): void {
  seen.clear();
  changed = false;
  const held = state.automation.held;
  for (const track of state.song.tracks) refreshTrack(track, held, tick);
  for (const key of positions.keys()) {
    if (seen.has(key)) continue;
    positions.delete(key);
    changed = true;
  }
  if (changed) moved += 1;
}
let changed = false;

function refreshTrack(track: TrackView, held: AppState["automation"]["held"], tick: number): void {
  for (const lane of track.lanes) {
    if (!laneMoves(lane, held, track.id)) continue;
    const points = peekLanePoints(lane.laneId) ?? pointsOf(lane.laneId);
    if (!points) continue;
    const v = valueAt(points, tick);
    if (v === null) continue;
    const key = laneKey(track.id, lane.target);
    seen.add(key);
    const pos = Math.round(v / STEP) * STEP;
    if (positions.get(key) === pos) continue;
    positions.set(key, pos);
    changed = true;
  }
}

/** 帯があるか（コマごとに値を出すかどうか）。 */
export const anyLane = (state: AppState) => state.song.tracks.some((t) => t.lanes.length > 0);

// ---- 再生の線の位置 ----

let offsetUs: number | null = null;
let askingOffset = false;

/** 再生の線がいる所（再生まわりの数字と同じ。docs: components/time.ts の shownTick）。 */
export function playTick(): number {
  const state = current.state;
  if (!state) return 0;
  if (state.transport.state !== "stopped" && offsetUs === null && !askingOffset) {
    askingOffset = true;
    clockOffsetUs().then(
      (v) => (offsetUs = v),
      () => (askingOffset = false),
    );
  }
  return shownTick(state.transport, current.telemetry, offsetUs === null ? null : performance.now() * 1000 + offsetUs);
}

/** 点が届いた数（描きなおしのきっかけに読む）。 */
export const pointsVersion = () => version;
