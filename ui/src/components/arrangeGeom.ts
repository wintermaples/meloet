// アレンジの形の計算（クリップの絵、テイクの札、目盛のドラッグ、ページ送り、全体図）。DOM を知らない純粋な関数。
// 位置は CSS px。x は「曲の頭からの px − 横のスクロール」。y はトラックの行の並びの上からの px。

import type { ClipView, Command, LiveNote, TickSpan, TrackView } from "../bridge";
import type { ColorName } from "../design/tokens";
import type { GlRect } from "../gl/slot";

/** その率での寸法（CSS px）。 */
export interface LaneDims {
  /** size.track: 行の高さ（下の線をふくむ） */
  trackH: number;
  /** 行の上下からクリップまで */
  inset: number;
  /** line.hair */
  hair: number;
  /** line.mark: クリップの上辺 */
  mark: number;
  /** text.s の行の高さ: 名前の行 */
  nameH: number;
  /** space.1: 名前の行とノートの間、ノートと下の枠の間 */
  pad: number;
  /** ノートの絵の高さ（mock.arrange.note） */
  noteH: number;
}

export interface View {
  /** 1 ティックの px */
  pxPerTick: number;
  scrollX: number;
  /** 見えている幅 */
  width: number;
}

export const tickToX = (v: View, tick: number) => tick * v.pxPerTick - v.scrollX;
export const xToTick = (v: View, x: number) => (x + v.scrollX) / v.pxPerTick;

/** クリップの絵の中のノート。start は曲の中のティック。 */
export interface NoteShape {
  start: number;
  length: number;
  key: number;
}

export interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** トラックの行 `row`（0 から）の、ティック [start, end) のクリップの箱。 */
export function clipBox(v: View, d: LaneDims, row: number, start: number, end: number): Box {
  return {
    x0: tickToX(v, start),
    x1: tickToX(v, end),
    y0: row * d.trackH + d.inset,
    y1: (row + 1) * d.trackH - d.hair - d.inset,
  };
}

export interface ClipColors {
  frame: ColorName;
  top: ColorName;
  note: ColorName;
}

export const CLIP_PLAIN = (track: ColorName): ClipColors => ({
  frame: "color.gray.6",
  top: track,
  note: "color.gray.8",
});
export const CLIP_SELECTED: ClipColors = {
  frame: "color.accent",
  top: "color.accent",
  note: "color.gray.8",
};
export const CLIP_RECORDING: ClipColors = {
  frame: "color.rec",
  top: "color.rec",
  note: "color.rec",
};

export function trackColor(track: Pick<TrackView, "color">): ColorName {
  return `color.track.${((track.color - 1) % 5) + 1}` as ColorName;
}

/** 鍵の範囲（いちばん低い・高い鍵）。ノートがなければ null。 */
export function keyRange(notes: Iterable<NoteShape>): { low: number; high: number } | null {
  let low = Infinity;
  let high = -Infinity;
  for (const n of notes) {
    low = Math.min(low, n.key);
    high = Math.max(high, n.key);
  }
  return low <= high ? { low, high } : null;
}

export interface ClipPaintOptions {
  /** 左の枠を描かない（録っているクリップの、先に描いた部分の続き） */
  openLeft?: boolean;
  /** 右の枠を描かない（続きは別の面が描く） */
  openRight?: boolean;
  /** ノートの鍵の範囲。なければ `notes` から */
  range?: { low: number; high: number } | null;
  /** ノートをこのティックまでで切る（押している鍵） */
  until?: number;
}

/**
 * クリップ 1 つの四角: 枠、上辺、面、ノートの小さな絵（名前の行の下、鍵の範囲を高さいっぱいに）。
 * 見えている幅の外は描かない。
 */
export function paintClip(
  out: GlRect[],
  v: View,
  d: LaneDims,
  box: Box,
  colors: ClipColors,
  notes: readonly NoteShape[],
  o: ClipPaintOptions = {},
): void {
  const { y0, y1 } = box;
  const x0 = Math.max(box.x0, -d.hair);
  const x1 = Math.min(box.x1, v.width + d.hair);
  if (x1 <= x0 || y1 <= y0) return;
  const left = o.openLeft || box.x0 < x0 ? 0 : d.hair;
  const right = o.openRight || box.x1 > x1 ? 0 : d.hair;
  out.push({ x: x0, y: y0, w: x1 - x0, h: y1 - y0, color: colors.frame });
  out.push({ x: x0, y: y0, w: x1 - x0, h: d.mark, color: colors.top });
  out.push({
    x: x0 + left,
    y: y0 + d.mark,
    w: x1 - x0 - left - right,
    h: y1 - y0 - d.mark - d.hair,
    color: "color.gray.4",
  });

  const range = o.range === undefined ? keyRange(notes) : o.range;
  if (!range) return;
  const top = y0 + d.mark + d.nameH + d.pad;
  const bottom = y1 - d.hair - d.pad - d.noteH;
  if (bottom < top) return;
  const span = range.high - range.low;
  const inL = box.x0 + (o.openLeft ? 0 : d.hair);
  const inR = box.x1 - (o.openRight ? 0 : d.hair);
  for (const n of notes) {
    const end = o.until === undefined ? n.start + n.length : Math.min(n.start + n.length, o.until);
    const nx0 = Math.max(tickToX(v, n.start), inL, x0);
    const nx1 = Math.min(Math.max(tickToX(v, end), tickToX(v, n.start) + d.hair), inR, x1);
    if (nx1 <= nx0) continue;
    const y = span === 0 ? (top + bottom) / 2 : top + ((range.high - n.key) / span) * (bottom - top);
    out.push({ x: nx0, y, w: nx1 - nx0, h: d.noteH, color: colors.note });
  }
}

/** 枠だけ（仕様書 2 章「クリップ」の「動かしている（もとの場所に gray.7 の枠）」）。見えている幅の外の辺は描かない。 */
export function paintFrame(out: GlRect[], v: View, d: LaneDims, box: Box, color: ColorName): void {
  const { y0, y1 } = box;
  const x0 = Math.max(box.x0, -d.hair);
  const x1 = Math.min(box.x1, v.width + d.hair);
  if (x1 <= x0 || y1 <= y0) return;
  const h = d.hair;
  out.push({ x: x0, y: y0, w: x1 - x0, h, color }, { x: x0, y: y1 - h, w: x1 - x0, h, color });
  if (box.x0 >= x0) out.push({ x: box.x0, y: y0, w: h, h: y1 - y0, color });
  if (box.x1 <= x1) out.push({ x: box.x1 - h, y: y0, w: h, h: y1 - y0, color });
}

/**
 * テイクのノート（テイクの 0 ティック目からの位置）を、曲の中の位置に。鳴るノート（始まりがクリップの範囲 [start, end) の中）だけ。
 * はしをつめて外に出たノートは描かない（終わりがはみ出す所は paintClip が切る）。
 */
export function takeShapes(
  clip: Pick<ClipView, "start" | "end" | "takeOffset">,
  take: {
    starts: ArrayLike<number>;
    lengths: ArrayLike<number>;
    keys: ArrayLike<number>;
  },
): NoteShape[] {
  const at = clip.start + clip.takeOffset;
  const out: NoteShape[] = [];
  for (let i = 0; i < take.starts.length; i++) {
    const start = at + take.starts[i]!;
    const length = take.lengths[i]!;
    if (start >= clip.end || start < clip.start) continue;
    out.push({ start, length, key: take.keys[i]! });
  }
  return out;
}

/** 録っているノートを形に。押している鍵（長さ null）は `now` まで。`from` より前に終わったノートは落とす。 */
export function liveShapes(notes: readonly LiveNote[], now: number, from = -Infinity): NoteShape[] {
  const out: NoteShape[] = [];
  for (const n of notes) {
    const length = n.length ?? Math.max(0, now - n.start);
    if (n.start + length < from) continue;
    out.push({ start: n.start, length, key: n.key });
  }
  return out;
}

/** 名前の前の札: なし、「2/3」、幅が足りないときは四角だけ。 */
export type TakeMark = "none" | "tag" | "square";

export function takeMark(clip: Pick<ClipView, "takes">, width: number, minWidth: number): TakeMark {
  if (clip.takes < 2) return "none";
  return width < minWidth ? "square" : "tag";
}

/** 1 ページぶん送る: 位置 `x`（曲の頭からの px）が見えるように、`width` きざみで動かす。 */
export function followScroll(x: number, scrollX: number, width: number): number {
  if (!(width > 0)) return scrollX;
  if (x >= scrollX && x < scrollX + width) return scrollX;
  return Math.max(0, scrollX + Math.floor((x - scrollX) / width) * width);
}

/** 小節の線の番号（1 から）。「5–9」の 5 と 9。 */
export function barLineAt(v: View, x: number, perBar: number): number {
  return Math.max(1, Math.round(xToTick(v, x) / perBar) + 1);
}

export interface BarRange {
  from: number;
  to: number;
}

/** ループの範囲（ティック）を小節の線の番号に。 */
export function loopBars(range: TickSpan | null, perBar: number): BarRange | null {
  if (!range) return null;
  return {
    from: Math.round(range.start / perBar) + 1,
    to: Math.round(range.end / perBar) + 1,
  };
}

/** 目盛のドラッグ。range: 押した所の線から新しく。edge: すでにある範囲の片はしを動かす（fixed は動かないほう）。 */
export type RulerDrag = { kind: "range"; anchor: number } | { kind: "edge"; fixed: number };

/** 押した所が範囲のはしから `grab` px 以内なら、そのはしをつかむ。 */
export function startRulerDrag(v: View, x: number, perBar: number, loop: BarRange | null, grab: number): RulerDrag {
  if (loop) {
    const lineX = (bar: number) => tickToX(v, (bar - 1) * perBar);
    const dFrom = Math.abs(x - lineX(loop.from));
    const dTo = Math.abs(x - lineX(loop.to));
    if (Math.min(dFrom, dTo) <= grab) return { kind: "edge", fixed: dFrom <= dTo ? loop.to : loop.from };
  }
  return { kind: "range", anchor: barLineAt(v, x, perBar) };
}

/** ドラッグ中の範囲。いちばん短くて 1 小節。 */
export function dragRange(drag: RulerDrag, v: View, x: number, perBar: number): BarRange {
  const fixed = drag.kind === "range" ? drag.anchor : drag.fixed;
  const at = barLineAt(v, x, perBar);
  if (at === fixed)
    return at > 1 && x < tickToX(v, (fixed - 1) * perBar)
      ? { from: at - 1, to: fixed }
      : { from: fixed, to: fixed + 1 };
  return { from: Math.min(at, fixed), to: Math.max(at, fixed) };
}

/**
 * 目盛を押したときに吸いつく線のきざみ（docs/design-notes.md B-61「目盛に見えている線」）。縮小して番号を間引いているときは
 * 番号のある小節おき、ふだんは 1 小節。拡大して 1 拍が minGap px（番号どうしの間と同じ）以上になれば拍。
 */
export function rulerStep(barPx: number, perBar: number, perBeat: number, labelEvery: number, minGap: number): number {
  if ((barPx * perBeat) / perBar >= minGap) return perBeat;
  return labelEvery * perBar;
}

/** 目盛を離したときの命令。動かしていなければ、押した所へ（step のきざみの近い線に吸いつく。Shift のときは step が null で吸いつかない）。 */
export function rulerCommand(
  drag: RulerDrag,
  v: View,
  downX: number,
  upX: number,
  perBar: number,
  still: number,
  step: number | null,
): Command {
  if (Math.abs(upX - downX) < still) {
    const at = xToTick(v, downX);
    return { type: "locate", tick: Math.max(0, step ? Math.round(at / step) * step : Math.round(at)) };
  }
  const r = dragRange(drag, v, upX, perBar);
  return { type: "setLoop", fromBar: r.from, toBar: r.to };
}

/** 押した所のクリップ（なければ null）。 */
export function clipAt(v: View, d: LaneDims, tracks: readonly TrackView[], x: number, y: number): number | null {
  const row = Math.floor(y / d.trackH);
  const track = tracks[row];
  if (!track) return null;
  for (const c of track.clips) {
    const b = clipBox(v, d, row, c.start, c.end);
    if (x >= b.x0 && x < b.x1 && y >= b.y0 && y < b.y1) return c.id;
  }
  return null;
}

/** 録っているクリップの、いまの周の頭。 */
export function lapStart(from: number, lap: number, loop: TickSpan | null): number {
  return lap > 0 && loop ? loop.start : from;
}

/**
 * 録っているクリップの分けかた。[lapFrom, commit) は大きな面（arrangeLanes）、[tailFrom, now] は小さな面（recordingClip）。
 * commit は小節ごとに進む。tailFrom は commit の 1 小節前からなので、大きな面が描きおわるまでの 1 コマも、すきまが出ない。
 */
export function recordingSplit(lapFrom: number, now: number, perBar: number): { commit: number; tailFrom: number } {
  const done = Math.max(0, Math.floor((now - lapFrom) / perBar));
  const commit = lapFrom + done * perBar;
  return { commit, tailFrom: Math.max(lapFrom, commit - perBar) };
}

/** 全体図の、トラックごとのクリップの線。高さ 40（size.overview）より高ければ、縦にのばす。 */
export function overviewRects(
  v: View,
  tracks: readonly TrackView[],
  height: number,
  base: { overviewH: number; lineH: number; gap: number; hair: number },
  recording: { track: number; from: number; now: number } | null,
): GlRect[] {
  const n = tracks.length;
  if (n === 0 || !(height > 0)) return [];
  const stretch = Math.max(1, height / base.overviewH);
  let stride = (base.lineH + base.gap) * stretch;
  let lineH = base.lineH * stretch;
  if (stride * n > height) {
    stride = height / n;
    lineH = Math.max(base.hair, stride - base.gap);
  }
  const top = (height - (stride * n - (stride - lineH))) / 2;
  const out: GlRect[] = [];
  tracks.forEach((track, i) => {
    const y = top + i * stride;
    const line = (start: number, end: number, color: ColorName) => {
      const x0 = Math.max(tickToX(v, start), 0);
      const x1 = Math.min(tickToX(v, end) - base.gap, v.width);
      if (x1 > x0) out.push({ x: x0, y, w: x1 - x0, h: lineH, color });
    };
    for (const c of track.clips) line(c.start, c.end, trackColor(track));
    if (recording && recording.track === track.number && recording.now > recording.from) {
      line(recording.from, recording.now, "color.rec");
    }
  });
  return out;
}

/** ホイールで横に動かした後のスクロール（px）。右へは `max` まで（ページ送りでそれより先にいれば、そこまで）。 */
export function wheelScroll(scrollX: number, dx: number, max: number): number {
  return Math.max(0, Math.min(scrollX + dx, Math.max(max, scrollX)));
}

/** 横にスクロールできる右のはし: 曲の中のいちばん後ろの物が、画面のまん中に来るまで。 */
export function scrollMax(
  tracks: readonly TrackView[],
  loop: TickSpan | null,
  position: number,
  v: Omit<View, "scrollX">,
): number {
  let end = Math.max(position, loop?.end ?? 0);
  for (const t of tracks) for (const c of t.clips) end = Math.max(end, c.end);
  return Math.max(0, end * v.pxPerTick - v.width / 2);
}

// ---- 横の拡大 ・ 縮小と、全体図の「見えている範囲の枠」（仕様書 3 章「第 3 回のための決まり」） ----

/** 目盛の番号を何小節おきに出すか（1・2・4・8 …）。番号どうしが minGap px より近くならないように。 */
export function labelEvery(barPx: number, minGap: number): number {
  let every = 1;
  while (every * barPx < minGap && every < 1024) every *= 2;
  return every;
}

/** Mod ＋ ホイールの、1 段ぶんの回した量（WheelEvent の px）。ふつうのマウスの 1 目盛りで 1 段になるように。 */
export const WHEEL_ZOOM_STEP = 50;

/**
 * Mod ＋ ホイールを段に。上へ回す（deltaY < 0）と拡大（+1）。1 回のできごとで動くのは 1 段まで。
 * 行や画面の単位（deltaMode 1・2）は 1 回で 1 段。px の単位（タッチパッドの細かい動き）は WHEEL_ZOOM_STEP たまるごとに 1 段。
 */
export function wheelZoomSteps(acc: number, deltaY: number, deltaMode: number): { steps: number; acc: number } {
  if (deltaY === 0) return { steps: 0, acc };
  if (deltaMode !== 0) return { steps: deltaY < 0 ? 1 : -1, acc: 0 };
  const next = Math.sign(acc) === Math.sign(deltaY) ? acc + deltaY : deltaY;
  if (Math.abs(next) < WHEEL_ZOOM_STEP) return { steps: 0, acc: next };
  return { steps: next < 0 ? 1 : -1, acc: 0 };
}

/**
 * 全体図の「見えている範囲の枠」（accent の 1px。上下は inset あける）。範囲は曲の中のティック。
 * 線は画面の画素にそろえる。見えている幅の外の辺は描かない。
 */
export function rangeFrameRects(
  v: View,
  range: { start: number; end: number },
  height: number,
  o: { inset: number; hair: number; dpr: number },
): GlRect[] {
  const snap = (x: number) => Math.round(x * o.dpr) / o.dpr;
  const x0 = snap(tickToX(v, range.start));
  const x1 = snap(tickToX(v, range.end));
  const y0 = o.inset;
  const y1 = height - o.inset;
  const l = Math.max(x0, 0);
  const r = Math.min(x1, v.width);
  if (r <= l || y1 - y0 < 2 * o.hair) return [];
  const color: ColorName = "color.accent";
  const out: GlRect[] = [
    { x: l, y: y0, w: r - l, h: o.hair, color },
    { x: l, y: y1 - o.hair, w: r - l, h: o.hair, color },
  ];
  if (x0 >= 0) out.push({ x: x0, y: y0, w: o.hair, h: y1 - y0, color });
  if (x1 <= v.width) out.push({ x: x1 - o.hair, y: y0, w: o.hair, h: y1 - y0, color });
  return out;
}

/**
 * 枠 [startPx, endPx)（曲の頭からの px）が全体図に入りきっていなければ、送った後の scrollX。入っていればそのまま。
 * 枠が全体図より広いときは、枠の左はしを全体図の左はしに。そうでなければ、枠をまん中に。
 */
export function revealRange(startPx: number, endPx: number, scrollX: number, width: number): number {
  if (!(width > 0)) return scrollX;
  if (startPx >= scrollX && endPx <= scrollX + width) return scrollX;
  const span = endPx - startPx;
  return Math.max(0, span >= width ? startPx : startPx - (width - span) / 2);
}
