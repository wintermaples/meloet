// Of the keys (spec 4), the intents when the piano roll has focus. Does not know the DOM.
// Only when an intent to move notes with ↑ ↓ is made, the piano roll part is told (to play the moved notes; audition.ts).

import type { AppState, Command } from "../../bridge";
import { gridTick, mapOf, meterAtOf } from "../../time/bars";
import { nudgedKeys } from "./audition";
import { drumKeysOf, pianoRollDrums, pianoRollTrack } from "./drums";
import { drumMoveKeys, duplicateTicks, gridTicks, indexesOf, noteStart, type Shown } from "./geom";

export type PianoRollKey =
  | { kind: "nudge"; steps: number; keys: number }
  | { kind: "deleteNotes" }
  /** With Delete, of points and notes, remove the points (when only points are selected, or a point was pressed last). */
  | { kind: "deleteControls" }
  | { kind: "quantize" }
  | { kind: "selectAll" }
  | { kind: "duplicate" }
  | { kind: "cut" }
  | { kind: "copy" }
  | { kind: "paste" };

export interface PianoRollKeyContext {
  state: AppState;
  /** The shown take (null if none) */
  shown: Shown | null;
  /** The current play position (ticks) */
  playTick: number;
}

/** null is "do nothing". */
export function pianoRollCommand(key: PianoRollKey, c: PianoRollKeyContext): Command | null {
  const pr = c.state.pianoRoll;
  const map = mapOf(c.state.timeline);
  /** One grid step at tick: with the 1-bar grid, the bar there. */
  const stepAt = (tick: number) => gridTicks(pr.grid, meterAtOf(map, tick));
  if (key.kind === "paste") {
    const range = pasteRange(c);
    if (pr.clipboardNotes === 0 || !range) return null;
    // Outside the clip, at its head: the clip grows only as far as the notes need.
    const inside = c.playTick >= range.from && c.playTick < range.to;
    return { type: "pasteNotes", at: inside ? gridTick(map, c.playTick, stepAt(c.playTick), false, pr.grid.division === 0) : range.from };
  }
  if (key.kind === "selectAll") return { type: "selectAllNotes" };
  const n = c.shown;
  if (!n || pr.take !== n.take.id) return null;
  if (key.kind === "deleteControls") {
    const points = indexesOf(pr.selectedControls).filter((i) => i < n.take.controlTicks.length);
    return points.length > 0 ? { type: "deleteControls", take: n.take.id, points } : null;
  }
  if (key.kind === "quantize") {
    if (n.take.starts.length === 0) return null;
    // If nothing is selected, empty notes (all notes of the take).
    return { type: "quantize", take: n.take.id, notes: indexesOf(pr.selectedNotes).filter((i) => i < n.take.starts.length) };
  }
  const notes = indexesOf(pr.selectedNotes).filter((i) => i < n.take.starts.length);
  if (notes.length === 0) return null;
  const take = n.take.id;
  let first = Infinity;
  for (const i of notes) first = Math.min(first, noteStart(n, i));
  switch (key.kind) {
    case "nudge": {
      let keys = key.keys;
      // Drum rows are not in key order, so ↑ ↓ move one row at a time (no move if there is nowhere to go). Shift ↑ ↓ (one octave) is not used.
      const drums = pianoRollDrums(c.state);
      if (keys !== 0 && drums) {
        const rows = drumKeysOf(pianoRollTrack(c.state), drums);
        keys = Math.abs(keys) === 1 ? drumMoveKeys({ keys: rows }, n.take.keys, notes, notes[0]!, -keys) : 0;
        if (keys === 0 && key.steps === 0) return null;
      }
      // Moving back from a bar's start takes the length of the bar before.
      const step = stepAt(key.steps < 0 ? Math.max(0, first - 1) : first);
      const command = { type: "moveNotes", take, notes, ticks: key.steps * step, keys, copy: false } as const;
      if (keys !== 0) nudgedKeys(command);
      return command;
    }
    case "deleteNotes":
      return { type: "deleteNotes", take, notes };
    case "duplicate":
      return { type: "moveNotes", take, notes, ticks: duplicateTicks(n, notes, stepAt(first)), keys: 0, copy: true };
    case "cut":
      return { type: "cutNotes", take, notes };
    case "copy":
      return { type: "copyNotes", take, notes };
  }
}

/** The range in the song of the clip the piano roll shows, or null when it shows none. */
function pasteRange(c: PianoRollKeyContext): { from: number; to: number } | null {
  if (c.shown) return c.shown;
  const id = c.state.arrange.pianoRollClip;
  for (const track of c.state.song.tracks) {
    const clip = track.clips.find((x) => x.id === id && x.audio === null);
    if (clip) return { from: clip.start, to: clip.end };
  }
  return null;
}
