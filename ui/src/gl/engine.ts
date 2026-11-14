import type { Event, Telemetry } from "../bridge";
import { SCALES, tokensAt, type Scale } from "../design/tokens";
import { Batch } from "./batch";
import { OVERVIEW } from "./dims";
import { HeldKeys } from "./keyboard";
import { Levels } from "./levels";
import { paintArrangeGrid, paintKeyboard, paintMeter, paintOverview, paintVelocity, type Box } from "./paint";
import { Renderer } from "./renderer";
import { liveSlots, type GlSlot } from "./slot";

export interface EngineStats {
  /** Animation frames that ran. */
  frames: number;
  /** Frames that found a change and drew. */
  draws: number;
  /** Time spent reading the slots and building the rectangles, all frames together. */
  buildMs: number;
  /** Time spent in WebGL calls, all draws together. */
  drawMs: number;
  rects: number;
}

/**
 * Owns the canvas. Anything that may change the picture calls `invalidate()`; the next animation
 * frame builds the rectangles again and draws only when they differ from the last drawn ones.
 */
export class Engine {
  readonly stats: EngineStats = { frames: 0, draws: 0, buildMs: 0, drawMs: 0, rects: 0 };
  /** Called at the end of every frame that ran, with the frame's timestamp. */
  onFrame: ((ts: number) => void) | null = null;

  private readonly renderer: Renderer;
  private readonly held = new HeldKeys();
  private readonly levels = new Levels();
  private velocity = 0;
  private scale: Scale = 100;
  private trackNumbers: readonly number[] = [];
  private next = new Batch();
  private drawn = new Batch();
  private mustDraw = true;
  private lost = false;
  private frameId = 0;
  private clipTimer = 0;
  private readonly sizes: ResizeObserver;
  private readonly watched = new WeakSet<Element>();
  private readonly stop: (() => void)[] = [];
  private readonly isDown = (note: number) => this.held.isDown(note);
  private readonly box: Box = { x0: 0, y0: 0, x1: 0, y1: 0 };

  constructor(private readonly canvas: HTMLCanvasElement) {
    this.renderer = new Renderer(canvas);
    const invalidate = () => this.invalidate();

    // A slot's box moves when the page around it changes; these are the ways that can happen.
    const mutations = new MutationObserver(invalidate);
    mutations.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
    this.sizes = new ResizeObserver(invalidate);
    this.sizes.observe(canvas);
    addEventListener("resize", invalidate);
    addEventListener("scroll", invalidate, { capture: true, passive: true });

    const onLost = (e: globalThis.Event) => {
      e.preventDefault();
      this.lost = true;
    };
    const onRestored = () => {
      this.renderer.setUp();
      this.lost = false;
      this.mustDraw = true;
      this.invalidate();
    };
    canvas.addEventListener("webglcontextlost", onLost);
    canvas.addEventListener("webglcontextrestored", onRestored);

    this.stop.push(
      () => mutations.disconnect(),
      () => this.sizes.disconnect(),
      () => removeEventListener("resize", invalidate),
      () => removeEventListener("scroll", invalidate, { capture: true }),
      () => canvas.removeEventListener("webglcontextlost", onLost),
      () => canvas.removeEventListener("webglcontextrestored", onRestored),
    );
    this.invalidate();
  }

  destroy(): void {
    for (const stop of this.stop) stop();
    cancelAnimationFrame(this.frameId);
    clearTimeout(this.clipTimer);
  }

  invalidate(): void {
    if (this.frameId === 0) this.frameId = requestAnimationFrame(this.frame);
  }

  /** The app's scale (100, 125 or 150). */
  setScale(scale: number): void {
    this.scale = (SCALES as readonly number[]).includes(scale) ? (scale as Scale) : 100;
    this.invalidate();
  }

  /** Track numbers in the order of the song, which is the order of the telemetry's track levels. */
  setTracks(numbers: readonly number[]): void {
    this.trackNumbers = numbers;
    this.invalidate();
  }

  onTelemetry(t: Telemetry): void {
    this.held.onTelemetry(t.keys, t.sentUs);
    if (t.lastVelocity !== 0) this.velocity = t.lastVelocity;
    this.levels.onTelemetry(t, performance.now());
    this.invalidate();
  }

  onEvent(e: Event): void {
    if (e.type !== "note") return;
    this.held.onEvent(e.note, e.on, e.tUs);
    if (e.on) this.velocity = e.velocity;
    this.invalidate();
  }

  private readonly frame = (ts: number): void => {
    this.frameId = 0;
    if (this.lost) return;
    const stats = this.stats;
    stats.frames += 1;
    const start = performance.now();
    const falling = this.levels.advance(start);

    const canvas = this.canvas;
    const cssW = canvas.clientWidth;
    const cssH = canvas.clientHeight;
    const dpr = window.devicePixelRatio || 1;
    const width = Math.round(cssW * dpr);
    const height = Math.round(cssH * dpr);
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
      this.mustDraw = true;
    }

    if (width > 0 && height > 0) {
      this.build(start, width / cssW, height / cssH, width, height);
      const built = performance.now();
      stats.buildMs += built - start;
      if (this.mustDraw || !this.next.sameAs(this.drawn)) {
        this.renderer.draw(this.next, width, height);
        [this.next, this.drawn] = [this.drawn, this.next];
        this.mustDraw = false;
        stats.draws += 1;
        stats.rects = this.drawn.rectCount;
        stats.drawMs += performance.now() - built;
      }
    }

    this.onFrame?.(ts);
    if (falling) this.invalidate();
    clearTimeout(this.clipTimer);
    const clipEnd = this.levels.nextClipEnd(start);
    if (clipEnd !== null) this.clipTimer = window.setTimeout(() => this.invalidate(), clipEnd - start + 1);
  };

  private build(nowMs: number, sx: number, sy: number, width: number, height: number): void {
    const out = this.next;
    out.reset();
    const origin = this.canvas.getBoundingClientRect();
    const tokens = tokensAtCached(this.scale);
    const mark = Math.max(1, Math.round(tokens.mark * sx));
    const inset = Math.max(1, Math.round(((OVERVIEW.inset * this.scale) / 100) * sy));
    const box = this.box;

    for (const [node, slot] of liveSlots()) {
      if (!this.watched.has(node)) {
        this.watched.add(node);
        this.sizes.observe(node);
      }
      const r = node.getBoundingClientRect();
      if (r.width <= 0 || r.height <= 0) continue;
      box.x0 = Math.round((r.left - origin.left) * sx);
      box.y0 = Math.round((r.top - origin.top) * sy);
      box.x1 = Math.round((r.right - origin.left) * sx);
      box.y1 = Math.round((r.bottom - origin.top) * sy);
      out.clip(Math.max(box.x0, 0), Math.max(box.y0, 0), Math.min(box.x1, width), Math.min(box.y1, height));
      this.paint(out, box, slot, nowMs, sx, sy, mark, inset);
    }
  }

  private paint(
    out: Batch,
    box: Box,
    slot: GlSlot,
    nowMs: number,
    sx: number,
    sy: number,
    mark: number,
    inset: number,
  ): void {
    switch (slot.kind) {
      case "keyboard":
        paintKeyboard(out, box, slot.size, slot.firstNote, slot.lastNote, this.isDown);
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
          Math.round(slot.gap * sy),
          mark,
          track < 0 ? SILENT : levels.bar(first, nowMs),
          track < 0 ? SILENT : levels.bar(first + 1, nowMs),
        );
        break;
      }
      case "arrangeGrid":
        paintArrangeGrid(out, box, slot.firstBarX * sx, slot.barPx * sx);
        break;
      case "overview":
        paintOverview(out, box, inset);
        break;
    }
  }
}

const SILENT = { length: 0, clipped: false } as const;

const markByScale = new Map<Scale, { mark: number }>();
function tokensAtCached(scale: Scale): { mark: number } {
  let t = markByScale.get(scale);
  if (!t) markByScale.set(scale, (t = { mark: tokensAt(scale, 1)["line.mark"] as number }));
  return t;
}
