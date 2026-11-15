// Where the page's main thread spends each frame, for development (MELOET_MEASURE_DIR). Off unless Rust says so.
//
// For every frame of the engine: the time in the frame callbacks (the play line, the clip being
// recorded), in measuring the boxes (getBoundingClientRect: this is where a pending layout runs),
// in building the rectangles, and in WebGL; which regions were drawn and how many rectangles.
// `busyMs` is from the start of the engine's frame to a message posted then: every animation frame
// callback after it, the microtasks (Svelte), style, layout and paint of that frame.

import type { RegionName } from "./budget";

export const PROFILE_NAME = "frame-profile.json";
const REPORT_EVERY_MS = 2000;
/** Frames kept in full (about a minute); the totals go on. */
const KEEP = 4000;

export interface FrameRecord {
  atMs: number;
  callbacksMs: number;
  measureMs: number;
  paintMs: number;
  glMs: number;
  busyMs: number | null;
  /** Region name → rectangles drawn into it this frame. */
  drawn: Partial<Record<RegionName, number>>;
}

export class FrameProfile {
  private readonly frames: FrameRecord[] = [];
  private readonly draws: Partial<Record<RegionName, number>> = {};
  private total = 0;
  private current: FrameRecord | null = null;
  private readonly channel = new MessageChannel();
  private readonly waiting: FrameRecord[] = [];
  private sent = 0;
  private timer = 0;

  constructor(
    private readonly report: (name: string, json: string) => Promise<void>,
    private readonly now: () => number = () => performance.now(),
  ) {}

  start(): void {
    this.channel.port1.onmessage = () => {
      const f = this.waiting.shift();
      if (f) f.busyMs = this.now() - f.atMs;
    };
    this.timer = window.setInterval(() => void this.flush(), REPORT_EVERY_MS);
  }

  stop(): void {
    clearInterval(this.timer);
    this.channel.port1.close();
  }

  /** Called at the start of an engine frame. Returns the record to fill. */
  begin(): FrameRecord {
    const f: FrameRecord = {
      atMs: this.now(),
      callbacksMs: 0,
      measureMs: 0,
      paintMs: 0,
      glMs: 0,
      busyMs: null,
      drawn: {},
    };
    this.total += 1;
    if (this.frames.length < KEEP) this.frames.push(f);
    this.current = f;
    this.waiting.push(f);
    this.channel.port2.postMessage(0);
    return f;
  }

  drew(region: RegionName, rects: number): void {
    if (!this.current) return;
    this.current.drawn[region] = (this.current.drawn[region] ?? 0) + rects;
    this.draws[region] = (this.draws[region] ?? 0) + 1;
  }

  json(): string {
    return JSON.stringify({ frames: this.total, draws: this.draws, kept: this.frames });
  }

  async flush(): Promise<void> {
    if (this.total === this.sent) return;
    const count = this.total;
    try {
      await this.report(PROFILE_NAME, this.json());
      this.sent = count;
    } catch {
      // Sent again with the next flush.
    }
  }
}
