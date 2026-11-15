// 状態行の右がわの入れかた（仕様書 3 章「状態行の右がわは、横が足りなければ…」）。DOM を知らない純粋な関数。
// 幅はどれも、いまの率での CSS px。

export const STATUS_RIGHT_ORDER = ["inputs", "output", "buffer"] as const;
export type StatusRightPart = (typeof STATUS_RIGHT_ORDER)[number];
export type StatusRightShown = Record<StatusRightPart, boolean>;

export interface StatusFitInput {
  /** 行の中で使える幅（内がわをのぞく）。 */
  available: number;
  /** 物と物のすきま。 */
  gap: number;
  /** 左の物（「■ 停止中」、文、ボタン）を、ちぢめずにならべた幅（あいだのすきまを入れて）。 */
  left: number;
  /** 右がわの物の幅。出ていない物は入れない。 */
  right: Partial<Record<StatusRightPart, number>>;
  /** 拡大率のボタンの幅。いつも出す。 */
  scale: number;
}

/**
 * 右がわのどれを出すか。入口 → 出口 → バッファ の順に引っこめる。拡大率は消さない。
 * それでも入らなければ、画面が左の文のうしろを「…」でちぢめる。
 */
export function statusFit({ available, gap, left, right, scale }: StatusFitInput): StatusRightShown {
  const shown: StatusRightShown = { inputs: true, output: true, buffer: true };
  // 左、すきま、伸びる空き、すきま、右の物とすきま …、拡大率。
  let need = left + 2 * gap + scale;
  for (const k of STATUS_RIGHT_ORDER) if (right[k] !== undefined) need += right[k] + gap;
  for (const k of STATUS_RIGHT_ORDER) {
    if (need <= available) break;
    if (right[k] === undefined) continue;
    shown[k] = false;
    need -= right[k] + gap;
  }
  return shown;
}
