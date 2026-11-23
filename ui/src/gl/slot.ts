// Places where WebGL draws. A component puts a box in the HTML layout and marks it with
// `use:glSlot={...}`, inside a box (or on the same box) marked with `use:glRegion={name}`. The engine
// (engine.ts) puts one canvas into each region's box, as big as the part of the box its slots cover,
// above what the box holds, and draws every slot of the region into it when the picture changes.
// A box inside the region that scrolls its content is marked `data-gl-clip`: the slots in it are drawn
// only where that box shows them (the arrange's track rows, scrolled up and down below the tempo band).

import type { ColorName } from "../design/tokens";
import type { Batch } from "./batch";
import type { RegionName } from "./budget";
import type { Box } from "./paint";
import type { WaveSource } from "./wave";

/** A rectangle of a "rects" slot, in CSS px from the top left of the slot's box. */
export interface GlRect {
  x: number;
  y: number;
  w: number;
  h: number;
  color: ColorName;
}

/**
 * A wave of a "rects" slot: the peaks of an audio file, drawn by the wave shader from a texture (one rectangle for a
 * clip, or for each part of it with a tempo of its own). Places are CSS px from the top left of the slot's box.
 */
export interface GlWave {
  /** Drawn after this many of the slot's rectangles, before the rest. */
  after: number;
  /** The part to paint. From top to bottom it is the whole place of the wave: full scale reaches from `y` to `y + h`. */
  x: number;
  y: number;
  w: number;
  h: number;
  source: WaveSource;
  /** The peak, with its fraction, that sounds at `x`; and peaks to a px. */
  peak: number;
  peaksPerPx: number;
  /** Only the peaks `first .. end` are drawn (the part of the file the take has). */
  first: number;
  end: number;
  /** The clip's volume as a factor. */
  gain: number;
  /** The two ends of the clip and how long its fades are. The bars are counted from `clipX0`. */
  clipX0: number;
  clipX1: number;
  fadeIn: number;
  fadeOut: number;
  /** A bar `bar` wide every `pitch`, never lower than `minH`. */
  pitch: number;
  bar: number;
  minH: number;
  color: ColorName;
  /** The line of silence (a tall row). */
  mid: ColorName | null;
  /** The lines of the fades. */
  line: ColorName | null;
}

/** A line of a "gridLines" slot: the head of a bar, or else of a beat. */
export interface GridLine {
  x: number;
  bar: boolean;
}

/** `ret`: the return strip of send A or B (mixer). `mic`: what comes into a vocal track (`TrackView.id`), before its fader. */
export type MeterSource = "master" | { track: number } | { ret: "a" | "b" } | { mic: number };

/** What a "paint" slot gets besides its box. */
export interface PaintContext {
  dpr: number;
  /** The app's scale (100, 125 or 150). */
  scale: number;
  /** Held keys, as the input meter shows them. */
  isDown: (note: number) => boolean;
}

export type GlSlot =
  /**
   * Input meter keyboard. Notes are MIDI numbers, both ends included. `disabled`: no keyboard, drawn dim.
   * `narrow` (only with size "full"): the black keys of the narrow form, for a box whose white keys are 8 wide.
   */
  | { kind: "keyboard"; size: "full" | "slim"; narrow?: boolean; firstNote: number; lastNote: number; disabled?: boolean }
  /**
   * Paints nothing: the region's canvas covers this box too. For HTML that changes in the same frames
   * as the canvas (the numbers beside a meter): WebKit then repaints one rectangle instead of many
   * small ones that cut each other (docs/architecture.md, chapter 7, "WebKit that paints on the CPU").
   * `stamp`: the canvas is shown in the frame this changes, its picture changed or not (the HTML in the box changed).
   */
  | { kind: "cover"; stamp?: number }
  /** The bar beside the last velocity in the input meter. */
  | { kind: "velocity" }
  /**
   * Two level bars with `gap` CSS px between them: horizontal, left above right; `vertical` (mixer strips),
   * standing, left beside right. `mic`: the mic meter of the input meter (spec 3, 38-B: −60 to −12 the left half, −12 to 0 the
   * right half, ground meter.ground.mic) with a white line at `peakDb`, the peak of the last second (the same number as the dB beside it).
   */
  | { kind: "meter"; source: MeterSource; gap: number; vertical?: boolean; mic?: { peakDb: number } }
  /** The master's sound from low to high (E7): the telemetry's 24 bands as standing bars, one device pixel apart. */
  | { kind: "spectrum" }
  /** Bar lines of the arrange area. `barPx` and `firstBarX` are CSS px. `beats` > 1: beat lines too, that many to a bar. */
  | { kind: "arrangeGrid"; barPx: number; firstBarX: number; beats?: number }
  /**
   * Bar lines (and beat lines when `beats`) of the arrange area at the places given, for a song whose bars are not
   * all as long (meter changes). `x` is CSS px from the left of the slot's box.
   */
  | { kind: "gridLines"; lines: readonly GridLine[]; beats: boolean }
  /** The 40 px arrange overview (clip lines, visible range, play position). */
  | { kind: "overview" }
  /** Any rectangles (clips, the notes in them). Later ones on top; slots later in the page on top of earlier ones. */
  /**
   * `waves`: in the order of their `after`.
   * `alignY`: while this slot is there, the canvas of its region and the canvas of that region have the same top and
   * bottom on the screen (the two together). For a slot drawn every frame beside another region that is: where WebKit
   * paints the page on the CPU, rectangles that start and end at different heights cut each other and the play line
   * into many (docs/architecture.md, chapter 7, "WebKit that paints on the CPU").
   * `shiftX`: the rectangles and waves are drawn this many CSS px to the left (a picture made once, then scrolled).
   */
  | { kind: "rects"; rects: readonly GlRect[]; waves?: readonly GlWave[]; alignY?: RegionName; shiftX?: number }
  /**
   * Painted by its component (the piano roll) from what it holds; the component calls the slot's `update`
   * when that changes. `keys`: paint again when the held keys change.
   */
  | { kind: "paint"; paint: (out: Batch, box: Box, ctx: PaintContext) => void; keys?: boolean };

/** Told about every marked box: the ones already in the page, then each change. */
export interface SlotWatcher {
  attach(node: HTMLElement, slot: GlSlot): void;
  update(node: HTMLElement, slot: GlSlot): void;
  detach(node: HTMLElement): void;
  /** A region box appeared. */
  region?(node: HTMLElement, name: RegionName): void;
  regionGone?(node: HTMLElement): void;
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
  for (const [node, name] of regions) next.region?.(node, name);
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

/**
 * Marks the box whose one canvas draws every slot inside it. The name is a row of REGIONS (budget.ts).
 * The canvas is there for as long as the box is, with or without slots: making a WebGL context stops
 * the page for 50–200 ms, so it is made when the box comes (a band opens), not when a key or a drag
 * first puts a slot into it.
 */
export function glRegion(node: HTMLElement, name: RegionName) {
  regions.set(node, name);
  node.dataset.glRegion = name;
  watcher?.region?.(node, name);
  return {
    destroy() {
      regions.delete(node);
      watcher?.regionGone?.(node);
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
