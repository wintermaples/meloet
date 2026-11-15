// Development only (MELOET_MEASURE_DIR and MELOET_UI_SCRIPT): plays mouse input on the piano roll the way a
// person would, so that the frame profile (gl/profile.ts) can be read for scrolling, zooming and dragging.
// The input goes in as DOM events on the note area, so the page's own handlers do the work. Nothing here runs
// unless Rust hands over a script.

export type PartKind = "scroll" | "zoom" | "drag";

export interface ScriptPart {
  kind: PartKind;
  count: number;
}

export const SCRIPT_REPORT = "ui-script.json";
const DEFAULT_COUNT = 100;
const KINDS: Record<string, PartKind> = { "pr-scroll": "scroll", "pr-zoom": "zoom", "pr-drag": "drag" };

/** `pr-scroll:100,pr-zoom,pr-drag:1000`. Null when any part is not understood. */
export function parseUiScript(text: string): ScriptPart[] | null {
  const parts: ScriptPart[] = [];
  for (const raw of text.split(",")) {
    const [name, count, ...rest] = raw.trim().split(":");
    const kind = KINDS[name ?? ""];
    const n = count === undefined ? DEFAULT_COUNT : Number(count);
    if (!kind || rest.length > 0 || !Number.isInteger(n) || n <= 0) return null;
    parts.push({ kind, count: n });
  }
  return parts.length > 0 ? parts : null;
}

/** A note to grab, in client px, and how far one drag moves it (whole grid steps). */
export interface Grab {
  x: number;
  y: number;
  dx: number;
}

export interface Target {
  /** The note area (the element the page listens on). */
  area(): HTMLElement | null;
  /** The piano roll shows a take and has a size. */
  ready(): boolean;
  grab(): Grab | null;
  /** Something that changes when a wheel step did anything: the left edge, the zoom. */
  scrollTick(): number;
  zoom(): number;
  /** Id of the take shown; a drag is answered when it changes. */
  shownTake(): number | null;
  /** The reading the page shows while dragging (null when no drag is going on). */
  reading(): string | null;
}

export interface Clock {
  now(): number;
  wall(): number;
  wait(ms: number): Promise<void>;
  frame(): Promise<void>;
  /** Resolves after the microtasks queued so far (Svelte's effects). */
  settle(): Promise<void>;
  /** Calls back with each animation frame's time until the returned function is called. */
  everyFrame(callback: (ms: number) => void): () => void;
}

export interface Step {
  /** performance.now() when the input was dispatched. */
  atMs: number;
  /** From dispatch until the microtasks after it ran. */
  inputMs: number;
  changed: boolean;
  /** Drag only: from letting go until the new take is shown; null if it did not come. */
  replyMs?: number | null;
  /** Drag only: the reading just before letting go (null: the page did not take it as a drag). */
  reading?: string | null;
}

export interface PartRecord {
  kind: PartKind;
  count: number;
  startMs: number;
  endMs: number;
  steps: Step[];
  /** Time between animation frames while the part ran (the script keeps frames coming, so a long one is the main thread being busy). */
  frameGapsMs: number[];
  /** Every 100 steps (and the last): performance.now() and Date.now(), to line up with outside samples. */
  marks: { done: number; atMs: number; wallMs: number }[];
}

export interface ScriptRecord {
  script: string;
  done: boolean;
  error: string | null;
  parts: PartRecord[];
}

const STEP_GAP_MS = 50;
const PART_GAP_MS = 1000;
const SETTLE_MS = 1500;
const READY_LIMIT_MS = 60000;
const REPLY_LIMIT_MS = 3000;
const DRAG_MOVES = 4;
const MARK_EVERY = 100;

const wheel = (x: number, y: number, deltaY: number, mods: { shift?: boolean; ctrl?: boolean }) =>
  new WheelEvent("wheel", {
    deltaY,
    deltaMode: 0,
    shiftKey: mods.shift ?? false,
    ctrlKey: mods.ctrl ?? false,
    clientX: x,
    clientY: y,
    bubbles: true,
    cancelable: true,
    composed: true,
  });

function pointer(type: string, x: number, y: number, buttons: number): MouseEvent {
  const init = { clientX: x, clientY: y, button: 0, buttons, bubbles: true, cancelable: true, composed: true };
  if (typeof PointerEvent === "function") return new PointerEvent(type, { ...init, pointerId: 1, pointerType: "mouse", isPrimary: true });
  // jsdom has no PointerEvent.
  const e = new MouseEvent(type, init);
  Object.defineProperty(e, "pointerId", { value: 1 });
  return e;
}

export class UiScript {
  private stopped = false;
  readonly record: ScriptRecord;

  constructor(
    text: string,
    private readonly parts: ScriptPart[],
    private readonly target: Target,
    private readonly clock: Clock,
    private readonly report: (name: string, json: string) => Promise<void>,
  ) {
    this.record = { script: text, done: false, error: null, parts: [] };
  }

  stop(): void {
    this.stopped = true;
  }

  async run(): Promise<void> {
    try {
      const limit = this.clock.now() + READY_LIMIT_MS;
      while (!this.target.ready()) {
        if (this.stopped) return;
        if (this.clock.now() > limit) throw new Error("the piano roll did not show a take");
        await this.clock.wait(200);
      }
      await this.clock.wait(SETTLE_MS);
      for (const part of this.parts) {
        if (this.stopped) return;
        await this.runPart(part);
        await this.clock.wait(PART_GAP_MS);
      }
      this.record.done = true;
    } catch (e) {
      this.record.error = String(e);
    }
    await this.send();
  }

  private async send(): Promise<void> {
    try {
      await this.report(SCRIPT_REPORT, JSON.stringify(this.record));
    } catch {
      // The next report carries it.
    }
  }

  private async runPart(part: ScriptPart): Promise<void> {
    const rec: PartRecord = { kind: part.kind, count: part.count, startMs: this.clock.now(), endMs: 0, steps: [], frameGapsMs: [], marks: [] };
    this.record.parts.push(rec);
    let last: number | null = null;
    const stopFrames = this.clock.everyFrame((ms) => {
      if (last !== null) rec.frameGapsMs.push(Math.round((ms - last) * 100) / 100);
      last = ms;
    });
    try {
      await this.steps(part, rec);
    } finally {
      stopFrames();
    }
    rec.endMs = this.clock.now();
  }

  private async steps(part: ScriptPart, rec: PartRecord): Promise<void> {
    let direction = 1;
    for (let i = 0; i < part.count && !this.stopped; i++) {
      const area = this.target.area();
      if (!area) throw new Error("the note area is gone");
      let step: Step;
      if (part.kind === "drag") {
        step = await this.drag(area, i % 2 === 0 ? 1 : -1);
      } else {
        step = await this.wheelStep(area, part.kind, direction);
        if (!step.changed) {
          // At an end: turn around and count the step that moves.
          direction = -direction;
          step = await this.wheelStep(area, part.kind, direction);
        }
        await this.clock.wait(STEP_GAP_MS);
      }
      rec.steps.push(step);
      if ((i + 1) % MARK_EVERY === 0 || i + 1 === part.count) {
        rec.marks.push({ done: i + 1, atMs: this.clock.now(), wallMs: this.clock.wall() });
        await this.send();
      }
    }
  }

  private async wheelStep(area: HTMLElement, kind: "scroll" | "zoom", direction: number): Promise<Step> {
    const r = area.getBoundingClientRect();
    const x = r.left + r.width / 2;
    const y = r.top + r.height / 2;
    const before = kind === "scroll" ? this.target.scrollTick() : this.target.zoom();
    // Zoom: wheel up (negative) zooms in.
    const event = kind === "scroll" ? wheel(x, y, 100 * direction, { shift: true }) : wheel(x, y, -100 * direction, { ctrl: true });
    const atMs = this.clock.now();
    area.dispatchEvent(event);
    await this.clock.settle();
    const inputMs = this.clock.now() - atMs;
    const after = kind === "scroll" ? this.target.scrollTick() : this.target.zoom();
    return { atMs, inputMs, changed: after !== before };
  }

  private async drag(area: HTMLElement, direction: number): Promise<Step> {
    const grab = this.target.grab();
    if (!grab) throw new Error("no note to grab");
    const take = this.target.shownTake();
    const atMs = this.clock.now();
    area.dispatchEvent(pointer("pointerdown", grab.x, grab.y, 1));
    await this.clock.settle();
    const inputMs = this.clock.now() - atMs;
    for (let m = 1; m <= DRAG_MOVES; m++) {
      await this.clock.frame();
      area.dispatchEvent(pointer("pointermove", grab.x + (grab.dx * direction * m) / DRAG_MOVES, grab.y, 1));
    }
    await this.clock.frame();
    const reading = this.target.reading();
    const upAt = this.clock.now();
    area.dispatchEvent(pointer("pointerup", grab.x + grab.dx * direction, grab.y, 0));
    let replyMs: number | null = null;
    while (this.clock.now() - upAt < REPLY_LIMIT_MS) {
      if (this.target.shownTake() !== take) {
        replyMs = this.clock.now() - upAt;
        break;
      }
      await this.clock.frame();
    }
    await this.clock.frame();
    await this.clock.wait(STEP_GAP_MS);
    return { atMs, inputMs, changed: replyMs !== null, replyMs, reading };
  }
}

export const browserClock: Clock = {
  now: () => performance.now(),
  wall: () => Date.now(),
  wait: (ms) => new Promise((r) => setTimeout(r, ms)),
  frame: () => new Promise((r) => requestAnimationFrame(() => r())),
  settle: () => new Promise((r) => queueMicrotask(() => queueMicrotask(r))),
  everyFrame(callback) {
    let id = requestAnimationFrame(function tick(ms) {
      callback(ms);
      id = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(id);
  },
};
