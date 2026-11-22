// オートメーションの帯の点と、線の上のいまの値（docs/architecture.md 21-8）。
// 点は LaneStore（ui/src/bridge/lane.ts）が laneId ごとに 1 回だけ取る。ここは、届いたら描きなおすための数と、
// 帯が動かしている物（音量、パン、センド、ミュート、最初から入っているエフェクトのパラメータ）のいまの位置を持つ。
// 位置は AutoClock が、止まっているときは状態が変わったときに、動いているときは WebGL の係のコマごとに書く。
// 書くのは値が変わったときだけで、読んでいる所（ミキサー、帯の見出し、パラメータの欄）だけが動く。

import { clockOffsetUs, lanePoints, peekLanePoints, type AppState, type AutoTarget, type LanePoints, type TrackView } from "../../bridge";
import { current } from "../../state/app.svelte";
import { shownTick } from "../time";
import { laneMoves, targetKey, valueAt } from "./laneValue";

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

// 位置はふつうの Map に持ち、変わったらトラックごとの数を 1 つ進める（読む所はこの数で読みなおす）。トラックごとなので、
// 64 本の曲でも、読みなおすのは動いたトラックを見せている所だけ。数（$state）は書く側が先に作る: 読む所（$derived の中）で初めて
// 作った $state は、その $derived の依存にならない。まだない数を読んだ所は made で読みなおす。
class Tick {
  n = $state(0);
}
let made = $state(0);
function reads(ticks: Map<number, Tick>, track: number): void {
  const t = ticks.get(track);
  if (t) void t.n;
  else void made;
}
function bump(ticks: Map<number, Tick>, track: number): void {
  const t = ticks.get(track);
  if (t) t.n += 1;
  else {
    ticks.set(track, new Tick());
    made += 1;
  }
}

/** 線の上のいちばん新しい位置（トラック → 何を（targetKey）→ 位置）。帯の見出し、パラメータの欄、ミキサーの値の字が読む。 */
const latest = new Map<number, Map<string, number>>();
const latestTicks = new Map<number, Tick>();
/** ミキサーのストリップに見せている位置。待つトラックでは、latest よりおくれる。 */
const shown = new Map<number, Map<string, number>>();
const shownTicks = new Map<number, Tick>();
/** 位置を画面に書いた回数（書いたあとに何かする所が読む）。 */
let wrote = $state(0);
let any = false;

// ミキサーのストリップは、自分の上の canvas が出るコマにだけ動かす（docs/architecture.md 7 章「CPU で描く WebKit」: 同じコマで変わる物は
// 1 枚の canvas の下）。ミキサーの canvas は左右の半分ずつ 1 コマおきに出るので、出ない半分のつまみと字を同じコマで動かすと、
// よごれた四角が 11 個をこえて窓ぜんぶの描きなおしになる。
/** ストリップの中の、帯が動かす物の組。 */
export type StripPart = "gain" | "pan" | "sends";
const PARTS: readonly StripPart[] = ["gain", "pan", "sends"];
/** ストリップが見せない物（エフェクトや音源のパラメータ）は null。 */
function partOf(key: string): StripPart | null {
  if (key === "volume" || key === "mute") return "gain";
  if (key === "pan") return "pan";
  return key.startsWith("send:") ? "sends" : null;
}
/** 待つトラック → 組ごとの、位置を見せたときに呼んでほしい関数（coverIf ・ askIf）。 */
const gates = new Map<number, Map<StripPart, Set<() => void>>>();
/**
 * 待つトラックのパンとセンドは、1 回に片方ずつ見せる: パンの行とセンドの行は canvas の外にあるので、同じコマに両方の四角を足すと、
 * 1 コマに描きなおす広さが 25 % の決まりに近づく。どちらの番かは、どのトラックも同じ（コマ 2 つごとに入れかわる。canvas は左右の半分を
 * 1 コマおきに出すので、どちらの半分にも、パンの番とセンドの番が交互に来る）。そろえないと、行の四角がストリップごとにばらばらになる。
 */
const rowTurn = (): StripPart => ((frameNo >> 1) & 1 ? "pan" : "sends");
/** いちばん新しい位置をまだ見せていない、待つトラック → 待ったコマの数。 */
const waiting = new Map<number, number>();
/**
 * 上の canvas が描かれないまま待つコマの数の上限。メーターの知らせが来ないコマは canvas が描かれないので、次の番まで待つ。
 * 待たずに見せると、左右の半分のつまみが同じコマで動いて、窓ぜんぶの描きなおしになる。
 * ほかのトラックの canvas は描かれているのに自分の番が来ないトラックは、見えていない（横に送った外）ので、見えるまで待つ。
 */
export const MAX_WAIT_FRAMES = 3;
let frameNo = 0;
/** どれかの箱の上の canvas が、最後に描かれたコマ。 */
let paintedAt = -Infinity;

/** 帯が動かしている物の、線の上のいまの位置（0〜1）。動かしていなければ null（手で置いた値のまま）。 */
export function livePosition(track: number, target: AutoTarget): number | null {
  reads(latestTicks, track);
  return latest.get(track)?.get(targetKey(target)) ?? null;
}

/** ミキサーのストリップに見せる位置（待つトラックでは、その上の canvas を描いたコマの位置）。 */
export function shownPosition(track: number, target: AutoTarget): number | null {
  reads(shownTicks, track);
  return shown.get(track)?.get(targetKey(target)) ?? null;
}

/** 位置を書く細かさ（つまみの長さの 1/4096）。同じなら書かない。 */
const STEP = 1 / 4096;

const seenTracks = new Set<number>();
const seenKeys = new Set<string>();
/** 帯の番号 → 何を（毎コマ文字を作らない）。 */
const laneKeys = new Map<number, string>();

/**
 * tick の所の線の上の位置を書く。動かしていない帯と、なくなった帯は null に。
 * inFrame: WebGL の係のコマの中。待つトラックの位置は、その上の canvas を描くとき（showTrack）に見せる。
 */
export function refreshPositions(state: AppState, tick: number, inFrame = false): void {
  any = false;
  if (inFrame) frameNo += 1;
  const drawing = inFrame && frameNo - paintedAt <= MAX_WAIT_FRAMES;
  for (const [track, frames] of waiting) {
    if (inFrame && (frames < MAX_WAIT_FRAMES || drawing)) {
      if (frames < MAX_WAIT_FRAMES) waiting.set(track, frames + 1);
      continue;
    }
    waiting.delete(track);
    show(track);
  }
  seenTracks.clear();
  const held = state.automation.held;
  for (const track of state.song.tracks) refreshTrack(track, held, tick, inFrame && gates.has(track.id));
  for (const track of latest.keys()) {
    if (seenTracks.has(track)) continue;
    latest.delete(track);
    bump(latestTicks, track);
    tellLive(track);
    waiting.delete(track);
    show(track);
  }
  if (laneKeys.size > 4 * latest.size + 256) laneKeys.clear();
  if (any) wrote += 1;
}

function refreshTrack(track: TrackView, held: AppState["automation"]["held"], tick: number, wait: boolean): void {
  let map = latest.get(track.id);
  let changed = false;
  seenKeys.clear();
  for (const lane of track.lanes) {
    if (!laneMoves(lane, held, track.id)) continue;
    const points = peekLanePoints(lane.laneId) ?? pointsOf(lane.laneId);
    if (!points) continue;
    const v = valueAt(points, tick);
    if (v === null) continue;
    let key = laneKeys.get(lane.laneId);
    if (key === undefined) laneKeys.set(lane.laneId, (key = targetKey(lane.target)));
    seenKeys.add(key);
    const pos = Math.round(v / STEP) * STEP;
    if (map?.get(key) === pos) continue;
    if (!map) latest.set(track.id, (map = new Map()));
    map.set(key, pos);
    changed = true;
  }
  if (!map) return;
  if (map.size > seenKeys.size) {
    for (const key of map.keys()) if (!seenKeys.has(key)) map.delete(key);
    changed = true;
  }
  if (map.size === 0) return;
  seenTracks.add(track.id);
  if (!changed) return;
  bump(latestTicks, track.id);
  tellLive(track.id);
  if (!wait) show(track.id);
  else if (!waiting.has(track.id)) waiting.set(track.id, 0);
}

const differs: Record<StripPart, boolean> = { gain: false, pan: false, sends: false };

/**
 * そのトラックのいちばん新しい位置を、ストリップに見せる。変わったら、読んでいる所と、待たせていた箱に知らせる。
 * turn: canvas を描くコマの 1 回分。パンとセンドは、番のほうだけ見せる。番でないほうが残っていれば true を返す。
 */
function show(track: number, turn = false): boolean {
  const from = latest.get(track);
  let to = shown.get(track);
  differs.gain = differs.pan = differs.sends = false;
  if (from) {
    for (const [key, pos] of from) {
      const part = partOf(key);
      if (part && to?.get(key) !== pos) differs[part] = true;
    }
  }
  if (to) {
    for (const key of to.keys()) if (!from?.has(key)) differs[partOf(key)!] = true;
  }
  if (!differs.gain && !differs.pan && !differs.sends) return false;
  const skip: StripPart | null = turn ? (rowTurn() === "pan" ? "sends" : "pan") : null;
  const left = skip !== null && differs[skip];
  if (left) {
    differs[skip!] = false;
    if (!differs.gain && !differs.pan && !differs.sends) return true;
  }
  if (from) {
    for (const [key, pos] of from) {
      const part = partOf(key);
      if (!part || part === skip || to?.get(key) === pos) continue;
      if (!to) shown.set(track, (to = new Map()));
      to.set(key, pos);
    }
  }
  if (to) {
    for (const key of to.keys()) if (!from?.has(key) && partOf(key) !== skip) to.delete(key);
    if (to.size === 0) shown.delete(track);
  }
  any = true;
  bump(shownTicks, track);
  const told = gates.get(track);
  if (told) for (const part of PARTS) if (differs[part]) for (const tell of told.get(part) ?? []) tell();
  return left;
}

/** 待っていたトラックの、いちばん新しい位置を見せる（そのトラックの箱の上の canvas を描くとき）。 */
export function showTrack(track: number): void {
  paintedAt = frameNo;
  if (!waiting.has(track)) return;
  any = false;
  if (show(track, true)) waiting.set(track, 0);
  else waiting.delete(track);
  if (any) wrote += 1;
}

/**
 * そのトラックを待たせる（箱が canvas の下か、そのとなりにある間）。told は、そのトラックの、その組の見せる位置が変わるたびに呼ぶ。
 * 返す関数でやめる。
 */
export function gateTrack(track: number, part: StripPart, told: () => void): () => void {
  const parts = gates.get(track) ?? new Map<StripPart, Set<() => void>>();
  gates.set(track, parts);
  const set = parts.get(part) ?? new Set();
  parts.set(part, set);
  set.add(told);
  return () => {
    set.delete(told);
    if (set.size === 0) parts.delete(part);
    if (parts.size > 0 || gates.get(track) !== parts) return;
    gates.delete(track);
    if (!waiting.delete(track)) return;
    any = false;
    show(track);
    if (any) wrote += 1;
  };
}

// ミキサーの値の字は毎コマ（仕様書 3 章「オートメーションで動くつまみ」）。ただし帯の見出しの字と同じ書きかえ（同じレイアウト）に入れない:
// 見出しの字とミキサーの行が 1 回のレイアウトにたまると、WebKit の Region がこみ入って「ぜんぶを囲む四角」になる（7 章「レイアウトで
// ためる Region」）。見出しを書いたあとの次の小さな仕事で書き、書いたあとにもレイアウトをすませる（AutoClock の wroteVersion）。

/** トラック → いちばん新しい位置が変わるたびに呼ぶ関数（毎コマ変わる値の字のまわりの箱。coverIf の askLive）。 */
const liveWatchers = new Map<number, Set<() => void>>();
const mixerTicks = new Map<number, Tick>();
const mixerPending = new Set<number>();
function tellLive(track: number): void {
  if (mixerPending.size === 0) queueMicrotask(writeMixer);
  mixerPending.add(track);
}
function writeMixer(): void {
  for (const track of mixerPending) {
    bump(mixerTicks, track);
    const set = liveWatchers.get(track);
    if (set) for (const tell of set) tell();
  }
  mixerPending.clear();
  wrote += 1;
}

/** ミキサーの値の字に出す、いちばん新しい位置（帯の見出しより 1 つあとの書きかえで変わる）。動かしていなければ null。 */
export function mixerLivePosition(track: number, target: AutoTarget): number | null {
  reads(mixerTicks, track);
  return latest.get(track)?.get(targetKey(target)) ?? null;
}

/** そのトラックのいちばん新しい位置が変わるたびに told を呼ぶ。返す関数でやめる。 */
export function watchLive(track: number, told: () => void): () => void {
  const set = liveWatchers.get(track) ?? new Set();
  liveWatchers.set(track, set);
  set.add(told);
  return () => {
    set.delete(told);
    if (set.size === 0 && liveWatchers.get(track) === set) liveWatchers.delete(track);
  };
}

/** 位置を画面に書いた回数（書いたあとに何かするために読む）。 */
export const wroteVersion = () => wrote;

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
