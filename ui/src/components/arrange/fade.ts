// 歌のクリップのフェードの形（仕様書 3 章 27-A「線は曲がった形 1 つ（大きさがへこまない形）」）。
// Rust の `crates/meloet-model/src/audio/fade.rs` の写し。エンジンが音にかける形と、画面の線 ・ 波形の形が同じになる。
// 同じ答えかは、Rust の試験が書く表 `fade.table.json` で見る（tests/arrange.fade.test.ts）。

/** フェードインの、長さの x の所（0 = 頭、1 = 終わり）の大きさ。フェードアウトは `fadeGain(1 - x)`。 */
export function fadeGain(x: number): number {
  if (x >= 1) return 1;
  if (x > 0) return Math.sin((x * Math.PI) / 2);
  return 0;
}

/**
 * クリップの頭から `at` の所で、2 つのフェードがかける大きさ。数は、どれも同じ単位（ティックでも px でも）。
 * クリップの外は 0。クリップの音量は入っていない。
 */
export function clipFadeGain(at: number, length: number, fadeIn: number, fadeOut: number): number {
  if (!(at >= 0 && at < length)) return 0;
  const goingIn = fadeIn > 0 ? fadeGain(at / fadeIn) : 1;
  const goingOut = fadeOut > 0 ? fadeGain((length - at) / fadeOut) : 1;
  return goingIn * goingOut;
}
