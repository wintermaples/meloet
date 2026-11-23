// The chord of the draw tool, held by the screen only (like the tool itself), and kept when the band is folded.

import { SINGLE, type ChordPick } from "./chords";
import { prView } from "./view.svelte";

export const chordPick = $state({ pick: SINGLE as ChordPick });

/** The status line while the draw tool with a chord is over a row (null otherwise). */
export const chordHover = $state({ text: null as string | null });

/** Choosing a shape or a chord of the key also takes the draw tool. */
export function chooseChord(pick: ChordPick): void {
  chordPick.pick = pick;
  prView.tool = "draw";
}
