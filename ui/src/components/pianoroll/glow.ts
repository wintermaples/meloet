// Notes that sound light up in the piano roll while playing (round 12 E1, 「鳴っている音を光らせる」). Pure functions that do not know the DOM.
// The lit notes are drawn on the small pianoRollLive canvas (Roll.svelte), never on the large pianoRoll one, and only when the set changes.
// Their box is as tall as the note area, so its top and bottom line up with the keyboard column and the play line, which change in the
// same frames, and as wide as the lit notes. WebKit painting on the CPU repaints the whole bounding rectangle of a frame when what changed
// is 25 % of it or more (docs/architecture.md 7), so the box is kept at most `glowMaxWidth` wide: the widest notes are left unlit, and a
// change whose old and new boxes together are wider is shown in two frames (first the notes that stay lit, then the new ones).

import type { TrackView, TransportState } from "../../bridge";

/** The share of the window's area the box of lit notes may take: the rest of a frame while playing is about a tenth of it. */
export const GLOW_AREA = 0.15;

/** Whether the notes of `track` are heard: not muted, and soloed when any track is (the rule of the model, `Doc::plays`). */
export function trackPlays(tracks: readonly Pick<TrackView, "soloed">[], track: Pick<TrackView, "muted" | "soloed"> | null): boolean {
  if (!track || track.muted) return false;
  return track.soloed || !tracks.some((t) => t.soloed);
}

export interface GlowWhen {
  /** 「表示 ▸ 鳴っている音を光らせる」 (`WorkspaceView.noteGlow`). */
  setting: boolean;
  state: TransportState;
  /** The clip being recorded is shown (its notes are drawn as they grow, not the take). */
  recordingShown: boolean;
  /** Notes of the shown clip are being dragged (their marks show where they were). */
  dragging: boolean;
  tracks: readonly Pick<TrackView, "soloed">[];
  /** The track of the shown clip. */
  track: Pick<TrackView, "muted" | "soloed"> | null;
}

/** Whether sounding notes light now: the setting on, playing or recording (not in the count-in), and the shown clip's track heard. */
export function glowShows(w: GlowWhen): boolean {
  return (
    w.setting && (w.state === "playing" || w.state === "recording") && !w.recordingShown && !w.dragging && trackPlays(w.tracks, w.track)
  );
}

/** The widest the box of lit notes may be (CSS px): GLOW_AREA of the window's area, over the height of the note area. */
export function glowMaxWidth(windowW: number, windowH: number, areaH: number): number {
  return areaH > 0 ? (GLOW_AREA * windowW * windowH) / areaH : 0;
}

/** A lit note: its index in the take, and the part of it the note area shows (CSS px from the area's left edge). */
export interface GlowSpan {
  note: number;
  x0: number;
  x1: number;
}

/** The left and right of the box around `spans`; null when there are none. */
export function glowBox(spans: readonly GlowSpan[]): { x0: number; x1: number } | null {
  if (spans.length === 0) return null;
  let x0 = Infinity;
  let x1 = -Infinity;
  for (const s of spans) {
    x0 = Math.min(x0, s.x0);
    x1 = Math.max(x1, s.x1);
  }
  return { x0, x1 };
}

const widthOf = (spans: readonly GlowSpan[]) => {
  const box = glowBox(spans);
  return box ? box.x1 - box.x0 : 0;
};

/** The notes of `spans` to light: the widest are left out (the later note first between two as wide) until the box around the rest is at most `maxW` wide. In the order given. */
export function glowFit(spans: readonly GlowSpan[], maxW: number): readonly GlowSpan[] {
  if (widthOf(spans) <= maxW) return spans;
  const widest = [...spans].sort((a, b) => b.x1 - b.x0 - (a.x1 - a.x0) || b.note - a.note);
  const out = new Set(spans);
  for (const s of widest) {
    out.delete(s);
    if (widthOf([...out]) <= maxW) break;
  }
  return spans.filter((s) => out.has(s));
}

/** Whether two lists light the same notes. */
export function sameNotes(a: readonly GlowSpan[], b: readonly GlowSpan[]): boolean {
  return a.length === b.length && a.every((s, i) => s.note === b[i]!.note);
}

/**
 * What to show next on the way from `shown` to `target` (both placed in the same view): `target` when the box around both is at most `maxW`
 * wide; else only the notes of `target` that are shown now, whose box is inside the one shown, and `target` in the next frame. So the part
 * of the screen one frame repaints for the lit notes is never wider than `maxW` (when `shown` and `target` are each at most that wide).
 */
export function glowStep(shown: readonly GlowSpan[], target: readonly GlowSpan[], maxW: number): readonly GlowSpan[] {
  if (widthOf([...shown, ...target]) <= maxW) return target;
  const lit = new Set(shown.map((s) => s.note));
  return target.filter((s) => lit.has(s.note));
}
