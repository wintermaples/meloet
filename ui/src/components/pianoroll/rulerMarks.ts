// ピアノロールの目盛のマーカーと拍の数字（仕様書 3 章、モック R11-Answers92）。DOM を知らない。

/**
 * 出す拍の数字: マーカーの名前（地 gray.3 で数字の上に重ねる）がかかる数字は出さない。幅は CSS px で、まだはかっていない物は 0。
 * 小節の頭の数字にかかるときも、名前のほうを出す。
 */
export function labelsBesideMarkers<L extends { tick: number; x: number }, M extends { id: number; x: number }>(
  labels: readonly L[],
  markers: readonly M[],
  labelW: Readonly<Record<number, number>>,
  markerW: Readonly<Record<number, number>>,
): L[] {
  return labels.filter((l) => !markers.some((m) => l.x < m.x + (markerW[m.id] ?? 0) && l.x + (labelW[l.tick] ?? 0) > m.x));
}
