// The waveform shape of an audio clip (spec 3 (93-4 Waveform), spec 2 "Audio clip", mock F7-AudioEdit / R11-Audio). Pure functions that know neither the DOM nor WebGL.
// One waveform rect per clip (where the tempo changes inside it, one per straight part). The peaks are a texture,
// and the shader reads the peak for each x (ui/src/gl/renderer.ts). This file only decides which x reads which peak.

import type { ClipView } from "../../bridge";
import { levelFor } from "../../bridge/peaks";
import type { ColorName } from "../../design/tokens";
import { WAVE } from "../../gl/dims";
import type { GlWave } from "../../gl/slot";
import type { WaveSource } from "../../gl/wave";
import type { TempoMap } from "../../time/tempoMap";
import { tickToX, xToTick, type View } from "../arrangeGeom";
import { clampGain, gainFactor, type WaveArea } from "./audioClipGeom";
import type { AudioDragValue } from "./audioDrag.svelte";
import { cachedSourceMap, clipFrameAt, type FollowClip } from "./follow";

/**
 * How song ticks match file frames (`FollowClip` in follow.ts). `source` is the map of the clip's own tempo for a clip that follows
 * the song tempo (`cachedSourceMap`), or null when it does not follow (also for audio being recorded).
 */
export interface WaveTiming extends FollowClip {
  source: TempoMap | null;
}

/** The file frame (fractional) heard at song tick `tick`. For a clip, the same answer as `fileFrameAt` in `audioClipGeom.ts`. */
export const frameAtTick = (map: TempoMap, t: WaveTiming, tick: number) => clipFrameAt(map, t.source, t, tick);

/** What the shape needs from a clip (or audio being recorded) whose waveform is drawn. */
export interface WaveClip {
  timing: WaveTiming;
  /** The clip edges (ticks). Fades and the row of bars are counted from here. */
  start: number;
  end: number;
  /** Ticks */
  fadeIn: number;
  fadeOut: number;
  /** Factor (`gainFactor`) */
  gain: number;
  /** The take holds file frames from 0 up to here. Infinity while recording. */
  endFrame: number;
}

/** From the clip and the values during a drag (`audioDrag.now`: only its own values and the gain of clips moved with it count). null if it is not an audio clip. */
export function waveClipOf(clip: ClipView, drag: AudioDragValue | null = null): WaveClip | null {
  const a = clip.audio;
  if (a === null) return null;
  const d = drag?.clip === clip.id ? drag : null;
  const shifted = d === null && drag?.shift?.clips.includes(clip.id) ? clampGain(a.gainDb + drag.shift.byDb) : null;
  return {
    timing: { start: clip.start, takeOffset: clip.takeOffset, firstFrame: a.firstFrame, sampleRate: a.sampleRate, source: cachedSourceMap(a.sourceTempo) },
    start: clip.start,
    end: clip.end,
    fadeIn: d?.fadeIn ?? a.fadeIn,
    fadeOut: d?.fadeOut ?? a.fadeOut,
    gain: gainFactor(d?.gainDb ?? shifted ?? a.gainDb),
    endFrame: a.firstFrame + a.frames,
  };
}

/** Bar shape and color (CSS px). */
export interface WaveLook {
  /** From one bar to the next (mock.wave.pitch) */
  pitch: number;
  /** Bar width (mock.wave.bar) */
  bar: number;
  /** Lowest bar height (mock.wave.min) */
  minH: number;
  color: ColorName;
  /** The center line (160 row). */
  mid: ColorName | null;
}

/** A normal row has only bars; a 160 row (`tall`) also has a gray.5 center line. Bars use the clip's line color (gray.7 when muted, rec while recording). */
export const waveLook = (sizes: Pick<WaveLook, "pitch" | "bar" | "minH">, color: ColorName, tall: boolean): WaveLook => ({ ...sizes, color, mid: tall ? WAVE.mid : null });

/** File frames per px in the visible part (an average where the tempo changes). */
export function framesPerPx(map: TempoMap, v: View, timing: WaveTiming, from: number, to: number): number {
  let a = Math.max(from, xToTick(v, 0));
  let b = Math.min(to, xToTick(v, v.width));
  if (!(b > a)) {
    a = from;
    b = from + 1;
  }
  return (frameAtTick(map, timing, b) - frameAtTick(map, timing, a)) / ((b - a) * v.pxPerTick);
}

/**
 * The peak level to draw: the coarsest level that is still finer than one bar (`pitch` px). One bar reads 1-4 peaks
 * (more when zoomed out past the coarsest level; the shader reads up to 16).
 */
export const waveLevel = (framesPerPixel: number, pitch: number) => levelFor(framesPerPixel * pitch);

/**
 * Cut points (both ends included) that split [from, to] into parts where frames are straight against ticks (off by at most `tolerance` frames).
 * No split when the tempo does not change. Ramps are halved again and again (up to 64 parts).
 */
export function straightParts(frame: (tick: number) => number, from: number, to: number, tolerance: number, depth = 6): number[] {
  const out = [from];
  const cut = (a: number, fa: number, b: number, fb: number, left: number) => {
    let bent = false;
    for (const part of [0.25, 0.5, 0.75]) {
      if (Math.abs(frame(a + (b - a) * part) - (fa + (fb - fa) * part)) > tolerance) bent = true;
    }
    if (bent && left > 0) {
      const m = (a + b) / 2;
      const fm = frame(m);
      cut(a, fa, m, fm, left - 1);
      cut(m, fm, b, fb, left - 1);
    } else out.push(b);
  };
  if (to > from) cut(from, frame(from), to, frame(to), depth);
  return out;
}

/**
 * Adds the clip's waveform rects to `out`. `area` is the waveform area (`waveArea`); `after` is the number of rects under this waveform.
 * Draws only inside the visible width, and inside `range` (the done part of a clip being recorded, and so on) if given.
 * The fade line (a curved gray.10 line) only for clips with a fade.
 */
export function clipWaves(
  out: GlWave[],
  after: number,
  map: TempoMap,
  v: View,
  area: WaveArea,
  c: WaveClip,
  source: WaveSource,
  framesPerPeak: number,
  look: WaveLook,
  range: { from: number; to: number } | null = null,
): void {
  const x0 = Math.max(area.x0, range ? tickToX(v, range.from) : -Infinity, -look.pitch);
  const x1 = Math.min(area.x1, range ? tickToX(v, range.to) : Infinity, v.width + look.pitch);
  if (!(x1 > x0) || !(area.bottom > area.top)) return;
  const frame = (tick: number) => frameAtTick(map, c.timing, tick);
  const t0 = xToTick(v, x0);
  const t1 = xToTick(v, x1);
  // Split when it is off by more than half a px of frames.
  const tolerance = Math.abs(frame(t1) - frame(t0)) / (2 * (x1 - x0));
  const cuts = straightParts(frame, t0, t1, tolerance);
  const faded = c.fadeIn > 0 || c.fadeOut > 0;
  for (let i = 0; i + 1 < cuts.length; i++) {
    const a = cuts[i]!;
    const b = cuts[i + 1]!;
    const ax = tickToX(v, a);
    const bx = tickToX(v, b);
    if (!(bx > ax)) continue;
    const fa = frame(a);
    out.push({
      after,
      x: ax,
      y: area.top,
      w: bx - ax,
      h: area.bottom - area.top,
      source,
      peak: fa / framesPerPeak,
      peaksPerPx: (frame(b) - fa) / framesPerPeak / (bx - ax),
      first: 0,
      end: Math.ceil(c.endFrame / framesPerPeak),
      gain: c.gain,
      clipX0: tickToX(v, c.start),
      clipX1: tickToX(v, c.end),
      fadeIn: c.fadeIn * v.pxPerTick,
      fadeOut: c.fadeOut * v.pxPerTick,
      pitch: look.pitch,
      bar: look.bar,
      minH: look.minH,
      color: look.color,
      mid: look.mid,
      line: faded ? WAVE.fade : null,
    });
  }
}

/** Moves the waveform by (dx, dy) (when counting again from the top left of a small slot box). */
export function shiftWaves(waves: GlWave[], dx: number, dy: number): void {
  for (const w of waves) {
    w.x += dx;
    w.y += dy;
    w.clipX0 += dx;
    w.clipX1 += dx;
  }
}
