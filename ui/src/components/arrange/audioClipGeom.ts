// 歌のクリップの幾何（仕様書 2 章「歌のクリップ」、3 章 27-A ・ 29-A ・ 30-C、モック F7-AudioEdit ・ F7-AudioDrop ・ R11-Audio）。
// 波形を描く WebGL の係と、取っ手 ・ 音量の線 ・ 読み値 ・ 落とす所の枠を出す HTML の係が、同じ数を読むための 1 か所。
// どれも CSS px。x と y はアレンジのクリップの面（arrangeLanes）の左上から。

import type { ClipView, DropMode, TrackView } from "../../bridge";
import { CLIP_GAIN_MAX_DB, CLIP_GAIN_MIN_DB } from "../../bridge/types";
import type { TempoMap } from "../../time/tempoMap";
import { clipBox, rowHeight, rowTop, tickToX, trackRowAt, xToTick, type Box, type LaneDims, type View } from "../arrangeGeom";
import { fadeGain } from "./fade";

/** 歌のクリップの形のうち、幾何に要る物（`ClipView` と `AudioClipView` から）。 */
export interface AudioClipShape {
  start: number;
  end: number;
  fadeIn: number;
  fadeOut: number;
}

export const audioShapeOf = (clip: ClipView): AudioClipShape | null =>
  clip.audio === null ? null : { start: clip.start, end: clip.end, fadeIn: clip.audio.fadeIn, fadeOut: clip.audio.fadeOut };

/** 波形 ・ フェードの線 ・ 音量の線が入る所: 名前の行の下から、下の枠の上まで。 */
export interface WaveArea {
  x0: number;
  x1: number;
  top: number;
  bottom: number;
  /** 波形のまん中の線（160 の行では gray.5）。 */
  mid: number;
}

export function waveArea(d: LaneDims, box: Box): WaveArea {
  const top = Math.min(box.y0 + d.mark + d.nameH, box.y1 - d.hair);
  const bottom = box.y1 - d.hair;
  return { x0: box.x0 + d.hair, x1: box.x1 - d.hair, top, bottom, mid: (top + bottom) / 2 };
}

// ---- 音量の線（accent の 1px。上下にドラッグして −60〜+12 dB） ----

/** dB → 線の y。dB でまっすぐ: 上のはしが +12、下のはしが −60。 */
export function gainLineY(area: WaveArea, db: number): number {
  const part = (CLIP_GAIN_MAX_DB - clampGain(db)) / (CLIP_GAIN_MAX_DB - CLIP_GAIN_MIN_DB);
  return area.top + part * (area.bottom - area.top);
}

/** 線の y → dB（範囲の中、0.1 きざみ: 読み値「−3.0 dB」と同じ細かさ）。 */
export function gainAtY(area: WaveArea, y: number): number {
  const height = area.bottom - area.top;
  if (height <= 0) return 0;
  const part = Math.min(1, Math.max(0, (y - area.top) / height));
  return clampGain(Math.round((CLIP_GAIN_MAX_DB - part * (CLIP_GAIN_MAX_DB - CLIP_GAIN_MIN_DB)) * 10) / 10);
}

export const clampGain = (db: number) => Math.min(CLIP_GAIN_MAX_DB, Math.max(CLIP_GAIN_MIN_DB, db));

/** クリップの音量（dB）を、波形の高さにかける倍率に。 */
export const gainFactor = (db: number) => 10 ** (clampGain(db) / 20);

// ---- フェード（上の左右の角の取っ手 6×6 を横にひっぱる。線は fade.ts の形） ----

export type FadeEdge = "fadeIn" | "fadeOut";

/** フェードインの終わりの x と、フェードアウトの頭の x。 */
export function fadeXs(v: View, clip: AudioClipShape): { inX: number; outX: number } {
  return { inX: tickToX(v, clip.start + clip.fadeIn), outX: tickToX(v, clip.end - clip.fadeOut) };
}

/**
 * 取っ手の箱（`size` は part.clip.edge の 6）。フェードの切れ目の x をまん中に、名前の行の下の線をまたぐ高さ。
 * フェードがない（0）ときは、クリップの角にぴったり。
 */
export function fadeHandleBox(v: View, d: LaneDims, box: Box, clip: AudioClipShape, edge: FadeEdge, size: number): Box {
  const area = waveArea(d, box);
  const xs = fadeXs(v, clip);
  const center = edge === "fadeIn" ? xs.inX : xs.outX;
  const x0 = Math.min(Math.max(center - size / 2, area.x0), Math.max(area.x0, area.x1 - size));
  const y0 = area.top - size / 3;
  return { x0, x1: x0 + size, y0, y1: y0 + size };
}

/** 取っ手を x までひっぱったときのフェードの長さ（ティック）。0 から「クリップの長さ − もう片方のフェード」まで。 */
export function fadeTicksAt(v: View, clip: AudioClipShape, edge: FadeEdge, x: number): number {
  const tick = xToTick(v, x);
  const other = edge === "fadeIn" ? clip.fadeOut : clip.fadeIn;
  const most = Math.max(0, clip.end - clip.start - other);
  const ticks = edge === "fadeIn" ? tick - clip.start : clip.end - tick;
  return Math.round(Math.min(most, Math.max(0, ticks)));
}

/** 大きさ（0〜1）→ フェードの線の y: 0 が下のはし、1 が上のはし。 */
export const fadeLineY = (area: WaveArea, gain: number) => area.bottom - gain * (area.bottom - area.top);

/**
 * フェードの線の点（左から右へ）。`stepPx` ごとに 1 点と、両はし。フェードがなければ空。
 * 見えている幅の外は切らない（短い線なので）。
 */
export function fadeLinePoints(v: View, area: WaveArea, clip: AudioClipShape, edge: FadeEdge, stepPx = 2): { x: number; y: number }[] {
  const length = edge === "fadeIn" ? clip.fadeIn : clip.fadeOut;
  if (length <= 0) return [];
  const { inX, outX } = fadeXs(v, clip);
  const from = edge === "fadeIn" ? tickToX(v, clip.start) : outX;
  const to = edge === "fadeIn" ? inX : tickToX(v, clip.end);
  const width = to - from;
  const count = Math.max(1, Math.ceil(width / stepPx));
  const points: { x: number; y: number }[] = [];
  for (let i = 0; i <= count; i++) {
    const part = i / count;
    points.push({ x: from + part * width, y: fadeLineY(area, fadeGain(edge === "fadeIn" ? part : 1 - part)) });
  }
  return points;
}

// ---- 波形の場所（テイクの中のどこが、クリップのどこに来るか） ----

/**
 * 曲のティック `tick` で鳴る、ファイルのこま（小数）。テイクの 0 ティック（`clip.start + clip.takeOffset`）が
 * `firstFrame`。音はファイルの速さのまま鳴るので、間は秒で数える（約束 24 の `AudioClipView` の決まり）。
 */
export function fileFrameAt(map: TempoMap, clip: ClipView, tick: number): number {
  const audio = clip.audio;
  if (audio === null) return 0;
  const zero = clip.start + clip.takeOffset;
  return audio.firstFrame + (map.secondsAt(tick) - map.secondsAt(zero)) * audio.sampleRate;
}

// ---- 歌を直すときの行（160） ----

/** トラックの行の高さ: 歌を直している行だけ `tall`（part.track.vocal の 160）、ほかは `d.trackH`。 */
export function rowHeights(d: LaneDims, tracks: readonly Pick<TrackView, "id">[], tallTrack: number | null, tall: number): number[] {
  return tracks.map((t) => (t.id === tallTrack ? tall : d.trackH));
}

// ---- 音のファイルを持ってきている間（29-A ・ 30-C） ----

/** まん中の箱のどちら側に乗っているか: 窓の左半分が「別々のトラックに」、右半分が「1 本にならべる」。 */
export const dropModeAt = (pageX: number, windowWidth: number): DropMode => (pageX < windowWidth / 2 ? "separate" : "inRow");

export interface DropTarget {
  /** 落とした所の歌のトラック（`TrackView.id`）。鍵盤のトラックの上か、トラックのない所なら null（新しい歌のトラック）。 */
  track: number | null;
  /** 枠を出す行。新しいトラックなら、いちばん下の行の次。 */
  row: number;
  /** 拍に吸いつけたティック。 */
  tick: number;
}

/** ポインタの下の、音のファイルが入る所。`snap` は拍に吸いつける関数（`nearestBeatTick`）。 */
export function dropTarget(v: View, d: LaneDims, tracks: readonly TrackView[], x: number, y: number, snap: (tick: number) => number): DropTarget {
  const tick = Math.max(0, snap(Math.max(0, xToTick(v, x))));
  const row = trackRowAt(d, y);
  const under = row >= 0 ? tracks[row] : undefined;
  if (under && under.kind.kind === "vocal") return { track: under.id, row, tick };
  return { track: null, row: tracks.length, tick };
}

/**
 * 入る所の gray.7 の枠の箱。長さがまだ分からないので、`ticks`（1 小節など）の幅で出す。`rows` はトラックの数。
 * 新しいトラック（`target.row === rows`）は、いちばん下の行のすぐ下に、ふつうの行の高さで。
 */
export function dropFrameBox(v: View, d: LaneDims, target: DropTarget, rows: number, ticks: number): Box {
  if (target.row < rows) return clipBox(v, d, target.row, target.tick, target.tick + ticks);
  const top = rows === 0 ? 0 : rowTop(d, rows - 1) + rowHeight(d, rows - 1);
  return { x0: tickToX(v, target.tick), x1: tickToX(v, target.tick + ticks), y0: top + d.inset, y1: top + d.trackH - d.hair - d.inset };
}
