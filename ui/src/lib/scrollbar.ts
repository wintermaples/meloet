// スクロールの棒の計算（仕様書 2 章「スクロールの棒」）。DOM を知らない純粋な関数。

export interface ScrollBox {
  /** 見えている高さ（clientHeight）。 */
  view: number;
  /** 中身の高さ（scrollHeight）。 */
  content: number;
  /** いまの位置（scrollTop）。 */
  top: number;
}

export interface Thumb {
  /** つまみの長さと、棒の上はしからの位置（棒の長さは view と同じ）。 */
  length: number;
  offset: number;
}

/** 中身が入りきるときは null（棒を出さない）。1px 未満のはみ出しは入りきると見る（拡大率の端数）。 */
export function thumbOf(box: ScrollBox, minLength: number): Thumb | null {
  const { view, content } = box;
  if (view <= 0 || content - view < 1) return null;
  const length = Math.min(view, Math.max(minLength, (view * view) / content));
  const range = content - view;
  const top = Math.min(Math.max(box.top, 0), range);
  return { length, offset: (top / range) * (view - length) };
}

/** つまみを dy 動かしたときの、新しい scrollTop。startTop はつかんだときの scrollTop。 */
export function dragTo(box: ScrollBox, startTop: number, dy: number, minLength: number): number {
  const thumb = thumbOf(box, minLength);
  if (!thumb) return 0;
  const range = box.content - box.view;
  const track = box.view - thumb.length;
  if (track <= 0) return startTop;
  return Math.min(Math.max(startTop + (dy * range) / track, 0), range);
}

/** 棒のつまみの外を押したとき: その向きに 1 画面ぶん動かす。y は棒の上はしからの位置。 */
export function pageTo(box: ScrollBox, y: number, minLength: number): number {
  const thumb = thumbOf(box, minLength);
  if (!thumb) return 0;
  const range = box.content - box.view;
  const next = y < thumb.offset ? box.top - box.view : box.top + box.view;
  return Math.min(Math.max(next, 0), range);
}
