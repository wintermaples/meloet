// Tools that let WebKit repaint what automation moves every frame (knobs, value text) in small rectangles
// (docs/architecture.md 7, "WebKit that paints on the CPU", "The Region collected during layout").

import { glSlot } from "../../gl/slot";
import { gateTrack, showTrack, watchLive, type StripPart } from "./autoState.svelte";

/** For a box with no background picture, both look the same. */
const SAME = ["0% 0%", "100% 0%"] as const;

/**
 * Makes WebKit ask for one rectangle of the whole node at the next style calculation (the look does not change. For boxes with no background picture).
 * Small rectangles of knobs and text that moved inside node in the same frame fall into it. If dozens of small rectangles are collected, WebKit merges them into
 * "one rectangle around all" that even includes the top band's numbers, so one large rectangle is asked for first (style rectangles are collected before layout ones).
 */
export function askWhole(node: HTMLElement): void {
  // Switching twice in the same frame goes back to the first value and asks for no rectangle. Only once a frame (forgotten when the frame's work ends).
  if (askedNow.has(node)) return;
  if (askedNow.size === 0) queueMicrotask(() => askedNow.clear());
  askedNow.add(node);
  node.style.backgroundPosition = node.style.backgroundPosition === SAME[0] ? SAME[1] : SAME[0];
}
const askedNow = new Set<HTMLElement>();

/** An empty box placed to fill node (passed to askWhole). node is a box whose position is not static. To reach out left and right, set the CSS variables
 * --ask-left and --ask-right on node; to reach up, --ask-top (a negative length). */
function wholeBox(node: HTMLElement): HTMLElement {
  const whole = document.createElement("span");
  whole.setAttribute("aria-hidden", "true");
  whole.dataset.test = "ask-whole";
  Object.assign(whole.style, { position: "absolute", inset: "var(--ask-top, 0) var(--ask-right, 0) 0 var(--ask-left, 0)", pointerEvents: "none" });
  node.prepend(whole);
  return whole;
}

/**
 * Puts the box under the region's canvas only while track is a number (the canvas is not stretched when nothing moves). Used for faders and value boxes.
 * - What goes under the canvas and what asks for the rectangle is an empty box placed to fill node (the same box, so the asked rectangle lies exactly on the canvas. If node itself
 *   asked, the rectangle would include text that sticks out, and would go outside the canvas).
 * - It draws nothing, but it is called when the WebGL manager draws the canvas above this box (for regions shown in left and right halves, on the turn of the half this box is in),
 *   and there it shows the track's position (showTrack in autoState). Knobs and text change in the same frame as the canvas above.
 * - Each time the fader or value position is shown, it asks for the rectangle of the whole box (askWhole).
 * track: the track where a lane is moving something. Null if nothing is moving.
 */
export function coverIf(node: HTMLElement, track: number | null) {
  let on: { slot: ReturnType<typeof glSlot>; open: () => void; whole: HTMLElement } | null = null;
  let now: number | null = null;
  function set(next: number | null): void {
    if (next === now) return;
    now = next;
    on?.slot.destroy();
    on?.open();
    on?.whole.remove();
    on = null;
    if (next === null) return;
    const whole = wholeBox(node);
    on = { slot: glSlot(whole, { kind: "paint", paint: () => showTrack(next) }), open: gateTrack(next, "gain", () => askWhole(whole)), whole };
  }
  set(track);
  return {
    update: set,
    destroy() {
      set(null);
    },
  };
}

/**
 * A box not under a canvas (the pan row, the send row): each time the position of that group of that track is shown, asks for the rectangle of the whole box.
 * When it is shown is decided by the coverIf box of the same track.
 */
export function askIf(node: HTMLElement, what: { track: number; part: StripPart } | null) {
  let on: { open: () => void; whole: HTMLElement } | null = null;
  let now: string | null = null;
  function set(next: { track: number; part: StripPart } | null): void {
    const key = next ? `${next.track}/${next.part}` : null;
    if (key === now) return;
    now = key;
    on?.open();
    on?.whole.remove();
    on = null;
    if (!next) return;
    const whole = wholeBox(node);
    on = { open: gateTrack(next.track, next.part, () => askWhole(whole)), whole };
  }
  set(what);
  return {
    update: set,
    destroy() {
      set(null);
    },
  };
}

/**
 * A box under a canvas with value text that changes every frame (the fader value): each time the newest position changes, asks for the rectangle of the whole box.
 * Even in a frame where the canvas above it is not shown, it makes one rectangle joined with the boxes of the next strips.
 */
export function askLive(node: HTMLElement, track: number | null) {
  let on: { close: () => void; whole: HTMLElement } | null = null;
  let now: number | null = null;
  function set(next: number | null): void {
    if (next === now) return;
    now = next;
    on?.close();
    on?.whole.remove();
    on = null;
    if (next === null) return;
    const whole = wholeBox(node);
    on = { close: watchLive(next, () => askWhole(whole)), whole };
  }
  set(track);
  return {
    update: set,
    destroy() {
      set(null);
    },
  };
}

/**
 * Tells by tell whether node is visible (not scrolled out sideways). Where there is no IntersectionObserver (tests), it is always treated as visible.
 * Text that changes every frame is written only into visible boxes: mixer strips have content-visibility: auto, so changing text inside a strip out of view makes
 * WebKit lay out the box whose inside it had skipped, and stretch the flex of the whole mixer row again (over 200 ms a frame with 64 tracks).
 */
export function whenSeen(node: HTMLElement, tell: (seen: boolean) => void) {
  if (typeof IntersectionObserver === "undefined") return;
  seenBy ??= new IntersectionObserver((entries) => {
    for (const e of entries) seenTell.get(e.target)?.(e.isIntersecting);
  });
  seenTell.set(node, tell);
  seenBy.observe(node);
  return {
    update(next: (seen: boolean) => void) {
      seenTell.set(node, next);
    },
    destroy() {
      seenTell.delete(node);
      seenBy?.unobserve(node);
    },
  };
}
let seenBy: IntersectionObserver | null = null;
const seenTell = new Map<Element, (seen: boolean) => void>();
