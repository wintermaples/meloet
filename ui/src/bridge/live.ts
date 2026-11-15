// The notes of the recording going on (docs/architecture.md 3-1, live notes). Messages come in
// order on the messages Channel, numbered with `seq` for the whole run of the app. The book
// takes the whole list with `live_notes` at the start and whenever a number is skipped, the same
// way the state does with `snapshot`.

import type { LiveNote, LiveNotes, LiveNotesSnapshot } from "./types";

const keyOf = (n: LiveNote) => `${n.lap}:${n.index}`;

export class LiveNotesBook {
  private recordingId: number | null = null;
  /** null until the first fetch has come back. */
  private seq: number | null = null;
  private notes = new Map<string, LiveNote>();
  private waiting: LiveNotes[] = [];
  private fetching = false;
  private started = false;
  private readonly listeners = new Set<() => void>();

  constructor(
    private readonly fetch: () => Promise<LiveNotesSnapshot>,
    private readonly onError: (error: unknown) => void = () => {},
  ) {}

  /** Takes the whole list. Call it once the Channel is listening. */
  start(): void {
    this.started = true;
    this.sync().catch(this.onError);
  }

  receive(message: LiveNotes): void {
    if (this.fetching || !this.started || this.seq === null) {
      this.waiting.push(message);
      // A first fetch that failed is tried again on the next message.
      if (this.started && !this.fetching) this.sync().catch(this.onError);
      return;
    }
    if (message.seq <= this.seq) return;
    if (message.seq === this.seq + 1) {
      this.apply(message);
      this.changed();
      return;
    }
    this.sync().catch(this.onError);
  }

  /** The recording the notes belong to (`RecordingView.id`), or null before any. */
  recording(): number | null {
    return this.recordingId;
  }

  /** The notes of `recording` (when it is the one held), in pass `lap`, in order of key press. */
  notesOf(recording: number, lap: number): LiveNote[] {
    if (recording !== this.recordingId) return [];
    return [...this.notes.values()].filter((n) => n.lap === lap).sort((a, b) => a.index - b.index);
  }

  /** Called after every change. Returns the function that stops it. */
  listen(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private apply(message: LiveNotes): void {
    if (message.recording !== this.recordingId) {
      this.recordingId = message.recording;
      this.notes = new Map();
    }
    for (const n of message.notes) this.notes.set(keyOf(n), n);
    this.seq = message.seq;
  }

  private changed(): void {
    for (const l of this.listeners) l();
  }

  private async sync(): Promise<void> {
    if (this.fetching) return;
    this.fetching = true;
    try {
      for (;;) {
        const snap = await this.fetch();
        if (this.seq === null || snap.seq >= this.seq) {
          this.seq = snap.seq;
          this.recordingId = snap.recording;
          this.notes = new Map(snap.notes.map((n) => [keyOf(n), n]));
        }
        const queued = this.waiting;
        this.waiting = [];
        let gap = false;
        for (const m of queued) {
          if (m.seq <= this.seq!) continue;
          if (m.seq !== this.seq! + 1) {
            gap = true;
            break;
          }
          this.apply(m);
        }
        if (!gap) break;
      }
    } finally {
      this.fetching = false;
    }
    this.changed();
  }
}
