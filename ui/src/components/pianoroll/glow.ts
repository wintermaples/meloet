// Notes that sound light up in the piano roll while playing (round 12 E1, 「鳴っている音を光らせる」). Pure functions that do not know the DOM.
// The lit notes are drawn on the small pianoRollLive canvas (Roll.svelte), never on the large pianoRoll one, and only when the set changes.
// Their box is as tall as the note area, so its top and bottom line up with the keyboard column and the play line, which change in the
// same frames. It starts at the lit notes and leaves room to their right, and stays where it is while the lit notes stay inside it, so
// most changes only draw the canvas again, in the same place and at the same size (no layout; `glowNext`). WebKit painting on the CPU
// repaints the whole bounding rectangle of a frame when what changed is 25 % of it or more (docs/architecture.md 7), so the lit notes
// fit in `glowMaxWidth`: the widest notes are left unlit, and a change whose old and new boxes together are wider is shown in two frames
// (first the notes that stay lit, then the new ones).

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

/** The step (CSS px) a new box's width goes by; it leaves one to two steps of room to the right of the lit notes, where the next notes come. */
export const GLOW_STEP = 32;

/** A box of lit notes: left and right (CSS px from the note area's left edge). */
export interface GlowBox {
  x0: number;
  x1: number;
}

const spanOf = (a: GlowBox, b: GlowBox) => Math.max(a.x1, b.x1) - Math.min(a.x0, b.x0);
const snapped = (b: GlowBox, dpr: number): GlowBox => ({ x0: Math.floor(b.x0 * dpr) / dpr, x1: Math.ceil(b.x1 * dpr) / dpr });

/**
 * A new box for lit notes around `lit`: from their left edge to one or two steps past their right edge, the width a whole number of steps,
 * at most `maxW` but never narrower than the notes; moved left to stay inside the note area (`areaW` wide). Edges on device pixels.
 */
export function glowPlace(lit: GlowBox, maxW: number, areaW: number, dpr: number): GlowBox {
  const { x0, x1 } = snapped(lit, dpr);
  const need = x1 - x0;
  const w = Math.max(need, Math.min(Math.ceil((need + GLOW_STEP) / GLOW_STEP) * GLOW_STEP, Math.floor(maxW * dpr) / dpr));
  const left = Math.max(0, Math.min(x0, Math.floor((areaW - w) * dpr) / dpr));
  return { x0: left, x1: left + w };
}

/** What to light next, in which box; `pending` when it is the first of a change shown in two frames. */
export interface GlowNext {
  lit: readonly GlowSpan[];
  box: GlowBox | null;
  pending: boolean;
}

/**
 * The next lit notes and their box, on the way from `shown` in the box `kept` to `target`. `fresh`: the view, the take or the pixel ratio changed
 * since `kept` was placed (the note area is drawn again then anyway).
 * - The box stays while it holds `target` and is at most one step wider than a new one would be; it also stays while nothing is lit.
 * - Else a new box (`glowPlace`), or one just around `target` when the new box and the kept one together would be wider than `maxW`.
 * - Else first only the notes of `target` lit now, in a box just around them (inside the kept one), and `target` in the next frame.
 * So the part of the screen one frame repaints for the lit notes is at most `maxW` wide (give or take the device pixels the edges snap to).
 */
export function glowNext(
  kept: GlowBox | null,
  shown: readonly GlowSpan[],
  target: readonly GlowSpan[],
  maxW: number,
  areaW: number,
  dpr: number,
  fresh: boolean,
): GlowNext {
  const lit = glowBox(target);
  if (!lit) return { lit: target, box: kept, pending: false };
  const placed = glowPlace(lit, maxW, areaW, dpr);
  if (!kept || fresh) return { lit: target, box: placed, pending: false };
  if (kept.x0 <= lit.x0 && lit.x1 <= kept.x1 && kept.x1 - kept.x0 <= placed.x1 - placed.x0 + GLOW_STEP) return { lit: target, box: kept, pending: false };
  const limit = maxW + 2 / dpr;
  if (spanOf(kept, placed) <= limit) return { lit: target, box: placed, pending: false };
  const tight = snapped(lit, dpr);
  if (spanOf(kept, tight) <= limit) return { lit: target, box: tight, pending: false };
  const was = new Set(shown.map((s) => s.note));
  const stay = target.filter((s) => was.has(s.note));
  const inner = glowBox(stay);
  return { lit: stay, box: inner && snapped(inner, dpr), pending: true };
}
