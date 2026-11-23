// The points of automation lanes, and the current value on the line (docs/architecture.md 21-8).
// LaneStore (ui/src/bridge/lane.ts) fetches the points only once per laneId. This keeps a counter for redrawing when they arrive, and
// the current position of what lanes move (gain, pan, send, mute, parameters of built-in effects).
// AutoClock writes the position: when stopped, when the state changes; when moving, every frame of the WebGL manager.
// It is written only when the value changes, so only the places that read it (mixer, lane headings, params panel) move.

import { clockOffsetUs, lanePoints, peekLanePoints, type AppState, type AutoTarget, type LanePoints, type TrackView } from "../../bridge";
import { current } from "../../state/app.svelte";
import { shownTick } from "../time";
import { laneMoves, targetKey, valueAt } from "./laneValue";

let version = $state(0);
const RETRY_MS = 100;
const asking = new Set<number>();

/** The lane's points. If not here yet, they are fetched, and on arrival the places that read this redraw. */
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
    // Before connecting (the first state comes a little before the connection is complete). A little later, the readers are made to fetch again.
    asking.delete(laneId);
    setTimeout(() => (version += 1), RETRY_MS);
  }
  return undefined;
}

// ---- Current position ----

// Positions are kept in a plain Map, and when one changes, the per-track counter goes up by 1 (readers read again by this counter). It is per track, so
// even in a song with 64 tracks, only the places showing the moved track read again. The writer makes the counter ($state) first: a $state first
// made by a reader (inside a $derived) does not become a dependency of that $derived. A reader that read a counter that did not exist yet reads again by made.
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

/** The newest position on the line (track → what (targetKey) → position). Read by lane headings, the params panel and the mixer's value text. */
const latest = new Map<number, Map<string, number>>();
const latestTicks = new Map<number, Tick>();
/** The position shown in mixer strips. For waiting tracks, it lags behind latest. */
const shown = new Map<number, Map<string, number>>();
const shownTicks = new Map<number, Tick>();
/** How many times positions were written to the screen (read by places that do something after writing). */
let wrote = $state(0);
let any = false;

// A mixer strip moves only in a frame where the canvas above it is shown (docs/architecture.md 7, "WebKit that paints on the CPU": things that change in the same frame go
// under one canvas). The mixer's canvas shows its left and right halves in turns, one per frame, so moving the knobs and text of the half not shown in the same frame
// makes more than 11 dirty rectangles and repaints the whole window.
/** A group of things that lanes move in a strip. */
export type StripPart = "gain" | "pan" | "sends";
const PARTS: readonly StripPart[] = ["gain", "pan", "sends"];
/** Null for things a strip does not show (effect and instrument parameters). */
function partOf(key: string): StripPart | null {
  if (key === "volume" || key === "mute") return "gain";
  if (key === "pan") return "pan";
  return key.startsWith("send:") ? "sends" : null;
}
/** Waiting track → per group, the functions to call when the position is shown (coverIf, askIf). */
const gates = new Map<number, Map<StripPart, Set<() => void>>>();
/**
 * For a waiting track, pan and send are shown one at a time: the pan row and the send row are outside the canvas, so adding both rectangles in the same frame
 * brings the area repainted in one frame close to the 25 % rule. Whose turn it is is the same for every track (it switches every 2 frames. The canvas shows its left and right halves
 * in turns, one per frame, so each half gets the pan turn and the send turn in turns). If they were not lined up, the row rectangles would be scattered per strip.
 */
const rowTurn = (): StripPart => ((frameNo >> 1) & 1 ? "pan" : "sends");
/** Waiting tracks whose newest position is not shown yet → the number of frames waited. */
const waiting = new Map<number, number>();
/**
 * The most frames to wait while the canvas above is not drawn. In frames with no meter message the canvas is not drawn, so it waits for the next turn.
 * Showing without waiting would move the knobs of both halves in the same frame and repaint the whole window.
 * A track whose turn does not come while the other tracks' canvases are drawn is not visible (scrolled out sideways), so it waits until it is visible.
 */
export const MAX_WAIT_FRAMES = 3;
let frameNo = 0;
/** The frame in which the canvas above any box was last drawn. */
let paintedAt = -Infinity;

/** The current position (0 to 1) on the line of what a lane moves. Null if it is not moving it (it stays at the value set by hand). */
export function livePosition(track: number, target: AutoTarget): number | null {
  reads(latestTicks, track);
  return latest.get(track)?.get(targetKey(target)) ?? null;
}

/** The position shown in a mixer strip (for waiting tracks, the position in the frame where the canvas above it was drawn). */
export function shownPosition(track: number, target: AutoTarget): number | null {
  reads(shownTicks, track);
  return shown.get(track)?.get(targetKey(target)) ?? null;
}

/** How finely positions are written (1/4096 of a knob's length). Not written if the same. */
const STEP = 1 / 4096;

const seenTracks = new Set<number>();
const seenKeys = new Set<string>();
/** Lane index → what (so text is not made every frame). */
const laneKeys = new Map<number, string>();

/**
 * Writes the position on the line at tick. Lanes that are not moving anything, and lanes that are gone, get null.
 * inFrame: inside a frame of the WebGL manager. The position of a waiting track is shown when the canvas above it is drawn (showTrack).
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
 * Shows the track's newest position in its strip. If it changed, tells the readers and the boxes that were kept waiting.
 * turn: one turn of drawing the canvas. Of pan and send, only the one whose turn it is is shown. Returns true if the other one is left.
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

/** Shows the newest position of a waiting track (when the canvas above that track's box is drawn). */
export function showTrack(track: number): void {
  paintedAt = frameNo;
  if (!waiting.has(track)) return;
  any = false;
  if (show(track, true)) waiting.set(track, 0);
  else waiting.delete(track);
  if (any) wrote += 1;
}

/**
 * Makes the track wait (while its box is under the canvas or next to it). told is called each time the shown position of that group of that track changes.
 * Stop with the returned function.
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

// The mixer's value text changes every frame (spec 3, "Knobs moved by automation"). But it is not put in the same rewrite (same layout) as the lane heading text:
// if the heading text and the mixer row are collected in one layout, WebKit's Region gets complex and becomes "one rectangle around all" (chapter 7, "The Region collected
// during layout"). It is written in the next small task after the heading is written, and layout is finished after writing too (AutoClock's wroteVersion).

/** Track → functions to call each time the newest position changes (the boxes around value text that changes every frame; askLive in coverIf). */
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

/** The newest position for the mixer's value text (it changes one rewrite after the lane heading). Null if not moving. */
export function mixerLivePosition(track: number, target: AutoTarget): number | null {
  reads(mixerTicks, track);
  return latest.get(track)?.get(targetKey(target)) ?? null;
}

/** Calls told each time the track's newest position changes. Stop with the returned function. */
export function watchLive(track: number, told: () => void): () => void {
  const set = liveWatchers.get(track) ?? new Set();
  liveWatchers.set(track, set);
  set.add(told);
  return () => {
    set.delete(told);
    if (set.size === 0 && liveWatchers.get(track) === set) liveWatchers.delete(track);
  };
}

/** How many times positions were written to the screen (read to do something after writing). */
export const wroteVersion = () => wrote;

/** Whether any lane exists (whether to put out values per frame). */
export const anyLane = (state: AppState) => state.song.tracks.some((t) => t.lanes.length > 0);

// ---- The play line position ----

let offsetUs: number | null = null;
let askingOffset = false;

/** Where the play line is (the same as the transport numbers. docs: shownTick in components/time.ts). */
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

/** How many times points arrived (read as the cue to redraw). */
export const pointsVersion = () => version;
