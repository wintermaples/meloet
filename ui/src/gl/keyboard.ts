import { EVENT_WINS_US } from "./dims";

const BLACK = [false, true, false, true, false, false, true, false, true, false, true, false];

export function isBlack(note: number): boolean {
  return BLACK[((note % 12) + 12) % 12]!;
}

export interface KeyPlace {
  note: number;
  black: boolean;
  /** White: its cell, counted from the left. Black: the line it sits on (line i is the left edge of cell i). */
  index: number;
}

export interface KeyboardLayout {
  whiteCount: number;
  keys: KeyPlace[];
}

const layouts = new Map<string, KeyboardLayout>();

/** Where each key from firstNote to lastNote (both included) goes. */
export function keyboardLayout(firstNote: number, lastNote: number): KeyboardLayout {
  const id = `${firstNote}:${lastNote}`;
  let layout = layouts.get(id);
  if (!layout) {
    const keys: KeyPlace[] = [];
    let whites = 0;
    for (let note = firstNote; note <= lastNote; note++) {
      const black = isBlack(note);
      keys.push({ note, black, index: whites });
      if (!black) whites += 1;
    }
    layouts.set(id, (layout = { whiteCount: whites, keys }));
  }
  return layout;
}

/**
 * The keys that are down, from two sources: Events (at once, in order) and telemetry (about
 * 60 times a second, the whole set). They travel separately, so a telemetry message can be
 * older than an Event that arrived before it; the Event then keeps its say.
 */
export class HeldKeys {
  private readonly down = new Uint32Array(4);
  private readonly eventUs = new Float64Array(128).fill(-Infinity);

  isDown(note: number): boolean {
    return ((this.down[note >>> 5]! >>> (note & 31)) & 1) === 1;
  }

  private set(note: number, on: boolean): boolean {
    if (this.isDown(note) === on) return false;
    this.down[note >>> 5]! ^= 1 << (note & 31);
    return true;
  }

  /** Returns whether anything changed. */
  onEvent(note: number, on: boolean, tUs: number): boolean {
    if (note < 0 || note > 127) return false;
    this.eventUs[note] = tUs;
    return this.set(note, on);
  }

  /** `keys`: four 32-bit words, bit n of word w is note 32 * w + n. Returns whether anything changed. */
  onTelemetry(keys: Uint32Array, sentUs: number): boolean {
    let changed = false;
    for (let w = 0; w < 4; w++) {
      let diff = (keys[w]! ^ this.down[w]!) >>> 0;
      while (diff !== 0) {
        const bit = 31 - Math.clz32(diff);
        diff = (diff & ~(1 << bit)) >>> 0;
        const note = 32 * w + bit;
        if (sentUs - this.eventUs[note]! > EVENT_WINS_US) {
          changed = this.set(note, ((keys[w]! >>> bit) & 1) === 1) || changed;
        }
      }
    }
    return changed;
  }
}
