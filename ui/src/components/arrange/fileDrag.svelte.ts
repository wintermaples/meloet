// The pointer place while audio files are dragged over the window (fileDrag / fileDrop of protocol 24; docs/architecture.md 22-17).
// Rust owns the paths. The screen only decides where they are dropped: from the place it finds the track, the tick and the 30-C side, and fileDrop sends one DropAudio.
// Page CSS px. The arrange part (DropFrame) registers how the target is found (with no arrange, tick 0 of a new vocal track).

import type { DropMode, Event } from "../../bridge";
import { current, onEvent, send } from "../../state/app.svelte";
import { dropModeAt } from "./audioClipGeom";

/** Where to drop: a vocal track (null is a new vocal track) and the tick snapped to a beat. */
export interface DropPlace {
  track: number | null;
  tick: number;
}

export const fileDrag = $state<{ at: { x: number; y: number } | null }>({ at: null });

type Resolver = (pageX: number, pageY: number, mode: DropMode) => DropPlace;
let resolver: Resolver | null = null;

/** The arrange part registers page place → drop target. Call the returned function to remove it. */
export function setDropResolver(next: Resolver): () => void {
  resolver = next;
  return () => {
    if (resolver === next) resolver = null;
  };
}

/** Where a drop at this page place goes (null with no arrange). */
export function dropPlaceAt(pageX: number, pageY: number, mode: DropMode): DropPlace | null {
  return resolver?.(pageX, pageY, mode) ?? null;
}

/** With two or more audio files, the left and right halves of the window split as in 30-C. With one, it goes to the track where it is dropped (spec 3 (29-A)). */
export function dropModeFor(audio: number, pageX: number, windowWidth: number): DropMode {
  return audio >= 2 ? dropModeAt(pageX, windowWidth) : "inRow";
}

/** The number of audio files last dragged in (when dropped, the notice box may already be closed). */
let lastAudio = 1;

export function heardFileEvent(e: Event): void {
  if (e.type === "fileDrag") {
    fileDrag.at = { x: e.x, y: e.y };
    lastAudio = current.state?.workspace.fileDrop?.audio ?? lastAudio;
    return;
  }
  if (e.type !== "fileDrop") return;
  fileDrag.at = null;
  const audio = current.state?.workspace.fileDrop?.audio ?? lastAudio;
  const mode = dropModeFor(audio, e.x, window.innerWidth);
  const place = resolver?.(e.x, e.y, mode) ?? { track: null, tick: 0 };
  void send({ type: "dropAudio", drop: e.drop, track: place.track, tick: place.tick, mode });
}

onEvent(heardFileEvent);
