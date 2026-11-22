// 歌のクリップの波形の形（仕様書 3 章「波形」、2 章「歌のクリップ」、モック F7-AudioEdit ・ R11-Audio）。DOM も WebGL も知らない純粋な関数。
// クリップ 1 つに、波形の四角 1 つ（テンポが途中で変わる所では、まっすぐな所ごとに 1 つ）。ピークはテクスチャで、
// シェーダーが x からピークを読む（ui/src/gl/renderer.ts）。ここで決めるのは「どの x が、どのピークか」だけ。

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
 * 曲のティックと、ファイルのこまの合わせ（follow.ts の `FollowClip`）。`source` は、ついていくクリップの「この音のテンポ」の地図
 * （`cachedSourceMap`）、ついていかなければ（録っている歌も）null。
 */
export interface WaveTiming extends FollowClip {
  source: TempoMap | null;
}

/** 曲のティック `tick` で鳴る、ファイルのこま（小数）。クリップでは `audioClipGeom.ts` の `fileFrameAt` と同じ答え。 */
export const frameAtTick = (map: TempoMap, t: WaveTiming, tick: number) => clipFrameAt(map, t.source, t, tick);

/** 波形を描くクリップ（か、録っている歌）の、形に要る物。 */
export interface WaveClip {
  timing: WaveTiming;
  /** クリップのはし（ティック）。フェードと棒のならびは、ここから数える。 */
  start: number;
  end: number;
  /** ティック */
  fadeIn: number;
  fadeOut: number;
  /** 倍率（`gainFactor`） */
  gain: number;
  /** テイクが持っているのは、ファイルのこま 0 からここまで。録っている間は Infinity。 */
  endFrame: number;
}

/** クリップと、ドラッグしている間の値（`audioDrag.now`。そのクリップの物と、いっしょにずらすクリップの音量だけ効く）から。歌のクリップでなければ null。 */
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

/** 棒の形と色（CSS px）。 */
export interface WaveLook {
  /** 棒から次の棒まで（mock.wave.pitch） */
  pitch: number;
  /** 棒の幅（mock.wave.bar） */
  bar: number;
  /** 棒のいちばん低い高さ（mock.wave.min） */
  minH: number;
  color: ColorName;
  /** まん中の線（160 の行）。 */
  mid: ColorName | null;
}

/** ふつうの行は棒だけ、160 の行（`tall`）はまん中に gray.5 の線。棒の色はクリップの中の線の色（ミュートは gray.7、録音中は rec）。 */
export const waveLook = (sizes: Pick<WaveLook, "pitch" | "bar" | "minH">, color: ColorName, tall: boolean): WaveLook => ({ ...sizes, color, mid: tall ? WAVE.mid : null });

/** 見えている所の、1 px あたりのファイルのこま（テンポが変わる所では、ならした数）。 */
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
 * 描くピークの段: 棒 1 本（`pitch` px）の幅より細かい段のうち、いちばん粗い物。棒 1 本が読むピークは 1〜4 個になる
 * （いちばん粗い段より縮小したときは、もっと多い。シェーダーは 16 個まで読む）。
 */
export const waveLevel = (framesPerPixel: number, pitch: number) => levelFor(framesPerPixel * pitch);

/**
 * [from, to] を、こまがティックにまっすぐ（ずれが `tolerance` こま以内）な所に分ける切れ目（両はしをふくむ）。
 * テンポが変わらなければ分けない。だんだん変わる所は、半分ずつに割っていく（64 個まで）。
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
 * クリップの波形の四角を `out` に足す。`area` は波形の所（`waveArea`）、`after` は、この波形の下になる四角の数。
 * 描くのは、見えている幅の中で、`range`（録っているクリップの、できた所や先）があればその中だけ。
 * フェードの線（gray.10 の曲がった線）は、フェードがあるクリップにだけ。
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
  // 半 px ぶんのこまより大きくずれたら、分ける。
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

/** 波形を (dx, dy) だけずらす（小さな区画の箱の左上から数えなおすとき）。 */
export function shiftWaves(waves: GlWave[], dx: number, dy: number): void {
  for (const w of waves) {
    w.x += dx;
    w.y += dy;
    w.clipX0 += dx;
    w.clipX1 += dx;
  }
}
