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
/** 見せている位置。 */
const positions = new Map<string, number>();
/** 線の上のいちばん新しい位置。 */
const latest = new Map<string, number>();
let moved = $state(0);

// ミキサーのストリップは、自分の上の canvas が出るコマにだけ動かす（docs/architecture.md 7 章「CPU で描く WebKit」: 同じコマで変わる物は
// 1 枚の canvas の下）。トラックが 10 本をこえると canvas は左右の半分ずつ 1 コマおきに出るので、出ない半分のつまみと字を同じコマで
// 動かすと、よごれた四角が 11 個をこえて窓ぜんぶの描きなおしになる。
/** 待つトラック → 待たせている箱（coverIf）が、位置を見せたときに呼んでほしい関数。 */
const gates = new Map<number, Set<() => void>>();
/** 今回、見せる位置が変わった待つトラック。 */
const touched = new Set<number>();
/** いちばん新しい位置をまだ見せていない、待つトラック → 待ったコマの数。 */
const waiting = new Map<number, number>();
/**
 * 上の canvas が描かれないまま待つコマの数の上限。メーターの知らせが来ないコマは canvas が描かれないので、次の番まで待つ。
 * 待たずに見せると、左右の半分のつまみが同じコマで動いて、窓ぜんぶの描きなおしになる。
 */
export const MAX_WAIT_FRAMES = 3;

/** 帯が動かしている物の、線の上のいまの位置（0〜1）。動かしていなければ null（手で置いた値のまま）。 */
export function livePosition(track: number, target: AutoTarget): number | null {
  void moved;
  return positions.get(laneKey(track, target)) ?? null;
}

/** 位置を書く細かさ（つまみの長さの 1/4096）。同じなら書かない。 */
const STEP = 1 / 4096;

const seen = new Set<string>();

/**
 * tick の所の線の上の位置を書く。動かしていない帯と、なくなった帯は null に。
 * inFrame: WebGL の係のコマの中。待つトラックの位置は、その上の canvas を描くとき（showTrack）か、MAX_WAIT_FRAMES コマ描かれなければ見せる。
 */
export function refreshPositions(state: AppState, tick: number, inFrame = false): void {
  changed = false;
  for (const [track, frames] of waiting) {
    if (inFrame && frames < MAX_WAIT_FRAMES) {
      waiting.set(track, frames + 1);
      continue;
    }
    waiting.delete(track);
    show(track);
  }
  seen.clear();
  const held = state.automation.held;
  for (const track of state.song.tracks) refreshTrack(track, held, tick, inFrame && gates.has(track.id));
  for (const key of latest.keys()) {
    if (seen.has(key)) continue;
    latest.delete(key);
    if (positions.delete(key)) changed = true;
  }
  told();
}
let changed = false;

/** 見せる位置が変わったら、読んでいる所と、待たせていた箱に知らせる。 */
function told(): void {
  if (changed) moved += 1;
  for (const track of touched) for (const shown of gates.get(track) ?? []) shown();
  touched.clear();
}

function show(track: number): void {
  const prefix = `${track}/`;
  for (const [key, pos] of latest) {
    if (!key.startsWith(prefix) || positions.get(key) === pos) continue;
    positions.set(key, pos);
    changed = true;
    touched.add(track);
  }
}

/** 待っていたトラックの、いちばん新しい位置を見せる（そのトラックの箱の上の canvas を描くとき）。 */
export function showTrack(track: number): void {
  if (!waiting.delete(track)) return;
  changed = false;
  show(track);
  told();
}

/** そのトラックを待たせる（箱が canvas の下にある間）。shown は、そのトラックの見せる位置が変わるたびに呼ぶ。返す関数でやめる。 */
export function gateTrack(track: number, shown: () => void): () => void {
  const set = gates.get(track) ?? new Set();
  gates.set(track, set);
  set.add(shown);
  return () => {
    set.delete(shown);
    if (set.size > 0 || gates.get(track) !== set) return;
    gates.delete(track);
    showTrack(track);
  };
}

/** 見せる位置が変わった数（変わったあとに何かするために読む）。 */
export const positionsVersion = () => moved;

function refreshTrack(track: TrackView, held: AppState["automation"]["held"], tick: number, wait: boolean): void {
  for (const lane of track.lanes) {
    if (!laneMoves(lane, held, track.id)) continue;
    const points = peekLanePoints(lane.laneId) ?? pointsOf(lane.laneId);
    if (!points) continue;
    const v = valueAt(points, tick);
    if (v === null) continue;
    const key = laneKey(track.id, lane.target);
    seen.add(key);
    const pos = Math.round(v / STEP) * STEP;
    latest.set(key, pos);
    if (positions.get(key) === pos) continue;
    if (wait) {
      if (!waiting.has(track.id)) waiting.set(track.id, 0);
    }
    else {
      positions.set(key, pos);
      changed = true;
      if (gates.has(track.id)) touched.add(track.id);
    }
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
