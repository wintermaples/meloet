// 「ほかのトラックも見る」 (round 12 P2): the notes of other tracks, drawn faintly in the note area. Pure functions that do not know the DOM.
// Which tracks, which clips (a window three visible widths wide, as Arrange's lanePicture), and the picture: one color.gray.6 rectangle per note,
// no edge, drawn by paintRoll after the grid lines and before the shown clip's notes. They cannot be pressed: nothing here is used by hit tests.

import type { ClipView, TakeNotes, TrackView } from "../../bridge";
import type { Batch } from "../../gl/batch";
import { rgb } from "../../gl/color";
import { HAIR, type Box } from "../../gl/paint";
import { trackDrums } from "./drums";
import { eachNoteIn, noteEnd, noteStart, shownOf, type RollGeom, type Shown } from "./geom";

/** The most rectangles of other tracks in one picture. Past it, the notes farthest from the play line are left out. */
export const GHOST_MAX = 20000;

/** The track the piano roll shows: the shown clip's, or where writing makes a clip (keysTarget) when there is no clip. */
export function shownTrackId(clip: ClipView | null, track: TrackView | null, keysTarget: number | null): number | null {
  return clip ? (track?.id ?? null) : keysTarget;
}

/** The tracks the list offers: key tracks that are not drums, other than the track the piano roll shows. */
export function ghostCandidates(tracks: readonly TrackView[], current: number | null): TrackView[] {
  return tracks.filter((t) => t.id !== current && t.kind.kind === "keys" && !trackDrums(t));
}

/** The tracks taken off in the list, for the song whose serial is `serial` (ids of another song count for nothing). */
export function ghostHidden(view: { hidden: readonly number[]; serial: number }, serial: number): readonly number[] {
  return view.serial === serial ? view.hidden : [];
}

/** The tracks drawn: those the list offers that were not taken off. */
export function ghostTracks(tracks: readonly TrackView[], current: number | null, hidden: readonly number[]): TrackView[] {
  return ghostCandidates(tracks, current).filter((t) => !hidden.includes(t.id));
}

/** `hidden` with `track` taken off the list, or put back on. */
export function toggleGhostTrack(hidden: readonly number[], track: number): number[] {
  return hidden.includes(track) ? hidden.filter((id) => id !== track) : [...hidden, track];
}

/** Which window the left edge is in (it changes only when the left edge moves by a whole visible width). */
export function ghostWindowIndex(scrollTick: number, ppt: number, width: number): number {
  return Math.floor((scrollTick * ppt) / Math.max(1, width));
}

/** The window of three visible widths (one to the left, two to the right) in ticks. */
export function ghostWindow(index: number, ppt: number, width: number): { from: number; to: number } {
  const w = Math.max(1, width) / ppt;
  return { from: (index - 1) * w, to: (index + 2) * w };
}

/** The clips of notes on `tracks` that overlap ticks [from, to). Audio clips have no notes. */
export function ghostClips(tracks: readonly TrackView[], from: number, to: number): ClipView[] {
  const out: ClipView[] = [];
  for (const t of tracks) for (const c of t.clips) if (!c.audio && c.start < to && c.end > from) out.push(c);
  return out;
}

/** A clip's shown take, read at song positions: tick 0 of the take is at clip.start + takeOffset, and only notes that start inside the clip count. */
export function ghostShown(take: TakeNotes, clip: Pick<ClipView, "start" | "end" | "takeOffset">, had?: Shown): Shown {
  const base = clip.start + clip.takeOffset;
  if (had && had.take === take && had.base === base && had.from === clip.start && had.to === clip.end) return had;
  return shownOf(take, base, { from: clip.start, to: clip.end });
}

/** What paintRoll needs to draw them. */
export interface GhostPicture {
  notes: readonly Shown[];
  /** Where the play line was when the picture was made (to choose what to leave out past GHOST_MAX). */
  playTick: number;
}

/** How far a note is from the play line (0 when the line is inside it), in ticks. */
const fromPlay = (start: number, end: number, play: number) => (start > play ? start - play : end < play ? play - end : 0);

/** One rectangle per note in view: from 1px right of the start line to 1px left of the end line (the same place as a note's face and edge), height noteH. */
export function paintGhosts(out: Batch, box: Box, g: RollGeom, p: GhostPicture, noteH: number, dpr: number): void {
  if (p.notes.length === 0) return;
  const rowH = g.rowH * dpr;
  const ppt = g.ppt * dpr;
  const from = g.scrollTick;
  const to = g.scrollTick + g.width / g.ppt;
  const rowTop = (key: number) => Math.round(box.y0 + (g.topKey - key) * rowH);
  const inView = (y: number) => !(y > box.y1 || y + rowH < box.y0);

  let count = 0;
  for (const n of p.notes) {
    eachNoteIn(n, from, to, (i) => {
      if (inView(rowTop(n.take.keys[i]!))) count++;
    });
  }
  if (count === 0) return;

  // Past GHOST_MAX: the distance of the GHOST_MAX-th nearest note is the limit; of the notes exactly at it, the first ones met are drawn.
  let limit = Infinity;
  let atLimit = Infinity;
  if (count > GHOST_MAX) {
    const far = new Float64Array(count);
    let k = 0;
    for (const n of p.notes) {
      eachNoteIn(n, from, to, (i) => {
        if (inView(rowTop(n.take.keys[i]!))) far[k++] = fromPlay(noteStart(n, i), noteEnd(n, i), p.playTick);
      });
    }
    far.sort();
    limit = far[GHOST_MAX - 1]!;
    let below = 0;
    while (far[below]! < limit) below++;
    atLimit = GHOST_MAX - below;
  }

  const color = rgb("color.gray.6");
  for (const n of p.notes) {
    eachNoteIn(n, from, to, (i) => {
      const y = rowTop(n.take.keys[i]!);
      if (!inView(y)) return;
      const start = noteStart(n, i);
      const end = noteEnd(n, i);
      if (limit !== Infinity) {
        const d = fromPlay(start, end, p.playTick);
        if (d > limit) return;
        if (d === limit) {
          if (atLimit <= 0) return;
          atLimit--;
        }
      }
      const a = Math.round(box.x0 + (start - g.scrollTick) * ppt) + HAIR;
      const b = Math.max(a + HAIR, Math.round(box.x0 + (end - g.scrollTick) * ppt) - HAIR);
      out.rect(a, y, b, y + noteH, color);
    });
  }
}
