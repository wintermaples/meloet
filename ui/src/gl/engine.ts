import type { Event, Telemetry } from "../bridge";
import { SCALES, type Scale } from "../design/tokens";
import { Batch } from "./batch";
import { REGIONS, SHOWN_WITH, fault, type RegionName } from "./budget";
import { OVERVIEW } from "./dims";
import { HeldKeys } from "./keyboard";
import { Levels, MASTER_CHANNEL, RETURN_A_CHANNEL, RETURN_B_CHANNEL, trackChannel } from "./levels";
import {
  blackKeyPx,
  clipMarkPx,
  paintArrangeGrid,
  paintKeyboard,
  paintMeter,
  paintOverview,
  paintRects,
  paintVelocity,
  type Box,
} from "./paint";
import { giveBack, takeCanvas, type GlCanvas } from "./pool";
import type { FrameProfile, FrameRecord, MadeRecord } from "./profile";
import { regionOf, runFrameCallbacks, watchFrames, watchSlots, type GlSlot, type MeterSource } from "./slot";

export interface EngineStats {
  /** Animation frames that ran. */
  frames: number;
  /** Canvases drawn (a frame may draw several, or none). */
  draws: number;
}

interface Slot {
  node: HTMLElement;
  slot: GlSlot;
  region: Region | null;
  /** The box in device pixels of the window, measured when its region is drawn. */
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** The one canvas of a region box, covering the part of the box (and the window) that its slots cover. */
interface Region {
  host: HTMLElement;
  name: RegionName;
  gl: GlCanvas;
  /** In page order: later slots draw over earlier ones. */
  slots: Slot[];
  /** The canvas in device pixels of the window. */
  x0: number;
  y0: number;
  width: number;
  height: number;
  /** Measure, place and build the rectangles again; draw if they differ from the ones shown. */
  dirty: boolean;
  /** The canvas lost its picture: draw even when the rectangles are the same. */
  mustDraw: boolean;
  drawn: Batch;
  /** The profile's record of the context made for this region, until its first draw. */
  made: MadeRecord | null;
}

/**
 * Draws every slot into its region's canvas. Presenting a canvas costs the page's main thread time
 * that grows with its area, plus a fixed part per canvas, and keys wait behind it; so one canvas
 * serves a whole region (one fixed part), it covers only where the region's slots are (small area),
 * and it is drawn only when its rectangles differ from the ones it shows.
 */
export class Engine {
  readonly stats: EngineStats = { frames: 0, draws: 0 };
  /** Called at the end of every frame that ran, with the frame's timestamp. */
  onFrame: ((ts: number) => void) | null = null;
  /** Development only: where each frame's time goes (profile.ts). */
  profile: FrameProfile | null = null;

  private readonly slots = new Map<HTMLElement, Slot>();
  private readonly regions = new Map<HTMLElement, Region>();
  /** Slots that have not found their region yet (the region's mark may come after the slot's). */
  private readonly pending = new Set<Slot>();
  /** Region boxes that have not got their canvas yet. */
  private readonly pendingRegions = new Map<HTMLElement, RegionName>();
  private readonly held = new HeldKeys();
  private readonly levels = new Levels();
  private velocity = 0;
  private scale: Scale = 100;
  private trackNumbers: readonly number[] = [];
  private next = new Batch();
  private frameId = 0;
  private clipTimer = 0;
  private readonly again = () => {
    for (const r of this.regions.values()) r.dirty = true;
    this.invalidate();
  };
  private readonly sizes = new ResizeObserver(this.again);
  private readonly stop: (() => void)[] = [];
  private readonly isDown = (note: number) => this.held.isDown(note);
  private readonly box: Box = { x0: 0, y0: 0, x1: 0, y1: 0 };

  constructor() {
    const scrolled = { capture: true, passive: true } as const;
    addEventListener("resize", this.again);
    // A scroll moves slots without changing any size.
    document.addEventListener("scroll", this.again, scrolled);
    this.stop.push(
      () => removeEventListener("resize", this.again),
      () => document.removeEventListener("scroll", this.again, scrolled),
      watchFrames(() => this.invalidate()),
      watchSlots({
        attach: (node, slot) => this.attach(node, slot),
        update: (node, slot) => {
          const s = this.slots.get(node);
          if (!s) return;
          s.slot = slot;
          if (s.region) s.region.dirty = true;
          this.invalidate();
        },
        detach: (node) => this.detach(node),
        region: (node, name) => {
          this.pendingRegions.set(node, name);
          this.invalidate();
        },
        regionGone: (node) => {
          this.pendingRegions.delete(node);
          const r = this.regions.get(node);
          if (r) this.close(r);
        },
      }),
    );
  }

  destroy(): void {
    for (const stop of this.stop) stop();
    for (const node of [...this.slots.keys()]) this.detach(node);
    for (const r of [...this.regions.values()]) this.close(r);
    this.pendingRegions.clear();
    this.sizes.disconnect();
    cancelAnimationFrame(this.frameId);
    clearTimeout(this.clipTimer);
  }

  private attach(node: HTMLElement, slot: GlSlot): void {
    const s: Slot = { node, slot, region: null, x0: 0, y0: 0, x1: 0, y1: 0 };
    this.slots.set(node, s);
    this.pending.add(s);
    this.sizes.observe(node);
    this.invalidate();
  }

  private detach(node: HTMLElement): void {
    const s = this.slots.get(node);
    if (!s) return;
    this.slots.delete(node);
    this.pending.delete(s);
    if (!this.regions.has(node)) this.sizes.unobserve(node);
    const r = s.region;
    if (!r) return;
    r.slots.splice(r.slots.indexOf(s), 1);
    r.dirty = true;
    this.invalidate();
  }

  /** The region box went away: its canvas goes back to the pool. */
  private close(r: Region): void {
    this.regions.delete(r.host);
    if (!this.slots.has(r.host)) this.sizes.unobserve(r.host);
    giveBack(r.gl);
    for (const s of r.slots) {
      s.region = null;
      this.pending.add(s);
    }
  }

  /** Opens each new region box (taking a canvas), then puts each new slot into its region. */
  private resolve(): void {
    for (const [host, name] of this.pendingRegions) {
      this.pendingRegions.delete(host);
      if (!this.regions.has(host)) this.open(host, name);
    }
    for (const s of this.pending) {
      this.pending.delete(s);
      const found = regionOf(s.node);
      if (!found) {
        fault(`gl: a "${s.slot.kind}" slot is not inside any glRegion box`);
        continue;
      }
      const r = this.regions.get(found.host) ?? this.open(found.host, found.name);
      if (!r) continue;
      const after = r.slots.findIndex((o) => o.node.compareDocumentPosition(s.node) & Node.DOCUMENT_POSITION_PRECEDING);
      r.slots.splice(after < 0 ? r.slots.length : after, 0, s);
      s.region = r;
      r.dirty = true;
    }
  }

  private open(host: HTMLElement, name: RegionName): Region | null {
    const gl = takeCanvas(REGIONS[name].opaque, name);
    if (!gl) return null;
    const r: Region = { host, name, gl, slots: [], x0: 0, y0: 0, width: 0, height: 0, dirty: true, mustDraw: true, drawn: new Batch(), made: null };
    if (gl.fresh && this.profile) {
      const m = gl.renderer.madeMs;
      r.made = { region: name, contextMs: m.context, programMs: m.program, firstDrawMs: null };
      this.profile.made(r.made);
    }
    gl.fresh = false;
    gl.onRestored = () => {
      r.mustDraw = true;
      r.dirty = true;
      this.invalidate();
    };
    // The canvas is placed against the region's own box.
    if (getComputedStyle(host).position === "static") host.style.position = "relative";
    host.appendChild(gl.canvas);
    this.regions.set(host, r);
    this.sizes.observe(host);
    return r;
  }

  invalidate(): void {
    if (this.frameId === 0) this.frameId = requestAnimationFrame(this.frame);
  }

  private touch(kind: GlSlot["kind"] | null): void {
    for (const s of this.slots.values()) {
      if (!s.region) continue;
      const slot = s.slot;
      if (kind === null || slot.kind === kind || (kind === "keyboard" && slot.kind === "paint" && slot.keys)) s.region.dirty = true;
    }
    this.invalidate();
  }

  /** The app's scale (100, 125 or 150). */
  setScale(scale: number): void {
    this.scale = (SCALES as readonly number[]).includes(scale) ? (scale as Scale) : 100;
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
    const p = this.profile?.begin() ?? null;
    const moving = runFrameCallbacks();
    const t = p ? performance.now() : 0;
    if (p) p.callbacksMs = t - p.atMs;
    this.resolve();

    // All the measuring first, then all the writing, so the page is laid out once.
    const due: [Region, DOMRect][] = [];
    for (const r of this.regions.values()) {
      if (!r.dirty) continue;
      r.dirty = false;
      due.push([r, r.host.getBoundingClientRect()]);
      for (const s of r.slots) measure(s, dpr);
    }
    for (const [r, host] of due) this.place(r, host, dpr);
    if (p) p.measureMs = performance.now() - t;
    const shown = new Set<RegionName>();
    for (const [r] of due) if (this.draw(r, now, dpr, p)) shown.add(r.name);
    for (const name of shown) {
      const partner = SHOWN_WITH[name];
      if (!partner || shown.has(partner)) continue;
      for (const r of this.regions.values()) {
        if (r.name !== partner) continue;
        r.mustDraw = true;
        if (this.draw(r, now, dpr, p)) shown.add(partner);
      }
    }

    this.onFrame?.(ts);
    if (falling) this.touch("meter");
    if (moving) this.invalidate();
    clearTimeout(this.clipTimer);
    const clipEnd = this.levels.nextClipEnd(now);
    if (clipEnd !== null) this.clipTimer = window.setTimeout(() => this.touch("meter"), clipEnd - now + 1);
  };

  /**
   * Sizes the canvas to what its slots cover inside the region box and the window. Every edge is
   * on a device pixel (a box may start between two), so one canvas pixel is one pixel of the screen.
   */
  private place(r: Region, host: DOMRect, dpr: number): void {
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (const s of r.slots) {
      if (s.x1 <= s.x0 || s.y1 <= s.y0) continue;
      x0 = Math.min(x0, s.x0);
      y0 = Math.min(y0, s.y0);
      x1 = Math.max(x1, s.x1);
      y1 = Math.max(y1, s.y1);
    }
    x0 = Math.max(x0, Math.round(host.left * dpr), 0);
    y0 = Math.max(y0, Math.round(host.top * dpr), 0);
    x1 = Math.min(x1, Math.round(host.right * dpr), Math.round(innerWidth * dpr));
    y1 = Math.min(y1, Math.round(host.bottom * dpr), Math.round(innerHeight * dpr));
    let width = x1 - x0;
    let height = y1 - y0;
    if (!(width > 0 && height > 0)) x0 = y0 = width = height = 0;

    const canvas = r.gl.canvas;
    const h = r.host;
    const style = canvas.style;
    style.left = `${x0 / dpr - host.left - h.clientLeft + h.scrollLeft}px`;
    style.top = `${y0 / dpr - host.top - h.clientTop + h.scrollTop}px`;
    style.width = `${width / dpr}px`;
    style.height = `${height / dpr}px`;
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
      r.mustDraw = true;
    }
    r.x0 = x0;
    r.y0 = y0;
    r.width = width;
    r.height = height;
  }

  /** True when the canvas was shown: its rectangles differ from the ones it showed, or it had to be drawn. */
  private draw(r: Region, nowMs: number, dpr: number, p: FrameRecord | null): boolean {
    if (r.gl.lost || r.width <= 0 || r.height <= 0) return false;
    const t0 = p ? performance.now() : 0;
    const out = this.next;
    out.reset();
    const box = this.box;
    for (const s of r.slots) {
      box.x0 = s.x0 - r.x0;
      box.y0 = s.y0 - r.y0;
      box.x1 = s.x1 - r.x0;
      box.y1 = s.y1 - r.y0;
      const cx0 = Math.max(box.x0, 0);
      const cy0 = Math.max(box.y0, 0);
      const cx1 = Math.min(box.x1, r.width);
      const cy1 = Math.min(box.y1, r.height);
      if (cx1 <= cx0 || cy1 <= cy0) continue;
      out.clip(cx0, cy0, cx1, cy1);
      this.paint(out, s.slot, box, nowMs, dpr);
    }
    const same = !r.mustDraw && out.sameAs(r.drawn);
    const t1 = p ? performance.now() : 0;
    if (p) p.paintMs += t1 - t0;
    if (same) return false;
    r.gl.renderer.draw(out, r.width, r.height);
    [this.next, r.drawn] = [r.drawn, out];
    r.mustDraw = false;
    this.stats.draws += 1;
    if (p) {
      const t2 = performance.now();
      p.glMs += t2 - t1;
      if (r.made) {
        r.made.firstDrawMs = t2 - t1;
        r.made = null;
      }
      this.profile!.drew(r.name, r.drawn.rectCount);
    }
    return true;
  }

  /** The left channel of a meter's source; -1 for a track the song does not have (it reads as silence). */
  private meterChannel(source: MeterSource): number {
    if (source === "master") return MASTER_CHANNEL;
    if ("ret" in source) return source.ret === "a" ? RETURN_A_CHANNEL : RETURN_B_CHANNEL;
    const i = this.trackNumbers.indexOf(source.track);
    return i < 0 ? -1 : trackChannel(i);
  }

  private paint(out: Batch, slot: GlSlot, box: Box, nowMs: number, dpr: number): void {
    switch (slot.kind) {
      case "keyboard": {
        const form = slot.size === "slim" ? "slim" : slot.narrow ? "narrow" : "full";
        const black = blackKeyPx(form, this.scale, dpr);
        paintKeyboard(out, box, black, slot.firstNote, slot.lastNote, this.isDown, slot.disabled === true);
        break;
      }
      case "cover":
        break;
      case "velocity":
        paintVelocity(out, box, this.velocity);
        break;
      case "meter": {
        const first = this.meterChannel(slot.source);
        const levels = this.levels;
        paintMeter(
          out,
          box,
          Math.round(slot.gap * dpr),
          clipMarkPx(this.scale, dpr),
          first < 0 ? SILENT : levels.bar(first, nowMs),
          first < 0 ? SILENT : levels.bar(first + 1, nowMs),
          slot.vertical === true,
        );
        break;
      }
      case "arrangeGrid":
        paintArrangeGrid(out, box, slot.firstBarX * dpr, slot.barPx * dpr, slot.beats ?? 1);
        break;
      case "overview":
        paintOverview(out, box, Math.max(1, Math.round(((OVERVIEW.inset * this.scale) / 100) * dpr)));
        break;
      case "rects":
        paintRects(out, box, slot.rects, dpr);
        break;
      case "paint":
        slot.paint(out, box, { dpr, scale: this.scale, isDown: this.isDown });
        break;
    }
  }
}

const SILENT = { length: 0, clipped: false } as const;

function measure(s: Slot, dpr: number): void {
  const r = s.node.getBoundingClientRect();
  s.x0 = Math.round(r.left * dpr);
  s.y0 = Math.round(r.top * dpr);
  s.x1 = Math.round(r.right * dpr);
  s.y1 = Math.round(r.bottom * dpr);
}
