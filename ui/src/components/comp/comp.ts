// 「いいとこ取り」 of a vocal clip (round 12, A8): the pieces of a comp and the shape of its band under the track.
// Pure functions that know neither the DOM nor WebGL. A piece is `{ from, take }`: from `from` (ticks from the clip's
// start) to the next piece's `from`, the take with that id sounds. The rules of a list of pieces are Rust's
// (`meloet_model::comp_is_valid`): the first at 0, in the order of `from`, inside the clip, no two in a row of one take.

import { COMP_ROWS_MAX, type AudioTakeView, type ClipView, type CompPiece } from "../../bridge";
import type { ColorName } from "../../design/tokens";
import type { GlRect } from "../../gl/slot";
import { tickToX, type View } from "../arrangeGeom";

/** What the clip plays: its comp, or the shown take all along. */
export const piecesOf = (clip: ClipView): CompPiece[] => (clip.comp.length > 0 ? clip.comp : [{ from: 0, take: clip.takeId }]);

export const samePieces = (a: readonly CompPiece[], b: readonly CompPiece[]) =>
  a.length === b.length && a.every((p, i) => p.from === b[i]!.from && p.take === b[i]!.take);

/**
 * `pieces` of a clip `length` ticks long with take `take` from `from` to `to` (ticks from the clip's start, put inside
 * the clip): what was there before and after stays, a piece left with no length goes, and neighbours of one take
 * become one piece. A range with no length changes nothing.
 */
export function chooseTake(pieces: readonly CompPiece[], length: number, from: number, to: number, take: number): CompPiece[] {
  const a = Math.max(0, Math.min(from, to));
  const b = Math.min(length, Math.max(from, to));
  if (!(b > a) || pieces.length === 0) return [...pieces];
  const cut: CompPiece[] = [];
  for (const [i, p] of pieces.entries()) {
    const end = pieces[i + 1]?.from ?? length;
    // What is left of the piece before the range and after it.
    if (p.from < a) cut.push({ from: p.from, take: p.take });
    if (end > b) cut.push({ from: Math.max(p.from, b), take: p.take });
  }
  cut.push({ from: a, take });
  cut.sort((x, y) => x.from - y.from);
  const out: CompPiece[] = [];
  for (const p of cut) {
    if (out.at(-1)?.take !== p.take) out.push(p);
  }
  return out;
}

/** The takes the band has rows for: the newest first, `COMP_ROWS_MAX` at most. */
export const rowTakes = (takes: readonly AudioTakeView[]): AudioTakeView[] => takes.slice(-COMP_ROWS_MAX).reverse();

/** A clip the band can be opened for: a vocal clip of two takes or more. */
export const compable = (clip: ClipView) => clip.audio !== null && clip.audio.takes.length >= 2;

// ---- The band: a heading row, then a row for each take ----

export interface BandDims {
  /** The heading row (size.row) */
  headH: number;
  /** A take's row (size.lane) */
  rowH: number;
  /** Above and below the bar of a take in its row (space.1) */
  pad: number;
  /** line.hair */
  hair: number;
}

export const bandHeight = (d: Pick<BandDims, "headH" | "rowH">, rows: number) => d.headH + rows * d.rowH;

/** The row of the band at `y` px from its top; -1 on the heading and below the last row. */
export function rowAt(d: BandDims, rows: number, y: number): number {
  const row = Math.floor((y - d.headH) / d.rowH);
  return y >= d.headH && row < rows ? row : -1;
}

export interface TakeBar {
  x: number;
  y: number;
  w: number;
  h: number;
  take: AudioTakeView;
  /** The parts of the bar where this take sounds: px from the band's left. */
  chosen: { x0: number; x1: number }[];
}

const GROUND: ColorName = "color.gray.5";
const CHOSEN: ColorName = "color.accent.ink";
/** The waves on the bars: light on the gray ground, dark on the accent. */
export const WAVE_PLAIN: ColorName = "color.gray.8";
export const WAVE_CHOSEN: ColorName = "color.gray.0";

/** The bar of each row (the clip's width, in its row) and where its take is chosen by `pieces`. */
export function takeBars(v: View, d: BandDims, clip: ClipView, takes: readonly AudioTakeView[], pieces: readonly CompPiece[]): TakeBar[] {
  const x0 = tickToX(v, clip.start);
  const x1 = tickToX(v, clip.end);
  return takes.map((take, row) => {
    const chosen: TakeBar["chosen"] = [];
    for (const [i, p] of pieces.entries()) {
      if (p.take !== take.takeId) continue;
      const end = pieces[i + 1]?.from ?? clip.end - clip.start;
      chosen.push({ x0: tickToX(v, clip.start + p.from), x1: tickToX(v, clip.start + end) });
    }
    return { x: x0, y: d.headH + row * d.rowH + d.pad, w: x1 - x0, h: d.rowH - 2 * d.pad, take, chosen };
  });
}

/** The rectangles under the waves: each row's bar (gray.5), accent where its take is chosen. */
export function bandRects(bars: readonly TakeBar[]): GlRect[] {
  const out: GlRect[] = [];
  for (const b of bars) {
    out.push({ x: b.x, y: b.y, w: b.w, h: b.h, color: GROUND });
    for (const c of b.chosen) out.push({ x: c.x0, y: b.y, w: c.x1 - c.x0, h: b.h, color: CHOSEN });
  }
  return out;
}

/**
 * Over the waves, while a drag changes the pieces: the parts that were chosen before it (`was`), each with a gray.7
 * hairline around it.
 */
export function wasFrames(d: BandDims, was: readonly TakeBar[] | null): GlRect[] {
  const out: GlRect[] = [];
  const line: ColorName = "color.gray.7";
  for (const b of was ?? []) {
    for (const c of b.chosen) {
      const w = c.x1 - c.x0;
      out.push({ x: c.x0, y: b.y, w, h: d.hair, color: line });
      out.push({ x: c.x0, y: b.y + b.h - d.hair, w, h: d.hair, color: line });
      out.push({ x: c.x0, y: b.y, w: d.hair, h: b.h, color: line });
      out.push({ x: c.x1 - d.hair, y: b.y, w: d.hair, h: b.h, color: line });
    }
  }
  return out;
}

// ---- The drag over a row ----

export interface CompDrag {
  take: number;
  /** Ticks of the song: where it was pressed and where the pointer is, each snapped and inside the clip. */
  from: number;
  to: number;
}

/** A tick of the song for a drag's end: on the grid (`snap`; none with Shift), inside the clip. */
export function dragTick(clip: ClipView, tick: number, snap: ((tick: number) => number) | null): number {
  return Math.min(clip.end, Math.max(clip.start, snap ? snap(tick) : Math.round(tick)));
}

/** What the clip would play when the drag ends now. */
export function draggedPieces(clip: ClipView, drag: CompDrag): CompPiece[] {
  return chooseTake(piecesOf(clip), clip.end - clip.start, drag.from - clip.start, drag.to - clip.start, drag.take);
}
