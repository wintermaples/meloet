// The drag shared by fader, pan and send handles. Only the screen moves first (shown); the start value (from) is drawn with a gray.7 frame.
// It does not jump to the pressed point; only movement from the pressed point changes the value. Esc cancels first in the key order (onDragCancel).
// Keys (spec 3 (86-7)): focus with Tab; each arrow press sends commit: false; 1 second after the key is released, or when focus leaves,
// one commit: true (one step). Home for the start value, Enter for the typing box (typing).

import { onMount } from "svelte";
import type { Command, StripRef } from "../../bridge";
import { send } from "../../state/app.svelte";
import { onDragCancel } from "../pianoroll/view.svelte";
import { KeySteps, MixPreview } from "./mixDrag";
import { dbToFrac, fracToDb, parseDb } from "./scale";

export interface MixControlOptions<V> {
  /** The song value (Rust state). */
  value: () => V;
  /** The value after moving dx / dy (CSS px) from the value start at grab time. box is the part's box at grab time. */
  read: (start: V, dx: number, dy: number, box: DOMRect) => V;
  make: (value: V, commit: boolean) => Command;
  same?: (a: V, b: V) => boolean;
  /** The value after one arrow key press. undefined if that key does not move it. */
  step?: (value: V, key: string, shift: boolean) => V | undefined;
  /** The Home value (the start value). */
  home?: () => V;
  /** The value of typed text. undefined if it cannot be read. */
  parse?: (text: string) => V | undefined;
}

export type MixControl<V> = ReturnType<typeof mixControl<V>>;

export function mixControl<V>(opts: MixControlOptions<V>) {
  const same = opts.same ?? ((a: V, b: V) => a === b);
  let press: { x: number; y: number; pointer: number; box: DOMRect; el: HTMLElement; preview: MixPreview<V> } | null = null;
  let live = $state<{ from: V; now: V } | null>(null);
  /** A value moved by keys that is not the song value yet. Cleared when the song value changes. */
  let keyed = $state<{ value: V } | null>(null);
  let typing = $state(false);
  const keys = new KeySteps<V>(opts.make, send);
  $effect(() => {
    opts.value();
    keyed = null;
  });

  function end(): void {
    if (press?.el.hasPointerCapture?.(press.pointer)) press.el.releasePointerCapture(press.pointer);
    press = null;
    live = null;
  }

  function down(e: PointerEvent): void {
    if (e.button !== 0 || press) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture?.(e.pointerId);
    const from = opts.value();
    press = { x: e.clientX, y: e.clientY, pointer: e.pointerId, box: el.getBoundingClientRect(), el, preview: new MixPreview(opts.make, send) };
    live = { from, now: from };
  }

  function move(e: PointerEvent): void {
    if (!press || !live || e.pointerId !== press.pointer) return;
    const next = opts.read(live.from, e.clientX - press.x, e.clientY - press.y, press.box);
    if (same(next, live.now)) return;
    live = { from: live.from, now: next };
    press.preview.move(next);
  }

  function up(e: PointerEvent): void {
    if (!press || !live || e.pointerId !== press.pointer) return;
    press.preview.commit(live.now);
    end();
  }

  function cancel(): boolean {
    if (!press) return false;
    press.preview.cancel();
    end();
    return true;
  }

  onMount(() => {
    const off = onDragCancel(cancel);
    return () => {
      off();
      keys.flush();
    };
  });

  function keydown(e: KeyboardEvent): void {
    if (e.ctrlKey || e.metaKey || e.altKey || e.isComposing || press) return;
    const now = keyed ? keyed.value : opts.value();
    let next: V | undefined;
    if (e.key === "Enter" && !e.shiftKey && opts.parse) {
      e.preventDefault();
      e.stopPropagation();
      keys.flush();
      typing = true;
      return;
    }
    if (e.key === "Home" && !e.shiftKey) next = opts.home?.();
    else next = opts.step?.(now, e.key, e.shiftKey);
    if (next === undefined) return;
    // Do not pass it on to Home (go to start) or to arrows of other bands.
    e.preventDefault();
    e.stopPropagation();
    if (same(next, now)) return;
    keyed = { value: next };
    keys.step(next);
  }

  function keyup(): void {
    keys.release();
  }

  /** Opens the typing field, the same as Enter (a double click on the number). */
  function startTyping(): void {
    if (press || !opts.parse) return;
    keys.flush();
    typing = true;
  }

  function submit(text: string): boolean {
    const v = opts.parse?.(text);
    if (v === undefined) return false;
    typing = false;
    keyed = { value: v };
    send(opts.make(v, true));
    return true;
  }

  return {
    /** { from, now } only while moving. */
    get drag() {
      return live;
    },
    /** The value to show now. */
    get shown(): V {
      return live ? live.now : keyed ? keyed.value : opts.value();
    },
    /** Whether the typing box is open. */
    get typing() {
      return typing;
    },
    stopTyping() {
      typing = false;
    },
    startTyping,
    down,
    move,
    up,
    cancel,
    keydown,
    keyup,
    /** Focus left: make the key moves one step now. */
    blur: () => keys.flush(),
    submit,
  };
}

/** The fader's intent. The track head's typed dB sends the same one. */
export function gainCommand(strip: StripRef, db: number | null, commit: boolean): Command {
  return { type: "setGain", strip, db, commit };
}

/** Fader: moves up and down. The part height covers the scale from bottom to top. The bottom is −∞. */
export function faderControl(strip: () => StripRef, db: () => number | null): MixControl<number | null> {
  return mixControl<number | null>({
    value: db,
    read: (start, _dx, dy, box) => fracToDb(Math.max(0, Math.min(1, dbToFrac(start) - dy / Math.max(1, box.height)))),
    make: (value, commit) => gainCommand(strip(), value, commit),
    step: (value, key, shift) => (key === "ArrowUp" || key === "ArrowDown" ? stepDb(value, key === "ArrowUp" ? 1 : -1, shift) : undefined),
    home: () => 0,
    parse: parseDb,
  });
}

/** Fader / send ↑ ↓: 1 dB, 0.1 dB with Shift. Raising from −∞ gives −60, lowering below −60 gives −∞, up to +6. */
export function stepDb(db: number | null, dir: 1 | -1, fine: boolean): number | null {
  const by = fine ? 0.1 : 1;
  if (db === null) return dir > 0 ? FLOOR : null;
  const next = Math.round((db + dir * by) * 10) / 10;
  if (next < FLOOR) return null;
  return Math.min(TOP, next);
}
const FLOOR = -60;
const TOP = 6;
