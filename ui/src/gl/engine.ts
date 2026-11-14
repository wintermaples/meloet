import type { Event, Telemetry } from "../bridge";
import { SCALES, tokensAt, type Scale } from "../design/tokens";
import { Batch } from "./batch";
import { OVERVIEW } from "./dims";
import { HeldKeys } from "./keyboard";
import { Levels } from "./levels";
import { paintArrangeGrid, paintKeyboard, paintMeter, paintOverview, paintVelocity, type Box } from "./paint";
import { Renderer } from "./renderer";
import { watchSlots, type GlSlot } from "./slot";

export interface EngineStats {
  /** Animation frames that ran. */
  frames: number;
  /** Canvases drawn (a frame may draw several, or none). */
  draws: number;
}

/** These kinds paint their whole box, so their canvas needs no see-through. */
const OPAQUE: Record<GlSlot["kind"], boolean> = {
  keyboard: true,
  velocity: true,
  meter: false,
  arrangeGrid: false,
  overview: false,
};

/** One slot's canvas. `width` and `height` are device pixels. */
interface Surface {
  node: HTMLElement;
  slot: GlSlot;
  canvas: HTMLCanvasElement;
  renderer: Renderer;
  width: number;
  height: number;
  /** The box may have moved or changed size: measure it again. */
  unplaced: boolean;
  /** The picture may have changed: build its rectangles again. */
  dirty: boolean;
  /** The canvas lost its picture: draw even when the rectangles are the same. */
  mustDraw: boolean;
  lost: boolean;
  drawn: Batch;
  stop(): void;
}

/**
 * Draws every slot into a canvas of its own, put inside the slot's box. Presenting a canvas
 * costs the page's main thread time that grows with its area (and a fixed part per canvas), and
 * keys wait behind it; so a canvas is only as big as its slot, and it is drawn only when its own
 * rectangles differ from the ones it shows.
 */
export class Engine {
  readonly stats: EngineStats = { frames: 0, draws: 0 };
  /** Called at the end of every frame that ran, with the frame's timestamp. */
  onFrame: ((ts: number) => void) | null = null;

  private readonly surfaces = new Map<HTMLElement, Surface>();
  private readonly held = new HeldKeys();
  private readonly levels = new Levels();
  private velocity = 0;
  private scale: Scale = 100;
  private trackNumbers: readonly number[] = [];
  private next = new Batch();
  private failed = false;
  private frameId = 0;
  private clipTimer = 0;
  private readonly sizes = new ResizeObserver((entries) => {
    for (const e of entries) {
      const s = this.surfaces.get(e.target as HTMLElement);
      if (s) s.unplaced = true;
    }
    this.invalidate();
  });
  private readonly stop: (() => void)[] = [];
  private readonly isDown = (note: number) => this.held.isDown(note);
  private readonly box: Box = { x0: 0, y0: 0, x1: 0, y1: 0 };

  constructor() {
    const replace = () => {
      for (const s of this.surfaces.values()) s.unplaced = true;
      this.invalidate();
    };
    addEventListener("resize", replace);
    this.stop.push(
      () => removeEventListener("resize", replace),
      watchSlots({
        attach: (node, slot) => this.attach(node, slot),
        update: (node, slot) => {
          const s = this.surfaces.get(node);
          if (s && OPAQUE[s.slot.kind] !== OPAQUE[slot.kind]) {
            this.detach(node);
            this.attach(node, slot);
          } else if (s) {
            s.slot = slot;
            s.dirty = true;
            this.invalidate();
          }
        },
        detach: (node) => this.detach(node),
      }),
    );
  }

  destroy(): void {
    for (const stop of this.stop) stop();
    for (const node of [...this.surfaces.keys()]) this.detach(node);
    this.sizes.disconnect();
    cancelAnimationFrame(this.frameId);
    clearTimeout(this.clipTimer);
  }

  private attach(node: HTMLElement, slot: GlSlot): void {
    const canvas = document.createElement("canvas");
    canvas.setAttribute("aria-hidden", "true");
    canvas.style.cssText = "position:absolute;pointer-events:none";
    let renderer: Renderer;
    try {
      renderer = new Renderer(canvas, OPAQUE[slot.kind]);
    } catch (e) {
      // No WebGL2: the rest of the screen still works; the slots stay empty.
      if (!this.failed) console.error(e);
      this.failed = true;
      return;
    }
    const surface: Surface = {
      node,
      slot,
      canvas,
      renderer,
      width: 0,
      height: 0,
      unplaced: true,
      dirty: true,
      mustDraw: true,
      lost: false,
      drawn: new Batch(),
      stop: () => {
        canvas.removeEventListener("webglcontextlost", onLost);
        canvas.removeEventListener("webglcontextrestored", onRestored);
      },
    };
    const onLost = (e: globalThis.Event) => {
      e.preventDefault();
      surface.lost = true;
    };
    const onRestored = () => {
      renderer.setUp();
      surface.lost = false;
      surface.mustDraw = true;
      surface.dirty = true;
      this.invalidate();
    };
    canvas.addEventListener("webglcontextlost", onLost);
    canvas.addEventListener("webglcontextrestored", onRestored);
    // The canvas is placed against the slot's own box.
    if (getComputedStyle(node).position === "static") node.style.position = "relative";
    node.appendChild(canvas);
    this.surfaces.set(node, surface);
    this.sizes.observe(node);
    this.invalidate();
  }

  private detach(node: HTMLElement): void {
    const s = this.surfaces.get(node);
    if (!s) return;
    this.surfaces.delete(node);
    this.sizes.unobserve(node);
    s.stop();
    s.canvas.remove();
  }

  invalidate(): void {
    if (this.frameId === 0) this.frameId = requestAnimationFrame(this.frame);
  }

  private touch(kind: GlSlot["kind"] | null): void {
    for (const s of this.surfaces.values()) if (kind === null || s.slot.kind === kind) s.dirty = true;
    this.invalidate();
  }

  /** The app's scale (100, 125 or 150). */
  setScale(scale: number): void {
    this.scale = (SCALES as readonly number[]).includes(scale) ? (scale as Scale) : 100;
    for (const s of this.surfaces.values()) s.unplaced = true;
    this.touch(null);
  }

  /** Track numbers in the order of the song, which is the order of the telemetry's track levels. */
  setTracks(numbers: readonly number[]): void {
    this.trackNumbers = numbers;
    this.touch("meter");
  }

  onTelemetry(t: Telemetry): void {
    if (this.held.onTelemetry(t.keys, t.sentUs)) this.touch("keyboard");
    if (t.lastVelocity !== 0 && t.lastVelocity !== this.velocity) {
      this.velocity = t.lastVelocity;
      this.touch("velocity");
    }
    this.levels.onTelemetry(t, performance.now());
    this.touch("meter");
  }

  onEvent(e: Event): void {
    if (e.type !== "note") return;
    if (this.held.onEvent(e.note, e.on, e.tUs)) this.touch("keyboard");
    if (e.on && e.velocity !== this.velocity) {
      this.velocity = e.velocity;
      this.touch("velocity");
    }
    this.invalidate();
  }

  private readonly frame = (ts: number): void => {
    this.frameId = 0;
    this.stats.frames += 1;
    const now = performance.now();
    const falling = this.levels.advance(now);
    const dpr = window.devicePixelRatio || 1;

    // All the measuring first, then all the writing, so the page is laid out once.
    const moved: [Surface, DOMRect][] = [];
    for (const s of this.surfaces.values()) if (s.unplaced) moved.push([s, s.node.getBoundingClientRect()]);
    for (const [s, r] of moved) this.place(s, r, dpr);

    for (const s of this.surfaces.values()) {
      if (!s.dirty || s.lost || s.width <= 0 || s.height <= 0) continue;
      s.dirty = false;
      const out = this.next;
      out.reset();
      out.clip(0, 0, s.width, s.height);
      this.paint(out, s, now, dpr);
      if (!s.mustDraw && out.sameAs(s.drawn)) continue;
      s.renderer.draw(out, s.width, s.height);
      [this.next, s.drawn] = [s.drawn, out];
      s.mustDraw = false;
      this.stats.draws += 1;
    }

    this.onFrame?.(ts);
    if (falling) this.touch("meter");
    clearTimeout(this.clipTimer);
    const clipEnd = this.levels.nextClipEnd(now);
    if (clipEnd !== null) this.clipTimer = window.setTimeout(() => this.touch("meter"), clipEnd - now + 1);
  };

  /**
   * Sizes the canvas to the slot's box, with every edge moved to the nearest device pixel
   * (a box may start between two pixels), so one canvas pixel is one pixel of the screen.
   */
  private place(s: Surface, r: DOMRect, dpr: number): void {
    s.unplaced = false;
    const x0 = Math.round(r.left * dpr);
    const y0 = Math.round(r.top * dpr);
    const width = Math.max(0, Math.round(r.right * dpr) - x0);
    const height = Math.max(0, Math.round(r.bottom * dpr) - y0);
    const style = s.canvas.style;
    style.left = `${x0 / dpr - r.left - s.node.clientLeft}px`;
    style.top = `${y0 / dpr - r.top - s.node.clientTop}px`;
    style.width = `${width / dpr}px`;
    style.height = `${height / dpr}px`;
    if (width !== s.width || height !== s.height) {
      s.width = s.canvas.width = width;
      s.height = s.canvas.height = height;
      s.mustDraw = true;
    }
    s.dirty = true;
  }

  private paint(out: Batch, s: Surface, nowMs: number, dpr: number): void {
    const slot = s.slot;
    const box = this.box;
    box.x1 = s.width;
    box.y1 = s.height;
    switch (slot.kind) {
      case "keyboard":
        paintKeyboard(out, box, slot.size, slot.firstNote, slot.lastNote, this.isDown, slot.disabled === true);
        break;
      case "velocity":
        paintVelocity(out, box, this.velocity);
        break;
      case "meter": {
        // A track the song does not have reads as silence.
        const track = slot.source === "master" ? 0 : this.trackNumbers.indexOf(slot.source.track);
        const first = slot.source === "master" ? 0 : 2 + 2 * track;
        const levels = this.levels;
        paintMeter(
          out,
          box,
          Math.round(slot.gap * dpr),
          Math.max(1, Math.round(markAt(this.scale) * dpr)),
          track < 0 ? SILENT : levels.bar(first, nowMs),
          track < 0 ? SILENT : levels.bar(first + 1, nowMs),
        );
        break;
      }
      case "arrangeGrid":
        paintArrangeGrid(out, box, slot.firstBarX * dpr, slot.barPx * dpr);
        break;
      case "overview":
        paintOverview(out, box, Math.max(1, Math.round(((OVERVIEW.inset * this.scale) / 100) * dpr)));
        break;
    }
  }
}

const SILENT = { length: 0, clipped: false } as const;

const marks = new Map<Scale, number>();
function markAt(scale: Scale): number {
  let mark = marks.get(scale);
  if (mark === undefined) marks.set(scale, (mark = tokensAt(scale, 1)["line.mark"] as number));
  return mark;
}
