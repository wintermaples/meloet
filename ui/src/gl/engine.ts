import type { Event, Telemetry } from "../bridge";
import { SCALES, type Scale } from "../design/tokens";
import { Batch } from "./batch";
import { REGIONS, SHOWN_WITH, fault, regionHz, type RegionName } from "./budget";
import { OVERVIEW } from "./dims";
import { HeldKeys } from "./keyboard";
import { Levels, MASTER_CHANNEL, RETURN_A_CHANNEL, RETURN_B_CHANNEL, trackChannel } from "./levels";
import {
  blackKeyPx,
  clipMarkPx,
  paintArrangeGrid,
  paintGridLines,
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
  /**
   * The nearest box around the slot marked `data-gl-clip` (inside the region box), or null: a box that scrolls its
   * content. The slot is drawn only where that box shows it, and the canvas covers only that part.
   */
  clip: HTMLElement | null;
  /** The part of the box that shows (the box cut to `clip`). */
  vx0: number;
  vy0: number;
  vx1: number;
  vy1: number;
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
  /** When the canvas was last looked at for drawing (performance.now()), for a region drawn below 60 Hz. */
  lastMs: number;
  /** Something moved or changed size, not only the levels: drawn now even below 60 Hz. */
  urgent: boolean;
  /**
   * The parts of a region shown below 60 Hz, left to right; empty when the WebGL canvas itself is in the page.
   * Each frame shows one part, in turn, so the page repaints a part's rectangle and not the whole region's.
   */
  tiles: Tile[];
  /** The tile whose turn is next. */
  turn: number;
  /** This frame draws every tile (see `urgent`, `mustDraw`). */
  whole: boolean;
}

/**
 * A canvas in the page that shows a copy of what the region's WebGL canvas drew for some of its slots.
 * It has no WebGL context of its own (budget.ts counts those), only a place to put the copy.
 */
interface Tile {
  canvas: HTMLCanvasElement;
  out: ImageBitmapRenderingContext;
  slots: Slot[];
  /** In device pixels of the window. */
  x0: number;
  y0: number;
  width: number;
  height: number;
  drawn: Batch;
  mustDraw: boolean;
  /** Goes up with each copy asked for; a copy that comes after a newer one was asked for is dropped. */
  asked: number;
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
    for (const r of this.regions.values()) r.dirty = r.urgent = true;
    this.invalidate();
  };
  private readonly sizes = new ResizeObserver(this.again);
  private readonly stop: (() => void)[] = [];
  /** The boxes of the `data-gl-clip` elements, measured once a frame. */
  private readonly clips = new Map<HTMLElement, DOMRect>();
  private readonly isDown = (note: number) => this.held.isDown(note);
  private readonly box: Box = { x0: 0, y0: 0, x1: 0, y1: 0 };
  /** The page gives no place to put a copy of a canvas: regions below 60 Hz are drawn whole, less often. */
  private noTiles = false;

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
          const stamped = slot.kind === "cover" && s.slot.kind === "cover" && slot.stamp !== s.slot.stamp;
          s.slot = slot;
          if (s.region) s.region.dirty = s.region.urgent = true;
          if (s.region && stamped) s.region.mustDraw = true;
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
    const s: Slot = { node, slot, region: null, x0: 0, y0: 0, x1: 0, y1: 0, clip: null, vx0: 0, vy0: 0, vx1: 0, vy1: 0 };
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
    r.dirty = r.urgent = true;
    this.invalidate();
  }

  /** The region box went away: its canvas goes back to the pool. */
  private close(r: Region): void {
    this.regions.delete(r.host);
    if (!this.slots.has(r.host)) this.sizes.unobserve(r.host);
    this.split(r, 1);
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
      const clip = s.node.closest<HTMLElement>("[data-gl-clip]");
      s.clip = clip && found.host.contains(clip) && clip !== found.host ? clip : null;
      r.dirty = r.urgent = true;
    }
  }

  private open(host: HTMLElement, name: RegionName): Region | null {
    const gl = takeCanvas(REGIONS[name].opaque, name);
    if (!gl) return null;
    const r: Region = { host, name, gl, slots: [], x0: 0, y0: 0, width: 0, height: 0, dirty: true, mustDraw: true, drawn: new Batch(), made: null, lastMs: -Infinity, urgent: true, tiles: [], turn: 0, whole: true };
    if (gl.fresh && this.profile) {
      const m = gl.renderer.madeMs;
      r.made = { region: name, contextMs: m.context, programMs: m.program, firstDrawMs: null };
      this.profile.made(r.made);
    }
    gl.fresh = false;
    gl.onRestored = () => {
      r.mustDraw = true;
      r.dirty = r.urgent = true;
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
    let held = false;
    for (const r of this.regions.values()) {
      if (!r.dirty) continue;
      const hz = regionHz(r.name);
      const parts = hz < 60 && this.canCopy() ? Math.round(60 / hz) : 1;
      if (parts !== Math.max(1, r.tiles.length)) this.split(r, parts);
      // Without tiles, a region drawn below 60 Hz waits for its turn; a frame between two of 60 Hz comes a few ms early or late.
      if (r.tiles.length === 0 && hz < 60 && !r.mustDraw && !r.urgent && now - r.lastMs < 1000 / hz - SLACK_MS) {
        held = true;
        continue;
      }
      r.lastMs = now;
      r.whole = r.urgent || r.mustDraw;
      r.urgent = false;
      r.dirty = false;
      due.push([r, r.host.getBoundingClientRect()]);
      this.clips.clear();
      for (const s of r.slots) measure(s, dpr, this.clips);
    }
    for (const [r, host] of due) this.place(r, host, dpr);
    if (p) p.measureMs = performance.now() - t;
    const shown = new Set<RegionName>();
    for (const [r] of due) if (r.tiles.length > 0 ? this.drawTiles(r, now, dpr, p) : this.draw(r, now, dpr, p)) shown.add(r.name);
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
    if (moving || held) this.invalidate();
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
      if (s.vx1 <= s.vx0 || s.vy1 <= s.vy0) continue;
      x0 = Math.min(x0, s.vx0);
      y0 = Math.min(y0, s.vy0);
      x1 = Math.max(x1, s.vx1);
      y1 = Math.max(y1, s.vy1);
    }
    x0 = Math.max(x0, Math.round(host.left * dpr), 0);
    y0 = Math.max(y0, Math.round(host.top * dpr), 0);
    x1 = Math.min(x1, Math.round(host.right * dpr), Math.round(innerWidth * dpr));
    y1 = Math.min(y1, Math.round(host.bottom * dpr), Math.round(innerHeight * dpr));
    let width = x1 - x0;
    let height = y1 - y0;
    if (!(width > 0 && height > 0)) x0 = y0 = width = height = 0;
    r.x0 = x0;
    r.y0 = y0;
    r.width = width;
    r.height = height;

    const canvas = r.gl.canvas;
    if (r.tiles.length === 0) {
      put(canvas, r.host, host, x0, y0, width, height, dpr);
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
        r.mustDraw = true;
      }
      return;
    }

    // The slots that show, left to right, in as many runs as there are tiles, each run as wide as the others (a slot
    // goes where its middle lies): the widest tile decides how much of the page a frame repaints. A tile covers its run.
    const right = x0 + width;
    const bottom = y0 + height;
    const n = r.tiles.length;
    const runOf = (s: Slot) => Math.min(n - 1, Math.max(0, Math.floor((((s.vx0 + s.vx1) / 2 - x0) * n) / width)));
    let widest = 0;
    let highest = 0;
    r.tiles.forEach((tile, i) => {
      // Page order, as in the canvas of a region without tiles.
      tile.slots = r.slots.filter((s) => s.vx1 > x0 && s.vx0 < right && s.vy1 > y0 && s.vy0 < bottom && runOf(s) === i);
      let tx0 = Infinity;
      let ty0 = Infinity;
      let tx1 = -Infinity;
      let ty1 = -Infinity;
      for (const s of tile.slots) {
        tx0 = Math.min(tx0, s.vx0);
        ty0 = Math.min(ty0, s.vy0);
        tx1 = Math.max(tx1, s.vx1);
        ty1 = Math.max(ty1, s.vy1);
      }
      tx0 = Math.max(tx0, x0);
      ty0 = Math.max(ty0, y0);
      let w = Math.min(tx1, right) - tx0;
      let t = Math.min(ty1, bottom) - ty0;
      if (!(w > 0 && t > 0)) tx0 = ty0 = w = t = 0;
      if (tile.x0 !== tx0 || tile.y0 !== ty0 || tile.width !== w || tile.height !== t) tile.mustDraw = true;
      tile.x0 = tx0;
      tile.y0 = ty0;
      tile.width = w;
      tile.height = t;
      put(tile.canvas, r.host, host, tx0, ty0, w, t, dpr);
      widest = Math.max(widest, w);
      highest = Math.max(highest, t);
    });
    // The WebGL canvas, out of the page, is as big as the biggest tile: each tile is drawn at its top left.
    if (canvas.width !== widest) canvas.width = widest;
    if (canvas.height !== highest) canvas.height = highest;
  }

  /** `createImageBitmap` copies a WebGL canvas within the frame (WebKitGTK 2.52: under 1 ms for 900×264). */
  private canCopy(): boolean {
    return !this.noTiles && typeof createImageBitmap === "function";
  }

  /** Shows the region as `parts` tiles, or (1) as its WebGL canvas itself. */
  private split(r: Region, parts: number): void {
    for (const tile of r.tiles) {
      tile.asked += 1;
      tile.canvas.remove();
    }
    r.tiles = [];
    r.turn = 0;
    if (parts > 1) {
      for (let i = 0; i < parts; i++) {
        const canvas = document.createElement("canvas");
        canvas.setAttribute("aria-hidden", "true");
        canvas.style.cssText = "position:absolute;pointer-events:none";
        const out = canvas.getContext("bitmaprenderer");
        if (!out) {
          this.noTiles = true;
          return this.split(r, 1);
        }
        r.tiles.push({ canvas, out, slots: [], x0: 0, y0: 0, width: 0, height: 0, drawn: new Batch(), mustDraw: true, asked: 0 });
      }
      r.gl.canvas.remove();
      for (const tile of r.tiles) r.host.appendChild(tile.canvas);
    } else if (r.gl.canvas.parentElement !== r.host && this.regions.get(r.host) === r) r.host.appendChild(r.gl.canvas);
    r.mustDraw = true;
    r.dirty = r.urgent = true;
  }

  /**
   * Draws the tile whose turn it is, or the next one whose picture changed, and leaves the others for
   * the coming frames; every tile when the region moved. True when a tile was shown.
   */
  private drawTiles(r: Region, nowMs: number, dpr: number, p: FrameRecord | null): boolean {
    const n = r.tiles.length;
    let shown = false;
    for (let k = 0; k < n; k++) {
      const i = (r.turn + k) % n;
      if (!this.drawTile(r, r.tiles[i]!, nowMs, dpr, p)) continue;
      shown = true;
      if (r.whole) continue;
      r.turn = (i + 1) % n;
      if (k < n - 1) {
        r.dirty = true;
        this.invalidate();
      }
      break;
    }
    if (r.whole && !r.gl.lost) r.mustDraw = false;
    return shown;
  }

  private drawTile(r: Region, tile: Tile, nowMs: number, dpr: number, p: FrameRecord | null): boolean {
    if (r.gl.lost || tile.width <= 0 || tile.height <= 0) return false;
    const t0 = p ? performance.now() : 0;
    const out = this.build(tile.slots, tile.x0, tile.y0, tile.width, tile.height, nowMs, dpr);
    const same = !tile.mustDraw && !r.mustDraw && out.sameAs(tile.drawn);
    const t1 = p ? performance.now() : 0;
    if (p) p.paintMs += t1 - t0;
    if (same) return false;
    const canvas = r.gl.canvas;
    r.gl.renderer.draw(out, tile.width, tile.height, canvas.height - tile.height);
    [this.next, tile.drawn] = [tile.drawn, out];
    tile.mustDraw = false;
    const asked = ++tile.asked;
    createImageBitmap(canvas, 0, 0, tile.width, tile.height).then(
      (copy) => {
        if (tile.asked === asked) tile.out.transferFromImageBitmap(copy);
        else copy.close();
      },
      () => {
        tile.mustDraw = true;
      },
    );
    this.stats.draws += 1;
    if (p) {
      const t2 = performance.now();
      p.glMs += t2 - t1;
      if (r.made) {
        r.made.firstDrawMs = t2 - t1;
        r.made = null;
      }
      this.profile!.drew(r.name, tile.drawn.rectCount);
    }
    return true;
  }

  /** The rectangles of `slots`, in device pixels from (x0, y0), cut to a picture of width × height. */
  private build(slots: readonly Slot[], x0: number, y0: number, width: number, height: number, nowMs: number, dpr: number): Batch {
    const out = this.next;
    out.reset();
    const box = this.box;
    for (const s of slots) {
      box.x0 = s.x0 - x0;
      box.y0 = s.y0 - y0;
      box.x1 = s.x1 - x0;
      box.y1 = s.y1 - y0;
      const cx0 = Math.max(s.vx0 - x0, 0);
      const cy0 = Math.max(s.vy0 - y0, 0);
      const cx1 = Math.min(s.vx1 - x0, width);
      const cy1 = Math.min(s.vy1 - y0, height);
      if (cx1 <= cx0 || cy1 <= cy0) continue;
      out.clip(cx0, cy0, cx1, cy1);
      this.paint(out, s.slot, box, nowMs, dpr);
    }
    return out;
  }

  /** True when the canvas was shown: its rectangles differ from the ones it showed, or it had to be drawn. */
  private draw(r: Region, nowMs: number, dpr: number, p: FrameRecord | null): boolean {
    if (r.gl.lost || r.width <= 0 || r.height <= 0) return false;
    const t0 = p ? performance.now() : 0;
    const out = this.build(r.slots, r.x0, r.y0, r.width, r.height, nowMs, dpr);
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
    if ("mic" in source) return this.levels.micChannel(source.mic);
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
          slot.toZero === true,
        );
        break;
      }
      case "arrangeGrid":
        paintArrangeGrid(out, box, slot.firstBarX * dpr, slot.barPx * dpr, slot.beats ?? 1);
        break;
      case "gridLines":
        paintGridLines(out, box, slot.lines, slot.beats, dpr);
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
/** How early a frame may come for a region drawn below 60 Hz (a frame of 60 Hz is 16.7 ms). */
const SLACK_MS = 4;

/** Places a canvas of a region at a rectangle given in device pixels of the window. */
function put(canvas: HTMLCanvasElement, h: HTMLElement, host: DOMRect, x0: number, y0: number, width: number, height: number, dpr: number): void {
  const style = canvas.style;
  style.left = `${x0 / dpr - host.left - h.clientLeft + h.scrollLeft}px`;
  style.top = `${y0 / dpr - host.top - h.clientTop + h.scrollTop}px`;
  style.width = `${width / dpr}px`;
  style.height = `${height / dpr}px`;
}

function measure(s: Slot, dpr: number, clips: Map<HTMLElement, DOMRect>): void {
  const r = s.node.getBoundingClientRect();
  s.x0 = s.vx0 = Math.round(r.left * dpr);
  s.y0 = s.vy0 = Math.round(r.top * dpr);
  s.x1 = s.vx1 = Math.round(r.right * dpr);
  s.y1 = s.vy1 = Math.round(r.bottom * dpr);
  if (!s.clip) return;
  let c = clips.get(s.clip);
  if (!c) clips.set(s.clip, (c = s.clip.getBoundingClientRect()));
  s.vx0 = Math.max(s.vx0, Math.round(c.left * dpr));
  s.vy0 = Math.max(s.vy0, Math.round(c.top * dpr));
  s.vx1 = Math.min(s.vx1, Math.round(c.right * dpr));
  s.vy1 = Math.min(s.vy1, Math.round(c.bottom * dpr));
}
