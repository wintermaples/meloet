// ピアノロールの、画面だけが持つ状態（どこを見ているか、どの道具か）。Rust に送らない見た目の物（アレンジの横のスクロールと同じ）。
// 帯をたたんでも覚えておく（仕様書 3 章「帯をたたんでも中身の状態（選んだノート、スクロールの位置、ひらいていた CC の帯）は覚えている」）ので、
// 部品の外のこのモジュールに置く。アレンジの全体図の「見えている範囲の枠」も、ここを読む。
//
// 位置はティック（4 分音符 = 480）。幅は拡大率 100 % の CSS px で持ち、描く所で拡大率をかける。

import { TICKS_PER_QUARTER } from "../time";

export type Tool = "select" | "draw" | "erase";

/** 横の拡大の段（拡大率 100 % で、4 分音符 1 つの px）。80 はモック A-PianoRoll（1 拍 80 px）。 */
export const QUARTER_PX_STEPS = [10, 14, 20, 28, 40, 57, 80, 113, 160, 226, 320] as const;
export const DEFAULT_QUARTER_PX = 80;

export const prView = $state({
  tool: "select" as Tool,
  /** 拡大率 100 % で、4 分音符 1 つの px（QUARTER_PX_STEPS のどれか）。 */
  quarterPx: DEFAULT_QUARTER_PX as number,
  /** ノートの場所の左はしの、曲の中のティック。 */
  scrollTick: 0,
  /** ノートの場所のいちばん上の行の鍵（MIDI 0〜127）。 */
  topKey: 84,
  /** ノートの場所の幅と高さ（CSS px、拡大率をかけたあと）。ノートの場所の部品が入れる。 */
  areaWidth: 0,
  areaHeight: 0,
  /** 再生の線を追って横に送るか。手で横に動かしたら、次に再生を始めるまで追わない（仕様書 3 章、アレンジと同じ）。 */
  follow: true,
  /** いま見せているクリップ（ひらいたときに、そのクリップのはじまりまで送るため）。 */
  shownClip: null as number | null,
  /** Delete・Mod A・Mod D・切り取りなどを、ピアノロールとアレンジのどちらに向けるか。最後に押した場所。 */
  focus: "arrange" as "arrange" | "pianoRoll",
});

/** 拡大率 scale（100・125・150）での、1 ティックの CSS px。 */
export function pxPerTick(scale: number): number {
  return (prView.quarterPx * scale) / 100 / TICKS_PER_QUARTER;
}

/** ノートの場所で見えている範囲 [start, end)（ティック）。全体図の枠はこれを描く。 */
export function visibleTicks(scale: number): { start: number; end: number } {
  const ppt = pxPerTick(scale);
  return { start: prView.scrollTick, end: ppt > 0 ? prView.scrollTick + prView.areaWidth / ppt : prView.scrollTick };
}

/** 手で横に動かした（全体図の枠のドラッグ、横の棒、Shift ＋ ホイール）。 */
export function scrollToTick(tick: number): void {
  prView.scrollTick = Math.max(0, tick);
  prView.follow = false;
}

/** 横の拡大・縮小（Mod ＋ ・ Mod −、− ＋ のボタン、Mod ＋ ホイール）を 1 段。anchorTick の位置が画面の同じ所に残るように送る（なければ左はし）。 */
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

// ---- ドラッグ中の読み値（仕様書 2 章「その場の読み値」: 同じ値を状態行にも出す） ----

/** ドラッグしているあいだだけ、動かしている値の文（「+0.240 ・ +2 半音」など）。状態行は、これがあれば core の文の代わりに出す。 */
export const dragReading = $state({ text: null as string | null });

// ---- Esc の順のいちばん前の「ドラッグをやめる」（仕様書 4 章「ドラッグをやめて、動かす前に戻す Esc」） ----

let cancelers: Array<() => boolean> = [];

/** ドラッグを持つ部品が登録する。返り値の関数で外す。canceler は、ドラッグ中ならやめて true、そうでなければ false を返す。 */
export function onDragCancel(canceler: () => boolean): () => void {
  cancelers.push(canceler);
  return () => {
    cancelers = cancelers.filter((c) => c !== canceler);
  };
}

/** Esc のとき、キーの係がいちばん先に呼ぶ。どれかのドラッグをやめたら true（そのときは、ほかのことをしない）。 */
export function cancelDrag(): boolean {
  for (const c of cancelers) if (c()) return true;
  return false;
}

// ---- アレンジの横の拡大（仕様書 3 章「横の拡大 ・ 縮小」）。キーの係（Mod ＋ ・ Mod −）とアレンジの部品の両方が使うので、ここに置く ----

/**
 * アレンジの横の拡大の段（拡大率 100 % で、1 小節の px）。64 はモック F-Empty（mock.arrange.bar）。
 * ピアノロールの段（QUARTER_PX_STEPS）と同じ √2 きざみ。下は 8（120 小節が 960 px に入る）、上は 256（1 拍 64 px。ここより細かくはピアノロールで）。
 */
export const ARRANGE_BAR_STEPS = [8, 11, 16, 23, 32, 45, 64, 91, 128, 181, 256] as const;
export const DEFAULT_ARRANGE_BAR = 64;

export const arrangeView = $state({
  /** 拡大率 100 % で、1 小節の px（ARRANGE_BAR_STEPS のどれか）。 */
  barPx: DEFAULT_ARRANGE_BAR as number,
});

/** 段の並び steps の中で、now から by 段動かした値（はしで止まる。now が並びになければ fallback から）。 */
export function stepFrom(steps: readonly number[], now: number, by: number, fallback: number): number {
  const at = steps.indexOf(now);
  const from = at < 0 ? steps.indexOf(fallback) : at;
  return steps[Math.min(steps.length - 1, Math.max(0, from + by))]!;
}

/** アレンジの横を by 段。変わったら true。スクロールの位置はティックで持っているので、左はしの位置はそのまま残る。 */
export function zoomArrangeBy(by: number): boolean {
  const next = stepFrom(ARRANGE_BAR_STEPS, arrangeView.barPx, by, DEFAULT_ARRANGE_BAR);
  if (next === arrangeView.barPx) return false;
  arrangeView.barPx = next;
  return true;
}
