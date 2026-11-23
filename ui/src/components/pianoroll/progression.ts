// 「進行から並べる ▾」 (round 12, P6): where the chords go, the notes to send, and the ▶ that plays a progression. No DOM.

import type { Command, NoteAt } from "../../bridge";
import { barAt } from "../../time/bars";
import type { TempoMap } from "../../time/tempoMap";
import { TICKS_PER_QUARTER } from "../../time/ticks";
import { ticksToMs, type Timer } from "./audition";
import { auditionKeys, placedKeys, type Triad } from "./chords";

/**
 * The bar (from 0) the first chord goes in: the bar of the play line when it is inside the clip, else the clip's first bar.
 * A bar whose head is before the clip's start is the next one. With no clip, the bar of the play line.
 */
export function firstBar(map: TempoMap, playLine: number, clip: { start: number; end: number } | null): number {
  if (!clip) return map.meterAt(playLine).bar;
  const from = playLine >= clip.start && playLine < clip.end ? playLine : clip.start;
  const m = map.meterAt(from);
  return m.barTick < clip.start ? m.bar + 1 : m.bar;
}

/** One chord a bar from bar `first` (from 0), each the whole bar long (bars follow the meter changes). */
export function progressionNotes(chords: readonly { root: number; triad: Triad }[], first: number, map: TempoMap, velocity: number): NoteAt[] {
  return chords.flatMap((c, i) => {
    const start = barAt(map, first + i + 1).head;
    const length = barAt(map, first + i + 2).head - start;
    return placedKeys(c.root, c.triad).map((key) => ({ start, length, key, velocity }));
  });
}

/** How long a chord of ▶ lasts: two quarter notes at `bpm`. */
export const chordMs = (bpm: number) => ticksToMs(2 * TICKS_PER_QUARTER, bpm);

const realTimer: Timer = {
  set: (run, ms) => setTimeout(run, ms),
  clear: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
};

/** ▶ of a row: the chords one after another through `audition`, then silence. `stop` stops it at once. */
export class ChordSequence {
  private timer: unknown = null;
  private sounding = false;
  /** The row playing, or null. */
  playing: string | null = null;

  constructor(
    private readonly send: (command: Command) => void,
    private readonly onChange: (playing: string | null) => void = () => {},
    private readonly clock: Timer = realTimer,
  ) {}

  play(id: string, chords: readonly { root: number; triad: Triad }[], velocity: number, ms: number): void {
    this.clearTimer();
    this.playing = id;
    this.onChange(id);
    const step = (i: number) => {
      const c = chords[i];
      if (!c) {
        this.stop();
        return;
      }
      this.send({ type: "audition", keys: auditionKeys(c.root, c.triad), velocity });
      this.sounding = true;
      this.timer = this.clock.set(() => step(i + 1), ms);
    };
    step(0);
  }

  stop(): void {
    this.clearTimer();
    if (this.sounding) this.send({ type: "audition", keys: [], velocity: 0 });
    this.sounding = false;
    if (this.playing !== null) {
      this.playing = null;
      this.onChange(null);
    }
  }

  private clearTimer(): void {
    if (this.timer !== null) this.clock.clear(this.timer);
    this.timer = null;
  }
}
