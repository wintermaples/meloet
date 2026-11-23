// Mouse in the piano roll (spec 3 "Rules for round 3 (piano roll and clips)", mock R3-PRMouse, R3-Lanes). Does not know the DOM.
// Press → move → release. While moving only the look changes (drag is drawn), and one intent that changes the song is sent on release.
// Esc (cancel) goes back to before moving. Coordinates are CSS px of the note area (the velocity lane uses the same horizontal coordinate).

import type { Command, MusicalScale } from "../../bridge";
import type { Meter } from "../time";
import type { Snapper } from "./geom";
import { chordOf } from "./audition";
import { isChord, rootOf, SINGLE, stampKeys, type ChordPick } from "./chords";
import {
  clampVelocity,
  drawLength,
  drawStart,
  drumMoveKeys,
  hitNote,
  indexesOf,
  keyToY,
  lengthText,
  moveDelta,
  noteEnd,
  noteShown,
  noteStart,
  notesInBox,
  positionText,
  resizeDelta,
  resizeStartDelta,
  signed,
  signedLength,
  tickToX,
  velocityDelta,
  writable,
  xToTick,
  yToKey,
  type RollGeom,
  type Shown,
} from "./geom";
import { keyLabel } from "./drums";
import type { Tool } from "./view.svelte";
import { t } from "../../text";
import type { IndexRange } from "../../bridge";

export interface Mods {
  shift: boolean;
  alt: boolean;
  /** Ctrl (Cmd on Mac) */
  mod: boolean;
}

interface Press {
  x: number;
  y: number;
  /** True once it moved more than space.1 (until then, it is only a press) */
  moved: boolean;
}

export type Drag =
  | { kind: "move"; press: Press; notes: number[]; grab: number; copy: boolean; ticks: number; keys: number; selectOnClick: boolean }
  /** edge: the end being moved. start moves the start, and a + ticks makes it shorter. */
  | { kind: "resize"; press: Press; notes: number[]; grab: number; edge: "start" | "end"; ticks: number }
  /** key: the pressed row, or the chord's root. keys: every key written (only `key` for one note). */
  | { kind: "draw"; press: Press; start: number; key: number; keys: number[]; length: number; velocity: number }
  | { kind: "box"; press: Press; x: number; y: number; add: boolean; hits: number[] }
  | { kind: "erase"; press: Press; x: number; y: number; hits: Set<number> }
  | { kind: "velocity"; press: Press; notes: number[]; grab: number; delta: number };

/** The cursor shape. pencil and eraser are meloet's own parts; the others are OS shapes. */
export type CursorKind = "default" | "move" | "copy" | "ew-resize" | "ns-resize" | "not-allowed" | "pencil" | "eraser";

export interface RollInput {
  shown(): Shown | null;
  geom(): RollGeom;
  /** The ticks of one grid step */
  step(): number;
  /** Snap to a grid counted from the start of the time signature span (from 0 in steps of step if not given). */
  grid?(): Snapper;
  scale(): MusicalScale;
  meter(): Meter;
  tool(): Tool;
  /** pianoRoll.selectedNotes (empty if not from the shown take) */
  selected(): readonly IndexRange[];
  /** part.note.resize, space.1 (movement still treated as a press), the height of velocity 127 (CSS px) */
  edgePx(): number;
  stillPx(): number;
  velocityFullPx(): number;
  /** The height of the velocity lane (the bottom edge of the bars, CSS px) */
  velocityHeight(): number;
  send(command: Command): void;
  /** The draw tool's note and its audition strength (the fixed velocity value when on, otherwise 100). */
  drawVelocity(): number;
  /** Whether to play when a note is pressed or moved (only when stopped). */
  mayAudition(): boolean;
  /** How many ms a length of ticks lasts (from the tempo). */
  ticksToMs(ticks: number): number;
  /** The sound the screen plays (Auditioner in audition.ts). */
  audition: { play(keys: readonly number[], velocity: number, ms?: number | null): void; stop(): void };
  /** The chord of the draw tool (one note when missing, and on drums). */
  chord?(): ChordPick;
}

/** Where the draw tool with a chord would write: drawn as accent outlines before pressing. */
export interface Shadow {
  start: number;
  length: number;
  keys: number[];
  /** The row under the cursor. */
  key: number;
}

export class RollDrag {
  drag: Drag | null = null;
  /** The note the erase tool is over (-1 for none) */
  eraseHover = -1;
  shadow: Shadow | null = null;

  constructor(private readonly input: RollInput) {}

  private selectedSet(): Set<number> {
    return new Set(indexesOf(this.input.selected()));
  }

  private takeId(): number | null {
    return this.input.shown()?.take.id ?? null;
  }

  /** The cursor and the note the erase tool is over when nothing is pressed. True if they changed. */
  hover(x: number, y: number, mods: Mods): { cursor: CursorKind; changed: boolean; shadowChanged: boolean } {
    const i = this.input;
    const hit = hitNote(i.shown(), i.geom(), x, y, i.edgePx(), true);
    const erase = i.tool() === "erase" && hit && !hit.edge ? hit.note : -1;
    const changed = erase !== this.eraseHover;
    this.eraseHover = erase;
    const pick = this.chord();
    const key = yToKey(i.geom(), y);
    let shadow: Shadow | null = null;
    if (!hit && i.tool() === "draw" && isChord(pick) && key >= 0) {
      const start = drawStart(xToTick(i.geom(), x), mods.shift ? null : i.step(), i.grid?.());
      shadow = { start, length: i.step(), keys: stampKeys(pick, key), key };
    }
    const shadowChanged = this.setShadow(shadow);
    let cursor: CursorKind;
    if (hit?.edge) cursor = "ew-resize";
    else if (i.tool() === "erase") cursor = "eraser";
    else if (hit) cursor = mods.alt ? "copy" : "move";
    // A chord is written also where the scale lock refuses one note (the owner's decision).
    else if (i.tool() === "draw") cursor = (isChord(pick) ? key >= 0 : drawable(key, i.scale())) ? "pencil" : "not-allowed";
    else cursor = "default";
    return { cursor, changed, shadowChanged };
  }

  /** True when it changed. */
  setShadow(shadow: Shadow | null): boolean {
    const was = this.shadow;
    const same =
      was === shadow ||
      (was !== null &&
        shadow !== null &&
        was.start === shadow.start &&
        was.length === shadow.length &&
        was.key === shadow.key &&
        was.keys.length === shadow.keys.length &&
        was.keys.every((k, j) => k === shadow.keys[j]));
    if (!same) this.shadow = shadow;
    return !same;
  }

  private chord(): ChordPick {
    return this.input.chord?.() ?? SINGLE;
  }

  /** The cursor while dragging. */
  cursor(): CursorKind {
    const d = this.drag;
    if (!d) return "default";
    switch (d.kind) {
      case "move":
        return d.copy ? "copy" : "move";
      case "resize":
        return "ew-resize";
      case "draw":
        return "pencil";
      case "erase":
        return "eraser";
      case "velocity":
        return "ns-resize";
      default:
        return "default";
    }
  }

  /** Pressed in the note area. */
  down(x: number, y: number, mods: Mods): void {
    const i = this.input;
    const n = i.shown();
    const g = i.geom();
    const press: Press = { x, y, moved: false };
    const hit = hitNote(n, g, x, y, i.edgePx(), true);
    const selected = this.selectedSet();
    this.eraseHover = -1;
    this.shadow = null;

    if (hit && n && hit.edge) {
      if (!selected.has(hit.note)) i.send({ type: "selectNotes", take: n.take.id, notes: [hit.note], mode: "replace" });
      const notes = selected.has(hit.note) ? [...selected] : [hit.note];
      this.drag = { kind: "resize", press, notes, grab: hit.note, edge: hit.start ? "start" : "end", ticks: 0 };
      return;
    }
    if (i.tool() === "erase") {
      this.drag = { kind: "erase", press, x, y, hits: new Set(hit ? [hit.note] : []) };
      return;
    }
    if (hit && n) {
      if (mods.mod) {
        i.send({ type: "selectNotes", take: n.take.id, notes: [hit.note], mode: "toggle" });
        return;
      }
      let notes: number[];
      let selectOnClick = false;
      if (selected.has(hit.note)) {
        notes = [...selected];
        selectOnClick = !mods.shift;
      } else {
        i.send({ type: "selectNotes", take: n.take.id, notes: [hit.note], mode: mods.shift ? "add" : "replace" });
        notes = mods.shift ? [...selected, hit.note] : [hit.note];
      }
      this.drag = { kind: "move", press, notes, grab: hit.note, copy: mods.alt, ticks: 0, keys: 0, selectOnClick };
      // The pressed note, at its pitch and velocity, until release (stopped at the note's length).
      if (i.mayAudition()) i.audition.play([n.take.keys[hit.note]!], n.take.velocities[hit.note]!, i.ticksToMs(n.take.lengths[hit.note]!));
      return;
    }
    if (i.tool() === "draw") {
      const key = yToKey(g, y);
      // Nothing is written below the drum rows.
      if (key < 0) return;
      const step = i.step();
      const start = drawStart(xToTick(g, x), mods.shift ? null : step, i.grid?.());
      const velocity = i.drawVelocity();
      const pick = this.chord();
      if (isChord(pick)) {
        const keys = stampKeys(pick, key);
        if (keys.length === 0) return;
        i.audition.play(keys, velocity);
        this.drag = { kind: "draw", press, start, key: rootOf(pick, key), keys, length: step, velocity };
        return;
      }
      if (!writable(key, i.scale())) {
        // Not written. Rust says so on the status line (spec 6, "Tried to write outside the scale").
        i.send({ type: "addNote", take: this.takeId(), start, length: step, key, velocity });
        return;
      }
      i.audition.play([key], velocity);
      this.drag = { kind: "draw", press, start, key, keys: [key], length: step, velocity };
      return;
    }
    this.drag = { kind: "box", press, x, y, add: mods.shift, hits: [] };
  }

  /** Pressed in the velocity lane. Does nothing unless near the top edge of a bar (sideways the width of the cap line, vertically space.1). */
  downVelocity(x: number, y: number): void {
    const i = this.input;
    const n = i.shown();
    if (!n) return;
    const g = i.geom();
    const full = i.velocityFullPx();
    const bottom = i.velocityHeight();
    const near = i.stillPx();
    let grab = -1;
    let best = Infinity;
    const t0 = xToTick(g, x - near);
    const t1 = xToTick(g, x + near);
    for (let k = 0; k < n.take.starts.length; k++) {
      const s = noteStart(n, k);
      if (s < t0 || s > t1 || !noteShown(n, k)) continue;
      const top = bottom - (n.take.velocities[k]! * full) / 127;
      const d = Math.abs(y - top) + Math.abs(x - tickToX(g, s));
      if (Math.abs(y - top) <= near && d < best) {
        best = d;
        grab = k;
      }
    }
    if (grab < 0) return;
    const selected = this.selectedSet();
    const notes = selected.has(grab) ? [...selected] : [grab];
    this.drag = { kind: "velocity", press: { x, y, moved: false }, notes, grab, delta: 0 };
  }

  /** Whether it is over the top edge of a bar (the cursor becomes the up-down shape). */
  overVelocityBar(x: number, y: number): boolean {
    const keep = this.drag;
    this.downVelocity(x, y);
    const over = this.drag !== keep;
    this.drag = keep;
    return over;
  }

  /** Moved while held. True if the look changed. */
  move(x: number, y: number, mods: Mods): boolean {
    const d = this.drag;
    const i = this.input;
    const n = i.shown();
    if (!d) return false;
    if (!d.press.moved) {
      if (Math.hypot(x - d.press.x, y - d.press.y) < i.stillPx() && d.kind !== "erase") return false;
      d.press.moved = true;
    }
    const g = i.geom();
    const step = mods.shift ? null : i.step();
    switch (d.kind) {
      case "move": {
        if (!n) return false;
        const dKeys = g.drum
          ? drumMoveKeys(g.drum, n.take.keys, d.notes, d.grab, Math.floor(y / g.rowH) - Math.floor(d.press.y / g.rowH))
          : yToKey(g, y) - yToKey(g, d.press.y);
        const { ticks, keys } = moveDelta(n, d.notes, d.grab, (x - d.press.x) / g.ppt, dKeys, step, i.grid?.());
        const copy = mods.alt;
        if (ticks === d.ticks && keys === d.keys && copy === d.copy) return false;
        // Each time the pitch changes, play the moved notes as a chord.
        if (keys !== d.keys && i.mayAudition()) {
          i.audition.play(
            chordOf(d.notes.map((k) => n.take.keys[k]!), keys),
            n.take.velocities[d.grab]!,
            i.ticksToMs(n.take.lengths[d.grab]!),
          );
        }
        d.ticks = ticks;
        d.keys = keys;
        d.copy = copy;
        return true;
      }
      case "resize": {
        if (!n) return false;
        const dx = (x - d.press.x) / g.ppt;
        const ticks = d.edge === "start" ? resizeStartDelta(n, d.notes, d.grab, dx, step, i.grid?.()) : resizeDelta(n, d.notes, d.grab, dx, step, i.grid?.());
        if (ticks === d.ticks) return false;
        d.ticks = ticks;
        return true;
      }
      case "draw": {
        // Drums do not show length, so a written note is always one grid cell (spec 3).
        if (g.drum) return false;
        const length = drawLength(d.start, xToTick(g, x), i.step(), step !== null, i.grid?.());
        if (length === d.length) return false;
        d.length = length;
        return true;
      }
      case "box": {
        d.x = x;
        d.y = y;
        d.hits = notesInBox(n, g, d.press.x, d.press.y, x, y);
        return true;
      }
      case "erase": {
        const before = d.hits.size;
        // Check half a row at a time, so notes in between are not skipped when moving fast.
        const steps = Math.max(1, Math.ceil(Math.hypot(x - d.x, y - d.y) / (g.rowH / 2)));
        for (let s = 1; s <= steps; s++) {
          const px = d.x + ((x - d.x) * s) / steps;
          const py = d.y + ((y - d.y) * s) / steps;
          const hit = hitNote(n, g, px, py, 0);
          if (hit) d.hits.add(hit.note);
        }
        d.x = x;
        d.y = y;
        return d.hits.size !== before;
      }
      case "velocity": {
        const delta = velocityDelta(d.press.y - y, i.velocityFullPx());
        if (delta === d.delta) return false;
        d.delta = delta;
        return true;
      }
    }
  }

  /** Released. Returns the intent that was sent (one that changes the song, or one that selects). */
  up(): Command | null {
    const d = this.drag;
    this.drag = null;
    if (!d) return null;
    const i = this.input;
    const take = this.takeId();
    const command = finish(d, take);
    if (d.kind === "draw" || d.kind === "move") i.audition.stop();
    if (command) i.send(command);
    return command;
  }

  /** Esc: cancels the drag and goes back to before moving. True if cancelled. */
  cancel(): boolean {
    const d = this.drag;
    if (!d) return false;
    this.drag = null;
    if (d.kind === "draw" || d.kind === "move") this.input.audition.stop();
    return true;
  }

  /** The text for the 「その場の読み値」 and the status line. Null when not shown. */
  reading(): string | null {
    const d = this.drag;
    const n = this.input.shown();
    const meter = this.input.meter();
    if (!d) return null;
    switch (d.kind) {
      case "draw":
        return t("pr.readDraw", { key: keyLabel(d.key, this.input.geom().drum ? (this.input.geom().drum!.names ?? "builtin") : false), position: positionText(d.start, meter), length: lengthText(d.length, meter) });
      case "move": {
        if (!d.press.moved) return null;
        const drum = this.input.geom().drum;
        if (!drum || !n) return t("pr.readMove", { ticks: signedLength(d.ticks, meter), keys: signed(d.keys) });
        return drumMoveReading(drum.keys, n.take.keys[d.grab]!, d.keys, signedLength(d.ticks, meter), d.ticks !== 0);
      }
      case "resize": {
        if (!d.press.moved || !n) return null;
        const length = n.take.lengths[d.grab]!;
        const to = d.edge === "start" ? length - d.ticks : length + d.ticks;
        return t("pr.readResize", { from: lengthText(length, meter), to: lengthText(to, meter) });
      }
      case "box":
        return d.press.moved ? t("pr.readBox", { count: d.hits.length }) : null;
      case "velocity":
        return d.press.moved ? t("pr.readVelocity", { delta: signed(d.delta), count: d.notes.length }) : null;
      default:
        return null;
    }
  }

  /** The current box of the moved notes (CSS px of the note area). Used to place the reading and the live surface. */
  movedBox(): { x0: number; x1: number; y0: number; y1: number } | null {
    const d = this.drag;
    const n = this.input.shown();
    if (!d || !n || d.kind !== "move") return null;
    const g = this.input.geom();
    let x0 = Infinity;
    let x1 = -Infinity;
    let y0 = Infinity;
    let y1 = -Infinity;
    for (const k of d.notes) {
      x0 = Math.min(x0, tickToX(g, noteStart(n, k)));
      x1 = Math.max(x1, tickToX(g, noteEnd(n, k)));
      const top = keyToY(g, n.take.keys[k]!);
      if (Number.isNaN(top)) continue;
      y0 = Math.min(y0, top);
      y1 = Math.max(y1, top + g.rowH);
    }
    return { x0, x1, y0, y1 };
  }
}

/** Whether the draw tool can write on the row (not below the drum rows). */
const drawable = (key: number, scale: MusicalScale) => key >= 0 && writable(key, scale);

/** The intent sent on release. For a press only, or a move that changes nothing, a select intent or null. */
export function finish(d: Drag, take: number | null): Command | null {
  switch (d.kind) {
    case "move":
      if (take === null) return null;
      if (!d.press.moved || (d.ticks === 0 && d.keys === 0)) {
        return !d.press.moved && d.selectOnClick ? { type: "selectNotes", take, notes: [d.grab], mode: "replace" } : null;
      }
      return { type: "moveNotes", take, notes: d.notes, ticks: d.ticks, keys: d.keys, copy: d.copy };
    case "resize":
      return take !== null && d.ticks !== 0 ? { type: "resizeNotes", take, notes: d.notes, edge: d.edge, ticks: d.ticks } : null;
    case "draw":
      if (d.keys.length > 1) return { type: "addNotes", take, start: d.start, length: d.length, keys: d.keys, velocity: d.velocity };
      return { type: "addNote", take, start: d.start, length: d.length, key: d.key, velocity: d.velocity };
    case "box":
      if (!d.press.moved || (d.hits.length === 0 && !d.add)) return d.add ? null : { type: "clearNoteSelection" };
      if (take === null || d.hits.length === 0) return null;
      return { type: "selectNotes", take, notes: d.hits, mode: d.add ? "add" : "replace" };
    case "erase":
      return take !== null && d.hits.size > 0 ? { type: "deleteNotes", take, notes: [...d.hits].sort((a, b) => a - b) } : null;
    case "velocity":
      return take !== null && d.press.moved && d.delta !== 0 ? { type: "changeVelocity", take, notes: d.notes, delta: d.delta } : null;
  }
}

/** The value while moving in the velocity lane (clamped to 1 to 127). */
export function draggedVelocity(v: number, delta: number): number {
  return clampVelocity(v + delta);
}

/**
 * The reading when moving drum notes up or down (spec 3 (85)): 「+0.0480 ・ 上へ 1 行」「下へ 2 行」.
 * Only rows if not moved sideways; only the sideways amount if the row did not change.
 */
export function drumMoveReading(rows: readonly number[], grabKey: number, keys: number, ticks: string, sideways: boolean): string {
  const from = rows.indexOf(grabKey);
  const to = rows.indexOf(grabKey + keys);
  const moved = from < 0 || to < 0 ? 0 : to - from;
  if (moved === 0) return ticks;
  const rowText = t(moved < 0 ? "pr.rowsUp" : "pr.rowsDown", { count: Math.abs(moved) });
  return sideways ? t("pr.readMoveRows", { ticks, rows: rowText }) : rowText;
}
