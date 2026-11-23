// State of the piano roll that only the screen keeps (where it is looking, which tool). Looks that are not sent to Rust (like the arrange's horizontal scroll).
// It is remembered when the band folds (spec 3: "Folding a band keeps the state of what is inside (the selected notes, the scroll position, the CC bands that were open)"), so
// it is in this module outside the part. The 「見えている範囲の枠」 in the arrange overview reads this too.
//
// Positions are ticks (quarter note = TICKS_PER_QUARTER). Widths are kept in CSS px at 100 % scale, and the scale is applied where drawn.

import { DRUM_ROWS } from "../../bridge";
import { metrics } from "../metrics.svelte";
import { TICKS_PER_QUARTER } from "../time";

export type Tool = "select" | "draw" | "erase";

/**
 * Steps of horizontal zoom (px per quarter note at 100 % scale). When a clip is opened, the largest step at which that clip fits
 * (fitQuarterPx in geom.ts; mock A-PianoRoll is 40 px per beat, F-WidePR 80 px). DEFAULT_QUARTER_PX is the step before any clip is opened.
 */
export const QUARTER_PX_STEPS = [10, 14, 20, 28, 40, 57, 80, 113, 160, 226, 320] as const;
export const DEFAULT_QUARTER_PX = 80;

/** Steps of vertical zoom (px per row = one semitone at 100 % scale; docs/history/design-notes.ja.md B-63). At first, the spec's size.note, 12. */
export const ROW_PX_STEPS = [8, 12, 16, 20, 24] as const;
export const DEFAULT_ROW_PX = 12;

export const prView = $state({
  tool: "select" as Tool,
  /** px per quarter note at 100 % scale (one of QUARTER_PX_STEPS). */
  quarterPx: DEFAULT_QUARTER_PX as number,
  /** px per row at 100 % scale (one of ROW_PX_STEPS). */
  rowPx: DEFAULT_ROW_PX as number,
  /** The song tick at the left edge of the note area. */
  scrollTick: 0,
  /** The key of the top row of the note area (MIDI 0 to 127). */
  topKey: 84,
  /** The width and height of the note area (CSS px, with the scale applied). Set by the note area part. */
  areaWidth: 0,
  areaHeight: 0,
  /** Whether to scroll sideways following the play line. If moved sideways by hand, it does not follow until playback starts again (spec 3, the same as the arrange). */
  follow: true,
  /** The clip shown now (to scroll to its start when it opens). */
  shownClip: null as number | null,
  /** Whether Delete, Mod A, Mod D, cut and so on go to the piano roll or the arrange. The place pressed last. */
  focus: "arrange" as "arrange" | "pianoRoll",
  /** Whether the last thing pressed in the piano roll was a note (including the velocity lane) or a CC point. Delete goes here when both are selected. */
  picked: "notes" as "notes" | "controls",
  /** Whether drum rows are shown (set by the note area part). Vertical zoom goes to the drum row height. */
  drums: false,
});

/** CSS px per tick at scale (100, 125, 150). */
export function pxPerTick(scale: number): number {
  return (prView.quarterPx * scale) / 100 / TICKS_PER_QUARTER;
}

/** The visible range [start, end) of the note area (ticks). The overview border draws this. */
export function visibleTicks(scale: number): { start: number; end: number } {
  const ppt = pxPerTick(scale);
  return { start: prView.scrollTick, end: ppt > 0 ? prView.scrollTick + prView.areaWidth / ppt : prView.scrollTick };
}

/** Moved sideways by hand (dragging the overview border, the horizontal bar, Shift + wheel). */
export function scrollToTick(tick: number): void {
  prView.scrollTick = Math.max(0, tick);
  prView.follow = false;
}

/** One step of horizontal zoom (Mod ＋ / Mod −, the − ＋ buttons, Mod + wheel). Scrolls so the anchorTick position stays at the same place on screen (the left edge if not given). */
export function zoomBy(steps: number, scale: number, anchorTick?: number): void {
  const now = QUARTER_PX_STEPS.indexOf(prView.quarterPx as (typeof QUARTER_PX_STEPS)[number]);
  const at = Math.min(QUARTER_PX_STEPS.length - 1, Math.max(0, (now < 0 ? QUARTER_PX_STEPS.indexOf(DEFAULT_QUARTER_PX) : now) + steps));
  const next = QUARTER_PX_STEPS[at]!;
  if (next === prView.quarterPx) return;
  const anchor = anchorTick ?? prView.scrollTick;
  const x = (anchor - prView.scrollTick) * pxPerTick(scale);
  prView.quarterPx = next;
  prView.scrollTick = Math.max(0, anchor - x / pxPerTick(scale));
}

/** CSS px per row at scale (ROW_PX_STEPS are 100 % values; they are integers at 125 % too). */
export function rowHeight(scale: number): number {
  return (prView.rowPx * scale) / 100;
}

/**
 * One step of vertical zoom (− ＋ above the keyboard column, Mod Shift + wheel, the 「表示」 menu). True if it changed.
 * Scrolls so the row at anchorY (CSS px from the top edge of the note area) stays at the same height on screen (the middle if not given).
 */
export function zoomRowsBy(steps: number, anchorY?: number): boolean {
  if (prView.drums) return zoomDrumRowsBy(steps, anchorY);
  const next = stepFrom(ROW_PX_STEPS, prView.rowPx, steps, DEFAULT_ROW_PX);
  if (next === prView.rowPx) return false;
  const scale = metrics.scale;
  const before = rowHeight(scale);
  const y = anchorY ?? prView.areaHeight / 2;
  prView.rowPx = next;
  const after = rowHeight(scale);
  const rows = Math.max(1, Math.floor(prView.areaHeight / after));
  const top = Math.round(prView.topKey - y / before + y / after);
  prView.topKey = Math.min(127, Math.max(Math.min(127, rows - 1), top));
  return true;
}

// ---- Drum rows (spec 3 "Drum piano roll"). Kept apart from the normal row height and the shown key ----

/** Drum row heights (px at 100 % scale): 12 to 48 in steps of 4. */
export const DRUM_ROW_MIN = 12;
export const DRUM_ROW_MAX = 48;
export const DRUM_ROW_STEP = 4;

export const drumView = $state({
  /** The row height at 100 % scale. null means the vertical − ＋ has not been pressed yet (the height at which all rows just fit). */
  rowPx: null as number | null,
  /** The top visible row (an index in the row keys). */
  topRow: 0,
  /** The number of rows (12 for built-in kits; for SF2 kits, the number of sounds the kit has). The piano roll sets it. */
  rows: DRUM_ROWS.length as number,
});

/** The height at which all rows just fit: the note area height (CSS px) ÷ the number of rows, rounded down to a multiple of 4, in 12 to 48 (px at 100 % scale). */
export function drumFitRowPx(areaHeight: number, rows: number, scale: number): number {
  const each = (areaHeight * 100) / scale / Math.max(1, rows);
  const floored = Math.floor(each / DRUM_ROW_STEP) * DRUM_ROW_STEP;
  return Math.min(DRUM_ROW_MAX, Math.max(DRUM_ROW_MIN, floored));
}

/** The current drum row height (px at 100 % scale). */
export function drumRowPx(rows: number, scale: number): number {
  return drumView.rowPx ?? drumFitRowPx(prView.areaHeight, rows, scale);
}

/** Puts the top row inside the range it can move to for the number of visible rows. */
export function clampTopRow(top: number, rows: number, shownRows: number): number {
  return Math.min(Math.max(0, rows - Math.max(1, shownRows)), Math.max(0, Math.round(top)));
}

function zoomDrumRowsBy(steps: number, anchorY?: number): boolean {
  const scale = metrics.scale;
  const now = drumRowPx(drumView.rows, scale);
  const next = Math.min(DRUM_ROW_MAX, Math.max(DRUM_ROW_MIN, now + steps * DRUM_ROW_STEP));
  if (next === now) return false;
  const before = (now * scale) / 100;
  const after = (next * scale) / 100;
  const y = anchorY ?? prView.areaHeight / 2;
  drumView.rowPx = next;
  const top = drumView.topRow + y / before - y / after;
  drumView.topRow = clampTopRow(top, drumView.rows, Math.floor(prView.areaHeight / after));
  return true;
}

// ---- The reading while dragging (spec 2 "Reading in place": the same value is also shown on the status line) ----

/** Only while dragging, the text of the value being moved (「+0.0960 ・ +2 半音」 and so on). When this is set, the status line shows it instead of the core's text. */
export const dragReading = $state({ text: null as string | null });

// ---- "Cancel the drag" at the very front of the Esc order (spec 4 "Cancel the drag and go back to before the move") ----

let cancelers: Array<() => boolean> = [];

/** A part that has a drag registers here. Remove with the returned function. canceler returns true after cancelling if dragging, otherwise false. */
export function onDragCancel(canceler: () => boolean): () => void {
  cancelers.push(canceler);
  return () => {
    cancelers = cancelers.filter((c) => c !== canceler);
  };
}

/** On Esc, the key handler calls this first. True if any drag was cancelled (then nothing else is done). */
export function cancelDrag(): boolean {
  for (const c of cancelers) if (c()) return true;
  return false;
}

// ---- Horizontal zoom of the arrange (spec 3 "Horizontal zoom in and out"). Both the key handler (Mod ＋ / Mod −) and the arrange part use it, so it is here ----

/**
 * Steps of horizontal zoom of the arrange (px per bar at 100 % scale). 64 is mock F-Empty (mock.arrange.bar).
 * The same √2 steps as the piano roll (QUARTER_PX_STEPS). The lowest is 8 (120 bars fit in 960 px), the highest 256 (64 px per beat; finer than this is for the piano roll).
 */
export const ARRANGE_BAR_STEPS = [8, 11, 16, 23, 32, 45, 64, 91, 128, 181, 256] as const;
export const DEFAULT_ARRANGE_BAR = 64;

export const arrangeView = $state({
  /** px per bar at 100 % scale (one of ARRANGE_BAR_STEPS). */
  barPx: DEFAULT_ARRANGE_BAR as number,
});

/** The value moved by `by` steps from now in the list of steps (stops at the ends; from fallback if now is not in the list). */
export function stepFrom(steps: readonly number[], now: number, by: number, fallback: number): number {
  const at = steps.indexOf(now);
  const from = at < 0 ? steps.indexOf(fallback) : at;
  return steps[Math.min(steps.length - 1, Math.max(0, from + by))]!;
}

/** Moves the arrange's horizontal zoom by `by` steps. True if it changed. The scroll position is kept in ticks, so the left edge position stays. */
export function zoomArrangeBy(by: number): boolean {
  const next = stepFrom(ARRANGE_BAR_STEPS, arrangeView.barPx, by, DEFAULT_ARRANGE_BAR);
  if (next === arrangeView.barPx) return false;
  arrangeView.barPx = next;
  return true;
}

// ---- Whether the left and right panels (note list, selected notes) are open. Remembered when the band folds ----

/** null means not decided yet (decided when the band is first shown: if the note area is less than 4 bars (640), it starts folded. The band table in spec 3). */
export const prPanels = $state({ list: null as boolean | null, inspector: null as boolean | null });

// ---- Open CC lanes (spec 3: remembered when the band folds) ----

export type LaneTag = "sustain" | "pitchBend" | "modulation" | "other";

/** open is the order opened (oldest first). other is the number chosen with 「そのほかの CC」. */
export const ccLanes = $state({ open: [] as LaneTag[], other: null as number | null });

// ---- Whether the velocity lane is open (the 「ベロシティ」 row in the band table of spec 3: when folded, 「▸ ベロシティ」 at the left end of the CC tabs). Remembered when the band folds ----

export const velocityLane = $state({ open: true });

// ---- Whether 「長さをそろえる」「ベロシティをまとめて変える」 in the right panel and 「CC の一覧」 in the left panel are open. Remembered when the band folds ----

export const inspectorFolds = $state({ sameLength: false, velocityAll: false });
/** folded is the key of the folded kind headings (`ccGroupKey`). */
export const ccList = $state({ open: false, folded: [] as string[] });

// ---- 「ほかのトラックも見る」 (round 12 P2; ghosts.ts). Kept until the app closes, not saved ----

/**
 * on: the notes of other tracks are drawn. hidden: the tracks taken off in its list (every track the list offers is shown at first).
 * Track ids are per song, so hidden counts only while the song's serial is `serial` (`ghostHidden` in ghosts.ts).
 */
export const ghostView = $state({ on: false, hidden: [] as number[], serial: 0 });
