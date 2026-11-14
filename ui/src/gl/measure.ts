// Key → screen timing for development (docs/report.md 5-2). Off unless Rust says so.

export interface KeyRecord {
  note: number;
  /** When Rust received the key: CLOCK_MONOTONIC, microseconds. */
  tUs: number;
  /** performance.now() when the Event reached JS. */
  recvMs: number;
  /** Timestamp of the animation frame that drew the key lit; null until then. */
  drawMs: number | null;
}

export interface MeasureDeps {
  clockOffsetUs(): Promise<number>;
  report(name: string, json: string): Promise<void>;
  now(): number;
}

export const REPORT_NAME = "key-to-screen.json";
const REPORT_EVERY_MS = 2000;
const CLOCK_EVERY_MS = 10000;
const KEEP = 5000;

export class KeyToScreen {
  private readonly keys: KeyRecord[] = [];
  private firstUndrawn = 0;
  /** `rustUs ≈ ms * 1000 + offsetUs` for recvMs and drawMs. Null until the clocks are compared. */
  private offsetUs: number | null = null;
  private offsetAtMs = -Infinity;
  private sent = 0;
  private timer = 0;

  constructor(private readonly deps: MeasureDeps) {}

  start(): void {
    void this.tick();
    this.timer = window.setInterval(() => void this.tick(), REPORT_EVERY_MS);
  }

  stop(): void {
    clearInterval(this.timer);
  }

  noteOn(note: number, tUs: number): void {
    if (this.keys.length >= KEEP) return;
    this.keys.push({ note, tUs, recvMs: this.deps.now(), drawMs: null });
  }

  /** Every key received before this frame is on the screen after it. */
  drawn(frameMs: number): void {
    for (; this.firstUndrawn < this.keys.length; this.firstUndrawn++) {
      this.keys[this.firstUndrawn]!.drawMs = frameMs;
    }
  }

  json(): string {
    return JSON.stringify({ offsetUs: this.offsetUs, offsetAtMs: this.offsetAtMs, keys: this.keys });
  }

  async tick(): Promise<void> {
    const now = this.deps.now();
    if (now - this.offsetAtMs >= CLOCK_EVERY_MS) {
      try {
        this.offsetUs = await this.deps.clockOffsetUs();
        this.offsetAtMs = now;
      } catch {
        // Not connected yet; the next tick tries again.
      }
    }
    if (this.keys.length === this.sent) return;
    const count = this.keys.length;
    try {
      await this.deps.report(REPORT_NAME, this.json());
      this.sent = count;
    } catch {
      // Sent again with the next tick.
    }
  }
}
