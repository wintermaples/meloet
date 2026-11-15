import { CONTEXT_BUDGET, WEBKIT_CONTEXT_LIMIT, fault } from "./budget";
import { Renderer } from "./renderer";

/** A canvas and its WebGL context. Made once, then handed from region to region (see budget.ts). */
export interface GlCanvas {
  readonly canvas: HTMLCanvasElement;
  readonly renderer: Renderer;
  readonly opaque: boolean;
  lost: boolean;
  /** Not drawn yet since it was made (for the frame profile). */
  fresh: boolean;
  /** Set by the canvas's user: the context came back and the picture must be drawn again. */
  onRestored: (() => void) | null;
}

const free: GlCanvas[] = [];
let made = 0;
let failed = false;

/** Contexts made in this page so far. */
export function contextsMade(): number {
  return made;
}

/** A canvas not in use, or a new one. Null when WebGL2 is not there (told once in the console). */
export function takeCanvas(opaque: boolean, purpose: string): GlCanvas | null {
  const i = free.findIndex((c) => c.opaque === opaque && !c.lost);
  if (i >= 0) return free.splice(i, 1)[0]!;
  if (failed) return null;
  if (made >= CONTEXT_BUDGET) {
    fault(
      `gl: "${purpose}" needs WebGL context ${made + 1}, over the budget of ${CONTEXT_BUDGET} ` +
        `(WebKitGTK loses the oldest past ${WEBKIT_CONTEXT_LIMIT}). See ui/src/gl/budget.ts.`,
    );
  }
  const canvas = document.createElement("canvas");
  canvas.setAttribute("aria-hidden", "true");
  canvas.style.cssText = "position:absolute;pointer-events:none";
  let renderer: Renderer;
  try {
    renderer = new Renderer(canvas, opaque);
  } catch (e) {
    // No WebGL2: the rest of the screen still works; the slots stay empty.
    console.error(e);
    failed = true;
    return null;
  }
  made += 1;
  const c: GlCanvas = { canvas, renderer, opaque, lost: false, fresh: true, onRestored: null };
  canvas.addEventListener("webglcontextlost", (e) => {
    e.preventDefault();
    c.lost = true;
  });
  canvas.addEventListener("webglcontextrestored", () => {
    renderer.setUp();
    c.lost = false;
    c.onRestored?.();
  });
  return c;
}

/** Takes the canvas out of the page and keeps it for the next region that needs one. */
export function giveBack(c: GlCanvas): void {
  c.canvas.remove();
  c.onRestored = null;
  free.push(c);
}

/** For tests: forget every canvas, as a new page would. */
export function resetPool(): void {
  free.length = 0;
  made = 0;
  failed = false;
}
