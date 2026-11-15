// Places where WebGL draws. A component puts a box in the HTML layout and marks it with
// `use:glSlot={...}`, inside a box (or on the same box) marked with `use:glRegion={name}`. The engine
// (engine.ts) puts one canvas into each region's box, as big as the part of the box its slots cover,
// above what the box holds, and draws every slot of the region into it when the picture changes.

import type { ColorName } from "../design/tokens";
import type { RegionName } from "./budget";

/** A rectangle of a "rects" slot, in CSS px from the top left of the slot's box. */
export interface GlRect {
  x: number;
  y: number;
  w: number;
  h: number;
  color: ColorName;
}

export type MeterSource = "master" | { track: number };

export type GlSlot =
  /**
   * Input meter keyboard. Notes are MIDI numbers, both ends included. `disabled`: no keyboard, drawn dim.
   * `narrow` (only with size "full"): the black keys of the narrow form, for a box whose white keys are 8 wide.
   */
  | { kind: "keyboard"; size: "full" | "slim"; narrow?: boolean; firstNote: number; lastNote: number; disabled?: boolean }
  /** The bar beside the last velocity in the input meter. */
  | { kind: "velocity" }
  /** Two horizontal level bars (left above right) with `gap` CSS px between them. */
  | { kind: "meter"; source: MeterSource; gap: number }
  /** Bar lines of the arrange area. `barPx` and `firstBarX` are CSS px. */
  | { kind: "arrangeGrid"; barPx: number; firstBarX: number }
  /** The 40 px arrange overview (clip lines, visible range, play position). */
  | { kind: "overview" }
  /** Any rectangles (clips, the notes in them). Later ones on top; slots later in the page on top of earlier ones. */
  | { kind: "rects"; rects: readonly GlRect[] };

/** Told about every marked box: the ones already in the page, then each change. */
export interface SlotWatcher {
  attach(node: HTMLElement, slot: GlSlot): void;
  update(node: HTMLElement, slot: GlSlot): void;
  detach(node: HTMLElement): void;
}

const slots = new Map<HTMLElement, GlSlot>();
const regions = new Map<HTMLElement, RegionName>();
let watcher: SlotWatcher | null = null;

export function glSlot(node: HTMLElement, slot: GlSlot) {
  slots.set(node, slot);
  node.dataset.glSlot = slot.kind;
  watcher?.attach(node, slot);
  return {
    update(next: GlSlot) {
      slots.set(node, next);
      node.dataset.glSlot = next.kind;
      watcher?.update(node, next);
    },
    destroy() {
      slots.delete(node);
      watcher?.detach(node);
    },
  };
}

/** One watcher at a time (the engine). Returns how to stop watching. */
export function watchSlots(next: SlotWatcher): () => void {
  watcher = next;
  for (const [node, slot] of slots) next.attach(node, slot);
  return () => {
    if (watcher === next) watcher = null;
  };
}

const frameCallbacks = new Set<() => void>();
let wake: (() => void) | null = null;

/**
 * Runs `callback` at the start of every frame of the engine, before it measures and draws, for as
 * long as it is registered. Things that move every frame (the play line, the clip being recorded)
 * write their boxes and call their slot's `update` here, so the picture and the boxes change in the
 * same frame. Returns how to stop.
 */
export function everyGlFrame(callback: () => void): () => void {
  frameCallbacks.add(callback);
  wake?.();
  return () => frameCallbacks.delete(callback);
}

/** For the engine: `next` is called when a callback is added. */
export function watchFrames(next: () => void): () => void {
  wake = next;
  return () => {
    if (wake === next) wake = null;
  };
}

/** For the engine: runs the callbacks; true if there are any (so the next frame is needed too). */
export function runFrameCallbacks(): boolean {
  for (const callback of frameCallbacks) callback();
  return frameCallbacks.size > 0;
}

/** Marks the box whose one canvas draws every slot inside it. The name is a row of REGIONS (budget.ts). */
export function glRegion(node: HTMLElement, name: RegionName) {
  regions.set(node, name);
  node.dataset.glRegion = name;
  return {
    destroy() {
      regions.delete(node);
    },
  };
}

/** The nearest region box around a slot's box (or the box itself). */
export function regionOf(node: HTMLElement): { host: HTMLElement; name: RegionName } | null {
  for (let n: HTMLElement | null = node; n; n = n.parentElement) {
    const name = regions.get(n);
    if (name) return { host: n, name };
  }
  return null;
}
